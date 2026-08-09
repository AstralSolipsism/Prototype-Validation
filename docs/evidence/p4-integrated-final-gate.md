# P4 integrated final automated gate

Result: **FAIL**

Workflow run: 31290553461
Validated source: e7bf99016579a1c0b974e17412afaeb9efcd3837
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
[1m[92m    Checking[0m world_generation_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_generation_core)
[1m[92m    Checking[0m building_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/building_core)
[1m[92m    Checking[0m world_math v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_math)
[1m[92m    Checking[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[92m    Checking[0m camera_routes v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_routes)
[1m[92m    Checking[0m mobile_region_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/mobile_region_core)
[1m[92m    Checking[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m    Checking[0m mobile_region_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/mobile_region_visual)
[1m[92m    Checking[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m    Checking[0m region_scale_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/region_scale_core)
[1m[92m    Checking[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m    Checking[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m    Checking[0m world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/world_visual)
[1m[92m    Checking[0m p4_region_scale_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_region_scale_scenario)
[1m[92m    Checking[0m p4_region_scale_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_region_scale_trace)
[1m[92m    Checking[0m p4_region_scale_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_region_scale_visual)
[1m[91merror[E0560][0m[1m: struct `bevy::bevy_light::DirectionalLight` has no field named `shadows_enabled`[0m
   [1m[94m--> [0mapps/p4_region_scale_visual/src/main.rs:277:13
    [1m[94m|[0m
[1m[94m277[0m [1m[94m|[0m             shadows_enabled: true,
    [1m[94m|[0m             [1m[91m^^^^^^^^^^^^^^^[0m [1m[91munknown field[0m
    [1m[94m|[0m
[1m[96mhelp[0m: a field with a similar name exists
    [1m[94m|[0m
[1m[94m277[0m [1m[94m| [0m            shadow[92m_map[0ms_enabled: true,
    [1m[94m|[0m                   [92m++++[0m

[1m[92m    Checking[0m p4_integrated_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_integrated_scenario)
[1m[92m    Checking[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[92m    Checking[0m integrated_world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/integrated_world_visual)
[1mFor more information about this error, try `rustc --explain E0560`.[0m
[1m[91merror[0m: could not compile `p4_region_scale_visual` (bin "p4_region_scale_visual") due to 1 previous error
[1m[33mwarning[0m: build failed, waiting for other jobs to finish...
[1m[91merror[0m: could not compile `p4_region_scale_visual` (bin "p4_region_scale_visual" test) due to 1 previous error
```
