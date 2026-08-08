# P4 integrated final automated gate

Result: **FAIL**

Workflow run: 31265882327
Validated source: c969a0ae84d7d08a2bc2d874b83de505f5ca70e4
Branch: agent/p4-integrated-world-pipeline

## Failure summary

```text
```

## Diagnostic tail

```text
## Static preproduction checks
Architecture validation passed.
Workspace manifest and prototype status passed.
P0 deterministic test vectors passed.
Static preproduction checks passed.
## Formatting
## Dependency lockfile
## Complete workspace cargo check
[1m[92m    Checking[0m world_ids v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_ids)
[1m[92m    Checking[0m scroll_camera_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/scroll_camera_core)
[1m[92m    Checking[0m deterministic_rng v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/deterministic_rng)
[1m[92m    Checking[0m world_time v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_time)
[1m[92m    Checking[0m p1_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p1_scenario)
[1m[92m    Checking[0m building_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/building_core)
[1m[92m    Checking[0m world_generation_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_generation_core)
[1m[92m    Checking[0m world_math v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_math)
[1m[92m    Checking[0m camera_routes v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_routes)
[1m[92m    Checking[0m mobile_region_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/mobile_region_core)
[1m[92m    Checking[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m    Checking[0m mobile_region_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/mobile_region_visual)
[1m[92m    Checking[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[92m    Checking[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m    Checking[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m    Checking[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m    Checking[0m world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/world_visual)
[1m[92m    Checking[0m p4_world_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_world_trace)
[1m[92m    Checking[0m building_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/building_visual)
[1m[92m    Checking[0m p3_building_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p3_building_trace)
[1m[92m    Checking[0m p4_integrated_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_integrated_scenario)
[1m[92m    Checking[0m replay_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/replay_core)
[1m[92m    Checking[0m integrated_world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/integrated_world_visual)
[1m[92m    Checking[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[33mwarning[0m[1m: unused import: `LandformClass`[0m
  [1m[94m--> [0mapps/integrated_world_visual/src/main.rs:11:29
   [1m[94m|[0m
[1m[94m11[0m [1m[94m|[0m     LandCover, LandUseKind, LandformClass, TerrainGrid, detailed_height,
   [1m[94m|[0m                             [1m[33m^^^^^^^^^^^^^[0m
   [1m[94m|[0m
   [1m[94m= [0m[1mnote[0m: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

[1m[33mwarning[0m[1m: unused import: `f32::consts::PI`[0m
  [1m[94m--> [0mapps/integrated_world_visual/src/main.rs:15:34
   [1m[94m|[0m
[1m[94m15[0m [1m[94m|[0m use std::{collections::BTreeMap, f32::consts::PI};
   [1m[94m|[0m                                  [1m[33m^^^^^^^^^^^^^^^[0m

[1m[33mwarning[0m[1m: unused import: `BuildingInstanceId`[0m
  [1m[94m--> [0mapps/integrated_world_visual/src/main.rs:17:17
   [1m[94m|[0m
[1m[94m17[0m [1m[94m|[0m use world_ids::{BuildingInstanceId, EntityId};
   [1m[94m|[0m                 [1m[33m^^^^^^^^^^^^^^^^^^[0m

[1m[91merror[E0596][0m[1m: cannot borrow `material` as mutable, as it is not declared as mutable[0m
   [1m[94m--> [0mapps/integrated_world_visual/src/main.rs:896:13
    [1m[94m|[0m
[1m[94m896[0m [1m[94m|[0m             material.base_color = atlas_cell_color(cell, state.atlas_layer, &world.0.atlas.cells);
    [1m[94m|[0m             [1m[91m^^^^^^^^[0m [1m[91mcannot borrow as mutable[0m
    [1m[94m|[0m
[1m[96mhelp[0m: consider changing this to be mutable
    [1m[94m|[0m
[1m[94m894[0m [1m[94m| [0m            && let Some([92mmut [0mmaterial) = materials.get_mut(&material_handle.0)
    [1m[94m|[0m                         [92m+++[0m

[1mFor more information about this error, try `rustc --explain E0596`.[0m
[1m[33mwarning[0m: `integrated_world_visual` (bin "integrated_world_visual" test) generated 3 warnings
[1m[91merror[0m: could not compile `integrated_world_visual` (bin "integrated_world_visual" test) due to 1 previous error; 3 warnings emitted
[1m[33mwarning[0m: build failed, waiting for other jobs to finish...
[1m[33mwarning[0m: `integrated_world_visual` (bin "integrated_world_visual") generated 3 warnings (3 duplicates)
[1m[91merror[0m: could not compile `integrated_world_visual` (bin "integrated_world_visual") due to 1 previous error; 3 warnings emitted
```
