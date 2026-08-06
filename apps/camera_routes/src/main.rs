#![forbid(unsafe_code)]

use bevy::prelude::*;
use p1_scenario::{TravelDirection, grammar_at, route};
use scroll_camera_core::{CameraRigConfig, CameraRigState, ScrollGrammar, ViewSide};

const ROUTE_SPEED_MPS: f64 = 7.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "P1 · Complex Scroll Route Prototype · visual review required".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.62, 0.76, 0.88)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 850.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (control_route, update_camera).chain())
        .run();
}

#[derive(Component)]
struct Subject;

#[derive(Component)]
struct PrototypeCamera;

#[derive(Resource)]
struct RoutePrototype {
    route: scroll_camera_core::PolylineRoute,
    distance: f64,
    direction: TravelDirection,
    auto_run: bool,
}

#[derive(Resource)]
struct CameraPrototype {
    rig: CameraRigState,
    last_grammar: ScrollGrammar,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let route = route().expect("prototype route must be valid");

    commands.insert_resource(RoutePrototype {
        route: route.clone(),
        distance: 0.0,
        direction: TravelDirection::Forward,
        auto_run: true,
    });
    commands.insert_resource(CameraPrototype {
        rig: CameraRigState::new(
            CameraRigConfig::default(),
            Vec3::X.as_dvec3(),
            ViewSide::Left,
        )
        .expect("prototype camera rig"),
        last_grammar: ScrollGrammar::StandardSideView,
    });

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let road_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.34, 0.35),
        perceptual_roughness: 0.95,
        ..default()
    });
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.27, 0.49, 0.23),
        perceptual_roughness: 1.0,
        ..default()
    });
    let building_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.45, 0.30),
        perceptual_roughness: 0.9,
        ..default()
    });
    let roof_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.12, 0.10),
        perceptual_roughness: 0.85,
        ..default()
    });
    let landmark_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.61, 0.66),
        perceptual_roughness: 0.8,
        ..default()
    });
    let subject_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.32, 0.18),
        ..default()
    });

    spawn_box(
        &mut commands,
        cube.clone(),
        ground_material,
        Vec3::new(0.0, -0.65, -14.0),
        Vec3::new(110.0, 1.0, 90.0),
        Quat::IDENTITY,
    );

    for pair in route.points().windows(2) {
        spawn_road_segment(
            &mut commands,
            cube.clone(),
            road_material.clone(),
            pair[0].as_vec3(),
            pair[1].as_vec3(),
        );
    }

    // Unselected crossroad branches are real geometry rather than background art.
    spawn_road_segment(
        &mut commands,
        cube.clone(),
        road_material.clone(),
        Vec3::ZERO,
        Vec3::new(22.0, 0.0, 0.0),
    );
    spawn_road_segment(
        &mut commands,
        cube.clone(),
        road_material,
        Vec3::ZERO,
        Vec3::new(0.0, 0.0, 18.0),
    );

    for (position, scale) in [
        (Vec3::new(-18.0, 2.5, -8.0), Vec3::new(8.0, 5.0, 6.0)),
        (Vec3::new(-5.0, 3.0, 9.0), Vec3::new(10.0, 6.0, 7.0)),
        (Vec3::new(10.0, 2.0, 8.0), Vec3::new(7.0, 4.0, 8.0)),
        (Vec3::new(10.0, 3.5, -22.0), Vec3::new(9.0, 7.0, 6.0)),
        (Vec3::new(25.0, 2.5, -28.0), Vec3::new(8.0, 5.0, 9.0)),
        (Vec3::new(32.0, 4.0, -45.0), Vec3::new(12.0, 8.0, 8.0)),
    ] {
        spawn_box(
            &mut commands,
            cube.clone(),
            building_material.clone(),
            position,
            scale,
            Quat::IDENTITY,
        );
        spawn_box(
            &mut commands,
            cube.clone(),
            roof_material.clone(),
            position + Vec3::Y * (scale.y * 0.62),
            Vec3::new(scale.x * 1.08, 0.8, scale.z * 1.08),
            Quat::IDENTITY,
        );
    }

    // One stable landmark remains in the same world position through every camera grammar.
    spawn_box(
        &mut commands,
        cube.clone(),
        landmark_material.clone(),
        Vec3::new(18.0, 12.0, -58.0),
        Vec3::new(5.0, 24.0, 5.0),
        Quat::IDENTITY,
    );
    spawn_box(
        &mut commands,
        cube.clone(),
        landmark_material,
        Vec3::new(18.0, 25.0, -58.0),
        Vec3::new(8.0, 2.0, 8.0),
        Quat::IDENTITY,
    );

    commands.spawn((
        Mesh3d(cube),
        MeshMaterial3d(subject_material),
        Transform::from_xyz(-35.0, 0.75, 0.0).with_scale(Vec3::splat(1.5)),
        Subject,
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-35.0, 10.0, -18.0).looking_at(Vec3::ZERO, Vec3::Y),
        PrototypeCamera,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.6, 0.0)),
    ));

    info!("Controls: Space auto-run; A/Left reverse; D/Right forward; R start; E end");
}

fn control_route(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut route_state: ResMut<RoutePrototype>,
    mut subject: Query<&mut Transform, With<Subject>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        route_state.auto_run = !route_state.auto_run;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        route_state.distance = 0.0;
        route_state.direction = TravelDirection::Forward;
        route_state.auto_run = true;
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        route_state.distance = route_state.route.total_length();
        route_state.direction = TravelDirection::Reverse;
        route_state.auto_run = true;
    }

    let manual_direction =
        if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
            Some(TravelDirection::Reverse)
        } else if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
            Some(TravelDirection::Forward)
        } else {
            None
        };

    if let Some(direction) = manual_direction {
        route_state.direction = direction;
    }

    if route_state.auto_run || manual_direction.is_some() {
        route_state.distance = (route_state.distance
            + ROUTE_SPEED_MPS * route_state.direction.sign() * time.delta_secs_f64())
        .clamp(0.0, route_state.route.total_length());

        let reached_end = route_state.direction == TravelDirection::Forward
            && route_state.distance >= route_state.route.total_length();
        let reached_start =
            route_state.direction == TravelDirection::Reverse && route_state.distance <= 0.0;
        if reached_end || reached_start {
            route_state.auto_run = false;
        }
    }

    let frame = route_state
        .route
        .sample(route_state.distance)
        .expect("route sample");
    let mut transform = subject.single_mut().expect("one route subject");
    transform.translation = frame.position.as_vec3() + Vec3::Y * 0.75;
}

fn update_camera(
    time: Res<Time>,
    route_state: Res<RoutePrototype>,
    mut camera_state: ResMut<CameraPrototype>,
    subject: Query<&Transform, With<Subject>>,
    mut camera: Query<&mut Transform, (With<PrototypeCamera>, Without<Subject>)>,
) {
    let frame = route_state
        .route
        .sample(route_state.distance)
        .expect("route sample");
    let grammar = grammar_at(
        route_state.distance,
        route_state.route.total_length(),
        route_state.direction,
    );
    if grammar != camera_state.last_grammar {
        info!(
            ?grammar,
            direction = route_state.direction.label(),
            distance = route_state.distance,
            "camera grammar transition"
        );
        camera_state.last_grammar = grammar;
    }

    let subject_position = subject
        .single()
        .expect("one route subject")
        .translation
        .as_dvec3();
    let pose = camera_state
        .rig
        .update(
            subject_position,
            frame.tangent * route_state.direction.sign(),
            grammar,
            time.delta_secs_f64(),
        )
        .expect("valid camera pose");

    let mut camera_transform = camera.single_mut().expect("one prototype camera");
    *camera_transform = Transform::from_translation(pose.position.as_vec3())
        .looking_at(pose.target.as_vec3(), pose.up.as_vec3());
}

fn spawn_road_segment(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    start: Vec3,
    end: Vec3,
) {
    let delta = end - start;
    let horizontal_length = Vec2::new(delta.x, delta.z).length();
    let midpoint = (start + end) * 0.5 + Vec3::Y * 0.05;
    let yaw = (-delta.z).atan2(delta.x);
    let pitch = delta.y.atan2(horizontal_length);
    spawn_box(
        commands,
        mesh,
        material,
        midpoint,
        Vec3::new(delta.length(), 0.2, 3.4),
        Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch),
    );
}

fn spawn_box(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    scale: Vec3,
    rotation: Quat,
) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform {
            translation: position,
            rotation,
            scale,
        },
    ));
}
