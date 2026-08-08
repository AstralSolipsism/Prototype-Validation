use crate::model::*;
use crate::validate::{BuildingError, ValidationReport, index_blueprint, validate_blueprint};
use glam::{DVec2, DVec3};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use world_ids::RoomId;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    Validation(#[from] ValidationReport),
    #[error("building blueprint must contain at least one room")]
    EmptyBuilding,
}

pub fn compile_blueprint(
    blueprint: &BuildingBlueprint,
) -> Result<BuildingCompilation, CompileError> {
    validate_blueprint(blueprint)?;
    let index = index_blueprint(blueprint);

    let mut adjacency: BTreeMap<RoomId, BTreeSet<RoomId>> = index
        .rooms
        .keys()
        .copied()
        .map(|room_id| (room_id, BTreeSet::new()))
        .collect();
    let mut exterior_entries = BTreeSet::new();
    let mut portal_edges = Vec::new();

    let mut openings = blueprint.openings.iter().collect::<Vec<_>>();
    openings.sort_by_key(|opening| opening.id);
    for opening in openings {
        portal_edges.push(PortalEdge {
            opening_id: opening.id,
            from: opening.from,
            to: opening.to,
            traversable: opening.is_traversable(),
        });
        if !opening.is_traversable() {
            continue;
        }
        match (opening.from, opening.to) {
            (SpaceRef::Room(left), SpaceRef::Room(right)) => {
                adjacency.entry(left).or_default().insert(right);
                adjacency.entry(right).or_default().insert(left);
            }
            (SpaceRef::Exterior, SpaceRef::Room(room))
            | (SpaceRef::Room(room), SpaceRef::Exterior) => {
                exterior_entries.insert(room);
            }
            (SpaceRef::Exterior, SpaceRef::Exterior) => {}
        }
    }

    let mut stairs = blueprint.stairs.iter().collect::<Vec<_>>();
    stairs.sort_by_key(|stair| stair.id);
    for stair in stairs {
        adjacency
            .entry(stair.from_room)
            .or_default()
            .insert(stair.to_room);
        adjacency
            .entry(stair.to_room)
            .or_default()
            .insert(stair.from_room);
    }

    let room_graph = RoomGraph {
        adjacency,
        exterior_entries,
    };
    let inaccessible = inaccessible_rooms(&room_graph);
    if !inaccessible.is_empty() {
        return Err(CompileError::Validation(ValidationReport::new(
            inaccessible
                .into_iter()
                .map(BuildingError::InaccessibleRoom)
                .collect(),
        )));
    }

    let portal_graph = PortalGraph {
        edges: portal_edges,
    };
    let collision = compile_collision(blueprint, &index);
    let navigation = compile_navigation(blueprint, &index);
    let massing = compile_massing(blueprint, &index)?;
    let bounds = massing.bounds;
    let exterior_shell = compile_exterior_shell(blueprint);
    let cutaway_groups = compile_cutaway_groups(blueprint);
    let mesh_chunks = compile_mesh_chunks(blueprint, &index, &exterior_shell, bounds);

    Ok(BuildingCompilation {
        building_id: blueprint.id,
        blueprint_revision: blueprint.revision,
        semantic_fingerprint: semantic_fingerprint(blueprint),
        bounds,
        room_graph,
        portal_graph,
        collision,
        navigation,
        mesh_chunks,
        exterior_shell,
        cutaway_groups,
        massing,
    })
}

fn inaccessible_rooms(graph: &RoomGraph) -> Vec<RoomId> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    for room in &graph.exterior_entries {
        visited.insert(*room);
        queue.push_back(*room);
    }
    while let Some(room) = queue.pop_front() {
        if let Some(neighbors) = graph.adjacency.get(&room) {
            for neighbor in neighbors {
                if visited.insert(*neighbor) {
                    queue.push_back(*neighbor);
                }
            }
        }
    }
    graph
        .adjacency
        .keys()
        .filter(|room| !visited.contains(room))
        .copied()
        .collect()
}

fn compile_collision(
    blueprint: &BuildingBlueprint,
    index: &crate::validate::BlueprintIndex<'_>,
) -> CollisionProxy {
    let mut spans = Vec::new();
    let mut walls = blueprint.walls.iter().collect::<Vec<_>>();
    walls.sort_by_key(|wall| wall.id);

    for wall in walls {
        let level = index.levels[&wall.level_id];
        let mut gaps = blueprint
            .openings
            .iter()
            .filter(|opening| opening.wall_id == wall.id && opening.is_traversable())
            .map(|opening| (opening.offset_m, opening.end_offset_m()))
            .collect::<Vec<_>>();
        gaps.sort_by(|left, right| left.0.total_cmp(&right.0));

        let mut cursor = 0.0;
        for (start, end) in gaps {
            if start - cursor > BUILDING_EPSILON {
                spans.push(wall_span(wall, level.elevation_m, cursor, start));
            }
            cursor = cursor.max(end);
        }
        if wall.length_m() - cursor > BUILDING_EPSILON {
            spans.push(wall_span(wall, level.elevation_m, cursor, wall.length_m()));
        }
    }

    let mut floor_bounds = Vec::new();
    for level in &blueprint.levels {
        let mut rooms = level.rooms.iter().collect::<Vec<_>>();
        rooms.sort_by_key(|room| room.id);
        for room in rooms {
            floor_bounds.push(Aabb3::from_rect(
                room.footprint,
                level.elevation_m - 0.2,
                level.elevation_m,
            ));
        }
    }

    CollisionProxy {
        wall_spans: spans,
        floor_bounds,
    }
}

fn wall_span(wall: &WallRun, bottom_m: f64, start_m: f64, end_m: f64) -> CollisionWallSpan {
    CollisionWallSpan {
        wall_id: wall.id,
        level_id: wall.level_id,
        start: wall.point_at(start_m),
        end: wall.point_at(end_m),
        bottom_m,
        height_m: wall.height_m,
        thickness_m: wall.thickness_m,
    }
}

fn compile_navigation(
    blueprint: &BuildingBlueprint,
    index: &crate::validate::BlueprintIndex<'_>,
) -> Vec<NavigationPatch> {
    let mut patches = Vec::new();
    for level in &blueprint.levels {
        let mut rooms = level.rooms.iter().collect::<Vec<_>>();
        rooms.sort_by_key(|room| room.id);
        for room in rooms {
            let mut portals = Vec::new();
            for opening in blueprint.openings.iter().filter(|opening| {
                opening.is_traversable()
                    && (opening.from == SpaceRef::Room(room.id)
                        || opening.to == SpaceRef::Room(room.id))
            }) {
                let wall = index.walls[&opening.wall_id];
                let point = wall.point_at(opening.offset_m + opening.width_m * 0.5);
                portals.push(NavigationPortal {
                    opening_id: opening.id,
                    position: DVec3::new(point.x, level.elevation_m, point.y),
                    width_m: opening.width_m,
                });
            }
            portals.sort_by_key(|portal| portal.opening_id);
            patches.push(NavigationPatch {
                room_id: room.id,
                level_id: level.id,
                walkable: room.footprint,
                portals,
            });
        }
    }
    patches
}

fn compile_massing(
    blueprint: &BuildingBlueprint,
    index: &crate::validate::BlueprintIndex<'_>,
) -> Result<MassingProxy, CompileError> {
    let mut level_bounds = BTreeMap::new();
    let mut total: Option<Aabb3> = None;

    for level in &blueprint.levels {
        let mut current: Option<Aabb3> = None;
        for room in &level.rooms {
            let bounds = Aabb3::from_rect(
                room.footprint,
                level.elevation_m,
                level.elevation_m + level.height_m,
            );
            current = Some(current.map_or(bounds, |existing| existing.union(bounds)));
        }
        for wall in blueprint
            .walls
            .iter()
            .filter(|wall| wall.level_id == level.id)
        {
            let bounds = wall_bounds(wall, level.elevation_m);
            current = Some(current.map_or(bounds, |existing| existing.union(bounds)));
        }
        if let Some(bounds) = current {
            level_bounds.insert(level.id, bounds);
            total = Some(total.map_or(bounds, |existing| existing.union(bounds)));
        }
    }

    for roof in &blueprint.roof_regions {
        let roof_bounds = Aabb3::from_rect(
            roof.footprint,
            roof.base_elevation_m,
            roof.base_elevation_m + roof.height_m,
        );
        level_bounds
            .entry(roof.level_id)
            .and_modify(|bounds| *bounds = bounds.union(roof_bounds))
            .or_insert(roof_bounds);
        total = Some(total.map_or(roof_bounds, |existing| existing.union(roof_bounds)));
    }

    let _ = index;
    let bounds = total.ok_or(CompileError::EmptyBuilding)?;
    Ok(MassingProxy {
        bounds,
        level_bounds,
    })
}

fn compile_exterior_shell(blueprint: &BuildingBlueprint) -> ExteriorShell {
    let mut elements = BTreeSet::new();
    for wall in blueprint.walls.iter().filter(|wall| wall.is_exterior()) {
        elements.insert(ElementRef::Wall(wall.id));
        for opening in blueprint
            .openings
            .iter()
            .filter(|opening| opening.wall_id == wall.id)
        {
            elements.insert(ElementRef::Opening(opening.id));
        }
    }
    for roof in &blueprint.roof_regions {
        elements.insert(ElementRef::Roof(roof.id));
    }
    ExteriorShell { elements }
}

fn compile_cutaway_groups(blueprint: &BuildingBlueprint) -> Vec<CutawayGroup> {
    let mut groups: BTreeMap<CutawayDirection, BTreeSet<ElementRef>> = BTreeMap::new();
    for wall in blueprint.walls.iter().filter(|wall| wall.is_exterior()) {
        let direction = exterior_direction(wall);
        groups
            .entry(direction)
            .or_default()
            .insert(ElementRef::Wall(wall.id));
        for opening in blueprint
            .openings
            .iter()
            .filter(|opening| opening.wall_id == wall.id)
        {
            groups
                .entry(direction)
                .or_default()
                .insert(ElementRef::Opening(opening.id));
        }
    }
    for roof in &blueprint.roof_regions {
        groups
            .entry(CutawayDirection::Roof)
            .or_default()
            .insert(ElementRef::Roof(roof.id));
    }
    groups
        .into_iter()
        .map(|(direction, elements)| CutawayGroup {
            direction,
            elements,
        })
        .collect()
}

fn exterior_direction(wall: &WallRun) -> CutawayDirection {
    let tangent = wall.direction();
    let left_normal = DVec2::new(-tangent.y, tangent.x);
    let outward = if wall.left_room.is_none() {
        left_normal
    } else {
        -left_normal
    };
    if outward.x.abs() >= outward.y.abs() {
        if outward.x >= 0.0 {
            CutawayDirection::East
        } else {
            CutawayDirection::West
        }
    } else if outward.y >= 0.0 {
        CutawayDirection::North
    } else {
        CutawayDirection::South
    }
}

fn compile_mesh_chunks(
    blueprint: &BuildingBlueprint,
    index: &crate::validate::BlueprintIndex<'_>,
    shell: &ExteriorShell,
    building_bounds: Aabb3,
) -> Vec<MeshChunkPlan> {
    let mut chunks: BTreeMap<ChunkKey, (Aabb3, BTreeSet<ElementRef>)> = BTreeMap::new();

    for level in &blueprint.levels {
        for room in &level.rooms {
            let bounds =
                Aabb3::from_rect(room.footprint, level.elevation_m - 0.2, level.elevation_m);
            insert_chunk(
                &mut chunks,
                chunk_key(level.index, bounds.center(), ChunkLayer::Floor),
                bounds,
                ElementRef::Room(room.id),
            );
        }
    }

    for wall in &blueprint.walls {
        let level = index.levels[&wall.level_id];
        let bounds = wall_bounds(wall, level.elevation_m);
        insert_chunk(
            &mut chunks,
            chunk_key(level.index, bounds.center(), ChunkLayer::Structure),
            bounds,
            ElementRef::Wall(wall.id),
        );
        if shell.elements.contains(&ElementRef::Wall(wall.id)) {
            insert_chunk(
                &mut chunks,
                chunk_key(level.index, bounds.center(), ChunkLayer::ExteriorShell),
                bounds,
                ElementRef::Wall(wall.id),
            );
        }
    }

    for opening in &blueprint.openings {
        let wall = index.walls[&opening.wall_id];
        let level = index.levels[&wall.level_id];
        let point = wall.point_at(opening.offset_m + opening.width_m * 0.5);
        let center = DVec3::new(
            point.x,
            level.elevation_m + opening.sill_m + opening.height_m * 0.5,
            point.y,
        );
        let bounds = Aabb3::new(
            center
                - DVec3::new(
                    opening.width_m * 0.5,
                    opening.height_m * 0.5,
                    wall.thickness_m,
                ),
            center
                + DVec3::new(
                    opening.width_m * 0.5,
                    opening.height_m * 0.5,
                    wall.thickness_m,
                ),
        );
        insert_chunk(
            &mut chunks,
            chunk_key(level.index, center, ChunkLayer::Structure),
            bounds,
            ElementRef::Opening(opening.id),
        );
        if shell.elements.contains(&ElementRef::Opening(opening.id)) {
            insert_chunk(
                &mut chunks,
                chunk_key(level.index, center, ChunkLayer::ExteriorShell),
                bounds,
                ElementRef::Opening(opening.id),
            );
        }
    }

    for stair in &blueprint.stairs {
        let level = index.levels[&stair.from_level];
        let to_level = index.levels[&stair.to_level];
        let bottom = level.elevation_m.min(to_level.elevation_m);
        let top =
            (level.elevation_m + level.height_m).max(to_level.elevation_m + to_level.height_m);
        let bounds = Aabb3::from_rect(stair.footprint, bottom, top);
        insert_chunk(
            &mut chunks,
            chunk_key(level.index, bounds.center(), ChunkLayer::Structure),
            bounds,
            ElementRef::Stair(stair.id),
        );
    }

    for roof in &blueprint.roof_regions {
        let level = index.levels[&roof.level_id];
        let bounds = Aabb3::from_rect(
            roof.footprint,
            roof.base_elevation_m,
            roof.base_elevation_m + roof.height_m,
        );
        insert_chunk(
            &mut chunks,
            chunk_key(level.index, bounds.center(), ChunkLayer::Roof),
            bounds,
            ElementRef::Roof(roof.id),
        );
        insert_chunk(
            &mut chunks,
            chunk_key(level.index, bounds.center(), ChunkLayer::ExteriorShell),
            bounds,
            ElementRef::Roof(roof.id),
        );
    }

    if chunks.is_empty() {
        let key = ChunkKey {
            level_index: 0,
            x: 0,
            z: 0,
            layer: ChunkLayer::Structure,
        };
        chunks.insert(key, (building_bounds, BTreeSet::new()));
    }

    chunks
        .into_iter()
        .map(|(key, (bounds, elements))| MeshChunkPlan {
            key,
            bounds,
            elements,
        })
        .collect()
}

fn insert_chunk(
    chunks: &mut BTreeMap<ChunkKey, (Aabb3, BTreeSet<ElementRef>)>,
    key: ChunkKey,
    bounds: Aabb3,
    element: ElementRef,
) {
    chunks
        .entry(key)
        .and_modify(|(existing_bounds, elements)| {
            *existing_bounds = existing_bounds.union(bounds);
            elements.insert(element);
        })
        .or_insert_with(|| (bounds, BTreeSet::from([element])));
}

fn chunk_key(level_index: i16, center: DVec3, layer: ChunkLayer) -> ChunkKey {
    ChunkKey {
        level_index,
        x: (center.x / BUILDING_CHUNK_SIZE_METERS).floor() as i32,
        z: (center.z / BUILDING_CHUNK_SIZE_METERS).floor() as i32,
        layer,
    }
}

fn wall_bounds(wall: &WallRun, bottom_m: f64) -> Aabb3 {
    let half = wall.thickness_m * 0.5;
    Aabb3::new(
        DVec3::new(
            wall.start.x.min(wall.end.x) - half,
            bottom_m,
            wall.start.y.min(wall.end.y) - half,
        ),
        DVec3::new(
            wall.start.x.max(wall.end.x) + half,
            bottom_m + wall.height_m,
            wall.start.y.max(wall.end.y) + half,
        ),
    )
}

pub fn semantic_fingerprint(blueprint: &BuildingBlueprint) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    feed_u128(&mut hash, blueprint.id.as_u128());
    feed_u128(&mut hash, blueprint.style_id.as_u128());
    feed_u64(&mut hash, blueprint.revision);
    feed_u128(&mut hash, blueprint.primary_entry.as_u128());

    let mut levels = blueprint.levels.iter().collect::<Vec<_>>();
    levels.sort_by_key(|level| level.id);
    for level in levels {
        feed_u128(&mut hash, level.id.as_u128());
        feed_u64(&mut hash, level.index as i64 as u64);
        feed_f64(&mut hash, level.elevation_m);
        feed_f64(&mut hash, level.height_m);
        let mut rooms = level.rooms.iter().collect::<Vec<_>>();
        rooms.sort_by_key(|room| room.id);
        for room in rooms {
            feed_u128(&mut hash, room.id.as_u128());
            feed_u64(&mut hash, room.purpose as u64);
            feed_rect(&mut hash, room.footprint);
        }
    }

    let mut walls = blueprint.walls.iter().collect::<Vec<_>>();
    walls.sort_by_key(|wall| wall.id);
    for wall in walls {
        feed_u128(&mut hash, wall.id.as_u128());
        feed_u128(&mut hash, wall.level_id.as_u128());
        feed_vec2(&mut hash, wall.start);
        feed_vec2(&mut hash, wall.end);
        feed_f64(&mut hash, wall.thickness_m);
        feed_f64(&mut hash, wall.height_m);
        feed_optional_room(&mut hash, wall.left_room);
        feed_optional_room(&mut hash, wall.right_room);
    }

    let mut openings = blueprint.openings.iter().collect::<Vec<_>>();
    openings.sort_by_key(|opening| opening.id);
    for opening in openings {
        feed_u128(&mut hash, opening.id.as_u128());
        feed_u128(&mut hash, opening.wall_id.as_u128());
        feed_u64(&mut hash, opening.kind as u64);
        feed_f64(&mut hash, opening.offset_m);
        feed_f64(&mut hash, opening.width_m);
        feed_f64(&mut hash, opening.sill_m);
        feed_f64(&mut hash, opening.height_m);
        feed_space(&mut hash, opening.from);
        feed_space(&mut hash, opening.to);
    }

    let mut stairs = blueprint.stairs.iter().collect::<Vec<_>>();
    stairs.sort_by_key(|stair| stair.id);
    for stair in stairs {
        feed_u128(&mut hash, stair.id.as_u128());
        feed_u128(&mut hash, stair.from_level.as_u128());
        feed_u128(&mut hash, stair.to_level.as_u128());
        feed_u128(&mut hash, stair.from_room.as_u128());
        feed_u128(&mut hash, stair.to_room.as_u128());
        feed_rect(&mut hash, stair.footprint);
        feed_f64(&mut hash, stair.width_m);
    }

    let mut roofs = blueprint.roof_regions.iter().collect::<Vec<_>>();
    roofs.sort_by_key(|roof| roof.id);
    for roof in roofs {
        feed_u128(&mut hash, roof.id.as_u128());
        feed_u128(&mut hash, roof.level_id.as_u128());
        feed_rect(&mut hash, roof.footprint);
        feed_u64(&mut hash, roof.kind as u64);
        feed_f64(&mut hash, roof.base_elevation_m);
        feed_f64(&mut hash, roof.height_m);
    }

    hash
}

fn feed_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn feed_u128(hash: &mut u64, value: u128) {
    feed_u64(hash, value as u64);
    feed_u64(hash, (value >> 64) as u64);
}

fn feed_f64(hash: &mut u64, value: f64) {
    feed_u64(hash, value.to_bits());
}

fn feed_vec2(hash: &mut u64, value: DVec2) {
    feed_f64(hash, value.x);
    feed_f64(hash, value.y);
}

fn feed_rect(hash: &mut u64, rect: Rect2) {
    feed_vec2(hash, rect.min);
    feed_vec2(hash, rect.max);
}

fn feed_optional_room(hash: &mut u64, room: Option<RoomId>) {
    match room {
        Some(room) => {
            feed_u64(hash, 1);
            feed_u128(hash, room.as_u128());
        }
        None => feed_u64(hash, 0),
    }
}

fn feed_space(hash: &mut u64, space: SpaceRef) {
    match space {
        SpaceRef::Exterior => feed_u64(hash, 0),
        SpaceRef::Room(room) => {
            feed_u64(hash, 1);
            feed_u128(hash, room.as_u128());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::BuildingError;
    use glam::DVec2;
    use world_ids::{
        ArchitectureStyleId, BuildingId, BuildingLevelId, OpeningId, RoofRegionId, RoomId, WallId,
    };

    fn blueprint() -> BuildingBlueprint {
        let level = BuildingLevelId::from_u128(10);
        let room = RoomId::from_u128(20);
        let wall = WallId::from_u128(30);
        let entry = OpeningId::from_u128(40);
        BuildingBlueprint {
            id: BuildingId::from_u128(1),
            style_id: ArchitectureStyleId::from_u128(2),
            revision: 1,
            levels: vec![BuildingLevel {
                id: level,
                index: 0,
                elevation_m: 0.0,
                height_m: 3.0,
                rooms: vec![Room {
                    id: room,
                    footprint: Rect2::new(DVec2::new(0.0, 0.0), DVec2::new(4.0, 4.0)),
                    purpose: RoomPurpose::Entry,
                }],
            }],
            walls: vec![WallRun {
                id: wall,
                level_id: level,
                start: DVec2::new(0.0, 0.0),
                end: DVec2::new(4.0, 0.0),
                thickness_m: 0.2,
                height_m: 3.0,
                left_room: Some(room),
                right_room: None,
            }],
            openings: vec![Opening {
                id: entry,
                wall_id: wall,
                kind: OpeningKind::Door,
                offset_m: 1.0,
                width_m: 1.0,
                sill_m: 0.0,
                height_m: 2.1,
                from: SpaceRef::Room(room),
                to: SpaceRef::Exterior,
            }],
            stairs: vec![],
            roof_regions: vec![RoofRegion {
                id: RoofRegionId::from_u128(50),
                level_id: level,
                footprint: Rect2::new(DVec2::new(0.0, 0.0), DVec2::new(4.0, 4.0)),
                kind: RoofKind::Flat,
                base_elevation_m: 3.0,
                height_m: 0.3,
            }],
            primary_entry: entry,
        }
    }

    #[test]
    fn compilation_is_order_independent() {
        let original = blueprint();
        let mut reordered = original.clone();
        reordered.levels.reverse();
        reordered.walls.reverse();
        reordered.openings.reverse();
        reordered.roof_regions.reverse();
        assert_eq!(
            compile_blueprint(&original)
                .expect("original")
                .semantic_fingerprint,
            compile_blueprint(&reordered)
                .expect("reordered")
                .semantic_fingerprint
        );
    }

    #[test]
    fn door_splits_wall_collision_span() {
        let compilation = compile_blueprint(&blueprint()).expect("compile");
        assert_eq!(compilation.collision.wall_spans.len(), 2);
    }

    #[test]
    fn inaccessible_room_is_rejected() {
        let mut invalid = blueprint();
        invalid.levels[0].rooms.push(Room {
            id: RoomId::from_u128(21),
            footprint: Rect2::new(DVec2::new(5.0, 0.0), DVec2::new(8.0, 3.0)),
            purpose: RoomPurpose::Storage,
        });
        let error = compile_blueprint(&invalid).expect_err("inaccessible room");
        match error {
            CompileError::Validation(report) => assert!(
                report
                    .errors
                    .contains(&BuildingError::InaccessibleRoom(RoomId::from_u128(21)))
            ),
            CompileError::EmptyBuilding => panic!("unexpected empty building"),
        }
    }
}
