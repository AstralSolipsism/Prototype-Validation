#![forbid(unsafe_code)]

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use building_core::{BuildingCompilation, compile_blueprint};
use p3_building_scenario::two_storey_shop;
use p4_world_scenario::generate_baseline;
use scroll_camera_core::{
    CameraRigConfig, CameraRigState, PolylineRoute, ScrollGrammar, ViewSide,
};
use std::f32::consts::PI;
use world_generation_core::{
    AtlasCellManifest, Biome, BuildingPlacement, HexCoord, HexDirection, PortalKind,
    ScrollRouteManifest, WorldManifest,
};

const WATER_LEVEL_M: f32 = 0.0;
const SUBJECT_HEIGHT_M: f32 = 8.0;
const ROUTE_SPEED_MPS: f64 = 55.0;
const CORNER_NEIGHBORS: [(HexDirection, HexDirection); 6] = [
    (HexDirection::East, HexDirection::SouthEast),
    (HexDirection::SouthEast, HexDirection::SouthWest),
    (HexDirection::SouthWest, HexDirection::West),
    (HexDirection::West, HexDirection::NorthWest),
    (HexDirection::NorthWest, HexDirection::NorthEast),
    (HexDirection::NorthEast, HexDirection::East),
];
const EDGE_CORNERS: [(usize, usize); 6] = [(5, 0), (4, 5), (3, 4), (2, 3), (1, 2), (0, 1)];

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "P4 · deterministic generated world · visual review required".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.58, 0.74, 0.88)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 850.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                control_prototype,
                update_route_subject_and_camera,
                update_building_visibility,
                update_overlay_visibility,
                update_window_title,
            )
                .chain(),
        )
        .run();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraMode {
    Overview,
    FollowRoute,
}

impl CameraMode {
    const fn next(self) -> Self {
        match self {
            Self::Overview => Self::FollowRoute,
            Self::FollowRoute => Self::Overview,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuildingLodMode {
    Full,
    ExteriorShell,
    Massing,
}

impl BuildingLodMode {
    const fn next(self) -> Self {
        match self {
            Self::Full => Self::ExteriorShell,
            Self::ExteriorShell => Self::Massing,
            Self::Massing => Self::Full,
        }
    }
}

#[derive(Component)]
struct PrototypeCamera;

#[derive(Component)]
struct RouteSubject;

#[derive(Component, Clone, Copy)]
struct BuildingRepresentation(BuildingLodMode);

#[derive(Component, Clone, Copy)]
enum OverlayKind {
    CellBoundary,
    Portal,
    Route,
}

struct RouteRuntime {
    polyline: PolylineRoute,
    grammars: Vec<ScrollGrammar>,
}

#[derive(Resource)]
struct WorldVisualState {
    manifest: WorldManifest,
    routes: Vec<RouteRuntime>,
    selected_route: usize,
    distance_m: f64,
    playing: bool,
    camera_mode: CameraMode,
    building_lod: BuildingLodMode,
    view_side: ViewSide,
    rig: CameraRigState,
    current_grammar: ScrollGrammar,
    overview_angle: f32,
    overview_radius: f32,
    overview_height: f32,
    overview_target: Vec3,
    show_cell_boundaries: bool,
    show_portals: bool,
    show_routes: bool,
}

#[derive(Resource, Clone)]
struct VisualAssets {
    cube: Handle<Mesh>,
    terrain_ocean: Handle<StandardMaterial>,
    terrain_coast: Handle<StandardMaterial>,
    terrain_wetland: Handle<StandardMaterial>,
    terrain_forest: Handle<StandardMaterial>,
    terrain_meadow: Handle<StandardMaterial>,
    terrain_alpine: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    river: Handle<StandardMaterial>,
    road: Handle<StandardMaterial>,
    routes: [Handle<StandardMaterial>; 3],
    boundary: Handle<StandardMaterial>,
    portal_river: Handle<StandardMaterial>,
    portal_road: Handle<StandardMaterial>,
    portal_route: Handle<StandardMaterial>,
    building_full: Handle<StandardMaterial>,
    building_shell: Handle<StandardMaterial>,
    building_massing: Handle<StandardMaterial>,
    roof: Handle<StandardMaterial>,
    window: Handle<StandardMaterial>,
    door: Handle<StandardMaterial>,
    plaza: Handle<StandardMaterial>,
    landmark: Handle<StandardMaterial>,
    subject: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world = generate_baseline().expect("P4 baseline world must generate");
    let manifest = world.manifest;
    let building_blueprint = two_storey_shop();
    let building_compilation =
        compile_blueprint(&building_blueprint).expect("P3 building must remain valid");

    let assets = VisualAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        terrain_ocean: material(&mut materials, Color::srgb(0.10, 0.25, 0.34), 0.92),
        terrain_coast: material(&mut materials, Color::srgb(0.66, 0.58, 0.38), 0.95),
        terrain_wetland: material(&mut materials, Color::srgb(0.28, 0.43, 0.25), 0.96),
        terrain_forest: material(&mut materials, Color::srgb(0.20, 0.40, 0.20), 0.96),
        terrain_meadow: material(&mut materials, Color::srgb(0.43, 0.56, 0.29), 0.94),
        terrain_alpine: material(&mut materials, Color::srgb(0.50, 0.50, 0.48), 0.98),
        water: material(&mut materials, Color::srgb(0.10, 0.36, 0.60), 0.72),
        river: material(&mut materials, Color::srgb(0.06, 0.45, 0.76), 0.60),
        road: material(&mut materials, Color::srgb(0.28, 0.21, 0.15), 0.98),
        routes: [
            material(&mut materials, Color::srgb(1.0, 0.45, 0.06), 0.70),
            material(&mut materials, Color::srgb(0.93, 0.75, 0.08), 0.70),
            material(&mut materials, Color::srgb(0.80, 0.20, 0.60), 0.70),
        ],
        boundary: material(&mut materials, Color::srgb(0.08, 0.08, 0.08), 0.85),
        portal_river: material(&mut materials, Color::srgb(0.10, 0.82, 1.0), 0.55),
        portal_road: material(&mut materials, Color::srgb(1.0, 0.68, 0.12), 0.65),
        portal_route: material(&mut materials, Color::srgb(0.96, 0.18, 0.78), 0.60),
        building_full: material(&mut materials, Color::srgb(0.64, 0.43, 0.24), 0.92),
        building_shell: material(&mut materials, Color::srgb(0.77, 0.62, 0.40), 0.92),
        building_massing: material(&mut materials, Color::srgb(0.29, 0.42, 0.66), 0.78),
        roof: material(&mut materials, Color::srgb(0.28, 0.08, 0.07), 0.88),
        window: material(&mut materials, Color::srgb(0.18, 0.64, 0.83), 0.32),
        door: material(&mut materials, Color::srgb(0.14, 0.09, 0.05), 0.88),
        plaza: material(&mut materials, Color::srgb(0.46, 0.43, 0.38), 0.96),
        landmark: material(&mut materials, Color::srgb(0.78, 0.70, 0.53), 0.91),
        subject: material(&mut materials, Color::srgb(0.90, 0.04, 0.04), 0.62),
    };

    spawn_terrain(&mut commands, &mut meshes, &assets, &manifest);
    spawn_water(&mut commands, &assets, &manifest);
    spawn_hydrology(&mut commands, &assets, &manifest);
    spawn_transport(&mut commands, &assets, &manifest);
    spawn_route_overlays(&mut commands, &assets, &manifest);
    spawn_debug_overlays(&mut commands, &assets, &manifest);
    spawn_settlement(
        &mut commands,
        &assets,
        &manifest,
        &building_compilation,
    );
    spawn_landmark(&mut commands, &assets, &manifest);

    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.subject.clone()),
        Transform::from_scale(Vec3::new(6.0, SUBJECT_HEIGHT_M, 6.0)),
        RouteSubject,
    ));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            far: 5_000.0,
            ..default()
        }),
        Transform::IDENTITY,
        PrototypeCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 24_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.58, 0.0)),
    ));

    let routes = manifest
        .routes
        .iter()
        .map(route_runtime)
        .collect::<Vec<_>>();
    let initial_frame = routes[0]
        .polyline
        .sample(0.0)
        .expect("route starts with a valid segment");
    let rig = CameraRigState::new(camera_config(), initial_frame.tangent, ViewSide::Left)
        .expect("P4 camera config must be valid");
    let overview_target = world_center(&manifest);
    commands.insert_resource(WorldVisualState {
        manifest,
        routes,
        selected_route: 0,
        distance_m: 0.0,
        playing: true,
        camera_mode: CameraMode::Overview,
        building_lod: BuildingLodMode::Full,
        view_side: ViewSide::Left,
        rig,
        current_grammar: ScrollGrammar::VistaReveal,
        overview_angle: -0.65,
        overview_radius: 920.0,
        overview_height: 610.0,
        overview_target,
        show_cell_boundaries: true,
        show_portals: false,
        show_routes: true,
    });
    commands.insert_resource(assets);

    info!(
        fingerprint = world.semantic_fingerprint,
        "P4 world loaded. Controls: F overview/follow; 1/2/3 route; Space travel; V view side; L building LOD; H cell edges; P portals; T routes; A/D orbit; Up/Down height; R reset"
    );
}

fn material(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    roughness: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        ..default()
    })
}

fn terrain_material(assets: &VisualAssets, biome: Biome) -> Handle<StandardMaterial> {
    match biome {
        Biome::Ocean => assets.terrain_ocean.clone(),
        Biome::CoastalGrassland => assets.terrain_coast.clone(),
        Biome::Wetland => assets.terrain_wetland.clone(),
        Biome::TemperateForest => assets.terrain_forest.clone(),
        Biome::UplandMeadow => assets.terrain_meadow.clone(),
        Biome::AlpineRock => assets.terrain_alpine.clone(),
    }
}

fn spawn_terrain(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &VisualAssets,
    manifest: &WorldManifest,
) {
    for cell in &manifest.cells {
        commands.spawn((
            Mesh3d(meshes.add(cell_mesh(manifest, cell))),
            MeshMaterial3d(terrain_material(assets, cell.biome)),
            Transform::IDENTITY,
        ));
    }
}

fn cell_mesh(manifest: &WorldManifest, cell: &AtlasCellManifest) -> Mesh {
    let mut positions = Vec::with_capacity(7);
    let mut normals = Vec::with_capacity(7);
    let mut uvs = Vec::with_capacity(7);
    positions.push([
        cell.center_world.x as f32,
        cell.elevation_m as f32,
        cell.center_world.z as f32,
    ]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for corner in 0..6 {
        let position = corner_position(manifest, cell.coord, corner);
        positions.push(position.to_array());
        normals.push([0.0, 1.0, 0.0]);
        let angle = (30.0 + corner as f32 * 60.0).to_radians();
        uvs.push([0.5 + angle.cos() * 0.5, 0.5 + angle.sin() * 0.5]);
    }

    let mut indices = Vec::with_capacity(18);
    for corner in 0..6_u32 {
        let current = corner + 1;
        let next = (corner + 1) % 6 + 1;
        indices.extend_from_slice(&[0, next, current]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

fn corner_position(manifest: &WorldManifest, coord: HexCoord, corner: usize) -> Vec3 {
    let cell = manifest.cell(coord).expect("corner cell exists");
    let angle = (30.0 + corner as f64 * 60.0).to_radians();
    Vec3::new(
        (cell.center_world.x + manifest.cell_radius_m * angle.cos()) as f32,
        corner_height(manifest, coord, corner) as f32,
        (cell.center_world.z + manifest.cell_radius_m * angle.sin()) as f32,
    )
}

fn corner_height(manifest: &WorldManifest, coord: HexCoord, corner: usize) -> f64 {
    let mut total = manifest.cell(coord).expect("corner cell exists").elevation_m;
    let mut count = 1.0;
    for direction in [CORNER_NEIGHBORS[corner].0, CORNER_NEIGHBORS[corner].1] {
        if let Some(neighbor) = manifest.cell(coord.neighbor(direction)) {
            total += neighbor.elevation_m;
            count += 1.0;
        }
    }
    total / count
}

fn spawn_water(commands: &mut Commands, assets: &VisualAssets, manifest: &WorldManifest) {
    let (min, max) = world_bounds_xz(manifest);
    let size = max - min + Vec2::splat((manifest.cell_radius_m * 2.4) as f32);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.water.clone()),
        Transform::from_xyz(
            (min.x + max.x) * 0.5,
            WATER_LEVEL_M - 0.55,
            (min.y + max.y) * 0.5,
        )
        .with_scale(Vec3::new(size.x, 1.0, size.y)),
    ));
}

fn spawn_hydrology(commands: &mut Commands, assets: &VisualAssets, manifest: &WorldManifest) {
    for pair in manifest.river.cells.windows(2) {
        let start = river_point(manifest, pair[0]);
        let end = river_point(manifest, pair[1]);
        spawn_segment(
            commands,
            assets,
            assets.river.clone(),
            start,
            end,
            16.0,
            1.4,
            None,
        );
    }
}

fn river_point(manifest: &WorldManifest, coord: HexCoord) -> Vec3 {
    let cell = manifest.cell(coord).expect("river cell exists");
    let y = if cell.is_ocean() {
        WATER_LEVEL_M + 0.2
    } else {
        cell.elevation_m as f32 + 2.2
    };
    Vec3::new(cell.center_world.x as f32, y, cell.center_world.z as f32)
}

fn spawn_transport(commands: &mut Commands, assets: &VisualAssets, manifest: &WorldManifest) {
    for road in &manifest.roads {
        for pair in road.cells.windows(2) {
            spawn_segment(
                commands,
                assets,
                assets.road.clone(),
                cell_surface_point(manifest, pair[0], 3.3),
                cell_surface_point(manifest, pair[1], 3.3),
                9.0,
                1.0,
                None,
            );
        }
    }
}

fn spawn_route_overlays(
    commands: &mut Commands,
    assets: &VisualAssets,
    manifest: &WorldManifest,
) {
    for (route_index, route) in manifest.routes.iter().enumerate() {
        for pair in route.nodes.windows(2) {
            spawn_segment(
                commands,
                assets,
                assets.routes[route_index % assets.routes.len()].clone(),
                pair[0].world_position.as_vec3() + Vec3::Y * 6.0,
                pair[1].world_position.as_vec3() + Vec3::Y * 6.0,
                2.3,
                1.4,
                Some(OverlayKind::Route),
            );
        }
    }
}

fn spawn_debug_overlays(
    commands: &mut Commands,
    assets: &VisualAssets,
    manifest: &WorldManifest,
) {
    for cell in &manifest.cells {
        for direction in HexDirection::ALL {
            let neighbor = cell.coord.neighbor(direction);
            if manifest.cell(neighbor).is_some() && cell.coord > neighbor {
                continue;
            }
            let (left_corner, right_corner) = EDGE_CORNERS[direction.index()];
            spawn_segment(
                commands,
                assets,
                assets.boundary.clone(),
                corner_position(manifest, cell.coord, left_corner) + Vec3::Y * 1.2,
                corner_position(manifest, cell.coord, right_corner) + Vec3::Y * 1.2,
                1.2,
                0.8,
                Some(OverlayKind::CellBoundary),
            );
        }
    }

    for portal in &manifest.portals {
        let portal_material = match portal.kind {
            PortalKind::River => assets.portal_river.clone(),
            PortalKind::Road => assets.portal_road.clone(),
            PortalKind::ScrollRoute => assets.portal_route.clone(),
        };
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(portal_material),
            Transform::from_translation(portal.world_position.as_vec3() + Vec3::Y * 4.0)
                .with_scale(Vec3::splat(5.0)),
            Visibility::Hidden,
            OverlayKind::Portal,
        ));
    }
}

fn spawn_settlement(
    commands: &mut Commands,
    assets: &VisualAssets,
    manifest: &WorldManifest,
    compilation: &BuildingCompilation,
) {
    let settlement = manifest
        .settlements
        .first()
        .expect("P4 contains one settlement");
    let cell = manifest
        .cell(settlement.cell)
        .expect("settlement cell exists");
    let root = Vec3::new(
        cell.center_world.x as f32,
        cell.elevation_m as f32 + 1.2,
        cell.center_world.z as f32,
    );
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.plaza.clone()),
        Transform::from_translation(root + Vec3::Y * 0.2)
            .with_scale(Vec3::new(72.0, 0.8, 50.0)),
    ));

    for placement in &settlement.buildings {
        spawn_building_placement(commands, assets, root, placement, compilation);
    }
}

fn spawn_building_placement(
    commands: &mut Commands,
    assets: &VisualAssets,
    settlement_root: Vec3,
    placement: &BuildingPlacement,
    compilation: &BuildingCompilation,
) {
    let root_rotation = Quat::from_rotation_y(placement.yaw_radians as f32);
    let root_translation = settlement_root + placement.local_position.as_vec3();
    let bounds = compilation.massing.bounds;
    let size = bounds.size().as_vec3();
    let center = bounds.center().as_vec3();
    let body_height = 6.0_f32;
    let body_size = Vec3::new(size.x - 0.6, body_height, size.z - 0.6);

    spawn_building_part(
        commands,
        assets,
        assets.building_full.clone(),
        root_translation,
        root_rotation,
        Vec3::new(center.x, 1.5, center.z),
        Quat::IDENTITY,
        Vec3::new(body_size.x, 3.0, body_size.z),
        BuildingLodMode::Full,
    );
    spawn_building_part(
        commands,
        assets,
        assets.building_full.clone(),
        root_translation,
        root_rotation,
        Vec3::new(center.x, 4.5, center.z),
        Quat::IDENTITY,
        Vec3::new(body_size.x, 3.0, body_size.z),
        BuildingLodMode::Full,
    );
    spawn_gable_roof(
        commands,
        assets,
        assets.roof.clone(),
        root_translation,
        root_rotation,
        size,
        BuildingLodMode::Full,
    );
    spawn_facade_details(commands, assets, root_translation, root_rotation, body_size);

    spawn_building_part(
        commands,
        assets,
        assets.building_shell.clone(),
        root_translation,
        root_rotation,
        Vec3::new(center.x, body_height * 0.5, center.z),
        Quat::IDENTITY,
        Vec3::new(size.x - 0.25, body_height, size.z - 0.25),
        BuildingLodMode::ExteriorShell,
    );
    spawn_gable_roof(
        commands,
        assets,
        assets.roof.clone(),
        root_translation,
        root_rotation,
        size,
        BuildingLodMode::ExteriorShell,
    );

    spawn_building_part(
        commands,
        assets,
        assets.building_massing.clone(),
        root_translation,
        root_rotation,
        center,
        Quat::IDENTITY,
        size,
        BuildingLodMode::Massing,
    );
}

fn spawn_gable_roof(
    commands: &mut Commands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    root_translation: Vec3,
    root_rotation: Quat,
    size: Vec3,
    representation: BuildingLodMode,
) {
    let roof_height = (size.y - 6.0).max(1.0);
    let half_depth = size.z * 0.5;
    let slope_length = (half_depth * half_depth + roof_height * roof_height).sqrt();
    let angle = roof_height.atan2(half_depth);
    for side in [-1.0_f32, 1.0] {
        spawn_building_part(
            commands,
            assets,
            material.clone(),
            root_translation,
            root_rotation,
            Vec3::new(0.0, 6.0 + roof_height * 0.5, side * half_depth * 0.5),
            Quat::from_rotation_x(angle * side),
            Vec3::new(size.x, 0.32, slope_length),
            representation,
        );
    }
}

fn spawn_facade_details(
    commands: &mut Commands,
    assets: &VisualAssets,
    root_translation: Vec3,
    root_rotation: Quat,
    body_size: Vec3,
) {
    spawn_building_part(
        commands,
        assets,
        assets.door.clone(),
        root_translation,
        root_rotation,
        Vec3::new(0.0, 1.15, body_size.z * 0.5 + 0.08),
        Quat::IDENTITY,
        Vec3::new(1.5, 2.3, 0.18),
        BuildingLodMode::Full,
    );
    for floor_y in [1.7, 4.7] {
        for x in [-3.7, 3.7] {
            spawn_building_part(
                commands,
                assets,
                assets.window.clone(),
                root_translation,
                root_rotation,
                Vec3::new(x, floor_y, body_size.z * 0.5 + 0.10),
                Quat::IDENTITY,
                Vec3::new(1.8, 1.3, 0.16),
                BuildingLodMode::Full,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_building_part(
    commands: &mut Commands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    root_translation: Vec3,
    root_rotation: Quat,
    local_translation: Vec3,
    local_rotation: Quat,
    scale: Vec3,
    representation: BuildingLodMode,
) {
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform {
            translation: root_translation + root_rotation * local_translation,
            rotation: root_rotation * local_rotation,
            scale,
        },
        Visibility::Visible,
        BuildingRepresentation(representation),
    ));
}

fn spawn_landmark(commands: &mut Commands, assets: &VisualAssets, manifest: &WorldManifest) {
    let landmark = manifest
        .landmarks
        .first()
        .expect("P4 contains one landmark");
    let cell = manifest.cell(landmark.cell).expect("landmark cell exists");
    let height = (landmark.world_position.y - cell.elevation_m).max(20.0) as f32;
    let base = Vec3::new(
        landmark.world_position.x as f32,
        cell.elevation_m as f32,
        landmark.world_position.z as f32,
    );
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.landmark.clone()),
        Transform::from_translation(base + Vec3::Y * (height * 0.5))
            .with_scale(Vec3::new(15.0, height, 15.0)),
    ));
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.roof.clone()),
        Transform::from_translation(base + Vec3::Y * (height + 8.0))
            .with_rotation(Quat::from_rotation_y(PI * 0.25))
            .with_scale(Vec3::new(19.0, 16.0, 19.0)),
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_segment(
    commands: &mut Commands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    width: f32,
    height: f32,
    overlay: Option<OverlayKind>,
) {
    let delta = end - start;
    let length = delta.length();
    if length <= 1.0e-4 || !length.is_finite() {
        return;
    }
    let rotation = Quat::from_rotation_arc(Vec3::X, delta / length);
    let transform = Transform {
        translation: (start + end) * 0.5,
        rotation,
        scale: Vec3::new(length, height, width),
    };
    let mut entity = commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        transform,
        Visibility::Visible,
    ));
    if let Some(overlay) = overlay {
        entity.insert(overlay);
    }
}

fn cell_surface_point(manifest: &WorldManifest, coord: HexCoord, offset_y: f32) -> Vec3 {
    let cell = manifest.cell(coord).expect("surface cell exists");
    cell.center_world.as_vec3() + Vec3::Y * offset_y
}

fn route_runtime(route: &ScrollRouteManifest) -> RouteRuntime {
    RouteRuntime {
        polyline: PolylineRoute::new(
            route
                .nodes
                .iter()
                .map(|node| node.world_position)
                .collect(),
        )
        .expect("generated P4 route is a valid polyline"),
        grammars: route.nodes.iter().map(|node| node.grammar).collect(),
    }
}

fn camera_config() -> CameraRigConfig {
    CameraRigConfig {
        side_distance: 92.0,
        height: 58.0,
        look_ahead: 30.0,
        look_height: 8.0,
        trailing_offset: 10.0,
        heading_half_life_seconds: 0.75,
        composition_half_life_seconds: 0.6,
    }
}

fn control_prototype(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<WorldVisualState>,
) {
    if keyboard.just_pressed(KeyCode::KeyF) {
        state.camera_mode = state.camera_mode.next();
    }
    if keyboard.just_pressed(KeyCode::Space) {
        let total = state.routes[state.selected_route].polyline.total_length();
        if state.distance_m >= total - 1.0e-6 {
            state.distance_m = 0.0;
        }
        state.playing = !state.playing;
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        state.building_lod = state.building_lod.next();
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        state.show_cell_boundaries = !state.show_cell_boundaries;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.show_portals = !state.show_portals;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        state.show_routes = !state.show_routes;
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        state.view_side = match state.view_side {
            ViewSide::Left => ViewSide::Right,
            ViewSide::Right => ViewSide::Left,
        };
        let view_side = state.view_side;
        state.rig.set_view_side(view_side);
    }

    for (key, route_index) in [
        (KeyCode::Digit1, 0_usize),
        (KeyCode::Digit2, 1_usize),
        (KeyCode::Digit3, 2_usize),
    ] {
        if keyboard.just_pressed(key) && route_index < state.routes.len() {
            reset_route(&mut state, route_index);
        }
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        state.camera_mode = CameraMode::Overview;
        state.building_lod = BuildingLodMode::Full;
        state.show_cell_boundaries = true;
        state.show_portals = false;
        state.show_routes = true;
        state.overview_angle = -0.65;
        state.overview_height = 610.0;
        state.view_side = ViewSide::Left;
        reset_route(&mut state, 0);
    }

    if state.camera_mode == CameraMode::Overview {
        let angular_speed = 0.55;
        if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
            state.overview_angle += angular_speed * time.delta_secs();
        }
        if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
            state.overview_angle -= angular_speed * time.delta_secs();
        }
        if keyboard.pressed(KeyCode::ArrowUp) {
            state.overview_height =
                (state.overview_height + 180.0 * time.delta_secs()).clamp(220.0, 1_100.0);
        }
        if keyboard.pressed(KeyCode::ArrowDown) {
            state.overview_height =
                (state.overview_height - 180.0 * time.delta_secs()).clamp(220.0, 1_100.0);
        }
    }
}

fn reset_route(state: &mut WorldVisualState, route_index: usize) {
    state.selected_route = route_index;
    state.distance_m = 0.0;
    state.playing = true;
    let first = state.routes[route_index]
        .polyline
        .sample(0.0)
        .expect("selected route has a first segment");
    state.rig = CameraRigState::new(camera_config(), first.tangent, state.view_side)
        .expect("P4 camera config must remain valid");
    state.current_grammar = state.routes[route_index].grammars[0];
}

fn update_route_subject_and_camera(
    time: Res<Time>,
    mut state: ResMut<WorldVisualState>,
    mut subject: Query<&mut Transform, (With<RouteSubject>, Without<PrototypeCamera>)>,
    mut camera: Query<&mut Transform, (With<PrototypeCamera>, Without<RouteSubject>)>,
) {
    let selected_route = state.selected_route;
    let total_length = state.routes[selected_route].polyline.total_length();
    if state.playing {
        state.distance_m =
            (state.distance_m + ROUTE_SPEED_MPS * f64::from(time.delta_secs())).min(total_length);
        if state.distance_m >= total_length - 1.0e-6 {
            state.playing = false;
        }
    }

    let frame = state.routes[selected_route]
        .polyline
        .sample(state.distance_m)
        .expect("route distance remains valid");
    let grammar_index = frame
        .segment_index
        .min(state.routes[selected_route].grammars.len() - 1);
    let grammar = state.routes[selected_route].grammars[grammar_index];
    state.current_grammar = grammar;

    let mut subject_transform = subject.single_mut().expect("one P4 route subject");
    subject_transform.translation = frame.position.as_vec3() + Vec3::Y * (SUBJECT_HEIGHT_M * 0.5);
    subject_transform.rotation = Quat::from_rotation_y(
        (-(frame.tangent.z as f32)).atan2(frame.tangent.x as f32),
    );

    let mut camera_transform = camera.single_mut().expect("one P4 camera");
    match state.camera_mode {
        CameraMode::FollowRoute => {
            let pose = state
                .rig
                .update(
                    frame.position + glam_offset_y(SUBJECT_HEIGHT_M as f64 * 0.5),
                    frame.tangent,
                    grammar,
                    f64::from(time.delta_secs()),
                )
                .expect("generated route produces a valid camera pose");
            *camera_transform = Transform::from_translation(pose.position.as_vec3())
                .looking_at(pose.target.as_vec3(), pose.up.as_vec3());
        }
        CameraMode::Overview => {
            let position = state.overview_target
                + Vec3::new(
                    state.overview_angle.sin() * state.overview_radius,
                    state.overview_height,
                    state.overview_angle.cos() * state.overview_radius,
                );
            *camera_transform = Transform::from_translation(position)
                .looking_at(state.overview_target, Vec3::Y);
        }
    }
}

fn glam_offset_y(y: f64) -> glam::DVec3 {
    glam::DVec3::new(0.0, y, 0.0)
}

fn update_building_visibility(
    state: Res<WorldVisualState>,
    mut buildings: Query<(&BuildingRepresentation, &mut Visibility)>,
) {
    for (representation, mut visibility) in &mut buildings {
        *visibility = if representation.0 == state.building_lod {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_overlay_visibility(
    state: Res<WorldVisualState>,
    mut overlays: Query<(&OverlayKind, &mut Visibility)>,
) {
    for (kind, mut visibility) in &mut overlays {
        let visible = match kind {
            OverlayKind::CellBoundary => state.show_cell_boundaries,
            OverlayKind::Portal => state.show_portals,
            OverlayKind::Route => state.show_routes,
        };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_window_title(
    state: Res<WorldVisualState>,
    mut windows: Query<&mut Window>,
) {
    let mut window = windows.single_mut().expect("one P4 window");
    window.title = format!(
        "P4 · {:?} · route {} · {:?} · {:.0} m · LOD {:?} · cells {} · portals {}",
        state.camera_mode,
        state.selected_route + 1,
        state.current_grammar,
        state.distance_m,
        state.building_lod,
        state.show_cell_boundaries,
        state.show_portals,
    );
}

fn world_bounds_xz(manifest: &WorldManifest) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for cell in &manifest.cells {
        let point = Vec2::new(cell.center_world.x as f32, cell.center_world.z as f32);
        min = min.min(point);
        max = max.max(point);
    }
    (min, max)
}

fn world_center(manifest: &WorldManifest) -> Vec3 {
    let sum = manifest
        .cells
        .iter()
        .fold(Vec3::ZERO, |sum, cell| sum + cell.center_world.as_vec3());
    let mut center = sum / manifest.cells.len() as f32;
    center.y = manifest
        .cells
        .iter()
        .filter(|cell| !cell.is_ocean())
        .map(|cell| cell.elevation_m as f32)
        .sum::<f32>()
        / manifest.cells.iter().filter(|cell| !cell.is_ocean()).count() as f32;
    center
}
