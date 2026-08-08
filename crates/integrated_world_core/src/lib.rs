#![forbid(unsafe_code)]
#![allow(unused_variables, clippy::collapsible_if, clippy::needless_lifetimes)]

mod atlas;
mod history;
mod model;
mod terrain;
mod traversal;
mod validate;

pub use atlas::{
    build_atlas, classify_landform, detailed_height, detailed_slope, edge_endpoints, hex_corners,
    nearest_cell, point_in_hex, river_center_z,
};
pub use history::{apply_history_to_atlas, generate_history, validate_history};
pub use model::*;
pub use terrain::{default_materialized_cells, materialize_region, summarize_samples};
pub use traversal::{compile_traversal, validate_traversal};
pub use validate::build_cross_scale_report;

use thiserror::Error;
use world_generation_core::WorldCompilation;

#[derive(Debug, Error)]
pub enum IntegratedWorldError {
    #[error("integrated world serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("integrated world validation failed: {0:?}")]
    Validation(Vec<String>),
}

pub fn compile_integrated_world(
    base: &WorldCompilation,
) -> Result<IntegratedWorld, IntegratedWorldError> {
    let mut atlas = build_atlas(base)?;
    let materialized_cells = default_materialized_cells(&atlas);
    let detailed = materialize_region(&atlas, &materialized_cells)?;
    let history = generate_history(&atlas, &detailed, &base.manifest)?;
    apply_history_to_atlas(&mut atlas, &history)?;
    let traversal = compile_traversal(&atlas, &detailed, &history)?;
    let report = build_cross_scale_report(&atlas, &detailed, &history, &traversal, &base.manifest)?;
    let failed = report
        .checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| format!("{}: {}", check.name, check.detail))
        .collect::<Vec<_>>();
    if !failed.is_empty() {
        return Err(IntegratedWorldError::Validation(failed));
    }
    let building_instances = base
        .manifest
        .settlements
        .iter()
        .flat_map(|settlement| {
            settlement
                .buildings
                .iter()
                .map(|building| building.instance_id)
        })
        .collect();
    Ok(IntegratedWorld {
        atlas,
        detailed,
        history,
        traversal,
        report,
        building_instances,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use world_generation_core::{
        GeneratorVersion, TraversalOrder, WorldGenerationConfig, generate_world_with_order,
    };
    use world_ids::{BuildingId, WorldId};

    fn base_world(order: TraversalOrder) -> WorldCompilation {
        generate_world_with_order(
            WorldGenerationConfig {
                world_id: WorldId::from_u128(0x5044),
                world_seed: 0x706f_7274_2d76_616c_6c65_792d_3031,
                generator_version: GeneratorVersion(1),
                radius: 2,
                cell_radius_m: 128.0,
                building_blueprint_id: BuildingId::from_u128(1),
            },
            order,
        )
        .expect("base world")
    }

    #[test]
    fn integrated_pipeline_is_deterministic_across_base_traversal_orders() {
        let canonical = compile_integrated_world(&base_world(TraversalOrder::Canonical))
            .expect("canonical integrated world");
        let reverse = compile_integrated_world(&base_world(TraversalOrder::Reverse))
            .expect("reverse integrated world");
        let parity = compile_integrated_world(&base_world(TraversalOrder::Parity))
            .expect("parity integrated world");
        assert_eq!(
            canonical.report.integrated_fingerprint,
            reverse.report.integrated_fingerprint
        );
        assert_eq!(
            canonical.report.integrated_fingerprint,
            parity.report.integrated_fingerprint
        );
        assert_eq!(canonical, reverse);
        assert_eq!(canonical, parity);
    }

    #[test]
    fn integrated_world_round_trips_without_identity_loss() {
        let world = compile_integrated_world(&base_world(TraversalOrder::Canonical))
            .expect("integrated world");
        let encoded = serde_json::to_vec(&world).expect("serialize");
        let decoded: IntegratedWorld = serde_json::from_slice(&encoded).expect("deserialize");
        assert_eq!(decoded, world);
        assert!(decoded.report.all_passed());
        assert_eq!(
            decoded.traversal.target_landmark_id,
            decoded.atlas.landmark_id
        );
    }
}
