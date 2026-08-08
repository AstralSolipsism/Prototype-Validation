use crate::model::*;
use crate::validate::{BuildingError, ValidationReport, index_blueprint, validate_blueprint};

pub fn impact_for_delta(
    blueprint: &BuildingBlueprint,
    delta: &BlueprintDelta,
) -> Result<DirtySet, BuildingError> {
    let index = index_blueprint(blueprint);
    let mut dirty = DirtySet::default();

    match delta {
        BlueprintDelta::AddOpening(opening) => {
            let wall = index
                .walls
                .get(&opening.wall_id)
                .copied()
                .ok_or(BuildingError::DeltaUnknownWall(opening.wall_id))?;
            mark_wall_context(&mut dirty, wall);
            dirty.openings.insert(opening.id);
            dirty.mesh_layers.insert(ChunkLayer::Structure);
            dirty.rebuild_hlod = true;
            if opening.is_traversable() {
                dirty.rebuild_collision = true;
                dirty.rebuild_navigation = true;
            }
            if wall.is_exterior() {
                dirty.mesh_layers.insert(ChunkLayer::ExteriorShell);
                dirty.rebuild_exterior_shell = true;
                dirty.rebuild_cutaway_groups = true;
            }
        }
        BlueprintDelta::RemoveOpening(opening_id) => {
            let opening = index
                .openings
                .get(opening_id)
                .copied()
                .ok_or(BuildingError::DeltaUnknownOpening(*opening_id))?;
            let wall = index.walls[&opening.wall_id];
            mark_wall_context(&mut dirty, wall);
            dirty.openings.insert(*opening_id);
            dirty.mesh_layers.insert(ChunkLayer::Structure);
            dirty.rebuild_hlod = true;
            if opening.is_traversable() {
                dirty.rebuild_collision = true;
                dirty.rebuild_navigation = true;
            }
            if wall.is_exterior() {
                dirty.mesh_layers.insert(ChunkLayer::ExteriorShell);
                dirty.rebuild_exterior_shell = true;
                dirty.rebuild_cutaway_groups = true;
            }
        }
        BlueprintDelta::ReplaceWall(replacement) => {
            let existing = index
                .walls
                .get(&replacement.id)
                .copied()
                .ok_or(BuildingError::DeltaUnknownWall(replacement.id))?;
            mark_wall_context(&mut dirty, existing);
            mark_wall_context(&mut dirty, replacement);
            dirty.mesh_layers.insert(ChunkLayer::Structure);
            dirty.mesh_layers.insert(ChunkLayer::ExteriorShell);
            dirty.rebuild_collision = true;
            dirty.rebuild_navigation = true;
            dirty.rebuild_exterior_shell = true;
            dirty.rebuild_cutaway_groups = true;
            dirty.rebuild_massing = true;
            dirty.rebuild_hlod = true;
        }
        BlueprintDelta::ChangeRoomPurpose { room_id, .. } => {
            let (_, level_id) = index
                .rooms
                .get(room_id)
                .copied()
                .ok_or(BuildingError::DeltaUnknownRoom(*room_id))?;
            dirty.rooms.insert(*room_id);
            dirty.levels.insert(level_id);
            dirty.rebuild_navigation = true;
        }
    }

    Ok(dirty)
}

pub fn apply_delta(
    blueprint: &BuildingBlueprint,
    delta: &BlueprintDelta,
) -> Result<BuildingBlueprint, ValidationReport> {
    impact_for_delta(blueprint, delta).map_err(|error| ValidationReport::new(vec![error]))?;
    let mut updated = blueprint.clone();

    match delta {
        BlueprintDelta::AddOpening(opening) => updated.openings.push(opening.clone()),
        BlueprintDelta::RemoveOpening(opening_id) => {
            updated.openings.retain(|opening| opening.id != *opening_id);
            if updated.primary_entry == *opening_id {
                return Err(ValidationReport::new(vec![
                    BuildingError::InvalidPrimaryEntry(*opening_id),
                ]));
            }
        }
        BlueprintDelta::ReplaceWall(replacement) => {
            let wall = updated
                .walls
                .iter_mut()
                .find(|wall| wall.id == replacement.id)
                .expect("impact validation ensured wall exists");
            *wall = replacement.clone();
        }
        BlueprintDelta::ChangeRoomPurpose { room_id, purpose } => {
            let room = updated
                .levels
                .iter_mut()
                .flat_map(|level| &mut level.rooms)
                .find(|room| room.id == *room_id)
                .expect("impact validation ensured room exists");
            room.purpose = *purpose;
        }
    }

    updated.revision = updated.revision.saturating_add(1);
    validate_blueprint(&updated)?;
    Ok(updated)
}

fn mark_wall_context(dirty: &mut DirtySet, wall: &WallRun) {
    dirty.walls.insert(wall.id);
    dirty.levels.insert(wall.level_id);
    for room_id in [wall.left_room, wall.right_room].into_iter().flatten() {
        dirty.rooms.insert(room_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use world_ids::{BuildingLevelId, OpeningId, RoomId, WallId};

    fn wall() -> WallRun {
        WallRun {
            id: WallId::from_u128(1),
            level_id: BuildingLevelId::from_u128(2),
            start: DVec2::ZERO,
            end: DVec2::new(6.0, 0.0),
            thickness_m: 0.2,
            height_m: 3.0,
            left_room: Some(RoomId::from_u128(3)),
            right_room: None,
        }
    }

    #[test]
    fn exterior_window_rebuilds_shell_but_not_massing() {
        let blueprint = BuildingBlueprint {
            id: world_ids::BuildingId::from_u128(10),
            style_id: world_ids::ArchitectureStyleId::from_u128(11),
            revision: 1,
            levels: vec![BuildingLevel {
                id: BuildingLevelId::from_u128(2),
                index: 0,
                elevation_m: 0.0,
                height_m: 3.0,
                rooms: vec![Room {
                    id: RoomId::from_u128(3),
                    footprint: Rect2::new(DVec2::ZERO, DVec2::new(6.0, 4.0)),
                    purpose: RoomPurpose::Entry,
                }],
            }],
            walls: vec![wall()],
            openings: vec![Opening {
                id: OpeningId::from_u128(4),
                wall_id: WallId::from_u128(1),
                kind: OpeningKind::Door,
                offset_m: 0.5,
                width_m: 1.0,
                sill_m: 0.0,
                height_m: 2.1,
                from: SpaceRef::Room(RoomId::from_u128(3)),
                to: SpaceRef::Exterior,
            }],
            stairs: vec![],
            roof_regions: vec![],
            primary_entry: OpeningId::from_u128(4),
        };
        let delta = BlueprintDelta::AddOpening(Opening {
            id: OpeningId::from_u128(5),
            wall_id: WallId::from_u128(1),
            kind: OpeningKind::Window,
            offset_m: 2.0,
            width_m: 1.2,
            sill_m: 1.0,
            height_m: 1.2,
            from: SpaceRef::Room(RoomId::from_u128(3)),
            to: SpaceRef::Exterior,
        });
        let dirty = impact_for_delta(&blueprint, &delta).expect("impact");
        assert!(dirty.rebuild_exterior_shell);
        assert!(dirty.rebuild_cutaway_groups);
        assert!(dirty.rebuild_hlod);
        assert!(!dirty.rebuild_massing);
        assert!(!dirty.rebuild_navigation);
        assert!(!dirty.rebuild_collision);
    }
}
