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
        "crates/region_authority_core",
        "crates/p7_validation_harness",
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
        "apps/p7_region_trace",
        "apps/p7_manual_validation",
    }
    assert required <= members, required - members

    status = json.loads((ROOT / "prototype-status.json").read_text(encoding="utf-8"))
    stages = {entry["id"]: entry for entry in status["stages"]}
    assert stages["P0"]["passed"] is True
    assert stages["P1"]["passed"] is True
    assert stages["P2"]["passed"] is True
    assert stages["P3"]["passed"] is True
    assert stages["P4"]["passed"] is True
    assert all(stage["passed"] is True for stage in stages["P4"]["sub_stages"])
    assert stages["P5"]["passed"] is True
    assert stages["P5"]["status"] == "passed"
    assert stages["P6"]["passed"] is True
    assert stages["P6"]["status"] == "passed"
    assert (
        ROOT / "docs/evidence/p6-simulation-scale-manual-validation-2026-08-09.md"
    ).is_file()

    p7 = stages["P7"]
    assert p7["status"] in {
        "implementation-in-progress",
        "automated-gate-passed-awaiting-manual-validation",
        "passed",
    }
    assert p7["issue"] == 28
    assert p7["pull_request"] == 29
    if p7["status"] == "passed":
        assert p7["passed"] is True
        assert p7["evidence_missing"] == []
        assert p7["manual_validation"]["checks_passed"] == 15
        assert p7["manual_validation"]["checks_total"] == 15
        assert p7["manual_validation"]["fingerprint"] == p7["manual_validation"][
            "repeat_fingerprint"
        ]
        assert (
            ROOT / "docs/evidence/p7-region-authority-manual-validation-2026-08-09.md"
        ).is_file()
        assert stages["P8"]["status"] == "not-started"
        assert stages["P8"]["ready_to_start"] is True
        assert stages["P8"]["blocked_by"] == []
    else:
        assert p7["passed"] is False
        assert stages["P8"]["status"] == "not-started"
        assert stages["P8"]["blocked_by"] == ["P7 manual validation"]

    print("Workspace manifest and prototype status passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
