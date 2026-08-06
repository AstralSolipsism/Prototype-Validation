#!/usr/bin/env python3
from __future__ import annotations

import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MASK64 = (1 << 64) - 1
MASK128 = (1 << 128) - 1
GAMMA = 0x9E3779B97F4A7C15
FNV_OFFSET = 0x6C62272E07BB014262B821756295C58D
FNV_PRIME = 0x0000000001000000000000000000013B


def mix64(value: int) -> int:
    value &= MASK64
    value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & MASK64
    return value ^ (value >> 31)


def fingerprint(data: bytes, number: int) -> int:
    state = FNV_OFFSET

    def write(raw: bytes) -> None:
        nonlocal state
        for byte in raw:
            state ^= byte
            state = (state * FNV_PRIME) & MASK128

    write(len(data).to_bytes(8, "little"))
    write(data)
    write(number.to_bytes(8, "little"))
    return state


def main() -> int:
    path = ROOT / "test-vectors" / "p0-foundation.json"
    vector = json.loads(path.read_text(encoding="utf-8"))

    state = int(vector["rng"]["seed_hex"], 16)
    actual = []
    for _ in vector["rng"]["next_u64_hex"]:
        state = (state + GAMMA) & MASK64
        actual.append(f"{mix64(state):016x}")
    assert actual == vector["rng"]["next_u64_hex"], (actual, vector["rng"])

    stable_id = vector["stable_id"]
    combined = (stable_id["high"] << 64) | stable_id["low"]
    assert f"{combined:032x}" == stable_id["expected_hex"]

    relative = vector["camera_relative_identity"]["object_offset"]
    tolerance = vector["camera_relative_identity"]["tolerance"]
    assert all(math.isfinite(value) for value in relative)
    assert abs(relative[0] - 0.0125) <= tolerance

    fp = vector["fingerprint"]
    actual_fp = fingerprint(fp["bytes_utf8"].encode("utf-8"), fp["u64"])
    assert f"{actual_fp:032x}" == fp["expected_hex"]

    print("P0 deterministic test vectors passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
