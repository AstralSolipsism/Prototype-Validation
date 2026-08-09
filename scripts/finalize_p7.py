#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def base_evidence() -> list[str]:
    return [
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
    ]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode",
        choices=["implementation", "awaiting-manual", "passed"],
        required=True,
    )
    parser.add_argument("--workflow-run", type=int)
    parser.add_argument("--source")
    parser.add_argument("--manual-artifact-id", type=int)
    parser.add_argument("--manual-artifact-sha")
    parser.add_argument("--evidence-artifact-id", type=int)
    parser.add_argument("--evidence-artifact-sha")
    parser.add_argument("--report-sha")
    parser.add_argument("--interests-sha")
    parser.add_argument("--transfer-sha")
    args = parser.parse_args()

    path = ROOT / "prototype-status.json"
    status = json.loads(path.read_text(encoding="utf-8"))
    stages = {entry["id"]: entry for entry in status["stages"]}
    p7 = stages["P7"]
    p7.clear()

    if args.mode == "implementation":
        p7.update(
            {
                "id": "P7",
                "name": "multiplayer-interest-region-authority-and-handoff",
                "status": "implementation-in-progress",
                "passed": False,
                "issue": 28,
                "pull_request": 29,
                "ready_to_start": False,
                "blocked_by": [],
                "evidence_present": base_evidence(),
                "evidence_missing": [
                    "green complete-workspace P7 automated gate",
                    "Linux and Windows P7 deterministic trace",
                    "Windows x64 P7 manual validation package",
                    "project owner manual acceptance",
                ],
            }
        )
    elif args.mode == "awaiting-manual":
        if args.workflow_run is None or not args.source:
            parser.error("awaiting-manual mode requires --workflow-run and --source")
        p7.update(
            {
                "id": "P7",
                "name": "multiplayer-interest-region-authority-and-handoff",
                "status": "automated-gate-passed-awaiting-manual-validation",
                "passed": False,
                "issue": 28,
                "pull_request": 29,
                "ready_to_start": False,
                "blocked_by": [],
                "evidence_present": base_evidence(),
                "evidence_missing": ["project owner manual acceptance"],
                "automated_gate": {
                    "workflow_run": args.workflow_run,
                    "validated_source": args.source,
                    "result": "passed",
                    "manual_artifact": "p7-region-authority-manual-validation-windows-x64",
                    "evidence_artifact": "p7-region-authority-automated-evidence",
                },
            }
        )
    else:
        required = {
            "workflow_run": args.workflow_run,
            "source": args.source,
            "manual_artifact_id": args.manual_artifact_id,
            "manual_artifact_sha": args.manual_artifact_sha,
            "evidence_artifact_id": args.evidence_artifact_id,
            "evidence_artifact_sha": args.evidence_artifact_sha,
            "report_sha": args.report_sha,
            "interests_sha": args.interests_sha,
            "transfer_sha": args.transfer_sha,
        }
        missing = [name for name, value in required.items() if value in {None, ""}]
        if missing:
            parser.error(f"passed mode is missing: {', '.join(missing)}")

        p7.update(
            {
                "id": "P7",
                "name": "multiplayer-interest-region-authority-and-handoff",
                "status": "passed",
                "passed": True,
                "issue": 28,
                "pull_request": 29,
                "ready_to_start": False,
                "blocked_by": [],
                "evidence_present": base_evidence()
                + [
                    "Linux and Windows x64 automatic P7 validation passed",
                    "project owner Windows x64 interactive validation artifacts submitted",
                    "manual report with 15 of 15 checks passed",
                    "manual client-interest and transfer files exactly match both report runs",
                    "49 cross-file structural and semantic checks passed",
                    "manual result recorded in docs/evidence/p7-region-authority-manual-validation-2026-08-09.md",
                ],
                "evidence_missing": [],
                "known_non_blocking_evidence_gaps": [
                    "p7-machine-info.txt was not submitted; P6 retains the project-owner hardware baseline"
                ],
                "automated_gate": {
                    "workflow_run": args.workflow_run,
                    "validated_source": args.source,
                    "result": "passed",
                    "manual_artifact": "p7-region-authority-manual-validation-windows-x64",
                    "manual_artifact_id": args.manual_artifact_id,
                    "manual_artifact_sha256": args.manual_artifact_sha,
                    "evidence_artifact": "p7-region-authority-automated-evidence",
                    "evidence_artifact_id": args.evidence_artifact_id,
                    "evidence_artifact_sha256": args.evidence_artifact_sha,
                },
                "manual_validation": {
                    "date": "2026-08-09",
                    "result": "passed",
                    "evidence_file": "docs/evidence/p7-region-authority-manual-validation-2026-08-09.md",
                    "report_sha256": args.report_sha,
                    "client_interests_sha256": args.interests_sha,
                    "transfer_evidence_sha256": args.transfer_sha,
                    "checks_passed": 15,
                    "checks_total": 15,
                    "clients": 4,
                    "static_regions": 3,
                    "mobile_regions": 1,
                    "stale_writer_rejected": True,
                    "duplicate_transfer_operations": 4,
                    "fault_recoveries": 2,
                    "unique_entity_locations": True,
                    "persistent_facts_preserved": True,
                    "ship_internal_authority_preserved": True,
                    "parallel_workers": 2,
                    "parallel_equals_sequential": True,
                    "hotspot_commands": 256,
                    "hotspot_unique_writers": 1,
                    "hotspot_first_revision": 5,
                    "hotspot_final_revision": 261,
                    "hotspot_revisions_contiguous": True,
                    "fingerprint": "81aed9ca4d8bf3bb75068b37f6f9a7e2",
                    "repeat_fingerprint": "81aed9ca4d8bf3bb75068b37f6f9a7e2",
                },
                "final_verdict": "The four-client interest model, static and mobile region single-writer authority, epoch rebalance, idempotent cross-region transfer, duplicate recovery, mobile interior continuity, remote summaries, deterministic independent-region parallelism and single-writer hotspot execution passed automated and project-owner manual validation.",
            }
        )

    p8 = stages["P8"]
    p8.clear()
    if args.mode == "passed":
        p8.update(
            {
                "id": "P8",
                "status": "not-started",
                "passed": False,
                "ready_to_start": True,
                "blocked_by": [],
            }
        )
    else:
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
