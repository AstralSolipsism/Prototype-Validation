#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

from finalize_p4_engineered_routes import main as finalize_engineered_routes

ROOT = Path(__file__).resolve().parents[1]


def canonicalize_allowance(text: str, function_name: str, explanation: str) -> str:
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
        text = text.replace(f"\nfn {function_name}(", "\n" + marker, 1)
    return text


def normalize_visual_source() -> None:
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
    text = canonicalize_allowance(
        text,
        "update_camera_and_route_subject",
        "Bevy injects these resources and queries as independent system parameters.",
    )
    text = canonicalize_allowance(
        text,
        "spawn_segment",
        "A segment is one rendering primitive with explicit geometry, material and visibility data.",
    )
    required = [
        "visuals: Query<(Entity, &VisualTag)>",
        "mut commands: Commands",
        "commands\n            .entity(entity)",
    ]
    missing = [marker for marker in required if marker not in text]
    if missing:
        raise RuntimeError(f"integrated visibility system is not finalized: {missing}")
    path.write_text(text, encoding="utf-8")


def validate_persisted_core_fixes() -> None:
    atlas = (ROOT / "crates/integrated_world_core/src/atlas.rs").read_text(encoding="utf-8")
    history = (ROOT / "crates/integrated_world_core/src/history.rs").read_text(encoding="utf-8")
    terrain = (ROOT / "crates/integrated_world_core/src/terrain.rs").read_text(encoding="utf-8")
    traversal = (ROOT / "crates/integrated_world_core/src/traversal.rs").read_text(
        encoding="utf-8"
    )
    validation = (ROOT / "crates/integrated_world_core/src/validate.rs").read_text(
        encoding="utf-8"
    )
    required = {
        "Atlas swizzle trait": "Vec3Swizzles" in atlas,
        "history deterministic cell assignment": "let event_cells = history" in history,
        "river local-minimum classification": "let near_local_minimum" in terrain,
        "dense traversal path": "fn simplify_path(_grid: &TerrainGrid" in traversal,
        "engineered traversal cost": "let grade_penalty" in traversal,
        "river diagnostic offsets": "let river_height_offsets" in validation,
        "traversal lifetime elision": "fn nearest_sample(grid: &TerrainGrid" in traversal,
    }
    failed = [name for name, passed in required.items() if not passed]
    if failed:
        raise RuntimeError(f"persisted integrated fixes are missing: {failed}")


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
    validate_persisted_core_fixes()
    finalize_engineered_routes()
    normalize_visual_source()
    remove_temporary_lint_allowance()
    print("Integrated P4 source finalization completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
