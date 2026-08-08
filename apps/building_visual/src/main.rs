#![forbid(unsafe_code)]

use bevy::prelude::*;
use building_core::{
    Aabb3, BlueprintDelta, BuildingBlueprint, BuildingCompilation, BuildingInstanceBinding,
    CutawayDirection, ElementRef, Opening, OpeningKind, RoofKind, WallRun, apply_delta,
    compile_blueprint,
};
use p3_building_scenario::{
    add_north_display_window_delta, moving_platform_binding, static_world_binding,
    two_storey_shop,
};
use world_ids::WallId;

const TARGET_WALL: WallId = WallId::from_u128(1_004);
const WALL_EPSILON: f64 = 1.0e-6;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "P3 · Semantic Building Compiler · visual review required".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.63, 0.77, 0.89)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 900.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                control_prototype,
                update_motion,
                update_instance_transforms,
                update_part_visibility,
                update_camera,
            )
                .chain(),
        )
        .run();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstanceKind {
    Static,
    Moving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisplayMode {
    Full,
    ExteriorShell,
    Massing,
}

impl DisplayMode {
    const fn next(self) -> Self {
        match self {
            Self::Full => Self::ExteriorShell,
            Self::ExteriorShell => Self::Massing,
            Self::Massing => Self::Full,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Representation {
    Full,
    Massing,
    Always,
}

#[derive(Component, Clone, Copy, Debug)]
struct BuildingVisualPart {
    instance: InstanceKind,
    local_translation: Vec3,
    local_rotation: Quat,
    local_scale: Vec3,
    representation: Representation,
    exterior: bool,
    cutaway: Option<CutawayDirection>,
    source_wall: Option<WallId>,
}

#[derive(Component)]
struct PrototypeCamera;

#[derive(Resource)]
struct PrototypeState {
    baseline: BuildingBlueprint,
    current: BuildingBlueprint,
    compilation: BuildingCompilation,
    display_mode: DisplayMode,
    cutaway: Option<CutawayDirection>,
    delta_applied: bool,
    moving_enabled: bool,
    elapsed_seconds: f32,
}

#[derive(Resource)]
struct CameraOrbit {
    angle_radians: f32,
    radius_m: f32,
    height_m: f32,
}

#[derive(Resource, Clone)]
struct VisualAssets {
    cube: Handle<Mesh>,
    exterior_wall: Handle<StandardMaterial>,
    interior_wall: Handle<StandardMaterial>,
    floor: Handle<StandardMaterial>,
    roof: Handle<StandardMaterial>,
    opening_frame: Handle<StandardMaterial>,
    stair: Handle<StandardMaterial>,
    massing: Handle<StandardMaterial>,
    platform: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let baseline = two_storey_shop();
    let compilation = compile_blueprint(&baseline).expect("P3 scenario must compile");
    let assets = VisualAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        exterior_wall: materials.add(StandardMaterial {
            base_color: Color::srgb(0.69, 0.48, 0.27),
            perceptual_roughness: 0.9,
            ..default()
        }),
        interior_wall: materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.72, 0.56),
            perceptual_roughness: 0.95,
            ..default()
        }),
        floor: materials.add(StandardMaterial {
            base_color: Color::srgb(0.39, 0.24, 0.13),
            perceptual_roughness: 0.92,
            ..default()
        }),
        roof: materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.10, 0.08),
            perceptual_roughness: 0.88,
            ..default()
        }),
        opening_frame: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.12, 0.08),
            perceptual_roughness: 0.8,
            ..default()
        }),
        stair: materials.add(StandardMaterial {
            base_color: Color::srgb(0.47, 0.31, 0.17),
            perceptual_roughness: 0.9,
            ..default()
        }),
        massing: materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.42, 0.67),
            perceptual_roughness: 0.75,
            ..default()
        }),
        platform: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.25, 0.28),
            perceptual_roughness: 0.9,
            ..default()
        }),
    };

    commands.insert_resource(PrototypeState {
        baseline: baseline.clone(),
        current: baseline.clone(),
        compilation: compilation.clone(),
        display_mode: DisplayMode::Full,
        cutaway: Some(CutawayDirection::North),
        delta_applied: false,
        moving_enabled: true,
        elapsed_seconds: 0.0,
    });
    commands.insert_resource(CameraOrbit {
        angle_radians: 0.0,
        radius_m: 48.0,
        height_m: 19.0,
    });
    commands.insert_resource(assets.clone());

    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.platform.clone()),
        Transform::from_xyz(0.0, -0.8, 0.0).with_scale(Vec3::new(80.0, 1.0, 45.0)),
    ));

    spawn_platform(&mut commands, &assets, InstanceKind::Moving);
    spawn_building_instance(
        &mut commands,
        &assets,
        &baseline,
        &compilation,
        InstanceKind::Static,
    );
    spawn_building_instance(
        &mut commands,
        &assets,
        &baseline,
        &compilation,
        InstanceKind::Moving,
    );

    commands.spawn((Camera3d::default(), Transform::IDENTITY, PrototypeCamera));
    commands.spawn((
        DirectionalLight {
            illuminance: 22_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.55, 0.0)),
    ));

    info!(
        "Controls: M full/shell/massing; X cutaway direction; W add/remove one window; O moving platform; A/D orbit; Up/Down camera height; R reset"
    );
}

fn control_prototype(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    assets: Res<VisualAssets>,
    mut state: ResMut<PrototypeState>,
    parts: Query<(Entity, &BuildingVisualPart)>,
) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        state.display_mode = state.display_mode.next();
        info!(mode = ?state.display_mode, "building representation changed");
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        state.cutaway = next_cutaway(state.cutaway);
        info!(cutaway = ?state.cutaway, "cutaway direction changed");
    }
    if keyboard.just_pressed(KeyCode::KeyO) {
        state.moving_enabled = !state.moving_enabled;
    }
    if keyboard.just_pressed(KeyCode::KeyW) {
        let should_apply = !state.delta_applied;
        rebuild_target_wall(
            &mut commands,
            &assets,
            &mut state,
            &parts,
            should_apply,
        );
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        if state.delta_applied {
            rebuild_target_wall(&mut commands, &assets, &mut state, &parts, false);
        }
        state.display_mode = DisplayMode::Full;
        state.cutaway = Some(CutawayDirection::North);
        state.moving_enabled = true;
        state.elapsed_seconds = 0.0;
    }
}

fn rebuild_target_wall(
    commands: &mut Commands,
    assets: &VisualAssets,
    state: &mut PrototypeState,
    parts: &Query<(Entity, &BuildingVisualPart)>,
    apply_window: bool,
) {
    let next_blueprint = if apply_window {
        apply_delta(&state.baseline, &add_north_display_window_delta())
            .expect("display-window delta must remain valid")
    } else {
        state.baseline.clone()
    };
    let next_compilation = compile_blueprint(&next_blueprint).expect("updated building must compile");

    for (entity, part) in parts {
        if part.source_wall == Some(TARGET_WALL) {
            commands.entity(entity).despawn();
        }
    }
    for instance in [InstanceKind::Static, InstanceKind::Moving] {
        let wall = next_blueprint
            .walls
            .iter()
            .find(|wall| wall.id == TARGET_WALL)
            .expect("target wall");
        spawn_wall_geometry(
            commands,
            assets,
            &next_blueprint,
            &next_compilation,
            instance,
            wall,
        );
    }

    let delta = add_north_display_window_delta();
    let dirty = building_core::impact_for_delta(&state.baseline, &delta)
        .expect("display-window impact plan");
    info!(
        apply_window,
        dirty_levels = dirty.levels.len(),
        dirty_rooms = dirty.rooms.len(),
        dirty_walls = dirty.walls.len(),
        rebuild_massing = dirty.rebuild_massing,
        "localized building recompilation"
    );

    state.current = next_blueprint;
    state.compilation = next_compilation;
    state.delta_applied = apply_window;
}

fn update_motion(time: Res<Time>, mut state: ResMut<PrototypeState>) {
    if state.moving_enabled {
        state.elapsed_seconds += time.delta_secs();
    }
}

fn update_instance_transforms(
    state: Res<PrototypeState>,
    mut parts: Query<(&BuildingVisualPart, &mut Transform)>,
) {
    for (part, mut transform) in &mut parts {
        let (root_translation, root_rotation) = instance_transform(part.instance, &state);
        transform.translation = root_translation + root_rotation * part.local_translation;
        transform.rotation = root_rotation * part.local_rotation;
        transform.scale = part.local_scale;
    }
}

fn update_part_visibility(
    state: Res<PrototypeState>,
    mut parts: Query<(&BuildingVisualPart, &mut Visibility)>,
) {
    for (part, mut visibility) in &mut parts {
        let representation_visible = match part.representation {
            Representation::Always => true,
            Representation::Full => match state.display_mode {
                DisplayMode::Full => true,
                DisplayMode::ExteriorShell => part.exterior,
                DisplayMode::Massing => false,
            },
            Representation::Massing => state.display_mode == DisplayMode::Massing,
        };
        let hidden_by_cutaway = representation_visible
            && part.representation == Representation::Full
            && state.cutaway.is_some()
            && part.cutaway == state.cutaway;
        *visibility = if representation_visible && !hidden_by_cutaway {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_camera(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut orbit: ResMut<CameraOrbit>,
    mut camera: Query<&mut Transform, With<PrototypeCamera>>,
) {
    let angular_speed = 0.8;
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        orbit.angle_radians += angular_speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        orbit.angle_radians -= angular_speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        orbit.height_m = (orbit.height_m + 8.0 * time.delta_secs()).clamp(7.0, 32.0);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        orbit.height_m = (orbit.height_m - 8.0 * time.delta_secs()).clamp(7.0, 32.0);
    }

    let position = Vec3::new(
        orbit.angle_radians.sin() * orbit.radius_m,
        orbit.height_m,
        orbit.angle_radians.cos() * orbit.radius_m,
    );
    *camera.single_mut().expect("one prototype camera") =
        Transform::from_translation(position).looking_at(Vec3::new(0.0, 3.3, 0.0), Vec3::Y);
}

fn instance_transform(instance: InstanceKind, state: &PrototypeState) -> (Vec3, Quat) {
    let binding = binding_for(instance);
    let mut translation = binding.local_translation.as_vec3();
    let mut yaw = binding.local_yaw_radians as f32;
    if instance == InstanceKind::Moving {
        let time = state.elapsed_seconds;
        translation += Vec3::new(0.0, (time * 1.2).sin() * 0.35, (time * 0.35).sin() * 3.0);
        yaw += (time * 0.25).sin() * 0.28;
    }
    (translation, Quat::from_rotation_y(yaw))
}

fn binding_for(instance: InstanceKind) -> BuildingInstanceBinding {
    match instance {
        InstanceKind::Static => static_world_binding(),
        InstanceKind::Moving => moving_platform_binding(),
    }
}

fn next_cutaway(current: Option<CutawayDirection>) -> Option<CutawayDirection> {
    match current {
        None => Some(CutawayDirection::North),
        Some(CutawayDirection::North) => Some(CutawayDirection::East),
        Some(CutawayDirection::East) => Some(CutawayDirection::South),
        Some(CutawayDirection::South) => Some(CutawayDirection::West),
        Some(CutawayDirection::West) => Some(CutawayDirection::Roof),
        Some(CutawayDirection::Roof) => None,
    }
}

fn spawn_platform(commands: &mut Commands, assets: &VisualAssets, instance: InstanceKind) {
    spawn_part(
        commands,
        assets.cube.clone(),
        assets.platform.clone(),
        BuildingVisualPart {
            instance,
            local_translation: Vec3::new(0.0, -0.5, 0.0),
            local_rotation: Quat::IDENTITY,
            local_scale: Vec3::new(16.0, 0.7, 12.0),
            representation: Representation::Always,
            exterior: false,
            cutaway: None,
            source_wall: None,
        },
    );
}

fn spawn_building_instance(
    commands: &mut Commands,
    assets: &VisualAssets,
    blueprint: &BuildingBlueprint,
    compilation: &BuildingCompilation,
    instance: InstanceKind,
) {
    for level in &blueprint.levels {
        for room in &level.rooms {
            let center = room.footprint.center();
            spawn_part(
                commands,
                assets.cube.clone(),
                assets.floor.clone(),
                BuildingVisualPart {
                    instance,
                    local_translation: Vec3::new(
                        center.x as f32,
                        (level.elevation_m - 0.12) as f32,
                        center.y as f32,
                    ),
                    local_rotation: Quat::IDENTITY,
                    local_scale: Vec3::new(
                        room.footprint.width() as f32,
                        0.24,
                        room.footprint.depth() as f32,
                    ),
                    representation: Representation::Full,
                    exterior: false,
                    cutaway: None,
                    source_wall: None,
                },
            );
        }
    }

    for wall in &blueprint.walls {
        spawn_wall_geometry(commands, assets, blueprint, compilation, instance, wall);
    }
    for stair in &blueprint.stairs {
        spawn_stair_geometry(commands, assets, blueprint, instance, stair);
    }
    for roof in &blueprint.roof_regions {
        spawn_roof_geometry(commands, assets, compilation, instance, roof);
    }
    spawn_massing(commands, assets, instance, compilation.massing.bounds);
}

fn spawn_wall_geometry(
    commands: &mut Commands,
    assets: &VisualAssets,
    blueprint: &BuildingBlueprint,
    compilation: &BuildingCompilation,
    instance: InstanceKind,
    wall: &WallRun,
) {
    let level = blueprint
        .levels
        .iter()
        .find(|level| level.id == wall.level_id)
        .expect("wall level");
    let element = ElementRef::Wall(wall.id);
    let exterior = compilation.exterior_shell.elements.contains(&element);
    let cutaway = cutaway_for_element(compilation, element);
    let material = if exterior {
        assets.exterior_wall.clone()
    } else {
        assets.interior_wall.clone()
    };
    let mut openings = blueprint
        .openings
        .iter()
        .filter(|opening| opening.wall_id == wall.id)
        .collect::<Vec<_>>();
    openings.sort_by(|left, right| left.offset_m.total_cmp(&right.offset_m));

    let mut cursor = 0.0;
    for opening in &openings {
        spawn_wall_segment(
            commands,
            assets,
            material.clone(),
            instance,
            wall,
            level.elevation_m,
            cursor,
            opening.offset_m,
            0.0,
            wall.height_m,
            exterior,
            cutaway,
        );
        if opening.sill_m > WALL_EPSILON {
            spawn_wall_segment(
                commands,
                assets,
                material.clone(),
                instance,
                wall,
                level.elevation_m,
                opening.offset_m,
                opening.end_offset_m(),
                0.0,
                opening.sill_m,
                exterior,
                cutaway,
            );
        }
        let opening_top = opening.sill_m + opening.height_m;
        if wall.height_m - opening_top > WALL_EPSILON {
            spawn_wall_segment(
                commands,
                assets,
                material.clone(),
                instance,
                wall,
                level.elevation_m,
                opening.offset_m,
                opening.end_offset_m(),
                opening_top,
                wall.height_m,
                exterior,
                cutaway,
            );
        }
        spawn_opening_frame(
            commands,
            assets,
            instance,
            wall,
            level.elevation_m,
            opening,
            exterior,
            cutaway,
        );
        cursor = opening.end_offset_m();
    }
    spawn_wall_segment(
        commands,
        assets,
        material,
        instance,
        wall,
        level.elevation_m,
        cursor,
        wall.length_m(),
        0.0,
        wall.height_m,
        exterior,
        cutaway,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_wall_segment(
    commands: &mut Commands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    instance: InstanceKind,
    wall: &WallRun,
    level_elevation: f64,
    start_m: f64,
    end_m: f64,
    bottom_m: f64,
    top_m: f64,
    exterior: bool,
    cutaway: Option<CutawayDirection>,
) {
    let length = end_m - start_m;
    let height = top_m - bottom_m;
    if length <= WALL_EPSILON || height <= WALL_EPSILON {
        return;
    }
    let point = wall.point_at((start_m + end_m) * 0.5);
    let direction = wall.direction();
    let yaw = (-(direction.y as f32)).atan2(direction.x as f32);
    spawn_part(
        commands,
        assets.cube.clone(),
        material,
        BuildingVisualPart {
            instance,
            local_translation: Vec3::new(
                point.x as f32,
                (level_elevation + bottom_m + height * 0.5) as f32,
                point.y as f32,
            ),
            local_rotation: Quat::from_rotation_y(yaw),
            local_scale: Vec3::new(length as f32, height as f32, wall.thickness_m as f32),
            representation: Representation::Full,
            exterior,
            cutaway,
            source_wall: Some(wall.id),
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_opening_frame(
    commands: &mut Commands,
    assets: &VisualAssets,
    instance: InstanceKind,
    wall: &WallRun,
    level_elevation: f64,
    opening: &Opening,
    exterior: bool,
    cutaway: Option<CutawayDirection>,
) {
    let direction = wall.direction();
    let yaw = (-(direction.y as f32)).atan2(direction.x as f32);
    let bar = 0.12_f64;
    let thickness = wall.thickness_m * 1.35;
    let center_offset = opening.offset_m + opening.width_m * 0.5;

    for offset in [opening.offset_m, opening.end_offset_m()] {
        let point = wall.point_at(offset);
        spawn_part(
            commands,
            assets.cube.clone(),
            assets.opening_frame.clone(),
            BuildingVisualPart {
                instance,
                local_translation: Vec3::new(
                    point.x as f32,
                    (level_elevation + opening.sill_m + opening.height_m * 0.5) as f32,
                    point.y as f32,
                ),
                local_rotation: Quat::from_rotation_y(yaw),
                local_scale: Vec3::new(bar as f32, opening.height_m as f32, thickness as f32),
                representation: Representation::Full,
                exterior,
                cutaway,
                source_wall: Some(wall.id),
            },
        );
    }

    let center = wall.point_at(center_offset);
    for height in [opening.sill_m, opening.sill_m + opening.height_m] {
        if height <= WALL_EPSILON && opening.kind != OpeningKind::Window {
            continue;
        }
        spawn_part(
            commands,
            assets.cube.clone(),
            assets.opening_frame.clone(),
            BuildingVisualPart {
                instance,
                local_translation: Vec3::new(
                    center.x as f32,
                    (level_elevation + height) as f32,
                    center.y as f32,
                ),
                local_rotation: Quat::from_rotation_y(yaw),
                local_scale: Vec3::new(opening.width_m as f32, bar as f32, thickness as f32),
                representation: Representation::Full,
                exterior,
                cutaway,
                source_wall: Some(wall.id),
            },
        );
    }
}

fn spawn_stair_geometry(
    commands: &mut Commands,
    assets: &VisualAssets,
    blueprint: &BuildingBlueprint,
    instance: InstanceKind,
    stair: &building_core::Stair,
) {
    let from = blueprint
        .levels
        .iter()
        .find(|level| level.id == stair.from_level)
        .expect("from level");
    let to = blueprint
        .levels
        .iter()
        .find(|level| level.id == stair.to_level)
        .expect("to level");
    let rise = to.elevation_m - from.elevation_m;
    let steps = 8;
    let center = stair.footprint.center();
    let run = stair.footprint.depth();
    let step_depth = run / f64::from(steps);

    for index in 0..steps {
        let progress = (f64::from(index) + 0.5) / f64::from(steps);
        let height = rise.abs() * (f64::from(index) + 1.0) / f64::from(steps);
        let z = stair.footprint.min.y + step_depth * (f64::from(index) + 0.5);
        spawn_part(
            commands,
            assets.cube.clone(),
            assets.stair.clone(),
            BuildingVisualPart {
                instance,
                local_translation: Vec3::new(
                    center.x as f32,
                    (from.elevation_m + height * 0.5) as f32,
                    z as f32,
                ),
                local_rotation: Quat::IDENTITY,
                local_scale: Vec3::new(
                    stair.footprint.width() as f32,
                    height.max(0.05) as f32,
                    step_depth as f32,
                ),
                representation: Representation::Full,
                exterior: false,
                cutaway: None,
                source_wall: None,
            },
        );
        let _ = progress;
    }
}

fn spawn_roof_geometry(
    commands: &mut Commands,
    assets: &VisualAssets,
    compilation: &BuildingCompilation,
    instance: InstanceKind,
    roof: &building_core::RoofRegion,
) {
    let element = ElementRef::Roof(roof.id);
    let cutaway = cutaway_for_element(compilation, element);
    let width = roof.footprint.width();
    let depth = roof.footprint.depth();
    let center = roof.footprint.center();
    let thickness = 0.24;

    match roof.kind {
        RoofKind::Flat => spawn_part(
            commands,
            assets.cube.clone(),
            assets.roof.clone(),
            BuildingVisualPart {
                instance,
                local_translation: Vec3::new(
                    center.x as f32,
                    (roof.base_elevation_m + thickness * 0.5) as f32,
                    center.y as f32,
                ),
                local_rotation: Quat::IDENTITY,
                local_scale: Vec3::new(width as f32, thickness as f32, depth as f32),
                representation: Representation::Full,
                exterior: true,
                cutaway,
                source_wall: None,
            },
        ),
        RoofKind::GableX => {
            let half_depth = depth * 0.5;
            let slope_length = half_depth.hypot(roof.height_m);
            let angle = roof.height_m.atan2(half_depth) as f32;
            for side in [-1.0_f64, 1.0] {
                spawn_part(
                    commands,
                    assets.cube.clone(),
                    assets.roof.clone(),
                    BuildingVisualPart {
                        instance,
                        local_translation: Vec3::new(
                            center.x as f32,
                            (roof.base_elevation_m + roof.height_m * 0.5) as f32,
                            (center.y + side * half_depth * 0.5) as f32,
                        ),
                        local_rotation: Quat::from_rotation_x(angle * side as f32),
                        local_scale: Vec3::new(
                            width as f32,
                            thickness as f32,
                            slope_length as f32,
                        ),
                        representation: Representation::Full,
                        exterior: true,
                        cutaway,
                        source_wall: None,
                    },
                );
            }
        }
        RoofKind::GableZ => {
            let half_width = width * 0.5;
            let slope_length = half_width.hypot(roof.height_m);
            let angle = roof.height_m.atan2(half_width) as f32;
            for side in [-1.0_f64, 1.0] {
                spawn_part(
                    commands,
                    assets.cube.clone(),
                    assets.roof.clone(),
                    BuildingVisualPart {
                        instance,
                        local_translation: Vec3::new(
                            (center.x + side * half_width * 0.5) as f32,
                            (roof.base_elevation_m + roof.height_m * 0.5) as f32,
                            center.y as f32,
                        ),
                        local_rotation: Quat::from_rotation_z(-angle * side as f32),
                        local_scale: Vec3::new(
                            slope_length as f32,
                            thickness as f32,
                            depth as f32,
                        ),
                        representation: Representation::Full,
                        exterior: true,
                        cutaway,
                        source_wall: None,
                    },
                );
            }
        }
    }
}

fn spawn_massing(
    commands: &mut Commands,
    assets: &VisualAssets,
    instance: InstanceKind,
    bounds: Aabb3,
) {
    spawn_part(
        commands,
        assets.cube.clone(),
        assets.massing.clone(),
        BuildingVisualPart {
            instance,
            local_translation: bounds.center().as_vec3(),
            local_rotation: Quat::IDENTITY,
            local_scale: bounds.size().as_vec3(),
            representation: Representation::Massing,
            exterior: true,
            cutaway: None,
            source_wall: None,
        },
    );
}

fn cutaway_for_element(
    compilation: &BuildingCompilation,
    element: ElementRef,
) -> Option<CutawayDirection> {
    compilation
        .cutaway_groups
        .iter()
        .find(|group| group.elements.contains(&element))
        .map(|group| group.direction)
}

fn spawn_part(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    part: BuildingVisualPart,
) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Visible,
        part,
    ));
}
