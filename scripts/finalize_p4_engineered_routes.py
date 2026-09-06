#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TRAVERSAL = ROOT / "crates/integrated_world_core/src/traversal.rs"


def replace_function(text: str, name: str, next_name: str, replacement: str) -> str:
    updated, count = re.subn(
        rf"fn {name}\(.*?\n}}\n\n(?=fn {next_name}\()",
        replacement.rstrip() + "\n\n",
        text,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise RuntimeError(f"could not replace {name}")
    return updated


def finalize_route_starts(text: str) -> str:
    if "let mut selected_cells = BTreeSet::new();" in text:
        return text
    replacement = '''fn route_start_samples(detailed: &DetailedRegion, port: DVec3) -> [DVec3; 3] {
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
                .filter(|sample| {
                    sample.cell == Some(*cell) && is_route_passable(sample, None)
                })
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
}'''
    return replace_function(text, "route_start_samples", "find_path", replacement)


def finalize_waypoint_routing(text: str) -> str:
    old = '''    for (route_index, start) in starts.into_iter().enumerate() {
        let first = find_path(&detailed.terrain, start, port, bridge, history)
            .unwrap_or_else(|| direct_fallback(&detailed.terrain, start, port));
        let second = find_path(&detailed.terrain, port, target, bridge, history)
            .unwrap_or_else(|| direct_fallback(&detailed.terrain, port, target));
        let mut path_indices = first;
        path_indices.extend(second.into_iter().skip(1));
'''
    new = '''    for (route_index, start) in starts.into_iter().enumerate() {
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
'''
    if old in text:
        text = text.replace(old, new, 1)
    elif "let waypoint = route_waypoint_sample" not in text:
        raise RuntimeError("could not connect deterministic route waypoints")

    if "fn route_waypoint_sample(" in text:
        return text
    helper = '''fn route_waypoint_sample(
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
        .filter(|cell| {
            *cell != start_cell && *cell != port_cell && *cell != target_cell
        })
        .filter_map(|cell| {
            detailed
                .terrain
                .samples
                .iter()
                .filter(|sample| {
                    sample.cell == Some(cell) && is_route_passable(sample, None)
                })
                .min_by(|left, right| {
                    left.travel_cost
                        .total_cmp(&right.travel_cost)
                        .then_with(|| left.slope.total_cmp(&right.slope))
                        .then_with(|| {
                            left.world_position
                                .xz()
                                .distance_squared(port.xz())
                                .total_cmp(
                                    &right
                                        .world_position
                                        .xz()
                                        .distance_squared(port.xz()),
                                )
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

'''
    marker = "fn find_path("
    if marker not in text:
        raise RuntimeError("find_path insertion point missing")
    return text.replace(marker, helper + marker, 1)


def finalize_path_cost(text: str) -> str:
    old = '''            let current_sample = &grid.samples[current.index];
            let horizontal = current_sample
                .world_position
                .xz()
                .distance(sample.world_position.xz())
                .max(0.01);
            let grade =
                (current_sample.world_position.y - sample.world_position.y).abs() / horizontal;
            if grade > 0.58 + 1.0e-9 {
                continue;
            }
            let step = current_sample
                .world_position
                .distance(sample.world_position)
                * corridor_modifier(history, sample.world_position);
            let tentative = current_cost + step * sample.travel_cost.max(0.25);
'''
    new = '''            let current_sample = &grid.samples[current.index];
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
            let tentative =
                current_cost + step * sample.travel_cost.max(0.25) * grade_penalty;
'''
    if old in text:
        return text.replace(old, new, 1)
    if "let grade_penalty = 1.0 + grade.powi(2) * 18.0;" in text:
        return text
    raise RuntimeError("could not replace strict path grade gate")


def finalize_engineered_profile(text: str) -> str:
    old = '''        let world_path = simplified
            .iter()
            .map(|index| detailed.terrain.samples[*index].world_position + DVec3::Y * 0.6)
            .collect::<Vec<_>>();
'''
    new = '''        let terrain_path = simplified
            .iter()
            .map(|index| detailed.terrain.samples[*index].world_position + DVec3::Y * 0.6)
            .collect::<Vec<_>>();
        let world_path = engineer_route_profile(&terrain_path, 0.48);
'''
    if old in text:
        text = text.replace(old, new, 1)
    elif "let world_path = engineer_route_profile(&terrain_path, 0.48);" not in text:
        raise RuntimeError("could not connect route profile engineering")

    if "fn engineer_route_profile(" in text:
        return text
    helper = '''fn engineer_route_profile(terrain_path: &[DVec3], maximum_grade: f64) -> Vec<DVec3> {
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

'''
    marker = "fn route_grammars("
    if marker not in text:
        raise RuntimeError("route_grammars insertion point missing")
    return text.replace(marker, helper + marker, 1)


def main() -> int:
    text = TRAVERSAL.read_text(encoding="utf-8")
    text = finalize_route_starts(text)
    text = finalize_waypoint_routing(text)
    text = finalize_path_cost(text)
    text = finalize_engineered_profile(text)
    TRAVERSAL.write_text(text, encoding="utf-8")
    print("Integrated P4 engineered route finalization completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
