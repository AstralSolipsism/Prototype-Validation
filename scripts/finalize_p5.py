#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def validate_engine() -> None:
    path = ROOT / "crates/authoritative_world_core/src/engine.rs"
    text = path.read_text(encoding="utf-8")
    required = {
        "typed version conflict": "VersionConflict { expected: u64, actual: u64 }" in text,
        "journal clock recovery": "self.world.clock = record.envelope.issued_at;" in text,
        "version conflict receipt conversion": "Err(AuthorityError::VersionConflict" in text
        and "RejectionCode::VersionConflict" in text,
        "entity version gate": "return Err(AuthorityError::VersionConflict" in text,
        "idempotency lookup": "self.idempotency.get(&envelope.command_id)" in text,
        "snapshot restore": "pub fn from_snapshot(snapshot: AuthoritySnapshot)" in text,
    }
    failed = [name for name, passed in required.items() if not passed]
    if failed:
        raise RuntimeError(f"P5 authoritative engine markers are missing: {failed}")


def finalize_harness() -> None:
    path = ROOT / "crates/p5_validation_harness/src/lib.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "matches!(buffered, WireResponse::Buffered",
        "matches!(&buffered, WireResponse::Buffered",
    )
    path.write_text(text, encoding="utf-8")


def finalize_workflow() -> None:
    path = ROOT / ".github/workflows/p5-authority-final-gate.yml"
    text = path.read_text(encoding="utf-8")
    text = text.replace("  cancel-in-progress: false", "  cancel-in-progress: true")
    path.write_text(text, encoding="utf-8")


def finalize_status() -> None:
    path = ROOT / "prototype-status.json"
    status = json.loads(path.read_text(encoding="utf-8"))
    p5 = next(stage for stage in status["stages"] if stage["id"] == "P5")
    p5.update(
        {
            "name": "authoritative-server-persistence-recovery-and-replay",
            "status": "implementation-in-progress",
            "passed": False,
            "issue": 24,
            "pull_request": 25,
            "ready_to_start": False,
            "blocked_by": [],
            "evidence_present": [
                "engine-independent authoritative world state and command model",
                "persistent idempotency ledger and versioned domain events",
                "atomic region snapshot and transactional append-only journal",
                "ordered ingress for duplicate, delayed and out-of-order commands",
                "loopback TCP authority server and two client projections",
                "P5 deterministic scenario derived from the accepted P4 world",
            ],
            "evidence_missing": [
                "green complete-workspace P5 automated gate",
                "successful snapshot plus journal restart recovery trace",
                "Windows x64 P5 manual validation package",
                "project owner manual acceptance",
            ],
        }
    )
    path.write_text(json.dumps(status, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    validate_engine()
    finalize_harness()
    finalize_workflow()
    finalize_status()
    print("P5 deterministic source finalization completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
