use crate::atlas::{digest, stable_entity};
use crate::model::*;
use glam::{DVec2, DVec3, Vec3Swizzles};
use scroll_camera_core::ScrollGrammar;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use world_generation_core::HexCoord;
use world_ids::{EntityId, RoadId, RouteId};

const STAGE_TRAVERSAL: u64 = 0x4400;

#[derive(Clone, Copy, Debug)]
struct QueueState {
    index: usize,
    estimated_total: f64,
}

impl PartialEq for QueueState {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.estimated_total.to_bits() == other.estimated_total.to_bits()
    }
}

impl Eq for QueueState {}

impl PartialOrd for QueueState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total
            .total_cmp(&self.estimated_total)
            .then_with(|| other.index.cmp(&self.index))
    }
}

pub fn compile_traversal(
    atlas: &WorldAtlasManifest,
    detailed: &DetailedRegion,
    history: &HistoryLedger,
) -> Result<TraversalCompilation, serde_json::Error> {
    let target = atlas
        .features
        .iter()
        .find(|feature| feature.kind == AtlasFeatureKind::Landmark)
        .and_then(|feature| feature.path_world.first().copied())
        .expect("Atlas landmark has a world anchor");
    let port = history
        .land_use
        .iter()
        .find(|zone| zone.kind == LandUseKind::OldTown)
        .map(|zone| zone.center_world)
        .expect("history contains old town");
    let bridge = history
        .assets
        .iter()
        .find(|asset| asset.kind == HistoricalAssetKind::Bridge)
        .map(|asset| asset.anchor_world)
        .expect("history contains bridge");

    let starts = route_start_samples(detailed, port);
    let mut all_nodes = BTreeMap::<EntityId, TraversalNode>::new();
    let mut all_edges = Vec::new();
    let mut routes = Vec::new();

    for (route_index, start) in starts.into_iter().enumerate() {
        let waypoint = route_waypoint_sample(detailed, start, port, target, route_index);
        let first = if let Some(waypoint) = waypoint {
            let mut to_waypoint = find_path(&detailed.terrain, start, waypoint, bridge, history)
                .unwrap_or_else(|| direct_fallback(&detailed.terrain, start, waypoint));
            let to_port = find_path(&detailed.terrain, waypoint, port, bridge, history)
                .unwrap_or_else(|| direct_fallback(&detailed.terrain, waypoint, port));
            to_waypoint.extend(to_port.into_iter().skip(1));
            to_waypoint
        } else {
            find_path(&detailed.terrain, start, port, bridge, history)
                .unwrap_or_else(|| direct_fallback(&detailed.terrain, start, port))
        };
        let second = find_path(&detailed.terrain, port, target, bridge, history)
            .unwrap_or_else(|| direct_fallback(&detailed.terrain, port, target));
        let mut path_indices = first;
        path_indices.extend(second.into_iter().skip(1));
        path_indices.dedup();
        let simplified = simplify_path(&detailed.terrain, &path_indices);
        let route_id = RouteId::from_u128(
            stable_entity(
                atlas.world_seed,
                STAGE_TRAVERSAL,
                route_index as u128 + 1,
                0x710,
            )
            .as_u128(),
        );
        let road_id = RoadId::from_u128(
            stable_entity(
                atlas.world_seed,
                STAGE_TRAVERSAL,
                route_index as u128 + 1,
                0x711,
            )
            .as_u128(),
        );

        let terrain_path = simplified
            .iter()
            .map(|index| detailed.terrain.samples[*index].world_position + DVec3::Y * 0.6)
            .collect::<Vec<_>>();
        let world_path = engineer_route_profile(&terrain_path, 0.48);
        let port_index = world_path
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                left.distance_squared(port)
                    .total_cmp(&right.distance_squared(port))
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        let grammars = route_grammars(&world_path, port_index);
        let node_ids = world_path
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let node_id = stable_entity(
                    atlas.world_seed,
                    STAGE_TRAVERSAL,
                    route_id.as_u128(),
                    0x800 + index as u128,
                );
                let sample = nearest_sample(&detailed.terrain, point.xz());
                let source_asset_id = nearest_history_asset(history, *point, 34.0);
                all_nodes.entry(node_id).or_insert_with(|| TraversalNode {
                    id: node_id,
                    world_position: *point,
                    cell: sample.cell.unwrap_or(HexCoord::ZERO),
                    surface: surface_for(point, history),
                    modes: if sample.slope <= 0.22 {
                        vec![TraversalMode::Walking, TraversalMode::Wagon]
                    } else {
                        vec![TraversalMode::Walking]
                    },
                    source_asset_id,
                });
                node_id
            })
            .collect::<Vec<_>>();
        for pair in node_ids.windows(2) {
            let left = &all_nodes[&pair[0]];
            let right = &all_nodes[&pair[1]];
            let horizontal = left
                .world_position
                .xz()
                .distance(right.world_position.xz())
                .max(0.01);
            let grade = (left.world_position.y - right.world_position.y).abs() / horizontal;
            all_edges.push(TraversalEdge {
                from: left.id,
                to: right.id,
                length_m: left.world_position.distance(right.world_position),
                maximum_grade: grade,
                minimum_width_m: if matches!(left.surface, TraversalSurface::Trail) {
                    3.0
                } else {
                    6.0
                },
            });
        }

        let crossed_cells = world_path
            .iter()
            .filter_map(|point| nearest_sample(&detailed.terrain, point.xz()).cell)
            .fold(Vec::<HexCoord>::new(), |mut cells, cell| {
                if cells.last().copied() != Some(cell) {
                    cells.push(cell);
                }
                cells
            });
        let maximum_grade = world_path
            .windows(2)
            .map(|pair| {
                let horizontal = pair[0].xz().distance(pair[1].xz()).max(0.01);
                (pair[0].y - pair[1].y).abs() / horizontal
            })
            .fold(0.0_f64, f64::max);
        routes.push(CompiledScrollRoute {
            id: route_id,
            road_id,
            target_landmark_id: atlas.landmark_id,
            node_ids,
            world_path,
            grammars,
            crossed_cells,
            maximum_grade,
        });
    }

    routes.sort_by_key(|route| route.id);
    all_edges.sort_by_key(|edge| (edge.from, edge.to));
    let mut compilation = TraversalCompilation {
        nodes: all_nodes.into_values().collect(),
        edges: all_edges,
        routes,
        target_landmark_id: atlas.landmark_id,
        semantic_fingerprint: 0,
    };
    compilation.semantic_fingerprint = digest(&compilation)?;
    Ok(compilation)
}

fn route_start_samples(detailed: &DetailedRegion, port: DVec3) -> [DVec3; 3] {
    let port_cell = nearest_sample(&detailed.terrain, port.xz())
        .cell
        .unwrap_or(HexCoord::ZERO);
    let mut candidate_cells = detailed.materialized_cells.clone();
    candidate_cells.sort_by(|left, right| {
        right
            .distance(port_cell)
            .cmp(&left.distance(port_cell))
            .then_with(|| left.cmp(right))
    });

    let mut selected_cells = BTreeSet::new();
    let mut starts = Vec::new();
    for minimum_distance in [2, 0] {
        for cell in &candidate_cells {
            if selected_cells.contains(cell) || cell.distance(port_cell) < minimum_distance {
                continue;
            }
            let sample = detailed
                .terrain
                .samples
                .iter()
                .filter(|sample| sample.cell == Some(*cell) && is_route_passable(sample, None))
                .max_by(|left, right| {
                    left.world_position
                        .xz()
                        .distance_squared(port.xz())
                        .total_cmp(&right.world_position.xz().distance_squared(port.xz()))
                });
            if let Some(sample) = sample {
                selected_cells.insert(*cell);
                starts.push(sample.world_position);
            }
            if starts.len() == 3 {
                return [starts[0], starts[1], starts[2]];
            }
        }
    }

    let fallbacks = [
        port - DVec3::X * 240.0,
        port - DVec3::Z * 220.0,
        port + DVec3::Z * 220.0,
    ];
    for fallback in fallbacks {
        if starts.len() == 3 {
            break;
        }
        let sample = nearest_passable_index(&detailed.terrain, fallback.xz(), None)
            .map(|index| detailed.terrain.samples[index].world_position)
            .unwrap_or(fallback);
        starts.push(sample);
    }
    [starts[0], starts[1], starts[2]]
}

fn route_waypoint_sample(
    detailed: &DetailedRegion,
    start: DVec3,
    port: DVec3,
    target: DVec3,
    route_index: usize,
) -> Option<DVec3> {
    let start_cell = nearest_sample(&detailed.terrain, start.xz()).cell?;
    let port_cell = nearest_sample(&detailed.terrain, port.xz()).cell?;
    let target_cell = nearest_sample(&detailed.terrain, target.xz()).cell?;
    let mut candidates = detailed
        .materialized_cells
        .iter()
        .copied()
        .filter(|cell| *cell != start_cell && *cell != port_cell && *cell != target_cell)
        .filter_map(|cell| {
            detailed
                .terrain
                .samples
                .iter()
                .filter(|sample| sample.cell == Some(cell) && is_route_passable(sample, None))
                .min_by(|left, right| {
                    left.travel_cost
                        .total_cmp(&right.travel_cost)
                        .then_with(|| left.slope.total_cmp(&right.slope))
                        .then_with(|| {
                            left.world_position
                                .xz()
                                .distance_squared(port.xz())
                                .total_cmp(&right.world_position.xz().distance_squared(port.xz()))
                        })
                })
                .map(|sample| {
                    (
                        cell,
                        sample.world_position,
                        distance_to_segment_squared(
                            sample.world_position.xz(),
                            start.xz(),
                            port.xz(),
                        ),
                    )
                })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .2
            .total_cmp(&left.2)
            .then_with(|| left.0.cmp(&right.0))
    });
    candidates
        .get(route_index % candidates.len().max(1))
        .map(|(_, position, _)| *position)
}

fn distance_to_segment_squared(point: DVec2, start: DVec2, end: DVec2) -> f64 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f64::EPSILON {
        return point.distance_squared(start);
    }
    let amount = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(start + segment * amount)
}

fn find_path(
    grid: &TerrainGrid,
    start_world: DVec3,
    goal_world: DVec3,
    bridge: DVec3,
    history: &HistoryLedger,
) -> Option<Vec<usize>> {
    let start = nearest_passable_index(grid, start_world.xz(), Some(bridge))?;
    let goal = nearest_passable_index(grid, goal_world.xz(), Some(bridge))?;
    let mut open = BinaryHeap::new();
    let mut g_score = vec![f64::INFINITY; grid.samples.len()];
    let mut came_from = vec![None; grid.samples.len()];
    g_score[start] = 0.0;
    open.push(QueueState {
        index: start,
        estimated_total: heuristic(grid, start, goal),
    });

    while let Some(current) = open.pop() {
        if current.index == goal {
            return Some(reconstruct_path(&came_from, current.index));
        }
        let current_cost = g_score[current.index];
        for neighbor in neighbors(grid, current.index) {
            let sample = &grid.samples[neighbor];
            if !is_route_passable(sample, Some(bridge)) {
                continue;
            }
            let current_sample = &grid.samples[current.index];
            let horizontal = current_sample
                .world_position
                .xz()
                .distance(sample.world_position.xz())
                .max(0.01);
            let grade =
                (current_sample.world_position.y - sample.world_position.y).abs() / horizontal;
            let grade_penalty = 1.0 + grade.powi(2) * 18.0;
            let step = current_sample
                .world_position
                .distance(sample.world_position)
                * corridor_modifier(history, sample.world_position);
            let tentative = current_cost + step * sample.travel_cost.max(0.25) * grade_penalty;
            if tentative + 1.0e-9 < g_score[neighbor] {
                came_from[neighbor] = Some(current.index);
                g_score[neighbor] = tentative;
                open.push(QueueState {
                    index: neighbor,
                    estimated_total: tentative + heuristic(grid, neighbor, goal),
                });
            }
        }
    }
    None
}

fn direct_fallback(grid: &TerrainGrid, start: DVec3, goal: DVec3) -> Vec<usize> {
    let start_index = nearest_passable_index(grid, start.xz(), None).unwrap_or(0);
    let goal_index = nearest_passable_index(grid, goal.xz(), None).unwrap_or(start_index);
    let steps = 48;
    (0..=steps)
        .filter_map(|step| {
            let point = start.xz().lerp(goal.xz(), step as f64 / steps as f64);
            nearest_passable_index(grid, point, None)
        })
        .fold(Vec::new(), |mut path, index| {
            if path.last().copied() != Some(index) {
                path.push(index);
            }
            path
        })
        .into_iter()
        .chain(std::iter::once(goal_index))
        .collect()
}

fn reconstruct_path(came_from: &[Option<usize>], mut current: usize) -> Vec<usize> {
    let mut path = vec![current];
    while let Some(previous) = came_from[current] {
        current = previous;
        path.push(current);
    }
    path.reverse();
    path
}

fn simplify_path(_grid: &TerrainGrid, path: &[usize]) -> Vec<usize> {
    let mut detailed_path = path.to_vec();
    detailed_path.dedup();
    detailed_path
}

fn engineer_route_profile(terrain_path: &[DVec3], maximum_grade: f64) -> Vec<DVec3> {
    let mut profile = terrain_path.to_vec();
    if profile.len() < 2 {
        return profile;
    }

    for _ in 0..6 {
        constrain_profile_forward(&mut profile, maximum_grade);
        for index in (0..profile.len() - 1).rev() {
            let horizontal = profile[index]
                .xz()
                .distance(profile[index + 1].xz())
                .max(0.01);
            let maximum_delta = horizontal * maximum_grade;
            profile[index].y = profile[index].y.clamp(
                profile[index + 1].y - maximum_delta,
                profile[index + 1].y + maximum_delta,
            );
        }
    }
    constrain_profile_forward(&mut profile, maximum_grade);
    profile
}

fn constrain_profile_forward(profile: &mut [DVec3], maximum_grade: f64) {
    for index in 1..profile.len() {
        let horizontal = profile[index - 1]
            .xz()
            .distance(profile[index].xz())
            .max(0.01);
        let maximum_delta = horizontal * maximum_grade;
        profile[index].y = profile[index].y.clamp(
            profile[index - 1].y - maximum_delta,
            profile[index - 1].y + maximum_delta,
        );
    }
}

fn route_grammars(path: &[DVec3], port_index: usize) -> Vec<ScrollGrammar> {
    path.iter()
        .enumerate()
        .map(|(index, point)| {
            if index == 0 {
                return ScrollGrammar::VistaReveal;
            }
            if index + 1 == path.len() {
                return ScrollGrammar::StandardSideView;
            }
            if index + 2 == port_index {
                return ScrollGrammar::JunctionApproach;
            }
            if index == port_index {
                return ScrollGrammar::JunctionDecision;
            }
            if index == port_index.saturating_add(1) {
                return ScrollGrammar::TurnCommit;
            }
            if index == port_index.saturating_add(2) {
                return ScrollGrammar::CameraReorientation;
            }
            let previous = path[index - 1];
            let next = path[index + 1];
            let horizontal = previous.xz().distance(next.xz()).max(0.01);
            let grade = (previous.y - next.y).abs() / horizontal;
            if grade > 0.10 {
                return ScrollGrammar::VerticalTransition;
            }
            let incoming = (point.xz() - previous.xz()).normalize_or_zero();
            let outgoing = (next.xz() - point.xz()).normalize_or_zero();
            let turn = incoming.dot(outgoing).clamp(-1.0, 1.0).acos();
            if turn > 0.20 {
                ScrollGrammar::LightDepth
            } else {
                ScrollGrammar::StandardSideView
            }
        })
        .collect()
}

fn surface_for(point: &DVec3, history: &HistoryLedger) -> TraversalSurface {
    if history.assets.iter().any(|asset| {
        asset.kind == HistoricalAssetKind::Bridge && asset.anchor_world.distance(*point) < 28.0
    }) {
        TraversalSurface::Bridge
    } else if history.assets.iter().any(|asset| {
        asset.kind == HistoricalAssetKind::Harbor && asset.anchor_world.distance(*point) < 44.0
    }) {
        TraversalSurface::Dock
    } else if history.land_use.iter().any(|zone| {
        matches!(zone.kind, LandUseKind::OldTown | LandUseKind::NewTown)
            && zone.center_world.distance(*point) < zone.radius_m
    }) {
        TraversalSurface::Plaza
    } else if history.assets.iter().any(|asset| {
        matches!(
            asset.kind,
            HistoricalAssetKind::OldRoad | HistoricalAssetKind::NewRoad
        ) && asset.anchor_world.distance(*point) < asset.extent_m.x * 0.6
    }) {
        TraversalSurface::Road
    } else {
        TraversalSurface::Trail
    }
}

fn nearest_history_asset(
    history: &HistoryLedger,
    point: DVec3,
    maximum_distance: f64,
) -> Option<EntityId> {
    history
        .assets
        .iter()
        .filter(|asset| asset.retired_by.is_none() || asset.kind == HistoricalAssetKind::Ruins)
        .min_by(|left, right| {
            left.anchor_world
                .distance_squared(point)
                .total_cmp(&right.anchor_world.distance_squared(point))
        })
        .filter(|asset| asset.anchor_world.distance(point) <= maximum_distance)
        .map(|asset| asset.id)
}

fn corridor_modifier(history: &HistoryLedger, point: DVec3) -> f64 {
    let near_current_road = history.assets.iter().any(|asset| {
        matches!(
            asset.kind,
            HistoricalAssetKind::NewRoad | HistoricalAssetKind::Bridge
        ) && asset.retired_by.is_none()
            && asset.anchor_world.distance(point) <= asset.extent_m.x * 0.65
    });
    let near_old_road = history.assets.iter().any(|asset| {
        asset.kind == HistoricalAssetKind::OldRoad
            && asset.anchor_world.distance(point) <= asset.extent_m.x * 0.55
    });
    if near_current_road {
        0.42
    } else if near_old_road {
        0.72
    } else {
        1.0
    }
}

fn nearest_sample(grid: &TerrainGrid, point: DVec2) -> &TerrainSample {
    grid.samples
        .iter()
        .filter(|sample| sample.cell.is_some())
        .min_by(|left, right| {
            left.world_position
                .xz()
                .distance_squared(point)
                .total_cmp(&right.world_position.xz().distance_squared(point))
        })
        .expect("detailed grid contains valid samples")
}

fn nearest_passable_index(
    grid: &TerrainGrid,
    point: DVec2,
    bridge: Option<DVec3>,
) -> Option<usize> {
    grid.samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| is_route_passable(sample, bridge))
        .min_by(|(_, left), (_, right)| {
            left.world_position
                .xz()
                .distance_squared(point)
                .total_cmp(&right.world_position.xz().distance_squared(point))
        })
        .map(|(index, _)| index)
}

fn is_route_passable(sample: &TerrainSample, bridge: Option<DVec3>) -> bool {
    if sample.cell.is_none() || sample.slope > 0.58 || sample.landform == LandformClass::Ocean {
        return false;
    }
    if matches!(
        sample.landform,
        LandformClass::Estuary | LandformClass::Coast
    ) {
        return bridge.is_some_and(|bridge| bridge.distance(sample.world_position) <= 34.0);
    }
    true
}

fn neighbors(grid: &TerrainGrid, index: usize) -> Vec<usize> {
    let width = usize::from(grid.width);
    let height = usize::from(grid.height);
    let x = index % width;
    let z = index / width;
    let mut result = Vec::with_capacity(8);
    for dz in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let nz = z as isize + dz;
            if nx >= 0 && nz >= 0 && nx < width as isize && nz < height as isize {
                result.push(nz as usize * width + nx as usize);
            }
        }
    }
    result
}

fn heuristic(grid: &TerrainGrid, left: usize, right: usize) -> f64 {
    grid.samples[left]
        .world_position
        .xz()
        .distance(grid.samples[right].world_position.xz())
}

pub fn validate_traversal(
    atlas: &WorldAtlasManifest,
    detailed: &DetailedRegion,
    traversal: &TraversalCompilation,
) -> Vec<ValidationCheck> {
    let node_by_id = traversal
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<BTreeMap<_, _>>();
    let nodes_resolve = traversal
        .routes
        .iter()
        .all(|route| route.node_ids.iter().all(|id| node_by_id.contains_key(id)));
    let paths_on_world = traversal.routes.iter().all(|route| {
        route.world_path.iter().all(|point| {
            let sample = nearest_sample(&detailed.terrain, point.xz());
            sample.cell.is_some()
                && sample.landform != LandformClass::Ocean
                && sample.world_position.xz().distance(point.xz())
                    <= detailed.terrain.spacing_m * 1.5
        })
    });
    let route_grades = traversal
        .routes
        .iter()
        .map(|route| route.maximum_grade)
        .collect::<Vec<_>>();
    let grades_valid = route_grades.iter().all(|grade| *grade <= 0.58 + 1.0e-9);
    let target_identity = traversal.target_landmark_id == atlas.landmark_id
        && traversal
            .routes
            .iter()
            .all(|route| route.target_landmark_id == atlas.landmark_id);
    let route_cell_counts = traversal
        .routes
        .iter()
        .map(|route| {
            route
                .crossed_cells
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
        })
        .collect::<Vec<_>>();
    let multiple_cells = route_cell_counts.iter().all(|count| *count >= 3);
    let route_density = traversal
        .routes
        .iter()
        .all(|route| route.world_path.len() > route.crossed_cells.len().saturating_mul(2));
    let grammar_set = traversal
        .routes
        .iter()
        .flat_map(|route| route.grammars.iter().copied())
        .map(grammar_key)
        .collect::<BTreeSet<_>>();
    let required_grammars = [
        ScrollGrammar::StandardSideView,
        ScrollGrammar::LightDepth,
        ScrollGrammar::JunctionApproach,
        ScrollGrammar::JunctionDecision,
        ScrollGrammar::TurnCommit,
        ScrollGrammar::CameraReorientation,
        ScrollGrammar::VistaReveal,
        ScrollGrammar::VerticalTransition,
    ]
    .into_iter()
    .all(|grammar| grammar_set.contains(&grammar_key(grammar)));

    vec![
        ValidationCheck {
            name: "traversal-node-references".into(),
            passed: nodes_resolve,
            detail: format!(
                "{} route nodes resolve in the traversal graph",
                node_by_id.len()
            ),
        },
        ValidationCheck {
            name: "routes-follow-materialized-world".into(),
            passed: paths_on_world,
            detail: "route points stay on materialized land samples rather than cell-center chords"
                .into(),
        },
        ValidationCheck {
            name: "route-grade-budget".into(),
            passed: grades_valid,
            detail: format!("route maximum grades are {route_grades:?}; budget is 0.58"),
        },
        ValidationCheck {
            name: "route-target-identity".into(),
            passed: target_identity,
            detail: format!("all routes target the Atlas landmark {}", atlas.landmark_id),
        },
        ValidationCheck {
            name: "routes-cross-multiple-cells".into(),
            passed: multiple_cells,
            detail: format!("route unique materialized-cell counts are {route_cell_counts:?}"),
        },
        ValidationCheck {
            name: "routes-are-denser-than-atlas-centers".into(),
            passed: route_density,
            detail: "route polylines contain detailed terrain samples between cell transitions"
                .into(),
        },
        ValidationCheck {
            name: "scroll-grammar-coverage".into(),
            passed: required_grammars,
            detail: format!(
                "{} distinct scroll grammar classes are present",
                grammar_set.len()
            ),
        },
    ]
}

fn grammar_key(grammar: ScrollGrammar) -> u8 {
    match grammar {
        ScrollGrammar::StandardSideView => 0,
        ScrollGrammar::LightDepth => 1,
        ScrollGrammar::JunctionApproach => 2,
        ScrollGrammar::JunctionDecision => 3,
        ScrollGrammar::TurnCommit => 4,
        ScrollGrammar::CameraReorientation => 5,
        ScrollGrammar::VistaReveal => 6,
        ScrollGrammar::VerticalTransition => 7,
        ScrollGrammar::Cutaway => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        atlas::build_atlas,
        history::generate_history,
        terrain::{default_materialized_cells, materialize_region},
    };
    use world_generation_core::{
        GeneratorVersion, TraversalOrder, WorldGenerationConfig, generate_world_with_order,
    };
    use world_ids::{BuildingId, WorldId};

    #[test]
    fn routes_are_compiled_from_detailed_terrain() {
        let base = generate_world_with_order(
            WorldGenerationConfig {
                world_id: WorldId::from_u128(0x5044),
                world_seed: 0x706f_7274_2d76_616c_6c65_792d_3031,
                generator_version: GeneratorVersion(1),
                radius: 2,
                cell_radius_m: 128.0,
                building_blueprint_id: BuildingId::from_u128(1),
            },
            TraversalOrder::Canonical,
        )
        .expect("base");
        let atlas = build_atlas(&base).expect("atlas");
        let detailed =
            materialize_region(&atlas, &default_materialized_cells(&atlas)).expect("detailed");
        let history = generate_history(&atlas, &detailed, &base.manifest).expect("history");
        let traversal = compile_traversal(&atlas, &detailed, &history).expect("traversal");
        assert_eq!(traversal.routes.len(), 3);
        assert!(
            validate_traversal(&atlas, &detailed, &traversal)
                .iter()
                .all(|check| check.passed)
        );
    }
}
