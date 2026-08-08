use crate::hex::{BoundaryKey, HexCoord};
use glam::DVec3;
use scroll_camera_core::ScrollGrammar;
use serde::{Deserialize, Serialize};
use world_ids::{
    BuildingId, BuildingInstanceId, CellId, LandmarkId, PortalId, RiverId, RoadId, RouteId,
    SettlementId, WorldId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorVersion(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalOrder {
    Canonical,
    Reverse,
    Parity,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldGenerationConfig {
    pub world_id: WorldId,
    pub world_seed: u128,
    pub generator_version: GeneratorVersion,
    pub radius: i16,
    pub cell_radius_m: f64,
    pub building_blueprint_id: BuildingId,
}

impl WorldGenerationConfig {
    pub fn validate(self) -> bool {
        self.radius >= 1
            && self.radius <= 8
            && self.cell_radius_m.is_finite()
            && self.cell_radius_m >= 32.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceClass {
    Ocean,
    Coast,
    Lowland,
    Highland,
    Alpine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Ocean,
    CoastalGrassland,
    Wetland,
    TemperateForest,
    UplandMeadow,
    AlpineRock,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Climate {
    pub temperature_c: f64,
    pub moisture: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasCellManifest {
    pub id: CellId,
    pub coord: HexCoord,
    pub center_world: DVec3,
    pub elevation_m: f64,
    pub surface: SurfaceClass,
    pub climate: Climate,
    pub biome: Biome,
    pub travel_cost: f64,
    pub downstream: Option<HexCoord>,
    pub edge_elevations_m: [f64; 6],
    pub road_ids: Vec<RoadId>,
    pub route_ids: Vec<RouteId>,
    pub portal_ids: Vec<PortalId>,
}

impl AtlasCellManifest {
    pub fn is_ocean(&self) -> bool {
        self.surface == SurfaceClass::Ocean
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortalKind {
    River,
    Road,
    ScrollRoute,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundaryPortal {
    pub id: PortalId,
    pub boundary: BoundaryKey,
    pub kind: PortalKind,
    pub world_position: DVec3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RiverManifest {
    pub id: RiverId,
    pub cells: Vec<HexCoord>,
    pub boundaries: Vec<BoundaryKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadPurpose {
    WesternApproach,
    NorthernApproach,
    SouthernApproach,
    HarborSpine,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoadManifest {
    pub id: RoadId,
    pub purpose: RoadPurpose,
    pub cells: Vec<HexCoord>,
    pub boundaries: Vec<BoundaryKey>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteNode {
    pub cell: HexCoord,
    pub world_position: DVec3,
    pub grammar: ScrollGrammar,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollRouteManifest {
    pub id: RouteId,
    pub road_id: RoadId,
    pub cells: Vec<HexCoord>,
    pub nodes: Vec<RouteNode>,
    pub visible_landmark_ids: Vec<LandmarkId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementKind {
    PortTown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SettlementReason {
    FreshWater,
    ShelteredCoast,
    BuildableSlope,
    RoadConvergence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingPlacement {
    pub instance_id: BuildingInstanceId,
    pub blueprint_id: BuildingId,
    pub local_position: DVec3,
    pub yaw_radians: f64,
    pub half_extents_m: DVec3,
    pub entry_route_id: RouteId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettlementManifest {
    pub id: SettlementId,
    pub kind: SettlementKind,
    pub cell: HexCoord,
    pub reasons: Vec<SettlementReason>,
    pub buildings: Vec<BuildingPlacement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandmarkKind {
    MountainTower,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LandmarkManifest {
    pub id: LandmarkId,
    pub kind: LandmarkKind,
    pub cell: HexCoord,
    pub world_position: DVec3,
    pub visible_radius_m: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationStageDigest {
    pub stage: String,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldManifest {
    pub world_id: WorldId,
    pub world_seed: u128,
    pub generator_version: GeneratorVersion,
    pub radius: i16,
    pub cell_radius_m: f64,
    pub cells: Vec<AtlasCellManifest>,
    pub river: RiverManifest,
    pub coastline: Vec<BoundaryKey>,
    pub roads: Vec<RoadManifest>,
    pub routes: Vec<ScrollRouteManifest>,
    pub portals: Vec<BoundaryPortal>,
    pub settlements: Vec<SettlementManifest>,
    pub landmarks: Vec<LandmarkManifest>,
    pub stage_digests: Vec<GenerationStageDigest>,
}

impl WorldManifest {
    pub fn cell(&self, coord: HexCoord) -> Option<&AtlasCellManifest> {
        self.cells.iter().find(|cell| cell.coord == coord)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldCompilation {
    pub manifest: WorldManifest,
    pub semantic_fingerprint: u64,
}
