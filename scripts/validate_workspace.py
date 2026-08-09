#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    assert workspace["workspace"]["resolver"] == "3"
    assert workspace["workspace"]["package"]["edition"] == "2024"
    assert workspace["workspace"]["package"]["rust-version"] == "1.97"

    bevy = workspace["workspace"]["dependencies"]["bevy"]
    assert bevy["version"] == "=0.19.0"
    assert bevy["default-features"] is False
    assert bevy["features"] == ["3d"]

    members = set(workspace["workspace"]["members"])
    required = {
        "crates/world_ids",
        "crates/world_time",
        "crates/deterministic_rng",
        "crates/world_math",
        "crates/protocol",
        "crates/replay_core",
        "crates/scroll_camera_core",
        "crates/p1_scenario",
        "crates/mobile_region_core",
        "crates/building_core",
        "crates/p3_building_scenario",
        "crates/world_generation_core",
        "crates/p4_world_scenario",
        "crates/integrated_world_core",
        "crates/p4_integrated_scenario",
        "crates/region_scale_core",
        "crates/p4_region_scale_scenario",
        "crates/authoritative_world_core",
        "crates/authority_persistence",
        "crates/authority_transport",
        "crates/p5_authority_scenario",
        "crates/p5_validation_harness",
        "crates/simulation_scale_core",
        "crates/p6_validation_harness",
        "apps/camera_trace",
        "apps/camera_routes",
        "apps/p2_reference_frame_trace",
        "apps/mobile_region_visual",
        "apps/p3_building_trace",
        "apps/building_visual",
        "apps/p4_world_trace",
        "apps/world_visual",
        "apps/p4_integrated_trace",
        "apps/integrated_world_visual",
        "apps/p4_region_scale_trace",
        "apps/p4_region_scale_visual",
        "apps/p5_authority_trace",
        "apps/p5_manual_validation",
        "apps/p6_simulation_trace",
        "apps/p6_manual_validation",
    }
    assert required <= members, required - members

    status = json.loads((ROOT / "prototype-status.json").read_text(encoding="utf-8"))
    stages = {entry["id"]: entry for entry in status["stages"]}
    assert stages["P0"]["passed"] is True
    assert stages["P0"]["status"] == "passed"
    assert stages["P1"]["passed"] is True
    assert stages["P1"]["status"] == "passed-with-non-blocking-engineering-followups"
    assert stages["P2"]["passed"] is True
    assert stages["P2"]["status"] == "passed"
    assert stages["P3"]["passed"] is True
    assert stages["P3"]["status"] == "passed"
    assert stages["P4"]["passed"] is True
    assert stages["P4"]["status"] == "passed"
    assert all(stage["passed"] is True for stage in stages["P4"]["sub_stages"])
    assert (
        ROOT / "docs/evidence/p4-region-scale-manual-gpu-validation-2026-08-09.md"
    ).is_file()
    assert stages["P5"]["passed"] is True
    assert stages["P5"]["status"] == "passed"
    assert (
        ROOT / "docs/evidence/p5-authority-manual-validation-2026-08-09.md"
    ).is_file()

    assert stages["P6"]["status"] in {
        "implementation-in-progress",
        "automated-gate-passed-awaiting-manual-validation",
    }
    assert stages["P6"]["passed"] is False
    assert stages["P6"]["issue"] == 26
    assert stages["P6"]["pull_request"] == 27
    assert stages["P7"]["status"] == "not-started"
    assert stages["P8"]["status"] == "not-started"

    print("Workspace manifest and prototype status passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
