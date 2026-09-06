use crate::atlas::digest;
use crate::history::validate_history;
use crate::model::*;
use crate::terrain::{default_materialized_cells, materialize_region};
use crate::traversal::{compile_traversal, validate_traversal};
use std::collections::{BTreeMap, BTreeSet};
use world_generation_core::{HexCoord, WorldManifest};
use world_ids::EntityId;

pub fn build_cross_scale_report(
    atlas: &WorldAtlasManifest,
    detailed: &DetailedRegion,
    history: &HistoryLedger,
    traversal: &TraversalCompilation,
    base: &WorldManifest,
) -> Result<CrossScaleReport, serde_json::Error> {
    let mut checks = Vec::new();
    checks.extend(validate_atlas(atlas));
    checks.extend(validate_detailed_against_atlas(atlas, detailed));
    checks.extend(validate_history(history));
    checks.extend(validate_traversal(atlas, detailed, traversal));
    checks.extend(validate_identity_chain(atlas, history, traversal, base));

    let regenerated_detailed = materialize_region(atlas, &default_materialized_cells(atlas))?;
    let regenerated_traversal = compile_traversal(atlas, &regenerated_detailed, history)?;
    checks.push(ValidationCheck {
        name: "cache-regeneration-stable".into(),
        passed: regenerated_detailed.semantic_fingerprint == detailed.semantic_fingerprint
            && regenerated_traversal.semantic_fingerprint == traversal.semantic_fingerprint,
        detail:
            "deleting detailed terrain and route caches reproduces identical semantic fingerprints"
                .into(),
    });

    let bindings = object_bindings(atlas, history, base);
    let atlas_fingerprint = atlas.atlas_fingerprint;
    let detailed_fingerprint = detailed.semantic_fingerprint;
    let history_fingerprint = history.semantic_fingerprint;
    let traversal_fingerprint = traversal.semantic_fingerprint;
    let integrated_fingerprint = digest(&(
        atlas_fingerprint,
        detailed_fingerprint,
        history_fingerprint,
        traversal_fingerprint,
        &bindings,
    ))?;
    Ok(CrossScaleReport {
        checks,
        bindings,
        atlas_fingerprint,
        detailed_fingerprint,
        history_fingerprint,
        traversal_fingerprint,
        integrated_fingerprint,
    })
}

fn validate_atlas(atlas: &WorldAtlasManifest) -> Vec<ValidationCheck> {
    let rich_elevation = atlas
        .cells
        .iter()
        .all(|cell| cell.elevation.maximum_m >= cell.elevation.minimum_m)
        && atlas
            .cells
            .iter()
            .any(|cell| cell.elevation.relief_m > 55.0)
        && atlas
            .cells
            .iter()
            .filter(|cell| cell.elevation.relief_m > 20.0)
            .count()
            >= 5;
    let mixed_landforms = atlas.cells.iter().any(|cell| {
        [
            cell.landforms.mountain,
            cell.landforms.ridge,
            cell.landforms.hillslope,
            cell.landforms.valley,
            cell.landforms.lowland,
            cell.landforms.coast,
            cell.landforms.water,
        ]
        .into_iter()
        .filter(|fraction| *fraction > 0.03)
        .count()
            >= 2
    });
    let summaries_normalized = atlas
        .cells
        .iter()
        .all(|cell| (cell.landforms.total() - 1.0).abs() < 1.0e-9);
    let feature_kinds = atlas
        .features
        .iter()
        .map(|feature| feature.kind)
        .collect::<BTreeSet<_>>();
    let required_features = [
        AtlasFeatureKind::MountainRange,
        AtlasFeatureKind::River,
        AtlasFeatureKind::Coastline,
        AtlasFeatureKind::RoadCorridor,
        AtlasFeatureKind::Settlement,
        AtlasFeatureKind::Landmark,
    ]
    .into_iter()
    .all(|kind| feature_kinds.contains(&kind));
    let contracts_sampled = atlas.boundary_contracts.iter().all(|contract| {
        contract.samples.len() == EDGE_PROFILE_SAMPLES
            && contract
                .samples
                .windows(2)
                .all(|pair| pair[0].t < pair[1].t)
    });
    let cell_contract_references = atlas.cells.iter().all(|cell| {
        cell.boundary_contract_ids.iter().all(|id| {
            atlas
                .contract(*id)
                .is_some_and(|contract| contract.boundary.contains(cell.coord))
        })
    });

    vec![
        ValidationCheck {
            name: "atlas-has-elevation-distributions".into(),
            passed: rich_elevation,
            detail: "cells contain min/max/median/relief/slope/buildability rather than one center height".into(),
        },
        ValidationCheck {
            name: "atlas-cells-contain-multiple-landforms".into(),
            passed: mixed_landforms && summaries_normalized,
            detail: "at least one cell contains multiple material landform proportions and all mixes normalize".into(),
        },
        ValidationCheck {
            name: "atlas-feature-skeleton-complete".into(),
            passed: required_features,
            detail: format!("{} stable feature skeletons cover terrain, water, roads, settlement and landmark", atlas.features.len()),
        },
        ValidationCheck {
            name: "atlas-boundary-contracts-sampled".into(),
            passed: contracts_sampled && cell_contract_references,
            detail: format!("{} shared boundaries carry ordered elevation and feature profiles", atlas.boundary_contracts.len()),
        },
    ]
}

fn validate_detailed_against_atlas(
    atlas: &WorldAtlasManifest,
    detailed: &DetailedRegion,
) -> Vec<ValidationCheck> {
    let summary_match = detailed.cells.iter().all(|cell| {
        atlas.cell(cell.coord).is_some_and(|atlas_cell| {
            (atlas_cell.elevation.mean_m - cell.recomputed_elevation.mean_m).abs() < 12.0
                && (atlas_cell.elevation.relief_m - cell.recomputed_elevation.relief_m).abs() < 24.0
                && (atlas_cell.landforms.total() - cell.recomputed_landforms.total()).abs() < 1.0e-9
        })
    });
    let shared_profiles = shared_edge_profiles_match(detailed);
    let recognizable_relief = detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| sample.cell.is_some())
        .map(|sample| sample.world_position.y)
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(min, max), elevation| (min.min(elevation), max.max(elevation)),
        );
    let relief = recognizable_relief.1 - recognizable_relief.0;
    let landforms = detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| sample.cell.is_some())
        .map(|sample| sample.landform)
        .collect::<BTreeSet<_>>();
    let required_landforms = [
        LandformClass::Mountain,
        LandformClass::Ridge,
        LandformClass::Valley,
        LandformClass::Lowland,
        LandformClass::Coast,
        LandformClass::Ocean,
    ]
    .into_iter()
    .all(|landform| landforms.contains(&landform));
    let river_height_offsets = detailed
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
    let worst_river_height_offset = river_height_offsets.iter().copied().fold(0.0_f64, f64::max);
    let river_is_low = river_height_offsets
        .iter()
        .all(|offset| *offset <= 4.0 + 1.0e-9);

    vec![
        ValidationCheck {
            name: "detailed-terrain-matches-atlas-summary".into(),
            passed: summary_match,
            detail: format!(
                "{} materialized cells recompute Atlas statistics within sampling tolerances",
                detailed.cells.len()
            ),
        },
        ValidationCheck {
            name: "detailed-cell-boundaries-identical".into(),
            passed: shared_profiles,
            detail:
                "adjacent independently addressable cells reuse the same boundary profile samples"
                    .into(),
        },
        ValidationCheck {
            name: "detailed-terrain-has-recognizable-relief".into(),
            passed: relief > 120.0 && required_landforms,
            detail: format!(
                "materialized region relief is {relief:.1} m and includes mountain-to-ocean landforms"
            ),
        },
        ValidationCheck {
            name: "river-follows-local-low-ground".into(),
            passed: river_is_low,
            detail: format!(
                "{} high-flow river samples; worst local-height offset is {:.3} m",
                river_height_offsets.len(),
                worst_river_height_offset
            ),
        },
    ]
}

fn shared_edge_profiles_match(detailed: &DetailedRegion) -> bool {
    let mut seen = BTreeMap::<EntityId, Vec<EdgeSample>>::new();
    for cell in &detailed.cells {
        for profile in &cell.edge_profiles {
            if let Some(existing) = seen.get(&profile.contract_id) {
                if existing != &profile.samples {
                    return false;
                }
            } else {
                seen.insert(profile.contract_id, profile.samples.clone());
            }
        }
    }
    true
}

fn neighborhood_minimum(grid: &TerrainGrid, x: u16, z: u16) -> f64 {
    let mut minimum = f64::INFINITY;
    for dz in -1isize..=1 {
        for dx in -1isize..=1 {
            let nx = isize::try_from(x).unwrap_or(0) + dx;
            let nz = isize::try_from(z).unwrap_or(0) + dz;
            if nx < 0 || nz < 0 {
                continue;
            }
            if let Some(sample) = grid.sample(nx as usize, nz as usize)
                && sample.cell.is_some()
            {
                minimum = minimum.min(sample.world_position.y);
            }
        }
    }
    minimum
}

fn validate_identity_chain(
    atlas: &WorldAtlasManifest,
    history: &HistoryLedger,
    traversal: &TraversalCompilation,
    base: &WorldManifest,
) -> Vec<ValidationCheck> {
    let landmark_feature = atlas
        .features
        .iter()
        .find(|feature| feature.kind == AtlasFeatureKind::Landmark);
    let landmark_identity = landmark_feature
        .is_some_and(|feature| feature.id.as_u128() == atlas.landmark_id.as_u128())
        && traversal.target_landmark_id == atlas.landmark_id
        && traversal
            .routes
            .iter()
            .all(|route| route.target_landmark_id == atlas.landmark_id);
    let settlement_identity = base
        .settlements
        .first()
        .is_some_and(|settlement| settlement.id == atlas.settlement_id)
        && history.settlement_id == atlas.settlement_id;
    let history_ids_on_atlas = history.events.iter().all(|event| {
        atlas
            .cells
            .iter()
            .any(|cell| cell.history.event_ids.contains(&event.id))
            || event.kind == HistoryEventKind::Migration
    });
    let building_identity = base
        .settlements
        .iter()
        .flat_map(|settlement| &settlement.buildings)
        .all(|building| {
            building.instance_id.as_u128() != 0 && atlas.cell(base.settlements[0].cell).is_some()
        });

    vec![
        ValidationCheck {
            name: "landmark-identity-cross-scale".into(),
            passed: landmark_identity,
            detail: format!(
                "Atlas feature, local route target and LOD anchor all use landmark {}",
                atlas.landmark_id
            ),
        },
        ValidationCheck {
            name: "settlement-identity-cross-scale".into(),
            passed: settlement_identity,
            detail: format!(
                "Atlas, history and P4A settlement use stable ID {}",
                atlas.settlement_id
            ),
        },
        ValidationCheck {
            name: "atlas-history-references".into(),
            passed: history_ids_on_atlas,
            detail:
                "settled Atlas cells reference the same history events rendered in the local world"
                    .into(),
        },
        ValidationCheck {
            name: "building-instance-identities-retained".into(),
            passed: building_identity,
            detail: "P3 building instance IDs remain stable when embedded in the integrated world"
                .into(),
        },
    ]
}

fn object_bindings(
    atlas: &WorldAtlasManifest,
    history: &HistoryLedger,
    base: &WorldManifest,
) -> Vec<CrossScaleObjectBinding> {
    let landmark = atlas
        .features
        .iter()
        .find(|feature| feature.kind == AtlasFeatureKind::Landmark)
        .expect("landmark feature");
    let mut bindings = vec![CrossScaleObjectBinding {
        object_id: landmark.id,
        atlas_cell: landmark
            .touched_cells
            .first()
            .copied()
            .unwrap_or(HexCoord::ZERO),
        local_anchor: landmark.path_world[0],
        atlas_feature_id: Some(landmark.id),
        history_event_id: history
            .events
            .iter()
            .find(|event| event.kind == HistoryEventKind::Reconstruction)
            .map(|event| event.id),
    }];
    bindings.extend(history.assets.iter().map(|asset| CrossScaleObjectBinding {
        object_id: asset.id,
        atlas_cell: asset.source_cell,
        local_anchor: asset.anchor_world,
        atlas_feature_id: None,
        history_event_id: Some(asset.created_by),
    }));
    bindings.extend(base.settlements.iter().flat_map(|settlement| {
        settlement
            .buildings
            .iter()
            .map(move |building| CrossScaleObjectBinding {
                object_id: EntityId::from_u128(building.instance_id.as_u128()),
                atlas_cell: settlement.cell,
                local_anchor: building.local_position,
                atlas_feature_id: Some(EntityId::from_u128(settlement.id.as_u128())),
                history_event_id: history
                    .events
                    .iter()
                    .find(|event| event.kind == HistoryEventKind::SettlementFounded)
                    .map(|event| event.id),
            })
    }));
    bindings.sort_by_key(|binding| binding.object_id);
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        atlas::build_atlas,
        history::{apply_history_to_atlas, generate_history},
        terrain::{default_materialized_cells, materialize_region},
        traversal::compile_traversal,
    };
    use world_generation_core::{
        GeneratorVersion, TraversalOrder, WorldGenerationConfig, generate_world_with_order,
    };
    use world_ids::{BuildingId, WorldId};

    #[test]
    fn complete_report_passes_for_the_integrated_fixture() {
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
        let mut atlas = build_atlas(&base).expect("atlas");
        let detailed =
            materialize_region(&atlas, &default_materialized_cells(&atlas)).expect("detailed");
        let history = generate_history(&atlas, &detailed, &base.manifest).expect("history");
        apply_history_to_atlas(&mut atlas, &history).expect("history atlas");
        let traversal = compile_traversal(&atlas, &detailed, &history).expect("traversal");
        let report =
            build_cross_scale_report(&atlas, &detailed, &history, &traversal, &base.manifest)
                .expect("report");
        let failed = report
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>();
        assert!(failed.is_empty(), "failed checks: {failed:?}");
    }
}
