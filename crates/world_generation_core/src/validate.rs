use crate::hex::{BoundaryKey, HexCoord, HexDirection};
use crate::model::*;
use scroll_camera_core::ScrollGrammar;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use thiserror::Error;
use world_ids::{BuildingInstanceId, CellId, PortalId, RoadId, RouteId};

const EPSILON: f64 = 1.0e-8;

#[derive(Clone, Debug, PartialEq, Error)]
pub enum WorldValidationError {
    #[error("world contains {actual} cells, expected {expected}")]
    CellCount { actual: usize, expected: usize },
    #[error("duplicate hex coordinate {0:?}")]
    DuplicateCoord(HexCoord),
    #[error("duplicate cell ID {0}")]
    DuplicateCellId(CellId),
    #[error("cell {0:?} contains a non-finite or invalid scalar")]
    InvalidCellScalar(HexCoord),
    #[error("cell {cell:?} edge {direction:?} disagrees with its neighbor")]
    TerrainSeam {
        cell: HexCoord,
        direction: HexDirection,
    },
    #[error("cell {cell:?} has invalid downstream target {downstream:?}")]
    InvalidDownstream {
        cell: HexCoord,
        downstream: HexCoord,
    },
    #[error("cell {cell:?} drains uphill to {downstream:?}")]
    UphillDrainage {
        cell: HexCoord,
        downstream: HexCoord,
    },
    #[error("river path is empty or does not end in ocean")]
    RiverDoesNotReachOcean,
    #[error("river cells {left:?} and {right:?} are not adjacent")]
    DisconnectedRiver {
        left: HexCoord,
        right: HexCoord,
    },
    #[error("river path does not follow the cell downstream relation at {0:?}")]
    RiverIgnoresDownstream(HexCoord),
    #[error("coastline boundary {0:?} does not separate land and ocean")]
    InvalidCoastline(BoundaryKey),
    #[error("land-ocean boundary {0:?} is missing from the coastline")]
    MissingCoastline(BoundaryKey),
    #[error("road {0} is empty, disconnected, enters ocean, or misses the settlement")]
    InvalidRoad(RoadId),
    #[error("route {0} is empty, disconnected, or inconsistent with its road")]
    InvalidRoute(RouteId),
    #[error("required scroll grammar {0:?} is absent")]
    MissingGrammar(ScrollGrammar),
    #[error("portal {0} has invalid geometry or boundary references")]
    InvalidPortal(PortalId),
    #[error("portal {0} is not referenced by exactly two adjacent cells")]
    PortalReferenceCount(PortalId),
    #[error("required boundary portal is missing for {kind:?} at {boundary:?}")]
    MissingBoundaryPortal {
        kind: PortalKind,
        boundary: BoundaryKey,
    },
    #[error("P4 requires exactly one valid port town")]
    InvalidSettlementCount,
    #[error("port town lacks one or more required siting reasons")]
    MissingSettlementReason,
    #[error("port town is not on the river mouth, coast, or road convergence")]
    InvalidSettlementSite,
    #[error("building instance {0} is invalid or disconnected from a route")]
    InvalidBuilding(BuildingInstanceId),
    #[error("building instances {left} and {right} overlap")]
    OverlappingBuildings {
        left: BuildingInstanceId,
        right: BuildingInstanceId,
    },
    #[error("P4 requires exactly one stable landmark referenced by every route")]
    InvalidLandmark,
    #[error("generation stage digests are empty, duplicated, or zero")]
    InvalidStageDigests,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationReport {
    pub errors: Vec<WorldValidationError>,
}

impl ValidationReport {
    pub fn new(errors: Vec<WorldValidationError>) -> Self {
        Self { errors }
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "world validation failed with {} error(s):",
            self.errors.len()
        )?;
        for error in &self.errors {
            writeln!(formatter, "- {error}")?;
        }
        Ok(())
    }
}

impl Error for ValidationReport {}

pub fn validate_world(manifest: &WorldManifest) -> Result<(), ValidationReport> {
    let mut errors = Vec::new();
    let expected_cells = 1 + 3 * usize::from(manifest.radius as u16) * usize::from((manifest.radius + 1) as u16);
    if manifest.cells.len() != expected_cells {
        errors.push(WorldValidationError::CellCount {
            actual: manifest.cells.len(),
            expected: expected_cells,
        });
    }

    let mut coords = BTreeMap::new();
    let mut cell_ids = BTreeSet::new();
    for cell in &manifest.cells {
        if coords.insert(cell.coord, cell).is_some() {
            errors.push(WorldValidationError::DuplicateCoord(cell.coord));
        }
        if !cell_ids.insert(cell.id) {
            errors.push(WorldValidationError::DuplicateCellId(cell.id));
        }
        let scalars = [
            cell.center_world.x,
            cell.center_world.y,
            cell.center_world.z,
            cell.elevation_m,
            cell.climate.temperature_c,
            cell.climate.moisture,
            cell.travel_cost,
        ];
        if scalars.iter().any(|value| !value.is_finite())
            || !(0.0..=1.0).contains(&cell.climate.moisture)
            || cell.travel_cost <= 0.0
            || cell.edge_elevations_m.iter().any(|value| !value.is_finite())
        {
            errors.push(WorldValidationError::InvalidCellScalar(cell.coord));
        }
    }

    validate_terrain_and_hydrology(manifest, &coords, &mut errors);
    validate_coastline(manifest, &coords, &mut errors);
    validate_settlement_roads_routes(manifest, &coords, &mut errors);
    validate_portals(manifest, &coords, &mut errors);
    validate_landmark_and_stages(manifest, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationReport::new(errors))
    }
}

fn validate_terrain_and_hydrology(
    manifest: &WorldManifest,
    coords: &BTreeMap<HexCoord, &AtlasCellManifest>,
    errors: &mut Vec<WorldValidationError>,
) {
    for cell in &manifest.cells {
        for direction in HexDirection::ALL {
            let neighbor_coord = cell.coord.neighbor(direction);
            if let Some(neighbor) = coords.get(&neighbor_coord) {
                let left = cell.edge_elevations_m[direction.index()];
                let right = neighbor.edge_elevations_m[direction.opposite().index()];
                if (left - right).abs() > EPSILON {
                    errors.push(WorldValidationError::TerrainSeam {
                        cell: cell.coord,
                        direction,
                    });
                }
            }
        }

        if cell.is_ocean() {
            if cell.downstream.is_some() {
                errors.push(WorldValidationError::InvalidCellScalar(cell.coord));
            }
            continue;
        }
        let Some(downstream_coord) = cell.downstream else {
            errors.push(WorldValidationError::InvalidCellScalar(cell.coord));
            continue;
        };
        let Some(downstream) = coords.get(&downstream_coord) else {
            errors.push(WorldValidationError::InvalidDownstream {
                cell: cell.coord,
                downstream: downstream_coord,
            });
            continue;
        };
        if cell.coord.distance(downstream_coord) != 1 {
            errors.push(WorldValidationError::InvalidDownstream {
                cell: cell.coord,
                downstream: downstream_coord,
            });
        }
        if downstream.elevation_m > cell.elevation_m + EPSILON {
            errors.push(WorldValidationError::UphillDrainage {
                cell: cell.coord,
                downstream: downstream_coord,
            });
        }
    }

    if manifest.river.cells.len() < 2
        || manifest
            .river
            .cells
            .last()
            .and_then(|coord| coords.get(coord))
            .is_none_or(|cell| !cell.is_ocean())
    {
        errors.push(WorldValidationError::RiverDoesNotReachOcean);
        return;
    }
    for pair in manifest.river.cells.windows(2) {
        if pair[0].distance(pair[1]) != 1 {
            errors.push(WorldValidationError::DisconnectedRiver {
                left: pair[0],
                right: pair[1],
            });
            continue;
        }
        if coords[&pair[0]].downstream != Some(pair[1]) {
            errors.push(WorldValidationError::RiverIgnoresDownstream(pair[0]));
        }
    }
    if manifest.river.boundaries != boundaries_for(&manifest.river.cells) {
        errors.push(WorldValidationError::RiverDoesNotReachOcean);
    }
}

fn validate_coastline(
    manifest: &WorldManifest,
    coords: &BTreeMap<HexCoord, &AtlasCellManifest>,
    errors: &mut Vec<WorldValidationError>,
) {
    let coastline = manifest.coastline.iter().copied().collect::<BTreeSet<_>>();
    for boundary in &manifest.coastline {
        let Some(low) = coords.get(&boundary.low) else {
            errors.push(WorldValidationError::InvalidCoastline(*boundary));
            continue;
        };
        let Some(high) = coords.get(&boundary.high) else {
            errors.push(WorldValidationError::InvalidCoastline(*boundary));
            continue;
        };
        if low.is_ocean() == high.is_ocean() || boundary.low.distance(boundary.high) != 1 {
            errors.push(WorldValidationError::InvalidCoastline(*boundary));
        }
    }

    for cell in manifest.cells.iter().filter(|cell| !cell.is_ocean()) {
        for direction in HexDirection::ALL {
            let neighbor_coord = cell.coord.neighbor(direction);
            if coords.get(&neighbor_coord).is_some_and(|neighbor| neighbor.is_ocean()) {
                let boundary = BoundaryKey::new(cell.coord, neighbor_coord)
                    .expect("neighboring cells have a boundary");
                if !coastline.contains(&boundary) {
                    errors.push(WorldValidationError::MissingCoastline(boundary));
                }
            }
        }
    }
}

fn validate_settlement_roads_routes(
    manifest: &WorldManifest,
    coords: &BTreeMap<HexCoord, &AtlasCellManifest>,
    errors: &mut Vec<WorldValidationError>,
) {
    if manifest.settlements.len() != 1 {
        errors.push(WorldValidationError::InvalidSettlementCount);
        return;
    }
    let settlement = &manifest.settlements[0];
    let required_reasons = BTreeSet::from([
        SettlementReason::FreshWater,
        SettlementReason::ShelteredCoast,
        SettlementReason::BuildableSlope,
        SettlementReason::RoadConvergence,
    ]);
    if !required_reasons.is_subset(&settlement.reasons.iter().copied().collect()) {
        errors.push(WorldValidationError::MissingSettlementReason);
    }

    let settlement_cell = coords.get(&settlement.cell);
    let river_uses_settlement = manifest.river.cells.contains(&settlement.cell);
    let coastal = settlement_cell.is_some_and(|cell| {
        HexDirection::ALL.into_iter().any(|direction| {
            coords
                .get(&cell.coord.neighbor(direction))
                .is_some_and(|neighbor| neighbor.is_ocean())
        })
    });
    let converging_roads = manifest
        .roads
        .iter()
        .filter(|road| road.cells.contains(&settlement.cell))
        .count();
    if settlement_cell.is_none()
        || settlement_cell.is_some_and(|cell| cell.is_ocean() || cell.travel_cost >= 2.0)
        || !river_uses_settlement
        || !coastal
        || converging_roads < 3
    {
        errors.push(WorldValidationError::InvalidSettlementSite);
    }

    let roads = manifest
        .roads
        .iter()
        .map(|road| (road.id, road))
        .collect::<BTreeMap<_, _>>();
    for road in &manifest.roads {
        if road.cells.len() < 2
            || road.cells.last().copied() != Some(settlement.cell)
            || road.boundaries != boundaries_for(&road.cells)
            || road.cells.iter().any(|coord| {
                coords
                    .get(coord)
                    .is_none_or(|cell| cell.is_ocean())
            })
        {
            errors.push(WorldValidationError::InvalidRoad(road.id));
        }
    }

    let routes = manifest
        .routes
        .iter()
        .map(|route| (route.id, route))
        .collect::<BTreeMap<_, _>>();
    let mut grammars = BTreeSet::new();
    for route in &manifest.routes {
        let valid_road = roads.get(&route.road_id).is_some_and(|road| road.cells == route.cells);
        let finite_nodes = route.nodes.iter().all(|node| node.world_position.is_finite());
        let connected = route
            .cells
            .windows(2)
            .all(|pair| pair[0].distance(pair[1]) == 1);
        if route.cells.len() < 2 || route.nodes.is_empty() || !valid_road || !finite_nodes || !connected {
            errors.push(WorldValidationError::InvalidRoute(route.id));
        }
        grammars.extend(route.nodes.iter().map(|node| node.grammar));
    }
    for grammar in [
        ScrollGrammar::StandardSideView,
        ScrollGrammar::LightDepth,
        ScrollGrammar::JunctionApproach,
        ScrollGrammar::JunctionDecision,
        ScrollGrammar::TurnCommit,
        ScrollGrammar::CameraReorientation,
        ScrollGrammar::VistaReveal,
        ScrollGrammar::VerticalTransition,
    ] {
        if !grammars.contains(&grammar) {
            errors.push(WorldValidationError::MissingGrammar(grammar));
        }
    }

    if settlement.buildings.len() < 6 {
        errors.push(WorldValidationError::InvalidSettlementCount);
    }
    let mut building_ids = BTreeSet::new();
    for building in &settlement.buildings {
        if !building_ids.insert(building.instance_id)
            || !building.local_position.is_finite()
            || !building.half_extents_m.is_finite()
            || building.half_extents_m.min_element() <= 0.0
            || !building.yaw_radians.is_finite()
            || routes.get(&building.entry_route_id).is_none_or(|route| {
                !route.cells.contains(&settlement.cell)
            })
        {
            errors.push(WorldValidationError::InvalidBuilding(building.instance_id));
        }
    }
    for (index, left) in settlement.buildings.iter().enumerate() {
        for right in settlement.buildings.iter().skip(index + 1) {
            if overlaps_xz(left, right) {
                errors.push(WorldValidationError::OverlappingBuildings {
                    left: left.instance_id,
                    right: right.instance_id,
                });
            }
        }
    }
}

fn validate_portals(
    manifest: &WorldManifest,
    coords: &BTreeMap<HexCoord, &AtlasCellManifest>,
    errors: &mut Vec<WorldValidationError>,
) {
    let mut portal_ids = BTreeSet::new();
    let mut portal_keys = BTreeSet::new();
    for portal in &manifest.portals {
        let references = manifest
            .cells
            .iter()
            .filter(|cell| cell.portal_ids.contains(&portal.id))
            .count();
        if !portal_ids.insert(portal.id)
            || !portal_keys.insert((portal.boundary, portal_kind_key(portal.kind)))
            || portal.boundary.low.distance(portal.boundary.high) != 1
            || !coords.contains_key(&portal.boundary.low)
            || !coords.contains_key(&portal.boundary.high)
            || !portal.world_position.is_finite()
        {
            errors.push(WorldValidationError::InvalidPortal(portal.id));
        }
        if references != 2 {
            errors.push(WorldValidationError::PortalReferenceCount(portal.id));
        }
    }

    let existing = manifest
        .portals
        .iter()
        .map(|portal| (portal.boundary, portal_kind_key(portal.kind)))
        .collect::<BTreeSet<_>>();
    for boundary in &manifest.river.boundaries {
        require_portal(existing.contains(&(*boundary, 0)), PortalKind::River, *boundary, errors);
    }
    for road in &manifest.roads {
        for boundary in &road.boundaries {
            require_portal(existing.contains(&(*boundary, 1)), PortalKind::Road, *boundary, errors);
        }
    }
    for route in &manifest.routes {
        for boundary in boundaries_for(&route.cells) {
            require_portal(
                existing.contains(&(boundary, 2)),
                PortalKind::ScrollRoute,
                boundary,
                errors,
            );
        }
    }
}

fn validate_landmark_and_stages(
    manifest: &WorldManifest,
    errors: &mut Vec<WorldValidationError>,
) {
    if manifest.landmarks.len() != 1 {
        errors.push(WorldValidationError::InvalidLandmark);
    } else {
        let landmark = &manifest.landmarks[0];
        if !landmark.world_position.is_finite()
            || !landmark.visible_radius_m.is_finite()
            || landmark.visible_radius_m <= 0.0
            || manifest.routes.iter().any(|route| {
                route.visible_landmark_ids.as_slice() != [landmark.id]
            })
        {
            errors.push(WorldValidationError::InvalidLandmark);
        }
    }

    let stage_names = manifest
        .stage_digests
        .iter()
        .map(|stage| stage.stage.as_str())
        .collect::<BTreeSet<_>>();
    if manifest.stage_digests.len() < 5
        || stage_names.len() != manifest.stage_digests.len()
        || manifest.stage_digests.iter().any(|stage| stage.digest == 0)
    {
        errors.push(WorldValidationError::InvalidStageDigests);
    }
}

fn require_portal(
    condition: bool,
    kind: PortalKind,
    boundary: BoundaryKey,
    errors: &mut Vec<WorldValidationError>,
) {
    if !condition {
        errors.push(WorldValidationError::MissingBoundaryPortal { kind, boundary });
    }
}

fn boundaries_for(cells: &[HexCoord]) -> Vec<BoundaryKey> {
    cells
        .windows(2)
        .filter_map(|pair| BoundaryKey::new(pair[0], pair[1]))
        .collect()
}

fn portal_kind_key(kind: PortalKind) -> u8 {
    match kind {
        PortalKind::River => 0,
        PortalKind::Road => 1,
        PortalKind::ScrollRoute => 2,
    }
}

fn overlaps_xz(left: &BuildingPlacement, right: &BuildingPlacement) -> bool {
    let delta = (left.local_position - right.local_position).abs();
    delta.x < left.half_extents_m.x + right.half_extents_m.x - EPSILON
        && delta.z < left.half_extents_m.z + right.half_extents_m.z - EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeneratorVersion, WorldGenerationConfig, generate_world};
    use world_ids::{BuildingId, WorldId};

    fn config() -> WorldGenerationConfig {
        WorldGenerationConfig {
            world_id: WorldId::from_u128(1),
            world_seed: 0x5eed,
            generator_version: GeneratorVersion(1),
            radius: 2,
            cell_radius_m: 128.0,
            building_blueprint_id: BuildingId::from_u128(1),
        }
    }

    #[test]
    fn generated_world_passes_all_structural_validators() {
        let world = generate_world(config()).expect("valid world");
        validate_world(&world.manifest).expect("validated world");
    }

    #[test]
    fn broken_portal_reference_is_rejected() {
        let mut world = generate_world(config()).expect("valid world").manifest;
        let portal = world.portals[0].id;
        for cell in &mut world.cells {
            cell.portal_ids.retain(|id| *id != portal);
        }
        assert!(validate_world(&world).is_err());
    }
}
