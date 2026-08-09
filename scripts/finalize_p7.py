#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["implementation", "awaiting-manual"], required=True)
    parser.add_argument("--workflow-run", type=int)
    parser.add_argument("--source")
    args = parser.parse_args()

    path = ROOT / "prototype-status.json"
    status = json.loads(path.read_text(encoding="utf-8"))
    stages = {entry["id"]: entry for entry in status["stages"]}
    p7 = stages["P7"]
    p7.clear()
    p7.update(
        {
            "id": "P7",
            "name": "multiplayer-interest-region-authority-and-handoff",
            "status": (
                "implementation-in-progress"
                if args.mode == "implementation"
                else "automated-gate-passed-awaiting-manual-validation"
            ),
            "passed": False,
            "issue": 28,
            "pull_request": 29,
            "ready_to_start": False,
            "blocked_by": [],
            "evidence_present": [
                "four independent client interest projections",
                "separate visual, simulation, junction, window and vehicle-trajectory interests",
                "three static regions and one mobile ship region",
                "single writer token and epoch for every region",
                "stale writer rejection after region rebalance",
                "idempotent Prepare/Accept/Commit/Ack transfer protocol",
                "fault recovery that removes pre-commit or post-commit duplicate copies",
                "static-to-static, static-to-mobile and mobile-to-static transfers",
                "mobile interior authority retained while the ship crosses static AtlasCells",
                "remote Cold/Warm person summary without high-detail entity leakage",
                "deterministic parallel execution for independent regions",
                "single-writer revision-contiguous hotspot execution",
                "P0 through P6 regression coverage",
            ],
            "evidence_missing": ["project owner manual acceptance"],
        }
    )
    if args.mode == "implementation":
        p7["evidence_missing"] = [
            "green complete-workspace P7 automated gate",
            "Linux and Windows P7 deterministic trace",
            "Windows x64 P7 manual validation package",
            "project owner manual acceptance",
        ]
    else:
        if args.workflow_run is None or not args.source:
            parser.error("awaiting-manual mode requires --workflow-run and --source")
        p7["automated_gate"] = {
            "workflow_run": args.workflow_run,
            "validated_source": args.source,
            "result": "passed",
            "manual_artifact": "p7-region-authority-manual-validation-windows-x64",
            "evidence_artifact": "p7-region-authority-automated-evidence",
        }

    p8 = stages["P8"]
    p8.clear()
    p8.update(
        {
            "id": "P8",
            "status": "not-started",
            "passed": False,
            "blocked_by": ["P7 manual validation"],
        }
    )
    path.write_text(json.dumps(status, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"P7 status normalized to {p7['status']}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
