use crate::atlas::{digest, river_center_z, stable_entity};
use crate::model::*;
use glam::{DVec2, DVec3};
use std::collections::BTreeSet;
use world_generation_core::{HexCoord, WorldManifest};
use world_ids::{EntityId, EventId, RegionId};

const STAGE_HISTORY: u64 = 0x4300;
const EVENT_MIGRATION: u128 = 0x01;
const EVENT_FOUNDATION: u128 = 0x02;
const EVENT_HARBOR: u128 = 0x03;
const EVENT_AGRICULTURE: u128 = 0x04;
const EVENT_ROADS: u128 = 0x05;
const EVENT_FORTIFICATION: u128 = 0x06;
const EVENT_CONFLICT: u128 = 0x07;
const EVENT_FLOOD: u128 = 0x08;
const EVENT_RECONSTRUCTION: u128 = 0x09;

pub fn generate_history(
    atlas: &WorldAtlasManifest,
    detailed: &DetailedRegion,
    base: &WorldManifest,
) -> Result<HistoryLedger, serde_json::Error> {
    let settlement = base.settlements.first().expect("P4A contains a settlement");
    let port_cell = settlement.cell;
    let port_anchor = choose_port_anchor(atlas, detailed, port_cell);
    let farmland_anchor = choose_farmland_anchor(detailed, port_anchor);
    let defensive_anchor = choose_defensive_anchor(detailed, port_anchor);
    let flood_anchor = DVec3::new(
        port_anchor.x - 36.0,
        terrain_height_near(
            detailed,
            DVec2::new(port_anchor.x - 36.0, port_anchor.z + 24.0),
        ),
        port_anchor.z + 24.0,
    );
    let bridge_anchor = DVec3::new(
        port_anchor.x - 48.0,
        terrain_height_near(
            detailed,
            DVec2::new(
                port_anchor.x - 48.0,
                river_center_z(port_anchor.x - 48.0, atlas.cell_radius_m),
            ),
        ) + 1.5,
        river_center_z(port_anchor.x - 48.0, atlas.cell_radius_m),
    );

    let event = |key: u128| event_id(atlas.world_seed, key);
    let migration = event(EVENT_MIGRATION);
    let foundation = event(EVENT_FOUNDATION);
    let harbor = event(EVENT_HARBOR);
    let agriculture = event(EVENT_AGRICULTURE);
    let roads = event(EVENT_ROADS);
    let fortification = event(EVENT_FORTIFICATION);
    let conflict = event(EVENT_CONFLICT);
    let flood = event(EVENT_FLOOD);
    let reconstruction = event(EVENT_RECONSTRUCTION);

    let harbor_asset = asset_id(atlas.world_seed, 0x101);
    let bridge_asset = asset_id(atlas.world_seed, 0x102);
    let old_road_asset = asset_id(atlas.world_seed, 0x103);
    let new_road_asset = asset_id(atlas.world_seed, 0x104);
    let old_quarter_asset = asset_id(atlas.world_seed, 0x105);
    let new_quarter_asset = asset_id(atlas.world_seed, 0x106);
    let farmland_asset = asset_id(atlas.world_seed, 0x107);
    let fortification_asset = asset_id(atlas.world_seed, 0x108);
    let ruins_asset = asset_id(atlas.world_seed, 0x109);
    let monument_asset = asset_id(atlas.world_seed, 0x10a);

    let mut events = vec![
        HistoryEvent {
            id: migration,
            year: 0,
            kind: HistoryEventKind::Migration,
            location: farmland_anchor,
            causes: Vec::new(),
            created_asset_ids: Vec::new(),
            retired_asset_ids: Vec::new(),
            description: "Families follow the river valley toward fresh water and the sheltered bay.".into(),
        },
        HistoryEvent {
            id: foundation,
            year: 18,
            kind: HistoryEventKind::SettlementFounded,
            location: port_anchor,
            causes: vec![migration],
            created_asset_ids: vec![old_quarter_asset],
            retired_asset_ids: Vec::new(),
            description: "A permanent river-mouth settlement is founded on buildable low ground above the floodplain.".into(),
        },
        HistoryEvent {
            id: harbor,
            year: 46,
            kind: HistoryEventKind::HarborConstructed,
            location: DVec3::new(port_anchor.x + 40.0, 1.5, port_anchor.z),
            causes: vec![foundation],
            created_asset_ids: vec![harbor_asset],
            retired_asset_ids: Vec::new(),
            description: "Fishing and coastal trade justify a protected harbor and loading quay.".into(),
        },
        HistoryEvent {
            id: agriculture,
            year: 61,
            kind: HistoryEventKind::AgricultureExpanded,
            location: farmland_anchor,
            causes: vec![foundation],
            created_asset_ids: vec![farmland_asset],
            retired_asset_ids: Vec::new(),
            description: "Floodplain soils are drained and organized into fields supplying the growing port.".into(),
        },
        HistoryEvent {
            id: roads,
            year: 83,
            kind: HistoryEventKind::RoadAndBridgeBuilt,
            location: bridge_anchor,
            causes: vec![harbor, agriculture],
            created_asset_ids: vec![bridge_asset, old_road_asset],
            retired_asset_ids: Vec::new(),
            description: "A bridge and valley road connect farms, harbor, uplands and inland trade routes.".into(),
        },
        HistoryEvent {
            id: fortification,
            year: 121,
            kind: HistoryEventKind::FortificationRaised,
            location: defensive_anchor,
            causes: vec![roads],
            created_asset_ids: vec![fortification_asset],
            retired_asset_ids: Vec::new(),
            description: "A ridge fort protects the road convergence and watches the harbor approach.".into(),
        },
        HistoryEvent {
            id: conflict,
            year: 164,
            kind: HistoryEventKind::Conflict,
            location: defensive_anchor,
            causes: vec![fortification],
            created_asset_ids: vec![ruins_asset],
            retired_asset_ids: vec![fortification_asset],
            description: "A siege destroys the first ridge fort and leaves a visible ruin above the town.".into(),
        },
        HistoryEvent {
            id: flood,
            year: 197,
            kind: HistoryEventKind::Flood,
            location: flood_anchor,
            causes: vec![roads],
            created_asset_ids: Vec::new(),
            retired_asset_ids: vec![old_road_asset],
            description: "A major flood abandons the lowest road and forces the town to rebuild on terraces.".into(),
        },
        HistoryEvent {
            id: reconstruction,
            year: 213,
            kind: HistoryEventKind::Reconstruction,
            location: DVec3::new(port_anchor.x - 15.0, port_anchor.y + 8.0, port_anchor.z - 58.0),
            causes: vec![conflict, flood],
            created_asset_ids: vec![new_road_asset, new_quarter_asset, monument_asset],
            retired_asset_ids: Vec::new(),
            description: "A raised road, new quarter and memorial reorganize the settlement while preserving the ruins.".into(),
        },
    ];
    events.sort_by_key(|event| (event.year, event.id));

    let assets = vec![
        HistoricalAsset {
            id: harbor_asset,
            kind: HistoricalAssetKind::Harbor,
            anchor_world: DVec3::new(port_anchor.x + 40.0, 1.5, port_anchor.z),
            extent_m: DVec2::new(58.0, 34.0),
            created_by: harbor,
            retired_by: None,
            source_cell: port_cell,
        },
        HistoricalAsset {
            id: bridge_asset,
            kind: HistoricalAssetKind::Bridge,
            anchor_world: bridge_anchor,
            extent_m: DVec2::new(28.0, 8.0),
            created_by: roads,
            retired_by: None,
            source_cell: cell_at(detailed, bridge_anchor.xz()).unwrap_or(port_cell),
        },
        HistoricalAsset {
            id: old_road_asset,
            kind: HistoricalAssetKind::OldRoad,
            anchor_world: flood_anchor,
            extent_m: DVec2::new(120.0, 8.0),
            created_by: roads,
            retired_by: Some(flood),
            source_cell: cell_at(detailed, flood_anchor.xz()).unwrap_or(port_cell),
        },
        HistoricalAsset {
            id: new_road_asset,
            kind: HistoricalAssetKind::NewRoad,
            anchor_world: DVec3::new(
                port_anchor.x - 20.0,
                port_anchor.y + 8.0,
                port_anchor.z - 42.0,
            ),
            extent_m: DVec2::new(140.0, 10.0),
            created_by: reconstruction,
            retired_by: None,
            source_cell: port_cell,
        },
        HistoricalAsset {
            id: old_quarter_asset,
            kind: HistoricalAssetKind::OldQuarter,
            anchor_world: port_anchor,
            extent_m: DVec2::new(74.0, 54.0),
            created_by: foundation,
            retired_by: None,
            source_cell: port_cell,
        },
        HistoricalAsset {
            id: new_quarter_asset,
            kind: HistoricalAssetKind::NewQuarter,
            anchor_world: DVec3::new(
                port_anchor.x - 18.0,
                port_anchor.y + 8.0,
                port_anchor.z - 62.0,
            ),
            extent_m: DVec2::new(82.0, 62.0),
            created_by: reconstruction,
            retired_by: None,
            source_cell: cell_at(
                detailed,
                DVec2::new(port_anchor.x - 18.0, port_anchor.z - 62.0),
            )
            .unwrap_or(port_cell),
        },
        HistoricalAsset {
            id: farmland_asset,
            kind: HistoricalAssetKind::Farmland,
            anchor_world: farmland_anchor,
            extent_m: DVec2::new(132.0, 96.0),
            created_by: agriculture,
            retired_by: None,
            source_cell: cell_at(detailed, farmland_anchor.xz()).unwrap_or(port_cell),
        },
        HistoricalAsset {
            id: fortification_asset,
            kind: HistoricalAssetKind::Fortification,
            anchor_world: defensive_anchor,
            extent_m: DVec2::new(42.0, 32.0),
            created_by: fortification,
            retired_by: Some(conflict),
            source_cell: cell_at(detailed, defensive_anchor.xz()).unwrap_or(port_cell),
        },
        HistoricalAsset {
            id: ruins_asset,
            kind: HistoricalAssetKind::Ruins,
            anchor_world: defensive_anchor,
            extent_m: DVec2::new(46.0, 36.0),
            created_by: conflict,
            retired_by: None,
            source_cell: cell_at(detailed, defensive_anchor.xz()).unwrap_or(port_cell),
        },
        HistoricalAsset {
            id: monument_asset,
            kind: HistoricalAssetKind::Monument,
            anchor_world: DVec3::new(
                port_anchor.x - 4.0,
                port_anchor.y + 3.0,
                port_anchor.z - 20.0,
            ),
            extent_m: DVec2::new(8.0, 8.0),
            created_by: reconstruction,
            retired_by: None,
            source_cell: port_cell,
        },
    ];

    let land_use = vec![
        zone(
            atlas,
            0x201,
            LandUseKind::Harbor,
            assets[0].anchor_world,
            44.0,
            harbor,
            port_cell,
        ),
        zone(
            atlas,
            0x202,
            LandUseKind::OldTown,
            port_anchor,
            52.0,
            foundation,
            port_cell,
        ),
        zone(
            atlas,
            0x203,
            LandUseKind::NewTown,
            assets[5].anchor_world,
            58.0,
            reconstruction,
            assets[5].source_cell,
        ),
        zone(
            atlas,
            0x204,
            LandUseKind::Farmland,
            farmland_anchor,
            76.0,
            agriculture,
            assets[6].source_cell,
        ),
        zone(
            atlas,
            0x205,
            LandUseKind::Fortification,
            defensive_anchor,
            34.0,
            fortification,
            assets[7].source_cell,
        ),
        zone(
            atlas,
            0x206,
            LandUseKind::Ruins,
            defensive_anchor,
            38.0,
            conflict,
            assets[8].source_cell,
        ),
        zone(
            atlas,
            0x207,
            LandUseKind::Commons,
            DVec3::new(port_anchor.x - 6.0, port_anchor.y, port_anchor.z - 26.0),
            28.0,
            reconstruction,
            port_cell,
        ),
    ];

    let mut ledger = HistoryLedger {
        settlement_id: atlas.settlement_id,
        events,
        assets,
        land_use,
        current_year: 240,
        semantic_fingerprint: 0,
    };
    ledger.semantic_fingerprint = digest(&ledger)?;
    Ok(ledger)
}

pub fn apply_history_to_atlas(
    atlas: &mut WorldAtlasManifest,
    history: &HistoryLedger,
) -> Result<(), serde_json::Error> {
    for cell in &mut atlas.cells {
        let event_ids = history
            .events
            .iter()
            .filter(|event| cell_at_position(cell.coord, atlas.cell_radius_m, event.location.xz()))
            .map(|event| event.id)
            .collect::<Vec<_>>();
        let zones = history
            .land_use
            .iter()
            .filter(|zone| zone.source_cell == cell.coord)
            .collect::<Vec<_>>();
        if !event_ids.is_empty() || !zones.is_empty() {
            cell.history = AtlasHistorySummary {
                first_settlement_year: history
                    .events
                    .iter()
                    .filter(|event| event_ids.contains(&event.id))
                    .map(|event| event.year)
                    .min(),
                current_population: (cell.carrying_capacity * 4_500.0).round().max(80.0) as u32,
                dominant_economy: dominant_economy(zones.as_slice()),
                event_ids,
            };
        }
    }
    atlas.atlas_fingerprint = 0;
    atlas.atlas_fingerprint = digest(atlas)?;
    Ok(())
}

fn dominant_economy(zones: &[&LandUseZone]) -> String {
    if zones.iter().any(|zone| zone.kind == LandUseKind::Harbor) {
        "maritime trade and fishery".into()
    } else if zones.iter().any(|zone| zone.kind == LandUseKind::Farmland) {
        "agriculture".into()
    } else if zones
        .iter()
        .any(|zone| matches!(zone.kind, LandUseKind::OldTown | LandUseKind::NewTown))
    {
        "craft and local exchange".into()
    } else {
        "sparse rural use".into()
    }
}

fn choose_port_anchor(
    atlas: &WorldAtlasManifest,
    detailed: &DetailedRegion,
    port_cell: HexCoord,
) -> DVec3 {
    detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| sample.cell == Some(port_cell) && sample.buildable)
        .min_by(|left, right| {
            port_score(atlas, left)
                .total_cmp(&port_score(atlas, right))
                .then_with(|| left.grid_z.cmp(&right.grid_z))
                .then_with(|| left.grid_x.cmp(&right.grid_x))
        })
        .map(|sample| sample.world_position)
        .unwrap_or_else(|| {
            let center = port_cell.center_xz(atlas.cell_radius_m);
            DVec3::new(center.x, terrain_height_near(detailed, center), center.y)
        })
}

fn port_score(atlas: &WorldAtlasManifest, sample: &TerrainSample) -> f64 {
    let river_distance = (sample.world_position.z
        - river_center_z(sample.world_position.x, atlas.cell_radius_m))
    .abs();
    sample.slope * 160.0
        + river_distance * 0.36
        + (sample.world_position.y - 14.0).abs() * 0.5
        + sample.world_position.x.abs() * 0.02
}

fn choose_farmland_anchor(detailed: &DetailedRegion, port: DVec3) -> DVec3 {
    detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| sample.buildable && sample.world_position.distance(port) > 75.0)
        .min_by(|left, right| {
            farmland_score(left, port)
                .total_cmp(&farmland_score(right, port))
                .then_with(|| left.grid_z.cmp(&right.grid_z))
                .then_with(|| left.grid_x.cmp(&right.grid_x))
        })
        .map(|sample| sample.world_position)
        .unwrap_or(port + DVec3::new(-95.0, 0.0, 35.0))
}

fn farmland_score(sample: &TerrainSample, port: DVec3) -> f64 {
    sample.slope * 220.0
        + (sample.world_position.distance(port) - 125.0).abs() * 0.22
        + match sample.landform {
            LandformClass::Floodplain | LandformClass::Lowland | LandformClass::Terrace => 0.0,
            _ => 80.0,
        }
}

fn choose_defensive_anchor(detailed: &DetailedRegion, port: DVec3) -> DVec3 {
    detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| {
            sample.cell.is_some()
                && sample.world_position.distance(port) >= 95.0
                && sample.world_position.distance(port) <= 240.0
                && matches!(
                    sample.landform,
                    LandformClass::Ridge | LandformClass::Mountain | LandformClass::Hillslope
                )
        })
        .max_by(|left, right| {
            defensive_score(left, port)
                .total_cmp(&defensive_score(right, port))
                .then_with(|| right.grid_z.cmp(&left.grid_z))
                .then_with(|| right.grid_x.cmp(&left.grid_x))
        })
        .map(|sample| sample.world_position)
        .unwrap_or(port + DVec3::new(-150.0, 75.0, -40.0))
}

fn defensive_score(sample: &TerrainSample, port: DVec3) -> f64 {
    sample.world_position.y * 1.6
        - sample.slope * 80.0
        - (sample.world_position.distance(port) - 160.0).abs() * 0.2
}

fn zone(
    atlas: &WorldAtlasManifest,
    key: u128,
    kind: LandUseKind,
    center_world: DVec3,
    radius_m: f64,
    established_by: EventId,
    source_cell: HexCoord,
) -> LandUseZone {
    LandUseZone {
        id: RegionId::from_u128(asset_id(atlas.world_seed, key).as_u128()),
        kind,
        center_world,
        radius_m,
        established_by,
        source_cell,
    }
}

fn event_id(world_seed: u128, key: u128) -> EventId {
    EventId::from_u128(stable_entity(world_seed, STAGE_HISTORY, key, 0x601).as_u128())
}

fn asset_id(world_seed: u128, key: u128) -> EntityId {
    stable_entity(world_seed, STAGE_HISTORY, key, 0x602)
}

fn cell_at(detailed: &DetailedRegion, point: DVec2) -> Option<HexCoord> {
    detailed
        .terrain
        .samples
        .iter()
        .min_by(|left, right| {
            left.world_position
                .xz()
                .distance_squared(point)
                .total_cmp(&right.world_position.xz().distance_squared(point))
        })
        .and_then(|sample| sample.cell)
}

fn cell_at_position(coord: HexCoord, radius: f64, point: DVec2) -> bool {
    crate::atlas::point_in_hex(point, coord.center_xz(radius), radius)
}

fn terrain_height_near(detailed: &DetailedRegion, point: DVec2) -> f64 {
    detailed
        .terrain
        .samples
        .iter()
        .filter(|sample| sample.cell.is_some())
        .min_by(|left, right| {
            left.world_position
                .xz()
                .distance_squared(point)
                .total_cmp(&right.world_position.xz().distance_squared(point))
        })
        .map(|sample| sample.world_position.y)
        .unwrap_or(0.0)
}

pub fn validate_history(history: &HistoryLedger) -> Vec<ValidationCheck> {
    let event_ids = history
        .events
        .iter()
        .map(|event| event.id)
        .collect::<BTreeSet<_>>();
    let asset_ids = history
        .assets
        .iter()
        .map(|asset| asset.id)
        .collect::<BTreeSet<_>>();
    let chronological = history
        .events
        .windows(2)
        .all(|pair| (pair[0].year, pair[0].id) <= (pair[1].year, pair[1].id));
    let causes_exist_and_precede = history.events.iter().all(|event| {
        event.causes.iter().all(|cause| {
            history
                .events
                .iter()
                .find(|candidate| candidate.id == *cause)
                .is_some_and(|candidate| candidate.year <= event.year)
        })
    });
    let assets_traceable = history.assets.iter().all(|asset| {
        event_ids.contains(&asset.created_by)
            && asset
                .retired_by
                .is_none_or(|event| event_ids.contains(&event))
    });
    let created_assets_resolve = history.events.iter().all(|event| {
        event
            .created_asset_ids
            .iter()
            .all(|asset| asset_ids.contains(asset))
    });
    let zones_traceable = history
        .land_use
        .iter()
        .all(|zone| event_ids.contains(&zone.established_by));

    vec![
        ValidationCheck {
            name: "history-chronological".into(),
            passed: chronological,
            detail: format!(
                "{} events are sorted by year and stable ID",
                history.events.len()
            ),
        },
        ValidationCheck {
            name: "history-causes-precede-effects".into(),
            passed: causes_exist_and_precede,
            detail: "all causal references resolve to the same or an earlier year".into(),
        },
        ValidationCheck {
            name: "historical-assets-traceable".into(),
            passed: assets_traceable && created_assets_resolve,
            detail: format!(
                "{} historical assets map to creation and retirement events",
                history.assets.len()
            ),
        },
        ValidationCheck {
            name: "land-use-traceable".into(),
            passed: zones_traceable,
            detail: format!(
                "{} land-use zones map to an establishing event",
                history.land_use.len()
            ),
        },
        ValidationCheck {
            name: "history-has-five-or-more-stages".into(),
            passed: history.events.len() >= 5
                && history
                    .events
                    .iter()
                    .map(|event| event.kind)
                    .collect::<BTreeSet<_>>()
                    .len()
                    >= 5,
            detail: "migration, foundation, economy, infrastructure and disruption are represented"
                .into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        atlas::build_atlas,
        terrain::{default_materialized_cells, materialize_region},
    };
    use world_generation_core::{
        GeneratorVersion, TraversalOrder, WorldGenerationConfig, generate_world_with_order,
    };
    use world_ids::{BuildingId, WorldId};

    fn fixture() -> (WorldAtlasManifest, DetailedRegion, WorldManifest) {
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
        (atlas, detailed, base.manifest)
    }

    #[test]
    fn history_explains_current_assets() {
        let (atlas, detailed, base) = fixture();
        let history = generate_history(&atlas, &detailed, &base).expect("history");
        assert!(history.events.len() >= 9);
        assert!(
            history
                .assets
                .iter()
                .any(|asset| asset.kind == HistoricalAssetKind::Harbor)
        );
        assert!(
            history
                .assets
                .iter()
                .any(|asset| asset.kind == HistoricalAssetKind::Ruins)
        );
        assert!(validate_history(&history).iter().all(|check| check.passed));
    }
}
