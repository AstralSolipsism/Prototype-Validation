use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as StdError;
use std::fmt;
use thiserror::Error;
use world_ids::{BuildingLevelId, OpeningId, RoofRegionId, RoomId, StairId, WallId};

#[derive(Clone, Debug, PartialEq, Error)]
pub enum BuildingError {
    #[error("duplicate building level ID: {0}")]
    DuplicateLevel(BuildingLevelId),
    #[error("building level {0} has non-finite elevation or height")]
    NonFiniteLevel(BuildingLevelId),
    #[error("building level {0} must have positive height")]
    InvalidLevelHeight(BuildingLevelId),
    #[error("duplicate building level index: {0}")]
    DuplicateLevelIndex(i16),
    #[error("duplicate room ID: {0}")]
    DuplicateRoom(RoomId),
    #[error("room {0} has an invalid footprint")]
    InvalidRoomFootprint(RoomId),
    #[error("rooms {left} and {right} overlap on the same level")]
    OverlappingRooms { left: RoomId, right: RoomId },
    #[error("duplicate wall ID: {0}")]
    DuplicateWall(WallId),
    #[error("wall {0} references an unknown level")]
    WallUnknownLevel(WallId),
    #[error("wall {0} has invalid geometry")]
    InvalidWallGeometry(WallId),
    #[error("wall {0} does not border any room")]
    WallWithoutRoom(WallId),
    #[error("wall {wall_id} references unknown room {room_id}")]
    WallUnknownRoom { wall_id: WallId, room_id: RoomId },
    #[error("wall {wall_id} and room {room_id} are on different levels")]
    WallRoomLevelMismatch { wall_id: WallId, room_id: RoomId },
    #[error("duplicate opening ID: {0}")]
    DuplicateOpening(OpeningId),
    #[error("opening {0} references an unknown wall")]
    OpeningUnknownWall(OpeningId),
    #[error("opening {0} has invalid dimensions")]
    InvalidOpeningGeometry(OpeningId),
    #[error("opening {0} extends outside its wall")]
    OpeningOutsideWall(OpeningId),
    #[error("opening {0} extends above its wall")]
    OpeningAboveWall(OpeningId),
    #[error("opening {0} connects spaces that do not match its wall")]
    OpeningSpaceMismatch(OpeningId),
    #[error("openings {left} and {right} overlap on the same wall")]
    OverlappingOpenings { left: OpeningId, right: OpeningId },
    #[error("primary entry {0} does not exist")]
    PrimaryEntryMissing(OpeningId),
    #[error("primary entry {0} must be traversable and connect to exterior")]
    InvalidPrimaryEntry(OpeningId),
    #[error("duplicate stair ID: {0}")]
    DuplicateStair(StairId),
    #[error("stair {0} references an unknown level or room")]
    StairUnknownEndpoint(StairId),
    #[error("stair {0} endpoints do not match their declared levels")]
    StairRoomLevelMismatch(StairId),
    #[error("stair {0} must connect adjacent level indices")]
    StairNonAdjacentLevels(StairId),
    #[error("stair {0} has invalid geometry")]
    InvalidStairGeometry(StairId),
    #[error("duplicate roof region ID: {0}")]
    DuplicateRoof(RoofRegionId),
    #[error("roof region {0} references an unknown level")]
    RoofUnknownLevel(RoofRegionId),
    #[error("roof region {0} has invalid geometry")]
    InvalidRoofGeometry(RoofRegionId),
    #[error("room {0} is not reachable from an exterior entry")]
    InaccessibleRoom(RoomId),
    #[error("delta references unknown opening {0}")]
    DeltaUnknownOpening(OpeningId),
    #[error("delta references unknown wall {0}")]
    DeltaUnknownWall(WallId),
    #[error("delta references unknown room {0}")]
    DeltaUnknownRoom(RoomId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationReport {
    pub errors: Vec<BuildingError>,
}

impl ValidationReport {
    pub fn new(errors: Vec<BuildingError>) -> Self {
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
            "building validation failed with {} error(s):",
            self.errors.len()
        )?;
        for error in &self.errors {
            writeln!(formatter, "- {error}")?;
        }
        Ok(())
    }
}

impl StdError for ValidationReport {}

pub(crate) struct BlueprintIndex<'a> {
    pub levels: BTreeMap<BuildingLevelId, &'a BuildingLevel>,
    pub level_indices: BTreeMap<i16, BuildingLevelId>,
    pub rooms: BTreeMap<RoomId, (&'a Room, BuildingLevelId)>,
    pub walls: BTreeMap<WallId, &'a WallRun>,
    pub openings: BTreeMap<OpeningId, &'a Opening>,
}

pub(crate) fn index_blueprint(blueprint: &BuildingBlueprint) -> BlueprintIndex<'_> {
    let mut levels = BTreeMap::new();
    let mut level_indices = BTreeMap::new();
    let mut rooms = BTreeMap::new();
    let mut walls = BTreeMap::new();
    let mut openings = BTreeMap::new();

    for level in &blueprint.levels {
        levels.entry(level.id).or_insert(level);
        level_indices.entry(level.index).or_insert(level.id);
        for room in &level.rooms {
            rooms.entry(room.id).or_insert((room, level.id));
        }
    }
    for wall in &blueprint.walls {
        walls.entry(wall.id).or_insert(wall);
    }
    for opening in &blueprint.openings {
        openings.entry(opening.id).or_insert(opening);
    }

    BlueprintIndex {
        levels,
        level_indices,
        rooms,
        walls,
        openings,
    }
}

pub fn validate_blueprint(blueprint: &BuildingBlueprint) -> Result<(), ValidationReport> {
    let mut errors = Vec::new();
    let mut level_ids = BTreeSet::new();
    let mut level_indices = BTreeSet::new();
    let mut room_ids = BTreeSet::new();
    let mut wall_ids = BTreeSet::new();
    let mut opening_ids = BTreeSet::new();
    let mut stair_ids = BTreeSet::new();
    let mut roof_ids = BTreeSet::new();

    for level in &blueprint.levels {
        if !level_ids.insert(level.id) {
            errors.push(BuildingError::DuplicateLevel(level.id));
        }
        if !level_indices.insert(level.index) {
            errors.push(BuildingError::DuplicateLevelIndex(level.index));
        }
        if !level.elevation_m.is_finite() || !level.height_m.is_finite() {
            errors.push(BuildingError::NonFiniteLevel(level.id));
        } else if level.height_m <= BUILDING_EPSILON {
            errors.push(BuildingError::InvalidLevelHeight(level.id));
        }

        for room in &level.rooms {
            if !room_ids.insert(room.id) {
                errors.push(BuildingError::DuplicateRoom(room.id));
            }
            if !room.footprint.is_valid() {
                errors.push(BuildingError::InvalidRoomFootprint(room.id));
            }
        }

        for (index, left) in level.rooms.iter().enumerate() {
            for right in level.rooms.iter().skip(index + 1) {
                if left.footprint.overlaps_area(right.footprint) {
                    errors.push(BuildingError::OverlappingRooms {
                        left: left.id,
                        right: right.id,
                    });
                }
            }
        }
    }

    let index = index_blueprint(blueprint);

    for wall in &blueprint.walls {
        if !wall_ids.insert(wall.id) {
            errors.push(BuildingError::DuplicateWall(wall.id));
        }
        if !index.levels.contains_key(&wall.level_id) {
            errors.push(BuildingError::WallUnknownLevel(wall.id));
        }
        if !wall.start.is_finite()
            || !wall.end.is_finite()
            || !wall.thickness_m.is_finite()
            || !wall.height_m.is_finite()
            || wall.length_m() <= BUILDING_EPSILON
            || wall.thickness_m <= BUILDING_EPSILON
            || wall.height_m <= BUILDING_EPSILON
        {
            errors.push(BuildingError::InvalidWallGeometry(wall.id));
        }
        if wall.left_room.is_none() && wall.right_room.is_none() {
            errors.push(BuildingError::WallWithoutRoom(wall.id));
        }
        for room_id in [wall.left_room, wall.right_room].into_iter().flatten() {
            match index.rooms.get(&room_id) {
                None => errors.push(BuildingError::WallUnknownRoom {
                    wall_id: wall.id,
                    room_id,
                }),
                Some((_, level_id)) if *level_id != wall.level_id => {
                    errors.push(BuildingError::WallRoomLevelMismatch {
                        wall_id: wall.id,
                        room_id,
                    });
                }
                Some(_) => {}
            }
        }
    }

    let mut openings_by_wall: BTreeMap<WallId, Vec<&Opening>> = BTreeMap::new();
    for opening in &blueprint.openings {
        if !opening_ids.insert(opening.id) {
            errors.push(BuildingError::DuplicateOpening(opening.id));
        }
        let Some(wall) = index.walls.get(&opening.wall_id) else {
            errors.push(BuildingError::OpeningUnknownWall(opening.id));
            continue;
        };
        openings_by_wall
            .entry(opening.wall_id)
            .or_default()
            .push(opening);

        if !opening.offset_m.is_finite()
            || !opening.width_m.is_finite()
            || !opening.sill_m.is_finite()
            || !opening.height_m.is_finite()
            || opening.offset_m < -BUILDING_EPSILON
            || opening.width_m <= BUILDING_EPSILON
            || opening.sill_m < -BUILDING_EPSILON
            || opening.height_m <= BUILDING_EPSILON
        {
            errors.push(BuildingError::InvalidOpeningGeometry(opening.id));
        }
        if opening.end_offset_m() > wall.length_m() + BUILDING_EPSILON {
            errors.push(BuildingError::OpeningOutsideWall(opening.id));
        }
        if opening.sill_m + opening.height_m > wall.height_m + BUILDING_EPSILON {
            errors.push(BuildingError::OpeningAboveWall(opening.id));
        }

        let mut expected = wall.spaces();
        let mut actual = [opening.from, opening.to];
        expected.sort();
        actual.sort();
        if expected != actual {
            errors.push(BuildingError::OpeningSpaceMismatch(opening.id));
        }
    }

    for openings in openings_by_wall.values_mut() {
        openings.sort_by(|left, right| left.offset_m.total_cmp(&right.offset_m));
        for pair in openings.windows(2) {
            if pair[0].end_offset_m() > pair[1].offset_m + BUILDING_EPSILON {
                errors.push(BuildingError::OverlappingOpenings {
                    left: pair[0].id,
                    right: pair[1].id,
                });
            }
        }
    }

    match index.openings.get(&blueprint.primary_entry) {
        None => errors.push(BuildingError::PrimaryEntryMissing(blueprint.primary_entry)),
        Some(entry)
            if !entry.is_traversable()
                || (entry.from != SpaceRef::Exterior && entry.to != SpaceRef::Exterior) =>
        {
            errors.push(BuildingError::InvalidPrimaryEntry(entry.id));
        }
        Some(_) => {}
    }

    for stair in &blueprint.stairs {
        if !stair_ids.insert(stair.id) {
            errors.push(BuildingError::DuplicateStair(stair.id));
        }
        let endpoints = (
            index.levels.get(&stair.from_level),
            index.levels.get(&stair.to_level),
            index.rooms.get(&stair.from_room),
            index.rooms.get(&stair.to_room),
        );
        let (
            Some(from_level),
            Some(to_level),
            Some((_, from_room_level)),
            Some((_, to_room_level)),
        ) = endpoints
        else {
            errors.push(BuildingError::StairUnknownEndpoint(stair.id));
            continue;
        };
        if *from_room_level != stair.from_level || *to_room_level != stair.to_level {
            errors.push(BuildingError::StairRoomLevelMismatch(stair.id));
        }
        if (from_level.index - to_level.index).abs() != 1 {
            errors.push(BuildingError::StairNonAdjacentLevels(stair.id));
        }
        if !stair.footprint.is_valid()
            || !stair.width_m.is_finite()
            || stair.width_m <= BUILDING_EPSILON
        {
            errors.push(BuildingError::InvalidStairGeometry(stair.id));
        }
    }

    for roof in &blueprint.roof_regions {
        if !roof_ids.insert(roof.id) {
            errors.push(BuildingError::DuplicateRoof(roof.id));
        }
        if !index.levels.contains_key(&roof.level_id) {
            errors.push(BuildingError::RoofUnknownLevel(roof.id));
        }
        if !roof.footprint.is_valid()
            || !roof.base_elevation_m.is_finite()
            || !roof.height_m.is_finite()
            || roof.height_m < 0.0
        {
            errors.push(BuildingError::InvalidRoofGeometry(roof.id));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationReport::new(errors))
    }
}
