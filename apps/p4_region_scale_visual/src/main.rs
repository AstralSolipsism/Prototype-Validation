#![forbid(unsafe_code)]

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use glam::Vec3Swizzles;
use p4_region_scale_scenario::generate_region_scale_world;
use region_scale_core::{
    ActiveCellSet, AtlasCellRegion, CellMaterialization, GroundingResult, LandformClass,
    RegionScaleWorld, RouteSurfaceKind, activate_cell, terrain_height,
};
use scroll_camera_core::{CameraRigConfig, CameraRigState, PolylineRoute, ScrollGrammar, ViewSide};
use world_generation_core::HexCoord;

const ATLAS_SCALE: f32 = 0.045;
const ATLAS_CAMERA_HEIGHT: f32 = 980.0;
const SUBJECT_HEIGHT_M: f32 = 18.0;
const ROUTE_SPEED_MPS: f64 = 160.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "P4 corrected region-scale materialization · manual review pending".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.45, 0.63, 0.78)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 850.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                control_app,
                rebuild_local_scene,
                update_visibility,
                update_atlas_materials,
                update_selection_marker,
                update_camera_and_subject,
                update_window_title,
            )
                .chain(),
        )
        .run();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppMode {
    Atlas,
    LocalOverview,
    RouteTravel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AtlasLayer {
    Elevation,
    Landforms,
    Capacity,
}

impl AtlasLayer {
    const fn next(self) -> Self {
        match self {
            Self::Elevation => Self::Landforms,
            Self::Landforms => Self::Capacity,
            Self::Capacity => Self::Elevation,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuildingLod {
    Full,
    Shell,
    Massing,
}

impl BuildingLod {
    const fn next(self) -> Self {
        match self {
            Self::Full => Self::Shell,
            Self::Shell => Self::Massing,
            Self::Massing => Self::Full,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpaceLayer {
    Atlas,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalVisibility {
    Always,
    Route,
    NeighborProxy,
    CellBoundary,
    Foundation,
    BuildingFull,
    BuildingShell,
    BuildingMassing,
}

#[derive(Component)]
struct PrototypeCamera;

#[derive(Component)]
struct RouteSubject;

#[derive(Component)]
struct AtlasSelectionMarker;

#[derive(Component)]
struct AtlasCellVisual {
    coord: HexCoord,
}

#[derive(Component)]
struct SpaceTag(SpaceLayer);

#[derive(Component)]
struct LocalDynamic;

#[derive(Component)]
struct LocalVisual(LocalVisibility);

#[derive(Resource)]
struct WorldData(RegionScaleWorld);

#[derive(Resource)]
struct VisualState {
    mode: AppMode,
    atlas_layer: AtlasLayer,
    selected_cell_index: usize,
    active_cell: HexCoord,
    show_neighbors: bool,
    show_routes: bool,
    show_cell_boundary: bool,
    show_foundations: bool,
    building_lod: BuildingLod,
    route_index: usize,
    route_distance_m: f64,
    route_paused: bool,
    view_side: ViewSide,
    overview_angle: f32,
    overview_height: f32,
    overview_radius: f32,
}

#[derive(Resource)]
struct SceneDirty(bool);

#[derive(Resource)]
struct RouteRuntimes(Vec<RouteRuntime>);

struct RouteRuntime {
    polyline: PolylineRoute,
}

#[derive(Resource)]
struct RouteCamera(CameraRigState);

#[derive(Resource, Clone)]
struct VisualAssets {
    cube: Handle<Mesh>,
    atlas_boundary: Handle<StandardMaterial>,
    selection: Handle<StandardMaterial>,
    terrain_full: Handle<StandardMaterial>,
    terrain_proxy: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    route: [Handle<StandardMaterial>; 3],
    bridge: Handle<StandardMaterial>,
    building: Handle<StandardMaterial>,
    building_shell: Handle<StandardMaterial>,
    building_massing: Handle<StandardMaterial>,
    foundation: Handle<StandardMaterial>,
    roof: Handle<StandardMaterial>,
    landmark: Handle<StandardMaterial>,
    subject: Handle<StandardMaterial>,
    cell_boundary: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world = generate_region_scale_world().expect("region-scale fixture must compile");
    let selected_cell_index = world
        .atlas
        .cells
        .iter()
        .position(|cell| cell.coord == world.atlas.port_cell)
        .unwrap_or(0);
    let active_cell = world.atlas.port_cell;
    let routes = world
        .routes
        .iter()
        .map(|route| RouteRuntime {
            polyline: PolylineRoute::new(
                route
                    .points
                    .iter()
                    .map(|point| point.world_position)
                    .collect(),
            )
            .expect("route polyline"),
        })
        .collect::<Vec<_>>();
    let initial_tangent = routes
        .first()
        .and_then(|runtime| runtime.polyline.sample(0.0).ok())
        .map(|frame| frame.tangent)
        .unwrap_or(glam::DVec3::X);
    let route_camera = CameraRigState::new(
        CameraRigConfig {
            side_distance: 95.0,
            height: 58.0,
            look_ahead: 42.0,
            look_height: 10.0,
            trailing_offset: 8.0,
            heading_half_life_seconds: 0.72,
            composition_half_life_seconds: 0.58,
        },
        initial_tangent,
        ViewSide::Left,
    )
    .expect("route camera configuration");

    let assets = VisualAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        atlas_boundary: material(&mut materials, Color::srgb(0.10, 0.12, 0.15), 0.92),
        selection: material(&mut materials, Color::srgb(1.0, 0.82, 0.15), 0.45),
        terrain_full: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            ..default()
        }),
        terrain_proxy: materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.76, 0.70),
            perceptual_roughness: 1.0,
            ..default()
        }),
        water: material(&mut materials, Color::srgb(0.08, 0.37, 0.68), 0.38),
        route: [
            material(&mut materials, Color::srgb(0.96, 0.22, 0.12), 0.76),
            material(&mut materials, Color::srgb(0.26, 0.90, 0.34), 0.76),
            material(&mut materials, Color::srgb(0.78, 0.34, 0.94), 0.76),
        ],
        bridge: material(&mut materials, Color::srgb(0.32, 0.18, 0.08), 0.88),
        building: material(&mut materials, Color::srgb(0.72, 0.49, 0.25), 0.90),
        building_shell: material(&mut materials, Color::srgb(0.36, 0.58, 0.70), 0.84),
        building_massing: material(&mut materials, Color::srgb(0.31, 0.42, 0.64), 0.76),
        foundation: material(&mut materials, Color::srgb(0.30, 0.28, 0.24), 0.98),
        roof: material(&mut materials, Color::srgb(0.31, 0.10, 0.08), 0.90),
        landmark: material(&mut materials, Color::srgb(0.86, 0.80, 0.62), 0.78),
        subject: material(&mut materials, Color::srgb(0.94, 0.05, 0.04), 0.62),
        cell_boundary: material(&mut materials, Color::srgb(1.0, 0.72, 0.10), 0.70),
    };

    spawn_atlas(&mut commands, &mut meshes, &mut materials, &assets, &world);

    commands.spawn((Camera3d::default(), Transform::IDENTITY, PrototypeCamera));
    commands.spawn((
        DirectionalLight {
            illuminance: 25_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.55, 0.0)),
    ));
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.subject.clone()),
        Transform::IDENTITY.with_scale(Vec3::new(10.0, SUBJECT_HEIGHT_M, 10.0)),
        RouteSubject,
        SpaceTag(SpaceLayer::Local),
    ));

    commands.insert_resource(WorldData(world));
    commands.insert_resource(RouteRuntimes(routes));
    commands.insert_resource(RouteCamera(route_camera));
    commands.insert_resource(assets);
    commands.insert_resource(VisualState {
        mode: AppMode::Atlas,
        atlas_layer: AtlasLayer::Elevation,
        selected_cell_index,
        active_cell,
        show_neighbors: true,
        show_routes: true,
        show_cell_boundary: true,
        show_foundations: true,
        building_lod: BuildingLod::Full,
        route_index: 0,
        route_distance_m: 0.0,
        route_paused: true,
        view_side: ViewSide::Left,
        overview_angle: 0.72,
        overview_height: 1_750.0,
        overview_radius: 2_800.0,
    });
    commands.insert_resource(SceneDirty(true));

    info!(
        "Controls: M Atlas/local; Tab Atlas layer; Q/E select; Enter activate cell; 1/2/3 route; Space pause; V side; L building LOD; N neighbor proxies; G foundations; H cell boundary; T routes; A/D orbit; Up/Down height; R reset"
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

fn spawn_atlas(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &VisualAssets,
    world: &RegionScaleWorld,
) {
    for cell in &world.atlas.cells {
        let unique_material = materials.add(StandardMaterial {
            base_color: atlas_cell_color(cell, AtlasLayer::Elevation, &world.atlas.cells),
            perceptual_roughness: 0.96,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(flat_hex_mesh(cell.coord, world.atlas.cell_radius_m))),
            MeshMaterial3d(unique_material),
            Transform::from_scale(Vec3::new(ATLAS_SCALE, 1.0, ATLAS_SCALE)),
            AtlasCellVisual { coord: cell.coord },
            SpaceTag(SpaceLayer::Atlas),
        ));
        spawn_hex_outline(
            commands,
            assets,
            cell.coord,
            world.atlas.cell_radius_m,
            ATLAS_SCALE,
            2.2,
            assets.atlas_boundary.clone(),
            SpaceLayer::Atlas,
            None,
        );
    }

    let port = world.atlas.port_cell.center_xz(world.atlas.cell_radius_m) * f64::from(ATLAS_SCALE);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.building.clone()),
        Transform::from_xyz(port.x as f32, 12.0, port.y as f32)
            .with_scale(Vec3::new(24.0, 24.0, 24.0)),
        SpaceTag(SpaceLayer::Atlas),
    ));
    let landmark = world.landmark_world.xz() * f64::from(ATLAS_SCALE);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.landmark.clone()),
        Transform::from_xyz(landmark.x as f32, 30.0, landmark.y as f32)
            .with_scale(Vec3::new(12.0, 60.0, 12.0)),
        SpaceTag(SpaceLayer::Atlas),
    ));
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.selection.clone()),
        Transform::IDENTITY,
        AtlasSelectionMarker,
        SpaceTag(SpaceLayer::Atlas),
    ));
}

fn control_app(
    keyboard: Res<ButtonInput<KeyCode>>,
    world: Res<WorldData>,
    routes: Res<RouteRuntimes>,
    mut state: ResMut<VisualState>,
    mut dirty: ResMut<SceneDirty>,
    mut route_camera: ResMut<RouteCamera>,
) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        state.mode = match state.mode {
            AppMode::Atlas => AppMode::LocalOverview,
            AppMode::LocalOverview | AppMode::RouteTravel => AppMode::Atlas,
        };
        dirty.0 = state.mode != AppMode::Atlas;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        state.atlas_layer = state.atlas_layer.next();
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        state.selected_cell_index = state
            .selected_cell_index
            .checked_sub(1)
            .unwrap_or(world.0.atlas.cells.len() - 1);
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        state.selected_cell_index = (state.selected_cell_index + 1) % world.0.atlas.cells.len();
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        state.active_cell = world.0.atlas.cells[state.selected_cell_index].coord;
        state.mode = AppMode::LocalOverview;
        dirty.0 = true;
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        state.show_neighbors = !state.show_neighbors;
        dirty.0 = state.mode != AppMode::Atlas;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        state.show_routes = !state.show_routes;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        state.show_cell_boundary = !state.show_cell_boundary;
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        state.show_foundations = !state.show_foundations;
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        state.building_lod = state.building_lod.next();
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        state.view_side = match state.view_side {
            ViewSide::Left => ViewSide::Right,
            ViewSide::Right => ViewSide::Left,
        };
        route_camera.0.set_view_side(state.view_side);
    }
    if keyboard.just_pressed(KeyCode::Space) {
        state.route_paused = !state.route_paused;
    }
    for (key, route_index) in [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1usize),
        (KeyCode::Digit3, 2usize),
    ] {
        if keyboard.just_pressed(key) && route_index < routes.0.len() {
            state.route_index = route_index;
            state.route_distance_m = 0.0;
            state.route_paused = false;
            state.mode = AppMode::RouteTravel;
            let frame = routes.0[route_index]
                .polyline
                .sample(0.0)
                .expect("route starts");
            state.active_cell = world.0.atlas.nearest_cell(frame.position.xz());
            dirty.0 = true;
            route_camera.0 = CameraRigState::new(
                CameraRigConfig {
                    side_distance: 95.0,
                    height: 58.0,
                    look_ahead: 42.0,
                    look_height: 10.0,
                    trailing_offset: 8.0,
                    heading_half_life_seconds: 0.72,
                    composition_half_life_seconds: 0.58,
                },
                frame.tangent,
                state.view_side,
            )
            .expect("route camera");
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.mode = AppMode::Atlas;
        state.atlas_layer = AtlasLayer::Elevation;
        state.selected_cell_index = world
            .0
            .atlas
            .cells
            .iter()
            .position(|cell| cell.coord == world.0.atlas.port_cell)
            .unwrap_or(0);
        state.active_cell = world.0.atlas.port_cell;
        state.route_index = 0;
        state.route_distance_m = 0.0;
        state.route_paused = true;
        state.view_side = ViewSide::Left;
        state.overview_angle = 0.72;
        state.overview_height = 1_750.0;
        state.overview_radius = 2_800.0;
        route_camera.0.set_view_side(ViewSide::Left);
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_local_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    world: Res<WorldData>,
    state: Res<VisualState>,
    mut dirty: ResMut<SceneDirty>,
    assets: Res<VisualAssets>,
    existing: Query<Entity, With<LocalDynamic>>,
) {
    if !dirty.0 || state.mode == AppMode::Atlas {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let active = activate_cell(&world.0, state.active_cell).expect("active cell materialization");
    spawn_materialization(
        &mut commands,
        &mut meshes,
        &assets,
        &world.0,
        &active.focused,
        false,
    );
    if state.show_neighbors {
        for proxy in &active.neighbor_proxies {
            spawn_materialization(&mut commands, &mut meshes, &assets, &world.0, proxy, true);
        }
    }
    spawn_water(
        &mut commands,
        &assets,
        state.active_cell,
        world.0.atlas.cell_radius_m,
    );
    spawn_buildings(&mut commands, &assets, &active.focused);
    spawn_local_landmark(&mut commands, &assets, &world.0, &active);
    spawn_local_routes(&mut commands, &assets, &world.0, &active);
    spawn_hex_outline(
        &mut commands,
        &assets,
        state.active_cell,
        world.0.atlas.cell_radius_m,
        1.0,
        8.0,
        assets.cell_boundary.clone(),
        SpaceLayer::Local,
        Some(LocalVisibility::CellBoundary),
    );
    dirty.0 = false;
}

fn spawn_materialization(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &VisualAssets,
    _world: &RegionScaleWorld,
    materialization: &CellMaterialization,
    proxy: bool,
) {
    commands.spawn((
        Mesh3d(meshes.add(terrain_mesh(materialization, proxy))),
        MeshMaterial3d(if proxy {
            assets.terrain_proxy.clone()
        } else {
            assets.terrain_full.clone()
        }),
        Transform::IDENTITY,
        LocalDynamic,
        LocalVisual(if proxy {
            LocalVisibility::NeighborProxy
        } else {
            LocalVisibility::Always
        }),
        SpaceTag(SpaceLayer::Local),
    ));
}

fn spawn_water(commands: &mut Commands, assets: &VisualAssets, coord: HexCoord, radius: f64) {
    let center = coord.center_xz(radius);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.water.clone()),
        Transform::from_xyz(center.x as f32, -3.0, center.y as f32).with_scale(Vec3::new(
            (radius * 4.5) as f32,
            4.0,
            (radius * 4.5) as f32,
        )),
        LocalDynamic,
        LocalVisual(LocalVisibility::Always),
        SpaceTag(SpaceLayer::Local),
    ));
}

fn spawn_buildings(
    commands: &mut Commands,
    assets: &VisualAssets,
    materialization: &CellMaterialization,
) {
    for building in &materialization.buildings {
        spawn_grounded_building(commands, assets, building);
    }
}

fn spawn_grounded_building(
    commands: &mut Commands,
    assets: &VisualAssets,
    building: &GroundingResult,
) {
    let width = building.footprint_half_extents_m.x as f32 * 2.0;
    let depth = building.footprint_half_extents_m.y as f32 * 2.0;
    let foundation_height = (building.foundation_top_m - building.foundation_bottom_m) as f32;
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.foundation.clone()),
        Transform::from_xyz(
            building.grounded_center.x as f32,
            ((building.foundation_top_m + building.foundation_bottom_m) * 0.5) as f32,
            building.grounded_center.z as f32,
        )
        .with_scale(Vec3::new(
            width + 4.0,
            foundation_height.max(0.5),
            depth + 4.0,
        )),
        LocalDynamic,
        LocalVisual(LocalVisibility::Foundation),
        SpaceTag(SpaceLayer::Local),
    ));
    let body_center = building.grounded_center.as_vec3();
    let body_height = building.body_height_m as f32;
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.building.clone()),
        Transform::from_translation(body_center).with_scale(Vec3::new(width, body_height, depth)),
        LocalDynamic,
        LocalVisual(LocalVisibility::BuildingFull),
        SpaceTag(SpaceLayer::Local),
    ));
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.roof.clone()),
        Transform::from_xyz(
            body_center.x,
            building.foundation_top_m as f32 + body_height + 2.5,
            body_center.z,
        )
        .with_scale(Vec3::new(width + 6.0, 5.0, depth + 6.0)),
        LocalDynamic,
        LocalVisual(LocalVisibility::BuildingFull),
        SpaceTag(SpaceLayer::Local),
    ));
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.building_shell.clone()),
        Transform::from_translation(body_center).with_scale(Vec3::new(width, body_height, depth)),
        LocalDynamic,
        LocalVisual(LocalVisibility::BuildingShell),
        SpaceTag(SpaceLayer::Local),
    ));
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.building_massing.clone()),
        Transform::from_xyz(
            body_center.x,
            building.foundation_top_m as f32 + (body_height + 5.0) * 0.5,
            body_center.z,
        )
        .with_scale(Vec3::new(width + 6.0, body_height + 5.0, depth + 6.0)),
        LocalDynamic,
        LocalVisual(LocalVisibility::BuildingMassing),
        SpaceTag(SpaceLayer::Local),
    ));
}

fn spawn_local_landmark(
    commands: &mut Commands,
    assets: &VisualAssets,
    world: &RegionScaleWorld,
    active: &ActiveCellSet,
) {
    let visible_cells = std::iter::once(active.focused.coord)
        .chain(active.neighbor_proxies.iter().map(|proxy| proxy.coord))
        .collect::<Vec<_>>();
    if visible_cells.contains(&world.atlas.landmark_cell) {
        let base_y = terrain_height(
            world.atlas.world_seed,
            world.landmark_world.x,
            world.landmark_world.z,
        );
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.landmark.clone()),
            Transform::from_xyz(
                world.landmark_world.x as f32,
                (base_y + 54.0) as f32,
                world.landmark_world.z as f32,
            )
            .with_scale(Vec3::new(54.0, 108.0, 54.0)),
            LocalDynamic,
            LocalVisual(LocalVisibility::Always),
            SpaceTag(SpaceLayer::Local),
        ));
    }
}

fn spawn_local_routes(
    commands: &mut Commands,
    assets: &VisualAssets,
    world: &RegionScaleWorld,
    active: &ActiveCellSet,
) {
    let visible_cells = std::iter::once(active.focused.coord)
        .chain(active.neighbor_proxies.iter().map(|proxy| proxy.coord))
        .collect::<Vec<_>>();
    for (route_index, route) in world.routes.iter().enumerate() {
        for pair in route.points.windows(2) {
            if !visible_cells.contains(&pair[0].cell) && !visible_cells.contains(&pair[1].cell) {
                continue;
            }
            let bridge = pair[0].surface == RouteSurfaceKind::Bridge
                || pair[1].surface == RouteSurfaceKind::Bridge;
            spawn_segment(
                commands,
                assets,
                if bridge {
                    assets.bridge.clone()
                } else {
                    assets.route[route_index % 3].clone()
                },
                pair[0].world_position.as_vec3(),
                pair[1].world_position.as_vec3(),
                if bridge { 14.0 } else { 7.0 },
                if bridge { 2.5 } else { 1.2 },
                SpaceLayer::Local,
                Some(LocalVisibility::Route),
            );
        }
    }
}

fn update_visibility(
    state: Res<VisualState>,
    spaces: Query<(Entity, &SpaceTag, Option<&LocalVisual>)>,
    mut commands: Commands,
) {
    for (entity, space, local) in &spaces {
        let space_visible = match space.0 {
            SpaceLayer::Atlas => state.mode == AppMode::Atlas,
            SpaceLayer::Local => state.mode != AppMode::Atlas,
        };
        let local_visible = local.is_none_or(|tag| match tag.0 {
            LocalVisibility::Always => true,
            LocalVisibility::Route => state.show_routes,
            LocalVisibility::NeighborProxy => state.show_neighbors,
            LocalVisibility::CellBoundary => state.show_cell_boundary,
            LocalVisibility::Foundation => state.show_foundations,
            LocalVisibility::BuildingFull => state.building_lod == BuildingLod::Full,
            LocalVisibility::BuildingShell => state.building_lod == BuildingLod::Shell,
            LocalVisibility::BuildingMassing => state.building_lod == BuildingLod::Massing,
        });
        commands
            .entity(entity)
            .insert(if space_visible && local_visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            });
    }
}

fn update_atlas_materials(
    state: Res<VisualState>,
    world: Res<WorldData>,
    cells: Query<(&AtlasCellVisual, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (visual, handle) in &cells {
        if let Some(cell) = world.0.atlas.cell(visual.coord)
            && let Some(mut material) = materials.get_mut(&handle.0)
        {
            material.base_color = atlas_cell_color(cell, state.atlas_layer, &world.0.atlas.cells);
        }
    }
}

fn update_selection_marker(
    state: Res<VisualState>,
    world: Res<WorldData>,
    mut marker: Query<&mut Transform, With<AtlasSelectionMarker>>,
) {
    let cell = &world.0.atlas.cells[state.selected_cell_index];
    let center = cell.center_world * f64::from(ATLAS_SCALE);
    let mut transform = marker.single_mut().expect("one selection marker");
    transform.translation = Vec3::new(center.x as f32, 5.0, center.y as f32);
    transform.scale = Vec3::new(
        (world.0.atlas.cell_radius_m * 1.65 * f64::from(ATLAS_SCALE)) as f32,
        2.0,
        (world.0.atlas.cell_radius_m * 1.90 * f64::from(ATLAS_SCALE)) as f32,
    );
}

#[allow(clippy::too_many_arguments)]
fn update_camera_and_subject(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    world: Res<WorldData>,
    routes: Res<RouteRuntimes>,
    mut state: ResMut<VisualState>,
    mut dirty: ResMut<SceneDirty>,
    mut route_camera: ResMut<RouteCamera>,
    mut camera: Query<&mut Transform, (With<PrototypeCamera>, Without<RouteSubject>)>,
    mut subject: Query<&mut Transform, (With<RouteSubject>, Without<PrototypeCamera>)>,
) {
    let orbit_speed = 0.55;
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        state.overview_angle += orbit_speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        state.overview_angle -= orbit_speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        state.overview_height =
            (state.overview_height + 420.0 * time.delta_secs()).clamp(500.0, 3_600.0);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        state.overview_height =
            (state.overview_height - 420.0 * time.delta_secs()).clamp(500.0, 3_600.0);
    }
    let mut camera_transform = camera.single_mut().expect("one camera");
    match state.mode {
        AppMode::Atlas => {
            let selected = &world.0.atlas.cells[state.selected_cell_index];
            let target = Vec3::new(
                (selected.center_world.x * f64::from(ATLAS_SCALE)) as f32,
                0.0,
                (selected.center_world.y * f64::from(ATLAS_SCALE)) as f32,
            );
            *camera_transform =
                Transform::from_translation(target + Vec3::new(0.0, ATLAS_CAMERA_HEIGHT, 0.01))
                    .looking_at(target, Vec3::NEG_Z);
        }
        AppMode::LocalOverview => {
            let center = state.active_cell.center_xz(world.0.atlas.cell_radius_m);
            let target_y =
                terrain_height(world.0.atlas.world_seed, center.x, center.y) as f32 + 80.0;
            let target = Vec3::new(center.x as f32, target_y, center.y as f32);
            let position = target
                + Vec3::new(
                    state.overview_angle.sin() * state.overview_radius,
                    state.overview_height,
                    state.overview_angle.cos() * state.overview_radius,
                );
            *camera_transform = Transform::from_translation(position).looking_at(target, Vec3::Y);
        }
        AppMode::RouteTravel => {
            let runtime = &routes.0[state.route_index];
            if !state.route_paused {
                state.route_distance_m = (state.route_distance_m
                    + ROUTE_SPEED_MPS * f64::from(time.delta_secs()))
                .min(runtime.polyline.total_length());
            }
            let frame = runtime
                .polyline
                .sample(state.route_distance_m)
                .expect("route sample");
            let active_cell = world.0.atlas.nearest_cell(frame.position.xz());
            if active_cell != state.active_cell {
                state.active_cell = active_cell;
                dirty.0 = true;
            }
            let pose = route_camera
                .0
                .update(
                    frame.position,
                    frame.tangent,
                    ScrollGrammar::StandardSideView,
                    f64::from(time.delta_secs()),
                )
                .expect("camera update");
            *camera_transform = Transform::from_translation(pose.position.as_vec3())
                .looking_at(pose.target.as_vec3(), pose.up.as_vec3());
            let mut subject_transform = subject.single_mut().expect("one route subject");
            subject_transform.translation =
                frame.position.as_vec3() + Vec3::Y * (SUBJECT_HEIGHT_M * 0.5);
            subject_transform.rotation =
                Quat::from_rotation_y((-(frame.tangent.z as f32)).atan2(frame.tangent.x as f32));
        }
    }
}

fn update_window_title(
    state: Res<VisualState>,
    world: Res<WorldData>,
    mut windows: Query<&mut Window>,
) {
    let selected = &world.0.atlas.cells[state.selected_cell_index];
    let mut window = windows.single_mut().expect("one window");
    window.title = match state.mode {
        AppMode::Atlas => format!(
            "P4 corrected · ATLAS {:?} · selected ({},{}) · region {:.1} km · relief {:.0} m · tiles {}",
            state.atlas_layer,
            selected.coord.q,
            selected.coord.r,
            world.0.atlas.flat_to_flat_m / 1_000.0,
            selected.elevation.relief_m,
            selected.expected_tile_count,
        ),
        AppMode::LocalOverview => format!(
            "P4 corrected · LOCAL CELL ({},{}) · one full region + {} neighbor proxies · LOD {:?} · foundations {}",
            state.active_cell.q,
            state.active_cell.r,
            if state.show_neighbors { "up to 6" } else { "0" },
            state.building_lod,
            state.show_foundations,
        ),
        AppMode::RouteTravel => {
            let route = &world.0.routes[state.route_index];
            format!(
                "P4 corrected · ROUTE {} · {:.0} m · active cell ({},{}) · crossed {} cells · target {}",
                state.route_index + 1,
                state.route_distance_m,
                state.active_cell.q,
                state.active_cell.r,
                route.crossed_cells.len(),
                route.target_landmark_id,
            )
        }
    };
}

fn atlas_cell_color(cell: &AtlasCellRegion, layer: AtlasLayer, all: &[AtlasCellRegion]) -> Color {
    match layer {
        AtlasLayer::Elevation => {
            let minimum = all
                .iter()
                .map(|entry| entry.elevation.minimum_m)
                .fold(f64::INFINITY, f64::min);
            let maximum = all
                .iter()
                .map(|entry| entry.elevation.maximum_m)
                .fold(f64::NEG_INFINITY, f64::max);
            let normalized = ((cell.elevation.mean_m - minimum) / (maximum - minimum).max(1.0))
                .clamp(0.0, 1.0) as f32;
            if cell.elevation.maximum_m <= 0.0 {
                Color::srgb(0.10, 0.34, 0.66)
            } else {
                Color::srgb(
                    0.18 + normalized * 0.55,
                    0.55 - normalized * 0.25,
                    0.22 + normalized * 0.12,
                )
            }
        }
        AtlasLayer::Landforms => {
            let dominant = cell
                .landforms
                .fractions
                .iter()
                .max_by(|left, right| left.1.total_cmp(right.1))
                .map(|(landform, _)| *landform)
                .unwrap_or(LandformClass::Lowland);
            landform_color(dominant)
        }
        AtlasLayer::Capacity => {
            let value = cell.carrying_capacity.clamp(0.0, 1.7) as f32 / 1.7;
            Color::srgb(0.20 + value * 0.65, 0.18 + value * 0.55, 0.16)
        }
    }
}

fn landform_color(landform: LandformClass) -> Color {
    match landform {
        LandformClass::Ocean => Color::srgb(0.07, 0.30, 0.63),
        LandformClass::Coast => Color::srgb(0.86, 0.70, 0.38),
        LandformClass::Estuary => Color::srgb(0.18, 0.55, 0.68),
        LandformClass::Floodplain => Color::srgb(0.48, 0.72, 0.30),
        LandformClass::Valley => Color::srgb(0.36, 0.62, 0.28),
        LandformClass::Lowland => Color::srgb(0.50, 0.66, 0.30),
        LandformClass::Hillslope => Color::srgb(0.55, 0.45, 0.26),
        LandformClass::Ridge => Color::srgb(0.46, 0.38, 0.32),
        LandformClass::Mountain => Color::srgb(0.72, 0.72, 0.70),
    }
}

fn terrain_mesh(materialization: &CellMaterialization, proxy: bool) -> Mesh {
    let resolution = usize::from(materialization.resolution);
    let mut vertex_map = vec![None; materialization.samples.len()];
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut colors = Vec::<[f32; 4]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    for z in 0..resolution {
        for x in 0..resolution {
            let index = materialization.sample_index(x, z);
            let sample = &materialization.samples[index];
            if !sample.inside_hex {
                continue;
            }
            vertex_map[index] = Some(positions.len() as u32);
            positions.push(sample.world_position.as_vec3().to_array());
            normals.push(terrain_normal(materialization, x, z).to_array());
            let color = if proxy {
                Color::srgb(0.52, 0.56, 0.48)
            } else {
                landform_color(sample.landform)
            };
            colors.push(color.to_linear().to_f32_array());
            uvs.push([
                x as f32 / (resolution - 1) as f32,
                z as f32 / (resolution - 1) as f32,
            ]);
        }
    }
    let mut indices = Vec::<u32>::new();
    for z in 0..resolution - 1 {
        for x in 0..resolution - 1 {
            let a = vertex_map[materialization.sample_index(x, z)];
            let b = vertex_map[materialization.sample_index(x + 1, z)];
            let c = vertex_map[materialization.sample_index(x, z + 1)];
            let d = vertex_map[materialization.sample_index(x + 1, z + 1)];
            if let (Some(a), Some(b), Some(c), Some(d)) = (a, b, c, d) {
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

fn terrain_normal(materialization: &CellMaterialization, x: usize, z: usize) -> Vec3 {
    let resolution = usize::from(materialization.resolution);
    let left = materialization
        .sample(x.saturating_sub(1), z)
        .map(|sample| sample.world_position.y)
        .unwrap_or(
            materialization.samples[materialization.sample_index(x, z)]
                .world_position
                .y,
        );
    let right = materialization
        .sample((x + 1).min(resolution - 1), z)
        .map(|sample| sample.world_position.y)
        .unwrap_or(left);
    let down = materialization
        .sample(x, z.saturating_sub(1))
        .map(|sample| sample.world_position.y)
        .unwrap_or(left);
    let up = materialization
        .sample(x, (z + 1).min(resolution - 1))
        .map(|sample| sample.world_position.y)
        .unwrap_or(down);
    let step_x = (materialization.bounds_max.x - materialization.bounds_min.x)
        / f64::from(materialization.resolution - 1);
    let step_z = (materialization.bounds_max.y - materialization.bounds_min.y)
        / f64::from(materialization.resolution - 1);
    Vec3::new(
        ((left - right) / (2.0 * step_x)) as f32,
        1.0,
        ((down - up) / (2.0 * step_z)) as f32,
    )
    .normalize_or_zero()
}

fn flat_hex_mesh(coord: HexCoord, radius: f64) -> Mesh {
    let center = coord.center_xz(radius);
    let mut positions = vec![[center.x as f32, 0.0, center.y as f32]];
    let mut normals = vec![[0.0, 1.0, 0.0]];
    let mut uvs = vec![[0.5, 0.5]];
    for corner in 0..6 {
        let angle = (30.0 + corner as f64 * 60.0).to_radians();
        positions.push([
            (center.x + radius * angle.cos()) as f32,
            0.0,
            (center.y + radius * angle.sin()) as f32,
        ]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([
            0.5 + angle.cos() as f32 * 0.5,
            0.5 + angle.sin() as f32 * 0.5,
        ]);
    }
    let mut indices = Vec::with_capacity(18);
    for corner in 0..6_u32 {
        indices.extend_from_slice(&[0, (corner + 1) % 6 + 1, corner + 1]);
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

#[allow(clippy::too_many_arguments)]
fn spawn_hex_outline(
    commands: &mut Commands,
    assets: &VisualAssets,
    coord: HexCoord,
    radius: f64,
    scale: f32,
    width: f32,
    material: Handle<StandardMaterial>,
    space: SpaceLayer,
    local_kind: Option<LocalVisibility>,
) {
    let center = coord.center_xz(radius);
    let vertices = (0..6)
        .map(|corner| {
            let angle = (30.0 + corner as f64 * 60.0).to_radians();
            let x = center.x + radius * angle.cos();
            let z = center.y + radius * angle.sin();
            let y = if space == SpaceLayer::Local {
                terrain_height(region_scale_core::WORLD_SEED, x, z) + 8.0
            } else {
                3.0
            };
            Vec3::new(x as f32 * scale, y as f32, z as f32 * scale)
        })
        .collect::<Vec<_>>();
    for index in 0..6 {
        let mut entity = commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material.clone()),
            segment_transform(vertices[index], vertices[(index + 1) % 6], width, 1.2),
            SpaceTag(space),
        ));
        if let Some(kind) = local_kind {
            entity.insert((LocalDynamic, LocalVisual(kind)));
        }
    }
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
    space: SpaceLayer,
    local_kind: Option<LocalVisibility>,
) {
    let mut entity = commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        segment_transform(start, end, width, height),
        SpaceTag(space),
    ));
    if let Some(kind) = local_kind {
        entity.insert((LocalDynamic, LocalVisual(kind)));
    }
}

fn segment_transform(start: Vec3, end: Vec3, width: f32, height: f32) -> Transform {
    let delta = end - start;
    let length = delta.length().max(0.01);
    let midpoint = (start + end) * 0.5;
    Transform::from_translation(midpoint)
        .with_rotation(Quat::from_rotation_arc(Vec3::X, delta / length))
        .with_scale(Vec3::new(length, height, width))
}
