#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if new in text:
        return text
    if old not in text:
        raise RuntimeError(f"P5 finalizer could not find {label}")
    return text.replace(old, new, 1)


def finalize_engine() -> None:
    path = ROOT / "crates/authoritative_world_core/src/engine.rs"
    text = path.read_text(encoding="utf-8")
    text = replace_once(
        text,
        '    #[error("journal state fingerprint mismatch")]\n    JournalFingerprint,\n',
        '    #[error("journal state fingerprint mismatch")]\n    JournalFingerprint,\n'
        '    #[error("entity version conflict: expected {expected}, actual {actual}")]\n'
        '    VersionConflict { expected: u64, actual: u64 },\n',
        "authority version conflict variant",
    )
    text = replace_once(
        text,
        "            self.world.revision = delta.to_revision;\n",
        "            self.world.revision = delta.to_revision;\n"
        "            self.world.clock = record.envelope.issued_at;\n",
        "journal clock recovery",
    )
    text = replace_once(
        text,
        "                        self.execute_mutation(envelope, command)\n",
        "                        match self.execute_mutation(envelope, command) {\n"
        "                            Err(AuthorityError::VersionConflict { expected, actual }) => {\n"
        "                                self.rejected(\n"
        "                                    envelope.command_id,\n"
        "                                    RejectionCode::VersionConflict,\n"
        "                                    format!(\n"
        "                                        \"entity version mismatch: expected {expected}, actual {actual}\"\n"
        "                                    ),\n"
        "                                )\n"
        "                            }\n"
        "                            result => result,\n"
        "                        }\n",
        "version conflict receipt conversion",
    )
    text = replace_once(
        text,
        '            return Err(AuthorityError::Invariant(format!(\n'
        '                "VERSION_CONFLICT:{}:{}",\n'
        '                expected.0, actual.0\n'
        '            )));\n',
        "            return Err(AuthorityError::VersionConflict {\n"
        "                expected: expected.0,\n"
        "                actual: actual.0,\n"
        "            });\n",
        "entity version conflict error",
    )
    path.write_text(text, encoding="utf-8")


def finalize_harness() -> None:
    path = ROOT / "crates/p5_validation_harness/src/lib.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "matches!(buffered, WireResponse::Buffered",
        "matches!(&buffered, WireResponse::Buffered",
    )
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
    finalize_engine()
    finalize_harness()
    finalize_status()
    print("P5 deterministic source finalization completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
