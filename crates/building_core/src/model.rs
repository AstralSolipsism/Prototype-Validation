use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use world_ids::{
    ArchitectureStyleId, BuildingId, BuildingLevelId, FrameId, OpeningId, RoofRegionId, RoomId,
    StairId, WallId,
};

pub const BUILDING_CHUNK_SIZE_METERS: f64 = 8.0;
pub const BUILDING_EPSILON: f64 = 1.0e-6;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect2 {
    pub min: DVec2,
    pub max: DVec2,
}

impl Rect2 {
    pub const fn new(min: DVec2, max: DVec2) -> Self {
        Self { min, max }
    }

    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn depth(self) -> f64 {
        self.max.y - self.min.y
    }

    pub fn center(self) -> DVec2 {
        (self.min + self.max) * 0.5
    }

    pub fn is_valid(self) -> bool {
        self.min.is_finite()
            && self.max.is_finite()
            && self.width() > BUILDING_EPSILON
            && self.depth() > BUILDING_EPSILON
    }

    pub fn overlaps_area(self, other: Self) -> bool {
        let overlap_x = self.max.x.min(other.max.x) - self.min.x.max(other.min.x);
        let overlap_y = self.max.y.min(other.max.y) - self.min.y.max(other.min.y);
        overlap_x > BUILDING_EPSILON && overlap_y > BUILDING_EPSILON
    }

    pub fn expanded(self, amount: f64) -> Self {
        let expansion = DVec2::splat(amount);
        Self {
            min: self.min - expansion,
            max: self.max + expansion,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb3 {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb3 {
    pub const fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    pub fn from_rect(rect: Rect2, bottom: f64, top: f64) -> Self {
        Self {
            min: DVec3::new(rect.min.x, bottom, rect.min.y),
            max: DVec3::new(rect.max.x, top, rect.max.y),
        }
    }

    pub fn center(self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(self) -> DVec3 {
        self.max - self.min
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn is_valid(self) -> bool {
        self.min.is_finite()
            && self.max.is_finite()
            && self.max.x >= self.min.x
            && self.max.y >= self.min.y
            && self.max.z >= self.min.z
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoomPurpose {
    Entry,
    Living,
    Sleeping,
    Cooking,
    Storage,
    Workshop,
    Retail,
    Circulation,
    Utility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
    Passage,
}

impl OpeningKind {
    pub const fn is_traversable(self) -> bool {
        matches!(self, Self::Door | Self::Passage)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoofKind {
    Flat,
    GableX,
    GableZ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SpaceRef {
    Exterior,
    Room(RoomId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingBlueprint {
    pub id: BuildingId,
    pub style_id: ArchitectureStyleId,
    pub revision: u64,
    pub levels: Vec<BuildingLevel>,
    pub walls: Vec<WallRun>,
    pub openings: Vec<Opening>,
    pub stairs: Vec<Stair>,
    pub roof_regions: Vec<RoofRegion>,
    pub primary_entry: OpeningId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingLevel {
    pub id: BuildingLevelId,
    pub index: i16,
    pub elevation_m: f64,
    pub height_m: f64,
    pub rooms: Vec<Room>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: RoomId,
    pub footprint: Rect2,
    pub purpose: RoomPurpose,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallRun {
    pub id: WallId,
    pub level_id: BuildingLevelId,
    pub start: DVec2,
    pub end: DVec2,
    pub thickness_m: f64,
    pub height_m: f64,
    pub left_room: Option<RoomId>,
    pub right_room: Option<RoomId>,
}

impl WallRun {
    pub fn length_m(&self) -> f64 {
        self.start.distance(self.end)
    }

    pub fn direction(&self) -> DVec2 {
        (self.end - self.start).normalize_or_zero()
    }

    pub fn point_at(&self, distance_m: f64) -> DVec2 {
        self.start + self.direction() * distance_m
    }

    pub fn is_exterior(&self) -> bool {
        self.left_room.is_none() || self.right_room.is_none()
    }

    pub fn spaces(&self) -> [SpaceRef; 2] {
        [
            self.left_room.map_or(SpaceRef::Exterior, SpaceRef::Room),
            self.right_room.map_or(SpaceRef::Exterior, SpaceRef::Room),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    pub id: OpeningId,
    pub wall_id: WallId,
    pub kind: OpeningKind,
    pub offset_m: f64,
    pub width_m: f64,
    pub sill_m: f64,
    pub height_m: f64,
    pub from: SpaceRef,
    pub to: SpaceRef,
}

impl Opening {
    pub fn end_offset_m(&self) -> f64 {
        self.offset_m + self.width_m
    }

    pub const fn is_traversable(&self) -> bool {
        self.kind.is_traversable()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stair {
    pub id: StairId,
    pub from_level: BuildingLevelId,
    pub to_level: BuildingLevelId,
    pub from_room: RoomId,
    pub to_room: RoomId,
    pub footprint: Rect2,
    pub width_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoofRegion {
    pub id: RoofRegionId,
    pub level_id: BuildingLevelId,
    pub footprint: Rect2,
    pub kind: RoofKind,
    pub base_elevation_m: f64,
    pub height_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingInstanceBinding {
    pub building_id: BuildingId,
    pub frame_id: FrameId,
    pub local_translation: DVec3,
    pub local_yaw_radians: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomGraph {
    pub adjacency: BTreeMap<RoomId, BTreeSet<RoomId>>,
    pub exterior_entries: BTreeSet<RoomId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalEdge {
    pub opening_id: OpeningId,
    pub from: SpaceRef,
    pub to: SpaceRef,
    pub traversable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalGraph {
    pub edges: Vec<PortalEdge>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollisionWallSpan {
    pub wall_id: WallId,
    pub level_id: BuildingLevelId,
    pub start: DVec2,
    pub end: DVec2,
    pub bottom_m: f64,
    pub height_m: f64,
    pub thickness_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollisionProxy {
    pub wall_spans: Vec<CollisionWallSpan>,
    pub floor_bounds: Vec<Aabb3>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavigationPortal {
    pub opening_id: OpeningId,
    pub position: DVec3,
    pub width_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavigationPatch {
    pub room_id: RoomId,
    pub level_id: BuildingLevelId,
    pub walkable: Rect2,
    pub portals: Vec<NavigationPortal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ChunkLayer {
    Structure,
    Floor,
    Roof,
    ExteriorShell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChunkKey {
    pub level_index: i16,
    pub x: i32,
    pub z: i32,
    pub layer: ChunkLayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ElementRef {
    Wall(WallId),
    Opening(OpeningId),
    Room(RoomId),
    Stair(StairId),
    Roof(RoofRegionId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshChunkPlan {
    pub key: ChunkKey,
    pub bounds: Aabb3,
    pub elements: BTreeSet<ElementRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CutawayDirection {
    North,
    South,
    East,
    West,
    Roof,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutawayGroup {
    pub direction: CutawayDirection,
    pub elements: BTreeSet<ElementRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExteriorShell {
    pub elements: BTreeSet<ElementRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MassingProxy {
    pub bounds: Aabb3,
    pub level_bounds: BTreeMap<BuildingLevelId, Aabb3>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingCompilation {
    pub building_id: BuildingId,
    pub blueprint_revision: u64,
    pub semantic_fingerprint: u64,
    pub bounds: Aabb3,
    pub room_graph: RoomGraph,
    pub portal_graph: PortalGraph,
    pub collision: CollisionProxy,
    pub navigation: Vec<NavigationPatch>,
    pub mesh_chunks: Vec<MeshChunkPlan>,
    pub exterior_shell: ExteriorShell,
    pub cutaway_groups: Vec<CutawayGroup>,
    pub massing: MassingProxy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlueprintDelta {
    AddOpening(Opening),
    RemoveOpening(OpeningId),
    ReplaceWall(WallRun),
    ChangeRoomPurpose {
        room_id: RoomId,
        purpose: RoomPurpose,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtySet {
    pub levels: BTreeSet<BuildingLevelId>,
    pub rooms: BTreeSet<RoomId>,
    pub walls: BTreeSet<WallId>,
    pub openings: BTreeSet<OpeningId>,
    pub mesh_layers: BTreeSet<ChunkLayer>,
    pub rebuild_collision: bool,
    pub rebuild_navigation: bool,
    pub rebuild_exterior_shell: bool,
    pub rebuild_cutaway_groups: bool,
    pub rebuild_massing: bool,
    pub rebuild_hlod: bool,
}
