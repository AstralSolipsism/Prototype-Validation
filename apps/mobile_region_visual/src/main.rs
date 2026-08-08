#![forbid(unsafe_code)]

use bevy::prelude::*;
use glam::{DQuat, DVec3};
use mobile_region_core::{FrameBoundPose, VehiclePoseState, transfer_pose};
use std::f64::consts::FRAC_PI_2;
use world_ids::FrameId;
use world_math::{ReferenceFrameGraph, RigidTransform};

const WORLD_BASE: DVec3 = DVec3::new(9_000_000.0, 0.0, -7_000_000.0);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "P2 · Mobile Reference Frame · unified interior/exterior".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.88)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 900.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (update_motion, control_prototype, update_render_space).chain(),
        )
        .run();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraMode {
    CabinWindow,
    ExteriorCutaway,
}

#[derive(Resource)]
struct PrototypeState {
    frames: ReferenceFrameGraph,
    world_frame: FrameId,
    vehicle_frame: FrameId,
    authoritative_vehicle_pose: RigidTransform,
    presentation_vehicle_pose: RigidTransform,
    elapsed_seconds: f64,
    paused: bool,
    sway_enabled: bool,
    camera_mode: CameraMode,
}

#[derive(Component)]
struct SpatialVisual {
    frame: FrameId,
    local_pose: RigidTransform,
    scale: Vec3,
}

#[derive(Component)]
struct Subject;

#[derive(Component)]
struct PrototypeCamera;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world_frame = FrameId::from_u128(1);
    let vehicle_frame = FrameId::from_u128(2);
    let initial_vehicle_pose = vehicle_pose(0.0);
    let mut frames = ReferenceFrameGraph::default();
    frames.insert_root(world_frame).expect("world frame");
    frames
        .insert_child(vehicle_frame, world_frame, initial_vehicle_pose)
        .expect("vehicle frame");

    commands.insert_resource(PrototypeState {
        frames,
        world_frame,
        vehicle_frame,
        authoritative_vehicle_pose: initial_vehicle_pose,
        presentation_vehicle_pose: initial_vehicle_pose,
        elapsed_seconds: 0.0,
        paused: false,
        sway_enabled: true,
        camera_mode: CameraMode::CabinWindow,
    });

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.36, 0.52),
        perceptual_roughness: 0.55,
        ..default()
    });
    let terrain = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.50, 0.23),
        perceptual_roughness: 1.0,
        ..default()
    });
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.54, 0.57),
        perceptual_roughness: 0.92,
        ..default()
    });
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.28, 0.13),
        perceptual_roughness: 0.88,
        ..default()
    });
    let cabin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.69, 0.51, 0.30),
        perceptual_roughness: 0.9,
        ..default()
    });
    let subject = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.25, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });

    spawn_box(
        &mut commands,
        cube.clone(),
        water,
        world_frame,
        WORLD_BASE + DVec3::new(0.0, -1.2, 0.0),
        Vec3::new(500.0, 1.5, 500.0),
    );
    spawn_box(
        &mut commands,
        cube.clone(),
        terrain,
        world_frame,
        WORLD_BASE + DVec3::new(0.0, 3.0, -28.0),
        Vec3::new(42.0, 8.0, 42.0),
    );
    spawn_box(
        &mut commands,
        cube.clone(),
        stone.clone(),
        world_frame,
        WORLD_BASE + DVec3::new(0.0, 17.0, -28.0),
        Vec3::new(6.0, 28.0, 6.0),
    );
    spawn_box(
        &mut commands,
        cube.clone(),
        stone.clone(),
        world_frame,
        WORLD_BASE + DVec3::new(0.0, 32.0, -28.0),
        Vec3::new(10.0, 2.0, 10.0),
    );
    spawn_box(
        &mut commands,
        cube.clone(),
        stone,
        world_frame,
        WORLD_BASE + DVec3::new(62.0, 0.5, 2.0),
        Vec3::new(34.0, 2.0, 9.0),
    );

    for (position, scale) in [
        (DVec3::new(0.0, -0.6, 0.0), Vec3::new(36.0, 1.2, 10.0)),
        (DVec3::new(0.0, -1.8, 0.0), Vec3::new(30.0, 1.6, 8.0)),
        (DVec3::new(-17.5, 1.4, 0.0), Vec3::new(1.0, 4.0, 9.0)),
        (DVec3::new(17.5, 1.4, 0.0), Vec3::new(1.0, 4.0, 9.0)),
    ] {
        spawn_box(
            &mut commands,
            cube.clone(),
            wood.clone(),
            vehicle_frame,
            position,
            scale,
        );
    }

    for (position, scale) in [
        (DVec3::new(0.0, 0.6, -4.3), Vec3::new(20.0, 1.4, 0.5)),
        (DVec3::new(0.0, 6.3, -4.3), Vec3::new(20.0, 1.4, 0.5)),
        (DVec3::new(-9.5, 3.5, -4.3), Vec3::new(1.0, 4.4, 0.5)),
        (DVec3::new(-5.0, 3.5, -4.3), Vec3::new(0.8, 4.4, 0.5)),
        (DVec3::new(0.0, 3.5, -4.3), Vec3::new(0.8, 4.4, 0.5)),
        (DVec3::new(5.0, 3.5, -4.3), Vec3::new(0.8, 4.4, 0.5)),
        (DVec3::new(9.5, 3.5, -4.3), Vec3::new(1.0, 4.4, 0.5)),
        (DVec3::new(-10.0, 3.5, 0.0), Vec3::new(0.5, 6.8, 8.5)),
        (DVec3::new(10.0, 3.5, 0.0), Vec3::new(0.5, 6.8, 8.5)),
        (DVec3::new(0.0, 7.0, 0.0), Vec3::new(20.5, 0.5, 8.5)),
        (DVec3::new(0.0, 0.0, 0.0), Vec3::new(20.5, 0.4, 8.5)),
    ] {
        spawn_box(
            &mut commands,
            cube.clone(),
            cabin.clone(),
            vehicle_frame,
            position,
            scale,
        );
    }

    for (position, scale) in [
        (DVec3::new(-4.0, 1.0, -1.5), Vec3::new(4.0, 2.0, 2.0)),
        (DVec3::new(4.0, 0.8, 0.8), Vec3::new(3.0, 1.6, 2.5)),
    ] {
        spawn_box(
            &mut commands,
            cube.clone(),
            wood.clone(),
            vehicle_frame,
            position,
            scale,
        );
    }

    commands.spawn((
        Mesh3d(cube),
        MeshMaterial3d(subject),
        Transform::default(),
        SpatialVisual {
            frame: vehicle_frame,
            local_pose: RigidTransform::new(DVec3::new(0.0, 1.2, 1.0), DQuat::IDENTITY)
                .expect("subject pose"),
            scale: Vec3::splat(1.5),
        },
        Subject,
    ));

    commands.spawn((Camera3d::default(), Transform::IDENTITY, PrototypeCamera));
    commands.spawn((
        DirectionalLight {
            illuminance: 20_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.5, 0.0)),
    ));

    info!("Controls: Space pause; C camera mode; B board/disembark; P presentation sway; R reset");
}

fn update_motion(time: Res<Time>, mut state: ResMut<PrototypeState>) {
    if !state.paused {
        state.elapsed_seconds += time.delta_secs_f64();
    }

    let authoritative = vehicle_pose(state.elapsed_seconds);
    let presentation_offset = if state.sway_enabled {
        RigidTransform::new(
            DVec3::new(0.0, (state.elapsed_seconds * 1.6).sin() * 0.22, 0.0),
            (DQuat::from_rotation_z((state.elapsed_seconds * 1.1).sin() * 0.03)
                * DQuat::from_rotation_x((state.elapsed_seconds * 0.8).sin() * 0.018))
            .normalize(),
        )
        .expect("presentation sway")
    } else {
        RigidTransform::IDENTITY
    };
    let vehicle_state =
        VehiclePoseState::new(authoritative).with_presentation_offset(presentation_offset);

    state.authoritative_vehicle_pose = vehicle_state.authoritative_in_parent;
    state.presentation_vehicle_pose = vehicle_state.presentation_in_parent();
    let vehicle_frame = state.vehicle_frame;
    state
        .frames
        .set_pose_in_parent(vehicle_frame, authoritative)
        .expect("moving vehicle frame");
}

fn control_prototype(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<PrototypeState>,
    mut subject: Query<&mut SpatialVisual, With<Subject>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        state.paused = !state.paused;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.sway_enabled = !state.sway_enabled;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.camera_mode = match state.camera_mode {
            CameraMode::CabinWindow => CameraMode::ExteriorCutaway,
            CameraMode::ExteriorCutaway => CameraMode::CabinWindow,
        };
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.elapsed_seconds = 0.0;
        state.paused = false;
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        let mut visual = subject.single_mut().expect("one subject");
        let target = if visual.frame == state.vehicle_frame {
            state.world_frame
        } else {
            state.vehicle_frame
        };

        // Domain transfers use the authoritative frame graph. This visual harness
        // uses a presentation snapshot so the rendered subject does not jump when
        // the ship has client-only sway at the exact transfer frame.
        let mut presentation_frames = state.frames.clone();
        presentation_frames
            .set_pose_in_parent(state.vehicle_frame, state.presentation_vehicle_pose)
            .expect("presentation vehicle frame");
        let transfer = transfer_pose(
            FrameBoundPose::new(visual.frame, visual.local_pose),
            target,
            &presentation_frames,
        )
        .expect("frame transfer");
        visual.frame = transfer.after.frame;
        visual.local_pose = transfer.after.local_pose;
        info!(
            target_frame = %target,
            translation_error_meters = transfer.translation_error_meters,
            "subject frame transfer"
        );
    }
}

fn update_render_space(
    state: Res<PrototypeState>,
    mut visuals: Query<(&SpatialVisual, &mut Transform)>,
    mut camera: Query<&mut Transform, (With<PrototypeCamera>, Without<SpatialVisual>)>,
) {
    let (camera_local_position, camera_local_target) = match state.camera_mode {
        CameraMode::CabinWindow => (DVec3::new(0.0, 4.7, 2.5), DVec3::new(0.0, 3.2, -12.0)),
        CameraMode::ExteriorCutaway => (DVec3::new(-15.0, 10.0, 22.0), DVec3::new(0.0, 2.5, 0.0)),
    };
    let camera_position = state
        .presentation_vehicle_pose
        .transform_point(camera_local_position);
    let camera_target = state
        .presentation_vehicle_pose
        .transform_point(camera_local_target);
    let camera_world =
        RigidTransform::look_at(camera_position, camera_target, DVec3::Y).expect("camera pose");

    for (visual, mut transform) in &mut visuals {
        let frame_world = if visual.frame == state.world_frame {
            RigidTransform::IDENTITY
        } else if visual.frame == state.vehicle_frame {
            state.presentation_vehicle_pose
        } else {
            state
                .frames
                .world_transform(visual.frame)
                .expect("visual frame")
        };
        let object_world = frame_world.compose(visual.local_pose);
        let relative = object_world.relative_to(camera_world);
        transform.translation = relative.translation.as_vec3();
        transform.rotation = quat_from_dquat(relative.rotation);
        transform.scale = visual.scale;
    }

    *camera.single_mut().expect("one camera") = Transform::IDENTITY;
}

fn vehicle_pose(time: f64) -> RigidTransform {
    let angle = time * 0.09;
    let position = WORLD_BASE
        + DVec3::new(
            angle.cos() * 65.0,
            2.5 + (time * 0.25).sin() * 0.25,
            angle.sin() * 65.0,
        );
    RigidTransform::new(position, DQuat::from_rotation_y(-angle - FRAC_PI_2))
        .expect("vehicle navigation pose")
}

fn spawn_box(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    frame: FrameId,
    position: DVec3,
    scale: Vec3,
) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::default(),
        SpatialVisual {
            frame,
            local_pose: RigidTransform::new(position, DQuat::IDENTITY).expect("visual pose"),
            scale,
        },
    ));
}

fn quat_from_dquat(value: DQuat) -> Quat {
    Quat::from_xyzw(
        value.x as f32,
        value.y as f32,
        value.z as f32,
        value.w as f32,
    )
    .normalize()
}
