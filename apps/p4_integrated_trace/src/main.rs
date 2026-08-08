#![forbid(unsafe_code)]

use integrated_world_core::{IntegratedWorld, LandformClass, ValidationCheck};
use p4_integrated_scenario::{
    generate_integrated_baseline, generate_integrated_parity, generate_integrated_reverse,
};
use serde::Serialize;
use std::{env, error::Error, fs, path::PathBuf};

#[derive(Debug, Serialize)]
struct IntegratedTrace {
    integrated_fingerprint: u64,
    atlas_fingerprint: u64,
    detailed_fingerprint: u64,
    history_fingerprint: u64,
    traversal_fingerprint: u64,
    deterministic_across_p4a_traversal_orders: bool,
    complete_worlds_identical_across_orders: bool,
    atlas_cell_count: usize,
    atlas_feature_count: usize,
    boundary_contract_count: usize,
    detailed_cell_count: usize,
    detailed_sample_count: usize,
    region_relief_m: f64,
    represented_landforms: Vec<String>,
    history_event_count: usize,
    historical_asset_count: usize,
    land_use_zone_count: usize,
    route_count: usize,
    traversal_node_count: usize,
    traversal_edge_count: usize,
    route_target_matches_atlas_landmark: bool,
    all_cross_scale_checks_passed: bool,
    checks: Vec<ValidationCheck>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/p4-integrated-world-trace.json"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let canonical = generate_integrated_baseline()?;
    let reverse = generate_integrated_reverse()?;
    let parity = generate_integrated_parity()?;
    let deterministic = canonical.report.integrated_fingerprint
        == reverse.report.integrated_fingerprint
        && canonical.report.integrated_fingerprint == parity.report.integrated_fingerprint;
    let identical = canonical == reverse && canonical == parity;
    let trace = trace_for(&canonical, deterministic, identical);
    if !trace.all_cross_scale_checks_passed
        || !trace.deterministic_across_p4a_traversal_orders
        || !trace.complete_worlds_identical_across_orders
        || !trace.route_target_matches_atlas_landmark
        || trace.atlas_cell_count != 19
        || trace.detailed_cell_count < 7
        || trace.history_event_count < 5
        || trace.route_count != 3
        || trace.region_relief_m < 120.0
    {
        return Err("integrated P4 trace did not satisfy its stop-line thresholds".into());
    }

    fs::write(&output, serde_json::to_vec_pretty(&trace)?)?;
    println!("{}", serde_json::to_string_pretty(&trace)?);
    Ok(())
}

fn trace_for(world: &IntegratedWorld, deterministic: bool, identical: bool) -> IntegratedTrace {
    let valid_samples = world
        .detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| sample.cell.is_some())
        .collect::<Vec<_>>();
    let minimum = valid_samples
        .iter()
        .map(|sample| sample.world_position.y)
        .fold(f64::INFINITY, f64::min);
    let maximum = valid_samples
        .iter()
        .map(|sample| sample.world_position.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut landforms = valid_samples
        .iter()
        .map(|sample| sample.landform)
        .collect::<Vec<_>>();
    landforms.sort_by_key(|landform| landform_key(*landform));
    landforms.dedup();

    IntegratedTrace {
        integrated_fingerprint: world.report.integrated_fingerprint,
        atlas_fingerprint: world.report.atlas_fingerprint,
        detailed_fingerprint: world.report.detailed_fingerprint,
        history_fingerprint: world.report.history_fingerprint,
        traversal_fingerprint: world.report.traversal_fingerprint,
        deterministic_across_p4a_traversal_orders: deterministic,
        complete_worlds_identical_across_orders: identical,
        atlas_cell_count: world.atlas.cells.len(),
        atlas_feature_count: world.atlas.features.len(),
        boundary_contract_count: world.atlas.boundary_contracts.len(),
        detailed_cell_count: world.detailed.cells.len(),
        detailed_sample_count: valid_samples.len(),
        region_relief_m: maximum - minimum,
        represented_landforms: landforms
            .into_iter()
            .map(|landform| format!("{landform:?}"))
            .collect(),
        history_event_count: world.history.events.len(),
        historical_asset_count: world.history.assets.len(),
        land_use_zone_count: world.history.land_use.len(),
        route_count: world.traversal.routes.len(),
        traversal_node_count: world.traversal.nodes.len(),
        traversal_edge_count: world.traversal.edges.len(),
        route_target_matches_atlas_landmark: world.traversal.target_landmark_id
            == world.atlas.landmark_id
            && world
                .traversal
                .routes
                .iter()
                .all(|route| route.target_landmark_id == world.atlas.landmark_id),
        all_cross_scale_checks_passed: world.report.all_passed(),
        checks: world.report.checks.clone(),
    }
}

fn landform_key(landform: LandformClass) -> u8 {
    match landform {
        LandformClass::Ocean => 0,
        LandformClass::Coast => 1,
        LandformClass::Estuary => 2,
        LandformClass::Floodplain => 3,
        LandformClass::Valley => 4,
        LandformClass::Lowland => 5,
        LandformClass::Terrace => 6,
        LandformClass::Hillslope => 7,
        LandformClass::Ridge => 8,
        LandformClass::Mountain => 9,
    }
}
