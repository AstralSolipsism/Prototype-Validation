#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_exact(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


def refine_generator() -> None:
    path = ROOT / "crates" / "world_generation_core" / "src" / "generate.rs"
    text = path.read_text(encoding="utf-8")

    old_elevation = """    let elevation_m = if coord.q == config.radius {
        -42.0 - elevation_noise * 18.0
    } else {
        let inland_steps = f64::from(config.radius - coord.q - 1).max(0.0);
        let coastal_gradient = 34.0 + inland_steps * 145.0;
        let ridge_axis = f64::from(coord.r) + f64::from(coord.q) * 0.35;
        let ridge = if coord.q <= 0 {
            285.0 * (-0.72 * ridge_axis * ridge_axis).exp()
        } else {
            0.0
        };
        coastal_gradient + ridge + (elevation_noise - 0.5) * 24.0
    };

    let surface = if coord.q == config.radius {
        SurfaceClass::Ocean
    } else if coord.q == config.radius - 1 {
        SurfaceClass::Coast
    } else if elevation_m >= 650.0 {
        SurfaceClass::Alpine
    } else if elevation_m >= 420.0 {
        SurfaceClass::Highland
    } else {
        SurfaceClass::Lowland
    };
"""
    new_elevation = """    let elevation_m = if coord.q == config.radius {
        -12.0 - elevation_noise * 6.0
    } else if coord.q == config.radius - 1 {
        32.0 + (elevation_noise - 0.5) * 8.0
    } else {
        let inland_steps = f64::from(config.radius - coord.q - 1).max(1.0);
        let coastal_gradient = 32.0 + inland_steps * 30.0;
        let ridge_axis = f64::from(coord.r) + f64::from(coord.q) * 0.35;
        let ridge = 45.0 * (-0.72 * ridge_axis * ridge_axis).exp();
        coastal_gradient + ridge + (elevation_noise - 0.5) * 8.0
    };

    let surface = if coord.q == config.radius {
        SurfaceClass::Ocean
    } else if coord.q == config.radius - 1 {
        SurfaceClass::Coast
    } else if elevation_m >= 155.0 {
        SurfaceClass::Alpine
    } else if elevation_m >= 105.0 {
        SurfaceClass::Highland
    } else {
        SurfaceClass::Lowland
    };
"""
    text = replace_exact(text, old_elevation, new_elevation, "elevation model")

    text = replace_exact(
        text,
        """        SurfaceClass::Lowland => 1.0 + elevation_m.max(0.0) / 1_200.0,
        SurfaceClass::Highland => 1.8 + elevation_m / 1_000.0,
        SurfaceClass::Alpine => 3.0 + elevation_m / 800.0,
""",
        """        SurfaceClass::Lowland => 1.0 + elevation_m.max(0.0) / 1_000.0,
        SurfaceClass::Highland => 1.5 + elevation_m / 600.0,
        SurfaceClass::Alpine => 2.2 + elevation_m / 500.0,
""",
        "travel costs",
    )
    text = replace_exact(
        text,
        "greedy_land_path(start, port_cell, coord_set, drafts)",
        "slope_aware_land_path(start, port_cell, coord_set, drafts)",
        "road path invocation",
    )

    start = text.index("fn greedy_land_path(")
    end = text.index("\nfn generate_landmark(", start)
    slope_aware = r'''fn slope_aware_land_path(
    start: HexCoord,
    end: HexCoord,
    coord_set: &BTreeSet<HexCoord>,
    drafts: &BTreeMap<HexCoord, CellDraft>,
) -> Option<Vec<HexCoord>> {
    let mut frontier = BTreeSet::from([start]);
    let mut costs = BTreeMap::from([(start, 0.0_f64)]);
    let mut previous = BTreeMap::<HexCoord, HexCoord>::new();

    while !frontier.is_empty() {
        let current = *frontier.iter().min_by(|left, right| {
            costs[*left]
                .total_cmp(&costs[*right])
                .then_with(|| left.cmp(right))
        })?;
        frontier.remove(&current);
        if current == end {
            break;
        }

        for direction in HexDirection::ALL {
            let neighbor = current.neighbor(direction);
            if !coord_set.contains(&neighbor) || drafts[&neighbor].is_ocean() {
                continue;
            }

            let horizontal_distance = drafts[&current]
                .center_xz
                .distance(drafts[&neighbor].center_xz)
                .max(1.0);
            let grade = (drafts[&current].elevation_m - drafts[&neighbor].elevation_m).abs()
                / horizontal_distance;
            let edge_cost = 1.0 + drafts[&neighbor].travel_cost * 0.15 + grade.powi(2) * 60.0;
            let candidate_cost = costs[&current] + edge_cost;
            let incumbent = costs.get(&neighbor).copied().unwrap_or(f64::INFINITY);
            let predecessor_is_better = previous
                .get(&neighbor)
                .is_none_or(|incumbent_predecessor| current < *incumbent_predecessor);

            if candidate_cost < incumbent - 1.0e-12
                || ((candidate_cost - incumbent).abs() <= 1.0e-12
                    && predecessor_is_better)
            {
                costs.insert(neighbor, candidate_cost);
                previous.insert(neighbor, current);
                frontier.insert(neighbor);
            }
        }
    }

    if !costs.contains_key(&end) {
        return None;
    }
    let mut path = vec![end];
    let mut current = end;
    while current != start {
        current = *previous.get(&current)?;
        path.push(current);
    }
    path.reverse();
    Some(path)
}
'''
    text = text[:start] + slope_aware + text[end:]
    text = replace_exact(text, ">= 70.0", ">= 28.0", "vertical transition threshold")
    path.write_text(text, encoding="utf-8")


def refine_validator() -> None:
    path = ROOT / "crates" / "world_generation_core" / "src" / "validate.rs"
    text = path.read_text(encoding="utf-8")
    text = replace_exact(
        text,
        "const EPSILON: f64 = 1.0e-8;\n",
        "const EPSILON: f64 = 1.0e-8;\nconst MAX_ROAD_GRADE: f64 = 0.25;\n",
        "road grade constant",
    )
    text = replace_exact(
        text,
        """    #[error("road {0} is empty, disconnected, enters ocean, or misses the settlement")]
    InvalidRoad(RoadId),
""",
        """    #[error("road {0} is empty, disconnected, enters ocean, or misses the settlement")]
    InvalidRoad(RoadId),
    #[error(
        "road {road_id} exceeds the maximum grade between {left:?} and {right:?}: {grade:.3}"
    )]
    ExcessiveRoadGrade {
        road_id: RoadId,
        left: HexCoord,
        right: HexCoord,
        grade: f64,
    },
""",
        "road grade error",
    )

    old = """        {
            errors.push(WorldValidationError::InvalidRoad(road.id));
        }
    }

    let routes = manifest
"""
    new = """        {
            errors.push(WorldValidationError::InvalidRoad(road.id));
        }
        for pair in road.cells.windows(2) {
            let (Some(left), Some(right)) = (coords.get(&pair[0]), coords.get(&pair[1])) else {
                continue;
            };
            let delta_x = left.center_world.x - right.center_world.x;
            let delta_z = left.center_world.z - right.center_world.z;
            let horizontal_distance = delta_x.hypot(delta_z).max(1.0);
            let grade = (left.elevation_m - right.elevation_m).abs() / horizontal_distance;
            if grade > MAX_ROAD_GRADE + EPSILON {
                errors.push(WorldValidationError::ExcessiveRoadGrade {
                    road_id: road.id,
                    left: pair[0],
                    right: pair[1],
                    grade,
                });
            }
        }
    }

    let routes = manifest
"""
    text = replace_exact(text, old, new, "road grade validation")
    path.write_text(text, encoding="utf-8")


def refine_trace() -> None:
    path = ROOT / "apps" / "p4_world_trace" / "src" / "main.rs"
    text = path.read_text(encoding="utf-8")
    text = replace_exact(
        text,
        "    road_count: usize,\n",
        "    road_count: usize,\n    maximum_road_grade: f64,\n",
        "trace field",
    )
    text = replace_exact(
        text,
        "    let all_routes_reference_same_landmark = manifest.landmarks.len() == 1\n",
        """    let maximum_road_grade = manifest
        .roads
        .iter()
        .flat_map(|road| road.cells.windows(2))
        .filter_map(|pair| {
            let left = manifest.cell(pair[0])?;
            let right = manifest.cell(pair[1])?;
            let delta_x = left.center_world.x - right.center_world.x;
            let delta_z = left.center_world.z - right.center_world.z;
            let horizontal_distance = delta_x.hypot(delta_z).max(1.0);
            Some((left.elevation_m - right.elevation_m).abs() / horizontal_distance)
        })
        .fold(0.0_f64, f64::max);

    let all_routes_reference_same_landmark = manifest.landmarks.len() == 1
""",
        "trace grade calculation",
    )
    text = replace_exact(
        text,
        "        && manifest.roads.len() == 3\n",
        "        && manifest.roads.len() == 3\n        && maximum_road_grade <= 0.25\n",
        "trace grade threshold",
    )
    text = replace_exact(
        text,
        "        road_count: manifest.roads.len(),\n",
        "        road_count: manifest.roads.len(),\n        maximum_road_grade,\n",
        "trace grade output",
    )
    path.write_text(text, encoding="utf-8")


def main() -> int:
    refine_generator()
    refine_validator()
    refine_trace()
    print("Applied P4 deterministic geography refinement.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
