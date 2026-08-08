#![forbid(unsafe_code)]

use building_core::*;
use glam::{DVec2, DVec3};
use world_ids::{
    ArchitectureStyleId, BuildingId, BuildingLevelId, FrameId, OpeningId, RoofRegionId, RoomId,
    StairId, WallId,
};

pub fn two_storey_shop() -> BuildingBlueprint {
    let ground = BuildingLevelId::from_u128(100);
    let upper = BuildingLevelId::from_u128(101);
    let entry_room = RoomId::from_u128(200);
    let shop_room = RoomId::from_u128(201);
    let upper_landing = RoomId::from_u128(202);
    let upper_bedroom = RoomId::from_u128(203);

    let ground_walls = level_walls(1_000, ground, entry_room, shop_room);
    let upper_walls = level_walls(2_000, upper, upper_landing, upper_bedroom);
    let mut walls = ground_walls.clone();
    walls.extend(upper_walls.clone());

    let primary_entry = OpeningId::from_u128(3_000);
    let openings = vec![
        Opening {
            id: primary_entry,
            wall_id: ground_walls[0].id,
            kind: OpeningKind::Door,
            offset_m: 2.4,
            width_m: 1.2,
            sill_m: 0.0,
            height_m: 2.2,
            from: SpaceRef::Room(entry_room),
            to: SpaceRef::Exterior,
        },
        Opening {
            id: OpeningId::from_u128(3_001),
            wall_id: ground_walls[1].id,
            kind: OpeningKind::Window,
            offset_m: 1.0,
            width_m: 2.4,
            sill_m: 0.9,
            height_m: 1.5,
            from: SpaceRef::Room(shop_room),
            to: SpaceRef::Exterior,
        },
        Opening {
            id: OpeningId::from_u128(3_002),
            wall_id: ground_walls[6].id,
            kind: OpeningKind::Passage,
            offset_m: 3.3,
            width_m: 1.4,
            sill_m: 0.0,
            height_m: 2.4,
            from: SpaceRef::Room(entry_room),
            to: SpaceRef::Room(shop_room),
        },
        Opening {
            id: OpeningId::from_u128(3_003),
            wall_id: ground_walls[4].id,
            kind: OpeningKind::Window,
            offset_m: 2.0,
            width_m: 1.5,
            sill_m: 1.0,
            height_m: 1.3,
            from: SpaceRef::Room(entry_room),
            to: SpaceRef::Exterior,
        },
        Opening {
            id: OpeningId::from_u128(3_100),
            wall_id: upper_walls[0].id,
            kind: OpeningKind::Window,
            offset_m: 2.2,
            width_m: 1.6,
            sill_m: 0.9,
            height_m: 1.4,
            from: SpaceRef::Room(upper_landing),
            to: SpaceRef::Exterior,
        },
        Opening {
            id: OpeningId::from_u128(3_101),
            wall_id: upper_walls[1].id,
            kind: OpeningKind::Window,
            offset_m: 2.0,
            width_m: 1.8,
            sill_m: 0.9,
            height_m: 1.4,
            from: SpaceRef::Room(upper_bedroom),
            to: SpaceRef::Exterior,
        },
        Opening {
            id: OpeningId::from_u128(3_102),
            wall_id: upper_walls[6].id,
            kind: OpeningKind::Door,
            offset_m: 3.3,
            width_m: 1.1,
            sill_m: 0.0,
            height_m: 2.2,
            from: SpaceRef::Room(upper_landing),
            to: SpaceRef::Room(upper_bedroom),
        },
    ];

    BuildingBlueprint {
        id: BuildingId::from_u128(1),
        style_id: ArchitectureStyleId::from_u128(10),
        revision: 1,
        levels: vec![
            BuildingLevel {
                id: ground,
                index: 0,
                elevation_m: 0.0,
                height_m: 3.0,
                rooms: vec![
                    Room {
                        id: entry_room,
                        footprint: Rect2::new(DVec2::new(-6.0, -4.0), DVec2::new(0.0, 4.0)),
                        purpose: RoomPurpose::Entry,
                    },
                    Room {
                        id: shop_room,
                        footprint: Rect2::new(DVec2::new(0.0, -4.0), DVec2::new(6.0, 4.0)),
                        purpose: RoomPurpose::Retail,
                    },
                ],
            },
            BuildingLevel {
                id: upper,
                index: 1,
                elevation_m: 3.0,
                height_m: 3.0,
                rooms: vec![
                    Room {
                        id: upper_landing,
                        footprint: Rect2::new(DVec2::new(-6.0, -4.0), DVec2::new(0.0, 4.0)),
                        purpose: RoomPurpose::Circulation,
                    },
                    Room {
                        id: upper_bedroom,
                        footprint: Rect2::new(DVec2::new(0.0, -4.0), DVec2::new(6.0, 4.0)),
                        purpose: RoomPurpose::Sleeping,
                    },
                ],
            },
        ],
        walls,
        openings,
        stairs: vec![Stair {
            id: StairId::from_u128(4_000),
            from_level: ground,
            to_level: upper,
            from_room: entry_room,
            to_room: upper_landing,
            footprint: Rect2::new(DVec2::new(-3.5, -1.5), DVec2::new(-1.2, 2.2)),
            width_m: 1.2,
        }],
        roof_regions: vec![RoofRegion {
            id: RoofRegionId::from_u128(5_000),
            level_id: upper,
            footprint: Rect2::new(DVec2::new(-6.3, -4.3), DVec2::new(6.3, 4.3)),
            kind: RoofKind::GableX,
            base_elevation_m: 6.0,
            height_m: 1.8,
        }],
        primary_entry,
    }
}

pub fn add_north_display_window_delta() -> BlueprintDelta {
    BlueprintDelta::AddOpening(Opening {
        id: OpeningId::from_u128(3_010),
        wall_id: WallId::from_u128(1_004),
        kind: OpeningKind::Window,
        offset_m: 4.0,
        width_m: 2.0,
        sill_m: 0.9,
        height_m: 1.4,
        from: SpaceRef::Room(RoomId::from_u128(200)),
        to: SpaceRef::Exterior,
    })
}

pub fn inaccessible_upper_floor() -> BuildingBlueprint {
    let mut blueprint = two_storey_shop();
    blueprint.stairs.clear();
    blueprint
}

pub fn static_world_binding() -> BuildingInstanceBinding {
    BuildingInstanceBinding {
        building_id: BuildingId::from_u128(1),
        frame_id: FrameId::from_u128(1),
        local_translation: DVec3::new(-16.0, 0.0, 0.0),
        local_yaw_radians: 0.0,
    }
}

pub fn moving_platform_binding() -> BuildingInstanceBinding {
    BuildingInstanceBinding {
        building_id: BuildingId::from_u128(1),
        frame_id: FrameId::from_u128(2),
        local_translation: DVec3::new(16.0, 0.0, 0.0),
        local_yaw_radians: 0.35,
    }
}

fn level_walls(
    id_base: u128,
    level_id: BuildingLevelId,
    left_room: RoomId,
    right_room: RoomId,
) -> Vec<WallRun> {
    vec![
        wall(
            id_base,
            level_id,
            DVec2::new(-6.0, -4.0),
            DVec2::new(0.0, -4.0),
            Some(left_room),
            None,
        ),
        wall(
            id_base + 1,
            level_id,
            DVec2::new(0.0, -4.0),
            DVec2::new(6.0, -4.0),
            Some(right_room),
            None,
        ),
        wall(
            id_base + 2,
            level_id,
            DVec2::new(6.0, -4.0),
            DVec2::new(6.0, 4.0),
            Some(right_room),
            None,
        ),
        wall(
            id_base + 3,
            level_id,
            DVec2::new(6.0, 4.0),
            DVec2::new(0.0, 4.0),
            Some(right_room),
            None,
        ),
        wall(
            id_base + 4,
            level_id,
            DVec2::new(0.0, 4.0),
            DVec2::new(-6.0, 4.0),
            Some(left_room),
            None,
        ),
        wall(
            id_base + 5,
            level_id,
            DVec2::new(-6.0, 4.0),
            DVec2::new(-6.0, -4.0),
            Some(left_room),
            None,
        ),
        wall(
            id_base + 6,
            level_id,
            DVec2::new(0.0, -4.0),
            DVec2::new(0.0, 4.0),
            Some(left_room),
            Some(right_room),
        ),
    ]
}

fn wall(
    id: u128,
    level_id: BuildingLevelId,
    start: DVec2,
    end: DVec2,
    left_room: Option<RoomId>,
    right_room: Option<RoomId>,
) -> WallRun {
    WallRun {
        id: WallId::from_u128(id),
        level_id,
        start,
        end,
        thickness_m: 0.24,
        height_m: 3.0,
        left_room,
        right_room,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_compiles_and_is_reachable() {
        let compilation = compile_blueprint(&two_storey_shop()).expect("valid P3 scenario");
        assert_eq!(compilation.room_graph.adjacency.len(), 4);
        assert_eq!(compilation.room_graph.exterior_entries.len(), 1);
        assert!(compilation.bounds.size().y >= 7.8);
    }

    #[test]
    fn invalid_variant_is_rejected_by_reachability() {
        assert!(compile_blueprint(&inaccessible_upper_floor()).is_err());
    }

    #[test]
    fn display_window_has_local_incremental_impact() {
        let blueprint = two_storey_shop();
        let dirty =
            impact_for_delta(&blueprint, &add_north_display_window_delta()).expect("impact plan");
        assert_eq!(dirty.walls.len(), 1);
        assert_eq!(dirty.levels.len(), 1);
        assert!(dirty.rebuild_exterior_shell);
        assert!(!dirty.rebuild_massing);
    }
}
