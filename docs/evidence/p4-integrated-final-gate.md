# P4 integrated final automated gate

Result: **FAIL**

Workflow run: 31266848431
Validated source: 5c45be5974c2add6044a2888770c650726cf5c64
Branch: agent/p4-integrated-world-pipeline

## Failure summary

```text
test traversal::tests::routes_are_compiled_from_detailed_terrain ... FAILED
test tests::integrated_world_round_trips_without_identity_loss ... FAILED
test tests::integrated_pipeline_is_deterministic_across_base_traversal_orders ... FAILED
test validate::tests::complete_report_passes_for_the_integrated_fixture ... FAILED
thread 'traversal::tests::routes_are_compiled_from_detailed_terrain' (4296) panicked at crates/integrated_world_core/src/traversal.rs:732:9:
thread 'tests::integrated_world_round_trips_without_identity_loss' (4295) panicked at crates/integrated_world_core/src/lib.rs:117:14:
integrated world: Validation(["routes-cross-multiple-cells: route unique materialized-cell counts are [5, 2, 4]"])
thread 'tests::integrated_pipeline_is_deterministic_across_base_traversal_orders' (4294) panicked at crates/integrated_world_core/src/lib.rs:97:14:
canonical integrated world: Validation(["routes-cross-multiple-cells: route unique materialized-cell counts are [5, 2, 4]"])
thread 'validate::tests::complete_report_passes_for_the_integrated_fixture' (4297) panicked at crates/integrated_world_core/src/validate.rs:443:9:
failed checks: ["routes-cross-multiple-cells"]
test result: FAILED. 6 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
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
[1m[92m    Checking[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[92m    Checking[0m mobile_region_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/mobile_region_core)
[1m[92m    Checking[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m    Checking[0m mobile_region_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/mobile_region_visual)
[1m[92m    Checking[0m camera_routes v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_routes)
[1m[92m    Checking[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m    Checking[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m    Checking[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m    Checking[0m building_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/building_visual)
[1m[92m    Checking[0m world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/world_visual)
[1m[92m    Checking[0m p4_world_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_world_trace)
[1m[92m    Checking[0m p3_building_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p3_building_trace)
[1m[92m    Checking[0m protocol v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/protocol)
[1m[92m    Checking[0m p4_integrated_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_integrated_scenario)
[1m[92m    Checking[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[92m    Checking[0m integrated_world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/integrated_world_visual)
[1m[92m    Checking[0m replay_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/replay_core)
[1m[92m    Finished[0m `dev` profile [optimized + debuginfo] target(s) in 6.06s
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
[1m[92m    Checking[0m integrated_world_visual v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/integrated_world_visual)
[1m[92m    Checking[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[92m    Checking[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[92m    Finished[0m `dev` profile [optimized + debuginfo] target(s) in 4.57s
## Engine-independent tests and headless applications
[1m[92m   Compiling[0m world_ids v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_ids)
[1m[92m   Compiling[0m scroll_camera_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/scroll_camera_core)
[1m[92m   Compiling[0m deterministic_rng v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/deterministic_rng)
[1m[92m   Compiling[0m world_time v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_time)
[1m[92m   Compiling[0m p1_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p1_scenario)
[1m[92m   Compiling[0m building_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/building_core)
[1m[92m   Compiling[0m world_generation_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_generation_core)
[1m[92m   Compiling[0m world_math v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/world_math)
[1m[92m   Compiling[0m mobile_region_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/mobile_region_core)
[1m[92m   Compiling[0m p2_reference_frame_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p2_reference_frame_trace)
[1m[92m   Compiling[0m camera_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/camera_trace)
[1m[92m   Compiling[0m p3_building_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p3_building_scenario)
[1m[92m   Compiling[0m integrated_world_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/integrated_world_core)
[1m[92m   Compiling[0m p4_world_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_world_scenario)
[1m[92m   Compiling[0m p4_integrated_scenario v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/p4_integrated_scenario)
[1m[92m   Compiling[0m p3_building_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p3_building_trace)
[1m[92m   Compiling[0m protocol v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/protocol)
[1m[92m   Compiling[0m p4_world_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_world_trace)
[1m[92m   Compiling[0m p4_integrated_trace v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/apps/p4_integrated_trace)
[1m[92m   Compiling[0m replay_core v0.1.0 (/home/runner/work/Prototype-Validation/Prototype-Validation/crates/replay_core)
[1m[92m    Finished[0m `test` profile [optimized + debuginfo] target(s) in 28.18s
[1m[92m     Running[0m unittests src/lib.rs (target/debug/deps/building_core-550ac495ff19caf4)

running 4 tests
test compile::tests::door_splits_wall_collision_span ... ok
test compile::tests::compilation_is_order_independent ... ok
test compile::tests::inaccessible_room_is_rejected ... ok
test delta::tests::exterior_window_rebuilds_shell_but_not_massing ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[1m[92m     Running[0m unittests src/main.rs (target/debug/deps/camera_trace-b238d7b07c8e15c4)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[1m[92m     Running[0m unittests src/lib.rs (target/debug/deps/deterministic_rng-429122b050364ec2)

running 3 tests
test tests::rejects_empty_range ... ok
test tests::feature_streams_do_not_depend_on_other_stream_consumption ... ok
test tests::splitmix_sequence_is_stable ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[1m[92m     Running[0m unittests src/lib.rs (target/debug/deps/integrated_world_core-6604a7512c703b3b)

running 10 tests
test atlas::tests::atlas_contains_rich_per_cell_summaries ... ok
test atlas::tests::detailed_height_is_continuous_around_shared_edges ... ok
test terrain::tests::adjacent_cells_reuse_the_exact_same_edge_contract ... ok
test history::tests::history_explains_current_assets ... ok
test terrain::tests::detailed_summary_matches_atlas_within_sampling_tolerance ... ok
test terrain::tests::materializes_a_mountain_to_ocean_region ... ok
test traversal::tests::routes_are_compiled_from_detailed_terrain ... FAILED
test tests::integrated_world_round_trips_without_identity_loss ... FAILED
test tests::integrated_pipeline_is_deterministic_across_base_traversal_orders ... FAILED
test validate::tests::complete_report_passes_for_the_integrated_fixture ... FAILED

failures:

---- traversal::tests::routes_are_compiled_from_detailed_terrain stdout ----

thread 'traversal::tests::routes_are_compiled_from_detailed_terrain' (4296) panicked at crates/integrated_world_core/src/traversal.rs:732:9:
assertion failed: validate_traversal(&atlas, &detailed,
            &traversal).iter().all(|check| check.passed)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- tests::integrated_world_round_trips_without_identity_loss stdout ----

thread 'tests::integrated_world_round_trips_without_identity_loss' (4295) panicked at crates/integrated_world_core/src/lib.rs:117:14:
integrated world: Validation(["routes-cross-multiple-cells: route unique materialized-cell counts are [5, 2, 4]"])

---- tests::integrated_pipeline_is_deterministic_across_base_traversal_orders stdout ----

thread 'tests::integrated_pipeline_is_deterministic_across_base_traversal_orders' (4294) panicked at crates/integrated_world_core/src/lib.rs:97:14:
canonical integrated world: Validation(["routes-cross-multiple-cells: route unique materialized-cell counts are [5, 2, 4]"])

---- validate::tests::complete_report_passes_for_the_integrated_fixture stdout ----

thread 'validate::tests::complete_report_passes_for_the_integrated_fixture' (4297) panicked at crates/integrated_world_core/src/validate.rs:443:9:
failed checks: ["routes-cross-multiple-cells"]


failures:
    tests::integrated_pipeline_is_deterministic_across_base_traversal_orders
    tests::integrated_world_round_trips_without_identity_loss
    traversal::tests::routes_are_compiled_from_detailed_terrain
    validate::tests::complete_report_passes_for_the_integrated_fixture

test result: FAILED. 6 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s

[1m[91merror[0m: test failed, to rerun pass `-p integrated_world_core --lib`
```
