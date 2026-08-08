#![forbid(unsafe_code)]

use building_core::{apply_delta, compile_blueprint, impact_for_delta};
use p3_building_scenario::{
    add_north_display_window_delta, inaccessible_upper_floor, moving_platform_binding,
    static_world_binding, two_storey_shop,
};
use serde::Serialize;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct BuildingTraceReport {
    scenario: &'static str,
    baseline_fingerprint: u64,
    repeated_fingerprint: u64,
    updated_fingerprint: u64,
    deterministic_repeat: bool,
    invalid_variant_rejected: bool,
    room_count: usize,
    exterior_entry_count: usize,
    portal_count: usize,
    collision_span_count: usize,
    navigation_patch_count: usize,
    mesh_chunk_count: usize,
    cutaway_group_count: usize,
    shell_element_count: usize,
    baseline_bounds_size_m: [f64; 3],
    updated_bounds_size_m: [f64; 3],
    massing_bounds_delta_m: f64,
    dirty_level_count: usize,
    dirty_room_count: usize,
    dirty_wall_count: usize,
    dirty_opening_count: usize,
    dirty_rebuild_collision: bool,
    dirty_rebuild_navigation: bool,
    dirty_rebuild_shell: bool,
    dirty_rebuild_cutaway: bool,
    dirty_rebuild_massing: bool,
    dirty_rebuild_hlod: bool,
    same_blueprint_static_and_mobile: bool,
    distinct_reference_frames: bool,
    passed: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/p3-building-trace.json"));

    let blueprint = two_storey_shop();
    let baseline = compile_blueprint(&blueprint)?;
    let repeated = compile_blueprint(&blueprint)?;
    let delta = add_north_display_window_delta();
    let dirty = impact_for_delta(&blueprint, &delta)?;
    let updated_blueprint = apply_delta(&blueprint, &delta)?;
    let updated = compile_blueprint(&updated_blueprint)?;

    let baseline_size = baseline.bounds.size();
    let updated_size = updated.bounds.size();
    let bounds_delta = (baseline.bounds.min - updated.bounds.min)
        .abs()
        .max_element()
        .max(
            (baseline.bounds.max - updated.bounds.max)
                .abs()
                .max_element(),
        );

    let static_binding = static_world_binding();
    let mobile_binding = moving_platform_binding();
    let deterministic_repeat = baseline.semantic_fingerprint == repeated.semantic_fingerprint;
    let invalid_variant_rejected = compile_blueprint(&inaccessible_upper_floor()).is_err();
    let same_blueprint_static_and_mobile = static_binding.building_id == mobile_binding.building_id
        && static_binding.building_id == baseline.building_id;
    let distinct_reference_frames = static_binding.frame_id != mobile_binding.frame_id;

    let passed = deterministic_repeat
        && invalid_variant_rejected
        && updated.semantic_fingerprint != baseline.semantic_fingerprint
        && bounds_delta <= 1.0e-9
        && dirty.levels.len() == 1
        && dirty.walls.len() == 1
        && dirty.openings.len() == 1
        && dirty.rebuild_exterior_shell
        && dirty.rebuild_cutaway_groups
        && dirty.rebuild_hlod
        && !dirty.rebuild_massing
        && !dirty.rebuild_collision
        && !dirty.rebuild_navigation
        && same_blueprint_static_and_mobile
        && distinct_reference_frames
        && baseline.room_graph.adjacency.len() == 4
        && baseline.room_graph.exterior_entries.len() == 1
        && !baseline.mesh_chunks.is_empty()
        && !baseline.cutaway_groups.is_empty();

    let report = BuildingTraceReport {
        scenario: "two-storey-shop",
        baseline_fingerprint: baseline.semantic_fingerprint,
        repeated_fingerprint: repeated.semantic_fingerprint,
        updated_fingerprint: updated.semantic_fingerprint,
        deterministic_repeat,
        invalid_variant_rejected,
        room_count: baseline.room_graph.adjacency.len(),
        exterior_entry_count: baseline.room_graph.exterior_entries.len(),
        portal_count: baseline.portal_graph.edges.len(),
        collision_span_count: baseline.collision.wall_spans.len(),
        navigation_patch_count: baseline.navigation.len(),
        mesh_chunk_count: baseline.mesh_chunks.len(),
        cutaway_group_count: baseline.cutaway_groups.len(),
        shell_element_count: baseline.exterior_shell.elements.len(),
        baseline_bounds_size_m: [baseline_size.x, baseline_size.y, baseline_size.z],
        updated_bounds_size_m: [updated_size.x, updated_size.y, updated_size.z],
        massing_bounds_delta_m: bounds_delta,
        dirty_level_count: dirty.levels.len(),
        dirty_room_count: dirty.rooms.len(),
        dirty_wall_count: dirty.walls.len(),
        dirty_opening_count: dirty.openings.len(),
        dirty_rebuild_collision: dirty.rebuild_collision,
        dirty_rebuild_navigation: dirty.rebuild_navigation,
        dirty_rebuild_shell: dirty.rebuild_exterior_shell,
        dirty_rebuild_cutaway: dirty.rebuild_cutaway_groups,
        dirty_rebuild_massing: dirty.rebuild_massing,
        dirty_rebuild_hlod: dirty.rebuild_hlod,
        same_blueprint_static_and_mobile,
        distinct_reference_frames,
        passed,
    };

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("wrote {}", output.display());
    println!("{report:#?}");

    if passed {
        Ok(())
    } else {
        Err("P3 building compiler thresholds failed".into())
    }
}
