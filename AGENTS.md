# Repository operating rules

This repository contains disposable prototypes around non-disposable contracts.

## Required boundaries

- `world_*`, `protocol`, `replay_core`, `scroll_camera_core`, and `p1_scenario` stay engine-agnostic.
- Never persist or transmit Bevy `Entity`, `Transform`, asset handles, or renderer-specific IDs.
- Stable test vectors change only with an explicit schema or version decision.
- A prototype stage cannot be marked `passed` until every required evidence item is recorded.
- Visual convenience may not move authoritative landmarks, resize reference frames, or remove structural collision.

## Change discipline

- Keep one risk hypothesis per pull request when practical.
- Update `prototype-status.json` and the relevant prototype document with evidence-producing changes.
- Add a failing test before repairing deterministic math or protocol defects.
- Treat generated meshes, screenshots, and trace reports as rebuildable artifacts, not world truth.
- Do not introduce networking, physics, databases, or LLM dependencies into P0/P1.
