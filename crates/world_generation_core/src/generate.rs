use crate::hex::{BoundaryKey, HexCoord, HexDirection};
use crate::model::*;
use crate::validate::{ValidationReport, validate_world};
use deterministic_rng::{DeterministicRng, SeedMaterial};
use glam::{DVec2, DVec3};
use scroll_camera_core::ScrollGrammar;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use thiserror::Error;
use world_ids::{
    BuildingInstanceId, CellId, LandmarkId, PortalId, RiverId, RoadId, RouteId, SettlementId,
};

const STAGE_TERRAIN: u64 = 0x1001;
const STAGE_CLIMATE: u64 = 0x1002;
const FEATURE_CELL: u128 = 0x01;
const FEATURE_ELEVATION: u128 = 0x02;
const FEATURE_CLIMATE: u128 = 0x03;

#[derive(Clone, Debug)]
struct CellDraft {
    id: CellId,
    coord: HexCoord,
    center_xz: DVec2,
    elevation_m: f64,
    surface: SurfaceClass,
    climate: Climate,
    biome: Biome,
    travel_cost: f64,
    downstream: Option<HexCoord>,
    edge_elevations_m: [f64; 6],
}

#[derive(Debug, Error)]
pub enum WorldGenerationError {
    #[error("world generation configuration is invalid")]
    InvalidConfig,
    #[error("land cell {0:?} has no deterministic downstream neighbor")]
    MissingDownstream(HexCoord),
    #[error("hydrology cycle detected at {0:?}")]
    HydrologyCycle(HexCoord),
    #[error("main river did not produce a coastal land cell")]
    MissingPortCell,
    #[error("road path from {start:?} to {end:?} cannot be constructed")]
    MissingRoadPath { start: HexCoord, end: HexCoord },
    #[error("world manifest serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Validation(#[from] ValidationReport),
}

pub fn generate_world(
    config: WorldGenerationConfig,
) -> Result<WorldCompilation, WorldGenerationError> {
    generate_world_with_order(config, TraversalOrder::Canonical)
}

pub fn generate_world_with_order(
    config: WorldGenerationConfig,
    traversal: TraversalOrder,
) -> Result<WorldCompilation, WorldGenerationError> {
    if !config.validate() {
        return Err(WorldGenerationError::InvalidConfig);
    }

    let canonical_coords = HexCoord::disk(config.radius);
    let coord_set = canonical_coords.iter().copied().collect::<BTreeSet<_>>();
    let mut traversal_coords = canonical_coords.clone();
    match traversal {
        TraversalOrder::Canonical => {}
        TraversalOrder::Reverse => traversal_coords.reverse(),
        TraversalOrder::Parity => traversal_coords
            .sort_by_key(|coord| (i32::from(coord.q + coord.r).rem_euclid(2), coord.q, coord.r)),
    }

    let mut drafts = BTreeMap::new();
    for coord in traversal_coords {
        drafts.insert(coord, generate_cell(config, coord));
    }

    assign_downstream(&coord_set, &mut drafts)?;
    assign_edge_elevations(&coord_set, &mut drafts);

    let river = generate_river(config, &drafts)?;
    let coastline = generate_coastline(&coord_set, &drafts);
    let port_cell = river
        .cells
        .iter()
        .rev()
        .copied()
        .find(|coord| drafts.get(coord).is_some_and(|cell| !cell.is_ocean()))
        .ok_or(WorldGenerationError::MissingPortCell)?;

    let roads = generate_roads(config, &coord_set, &drafts, port_cell)?;
    let landmark = generate_landmark(config, &drafts);
    let routes = generate_routes(config, &drafts, &roads, port_cell, landmark.id);
    let settlement = generate_settlement(config, port_cell, &routes);
    let portals = generate_portals(config, &drafts, &river, &roads, &routes);
    let cells = finalize_cells(&drafts, &roads, &routes, &portals);

    let stage_digests = vec![
        GenerationStageDigest {
            stage: "terrain-climate".into(),
            digest: digest_serializable(&cells)?,
        },
        GenerationStageDigest {
            stage: "hydrology-coast".into(),
            digest: digest_serializable(&(river.clone(), coastline.clone()))?,
        },
        GenerationStageDigest {
            stage: "transport".into(),
            digest: digest_serializable(&roads)?,
        },
        GenerationStageDigest {
            stage: "settlement-landmark".into(),
            digest: digest_serializable(&(settlement.clone(), landmark.clone()))?,
        },
        GenerationStageDigest {
            stage: "scroll-routes-portals".into(),
            digest: digest_serializable(&(routes.clone(), portals.clone()))?,
        },
    ];

    let manifest = WorldManifest {
        world_id: config.world_id,
        world_seed: config.world_seed,
        generator_version: config.generator_version,
        radius: config.radius,
        cell_radius_m: config.cell_radius_m,
        cells,
        river,
        coastline,
        roads,
        routes,
        portals,
        settlements: vec![settlement],
        landmarks: vec![landmark],
        stage_digests,
    };

    validate_world(&manifest)?;
    let semantic_fingerprint = semantic_fingerprint(&manifest)?;
    Ok(WorldCompilation {
        manifest,
        semantic_fingerprint,
    })
}

pub fn semantic_fingerprint(manifest: &WorldManifest) -> Result<u64, serde_json::Error> {
    digest_serializable(manifest)
}

fn generate_cell(config: WorldGenerationConfig, coord: HexCoord) -> CellDraft {
    let spatial_key = coord_key(coord);
    let elevation_noise = unit_noise(config, STAGE_TERRAIN, spatial_key, FEATURE_ELEVATION);
    let climate_noise = unit_noise(config, STAGE_CLIMATE, spatial_key, FEATURE_CLIMATE);
    let center_xz = coord.center_xz(config.cell_radius_m);

    let elevation_m = if coord.q == config.radius {
        -12.0 - elevation_noise * 6.0
    } else if coord.q == config.radius - 1 {
        32.0 + (elevation_noise - 0.5) * 8.0
    } else {
        let inland_steps = f64::from(config.radius - coord.q - 1).max(1.0);
        let coastal_gradient = 32.0 + inland_steps * 30.0;
        let ridge_axis = f64::from(coord.r) + f64::from(coord.q) * 0.35;
        let ridge = 45.0 * (-0.72 * ridge_axis * ridge_axis).exp();
        coastal_gradient + ridge + (elevation_noise - 0.5) * 8.0
    };

    let surface = if coord.q == config.radius {
        SurfaceClass::Ocean
    } else if coord.q == config.radius - 1 {
        SurfaceClass::Coast
    } else if elevation_m >= 155.0 {
        SurfaceClass::Alpine
    } else if elevation_m >= 105.0 {
        SurfaceClass::Highland
    } else {
        SurfaceClass::Lowland
    };

    let distance_from_ocean = f64::from(config.radius - coord.q).max(0.0);
    let temperature_c = if surface == SurfaceClass::Ocean {
        15.0 - f64::from(coord.r.abs()) * 0.6
    } else {
        21.0 - f64::from(coord.r.abs()) * 1.2
            - elevation_m.max(0.0) * 0.0075
            - distance_from_ocean * 0.25
    };
    let moisture =
        (0.9 - distance_from_ocean * 0.075 + (climate_noise - 0.5) * 0.14).clamp(0.2, 1.0);
    let climate = Climate {
        temperature_c,
        moisture,
    };
    let biome = classify_biome(surface, elevation_m, climate);
    let travel_cost = match surface {
        SurfaceClass::Ocean => 12.0,
        SurfaceClass::Coast => 1.15,
        SurfaceClass::Lowland => 1.0 + elevation_m.max(0.0) / 1_000.0,
        SurfaceClass::Highland => 1.5 + elevation_m / 600.0,
        SurfaceClass::Alpine => 2.2 + elevation_m / 500.0,
    };

    CellDraft {
        id: CellId::from_u128(stable_value(config, 0xC311, spatial_key, FEATURE_CELL)),
        coord,
        center_xz,
        elevation_m,
        surface,
        climate,
        biome,
        travel_cost,
        downstream: None,
        edge_elevations_m: [elevation_m; 6],
    }
}

fn classify_biome(surface: SurfaceClass, elevation_m: f64, climate: Climate) -> Biome {
    match surface {
        SurfaceClass::Ocean => Biome::Ocean,
        SurfaceClass::Coast if climate.moisture >= 0.78 => Biome::Wetland,
        SurfaceClass::Coast => Biome::CoastalGrassland,
        SurfaceClass::Alpine => Biome::AlpineRock,
        SurfaceClass::Highland => Biome::UplandMeadow,
        SurfaceClass::Lowland if elevation_m < 180.0 && climate.moisture >= 0.8 => Biome::Wetland,
        SurfaceClass::Lowland if climate.moisture >= 0.58 => Biome::TemperateForest,
        SurfaceClass::Lowland => Biome::CoastalGrassland,
    }
}

fn assign_downstream(
    coord_set: &BTreeSet<HexCoord>,
    drafts: &mut BTreeMap<HexCoord, CellDraft>,
) -> Result<(), WorldGenerationError> {
    let coords = drafts.keys().copied().collect::<Vec<_>>();
    for coord in coords {
        if drafts.get(&coord).is_some_and(CellDraft::is_ocean) {
            continue;
        }
        let current_elevation = drafts[&coord].elevation_m;
        let mut candidates = HexDirection::ALL
            .into_iter()
            .map(|direction| coord.neighbor(direction))
            .filter(|neighbor| coord_set.contains(neighbor))
            .filter(|neighbor| neighbor.q > coord.q)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            candidates = HexDirection::ALL
                .into_iter()
                .map(|direction| coord.neighbor(direction))
                .filter(|neighbor| coord_set.contains(neighbor))
                .filter(|neighbor| drafts[neighbor].elevation_m < current_elevation)
                .collect();
        }
        candidates.sort_by(|left, right| {
            drafts[left]
                .elevation_m
                .total_cmp(&drafts[right].elevation_m)
                .then_with(|| left.cmp(right))
        });
        let downstream = candidates
            .into_iter()
            .next()
            .ok_or(WorldGenerationError::MissingDownstream(coord))?;
        drafts.get_mut(&coord).expect("cell exists").downstream = Some(downstream);
    }
    Ok(())
}

fn assign_edge_elevations(
    coord_set: &BTreeSet<HexCoord>,
    drafts: &mut BTreeMap<HexCoord, CellDraft>,
) {
    let elevations = drafts
        .iter()
        .map(|(coord, cell)| (*coord, cell.elevation_m))
        .collect::<BTreeMap<_, _>>();
    for cell in drafts.values_mut() {
        for direction in HexDirection::ALL {
            let neighbor = cell.coord.neighbor(direction);
            cell.edge_elevations_m[direction.index()] = if coord_set.contains(&neighbor) {
                (cell.elevation_m + elevations[&neighbor]) * 0.5
            } else {
                cell.elevation_m
            };
        }
    }
}

fn generate_river(
    config: WorldGenerationConfig,
    drafts: &BTreeMap<HexCoord, CellDraft>,
) -> Result<RiverManifest, WorldGenerationError> {
    let source = drafts
        .values()
        .filter(|cell| !cell.is_ocean() && cell.coord.q == -config.radius)
        .max_by(|left, right| {
            let left_score = left.elevation_m + left.climate.moisture * 80.0;
            let right_score = right.elevation_m + right.climate.moisture * 80.0;
            left_score
                .total_cmp(&right_score)
                .then_with(|| right.coord.cmp(&left.coord))
        })
        .map(|cell| cell.coord)
        .ok_or(WorldGenerationError::MissingPortCell)?;

    let mut cells = vec![source];
    let mut visited = BTreeSet::from([source]);
    let mut current = source;
    while !drafts[&current].is_ocean() {
        let next = drafts[&current]
            .downstream
            .ok_or(WorldGenerationError::MissingDownstream(current))?;
        if !visited.insert(next) {
            return Err(WorldGenerationError::HydrologyCycle(next));
        }
        cells.push(next);
        current = next;
    }
    let boundaries = path_boundaries(&cells);
    Ok(RiverManifest {
        id: RiverId::from_u128(stable_value(config, 0xA101, coord_key(source), 1)),
        cells,
        boundaries,
    })
}

fn generate_coastline(
    coord_set: &BTreeSet<HexCoord>,
    drafts: &BTreeMap<HexCoord, CellDraft>,
) -> Vec<BoundaryKey> {
    let mut coastline = BTreeSet::new();
    for cell in drafts.values().filter(|cell| !cell.is_ocean()) {
        for direction in HexDirection::ALL {
            let neighbor = cell.coord.neighbor(direction);
            if coord_set.contains(&neighbor) && drafts[&neighbor].is_ocean() {
                coastline.insert(
                    BoundaryKey::new(cell.coord, neighbor)
                        .expect("coastline neighbors share an edge"),
                );
            }
        }
    }
    coastline.into_iter().collect()
}

fn generate_roads(
    config: WorldGenerationConfig,
    coord_set: &BTreeSet<HexCoord>,
    drafts: &BTreeMap<HexCoord, CellDraft>,
    port_cell: HexCoord,
) -> Result<Vec<RoadManifest>, WorldGenerationError> {
    let land = drafts
        .values()
        .filter(|cell| !cell.is_ocean())
        .map(|cell| cell.coord)
        .collect::<Vec<_>>();

    let western = land
        .iter()
        .copied()
        .filter(|coord| coord.q == -config.radius)
        .min_by_key(|coord| (coord.r.abs(), *coord))
        .expect("radius world has western land");
    let northern = land
        .iter()
        .copied()
        .min_by_key(|coord| (coord.r, coord.q))
        .expect("world has northern land");
    let southern = land
        .iter()
        .copied()
        .max_by_key(|coord| (coord.r, -coord.q))
        .expect("world has southern land");

    let specs = [
        (RoadPurpose::WesternApproach, western),
        (RoadPurpose::NorthernApproach, northern),
        (RoadPurpose::SouthernApproach, southern),
    ];
    let mut roads = Vec::new();
    for (index, (purpose, start)) in specs.into_iter().enumerate() {
        let cells = slope_aware_land_path(start, port_cell, coord_set, drafts).ok_or(
            WorldGenerationError::MissingRoadPath {
                start,
                end: port_cell,
            },
        )?;
        roads.push(RoadManifest {
            id: RoadId::from_u128(stable_value(
                config,
                0xB201,
                coord_key(start),
                index as u128 + 1,
            )),
            purpose,
            boundaries: path_boundaries(&cells),
            cells,
        });
    }
    roads.sort_by_key(|road| road.id);
    Ok(roads)
}

fn slope_aware_land_path(
    start: HexCoord,
    end: HexCoord,
    coord_set: &BTreeSet<HexCoord>,
    drafts: &BTreeMap<HexCoord, CellDraft>,
) -> Option<Vec<HexCoord>> {
    let mut frontier = BTreeSet::from([start]);
    let mut costs = BTreeMap::from([(start, 0.0_f64)]);
    let mut previous = BTreeMap::<HexCoord, HexCoord>::new();

    while !frontier.is_empty() {
        let current = *frontier.iter().min_by(|left, right| {
            costs[*left]
                .total_cmp(&costs[*right])
                .then_with(|| left.cmp(right))
        })?;
        frontier.remove(&current);
        if current == end {
            break;
        }

        for direction in HexDirection::ALL {
            let neighbor = current.neighbor(direction);
            if !coord_set.contains(&neighbor) || drafts[&neighbor].is_ocean() {
                continue;
            }

            let horizontal_distance = drafts[&current]
                .center_xz
                .distance(drafts[&neighbor].center_xz)
                .max(1.0);
            let grade = (drafts[&current].elevation_m - drafts[&neighbor].elevation_m).abs()
                / horizontal_distance;
            let edge_cost = 1.0 + drafts[&neighbor].travel_cost * 0.15 + grade.powi(2) * 60.0;
            let candidate_cost = costs[&current] + edge_cost;
            let incumbent = costs.get(&neighbor).copied().unwrap_or(f64::INFINITY);
            let predecessor_is_better = previous
                .get(&neighbor)
                .is_none_or(|incumbent_predecessor| current < *incumbent_predecessor);

            if candidate_cost < incumbent - 1.0e-12
                || ((candidate_cost - incumbent).abs() <= 1.0e-12 && predecessor_is_better)
            {
                costs.insert(neighbor, candidate_cost);
                previous.insert(neighbor, current);
                frontier.insert(neighbor);
            }
        }
    }

    if !costs.contains_key(&end) {
        return None;
    }
    let mut path = vec![end];
    let mut current = end;
    while current != start {
        current = *previous.get(&current)?;
        path.push(current);
    }
    path.reverse();
    Some(path)
}

fn generate_landmark(
    config: WorldGenerationConfig,
    drafts: &BTreeMap<HexCoord, CellDraft>,
) -> LandmarkManifest {
    let cell = drafts
        .values()
        .filter(|cell| !cell.is_ocean())
        .max_by(|left, right| {
            left.elevation_m
                .total_cmp(&right.elevation_m)
                .then_with(|| right.coord.cmp(&left.coord))
        })
        .expect("world has land");
    LandmarkManifest {
        id: LandmarkId::from_u128(stable_value(config, 0xD301, coord_key(cell.coord), 1)),
        kind: LandmarkKind::MountainTower,
        cell: cell.coord,
        world_position: DVec3::new(cell.center_xz.x, cell.elevation_m + 90.0, cell.center_xz.y),
        visible_radius_m: config.cell_radius_m * 8.0,
    }
}

fn generate_routes(
    config: WorldGenerationConfig,
    drafts: &BTreeMap<HexCoord, CellDraft>,
    roads: &[RoadManifest],
    port_cell: HexCoord,
    landmark_id: LandmarkId,
) -> Vec<ScrollRouteManifest> {
    let mut routes = roads
        .iter()
        .enumerate()
        .map(|(index, road)| ScrollRouteManifest {
            id: RouteId::from_u128(stable_value(
                config,
                0xE401,
                road.id.as_u128(),
                index as u128 + 1,
            )),
            road_id: road.id,
            cells: road.cells.clone(),
            nodes: route_nodes(drafts, &road.cells, port_cell),
            visible_landmark_ids: vec![landmark_id],
        })
        .collect::<Vec<_>>();
    routes.sort_by_key(|route| route.id);
    routes
}

fn route_nodes(
    drafts: &BTreeMap<HexCoord, CellDraft>,
    cells: &[HexCoord],
    port_cell: HexCoord,
) -> Vec<RouteNode> {
    let mut nodes = Vec::new();
    for (index, coord) in cells.iter().copied().enumerate() {
        let cell = &drafts[&coord];
        let grammar = center_grammar(drafts, cells, index, port_cell);
        nodes.push(RouteNode {
            cell: coord,
            world_position: DVec3::new(cell.center_xz.x, cell.elevation_m + 1.0, cell.center_xz.y),
            grammar,
        });
        if let Some(next) = cells.get(index + 1).copied() {
            let next_cell = &drafts[&next];
            let next_turns = if index + 2 < cells.len() {
                HexDirection::between(coord, next) != HexDirection::between(next, cells[index + 2])
            } else {
                false
            };
            let boundary_grammar = if next == port_cell {
                ScrollGrammar::JunctionApproach
            } else if next_turns {
                ScrollGrammar::CameraReorientation
            } else {
                ScrollGrammar::LightDepth
            };
            nodes.push(RouteNode {
                cell: coord,
                world_position: DVec3::new(
                    (cell.center_xz.x + next_cell.center_xz.x) * 0.5,
                    (cell.elevation_m + next_cell.elevation_m) * 0.5 + 1.0,
                    (cell.center_xz.y + next_cell.center_xz.y) * 0.5,
                ),
                grammar: boundary_grammar,
            });
        }
    }
    nodes
}

fn center_grammar(
    drafts: &BTreeMap<HexCoord, CellDraft>,
    cells: &[HexCoord],
    index: usize,
    port_cell: HexCoord,
) -> ScrollGrammar {
    let coord = cells[index];
    if coord == port_cell {
        return ScrollGrammar::JunctionDecision;
    }
    if index == 0 {
        return ScrollGrammar::VistaReveal;
    }
    if let Some(next) = cells.get(index + 1).copied() {
        if (drafts[&coord].elevation_m - drafts[&next].elevation_m).abs() >= 28.0 {
            return ScrollGrammar::VerticalTransition;
        }
    }
    if index + 1 < cells.len() {
        let previous = cells[index - 1];
        let next = cells[index + 1];
        if HexDirection::between(previous, coord) != HexDirection::between(coord, next) {
            return ScrollGrammar::TurnCommit;
        }
    }
    ScrollGrammar::StandardSideView
}

fn generate_settlement(
    config: WorldGenerationConfig,
    port_cell: HexCoord,
    routes: &[ScrollRouteManifest],
) -> SettlementManifest {
    let entry_route_id = routes
        .first()
        .expect("P4 always generates approach routes")
        .id;
    let offsets = [
        (-22.0, -15.0, 0.0),
        (0.0, -15.0, 0.0),
        (22.0, -15.0, 0.0),
        (-22.0, 15.0, PI),
        (0.0, 15.0, PI),
        (22.0, 15.0, PI),
    ];
    let buildings = offsets
        .into_iter()
        .enumerate()
        .map(|(index, (x, z, yaw))| BuildingPlacement {
            instance_id: BuildingInstanceId::from_u128(stable_value(
                config,
                0xF501,
                coord_key(port_cell),
                index as u128 + 1,
            )),
            blueprint_id: config.building_blueprint_id,
            local_position: DVec3::new(x, 0.0, z),
            yaw_radians: yaw,
            half_extents_m: DVec3::new(6.3, 4.3, 4.3),
            entry_route_id,
        })
        .collect();

    SettlementManifest {
        id: SettlementId::from_u128(stable_value(config, 0xF502, coord_key(port_cell), 1)),
        kind: SettlementKind::PortTown,
        cell: port_cell,
        reasons: vec![
            SettlementReason::FreshWater,
            SettlementReason::ShelteredCoast,
            SettlementReason::BuildableSlope,
            SettlementReason::RoadConvergence,
        ],
        buildings,
    }
}

fn generate_portals(
    config: WorldGenerationConfig,
    drafts: &BTreeMap<HexCoord, CellDraft>,
    river: &RiverManifest,
    roads: &[RoadManifest],
    routes: &[ScrollRouteManifest],
) -> Vec<BoundaryPortal> {
    let mut entries = BTreeMap::<(BoundaryKey, u8), PortalKind>::new();
    for boundary in &river.boundaries {
        entries.insert((*boundary, 0), PortalKind::River);
    }
    for road in roads {
        for boundary in &road.boundaries {
            entries.insert((*boundary, 1), PortalKind::Road);
        }
    }
    for route in routes {
        for boundary in path_boundaries(&route.cells) {
            entries.insert((boundary, 2), PortalKind::ScrollRoute);
        }
    }

    entries
        .into_iter()
        .map(|((boundary, kind_key), kind)| {
            let low = &drafts[&boundary.low];
            let high = &drafts[&boundary.high];
            let vertical_offset = match kind {
                PortalKind::River => -0.5,
                PortalKind::Road => 0.1,
                PortalKind::ScrollRoute => 0.35,
            };
            BoundaryPortal {
                id: PortalId::from_u128(stable_value(
                    config,
                    0xAA01 + u64::from(kind_key),
                    boundary_material(boundary),
                    u128::from(kind_key) + 1,
                )),
                boundary,
                kind,
                world_position: DVec3::new(
                    (low.center_xz.x + high.center_xz.x) * 0.5,
                    (low.elevation_m + high.elevation_m) * 0.5 + vertical_offset,
                    (low.center_xz.y + high.center_xz.y) * 0.5,
                ),
            }
        })
        .collect()
}

fn finalize_cells(
    drafts: &BTreeMap<HexCoord, CellDraft>,
    roads: &[RoadManifest],
    routes: &[ScrollRouteManifest],
    portals: &[BoundaryPortal],
) -> Vec<AtlasCellManifest> {
    let mut road_ids = BTreeMap::<HexCoord, BTreeSet<RoadId>>::new();
    let mut route_ids = BTreeMap::<HexCoord, BTreeSet<RouteId>>::new();
    let mut portal_ids = BTreeMap::<HexCoord, BTreeSet<PortalId>>::new();
    for road in roads {
        for coord in &road.cells {
            road_ids.entry(*coord).or_default().insert(road.id);
        }
    }
    for route in routes {
        for coord in &route.cells {
            route_ids.entry(*coord).or_default().insert(route.id);
        }
    }
    for portal in portals {
        portal_ids
            .entry(portal.boundary.low)
            .or_default()
            .insert(portal.id);
        portal_ids
            .entry(portal.boundary.high)
            .or_default()
            .insert(portal.id);
    }

    drafts
        .values()
        .map(|cell| AtlasCellManifest {
            id: cell.id,
            coord: cell.coord,
            center_world: DVec3::new(cell.center_xz.x, cell.elevation_m, cell.center_xz.y),
            elevation_m: cell.elevation_m,
            surface: cell.surface,
            climate: cell.climate,
            biome: cell.biome,
            travel_cost: cell.travel_cost,
            downstream: cell.downstream,
            edge_elevations_m: cell.edge_elevations_m,
            road_ids: road_ids
                .remove(&cell.coord)
                .unwrap_or_default()
                .into_iter()
                .collect(),
            route_ids: route_ids
                .remove(&cell.coord)
                .unwrap_or_default()
                .into_iter()
                .collect(),
            portal_ids: portal_ids
                .remove(&cell.coord)
                .unwrap_or_default()
                .into_iter()
                .collect(),
        })
        .collect()
}

fn path_boundaries(cells: &[HexCoord]) -> Vec<BoundaryKey> {
    cells
        .windows(2)
        .map(|pair| BoundaryKey::new(pair[0], pair[1]).expect("path cells are neighbors"))
        .collect()
}

fn unit_noise(
    config: WorldGenerationConfig,
    stage_id: u64,
    spatial_key: u128,
    feature_key: u128,
) -> f64 {
    let mut rng = DeterministicRng::from_material(SeedMaterial {
        world_seed: config.world_seed,
        stage_id,
        spatial_key,
        feature_key,
    });
    rng.next_unit_f64()
}

fn stable_value(
    config: WorldGenerationConfig,
    stage_id: u64,
    spatial_key: u128,
    feature_key: u128,
) -> u128 {
    let high = SeedMaterial {
        world_seed: config.world_seed,
        stage_id,
        spatial_key,
        feature_key,
    }
    .derive();
    let low = SeedMaterial {
        world_seed: config.world_seed ^ u128::from(config.generator_version.0),
        stage_id: stage_id ^ 0x9e37_79b9_7f4a_7c15,
        spatial_key: spatial_key.rotate_left(37),
        feature_key: feature_key ^ 0xa5a5_a5a5_a5a5_a5a5,
    }
    .derive();
    (u128::from(high) << 64) | u128::from(low)
}

fn coord_key(coord: HexCoord) -> u128 {
    let q = zigzag_i16(coord.q);
    let r = zigzag_i16(coord.r);
    (u128::from(q) << 16) | u128::from(r)
}

fn boundary_material(boundary: BoundaryKey) -> u128 {
    (coord_key(boundary.low) << 32) | coord_key(boundary.high)
}

fn zigzag_i16(value: i16) -> u16 {
    ((i32::from(value) << 1) ^ (i32::from(value) >> 15)) as u16
}

fn digest_serializable<T: Serialize>(value: &T) -> Result<u64, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    }))
}

impl CellDraft {
    fn is_ocean(&self) -> bool {
        self.surface == SurfaceClass::Ocean
    }
}
