use glam::{DVec2, DVec3};
use scroll_camera_core::ScrollGrammar;
use serde::{Deserialize, Serialize};
use world_generation_core::{
    Biome, BoundaryKey, Climate, GeneratorVersion, HexCoord, HexDirection,
};
use world_ids::{
    BuildingInstanceId, CellId, EntityId, EventId, LandmarkId, RegionId, RoadId, RouteId,
    SettlementId, WorldId,
};

pub const ATLAS_INTERNAL_SAMPLE_RESOLUTION: u16 = 17;
pub const CELL_TERRAIN_RESOLUTION: u16 = 33;
pub const REGION_TERRAIN_RESOLUTION: u16 = 97;
pub const EDGE_PROFILE_SAMPLES: usize = 33;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LandformClass {
    Ocean,
    Coast,
    Estuary,
    Floodplain,
    Valley,
    Lowland,
    Terrace,
    Hillslope,
    Ridge,
    Mountain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LandCover {
    OpenWater,
    Wetland,
    Cropland,
    Grassland,
    Forest,
    Scrub,
    BareRock,
    Built,
    Ruins,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ElevationSummary {
    pub minimum_m: f64,
    pub maximum_m: f64,
    pub mean_m: f64,
    pub median_m: f64,
    pub relief_m: f64,
    pub mean_slope: f64,
    pub buildable_fraction: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LandformMix {
    pub mountain: f64,
    pub ridge: f64,
    pub hillslope: f64,
    pub valley: f64,
    pub lowland: f64,
    pub coast: f64,
    pub water: f64,
}

impl LandformMix {
    pub fn total(self) -> f64 {
        self.mountain
            + self.ridge
            + self.hillslope
            + self.valley
            + self.lowland
            + self.coast
            + self.water
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceSummary {
    pub fresh_water: f64,
    pub arable_land: f64,
    pub timber: f64,
    pub stone: f64,
    pub fishery: f64,
    pub harbor_quality: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AtlasFeatureKind {
    MountainRange,
    RidgeLine,
    ValleyLine,
    River,
    Coastline,
    RoadCorridor,
    Settlement,
    Landmark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasFeature {
    pub id: EntityId,
    pub kind: AtlasFeatureKind,
    pub path_world: Vec<DVec3>,
    pub touched_cells: Vec<HexCoord>,
    pub importance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeSample {
    pub t: f64,
    pub world_position: DVec3,
    pub landform: LandformClass,
    pub river_width_m: f64,
    pub road_width_m: f64,
    pub is_coastline: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundaryContract {
    pub id: EntityId,
    pub boundary: BoundaryKey,
    pub direction_from_low: HexDirection,
    pub samples: Vec<EdgeSample>,
    pub feature_ids: Vec<EntityId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasHistorySummary {
    pub first_settlement_year: Option<i32>,
    pub current_population: u32,
    pub dominant_economy: String,
    pub event_ids: Vec<EventId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasCellSpec {
    pub id: CellId,
    pub coord: HexCoord,
    pub center_world: DVec3,
    pub elevation: ElevationSummary,
    pub landforms: LandformMix,
    pub climate: Climate,
    pub biome: Biome,
    pub resources: ResourceSummary,
    pub carrying_capacity: f64,
    pub predominant_drainage: Option<HexDirection>,
    pub feature_ids: Vec<EntityId>,
    pub boundary_contract_ids: Vec<EntityId>,
    pub history: AtlasHistorySummary,
    pub materialization_version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldAtlasManifest {
    pub world_id: WorldId,
    pub world_seed: u128,
    pub generator_version: GeneratorVersion,
    pub cell_radius_m: f64,
    pub cells: Vec<AtlasCellSpec>,
    pub boundary_contracts: Vec<BoundaryContract>,
    pub features: Vec<AtlasFeature>,
    pub settlement_id: SettlementId,
    pub landmark_id: LandmarkId,
    pub base_world_fingerprint: u64,
    pub atlas_fingerprint: u64,
}

impl WorldAtlasManifest {
    pub fn cell(&self, coord: HexCoord) -> Option<&AtlasCellSpec> {
        self.cells.iter().find(|cell| cell.coord == coord)
    }

    pub fn contract(&self, id: EntityId) -> Option<&BoundaryContract> {
        self.boundary_contracts
            .iter()
            .find(|contract| contract.id == id)
    }

    pub fn feature(&self, id: EntityId) -> Option<&AtlasFeature> {
        self.features.iter().find(|feature| feature.id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainSample {
    pub grid_x: u16,
    pub grid_z: u16,
    pub world_position: DVec3,
    pub slope: f64,
    pub flow_accumulation: f64,
    pub landform: LandformClass,
    pub land_cover: LandCover,
    pub travel_cost: f64,
    pub buildable: bool,
    pub cell: Option<HexCoord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainGrid {
    pub width: u16,
    pub height: u16,
    pub origin_xz: DVec2,
    pub spacing_m: f64,
    pub samples: Vec<TerrainSample>,
}

impl TerrainGrid {
    pub fn index(&self, x: usize, z: usize) -> usize {
        z * usize::from(self.width) + x
    }

    pub fn sample(&self, x: usize, z: usize) -> Option<&TerrainSample> {
        if x >= usize::from(self.width) || z >= usize::from(self.height) {
            return None;
        }
        self.samples.get(self.index(x, z))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellEdgeProfile {
    pub contract_id: EntityId,
    pub boundary: BoundaryKey,
    pub samples: Vec<EdgeSample>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetailedCell {
    pub atlas_cell_id: CellId,
    pub coord: HexCoord,
    pub resolution: u16,
    pub samples: Vec<TerrainSample>,
    pub edge_profiles: Vec<CellEdgeProfile>,
    pub recomputed_elevation: ElevationSummary,
    pub recomputed_landforms: LandformMix,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetailedRegion {
    pub materialized_cells: Vec<HexCoord>,
    pub terrain: TerrainGrid,
    pub cells: Vec<DetailedCell>,
    pub semantic_fingerprint: u64,
}

impl DetailedRegion {
    pub fn cell(&self, coord: HexCoord) -> Option<&DetailedCell> {
        self.cells.iter().find(|cell| cell.coord == coord)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HistoryEventKind {
    Migration,
    SettlementFounded,
    HarborConstructed,
    AgricultureExpanded,
    RoadAndBridgeBuilt,
    FortificationRaised,
    Conflict,
    Flood,
    Reconstruction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub id: EventId,
    pub year: i32,
    pub kind: HistoryEventKind,
    pub location: DVec3,
    pub causes: Vec<EventId>,
    pub created_asset_ids: Vec<EntityId>,
    pub retired_asset_ids: Vec<EntityId>,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HistoricalAssetKind {
    Harbor,
    Bridge,
    OldRoad,
    NewRoad,
    OldQuarter,
    NewQuarter,
    Farmland,
    Fortification,
    Ruins,
    Monument,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoricalAsset {
    pub id: EntityId,
    pub kind: HistoricalAssetKind,
    pub anchor_world: DVec3,
    pub extent_m: DVec2,
    pub created_by: EventId,
    pub retired_by: Option<EventId>,
    pub source_cell: HexCoord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LandUseKind {
    Harbor,
    OldTown,
    NewTown,
    Farmland,
    Fortification,
    Ruins,
    Commons,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LandUseZone {
    pub id: RegionId,
    pub kind: LandUseKind,
    pub center_world: DVec3,
    pub radius_m: f64,
    pub established_by: EventId,
    pub source_cell: HexCoord,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryLedger {
    pub settlement_id: SettlementId,
    pub events: Vec<HistoryEvent>,
    pub assets: Vec<HistoricalAsset>,
    pub land_use: Vec<LandUseZone>,
    pub current_year: i32,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraversalSurface {
    Trail,
    Road,
    Bridge,
    Plaza,
    BuildingPortal,
    Dock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraversalMode {
    Walking,
    Wagon,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraversalNode {
    pub id: EntityId,
    pub world_position: DVec3,
    pub cell: HexCoord,
    pub surface: TraversalSurface,
    pub modes: Vec<TraversalMode>,
    pub source_asset_id: Option<EntityId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraversalEdge {
    pub from: EntityId,
    pub to: EntityId,
    pub length_m: f64,
    pub maximum_grade: f64,
    pub minimum_width_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledScrollRoute {
    pub id: RouteId,
    pub road_id: RoadId,
    pub target_landmark_id: LandmarkId,
    pub node_ids: Vec<EntityId>,
    pub world_path: Vec<DVec3>,
    pub grammars: Vec<ScrollGrammar>,
    pub crossed_cells: Vec<HexCoord>,
    pub maximum_grade: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraversalCompilation {
    pub nodes: Vec<TraversalNode>,
    pub edges: Vec<TraversalEdge>,
    pub routes: Vec<CompiledScrollRoute>,
    pub target_landmark_id: LandmarkId,
    pub semantic_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossScaleObjectBinding {
    pub object_id: EntityId,
    pub atlas_cell: HexCoord,
    pub local_anchor: DVec3,
    pub atlas_feature_id: Option<EntityId>,
    pub history_event_id: Option<EventId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossScaleReport {
    pub checks: Vec<ValidationCheck>,
    pub bindings: Vec<CrossScaleObjectBinding>,
    pub atlas_fingerprint: u64,
    pub detailed_fingerprint: u64,
    pub history_fingerprint: u64,
    pub traversal_fingerprint: u64,
    pub integrated_fingerprint: u64,
}

impl CrossScaleReport {
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntegratedWorld {
    pub atlas: WorldAtlasManifest,
    pub detailed: DetailedRegion,
    pub history: HistoryLedger,
    pub traversal: TraversalCompilation,
    pub report: CrossScaleReport,
    pub building_instances: Vec<BuildingInstanceId>,
}
