# P4 integrated final automated gate

Result: **FAIL**

Workflow run: 31290464612
Validated source: 7d67561357a53fd3d528eed267fc723f6f5a5dd4
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
[1m[92m    Checking[0m mobile_region_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/mobile_region_visual)
[1m[92m    Checking[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m    Checking[0m region_scale_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/region_scale_core)
[1m[92m    Checking[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m    Checking[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m    Checking[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m    Checking[0m p4_world_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_world_trace)
[1m[92m    Checking[0m world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/world_visual)
[1m[91merror[E0382][0m[1m: use of moved value: `summary.0`[0m
   [1m[94m--> [0mcrates/region_scale_core/src/lib.rs:680:32
    [1m[94m|[0m
[1m[94m671[0m [1m[94m|[0m             elevation: summary.0,
    [1m[94m|[0m                        [1m[94m---------[0m [1m[94mvalue moved here[0m
[1m[94m...[0m
[1m[94m680[0m [1m[94m|[0m             carrying_capacity: summary.0.buildable_fraction
    [1m[94m|[0m                                [1m[91m^^^^^^^^^^^^^^^^^^^^^^^^^^^^[0m [1m[91mvalue used here after move[0m
    [1m[94m|[0m
    [1m[94m= [0m[1mnote[0m: move occurs because `summary.0` has type `ElevationSummary`, which does not implement the `Copy` trait

[1m[91merror[E0382][0m[1m: borrow of moved value: `summary.1`[0m
   [1m[94m--> [0mcrates/region_scale_core/src/lib.rs:682:23
    [1m[94m|[0m
[1m[94m672[0m [1m[94m|[0m               landforms: summary.1,
    [1m[94m|[0m                          [1m[94m---------[0m [1m[94mvalue moved here[0m
[1m[94m...[0m
[1m[94m682[0m [1m[94m|[0m                       + summary
    [1m[94m|[0m [1m[91m _______________________^[0m
[1m[94m683[0m [1m[94m|[0m [1m[91m|[0m                         .1
[1m[94m684[0m [1m[94m|[0m [1m[91m|[0m                         .fractions
    [1m[94m|[0m [1m[91m|__________________________________^[0m [1m[91mvalue borrowed here after move[0m
    [1m[94m|[0m
    [1m[94m= [0m[1mnote[0m: move occurs because `summary.1` has type `LandformMix`, which does not implement the `Copy` trait

[1mFor more information about this error, try `rustc --explain E0382`.[0m
[1m[91merror[0m: could not compile `region_scale_core` (lib) due to 2 previous errors
[1m[33mwarning[0m: build failed, waiting for other jobs to finish...
[1m[91merror[0m: could not compile `region_scale_core` (lib test) due to 2 previous errors
```
