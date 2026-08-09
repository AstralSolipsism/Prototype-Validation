#![forbid(unsafe_code)]

use deterministic_rng::SeedMaterial;
use glam::{DVec2, DVec3, Vec3Swizzles};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::PI,
};
use thiserror::Error;
use world_generation_core::{HexCoord, HexDirection};
use world_ids::{
    BuildingInstanceId, CellId, EntityId, LandmarkId, RegionId, RoadId, RouteId, WorldId,
};

pub const WORLD_SEED: u128 = 0x7265_6769_6f6e_2d73_6361_6c65_2d30_3031;
pub const ATLAS_RADIUS: i16 = 2;
pub const CELL_RADIUS_M: f64 = 2_309.401_076_758_503;
pub const CELL_FLAT_TO_FLAT_M: f64 = 4_000.0;
pub const TERRAIN_TILE_SIZE_M: f64 = 250.0;
pub const DETAIL_RESOLUTION: u16 = 97;
pub const PROXY_RESOLUTION: u16 = 17;
pub const BOUNDARY_PROFILE_SAMPLES: usize = 65;
pub const SURFACE_OFFSET_M: f64 = 0.75;
pub const ROUTE_SAMPLE_SPACING_M: f64 = 60.0;

const STAGE_ATLAS: u64 = 0x5100;
const STAGE_MATERIALIZATION: u64 = 0x5200;
const STAGE_TILE: u64 = 0x5300;
const STAGE_BUILDING: u64 = 0x5400;
const STAGE_ROUTE: u64 = 0x5500;
const STAGE_STRUCTURE: u64 = 0x5600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LandformClass {
    Ocean,
    Coast,
    Estuary,
    Floodplain,
    Valley,
    Lowland,
    Hillslope,
    Ridge,
    Mountain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ClimateBand {
    Maritime,
    Temperate,
    CoolUpland,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GroundingStrategy {
    Slab,
    TerracedFoundation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RouteSurfaceKind {
    SurfaceConforming,
    Bridge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellLod {
    Full,
    NeighborProxy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElevationSummary {
    pub minimum_m: f64,
    pub maximum_m: f64,
    pub mean_m: f64,
    pub relief_m: f64,
    pub buildable_fraction: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LandformMix {
    pub fractions: BTreeMap<LandformClass, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasCellRegion {
    pub id: CellId,
    pub materialization_id: RegionId,
    pub coord: HexCoord,
    pub center_world: DVec2,
    pub elevation: ElevationSummary,
    pub landforms: LandformMix,
    pub climate: ClimateBand,
    pub carrying_capacity: f64,
    pub terrain_tile_size_m: f64,
    pub expected_tile_count: usize,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionAtlas {
    pub world_id: WorldId,
    pub world_seed: u128,
    pub cell_radius_m: f64,
    pub flat_to_flat_m: f64,
    pub cells: Vec<AtlasCellRegion>,
    pub port_cell: HexCoord,
    pub landmark_cell: HexCoord,
    pub landmark_id: LandmarkId,
    pub semantic_fingerprint: u64,
}

impl RegionAtlas {
    pub fn cell(&self, coord: HexCoord) -> Option<&AtlasCellRegion> {
        self.cells.iter().find(|cell| cell.coord == coord)
    }

    pub fn nearest_cell(&self, point: DVec2) -> HexCoord {
        self.cells
            .iter()
            .min_by(|left, right| {
                left.center_world
                    .distance_squared(point)
                    .total_cmp(&right.center_world.distance_squared(point))
            })
            .map(|cell| cell.coord)
            .unwrap_or(HexCoord::ZERO)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainSample {
    pub grid_x: u16,
    pub grid_z: u16,
    pub world_position: DVec3,
    pub inside_hex: bool,
    pub slope: f64,
    pub landform: LandformClass,
    pub buildable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainTileManifest {
    pub id: EntityId,
    pub cell: HexCoord,
    pub tile_x: i16,
    pub tile_z: i16,
    pub minimum_world: DVec2,
    pub maximum_world: DVec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundaryProfile {
    pub direction: HexDirection,
    pub world_points: Vec<DVec3>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingPlacementRequest {
    pub instance_id: BuildingInstanceId,
    pub center_xz: DVec2,
    pub half_extents_m: DVec2,
    pub body_height_m: f64,
    pub maximum_supported_relief_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundingResult {
    pub instance_id: BuildingInstanceId,
    pub strategy: GroundingStrategy,
    pub grounded_center: DVec3,
    pub foundation_top_m: f64,
    pub foundation_bottom_m: f64,
    pub sampled_ground_min_m: f64,
    pub sampled_ground_max_m: f64,
    pub support_points: Vec<DVec3>,
    pub footprint_half_extents_m: DVec2,
    pub body_height_m: f64,
    pub fill_volume_m3: f64,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellMaterialization {
    pub id: RegionId,
    pub cell_id: CellId,
    pub coord: HexCoord,
    pub lod: CellLod,
    pub resolution: u16,
    pub bounds_min: DVec2,
    pub bounds_max: DVec2,
    pub samples: Vec<TerrainSample>,
    pub terrain_tiles: Vec<TerrainTileManifest>,
    pub boundary_profiles: Vec<BoundaryProfile>,
    pub buildings: Vec<GroundingResult>,
    pub semantic_fingerprint: u64,
}

impl CellMaterialization {
    pub fn sample_index(&self, x: usize, z: usize) -> usize {
        z * usize::from(self.resolution) + x
    }

    pub fn sample(&self, x: usize, z: usize) -> Option<&TerrainSample> {
        if x >= usize::from(self.resolution) || z >= usize::from(self.resolution) {
            return None;
        }
        self.samples.get(self.sample_index(x, z))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveCellSet {
    pub focused: CellMaterialization,
    pub neighbor_proxies: Vec<CellMaterialization>,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutePoint {
    pub world_position: DVec3,
    pub terrain_height_m: f64,
    pub cell: HexCoord,
    pub surface: RouteSurfaceKind,
    pub structure_id: Option<EntityId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledRoute {
    pub id: RouteId,
    pub road_id: RoadId,
    pub target_landmark_id: LandmarkId,
    pub points: Vec<RoutePoint>,
    pub crossed_cells: Vec<HexCoord>,
    pub maximum_surface_grade: f64,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionScaleWorld {
    pub atlas: RegionAtlas,
    pub routes: Vec<CompiledRoute>,
    pub landmark_world: DVec3,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationMetrics {
    pub atlas_cells: usize,
    pub cell_flat_to_flat_m: f64,
    pub focused_samples: usize,
    pub focused_tiles: usize,
    pub neighbor_proxies: usize,
    pub grounded_buildings: usize,
    pub route_points: usize,
    pub route_crossed_cell_counts: Vec<usize>,
    pub maximum_surface_grade: f64,
    pub maximum_surface_vertical_error_m: f64,
    pub maximum_boundary_height_error_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub world_fingerprint: u64,
    pub checks: Vec<ValidationCheck>,
    pub metrics: ValidationMetrics,
}

impl ValidationReport {
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }
}

#[derive(Debug, Error)]
pub enum RegionScaleError {
    #[error("Atlas cell {0:?} does not exist")]
    MissingCell(HexCoord),
    #[error(
        "building {instance_id} exceeds supported terrain relief: {relief_m:.3} m > {budget_m:.3} m"
    )]
    UnsupportedBuildingRelief {
        instance_id: BuildingInstanceId,
        relief_m: f64,
        budget_m: f64,
    },
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub fn build_world() -> Result<RegionScaleWorld, RegionScaleError> {
    let mut atlas = build_atlas()?;
    let landmark_world = DVec3::new(
        -600.0,
        terrain_height(WORLD_SEED, -600.0, 1_000.0) + 0.5,
        1_000.0,
    );
    let routes = build_routes(&atlas, landmark_world)?;
    atlas.semantic_fingerprint = fingerprint(&atlas)?;
    let mut world = RegionScaleWorld {
        atlas,
        routes,
        landmark_world,
        semantic_fingerprint: 0,
    };
    world.semantic_fingerprint = fingerprint(&world)?;
    Ok(world)
}

pub fn activate_cell(
    world: &RegionScaleWorld,
    coord: HexCoord,
) -> Result<ActiveCellSet, RegionScaleError> {
    let focused = materialize_cell(world, coord, CellLod::Full)?;
    let mut neighbor_proxies = Vec::new();
    for direction in HexDirection::ALL {
        let neighbor = coord.neighbor(direction);
        if world.atlas.cell(neighbor).is_some() {
            neighbor_proxies.push(materialize_cell(world, neighbor, CellLod::NeighborProxy)?);
        }
    }
    neighbor_proxies.sort_by_key(|materialization| materialization.coord);
    let mut active = ActiveCellSet {
        focused,
        neighbor_proxies,
        semantic_fingerprint: 0,
    };
    active.semantic_fingerprint = fingerprint(&active)?;
    Ok(active)
}

pub fn materialize_cell(
    world: &RegionScaleWorld,
    coord: HexCoord,
    lod: CellLod,
) -> Result<CellMaterialization, RegionScaleError> {
    let cell = world
        .atlas
        .cell(coord)
        .ok_or(RegionScaleError::MissingCell(coord))?;
    let resolution = match lod {
        CellLod::Full => DETAIL_RESOLUTION,
        CellLod::NeighborProxy => PROXY_RESOLUTION,
    };
    let center = cell.center_world;
    let x_radius = CELL_RADIUS_M * (PI / 6.0).cos();
    let bounds_min = DVec2::new(center.x - x_radius, center.y - CELL_RADIUS_M);
    let bounds_max = DVec2::new(center.x + x_radius, center.y + CELL_RADIUS_M);
    let denominator = f64::from(resolution.saturating_sub(1));
    let step_x = (bounds_max.x - bounds_min.x) / denominator;
    let step_z = (bounds_max.y - bounds_min.y) / denominator;
    let mut samples = Vec::with_capacity(usize::from(resolution).pow(2));
    for grid_z in 0..resolution {
        for grid_x in 0..resolution {
            let x = bounds_min.x + step_x * f64::from(grid_x);
            let z = bounds_min.y + step_z * f64::from(grid_z);
            let point = DVec2::new(x, z);
            let inside = point_in_hex(coord, CELL_RADIUS_M, point);
            let height = terrain_height(world.atlas.world_seed, x, z);
            let slope = terrain_slope(world.atlas.world_seed, x, z, step_x.min(step_z));
            let landform = classify_landform(x, z, height, slope);
            samples.push(TerrainSample {
                grid_x,
                grid_z,
                world_position: DVec3::new(x, height, z),
                inside_hex: inside,
                slope,
                landform,
                buildable: inside
                    && height > 2.0
                    && slope <= 0.18
                    && !matches!(
                        landform,
                        LandformClass::Ocean | LandformClass::Coast | LandformClass::Estuary
                    ),
            });
        }
    }
    let terrain_tiles = if lod == CellLod::Full {
        terrain_tiles_for_cell(world.atlas.world_seed, coord, bounds_min, bounds_max)
    } else {
        Vec::new()
    };
    let boundary_profiles = boundary_profiles(world.atlas.world_seed, coord);
    let buildings = if lod == CellLod::Full {
        building_requests(world, coord)
            .into_iter()
            .map(|request| ground_building(world.atlas.world_seed, request))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let mut materialization = CellMaterialization {
        id: cell.materialization_id,
        cell_id: cell.id,
        coord,
        lod,
        resolution,
        bounds_min,
        bounds_max,
        samples,
        terrain_tiles,
        boundary_profiles,
        buildings,
        semantic_fingerprint: 0,
    };
    materialization.semantic_fingerprint = fingerprint(&materialization)?;
    Ok(materialization)
}

pub fn terrain_height(world_seed: u128, x: f64, z: f64) -> f64 {
    let continental = 140.0 - 0.012 * (x + 1_000.0);
    let ridge_center = -1_200.0 + 650.0 * ((x + 2_500.0) / 3_500.0).sin();
    let ridge = 420.0
        * (-((z - ridge_center) / 1_200.0).powi(2)).exp()
        * (-((x + 2_500.0) / 7_000.0).powi(2)).exp();
    let peak_one =
        260.0 * (-(((x + 2_800.0) / 1_400.0).powi(2) + ((z + 800.0) / 1_200.0).powi(2))).exp();
    let peak_two =
        220.0 * (-(((x - 500.0) / 1_600.0).powi(2) + ((z - 2_600.0) / 1_400.0).powi(2))).exp();
    let hills = 40.0 * (x / 850.0).sin() * (z / 1_100.0).cos() + 25.0 * ((x + z) / 1_300.0).sin();
    let river = river_center_z(x);
    let valley = -130.0 * (-((z - river) / 420.0).powi(2)).exp();
    let coastal_shelf = -220.0 / (1.0 + (-(x - 6_500.0) / 450.0).exp());
    let seed_phase = ((world_seed as u64) as f64 / u64::MAX as f64) * PI;
    let micro = 8.0 * (x / 310.0 + seed_phase).sin() * (z / 370.0 - seed_phase).cos();
    continental + ridge + peak_one + peak_two + hills + valley + coastal_shelf + micro
}

pub fn terrain_slope(world_seed: u128, x: f64, z: f64, spacing: f64) -> f64 {
    let delta = spacing.clamp(12.0, 80.0);
    let dx = terrain_height(world_seed, x + delta, z) - terrain_height(world_seed, x - delta, z);
    let dz = terrain_height(world_seed, x, z + delta) - terrain_height(world_seed, x, z - delta);
    ((dx / (2.0 * delta)).powi(2) + (dz / (2.0 * delta)).powi(2)).sqrt()
}

pub fn river_center_z(x: f64) -> f64 {
    450.0 * (x / 2_400.0).sin() - 250.0
}

pub fn point_in_hex(coord: HexCoord, radius: f64, point: DVec2) -> bool {
    let center = coord.center_xz(radius);
    let local = point - center;
    let qx = local.x.abs();
    let qz = local.y.abs();
    if qx > radius * (PI / 6.0).cos() || qz > radius {
        return false;
    }
    3.0_f64.sqrt() * qx + qz <= 3.0_f64.sqrt() * radius + 1.0e-9
}

pub fn validate_world(world: &RegionScaleWorld) -> Result<ValidationReport, RegionScaleError> {
    let port = activate_cell(world, world.atlas.port_cell)?;
    let port_again = activate_cell(world, world.atlas.port_cell)?;
    let landmark = activate_cell(world, world.atlas.landmark_cell)?;

    let mut checks = Vec::new();
    checks.push(check(
        "atlas-cell-is-region-scale",
        world.atlas.flat_to_flat_m >= 3_900.0,
        format!(
            "flat-to-flat {:.3} m; target is approximately 4 km",
            world.atlas.flat_to_flat_m
        ),
    ));
    checks.push(check(
        "atlas-cell-is-not-terrain-tile",
        port.focused.terrain_tiles.len() >= 120
            && world
                .atlas
                .cells
                .iter()
                .all(|cell| cell.id.as_u128() != cell.materialization_id.as_u128()),
        format!(
            "focused cell contains {} independent {} m terrain tiles",
            port.focused.terrain_tiles.len(),
            TERRAIN_TILE_SIZE_M
        ),
    ));
    checks.push(check(
        "one-full-cell-plus-neighbor-proxies",
        port.focused.lod == CellLod::Full
            && port.neighbor_proxies.len() <= 6
            && port.neighbor_proxies.iter().all(|proxy| {
                proxy.lod == CellLod::NeighborProxy && proxy.resolution < port.focused.resolution
            }),
        format!(
            "one {}² full materialization and {} {}² proxies",
            port.focused.resolution,
            port.neighbor_proxies.len(),
            PROXY_RESOLUTION
        ),
    ));
    checks.push(check(
        "independent-cell-materializations",
        port.focused.id != landmark.focused.id
            && port.focused.cell_id != landmark.focused.cell_id
            && port.focused.semantic_fingerprint != landmark.focused.semantic_fingerprint,
        format!(
            "port materialization {} and landmark materialization {} are distinct",
            port.focused.id, landmark.focused.id
        ),
    ));
    checks.push(check(
        "cache-rebuild-is-deterministic",
        port == port_again,
        format!("active-set fingerprint {}", port.semantic_fingerprint),
    ));

    let boundary_error = maximum_shared_boundary_error(world)?;
    checks.push(check(
        "independent-neighbor-boundaries-match",
        boundary_error <= 1.0e-9,
        format!("maximum shared-boundary height error {boundary_error:.12} m"),
    ));

    let grounded_buildings = port
        .focused
        .buildings
        .iter()
        .chain(landmark.focused.buildings.iter())
        .collect::<Vec<_>>();
    let grounding_ok = !grounded_buildings.is_empty()
        && grounded_buildings.iter().all(|building| {
            building.foundation_bottom_m <= building.sampled_ground_min_m + 1.0e-9
                && building.foundation_top_m >= building.sampled_ground_max_m - 1.0e-9
                && building.grounded_center.y
                    >= building.foundation_top_m + building.body_height_m * 0.5 - 1.0e-9
        });
    checks.push(check(
        "buildings-have-explicit-grounding",
        grounding_ok,
        format!(
            "{} buildings use slab or terraced foundations with sampled footprint support",
            grounded_buildings.len()
        ),
    ));

    let mut maximum_vertical_error = 0.0_f64;
    let mut maximum_surface_grade = 0.0_f64;
    let route_checks = world.routes.iter().all(|route| {
        for point in &route.points {
            if point.surface == RouteSurfaceKind::SurfaceConforming {
                maximum_vertical_error = maximum_vertical_error.max(
                    (point.world_position.y - point.terrain_height_m - SURFACE_OFFSET_M).abs(),
                );
            } else if point.structure_id.is_none()
                || point.world_position.y <= point.terrain_height_m + 2.0
            {
                return false;
            }
        }
        maximum_surface_grade = maximum_surface_grade.max(route.maximum_surface_grade);
        route
            .crossed_cells
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            >= 3
            && route.target_landmark_id == world.atlas.landmark_id
            && route.maximum_surface_grade <= 0.35 + 1.0e-9
    });
    checks.push(check(
        "routes-have-explicit-vertical-semantics",
        route_checks && maximum_vertical_error <= 1.0e-9,
        format!(
            "max surface error {maximum_vertical_error:.12} m; max surface grade {maximum_surface_grade:.3}"
        ),
    ));

    let route_cell_counts = world
        .routes
        .iter()
        .map(|route| {
            route
                .crossed_cells
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
        })
        .collect::<Vec<_>>();
    checks.push(check(
        "routes-cross-region-boundaries",
        route_cell_counts.iter().all(|count| *count >= 3),
        format!("route unique-cell counts {route_cell_counts:?}"),
    ));

    checks.push(check(
        "local-materialization-does-not-expand-to-whole-atlas",
        port.focused.coord == world.atlas.port_cell
            && port.neighbor_proxies.len() < world.atlas.cells.len() - 1
            && port
                .focused
                .samples
                .iter()
                .filter(|sample| sample.inside_hex)
                .all(|sample| {
                    point_in_hex(
                        port.focused.coord,
                        CELL_RADIUS_M,
                        sample.world_position.xz(),
                    )
                }),
        format!(
            "focused cell plus {} neighbors, not all {} Atlas cells",
            port.neighbor_proxies.len(),
            world.atlas.cells.len()
        ),
    ));

    let route_points = world.routes.iter().map(|route| route.points.len()).sum();
    let metrics = ValidationMetrics {
        atlas_cells: world.atlas.cells.len(),
        cell_flat_to_flat_m: world.atlas.flat_to_flat_m,
        focused_samples: port.focused.samples.len(),
        focused_tiles: port.focused.terrain_tiles.len(),
        neighbor_proxies: port.neighbor_proxies.len(),
        grounded_buildings: grounded_buildings.len(),
        route_points,
        route_crossed_cell_counts: route_cell_counts,
        maximum_surface_grade,
        maximum_surface_vertical_error_m: maximum_vertical_error,
        maximum_boundary_height_error_m: boundary_error,
    };
    Ok(ValidationReport {
        world_fingerprint: world.semantic_fingerprint,
        checks,
        metrics,
    })
}

fn build_atlas() -> Result<RegionAtlas, RegionScaleError> {
    let coords = HexCoord::disk(ATLAS_RADIUS);
    let mut cells = Vec::with_capacity(coords.len());
    for coord in coords {
        let center = coord.center_xz(CELL_RADIUS_M);
        let summary = summarize_cell(coord);
        let expected_tile_count = terrain_tiles_for_cell(
            WORLD_SEED,
            coord,
            DVec2::new(
                center.x - CELL_RADIUS_M * (PI / 6.0).cos(),
                center.y - CELL_RADIUS_M,
            ),
            DVec2::new(
                center.x + CELL_RADIUS_M * (PI / 6.0).cos(),
                center.y + CELL_RADIUS_M,
            ),
        )
        .len();
        let mut cell = AtlasCellRegion {
            id: CellId::from_u128(stable_u128(WORLD_SEED, STAGE_ATLAS, coord_key(coord), 1)),
            materialization_id: RegionId::from_u128(stable_u128(
                WORLD_SEED,
                STAGE_MATERIALIZATION,
                coord_key(coord),
                1,
            )),
            coord,
            center_world: center,
            elevation: summary.0,
            landforms: summary.1,
            climate: if summary.0.mean_m < 80.0 {
                ClimateBand::Maritime
            } else if summary.0.mean_m > 360.0 {
                ClimateBand::CoolUpland
            } else {
                ClimateBand::Temperate
            },
            carrying_capacity: summary.0.buildable_fraction
                * (1.0
                    + summary
                        .1
                        .fractions
                        .get(&LandformClass::Floodplain)
                        .copied()
                        .unwrap_or(0.0)),
            terrain_tile_size_m: TERRAIN_TILE_SIZE_M,
            expected_tile_count,
            semantic_fingerprint: 0,
        };
        cell.semantic_fingerprint = fingerprint(&cell)?;
        cells.push(cell);
    }
    cells.sort_by_key(|cell| cell.coord);
    let landmark_id = LandmarkId::from_u128(stable_u128(WORLD_SEED, STAGE_ATLAS, 0, 0x77));
    let mut atlas = RegionAtlas {
        world_id: WorldId::from_u128(0x5044_5253),
        world_seed: WORLD_SEED,
        cell_radius_m: CELL_RADIUS_M,
        flat_to_flat_m: CELL_FLAT_TO_FLAT_M,
        cells,
        port_cell: HexCoord::new(1, 0),
        landmark_cell: HexCoord::ZERO,
        landmark_id,
        semantic_fingerprint: 0,
    };
    atlas.semantic_fingerprint = fingerprint(&atlas)?;
    Ok(atlas)
}

fn summarize_cell(coord: HexCoord) -> (ElevationSummary, LandformMix) {
    let resolution = 17_u16;
    let center = coord.center_xz(CELL_RADIUS_M);
    let x_radius = CELL_RADIUS_M * (PI / 6.0).cos();
    let min = DVec2::new(center.x - x_radius, center.y - CELL_RADIUS_M);
    let max = DVec2::new(center.x + x_radius, center.y + CELL_RADIUS_M);
    let denominator = f64::from(resolution - 1);
    let mut heights = Vec::new();
    let mut counts = BTreeMap::<LandformClass, usize>::new();
    let mut buildable = 0_usize;
    for z_index in 0..resolution {
        for x_index in 0..resolution {
            let x = min.x + (max.x - min.x) * f64::from(x_index) / denominator;
            let z = min.y + (max.y - min.y) * f64::from(z_index) / denominator;
            let point = DVec2::new(x, z);
            if !point_in_hex(coord, CELL_RADIUS_M, point) {
                continue;
            }
            let height = terrain_height(WORLD_SEED, x, z);
            let slope = terrain_slope(WORLD_SEED, x, z, 60.0);
            let landform = classify_landform(x, z, height, slope);
            heights.push(height);
            *counts.entry(landform).or_default() += 1;
            if height > 2.0
                && slope <= 0.18
                && !matches!(
                    landform,
                    LandformClass::Ocean | LandformClass::Coast | LandformClass::Estuary
                )
            {
                buildable += 1;
            }
        }
    }
    heights.sort_by(f64::total_cmp);
    let minimum = heights.first().copied().unwrap_or(0.0);
    let maximum = heights.last().copied().unwrap_or(0.0);
    let mean = if heights.is_empty() {
        0.0
    } else {
        heights.iter().sum::<f64>() / heights.len() as f64
    };
    let total = heights.len().max(1) as f64;
    let fractions = counts
        .into_iter()
        .map(|(class, count)| (class, count as f64 / total))
        .collect();
    (
        ElevationSummary {
            minimum_m: minimum,
            maximum_m: maximum,
            mean_m: mean,
            relief_m: maximum - minimum,
            buildable_fraction: buildable as f64 / total,
        },
        LandformMix { fractions },
    )
}

fn classify_landform(x: f64, z: f64, height: f64, slope: f64) -> LandformClass {
    if height <= -2.0 {
        return LandformClass::Ocean;
    }
    if height <= 6.0 {
        return if (z - river_center_z(x)).abs() <= 350.0 {
            LandformClass::Estuary
        } else {
            LandformClass::Coast
        };
    }
    let river_distance = (z - river_center_z(x)).abs();
    if river_distance <= 160.0 {
        return LandformClass::Floodplain;
    }
    if river_distance <= 430.0 {
        return LandformClass::Valley;
    }
    if height >= 520.0 {
        return LandformClass::Mountain;
    }
    if slope >= 0.34 {
        return LandformClass::Ridge;
    }
    if slope >= 0.16 {
        return LandformClass::Hillslope;
    }
    LandformClass::Lowland
}

fn terrain_tiles_for_cell(
    world_seed: u128,
    coord: HexCoord,
    bounds_min: DVec2,
    bounds_max: DVec2,
) -> Vec<TerrainTileManifest> {
    let minimum_tile_x = (bounds_min.x / TERRAIN_TILE_SIZE_M).floor() as i16;
    let maximum_tile_x = (bounds_max.x / TERRAIN_TILE_SIZE_M).ceil() as i16;
    let minimum_tile_z = (bounds_min.y / TERRAIN_TILE_SIZE_M).floor() as i16;
    let maximum_tile_z = (bounds_max.y / TERRAIN_TILE_SIZE_M).ceil() as i16;
    let mut tiles = Vec::new();
    for tile_z in minimum_tile_z..maximum_tile_z {
        for tile_x in minimum_tile_x..maximum_tile_x {
            let minimum = DVec2::new(
                f64::from(tile_x) * TERRAIN_TILE_SIZE_M,
                f64::from(tile_z) * TERRAIN_TILE_SIZE_M,
            );
            let maximum = minimum + DVec2::splat(TERRAIN_TILE_SIZE_M);
            let center = (minimum + maximum) * 0.5;
            let intersects = point_in_hex(coord, CELL_RADIUS_M, center)
                || [
                    minimum,
                    DVec2::new(maximum.x, minimum.y),
                    maximum,
                    DVec2::new(minimum.x, maximum.y),
                ]
                .into_iter()
                .any(|point| point_in_hex(coord, CELL_RADIUS_M, point));
            if intersects {
                tiles.push(TerrainTileManifest {
                    id: EntityId::from_u128(stable_u128(
                        world_seed,
                        STAGE_TILE,
                        coord_key(coord),
                        ((tile_x as i32 as u32 as u128) << 32) | tile_z as i32 as u32 as u128,
                    )),
                    cell: coord,
                    tile_x,
                    tile_z,
                    minimum_world: minimum,
                    maximum_world: maximum,
                });
            }
        }
    }
    tiles.sort_by_key(|tile| (tile.tile_z, tile.tile_x));
    tiles
}

fn boundary_profiles(world_seed: u128, coord: HexCoord) -> Vec<BoundaryProfile> {
    HexDirection::ALL
        .into_iter()
        .map(|direction| {
            let (start, end) = edge_endpoints(coord, direction);
            let world_points = (0..BOUNDARY_PROFILE_SAMPLES)
                .map(|index| {
                    let t = index as f64 / (BOUNDARY_PROFILE_SAMPLES - 1) as f64;
                    let point = start.lerp(end, t);
                    DVec3::new(
                        point.x,
                        terrain_height(world_seed, point.x, point.y),
                        point.y,
                    )
                })
                .collect();
            BoundaryProfile {
                direction,
                world_points,
            }
        })
        .collect()
}

fn edge_endpoints(coord: HexCoord, direction: HexDirection) -> (DVec2, DVec2) {
    let center = coord.center_xz(CELL_RADIUS_M);
    let neighbor_center = coord.neighbor(direction).center_xz(CELL_RADIUS_M);
    let toward_neighbor = neighbor_center - center;
    let vertices = hex_vertices(coord, CELL_RADIUS_M);
    let edge_index = (0..6)
        .max_by(|left, right| {
            let left_midpoint = (vertices[*left] + vertices[(*left + 1) % 6]) * 0.5;
            let right_midpoint = (vertices[*right] + vertices[(*right + 1) % 6]) * 0.5;
            (left_midpoint - center)
                .dot(toward_neighbor)
                .total_cmp(&(right_midpoint - center).dot(toward_neighbor))
        })
        .expect("hex has six edges");
    (vertices[edge_index], vertices[(edge_index + 1) % 6])
}

fn hex_vertices(coord: HexCoord, radius: f64) -> [DVec2; 6] {
    let center = coord.center_xz(radius);
    std::array::from_fn(|index| {
        let angle = (30.0 + index as f64 * 60.0).to_radians();
        center + DVec2::new(angle.cos(), angle.sin()) * radius
    })
}

fn building_requests(world: &RegionScaleWorld, coord: HexCoord) -> Vec<BuildingPlacementRequest> {
    let center = coord.center_xz(CELL_RADIUS_M);
    let offsets: &[(f64, f64, f64, f64, f64)] = if coord == world.atlas.port_cell {
        &[
            (-420.0, 280.0, 44.0, 30.0, 24.0),
            (-250.0, 320.0, 38.0, 28.0, 20.0),
            (-80.0, 290.0, 48.0, 34.0, 28.0),
            (110.0, 360.0, 36.0, 26.0, 18.0),
            (300.0, 310.0, 52.0, 38.0, 26.0),
            (-300.0, 520.0, 42.0, 32.0, 22.0),
            (-90.0, 540.0, 46.0, 34.0, 24.0),
            (180.0, 540.0, 40.0, 30.0, 21.0),
        ]
    } else {
        &[]
    };
    offsets
        .iter()
        .enumerate()
        .map(
            |(index, (x, z, half_x, half_z, height))| BuildingPlacementRequest {
                instance_id: BuildingInstanceId::from_u128(stable_u128(
                    world.atlas.world_seed,
                    STAGE_BUILDING,
                    coord_key(coord),
                    index as u128 + 1,
                )),
                center_xz: center + DVec2::new(*x, *z),
                half_extents_m: DVec2::new(*half_x, *half_z),
                body_height_m: *height,
                maximum_supported_relief_m: 16.0,
            },
        )
        .collect()
}

fn ground_building(
    world_seed: u128,
    request: BuildingPlacementRequest,
) -> Result<GroundingResult, RegionScaleError> {
    let hx = request.half_extents_m.x;
    let hz = request.half_extents_m.y;
    let offsets = [
        DVec2::ZERO,
        DVec2::new(-hx, -hz),
        DVec2::new(hx, -hz),
        DVec2::new(hx, hz),
        DVec2::new(-hx, hz),
        DVec2::new(0.0, -hz),
        DVec2::new(hx, 0.0),
        DVec2::new(0.0, hz),
        DVec2::new(-hx, 0.0),
    ];
    let support_points = offsets
        .into_iter()
        .map(|offset| {
            let point = request.center_xz + offset;
            DVec3::new(
                point.x,
                terrain_height(world_seed, point.x, point.y),
                point.y,
            )
        })
        .collect::<Vec<_>>();
    let minimum = support_points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let maximum = support_points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let relief = maximum - minimum;
    if relief > request.maximum_supported_relief_m {
        return Err(RegionScaleError::UnsupportedBuildingRelief {
            instance_id: request.instance_id,
            relief_m: relief,
            budget_m: request.maximum_supported_relief_m,
        });
    }
    let strategy = if relief <= 3.0 {
        GroundingStrategy::Slab
    } else {
        GroundingStrategy::TerracedFoundation
    };
    let foundation_top = maximum + 0.25;
    let foundation_bottom = minimum - 0.5;
    let footprint_area = request.half_extents_m.x * 2.0 * request.half_extents_m.y * 2.0;
    let average_ground =
        support_points.iter().map(|point| point.y).sum::<f64>() / support_points.len() as f64;
    let fill_volume = footprint_area * (foundation_top - average_ground).max(0.0);
    let mut result = GroundingResult {
        instance_id: request.instance_id,
        strategy,
        grounded_center: DVec3::new(
            request.center_xz.x,
            foundation_top + request.body_height_m * 0.5,
            request.center_xz.y,
        ),
        foundation_top_m: foundation_top,
        foundation_bottom_m: foundation_bottom,
        sampled_ground_min_m: minimum,
        sampled_ground_max_m: maximum,
        support_points,
        footprint_half_extents_m: request.half_extents_m,
        body_height_m: request.body_height_m,
        fill_volume_m3: fill_volume,
        semantic_fingerprint: 0,
    };
    result.semantic_fingerprint = fingerprint(&result)?;
    Ok(result)
}

fn build_routes(
    atlas: &RegionAtlas,
    landmark_world: DVec3,
) -> Result<Vec<CompiledRoute>, RegionScaleError> {
    let control_paths = [
        vec![
            DVec2::new(5_500.0, -3_000.0),
            DVec2::new(5_200.0, -1_800.0),
            DVec2::new(4_500.0, -700.0),
            DVec2::new(3_800.0, 300.0),
            DVec2::new(2_800.0, 900.0),
            DVec2::new(1_800.0, 1_000.0),
            DVec2::new(1_000.0, river_center_z(1_000.0)),
            DVec2::new(200.0, 500.0),
            landmark_world.xz(),
        ],
        vec![
            DVec2::new(4_000.0, 6_500.0),
            DVec2::new(3_500.0, 5_000.0),
            DVec2::new(3_000.0, 3_500.0),
            DVec2::new(2_500.0, 2_200.0),
            DVec2::new(1_900.0, 1_300.0),
            DVec2::new(900.0, 600.0),
            DVec2::new(-100.0, 700.0),
            landmark_world.xz(),
        ],
        vec![
            DVec2::new(-6_000.0, 3_400.0),
            DVec2::new(-4_800.0, 3_000.0),
            DVec2::new(-3_800.0, 2_500.0),
            DVec2::new(-2_800.0, 1_800.0),
            DVec2::new(-1_800.0, 1_400.0),
            landmark_world.xz(),
        ],
    ];
    control_paths
        .into_iter()
        .enumerate()
        .map(|(index, controls)| compile_route(atlas, index, &controls))
        .collect()
}

fn compile_route(
    atlas: &RegionAtlas,
    route_index: usize,
    controls: &[DVec2],
) -> Result<CompiledRoute, RegionScaleError> {
    let xz_points = densify_controls(controls, ROUTE_SAMPLE_SPACING_M);
    let bridge_structure = EntityId::from_u128(stable_u128(
        atlas.world_seed,
        STAGE_STRUCTURE,
        route_index as u128,
        1,
    ));
    let mut points = Vec::with_capacity(xz_points.len());
    for point in xz_points {
        let terrain = terrain_height(atlas.world_seed, point.x, point.y);
        let river_distance = (point.y - river_center_z(point.x)).abs();
        let is_bridge =
            route_index == 0 && (650.0..=1_400.0).contains(&point.x) && river_distance <= 190.0;
        let (surface, structure_id, height) = if is_bridge {
            let bank_left =
                terrain_height(atlas.world_seed, point.x, river_center_z(point.x) - 260.0);
            let bank_right =
                terrain_height(atlas.world_seed, point.x, river_center_z(point.x) + 260.0);
            (
                RouteSurfaceKind::Bridge,
                Some(bridge_structure),
                bank_left.max(bank_right).max(terrain) + 7.0,
            )
        } else {
            (
                RouteSurfaceKind::SurfaceConforming,
                None,
                terrain + SURFACE_OFFSET_M,
            )
        };
        points.push(RoutePoint {
            world_position: DVec3::new(point.x, height, point.y),
            terrain_height_m: terrain,
            cell: atlas.nearest_cell(point),
            surface,
            structure_id,
        });
    }
    let crossed_cells =
        points
            .iter()
            .map(|point| point.cell)
            .fold(Vec::<HexCoord>::new(), |mut cells, cell| {
                if cells.last().copied() != Some(cell) {
                    cells.push(cell);
                }
                cells
            });
    let maximum_surface_grade = points
        .windows(2)
        .filter(|pair| {
            pair[0].surface == RouteSurfaceKind::SurfaceConforming
                && pair[1].surface == RouteSurfaceKind::SurfaceConforming
        })
        .map(|pair| {
            let horizontal = pair[0]
                .world_position
                .xz()
                .distance(pair[1].world_position.xz())
                .max(0.01);
            (pair[0].world_position.y - pair[1].world_position.y).abs() / horizontal
        })
        .fold(0.0_f64, f64::max);
    let mut route = CompiledRoute {
        id: RouteId::from_u128(stable_u128(
            atlas.world_seed,
            STAGE_ROUTE,
            route_index as u128,
            1,
        )),
        road_id: RoadId::from_u128(stable_u128(
            atlas.world_seed,
            STAGE_ROUTE,
            route_index as u128,
            2,
        )),
        target_landmark_id: atlas.landmark_id,
        points,
        crossed_cells,
        maximum_surface_grade,
        semantic_fingerprint: 0,
    };
    route.semantic_fingerprint = fingerprint(&route)?;
    Ok(route)
}

fn densify_controls(controls: &[DVec2], spacing: f64) -> Vec<DVec2> {
    let mut points = Vec::new();
    for pair in controls.windows(2) {
        let distance = pair[0].distance(pair[1]);
        let steps = (distance / spacing).ceil().max(1.0) as usize;
        for step in 0..steps {
            let t = step as f64 / steps as f64;
            let point = pair[0].lerp(pair[1], t);
            if points.last().copied() != Some(point) {
                points.push(point);
            }
        }
    }
    if let Some(last) = controls.last().copied() {
        points.push(last);
    }
    points
}

fn maximum_shared_boundary_error(world: &RegionScaleWorld) -> Result<f64, RegionScaleError> {
    let mut maximum = 0.0_f64;
    for cell in &world.atlas.cells {
        let left = materialize_cell(world, cell.coord, CellLod::Full)?;
        for direction in HexDirection::ALL {
            let neighbor_coord = cell.coord.neighbor(direction);
            if cell.coord >= neighbor_coord || world.atlas.cell(neighbor_coord).is_none() {
                continue;
            }
            let right = materialize_cell(world, neighbor_coord, CellLod::Full)?;
            let left_profile = left
                .boundary_profiles
                .iter()
                .find(|profile| profile.direction == direction)
                .expect("all cell edges have profiles");
            let right_profile = right
                .boundary_profiles
                .iter()
                .find(|profile| profile.direction == direction.opposite())
                .expect("neighbor opposite edge exists");
            for (left_point, right_point) in left_profile
                .world_points
                .iter()
                .zip(right_profile.world_points.iter().rev())
            {
                maximum = maximum.max(left_point.distance(*right_point));
            }
        }
    }
    Ok(maximum)
}

fn check(name: &str, passed: bool, detail: String) -> ValidationCheck {
    ValidationCheck {
        name: name.to_owned(),
        passed,
        detail,
    }
}

fn fingerprint<T: Serialize>(value: &T) -> Result<u64, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    }))
}

fn stable_u128(world_seed: u128, stage: u64, spatial: u128, feature: u128) -> u128 {
    let high = SeedMaterial {
        world_seed,
        stage_id: stage,
        spatial_key: spatial,
        feature_key: feature,
    }
    .derive();
    let low = SeedMaterial {
        world_seed,
        stage_id: stage ^ 0x9e37_79b9_7f4a_7c15,
        spatial_key: spatial ^ feature.rotate_left(17),
        feature_key: feature ^ 0xa5a5_a5a5_a5a5_a5a5,
    }
    .derive();
    (u128::from(high) << 64) | u128::from(low)
}

fn coord_key(coord: HexCoord) -> u128 {
    (u128::from(coord.q as i32 as u32) << 32) | u128::from(coord.r as i32 as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deterministic_rng::DeterministicRng;

    #[test]
    fn atlas_uses_region_scale_cells() {
        let world = build_world().expect("world");
        assert_eq!(world.atlas.cells.len(), 19);
        assert!((world.atlas.flat_to_flat_m - 4_000.0).abs() < 1.0e-9);
        assert!(
            world
                .atlas
                .cells
                .iter()
                .all(|cell| cell.expected_tile_count >= 120)
        );
    }

    #[test]
    fn materialization_is_cell_scoped_and_rebuildable() {
        let world = build_world().expect("world");
        let first = activate_cell(&world, world.atlas.port_cell).expect("first");
        let second = activate_cell(&world, world.atlas.port_cell).expect("second");
        assert_eq!(first, second);
        assert_eq!(first.focused.coord, world.atlas.port_cell);
        assert!(first.neighbor_proxies.len() <= 6);
        assert!(
            first
                .neighbor_proxies
                .iter()
                .all(|proxy| proxy.resolution == PROXY_RESOLUTION)
        );
    }

    #[test]
    fn buildings_are_grounded_with_foundations() {
        let world = build_world().expect("world");
        let cell = materialize_cell(&world, world.atlas.port_cell, CellLod::Full).expect("cell");
        assert!(!cell.buildings.is_empty());
        for building in &cell.buildings {
            assert!(building.foundation_bottom_m <= building.sampled_ground_min_m);
            assert!(building.foundation_top_m >= building.sampled_ground_max_m);
            assert!(
                building.grounded_center.y
                    >= building.foundation_top_m + building.body_height_m * 0.5
            );
        }
    }

    #[test]
    fn routes_never_fake_a_smooth_profile_inside_terrain() {
        let world = build_world().expect("world");
        for route in &world.routes {
            assert!(
                route
                    .crossed_cells
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    >= 3
            );
            for point in &route.points {
                match point.surface {
                    RouteSurfaceKind::SurfaceConforming => assert!(
                        (point.world_position.y - point.terrain_height_m - SURFACE_OFFSET_M).abs()
                            <= 1.0e-9
                    ),
                    RouteSurfaceKind::Bridge => {
                        assert!(point.structure_id.is_some());
                        assert!(point.world_position.y > point.terrain_height_m + 2.0);
                    }
                }
            }
        }
    }

    #[test]
    fn full_validation_passes() {
        let world = build_world().expect("world");
        let report = validate_world(&world).expect("report");
        let failures = report
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| (&check.name, &check.detail))
            .collect::<Vec<_>>();
        assert!(failures.is_empty(), "failed checks: {failures:?}");
    }

    #[test]
    fn separate_rng_streams_do_not_change_world() {
        let world = build_world().expect("world");
        let mut unrelated = DeterministicRng::from_material(SeedMaterial {
            world_seed: WORLD_SEED,
            stage_id: 0xdead,
            spatial_key: 0xbeef,
            feature_key: 0xcafe,
        });
        for _ in 0..10_000 {
            unrelated.next_u64();
        }
        let rebuilt = build_world().expect("rebuilt world");
        assert_eq!(world, rebuilt);
    }
}
