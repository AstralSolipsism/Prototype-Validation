#![forbid(unsafe_code)]

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use glam::Vec3Swizzles;
use integrated_world_core::{
    AtlasCellSpec, AtlasFeatureKind, CompiledScrollRoute, HistoricalAssetKind, IntegratedWorld,
    LandCover, LandUseKind, TerrainGrid, detailed_height,
};
use p4_integrated_scenario::generate_integrated_baseline;
use scroll_camera_core::{CameraRigConfig, CameraRigState, PolylineRoute, ScrollGrammar, ViewSide};
use std::collections::BTreeMap;
use world_generation_core::HexCoord;
use world_ids::EntityId;

const ATLAS_SCALE: f32 = 0.58;
const ATLAS_HEIGHT: f32 = 610.0;
const LOCAL_ROUTE_SPEED_MPS: f64 = 30.0;
const SUBJECT_HEIGHT_M: f32 = 5.5;
const MAP_Y: f32 = 0.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "P4 integrated Atlas-to-scroll world · final GPU review pending".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.46, 0.63, 0.77)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 900.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                control_app,
                update_visual_visibility,
                update_atlas_cell_materials,
                update_selection_marker,
                update_camera_and_route_subject,
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
    Climate,
    Resources,
    History,
}

impl AtlasLayer {
    const fn next(self) -> Self {
        match self {
            Self::Elevation => Self::Landforms,
            Self::Landforms => Self::Climate,
            Self::Climate => Self::Resources,
            Self::Resources => Self::History,
            Self::History => Self::Elevation,
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
enum VisualSpace {
    Atlas,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VisualKind {
    Always,
    AtlasBoundary,
    AtlasContour,
    AtlasHistory,
    LocalHistory,
    LocalRoute,
    RouteSubject,
    BuildingFull,
    BuildingShell,
    BuildingMassing,
}

#[derive(Component, Clone, Copy, Debug)]
struct VisualTag {
    space: VisualSpace,
    kind: VisualKind,
}

#[derive(Component)]
struct PrototypeCamera;

#[derive(Component)]
struct AtlasSelectionMarker;

#[derive(Component)]
struct RouteSubject;

#[derive(Component, Clone, Copy)]
struct AtlasCellVisual {
    coord: HexCoord,
}

#[derive(Resource)]
struct WorldData(IntegratedWorld);

#[derive(Resource)]
struct VisualState {
    mode: AppMode,
    atlas_layer: AtlasLayer,
    selected_cell_index: usize,
    show_atlas_boundaries: bool,
    show_contours: bool,
    show_history: bool,
    show_routes: bool,
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
struct RouteRuntimes(Vec<RouteRuntime>);

struct RouteRuntime {
    polyline: PolylineRoute,
    grammars: Vec<ScrollGrammar>,
}

#[derive(Resource)]
struct RouteCamera(CameraRigState);

#[derive(Resource, Clone)]
struct VisualAssets {
    cube: Handle<Mesh>,
    atlas_boundary: Handle<StandardMaterial>,
    contour_minor: Handle<StandardMaterial>,
    contour_major: Handle<StandardMaterial>,
    ridge: Handle<StandardMaterial>,
    river: Handle<StandardMaterial>,
    coast: Handle<StandardMaterial>,
    road: Handle<StandardMaterial>,
    route: [Handle<StandardMaterial>; 3],
    selection: Handle<StandardMaterial>,
    terrain: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    old_town: Handle<StandardMaterial>,
    new_town: Handle<StandardMaterial>,
    farmland: Handle<StandardMaterial>,
    ruins: Handle<StandardMaterial>,
    harbor: Handle<StandardMaterial>,
    fortification: Handle<StandardMaterial>,
    monument: Handle<StandardMaterial>,
    building: Handle<StandardMaterial>,
    building_shell: Handle<StandardMaterial>,
    building_massing: Handle<StandardMaterial>,
    roof: Handle<StandardMaterial>,
    landmark: Handle<StandardMaterial>,
    subject: Handle<StandardMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world = generate_integrated_baseline().expect("integrated P4 fixture must compile");
    let selected_cell_index = world
        .atlas
        .cells
        .iter()
        .position(|cell| cell.id.as_u128() == world.atlas.cells[0].id.as_u128())
        .unwrap_or(0);
    let routes = world
        .traversal
        .routes
        .iter()
        .map(route_runtime)
        .collect::<Vec<_>>();
    let initial_tangent = routes
        .first()
        .and_then(|runtime| runtime.polyline.sample(0.0).ok())
        .map(|frame| frame.tangent)
        .unwrap_or(glam::DVec3::X);
    let route_camera = CameraRigState::new(
        CameraRigConfig {
            side_distance: 28.0,
            height: 15.0,
            look_ahead: 8.0,
            look_height: 3.0,
            trailing_offset: 2.0,
            heading_half_life_seconds: 0.72,
            composition_half_life_seconds: 0.58,
        },
        initial_tangent,
        ViewSide::Left,
    )
    .expect("route camera configuration");

    let assets = VisualAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        atlas_boundary: material(&mut materials, Color::srgb(0.11, 0.13, 0.16), 0.92),
        contour_minor: material(&mut materials, Color::srgb(0.21, 0.18, 0.15), 0.95),
        contour_major: material(&mut materials, Color::srgb(0.08, 0.07, 0.06), 0.90),
        ridge: material(&mut materials, Color::srgb(0.45, 0.36, 0.29), 0.92),
        river: material(&mut materials, Color::srgb(0.12, 0.43, 0.82), 0.55),
        coast: material(&mut materials, Color::srgb(0.88, 0.72, 0.40), 0.88),
        road: material(&mut materials, Color::srgb(0.38, 0.22, 0.12), 0.94),
        route: [
            material(&mut materials, Color::srgb(0.93, 0.31, 0.20), 0.78),
            material(&mut materials, Color::srgb(0.27, 0.86, 0.38), 0.78),
            material(&mut materials, Color::srgb(0.75, 0.32, 0.92), 0.78),
        ],
        selection: material(&mut materials, Color::srgb(1.0, 0.84, 0.18), 0.45),
        terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            ..default()
        }),
        water: material(&mut materials, Color::srgb(0.08, 0.38, 0.68), 0.42),
        old_town: material(&mut materials, Color::srgb(0.46, 0.25, 0.13), 0.95),
        new_town: material(&mut materials, Color::srgb(0.72, 0.45, 0.22), 0.90),
        farmland: material(&mut materials, Color::srgb(0.61, 0.72, 0.25), 0.98),
        ruins: material(&mut materials, Color::srgb(0.35, 0.34, 0.34), 0.98),
        harbor: material(&mut materials, Color::srgb(0.27, 0.18, 0.12), 0.92),
        fortification: material(&mut materials, Color::srgb(0.43, 0.42, 0.39), 0.94),
        monument: material(&mut materials, Color::srgb(0.78, 0.76, 0.68), 0.82),
        building: material(&mut materials, Color::srgb(0.69, 0.49, 0.28), 0.90),
        building_shell: material(&mut materials, Color::srgb(0.37, 0.57, 0.69), 0.84),
        building_massing: material(&mut materials, Color::srgb(0.32, 0.42, 0.62), 0.76),
        roof: material(&mut materials, Color::srgb(0.31, 0.10, 0.08), 0.90),
        landmark: material(&mut materials, Color::srgb(0.84, 0.79, 0.62), 0.78),
        subject: material(&mut materials, Color::srgb(0.93, 0.08, 0.06), 0.62),
    };

    spawn_atlas(&mut commands, &mut meshes, &mut materials, &assets, &world);
    spawn_local_world(&mut commands, &mut meshes, &assets, &world);

    commands.spawn((Camera3d::default(), Transform::IDENTITY, PrototypeCamera));
    commands.spawn((
        DirectionalLight {
            illuminance: 24_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.55, 0.0)),
    ));

    commands.insert_resource(WorldData(world));
    commands.insert_resource(RouteRuntimes(routes));
    commands.insert_resource(RouteCamera(route_camera));
    commands.insert_resource(assets);
    commands.insert_resource(VisualState {
        mode: AppMode::Atlas,
        atlas_layer: AtlasLayer::Elevation,
        selected_cell_index,
        show_atlas_boundaries: true,
        show_contours: true,
        show_history: true,
        show_routes: true,
        building_lod: BuildingLod::Full,
        route_index: 0,
        route_distance_m: 0.0,
        route_paused: true,
        view_side: ViewSide::Left,
        overview_angle: 0.75,
        overview_height: 280.0,
        overview_radius: 430.0,
    });

    info!(
        "Controls: M Atlas/local; Tab layer; Q/E select cell; Enter local; 1/2/3 route; Space pause; V side; L building LOD; B history; C contours; H boundaries; T routes; A/D orbit; Up/Down height; R reset"
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
    world: &IntegratedWorld,
) {
    for cell in &world.atlas.cells {
        let unique_material = materials.add(StandardMaterial {
            base_color: atlas_cell_color(cell, AtlasLayer::Elevation, &world.atlas.cells),
            perceptual_roughness: 0.94,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(flat_hex_mesh(cell.coord, world.atlas.cell_radius_m))),
            MeshMaterial3d(unique_material),
            Transform::from_scale(Vec3::new(ATLAS_SCALE, 1.0, ATLAS_SCALE)),
            AtlasCellVisual { coord: cell.coord },
            VisualTag {
                space: VisualSpace::Atlas,
                kind: VisualKind::Always,
            },
        ));
        spawn_flat_hex_outline(
            commands,
            assets,
            cell.coord,
            world.atlas.cell_radius_m,
            1.4,
            assets.atlas_boundary.clone(),
            VisualKind::AtlasBoundary,
        );
    }

    let (minimum, maximum) = atlas_height_range(&world.atlas.cells);
    let contour_start = (minimum / 20.0).floor() as i32;
    let contour_end = (maximum / 20.0).ceil() as i32;
    for step in contour_start..=contour_end {
        let level = f64::from(step) * 20.0;
        let major = step.rem_euclid(5) == 0;
        let contour_material = if major {
            assets.contour_major.clone()
        } else {
            assets.contour_minor.clone()
        };
        let width = if major { 2.1 } else { 1.05 };
        for (start, end) in contour_segments(world, level) {
            spawn_segment(
                commands,
                assets,
                contour_material.clone(),
                map_point(start, 2.2),
                map_point(end, 2.2),
                width,
                0.8,
                VisualTag {
                    space: VisualSpace::Atlas,
                    kind: VisualKind::AtlasContour,
                },
            );
        }
    }

    for feature in &world.atlas.features {
        let (material, width, height, kind) = match feature.kind {
            AtlasFeatureKind::MountainRange | AtlasFeatureKind::RidgeLine => {
                (assets.ridge.clone(), 5.0, 2.4, VisualKind::Always)
            }
            AtlasFeatureKind::ValleyLine => {
                (assets.contour_minor.clone(), 3.0, 1.4, VisualKind::Always)
            }
            AtlasFeatureKind::River => (assets.river.clone(), 7.0, 1.6, VisualKind::Always),
            AtlasFeatureKind::Coastline => (assets.coast.clone(), 5.5, 1.8, VisualKind::Always),
            AtlasFeatureKind::RoadCorridor => (assets.road.clone(), 4.0, 1.4, VisualKind::Always),
            AtlasFeatureKind::Settlement | AtlasFeatureKind::Landmark => continue,
        };
        for pair in feature.path_world.windows(2) {
            spawn_segment(
                commands,
                assets,
                material.clone(),
                map_point(pair[0].xz(), height),
                map_point(pair[1].xz(), height),
                width,
                0.9,
                VisualTag {
                    space: VisualSpace::Atlas,
                    kind,
                },
            );
        }
    }

    for zone in &world.history.land_use {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material_for_land_use(assets, zone.kind)),
            Transform::from_translation(map_point(zone.center_world.xz(), 3.1)).with_scale(
                Vec3::new(
                    (zone.radius_m as f32 * 1.35 * ATLAS_SCALE).max(8.0),
                    1.2,
                    (zone.radius_m as f32 * 1.35 * ATLAS_SCALE).max(8.0),
                ),
            ),
            VisualTag {
                space: VisualSpace::Atlas,
                kind: VisualKind::AtlasHistory,
            },
        ));
    }

    if let Some(settlement) = world
        .atlas
        .features
        .iter()
        .find(|feature| feature.kind == AtlasFeatureKind::Settlement)
        .and_then(|feature| feature.path_world.first())
    {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.old_town.clone()),
            Transform::from_translation(map_point(settlement.xz(), 8.0))
                .with_scale(Vec3::new(18.0, 12.0, 18.0)),
            VisualTag {
                space: VisualSpace::Atlas,
                kind: VisualKind::Always,
            },
        ));
    }
    if let Some(landmark) = world
        .atlas
        .features
        .iter()
        .find(|feature| feature.kind == AtlasFeatureKind::Landmark)
        .and_then(|feature| feature.path_world.first())
    {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.landmark.clone()),
            Transform::from_translation(map_point(landmark.xz(), 18.0))
                .with_scale(Vec3::new(9.0, 34.0, 9.0)),
            VisualTag {
                space: VisualSpace::Atlas,
                kind: VisualKind::Always,
            },
        ));
    }

    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.selection.clone()),
        Transform::IDENTITY,
        AtlasSelectionMarker,
        VisualTag {
            space: VisualSpace::Atlas,
            kind: VisualKind::Always,
        },
    ));
}

fn spawn_local_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &VisualAssets,
    world: &IntegratedWorld,
) {
    commands.spawn((
        Mesh3d(meshes.add(detailed_terrain_mesh(&world.detailed.terrain))),
        MeshMaterial3d(assets.terrain.clone()),
        Transform::IDENTITY,
        VisualTag {
            space: VisualSpace::Local,
            kind: VisualKind::Always,
        },
    ));

    let (minimum, maximum) = detailed_bounds(&world.detailed.terrain);
    let size = maximum - minimum + Vec2::splat(120.0);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.water.clone()),
        Transform::from_xyz(
            (minimum.x + maximum.x) * 0.5,
            -1.2,
            (minimum.y + maximum.y) * 0.5,
        )
        .with_scale(Vec3::new(size.x, 1.4, size.y)),
        VisualTag {
            space: VisualSpace::Local,
            kind: VisualKind::Always,
        },
    ));

    for feature in &world.atlas.features {
        let (material, width) = match feature.kind {
            AtlasFeatureKind::MountainRange | AtlasFeatureKind::RidgeLine => {
                (assets.ridge.clone(), 4.2)
            }
            AtlasFeatureKind::River => (assets.river.clone(), 12.0),
            AtlasFeatureKind::Coastline => (assets.coast.clone(), 5.0),
            AtlasFeatureKind::RoadCorridor => (assets.road.clone(), 7.0),
            _ => continue,
        };
        for pair in feature.path_world.windows(2) {
            spawn_segment(
                commands,
                assets,
                material.clone(),
                pair[0].as_vec3() + Vec3::Y * 1.5,
                pair[1].as_vec3() + Vec3::Y * 1.5,
                width,
                1.0,
                VisualTag {
                    space: VisualSpace::Local,
                    kind: VisualKind::Always,
                },
            );
        }
    }

    for zone in &world.history.land_use {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material_for_land_use(assets, zone.kind)),
            Transform::from_translation(zone.center_world.as_vec3() + Vec3::Y * 0.8).with_scale(
                Vec3::new(
                    zone.radius_m as f32 * 1.45,
                    1.1,
                    zone.radius_m as f32 * 1.45,
                ),
            ),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::LocalHistory,
            },
        ));
    }

    for asset in &world.history.assets {
        let (material, scale) = match asset.kind {
            HistoricalAssetKind::Harbor => (
                assets.harbor.clone(),
                Vec3::new(asset.extent_m.x as f32, 3.0, asset.extent_m.y as f32),
            ),
            HistoricalAssetKind::Bridge => (
                assets.road.clone(),
                Vec3::new(asset.extent_m.x as f32, 2.2, asset.extent_m.y as f32),
            ),
            HistoricalAssetKind::Farmland => (
                assets.farmland.clone(),
                Vec3::new(asset.extent_m.x as f32, 1.2, asset.extent_m.y as f32),
            ),
            HistoricalAssetKind::Fortification => (
                assets.fortification.clone(),
                Vec3::new(asset.extent_m.x as f32, 10.0, asset.extent_m.y as f32),
            ),
            HistoricalAssetKind::Ruins => (
                assets.ruins.clone(),
                Vec3::new(asset.extent_m.x as f32, 5.0, asset.extent_m.y as f32),
            ),
            HistoricalAssetKind::Monument => (assets.monument.clone(), Vec3::new(5.0, 18.0, 5.0)),
            HistoricalAssetKind::OldRoad | HistoricalAssetKind::NewRoad => (
                assets.road.clone(),
                Vec3::new(asset.extent_m.x as f32, 1.0, asset.extent_m.y as f32),
            ),
            HistoricalAssetKind::OldQuarter | HistoricalAssetKind::NewQuarter => continue,
        };
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(asset.anchor_world.as_vec3() + Vec3::Y * (scale.y * 0.5))
                .with_scale(scale),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::LocalHistory,
            },
        ));
    }

    spawn_building_lods(commands, assets, world);

    if let Some(landmark) = world
        .atlas
        .features
        .iter()
        .find(|feature| feature.kind == AtlasFeatureKind::Landmark)
        .and_then(|feature| feature.path_world.first())
    {
        let base = landmark.as_vec3();
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.landmark.clone()),
            Transform::from_translation(base + Vec3::Y * 42.0)
                .with_scale(Vec3::new(18.0, 84.0, 18.0)),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::Always,
            },
        ));
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.roof.clone()),
            Transform::from_translation(base + Vec3::Y * 88.0)
                .with_scale(Vec3::new(26.0, 10.0, 26.0)),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::Always,
            },
        ));
    }

    for (route_index, route) in world.traversal.routes.iter().enumerate() {
        for pair in route.world_path.windows(2) {
            spawn_segment(
                commands,
                assets,
                assets.route[route_index % 3].clone(),
                pair[0].as_vec3() + Vec3::Y * 2.0,
                pair[1].as_vec3() + Vec3::Y * 2.0,
                3.5,
                0.8,
                VisualTag {
                    space: VisualSpace::Local,
                    kind: VisualKind::LocalRoute,
                },
            );
        }
    }

    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.subject.clone()),
        Transform::IDENTITY.with_scale(Vec3::new(3.5, SUBJECT_HEIGHT_M, 3.5)),
        RouteSubject,
        VisualTag {
            space: VisualSpace::Local,
            kind: VisualKind::RouteSubject,
        },
    ));
}

fn spawn_building_lods(commands: &mut Commands, assets: &VisualAssets, world: &IntegratedWorld) {
    let old_town = world
        .history
        .land_use
        .iter()
        .find(|zone| zone.kind == LandUseKind::OldTown)
        .map(|zone| zone.center_world)
        .unwrap_or(glam::DVec3::ZERO);
    let binding_by_id = world
        .report
        .bindings
        .iter()
        .map(|binding| (binding.object_id, binding.local_anchor))
        .collect::<BTreeMap<_, _>>();

    for (index, instance_id) in world.building_instances.iter().enumerate() {
        let entity_id = EntityId::from_u128(instance_id.as_u128());
        let local = binding_by_id
            .get(&entity_id)
            .copied()
            .unwrap_or(glam::DVec3::new(
                (index % 3) as f64 * 24.0 - 24.0,
                0.0,
                (index / 3) as f64 * 28.0 - 14.0,
            ));
        let root = old_town + local;
        let width = 13.0 + (index % 3) as f32 * 2.2;
        let depth = 10.0 + (index % 2) as f32 * 2.0;
        let height = 10.0 + (index % 2) as f32 * 5.0;

        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.building.clone()),
            Transform::from_translation(root.as_vec3() + Vec3::Y * (height * 0.5))
                .with_scale(Vec3::new(width, height, depth)),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::BuildingFull,
            },
        ));
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.roof.clone()),
            Transform::from_translation(root.as_vec3() + Vec3::Y * (height + 1.5))
                .with_scale(Vec3::new(width + 2.0, 3.0, depth + 2.0)),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::BuildingFull,
            },
        ));
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.building_shell.clone()),
            Transform::from_translation(root.as_vec3() + Vec3::Y * (height * 0.5))
                .with_scale(Vec3::new(width, height, depth)),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::BuildingShell,
            },
        ));
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.building_massing.clone()),
            Transform::from_translation(root.as_vec3() + Vec3::Y * ((height + 3.0) * 0.5))
                .with_scale(Vec3::new(width + 2.0, height + 3.0, depth + 2.0)),
            VisualTag {
                space: VisualSpace::Local,
                kind: VisualKind::BuildingMassing,
            },
        ));
    }
}

fn material_for_land_use(assets: &VisualAssets, kind: LandUseKind) -> Handle<StandardMaterial> {
    match kind {
        LandUseKind::Harbor => assets.harbor.clone(),
        LandUseKind::OldTown => assets.old_town.clone(),
        LandUseKind::NewTown => assets.new_town.clone(),
        LandUseKind::Farmland => assets.farmland.clone(),
        LandUseKind::Fortification => assets.fortification.clone(),
        LandUseKind::Ruins => assets.ruins.clone(),
        LandUseKind::Commons => assets.monument.clone(),
    }
}

fn control_app(
    keyboard: Res<ButtonInput<KeyCode>>,
    world: Res<WorldData>,
    routes: Res<RouteRuntimes>,
    mut state: ResMut<VisualState>,
    mut route_camera: ResMut<RouteCamera>,
) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        state.mode = match state.mode {
            AppMode::Atlas => AppMode::LocalOverview,
            AppMode::LocalOverview | AppMode::RouteTravel => AppMode::Atlas,
        };
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
        let selected = world.0.atlas.cells[state.selected_cell_index].coord;
        if world.0.detailed.materialized_cells.contains(&selected) {
            state.mode = AppMode::LocalOverview;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.show_contours = !state.show_contours;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        state.show_atlas_boundaries = !state.show_atlas_boundaries;
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        state.show_history = !state.show_history;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        state.show_routes = !state.show_routes;
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
            let tangent = routes.0[route_index]
                .polyline
                .sample(0.0)
                .map(|frame| frame.tangent)
                .unwrap_or(glam::DVec3::X);
            route_camera.0 = CameraRigState::new(
                CameraRigConfig {
                    side_distance: 28.0,
                    height: 15.0,
                    look_ahead: 8.0,
                    look_height: 3.0,
                    trailing_offset: 2.0,
                    heading_half_life_seconds: 0.72,
                    composition_half_life_seconds: 0.58,
                },
                tangent,
                state.view_side,
            )
            .expect("route camera");
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.mode = AppMode::Atlas;
        state.atlas_layer = AtlasLayer::Elevation;
        state.route_index = 0;
        state.route_distance_m = 0.0;
        state.route_paused = true;
        state.view_side = ViewSide::Left;
        state.overview_angle = 0.75;
        state.overview_height = 280.0;
        route_camera.0.set_view_side(ViewSide::Left);
    }
}

fn update_visual_visibility(
    state: Res<VisualState>,
    visuals: Query<(Entity, &VisualTag)>,
    mut commands: Commands,
) {
    for (entity, tag) in &visuals {
        let space_visible = match tag.space {
            VisualSpace::Atlas => state.mode == AppMode::Atlas,
            VisualSpace::Local => state.mode != AppMode::Atlas,
        };
        let kind_visible = match tag.kind {
            VisualKind::Always => true,
            VisualKind::AtlasBoundary => state.show_atlas_boundaries,
            VisualKind::AtlasContour => state.show_contours,
            VisualKind::AtlasHistory | VisualKind::LocalHistory => state.show_history,
            VisualKind::LocalRoute => state.show_routes,
            VisualKind::RouteSubject => state.mode == AppMode::RouteTravel,
            VisualKind::BuildingFull => state.building_lod == BuildingLod::Full,
            VisualKind::BuildingShell => state.building_lod == BuildingLod::Shell,
            VisualKind::BuildingMassing => state.building_lod == BuildingLod::Massing,
        };
        commands
            .entity(entity)
            .insert(if space_visible && kind_visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            });
    }
}
fn update_atlas_cell_materials(
    state: Res<VisualState>,
    world: Res<WorldData>,
    cells: Query<(&AtlasCellVisual, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (cell_visual, material_handle) in &cells {
        if let Some(cell) = world.0.atlas.cell(cell_visual.coord)
            && let Some(mut material) = materials.get_mut(&material_handle.0)
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
    let center = cell.coord.center_xz(world.0.atlas.cell_radius_m) * f64::from(ATLAS_SCALE);
    let mut transform = marker.single_mut().expect("one Atlas selection marker");
    transform.translation = Vec3::new(center.x as f32, 4.8, center.y as f32);
    transform.scale = Vec3::new(
        world.0.atlas.cell_radius_m as f32 * 1.30 * ATLAS_SCALE,
        1.8,
        world.0.atlas.cell_radius_m as f32 * 1.10 * ATLAS_SCALE,
    );
}

#[allow(clippy::too_many_arguments)]
// Bevy injects these resources and queries as independent system parameters.
#[allow(clippy::too_many_arguments)]
// Bevy injects these resources and queries as independent system parameters.
#[allow(clippy::too_many_arguments)]
// Bevy injects these resources and queries as independent system parameters.
fn update_camera_and_route_subject(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    world: Res<WorldData>,
    routes: Res<RouteRuntimes>,
    mut state: ResMut<VisualState>,
    mut route_camera: ResMut<RouteCamera>,
    mut camera: Query<&mut Transform, (With<PrototypeCamera>, Without<RouteSubject>)>,
    mut subject: Query<&mut Transform, (With<RouteSubject>, Without<PrototypeCamera>)>,
) {
    let orbit_speed = 0.65;
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        state.overview_angle += orbit_speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        state.overview_angle -= orbit_speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        state.overview_height =
            (state.overview_height + 90.0 * time.delta_secs()).clamp(80.0, 520.0);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        state.overview_height =
            (state.overview_height - 90.0 * time.delta_secs()).clamp(80.0, 520.0);
    }

    let selected = &world.0.atlas.cells[state.selected_cell_index];
    let selected_world = selected.coord.center_xz(world.0.atlas.cell_radius_m);
    let mut camera_transform = camera.single_mut().expect("one camera");
    match state.mode {
        AppMode::Atlas => {
            let target = Vec3::new(
                selected_world.x as f32 * ATLAS_SCALE,
                MAP_Y,
                selected_world.y as f32 * ATLAS_SCALE,
            );
            *camera_transform =
                Transform::from_translation(target + Vec3::new(0.0, ATLAS_HEIGHT, 0.01))
                    .looking_at(target, Vec3::NEG_Z);
        }
        AppMode::LocalOverview => {
            let target_y = detailed_height(
                world.0.atlas.world_seed,
                world.0.atlas.cell_radius_m,
                selected_world,
            ) as f32;
            let target = Vec3::new(
                selected_world.x as f32,
                target_y + 18.0,
                selected_world.y as f32,
            );
            let position = target
                + Vec3::new(
                    state.overview_angle.sin() * state.overview_radius,
                    state.overview_height,
                    state.overview_angle.cos() * state.overview_radius,
                );
            *camera_transform = Transform::from_translation(position).looking_at(target, Vec3::Y);
        }
        AppMode::RouteTravel => {
            let route = &routes.0[state.route_index];
            if !state.route_paused {
                state.route_distance_m = (state.route_distance_m
                    + LOCAL_ROUTE_SPEED_MPS * f64::from(time.delta_secs()))
                .min(route.polyline.total_length());
            }
            let frame = route
                .polyline
                .sample(state.route_distance_m)
                .expect("route sample");
            let grammar = route
                .grammars
                .get(frame.segment_index)
                .copied()
                .unwrap_or(ScrollGrammar::StandardSideView);
            let pose = route_camera
                .0
                .update(
                    frame.position,
                    frame.tangent,
                    grammar,
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
    let cell = &world.0.atlas.cells[state.selected_cell_index];
    let materialized = world.0.detailed.materialized_cells.contains(&cell.coord);
    let mut window = windows.single_mut().expect("one window");
    window.title = match state.mode {
        AppMode::Atlas => format!(
            "P4 integrated · ATLAS {:?} · cell ({},{}) · {:.0}–{:.0} m relief {:.0} · buildable {:.0}% · pop {} · {} · materialized {}",
            state.atlas_layer,
            cell.coord.q,
            cell.coord.r,
            cell.elevation.minimum_m,
            cell.elevation.maximum_m,
            cell.elevation.relief_m,
            cell.elevation.buildable_fraction * 100.0,
            cell.history.current_population,
            cell.history.dominant_economy,
            materialized,
        ),
        AppMode::LocalOverview => format!(
            "P4 integrated · LOCAL OVERVIEW · cell ({},{}) · history {} · LOD {:?} · routes {}",
            cell.coord.q, cell.coord.r, state.show_history, state.building_lod, state.show_routes,
        ),
        AppMode::RouteTravel => {
            let route = &world.0.traversal.routes[state.route_index];
            format!(
                "P4 integrated · ROUTE {} · {:.0} m · cells {} · target {} · side {:?} · paused {}",
                state.route_index + 1,
                state.route_distance_m,
                route.crossed_cells.len(),
                route.target_landmark_id,
                state.view_side,
                state.route_paused,
            )
        }
    };
}

fn route_runtime(route: &CompiledScrollRoute) -> RouteRuntime {
    RouteRuntime {
        polyline: PolylineRoute::new(route.world_path.clone()).expect("compiled route polyline"),
        grammars: route.grammars.clone(),
    }
}

fn flat_hex_mesh(coord: HexCoord, radius: f64) -> Mesh {
    let center = coord.center_xz(radius);
    let mut positions = vec![[center.x as f32, MAP_Y, center.y as f32]];
    let mut normals = vec![[0.0, 1.0, 0.0]];
    let mut uvs = vec![[0.5, 0.5]];
    for corner in 0..6 {
        let angle = (30.0 + corner as f64 * 60.0).to_radians();
        positions.push([
            (center.x + radius * angle.cos()) as f32,
            MAP_Y,
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

fn detailed_terrain_mesh(grid: &TerrainGrid) -> Mesh {
    let width = usize::from(grid.width);
    let height = usize::from(grid.height);
    let mut vertex_map = vec![None; grid.samples.len()];
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut colors = Vec::<[f32; 4]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();

    for z in 0..height {
        for x in 0..width {
            let index = grid.index(x, z);
            let sample = &grid.samples[index];
            if sample.cell.is_none() {
                continue;
            }
            vertex_map[index] = Some(positions.len() as u32);
            positions.push(sample.world_position.as_vec3().to_array());
            normals.push(terrain_normal(grid, x, z).to_array());
            colors.push(local_sample_color(sample));
            uvs.push([
                x as f32 / (width - 1) as f32,
                z as f32 / (height - 1) as f32,
            ]);
        }
    }

    let mut indices = Vec::<u32>::new();
    for z in 0..height - 1 {
        for x in 0..width - 1 {
            let a = vertex_map[grid.index(x, z)];
            let b = vertex_map[grid.index(x + 1, z)];
            let c = vertex_map[grid.index(x, z + 1)];
            let d = vertex_map[grid.index(x + 1, z + 1)];
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

fn terrain_normal(grid: &TerrainGrid, x: usize, z: usize) -> Vec3 {
    let sample_height = |sx: usize, sz: usize| {
        grid.sample(
            sx.min(usize::from(grid.width) - 1),
            sz.min(usize::from(grid.height) - 1),
        )
        .filter(|sample| sample.cell.is_some())
        .map(|sample| sample.world_position.y as f32)
        .unwrap_or_else(|| {
            grid.sample(x, z)
                .map(|sample| sample.world_position.y as f32)
                .unwrap_or(0.0)
        })
    };
    let left = sample_height(x.saturating_sub(1), z);
    let right = sample_height((x + 1).min(usize::from(grid.width) - 1), z);
    let down = sample_height(x, z.saturating_sub(1));
    let up = sample_height(x, (z + 1).min(usize::from(grid.height) - 1));
    Vec3::new(left - right, grid.spacing_m as f32 * 2.0, down - up).normalize_or_zero()
}

fn local_sample_color(sample: &integrated_world_core::TerrainSample) -> [f32; 4] {
    let (r, g, b) = match sample.land_cover {
        LandCover::OpenWater => (0.08, 0.33, 0.64),
        LandCover::Wetland => (0.30, 0.52, 0.35),
        LandCover::Cropland => (0.64, 0.68, 0.23),
        LandCover::Grassland => (0.43, 0.67, 0.31),
        LandCover::Forest => (0.17, 0.43, 0.22),
        LandCover::Scrub => (0.47, 0.48, 0.28),
        LandCover::BareRock => (0.48, 0.46, 0.43),
        LandCover::Built => (0.50, 0.36, 0.24),
        LandCover::Ruins => (0.35, 0.34, 0.34),
    };
    [r, g, b, 1.0]
}

fn atlas_cell_color(cell: &AtlasCellSpec, layer: AtlasLayer, all: &[AtlasCellSpec]) -> Color {
    match layer {
        AtlasLayer::Elevation => {
            let (minimum, maximum) = atlas_height_range(all);
            let t = ((cell.elevation.mean_m - minimum) / (maximum - minimum).max(1.0)) as f32;
            elevation_color(t)
        }
        AtlasLayer::Landforms => {
            let choices = [
                (cell.landforms.water, Color::srgb(0.10, 0.36, 0.68)),
                (cell.landforms.coast, Color::srgb(0.82, 0.70, 0.40)),
                (cell.landforms.lowland, Color::srgb(0.43, 0.67, 0.31)),
                (cell.landforms.valley, Color::srgb(0.27, 0.55, 0.28)),
                (cell.landforms.hillslope, Color::srgb(0.55, 0.48, 0.30)),
                (cell.landforms.ridge, Color::srgb(0.50, 0.41, 0.34)),
                (cell.landforms.mountain, Color::srgb(0.68, 0.68, 0.67)),
            ];
            choices
                .into_iter()
                .max_by(|left, right| left.0.total_cmp(&right.0))
                .map(|choice| choice.1)
                .unwrap_or(Color::srgb(0.5, 0.5, 0.5))
        }
        AtlasLayer::Climate => {
            let warmth = ((cell.climate.temperature_c + 5.0) / 30.0).clamp(0.0, 1.0) as f32;
            let moisture = cell.climate.moisture.clamp(0.0, 1.0) as f32;
            Color::srgb(
                0.20 + warmth * 0.65,
                0.22 + moisture * 0.58,
                0.58 - warmth * 0.35,
            )
        }
        AtlasLayer::Resources => {
            let value = ((cell.resources.fresh_water
                + cell.resources.arable_land
                + cell.resources.timber
                + cell.resources.fishery
                + cell.resources.harbor_quality)
                / 5.0)
                .clamp(0.0, 1.0) as f32;
            Color::srgb(
                0.20 + value * 0.72,
                0.18 + value * 0.62,
                0.16 + value * 0.12,
            )
        }
        AtlasLayer::History => {
            let population = (cell.history.current_population as f32 / 4_500.0).clamp(0.0, 1.0);
            if population <= 0.01 {
                Color::srgb(0.35, 0.40, 0.35)
            } else {
                Color::srgb(0.40 + population * 0.45, 0.22 + population * 0.22, 0.15)
            }
        }
    }
}

fn elevation_color(t: f32) -> Color {
    if t < 0.18 {
        Color::srgb(0.10, 0.36, 0.68)
    } else if t < 0.32 {
        Color::srgb(0.76, 0.68, 0.39)
    } else if t < 0.58 {
        Color::srgb(0.38, 0.62, 0.29)
    } else if t < 0.80 {
        Color::srgb(0.52, 0.43, 0.30)
    } else {
        Color::srgb(0.72, 0.72, 0.71)
    }
}

fn atlas_height_range(cells: &[AtlasCellSpec]) -> (f64, f64) {
    cells.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(minimum, maximum), cell| {
            (
                minimum.min(cell.elevation.minimum_m),
                maximum.max(cell.elevation.maximum_m),
            )
        },
    )
}

fn contour_segments(world: &IntegratedWorld, level: f64) -> Vec<(glam::DVec2, glam::DVec2)> {
    let radius = world.atlas.cell_radius_m;
    let centers = world
        .atlas
        .cells
        .iter()
        .map(|cell| cell.coord.center_xz(radius))
        .collect::<Vec<_>>();
    let minimum = centers
        .iter()
        .copied()
        .fold(glam::DVec2::splat(f64::INFINITY), glam::DVec2::min)
        - glam::DVec2::splat(radius * 1.1);
    let maximum = centers
        .iter()
        .copied()
        .fold(glam::DVec2::splat(f64::NEG_INFINITY), glam::DVec2::max)
        + glam::DVec2::splat(radius * 1.1);
    let resolution = 45usize;
    let step = (maximum - minimum) / (resolution - 1) as f64;
    let mut heights = vec![0.0; resolution * resolution];
    for z in 0..resolution {
        for x in 0..resolution {
            let point = minimum + glam::DVec2::new(x as f64 * step.x, z as f64 * step.y);
            heights[z * resolution + x] =
                detailed_height(world.atlas.world_seed, world.atlas.cell_radius_m, point);
        }
    }
    let mut result = Vec::new();
    for z in 0..resolution - 1 {
        for x in 0..resolution - 1 {
            let points = [
                minimum + glam::DVec2::new(x as f64 * step.x, z as f64 * step.y),
                minimum + glam::DVec2::new((x + 1) as f64 * step.x, z as f64 * step.y),
                minimum + glam::DVec2::new((x + 1) as f64 * step.x, (z + 1) as f64 * step.y),
                minimum + glam::DVec2::new(x as f64 * step.x, (z + 1) as f64 * step.y),
            ];
            let values = [
                heights[z * resolution + x],
                heights[z * resolution + x + 1],
                heights[(z + 1) * resolution + x + 1],
                heights[(z + 1) * resolution + x],
            ];
            let mut crossings = Vec::new();
            for (left, right) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
                let a = values[left] - level;
                let b = values[right] - level;
                if (a <= 0.0 && b > 0.0) || (a > 0.0 && b <= 0.0) {
                    let fraction = a.abs() / (a.abs() + b.abs()).max(1.0e-9);
                    crossings.push(points[left].lerp(points[right], fraction));
                }
            }
            if crossings.len() >= 2 {
                result.push((crossings[0], crossings[1]));
            }
            if crossings.len() == 4 {
                result.push((crossings[2], crossings[3]));
            }
        }
    }
    result
}

fn map_point(point: glam::DVec2, y: f32) -> Vec3 {
    Vec3::new(
        point.x as f32 * ATLAS_SCALE,
        y,
        point.y as f32 * ATLAS_SCALE,
    )
}

fn spawn_flat_hex_outline(
    commands: &mut Commands,
    assets: &VisualAssets,
    coord: HexCoord,
    radius: f64,
    y: f32,
    material: Handle<StandardMaterial>,
    kind: VisualKind,
) {
    let center = coord.center_xz(radius);
    let corners = std::array::from_fn::<_, 6, _>(|index| {
        let angle = (30.0 + index as f64 * 60.0).to_radians();
        map_point(
            center + glam::DVec2::new(angle.cos(), angle.sin()) * radius,
            y,
        )
    });
    for index in 0..6 {
        spawn_segment(
            commands,
            assets,
            material.clone(),
            corners[index],
            corners[(index + 1) % 6],
            1.5,
            0.7,
            VisualTag {
                space: VisualSpace::Atlas,
                kind,
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
// A segment is one rendering primitive with explicit geometry, material and visibility data.
#[allow(clippy::too_many_arguments)]
// A segment is one rendering primitive with explicit geometry, material and visibility data.
#[allow(clippy::too_many_arguments)]
// A segment is one rendering primitive with explicit geometry, material and visibility data.
fn spawn_segment(
    commands: &mut Commands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    width: f32,
    height: f32,
    tag: VisualTag,
) {
    let delta = end - start;
    let length = delta.length();
    if length <= 1.0e-4 {
        return;
    }
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform {
            translation: (start + end) * 0.5,
            rotation: Quat::from_rotation_arc(Vec3::X, delta / length),
            scale: Vec3::new(length, height, width),
        },
        tag,
    ));
}

fn detailed_bounds(grid: &TerrainGrid) -> (Vec2, Vec2) {
    grid.samples
        .iter()
        .filter(|sample| sample.cell.is_some())
        .fold(
            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
            |(minimum, maximum), sample| {
                let point = sample.world_position.xz().as_vec2();
                (minimum.min(point), maximum.max(point))
            },
        )
}
