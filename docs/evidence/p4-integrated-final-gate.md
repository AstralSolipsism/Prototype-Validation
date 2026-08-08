# P4 integrated final automated gate

Result: **FAIL**

Workflow run: 31265998665
Validated source: df35240384b28da7db6547ff8457d81725986c1b
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
[1m[92m    Checking[0m mobile_region_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/mobile_region_visual)
[1m[92m    Checking[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m    Checking[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m    Checking[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m    Checking[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m    Checking[0m p3_building_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p3_building_trace)
[1m[92m    Checking[0m p4_world_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_world_trace)
[1m[92m    Checking[0m world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/world_visual)
[1m[92m    Checking[0m building_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/building_visual)
[1m[92m    Checking[0m p4_integrated_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_integrated_scenario)
[1m[92m    Checking[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[92m    Checking[0m integrated_world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/integrated_world_visual)
[1m[92m    Checking[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[92m    Checking[0m replay_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/replay_core)
[1m[92m    Checking[0m protocol v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/protocol)
[1m[92m    Finished[0m `dev` profile [optimized + debuginfo] target(s) in 5.39s
## Complete workspace strict Clippy
[1m[92m    Checking[0m world_ids v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_ids)
[1m[92m    Checking[0m scroll_camera_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/scroll_camera_core)
[1m[92m    Checking[0m deterministic_rng v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/deterministic_rng)
[1m[92m    Checking[0m world_time v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_time)
[1m[92m    Checking[0m p1_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p1_scenario)
[1m[92m    Checking[0m camera_routes v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_routes)
[1m[92m    Checking[0m building_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/building_core)
[1m[92m    Checking[0m world_generation_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_generation_core)
[1m[92m    Checking[0m world_math v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_math)
[1m[92m    Checking[0m replay_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/replay_core)
[1m[92m    Checking[0m mobile_region_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/mobile_region_core)
[1m[92m    Checking[0m mobile_region_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/mobile_region_visual)
[1m[92m    Checking[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m    Checking[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m    Checking[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m    Checking[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m    Checking[0m world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/world_visual)
[1m[92m    Checking[0m p4_world_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_world_trace)
[1m[92m    Checking[0m building_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/building_visual)
[1m[92m    Checking[0m p3_building_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p3_building_trace)
[1m[92m    Checking[0m protocol v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/protocol)
[1m[92m    Checking[0m p4_integrated_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_integrated_scenario)
[1m[92m    Checking[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[92m    Checking[0m integrated_world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/integrated_world_visual)
[1m[92m    Checking[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[91merror[0m[1m: this function has too many arguments (8/7)[0m
   [1m[94m--> [0mapps/integrated_world_visual/src/main.rs:917:1
    [1m[94m|[0m
[1m[94m917[0m [1m[94m|[0m [1m[91m/[0m fn update_camera_and_route_subject(
[1m[94m918[0m [1m[94m|[0m [1m[91m|[0m     time: Res<Time>,
[1m[94m919[0m [1m[94m|[0m [1m[91m|[0m     keyboard: Res<ButtonInput<KeyCode>>,
[1m[94m920[0m [1m[94m|[0m [1m[91m|[0m     world: Res<WorldData>,
[1m[94m...[0m   [1m[91m|[0m
[1m[94m925[0m [1m[94m|[0m [1m[91m|[0m     mut subject: Query<&mut Transform, (With<RouteSubject>, Without<PrototypeCamera>)>,
[1m[94m926[0m [1m[94m|[0m [1m[91m|[0m ) {
    [1m[94m|[0m [1m[91m|_^[0m
    [1m[94m|[0m
    [1m[94m= [0m[1mhelp[0m: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#too_many_arguments
    [1m[94m= [0m[1mnote[0m: `-D clippy::too-many-arguments` implied by `-D warnings`
    [1m[94m= [0m[1mhelp[0m: to override `-D warnings` add `#[allow(clippy::too_many_arguments)]`

[1m[91merror[0m[1m: this function has too many arguments (8/7)[0m
    [1m[94m--> [0mapps/integrated_world_visual/src/main.rs:1367:1
     [1m[94m|[0m
[1m[94m1367[0m [1m[94m|[0m [1m[91m/[0m fn spawn_segment(
[1m[94m1368[0m [1m[94m|[0m [1m[91m|[0m     commands: &mut Commands,
[1m[94m1369[0m [1m[94m|[0m [1m[91m|[0m     assets: &VisualAssets,
[1m[94m1370[0m [1m[94m|[0m [1m[91m|[0m     material: Handle<StandardMaterial>,
[1m[94m...[0m    [1m[91m|[0m
[1m[94m1375[0m [1m[94m|[0m [1m[91m|[0m     tag: VisualTag,
[1m[94m1376[0m [1m[94m|[0m [1m[91m|[0m ) {
     [1m[94m|[0m [1m[91m|_^[0m
     [1m[94m|[0m
     [1m[94m= [0m[1mhelp[0m: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#too_many_arguments

[1m[91merror[0m: could not compile `integrated_world_visual` (bin "integrated_world_visual") due to 2 previous errors
[1m[33mwarning[0m: build failed, waiting for other jobs to finish...
[1m[91merror[0m: could not compile `integrated_world_visual` (bin "integrated_world_visual" test) due to 2 previous errors
```
