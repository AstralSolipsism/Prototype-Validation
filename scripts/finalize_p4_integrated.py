#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

from finalize_p4_generation_constraints import main as finalize_generation_constraints

ROOT = Path(__file__).resolve().parents[1]


def finalize_atlas() -> None:
    path = ROOT / "crates/integrated_world_core/src/atlas.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "    let settlement_cell = manifest\n"
        "        .cell(settlement.cell)\n"
        "        .expect(\"settlement cell exists\");\n",
        "",
        1,
    )
    old = """        if let Some(cell) = manifest.cells.iter().min_by(|left, right| {
            left.center_world
                .xz()
                .distance_squared(point.xz())
                .total_cmp(&right.center_world.xz().distance_squared(point.xz()))
        }) {
            if cell.center_world.xz().distance(point.xz()) <= manifest.cell_radius_m * 1.35 {
                touched.insert(cell.coord);
            }
        }
"""
    new = """        if let Some(cell) = manifest.cells.iter().min_by(|left, right| {
            left.center_world
                .xz()
                .distance_squared(point.xz())
                .total_cmp(&right.center_world.xz().distance_squared(point.xz()))
        }) && cell.center_world.xz().distance(point.xz()) <= manifest.cell_radius_m * 1.35
        {
            touched.insert(cell.coord);
        }
"""
    if old in text:
        text = text.replace(old, new, 1)
    elif "}) && cell.center_world.xz().distance(point.xz()) <= manifest.cell_radius_m * 1.35" not in text:
        raise RuntimeError("Atlas touched-cell selection is neither original nor finalized")
    path.write_text(text, encoding="utf-8")


def finalize_traversal() -> None:
    path = ROOT / "crates/integrated_world_core/src/traversal.rs"
    text = path.read_text(encoding="utf-8")
    old = "fn nearest_sample<'a>(grid: &'a TerrainGrid, point: DVec2) -> &'a TerrainSample {"
    new = "fn nearest_sample(grid: &TerrainGrid, point: DVec2) -> &TerrainSample {"
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise RuntimeError("Traversal nearest_sample signature is neither original nor finalized")
    path.write_text(text, encoding="utf-8")


def finalize_history() -> None:
    path = ROOT / "crates/integrated_world_core/src/history.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "use std::collections::BTreeSet;",
        "use std::collections::{BTreeMap, BTreeSet};",
        1,
    )
    if "let event_cells = history" in text and ".collect::<BTreeMap<_, _>>();" in text:
        path.write_text(text, encoding="utf-8")
        return

    function = '''pub fn apply_history_to_atlas(
    atlas: &mut WorldAtlasManifest,
    history: &HistoryLedger,
) -> Result<(), serde_json::Error> {
    let cell_radius_m = atlas.cell_radius_m;
    let event_cells = history
        .events
        .iter()
        .map(|event| {
            let point = event.location.xz();
            let coord = atlas
                .cells
                .iter()
                .find(|cell| cell_at_position(cell.coord, cell_radius_m, point))
                .or_else(|| {
                    atlas.cells.iter().min_by(|left, right| {
                        left.center_world
                            .xz()
                            .distance_squared(point)
                            .total_cmp(&right.center_world.xz().distance_squared(point))
                    })
                })
                .map(|cell| cell.coord)
                .expect("Atlas contains cells");
            (event.id, coord)
        })
        .collect::<BTreeMap<_, _>>();

    for cell in &mut atlas.cells {
        let event_ids = history
            .events
            .iter()
            .filter(|event| event_cells.get(&event.id).copied() == Some(cell.coord))
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
}'''
    text, count = re.subn(
        r"pub fn apply_history_to_atlas\(.*?\n}\n(?=\n?fn dominant_economy)",
        function + "\n",
        text,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise RuntimeError("Atlas history projection could not be finalized")
    path.write_text(text, encoding="utf-8")


def canonicalize_local_clippy_allowance(
    text: str,
    function_name: str,
    explanation: str,
) -> str:
    canonical = (
        "#[allow(clippy::too_many_arguments)]\n"
        f"// {explanation}\n"
    )
    text = re.sub(
        rf"(?:#\[allow\(clippy::too_many_arguments\)\]\n(?://[^\n]*\n)?)+(?=fn {function_name}\()",
        canonical,
        text,
        count=1,
    )
    marker = canonical + f"fn {function_name}("
    if marker not in text:
        text = text.replace(
            f"\nfn {function_name}(",
            "\n" + marker,
            1,
        )
    return text


def finalize_visual() -> None:
    path = ROOT / "apps/integrated_world_visual/src/main.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "    LandCover, LandUseKind, LandformClass, TerrainGrid, detailed_height,",
        "    LandCover, LandUseKind, TerrainGrid, detailed_height,",
        1,
    )
    text = text.replace(
        "use std::{collections::BTreeMap, f32::consts::PI};",
        "use std::collections::BTreeMap;",
        1,
    )
    text = text.replace(
        "use world_ids::{BuildingInstanceId, EntityId};",
        "use world_ids::EntityId;",
        1,
    )
    text = text.replace(
        "&& let Some(material) = materials.get_mut(&material_handle.0)",
        "&& let Some(mut material) = materials.get_mut(&material_handle.0)",
        1,
    )
    text = canonicalize_local_clippy_allowance(
        text,
        "update_camera_and_route_subject",
        "Bevy injects these resources and queries as independent system parameters.",
    )
    text = canonicalize_local_clippy_allowance(
        text,
        "spawn_segment",
        "A segment is one rendering primitive with explicit geometry, material and visibility data.",
    )

    if (
        "visuals: Query<(Entity, &VisualTag)>" in text
        and "mut commands: Commands" in text
        and "commands\n            .entity(entity)" in text
    ):
        path.write_text(text, encoding="utf-8")
        return

    function = '''fn update_visual_visibility(
    state: Res<VisualState>,
    visuals: Query<(Entity, &VisualTag)>,
    mut commands: Commands,
) {
    for (entity, tag) in &visuals {
        let space_visible = match tag.space {
            VisualSpace::Atlas => state.mode == AppMode::Atlas,
            VisualSpace::Local => state.mode != AppMode::Atlas,
        };
        let kind_visible = match tag.kind {
            VisualKind::Always => true,
            VisualKind::AtlasBoundary => state.show_atlas_boundaries,
            VisualKind::AtlasContour => state.show_contours,
            VisualKind::AtlasHistory | VisualKind::LocalHistory => state.show_history,
            VisualKind::LocalRoute => state.show_routes,
            VisualKind::RouteSubject => state.mode == AppMode::RouteTravel,
            VisualKind::BuildingFull => state.building_lod == BuildingLod::Full,
            VisualKind::BuildingShell => state.building_lod == BuildingLod::Shell,
            VisualKind::BuildingMassing => state.building_lod == BuildingLod::Massing,
        };
        commands.entity(entity).insert(if space_visible && kind_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}'''
    text, count = re.subn(
        r"fn update_visual_visibility\(.*?\n}\n(?=\n?fn update_atlas_cell_materials)",
        function + "\n",
        text,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise RuntimeError("Bevy visibility system could not be finalized")
    path.write_text(text, encoding="utf-8")


def remove_temporary_lint_allowance() -> None:
    path = ROOT / "crates/integrated_world_core/src/lib.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "#![allow(unused_variables, clippy::collapsible_if, clippy::needless_lifetimes)]\n",
        "",
        1,
    )
    path.write_text(text, encoding="utf-8")


def main() -> int:
    finalize_atlas()
    finalize_traversal()
    finalize_history()
    finalize_visual()
    remove_temporary_lint_allowance()
    finalize_generation_constraints()
    print("Integrated P4 source finalization completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
