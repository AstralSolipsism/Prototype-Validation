use crate::model::*;
use deterministic_rng::SeedMaterial;
use glam::{DVec2, DVec3, Vec3Swizzles};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use world_generation_core::{
    Biome, BoundaryKey, HexCoord, HexDirection, WorldCompilation, WorldManifest,
};
use world_ids::{EntityId, EventId};

const STAGE_ATLAS: u64 = 0x4100;
const STAGE_TERRAIN_DETAIL: u64 = 0x4200;
const FEATURE_MOUNTAIN_RANGE: u128 = 0x4101;
const FEATURE_RIVER: u128 = 0x4102;
const FEATURE_COAST: u128 = 0x4103;
const FEATURE_ROAD: u128 = 0x4104;
const FEATURE_BOUNDARY: u128 = 0x4105;

pub fn build_atlas(base: &WorldCompilation) -> Result<WorldAtlasManifest, serde_json::Error> {
    let manifest = &base.manifest;
    let mountain_id = stable_entity(manifest.world_seed, STAGE_ATLAS, 0, FEATURE_MOUNTAIN_RANGE);
    let river_id = stable_entity(manifest.world_seed, STAGE_ATLAS, 0, FEATURE_RIVER);
    let coast_id = stable_entity(manifest.world_seed, STAGE_ATLAS, 0, FEATURE_COAST);

    let mut features = vec![
        AtlasFeature {
            id: mountain_id,
            kind: AtlasFeatureKind::MountainRange,
            path_world: mountain_path(manifest),
            touched_cells: Vec::new(),
            importance: 1.0,
        },
        AtlasFeature {
            id: river_id,
            kind: AtlasFeatureKind::River,
            path_world: river_path(manifest),
            touched_cells: Vec::new(),
            importance: 1.0,
        },
        AtlasFeature {
            id: coast_id,
            kind: AtlasFeatureKind::Coastline,
            path_world: coastline_path(manifest),
            touched_cells: Vec::new(),
            importance: 0.95,
        },
    ];

    for (index, road) in manifest.roads.iter().enumerate() {
        let path_world = road
            .cells
            .iter()
            .filter_map(|coord| manifest.cell(*coord))
            .map(|cell| {
                let xz = cell.coord.center_xz(manifest.cell_radius_m);
                DVec3::new(
                    xz.x,
                    detailed_height(manifest.world_seed, manifest.cell_radius_m, xz),
                    xz.y,
                )
            })
            .collect::<Vec<_>>();
        features.push(AtlasFeature {
            id: stable_entity(
                manifest.world_seed,
                STAGE_ATLAS,
                road.id.as_u128(),
                FEATURE_ROAD + index as u128,
            ),
            kind: AtlasFeatureKind::RoadCorridor,
            path_world,
            touched_cells: road.cells.clone(),
            importance: 0.72,
        });
    }

    let settlement = manifest
        .settlements
        .first()
        .expect("P4A manifest contains one settlement");
    let settlement_cell = manifest
        .cell(settlement.cell)
        .expect("settlement cell exists");
    let settlement_xz = settlement.cell.center_xz(manifest.cell_radius_m);
    features.push(AtlasFeature {
        id: EntityId::from_u128(settlement.id.as_u128()),
        kind: AtlasFeatureKind::Settlement,
        path_world: vec![DVec3::new(
            settlement_xz.x,
            detailed_height(manifest.world_seed, manifest.cell_radius_m, settlement_xz),
            settlement_xz.y,
        )],
        touched_cells: vec![settlement.cell],
        importance: 0.95,
    });

    let landmark = manifest
        .landmarks
        .first()
        .expect("P4A manifest contains one landmark");
    let landmark_xz = landmark.cell.center_xz(manifest.cell_radius_m);
    features.push(AtlasFeature {
        id: EntityId::from_u128(landmark.id.as_u128()),
        kind: AtlasFeatureKind::Landmark,
        path_world: vec![DVec3::new(
            landmark_xz.x,
            detailed_height(manifest.world_seed, manifest.cell_radius_m, landmark_xz) + 90.0,
            landmark_xz.y,
        )],
        touched_cells: vec![landmark.cell],
        importance: 1.0,
    });

    for feature in &mut features {
        if feature.touched_cells.is_empty() {
            feature.touched_cells = cells_touched_by_path(manifest, &feature.path_world);
        }
    }

    let boundary_contracts = build_boundary_contracts(manifest, &features);
    let boundary_by_cell = boundary_contracts
        .iter()
        .flat_map(|contract| {
            [
                (contract.boundary.low, contract.id),
                (contract.boundary.high, contract.id),
            ]
        })
        .fold(
            BTreeMap::<HexCoord, Vec<EntityId>>::new(),
            |mut map, (coord, id)| {
                map.entry(coord).or_default().push(id);
                map
            },
        );

    let mut cells = manifest
        .cells
        .iter()
        .map(|base_cell| {
            let (elevation, landforms) = summarize_cell(manifest, base_cell.coord);
            let resources = resources_for(base_cell.biome, landforms, elevation);
            let feature_ids = features
                .iter()
                .filter(|feature| feature.touched_cells.contains(&base_cell.coord))
                .map(|feature| feature.id)
                .collect::<Vec<_>>();
            let drainage = base_cell
                .downstream
                .and_then(|downstream| HexDirection::between(base_cell.coord, downstream));
            AtlasCellSpec {
                id: base_cell.id,
                coord: base_cell.coord,
                center_world: DVec3::new(
                    base_cell.center_world.x,
                    detailed_height(
                        manifest.world_seed,
                        manifest.cell_radius_m,
                        base_cell.center_world.xz(),
                    ),
                    base_cell.center_world.z,
                ),
                elevation,
                landforms,
                climate: base_cell.climate,
                biome: dominant_biome(base_cell.biome, landforms, elevation),
                resources,
                carrying_capacity: carrying_capacity(resources, elevation),
                predominant_drainage: drainage,
                feature_ids,
                boundary_contract_ids: boundary_by_cell
                    .get(&base_cell.coord)
                    .cloned()
                    .unwrap_or_default(),
                history: AtlasHistorySummary {
                    first_settlement_year: None,
                    current_population: 0,
                    dominant_economy: "unsettled".into(),
                    event_ids: Vec::<EventId>::new(),
                },
                materialization_version: 1,
            }
        })
        .collect::<Vec<_>>();
    cells.sort_by_key(|cell| cell.coord);

    let mut atlas = WorldAtlasManifest {
        world_id: manifest.world_id,
        world_seed: manifest.world_seed,
        generator_version: manifest.generator_version,
        cell_radius_m: manifest.cell_radius_m,
        cells,
        boundary_contracts,
        features,
        settlement_id: settlement.id,
        landmark_id: landmark.id,
        base_world_fingerprint: base.semantic_fingerprint,
        atlas_fingerprint: 0,
    };
    atlas.atlas_fingerprint = digest(&atlas)?;
    Ok(atlas)
}

fn build_boundary_contracts(
    manifest: &WorldManifest,
    features: &[AtlasFeature],
) -> Vec<BoundaryContract> {
    let coords = manifest
        .cells
        .iter()
        .map(|cell| cell.coord)
        .collect::<BTreeSet<_>>();
    let river_boundaries = manifest
        .river
        .boundaries
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let road_boundaries = manifest
        .roads
        .iter()
        .flat_map(|road| road.boundaries.iter().copied())
        .collect::<BTreeSet<_>>();
    let coastline = manifest.coastline.iter().copied().collect::<BTreeSet<_>>();

    let mut boundaries = BTreeSet::new();
    for coord in &coords {
        for direction in HexDirection::ALL {
            let neighbor = coord.neighbor(direction);
            if coords.contains(&neighbor) {
                boundaries.insert(BoundaryKey::new(*coord, neighbor).expect("adjacent cells"));
            }
        }
    }

    boundaries
        .into_iter()
        .map(|boundary| {
            let direction = HexDirection::between(boundary.low, boundary.high)
                .expect("boundary cells are adjacent");
            let (start, end) = edge_endpoints(boundary.low, direction, manifest.cell_radius_m);
            let samples = (0..EDGE_PROFILE_SAMPLES)
                .map(|index| {
                    let t = index as f64 / (EDGE_PROFILE_SAMPLES - 1) as f64;
                    let xz = start.lerp(end, t);
                    let elevation =
                        detailed_height(manifest.world_seed, manifest.cell_radius_m, xz);
                    let slope = detailed_slope(manifest.world_seed, manifest.cell_radius_m, xz);
                    EdgeSample {
                        t,
                        world_position: DVec3::new(xz.x, elevation, xz.y),
                        landform: classify_landform(
                            manifest.world_seed,
                            manifest.cell_radius_m,
                            xz,
                            elevation,
                            slope,
                        ),
                        river_width_m: if river_boundaries.contains(&boundary) {
                            16.0
                        } else {
                            0.0
                        },
                        road_width_m: if road_boundaries.contains(&boundary) {
                            8.0
                        } else {
                            0.0
                        },
                        is_coastline: coastline.contains(&boundary) || elevation.abs() <= 2.0,
                    }
                })
                .collect::<Vec<_>>();
            let feature_ids = features
                .iter()
                .filter(|feature| {
                    feature.touched_cells.contains(&boundary.low)
                        || feature.touched_cells.contains(&boundary.high)
                })
                .map(|feature| feature.id)
                .collect::<Vec<_>>();
            BoundaryContract {
                id: stable_entity(
                    manifest.world_seed,
                    STAGE_ATLAS,
                    boundary_key(boundary),
                    FEATURE_BOUNDARY,
                ),
                boundary,
                direction_from_low: direction,
                samples,
                feature_ids,
            }
        })
        .collect()
}

fn summarize_cell(manifest: &WorldManifest, coord: HexCoord) -> (ElevationSummary, LandformMix) {
    let center = coord.center_xz(manifest.cell_radius_m);
    let radius = manifest.cell_radius_m;
    let width = 3.0_f64.sqrt() * radius;
    let mut elevations = Vec::new();
    let mut slope_sum = 0.0;
    let mut buildable = 0usize;
    let mut counts = [0usize; 7];

    let resolution = usize::from(ATLAS_INTERNAL_SAMPLE_RESOLUTION);
    for z_index in 0..resolution {
        let z = center.y - radius + 2.0 * radius * z_index as f64 / (resolution - 1) as f64;
        for x_index in 0..resolution {
            let x = center.x - width * 0.5 + width * x_index as f64 / (resolution - 1) as f64;
            let point = DVec2::new(x, z);
            if !point_in_hex(point, center, radius) {
                continue;
            }
            let elevation = detailed_height(manifest.world_seed, radius, point);
            let slope = detailed_slope(manifest.world_seed, radius, point);
            let landform = classify_landform(manifest.world_seed, radius, point, elevation, slope);
            elevations.push(elevation);
            slope_sum += slope;
            if slope <= 0.18
                && elevation > 2.0
                && !matches!(
                    landform,
                    LandformClass::Coast | LandformClass::Estuary | LandformClass::Ocean
                )
            {
                buildable += 1;
            }
            counts[landform_bucket(landform)] += 1;
        }
    }

    elevations.sort_by(f64::total_cmp);
    let count = elevations.len().max(1);
    let minimum = *elevations.first().unwrap_or(&0.0);
    let maximum = *elevations.last().unwrap_or(&0.0);
    let mean = elevations.iter().sum::<f64>() / count as f64;
    let median = elevations.get(count / 2).copied().unwrap_or(mean);
    let fraction = |value: usize| value as f64 / count as f64;
    (
        ElevationSummary {
            minimum_m: minimum,
            maximum_m: maximum,
            mean_m: mean,
            median_m: median,
            relief_m: maximum - minimum,
            mean_slope: slope_sum / count as f64,
            buildable_fraction: buildable as f64 / count as f64,
        },
        LandformMix {
            mountain: fraction(counts[0]),
            ridge: fraction(counts[1]),
            hillslope: fraction(counts[2]),
            valley: fraction(counts[3]),
            lowland: fraction(counts[4]),
            coast: fraction(counts[5]),
            water: fraction(counts[6]),
        },
    )
}

fn landform_bucket(landform: LandformClass) -> usize {
    match landform {
        LandformClass::Mountain => 0,
        LandformClass::Ridge => 1,
        LandformClass::Hillslope | LandformClass::Terrace => 2,
        LandformClass::Valley => 3,
        LandformClass::Lowland | LandformClass::Floodplain => 4,
        LandformClass::Coast | LandformClass::Estuary => 5,
        LandformClass::Ocean => 6,
    }
}

fn resources_for(biome: Biome, mix: LandformMix, elevation: ElevationSummary) -> ResourceSummary {
    let wet = (mix.valley + mix.coast + mix.water).clamp(0.0, 1.0);
    let forest = match biome {
        Biome::TemperateForest => 0.9,
        Biome::UplandMeadow => 0.45,
        Biome::Wetland => 0.35,
        _ => 0.15,
    };
    ResourceSummary {
        fresh_water: (wet * 1.5 + mix.lowland * 0.35).clamp(0.0, 1.0),
        arable_land: (elevation.buildable_fraction * (mix.lowland + mix.valley + 0.25))
            .clamp(0.0, 1.0),
        timber: (forest * (1.0 - mix.water)).clamp(0.0, 1.0),
        stone: (mix.mountain + mix.ridge * 0.75 + mix.hillslope * 0.25).clamp(0.0, 1.0),
        fishery: (mix.water * 1.4 + mix.coast * 0.8).clamp(0.0, 1.0),
        harbor_quality: (mix.coast * 1.2 + mix.estuary_proxy() * 0.8).clamp(0.0, 1.0),
    }
}

trait LandformMixExt {
    fn estuary_proxy(self) -> f64;
}

impl LandformMixExt for LandformMix {
    fn estuary_proxy(self) -> f64 {
        (self.valley * self.coast * 4.0).clamp(0.0, 1.0)
    }
}

fn carrying_capacity(resources: ResourceSummary, elevation: ElevationSummary) -> f64 {
    (resources.fresh_water * 0.24
        + resources.arable_land * 0.34
        + resources.timber * 0.10
        + resources.fishery * 0.18
        + resources.harbor_quality * 0.14)
        * elevation.buildable_fraction.sqrt()
}

fn dominant_biome(base: Biome, mix: LandformMix, elevation: ElevationSummary) -> Biome {
    if mix.water > 0.55 {
        Biome::Ocean
    } else if mix.coast > 0.22 && mix.valley > 0.08 {
        Biome::Wetland
    } else if mix.mountain + mix.ridge > 0.45 || elevation.maximum_m > 175.0 {
        Biome::AlpineRock
    } else {
        base
    }
}

pub fn detailed_height(world_seed: u128, cell_radius_m: f64, point: DVec2) -> f64 {
    let scale = cell_radius_m.max(1.0);
    let east = point.x / (3.0_f64.sqrt() * scale);
    let base = 42.0 - east * 18.0;

    let mountain_axis = -0.22 * point.x + 52.0 * (point.x / (scale * 1.7)).sin();
    let mountain_distance = point.y - mountain_axis;
    let west_fade = smoothstep(1.9, -0.15, east);
    let primary_ridge = 132.0 * (-(mountain_distance / (scale * 0.55)).powi(2)).exp() * west_fade;

    let secondary_axis = 0.34 * point.x - 125.0;
    let secondary_distance = point.y - secondary_axis;
    let secondary_ridge =
        48.0 * (-(secondary_distance / (scale * 0.42)).powi(2)).exp() * smoothstep(1.3, -0.8, east);

    let river_distance = point.y - river_center_z(point.x, scale);
    let valley = -46.0 * (-(river_distance / (scale * 0.24)).powi(2)).exp();
    let bay = -92.0 * smoothstep(0.65, 2.2, east) * (-(point.y / (scale * 1.08)).powi(2)).exp();

    let broad_noise = value_noise(world_seed, point / (scale * 0.62), 0x51) * 15.0;
    let detail_noise = value_noise(world_seed, point / (scale * 0.19), 0x52) * 5.5;
    base + primary_ridge + secondary_ridge + valley + bay + broad_noise + detail_noise
}

pub fn detailed_slope(world_seed: u128, cell_radius_m: f64, point: DVec2) -> f64 {
    let step = (cell_radius_m / 64.0).max(1.0);
    let dx = detailed_height(world_seed, cell_radius_m, point + DVec2::X * step)
        - detailed_height(world_seed, cell_radius_m, point - DVec2::X * step);
    let dz = detailed_height(world_seed, cell_radius_m, point + DVec2::Y * step)
        - detailed_height(world_seed, cell_radius_m, point - DVec2::Y * step);
    (DVec2::new(dx, dz) / (step * 2.0)).length()
}

pub fn classify_landform(
    world_seed: u128,
    cell_radius_m: f64,
    point: DVec2,
    elevation: f64,
    slope: f64,
) -> LandformClass {
    if elevation < -1.0 {
        return LandformClass::Ocean;
    }
    let river_distance = (point.y - river_center_z(point.x, cell_radius_m)).abs();
    if elevation <= 4.0 && river_distance <= cell_radius_m * 0.18 {
        return LandformClass::Estuary;
    }
    if elevation <= 8.0 {
        return LandformClass::Coast;
    }
    if river_distance <= cell_radius_m * 0.15 && slope <= 0.22 {
        return LandformClass::Floodplain;
    }
    if river_distance <= cell_radius_m * 0.33 {
        return LandformClass::Valley;
    }
    let local_curvature = local_curvature(world_seed, cell_radius_m, point);
    if elevation >= 150.0 && local_curvature < -0.015 {
        LandformClass::Mountain
    } else if elevation >= 105.0 && local_curvature < -0.006 {
        LandformClass::Ridge
    } else if slope >= 0.42 {
        LandformClass::Hillslope
    } else if elevation >= 70.0 && slope <= 0.18 {
        LandformClass::Terrace
    } else {
        LandformClass::Lowland
    }
}

pub fn river_center_z(x: f64, cell_radius_m: f64) -> f64 {
    0.12 * x + 30.0 * (x / (cell_radius_m * 1.25)).sin()
}

fn local_curvature(world_seed: u128, cell_radius_m: f64, point: DVec2) -> f64 {
    let step = (cell_radius_m / 48.0).max(1.0);
    let center = detailed_height(world_seed, cell_radius_m, point);
    let axial = detailed_height(world_seed, cell_radius_m, point + DVec2::X * step)
        + detailed_height(world_seed, cell_radius_m, point - DVec2::X * step)
        + detailed_height(world_seed, cell_radius_m, point + DVec2::Y * step)
        + detailed_height(world_seed, cell_radius_m, point - DVec2::Y * step)
        - center * 4.0;
    axial / (step * step)
}

fn value_noise(world_seed: u128, point: DVec2, feature: u128) -> f64 {
    let x0 = point.x.floor() as i64;
    let z0 = point.y.floor() as i64;
    let tx = smooth_curve(point.x - x0 as f64);
    let tz = smooth_curve(point.y - z0 as f64);
    let sample = |x: i64, z: i64| {
        let spatial = ((x as i128 as u128) << 64) ^ z as i128 as u128;
        let raw = SeedMaterial {
            world_seed,
            stage_id: STAGE_TERRAIN_DETAIL,
            spatial_key: spatial,
            feature_key: feature,
        }
        .derive();
        (raw as f64 / u64::MAX as f64) * 2.0 - 1.0
    };
    let a = lerp(sample(x0, z0), sample(x0 + 1, z0), tx);
    let b = lerp(sample(x0, z0 + 1), sample(x0 + 1, z0 + 1), tx);
    lerp(a, b, tz)
}

fn smooth_curve(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    if (edge1 - edge0).abs() <= f64::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(left: f64, right: f64, amount: f64) -> f64 {
    left + (right - left) * amount
}

pub fn point_in_hex(point: DVec2, center: DVec2, radius: f64) -> bool {
    let local = (point - center).abs();
    local.y <= radius + 1.0e-9
        && local.x <= 3.0_f64.sqrt() * radius * 0.5 + 1.0e-9
        && 3.0_f64.sqrt() * local.y + local.x <= 3.0_f64.sqrt() * radius + 1.0e-9
}

pub fn hex_corners(coord: HexCoord, radius: f64) -> [DVec2; 6] {
    let center = coord.center_xz(radius);
    std::array::from_fn(|index| {
        let angle = (30.0 + 60.0 * index as f64) * PI / 180.0;
        center + DVec2::new(angle.cos(), angle.sin()) * radius
    })
}

pub fn edge_endpoints(coord: HexCoord, direction: HexDirection, radius: f64) -> (DVec2, DVec2) {
    let corners = hex_corners(coord, radius);
    let (left, right) = match direction {
        HexDirection::East => (5, 0),
        HexDirection::NorthEast => (0, 1),
        HexDirection::NorthWest => (1, 2),
        HexDirection::West => (2, 3),
        HexDirection::SouthWest => (3, 4),
        HexDirection::SouthEast => (4, 5),
    };
    (corners[left], corners[right])
}

pub fn nearest_cell(atlas: &WorldAtlasManifest, point: DVec2) -> Option<HexCoord> {
    atlas
        .cells
        .iter()
        .filter(|cell| {
            point_in_hex(
                point,
                cell.coord.center_xz(atlas.cell_radius_m),
                atlas.cell_radius_m,
            )
        })
        .min_by(|left, right| {
            left.center_world
                .xz()
                .distance_squared(point)
                .total_cmp(&right.center_world.xz().distance_squared(point))
        })
        .map(|cell| cell.coord)
}

fn mountain_path(manifest: &WorldManifest) -> Vec<DVec3> {
    let radius = manifest.cell_radius_m;
    (-12..=8)
        .map(|step| {
            let x = step as f64 * radius * 0.24;
            let z = -0.22 * x + 52.0 * (x / (radius * 1.7)).sin();
            DVec3::new(
                x,
                detailed_height(manifest.world_seed, radius, DVec2::new(x, z)),
                z,
            )
        })
        .collect()
}

fn river_path(manifest: &WorldManifest) -> Vec<DVec3> {
    let radius = manifest.cell_radius_m;
    (-14..=16)
        .map(|step| {
            let x = step as f64 * radius * 0.20;
            let z = river_center_z(x, radius);
            DVec3::new(
                x,
                detailed_height(manifest.world_seed, radius, DVec2::new(x, z)) + 0.4,
                z,
            )
        })
        .collect()
}

fn coastline_path(manifest: &WorldManifest) -> Vec<DVec3> {
    let radius = manifest.cell_radius_m;
    (-12..=12)
        .map(|step| {
            let z = step as f64 * radius * 0.19;
            let mut low = radius * 0.35;
            let mut high = radius * 4.2;
            for _ in 0..28 {
                let mid = (low + high) * 0.5;
                if detailed_height(manifest.world_seed, radius, DVec2::new(mid, z)) > 0.0 {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            let x = (low + high) * 0.5;
            DVec3::new(x, 0.0, z)
        })
        .collect()
}

fn cells_touched_by_path(manifest: &WorldManifest, path: &[DVec3]) -> Vec<HexCoord> {
    let mut touched = BTreeSet::new();
    for point in path {
        if let Some(cell) = manifest.cells.iter().min_by(|left, right| {
            left.center_world
                .xz()
                .distance_squared(point.xz())
                .total_cmp(&right.center_world.xz().distance_squared(point.xz()))
        }) {
            if cell.center_world.xz().distance(point.xz()) <= manifest.cell_radius_m * 1.35 {
                touched.insert(cell.coord);
            }
        }
    }
    touched.into_iter().collect()
}

pub(crate) fn stable_entity(
    world_seed: u128,
    stage: u64,
    spatial: u128,
    feature: u128,
) -> EntityId {
    let low = SeedMaterial {
        world_seed,
        stage_id: stage,
        spatial_key: spatial,
        feature_key: feature,
    }
    .derive();
    let high = SeedMaterial {
        world_seed,
        stage_id: stage ^ 0x9e37_79b9,
        spatial_key: spatial.rotate_left(37),
        feature_key: feature ^ 0xa5a5_5a5a_3c3c_c3c3,
    }
    .derive();
    EntityId::from_parts(high, low)
}

fn boundary_key(boundary: BoundaryKey) -> u128 {
    let low = ((boundary.low.q as i64 as u64) << 32) | boundary.low.r as i64 as u64 & 0xffff_ffff;
    let high =
        ((boundary.high.q as i64 as u64) << 32) | boundary.high.r as i64 as u64 & 0xffff_ffff;
    ((low as u128) << 64) | high as u128
}

pub(crate) fn digest<T: Serialize>(value: &T) -> Result<u64, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use world_generation_core::WorldCompilation;

    fn build_fixture() -> WorldCompilation {
        p4_test_support::baseline_world()
    }

    mod p4_test_support {
        use world_generation_core::{
            GeneratorVersion, TraversalOrder, WorldCompilation, WorldGenerationConfig,
            generate_world_with_order,
        };
        use world_ids::{BuildingId, WorldId};

        pub fn baseline_world() -> WorldCompilation {
            generate_world_with_order(
                WorldGenerationConfig {
                    world_id: WorldId::from_u128(0x5044),
                    world_seed: 0x706f_7274_2d76_616c_6c65_792d_3031,
                    generator_version: GeneratorVersion(1),
                    radius: 2,
                    cell_radius_m: 128.0,
                    building_blueprint_id: BuildingId::from_u128(1),
                },
                TraversalOrder::Canonical,
            )
            .expect("fixture world")
        }
    }

    #[test]
    fn atlas_contains_rich_per_cell_summaries() {
        let atlas = build_atlas(&build_fixture()).expect("atlas");
        assert_eq!(atlas.cells.len(), 19);
        assert!(
            atlas
                .cells
                .iter()
                .any(|cell| cell.elevation.relief_m > 45.0)
        );
        assert!(
            atlas
                .cells
                .iter()
                .any(|cell| cell.landforms.mountain > 0.02)
        );
        assert!(
            atlas
                .cells
                .iter()
                .any(|cell| cell.landforms.coast + cell.landforms.water > 0.2)
        );
        assert!(
            atlas
                .boundary_contracts
                .iter()
                .all(|contract| contract.samples.len() == EDGE_PROFILE_SAMPLES)
        );
    }

    #[test]
    fn detailed_height_is_continuous_around_shared_edges() {
        let atlas = build_atlas(&build_fixture()).expect("atlas");
        for contract in &atlas.boundary_contracts {
            for sample in &contract.samples {
                let recomputed = detailed_height(
                    atlas.world_seed,
                    atlas.cell_radius_m,
                    sample.world_position.xz(),
                );
                assert!((recomputed - sample.world_position.y).abs() < 1.0e-9);
            }
        }
    }
}
