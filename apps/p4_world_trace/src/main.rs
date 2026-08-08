#![forbid(unsafe_code)]

use p4_world_scenario::{
    generate_baseline, generate_parity_order, generate_reverse_order, invalid_config,
};
use scroll_camera_core::ScrollGrammar;
use serde::Serialize;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use world_generation_core::{SettlementReason, generate_world, validate_world};

#[derive(Debug, Serialize)]
struct WorldTraceReport {
    scenario: &'static str,
    canonical_fingerprint: u64,
    reverse_fingerprint: u64,
    parity_fingerprint: u64,
    deterministic_across_traversal_orders: bool,
    manifests_identical_across_orders: bool,
    invalid_config_rejected: bool,
    cell_count: usize,
    river_cell_count: usize,
    river_boundary_count: usize,
    coastline_boundary_count: usize,
    road_count: usize,
    route_count: usize,
    route_node_count: usize,
    portal_count: usize,
    settlement_count: usize,
    building_count: usize,
    landmark_count: usize,
    all_routes_reference_same_landmark: bool,
    port_has_all_siting_reasons: bool,
    required_scroll_grammar_count: usize,
    stage_digest_count: usize,
    structural_validation_passed: bool,
    passed: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/p4-world-trace.json"));

    let canonical = generate_baseline()?;
    let reverse = generate_reverse_order()?;
    let parity = generate_parity_order()?;
    let structural_validation_passed = validate_world(&canonical.manifest).is_ok();
    let invalid_config_rejected = generate_world(invalid_config()).is_err();
    let deterministic_across_traversal_orders = canonical.semantic_fingerprint
        == reverse.semantic_fingerprint
        && canonical.semantic_fingerprint == parity.semantic_fingerprint;
    let manifests_identical_across_orders = canonical.manifest == reverse.manifest
        && canonical.manifest == parity.manifest;

    let manifest = &canonical.manifest;
    let settlement = manifest.settlements.first();
    let building_count = settlement.map_or(0, |settlement| settlement.buildings.len());
    let required_reasons = [
        SettlementReason::FreshWater,
        SettlementReason::ShelteredCoast,
        SettlementReason::BuildableSlope,
        SettlementReason::RoadConvergence,
    ];
    let port_has_all_siting_reasons = settlement.is_some_and(|settlement| {
        required_reasons
            .iter()
            .all(|reason| settlement.reasons.contains(reason))
    });

    let all_routes_reference_same_landmark = manifest.landmarks.len() == 1
        && manifest.routes.iter().all(|route| {
            route.visible_landmark_ids.as_slice() == [manifest.landmarks[0].id]
        });
    let grammars = manifest
        .routes
        .iter()
        .flat_map(|route| route.nodes.iter().map(|node| node.grammar))
        .collect::<Vec<_>>();
    let required_scroll_grammar = [
        ScrollGrammar::StandardSideView,
        ScrollGrammar::LightDepth,
        ScrollGrammar::JunctionApproach,
        ScrollGrammar::JunctionDecision,
        ScrollGrammar::TurnCommit,
        ScrollGrammar::CameraReorientation,
        ScrollGrammar::VistaReveal,
        ScrollGrammar::VerticalTransition,
    ];
    let required_scroll_grammar_count = required_scroll_grammar
        .iter()
        .filter(|grammar| grammars.contains(grammar))
        .count();

    let passed = deterministic_across_traversal_orders
        && manifests_identical_across_orders
        && invalid_config_rejected
        && structural_validation_passed
        && manifest.cells.len() == 19
        && manifest.river.cells.len() >= 4
        && manifest.roads.len() == 3
        && manifest.routes.len() == 3
        && manifest.settlements.len() == 1
        && building_count >= 6
        && all_routes_reference_same_landmark
        && port_has_all_siting_reasons
        && required_scroll_grammar_count == required_scroll_grammar.len()
        && manifest.stage_digests.len() >= 5;

    let report = WorldTraceReport {
        scenario: "radius-two-river-valley-port-town",
        canonical_fingerprint: canonical.semantic_fingerprint,
        reverse_fingerprint: reverse.semantic_fingerprint,
        parity_fingerprint: parity.semantic_fingerprint,
        deterministic_across_traversal_orders,
        manifests_identical_across_orders,
        invalid_config_rejected,
        cell_count: manifest.cells.len(),
        river_cell_count: manifest.river.cells.len(),
        river_boundary_count: manifest.river.boundaries.len(),
        coastline_boundary_count: manifest.coastline.len(),
        road_count: manifest.roads.len(),
        route_count: manifest.routes.len(),
        route_node_count: manifest.routes.iter().map(|route| route.nodes.len()).sum(),
        portal_count: manifest.portals.len(),
        settlement_count: manifest.settlements.len(),
        building_count,
        landmark_count: manifest.landmarks.len(),
        all_routes_reference_same_landmark,
        port_has_all_siting_reasons,
        required_scroll_grammar_count,
        stage_digest_count: manifest.stage_digests.len(),
        structural_validation_passed,
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
        Err("P4 deterministic world thresholds failed".into())
    }
}
