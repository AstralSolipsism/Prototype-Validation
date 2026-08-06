#!/usr/bin/env python3
"""Fail fast when engine-specific types leak into engine-agnostic crates."""

from __future__ import annotations

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
ENGINE_AGNOSTIC_CRATES = (
    "world_ids",
    "world_time",
    "deterministic_rng",
    "world_math",
    "protocol",
    "replay_core",
    "scroll_camera_core",
    "p1_scenario",
)
FORBIDDEN_TOKENS = (
    "bevy::",
    "bevy =",
    "bevy_",
    "GlobalTransform",
    "Mesh3d",
    "Handle<",
)


def main() -> int:
    failures: list[str] = []

    for crate in ENGINE_AGNOSTIC_CRATES:
        crate_dir = ROOT / "crates" / crate
        if not crate_dir.exists():
            failures.append(f"missing engine-agnostic crate: {crate_dir.relative_to(ROOT)}")
            continue

        for path in crate_dir.rglob("*"):
            if not path.is_file() or path.suffix not in {".rs", ".toml"}:
                continue
            text = path.read_text(encoding="utf-8")
            for token in FORBIDDEN_TOKENS:
                if token in text:
                    failures.append(
                        f"{path.relative_to(ROOT)} contains forbidden engine token {token!r}"
                    )

    required = (
        ROOT / "rust-toolchain.toml",
        ROOT / ".github" / "workflows" / "ci.yml",
        ROOT / "docs" / "technical-preproduction.md",
        ROOT / "prototype-status.json",
        ROOT / "test-vectors" / "p0-foundation.json",
    )
    for path in required:
        if not path.exists():
            failures.append(f"missing required file: {path.relative_to(ROOT)}")

    if failures:
        print("Architecture validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print("Architecture validation passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
