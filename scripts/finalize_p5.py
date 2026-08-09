#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        if new in text:
            return text
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
    text = text.replace("matches!(buffered, WireResponse::Buffered", "matches!(&buffered, WireResponse::Buffered")
    path.write_text(text, encoding="utf-8")


def main() -> int:
    finalize_engine()
    finalize_harness()
    print("P5 deterministic source finalization completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
