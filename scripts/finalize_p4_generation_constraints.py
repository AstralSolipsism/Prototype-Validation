#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: Path, old: str, new: str, marker: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old in text:
        text = text.replace(old, new, 1)
    elif marker not in text:
        raise RuntimeError(f"{label}: neither original nor finalized form was found")
    path.write_text(text, encoding="utf-8")


def finalize_terrain_flow() -> None:
    path = ROOT / "crates/integrated_world_core/src/terrain.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "    accumulate_flow(width, height, &mut samples);",
        "    accumulate_flow(width, height, atlas.cell_radius_m, &mut samples);",
        1,
    )
    text = text.replace(
        "fn accumulate_flow(width: usize, height: usize, samples: &mut [TerrainSample]) {",
        "fn accumulate_flow(\n"
        "    width: usize,\n"
        "    height: usize,\n"
        "    cell_radius_m: f64,\n"
        "    samples: &mut [TerrainSample],\n"
        ") {",
        1,
    )

    old = '''    let maximum = samples
        .iter()
        .map(|sample| sample.flow_accumulation)
        .fold(1.0_f64, f64::max)
        .ln_1p();
    for sample in samples.iter_mut().filter(|sample| sample.cell.is_some()) {
        sample.flow_accumulation = sample.flow_accumulation.ln_1p() / maximum;
        let distance_to_river =
            (sample.world_position.z - river_center_z(sample.world_position.x, 128.0)).abs();
        if sample.flow_accumulation > 0.72
            && distance_to_river < 38.0
            && sample.world_position.y > 0.0
        {
            sample.landform = if sample.world_position.y < 8.0 {
                LandformClass::Estuary
            } else {
                LandformClass::Floodplain
            };
        }
    }
'''
    new = '''    let maximum = samples
        .iter()
        .map(|sample| sample.flow_accumulation)
        .fold(1.0_f64, f64::max)
        .ln_1p();
    for sample in samples.iter_mut().filter(|sample| sample.cell.is_some()) {
        sample.flow_accumulation = sample.flow_accumulation.ln_1p() / maximum;
    }

    let elevations = samples
        .iter()
        .map(|sample| sample.world_position.y)
        .collect::<Vec<_>>();
    let occupied = samples
        .iter()
        .map(|sample| sample.cell.is_some())
        .collect::<Vec<_>>();
    for index in 0..samples.len() {
        if !occupied[index] {
            continue;
        }
        let x = index % width;
        let z = index / width;
        let mut local_minimum = elevations[index];
        for dz in -1isize..=1 {
            for dx in -1isize..=1 {
                let nx = x as isize + dx;
                let nz = z as isize + dz;
                if nx < 0 || nz < 0 || nx >= width as isize || nz >= height as isize {
                    continue;
                }
                let neighbor = nz as usize * width + nx as usize;
                if occupied[neighbor] {
                    local_minimum = local_minimum.min(elevations[neighbor]);
                }
            }
        }

        let elevation = elevations[index];
        let near_local_minimum = elevation <= local_minimum + 4.0;
        let sample = &mut samples[index];
        let distance_to_river = (sample.world_position.z
            - river_center_z(sample.world_position.x, cell_radius_m))
        .abs();
        if sample.flow_accumulation > 0.72
            && distance_to_river < cell_radius_m * 0.30
            && elevation > 0.0
            && near_local_minimum
        {
            sample.landform = if elevation < 8.0 {
                LandformClass::Estuary
            } else {
                LandformClass::Floodplain
            };
        } else if sample.flow_accumulation > 0.55
            && matches!(
                sample.landform,
                LandformClass::Valley | LandformClass::Floodplain | LandformClass::Estuary
            )
            && !near_local_minimum
        {
            sample.landform = if sample.slope > 0.22 {
                LandformClass::Hillslope
            } else {
                LandformClass::Lowland
            };
        }
    }
'''
    if old in text:
        text = text.replace(old, new, 1)
    elif "let near_local_minimum = elevation <= local_minimum + 4.0;" not in text:
        raise RuntimeError("Terrain flow classification block could not be finalized")
    path.write_text(text, encoding="utf-8")


def finalize_traversal_generation() -> None:
    path = ROOT / "crates/integrated_world_core/src/traversal.rs"
    text = path.read_text(encoding="utf-8")
    old = '''            let step = grid.samples[current.index]
                .world_position
                .distance(sample.world_position)
                * corridor_modifier(history, sample.world_position);
'''
    new = '''            let current_sample = &grid.samples[current.index];
            let horizontal = current_sample
                .world_position
                .xz()
                .distance(sample.world_position.xz())
                .max(0.01);
            let grade = (current_sample.world_position.y - sample.world_position.y).abs()
                / horizontal;
            if grade > 0.58 + 1.0e-9 {
                continue;
            }
            let step = current_sample
                .world_position
                .distance(sample.world_position)
                * corridor_modifier(history, sample.world_position);
'''
    if old in text:
        text = text.replace(old, new, 1)
    elif "if grade > 0.58 + 1.0e-9" not in text:
        raise RuntimeError("Traversal edge-grade gate could not be finalized")

    if "fn simplify_path(_grid: &TerrainGrid" not in text:
        text, count = re.subn(
            r"fn simplify_path\(.*?\n}\n\n(?=fn route_grammars)",
            '''fn simplify_path(_grid: &TerrainGrid, path: &[usize]) -> Vec<usize> {
    let mut detailed_path = path.to_vec();
    detailed_path.dedup();
    detailed_path
}

''',
            text,
            count=1,
            flags=re.DOTALL,
        )
        if count != 1:
            raise RuntimeError("Traversal dense-path preservation could not be finalized")

    old = '''    let grades_valid = traversal
        .routes
        .iter()
        .all(|route| route.maximum_grade <= 0.58 + 1.0e-9);
'''
    new = '''    let route_grades = traversal
        .routes
        .iter()
        .map(|route| route.maximum_grade)
        .collect::<Vec<_>>();
    let grades_valid = route_grades
        .iter()
        .all(|grade| *grade <= 0.58 + 1.0e-9);
'''
    if old in text:
        text = text.replace(old, new, 1)

    old = '''    let multiple_cells = traversal.routes.iter().all(|route| {
        route
            .crossed_cells
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            >= 3
    });
'''
    new = '''    let route_cell_counts = traversal
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
'''
    if old in text:
        text = text.replace(old, new, 1)

    text = text.replace(
        'detail: "all route segments remain below the prototype maximum grade".into(),',
        'detail: format!("route maximum grades are {route_grades:?}; budget is 0.58"),',
        1,
    )
    text = text.replace(
        'detail: "each route crosses at least three materialized Atlas cells".into(),',
        'detail: format!("route unique materialized-cell counts are {route_cell_counts:?}"),',
        1,
    )
    path.write_text(text, encoding="utf-8")


def finalize_validation_diagnostics() -> None:
    path = ROOT / "crates/integrated_world_core/src/validate.rs"
    text = path.read_text(encoding="utf-8")
    old = '''    let river_is_low = detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| {
            sample.cell.is_some()
                && matches!(
                    sample.landform,
                    LandformClass::Floodplain | LandformClass::Valley | LandformClass::Estuary
                )
                && sample.flow_accumulation > 0.55
        })
        .all(|sample| {
            neighborhood_minimum(&detailed.terrain, sample.grid_x, sample.grid_z)
                >= sample.world_position.y - 4.0
        });
'''
    new = '''    let river_height_offsets = detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| {
            sample.cell.is_some()
                && matches!(
                    sample.landform,
                    LandformClass::Floodplain | LandformClass::Valley | LandformClass::Estuary
                )
                && sample.flow_accumulation > 0.55
        })
        .map(|sample| {
            sample.world_position.y
                - neighborhood_minimum(&detailed.terrain, sample.grid_x, sample.grid_z)
        })
        .collect::<Vec<_>>();
    let worst_river_height_offset = river_height_offsets
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);
    let river_is_low = river_height_offsets
        .iter()
        .all(|offset| *offset <= 4.0 + 1.0e-9);
'''
    if old in text:
        text = text.replace(old, new, 1)
    elif "let river_height_offsets" not in text:
        raise RuntimeError("River validation diagnostics could not be finalized")
    text = text.replace(
        'detail: "high-accumulation valley and estuary samples remain near local terrain minima"\n                .into(),',
        'detail: format!(\n'
        '                "{} high-flow river samples; worst local-height offset is {:.3} m",\n'
        '                river_height_offsets.len(), worst_river_height_offset\n'
        '            ),',
        1,
    )
    path.write_text(text, encoding="utf-8")


def main() -> int:
    finalize_terrain_flow()
    finalize_traversal_generation()
    finalize_validation_diagnostics()
    print("Integrated P4 generation constraints finalized.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
