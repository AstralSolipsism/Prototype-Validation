#![forbid(unsafe_code)]

use glam::{DQuat, DVec3};
use mobile_region_core::{FrameBoundPose, VehiclePoseState, local_point, transfer_pose};
use serde::Serialize;
use std::{env, error::Error, fs, path::PathBuf};
use world_ids::FrameId;
use world_math::{CameraRelativePose, ReferenceFrameGraph, RigidTransform};

const STEPS: usize = 3_600;
const DELTA_SECONDS: f64 = 1.0 / 60.0;
const CAMERA_RELATIVE_LIMIT_METERS: f64 = 2.0e-4;
const TRANSFER_LIMIT_METERS: f64 = 1.0e-8;
const INTERIOR_DISTANCE_LIMIT_METERS: f64 = 1.0e-8;

#[derive(Debug, Serialize)]
struct TraceReport {
    schema_version: u32,
    samples: usize,
    world_origin_meters: [f64; 3],
    max_interior_distance_error_meters: f64,
    max_frame_transfer_translation_error_meters: f64,
    max_frame_transfer_rotation_error_radians: f64,
    max_camera_relative_quantization_error_meters: f64,
    non_finite_samples: usize,
    transfer_count: usize,
    thresholds: Thresholds,
    passed: bool,
}

#[derive(Debug, Serialize)]
struct Thresholds {
    max_interior_distance_error_meters: f64,
    max_frame_transfer_translation_error_meters: f64,
    max_camera_relative_quantization_error_meters: f64,
    non_finite_samples: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/p2-reference-frame-trace.json"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let report = run_trace()?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("wrote {}", output.display());
    println!("{report:#?}");

    if report.passed {
        Ok(())
    } else {
        Err("P2 reference-frame trace exceeded a validation threshold".into())
    }
}

fn run_trace() -> Result<TraceReport, Box<dyn Error>> {
    let world = FrameId::from_u128(1);
    let vehicle = FrameId::from_u128(2);
    let base = DVec3::new(9_000_000.0, 18.0, -7_000_000.0);
    let mut frames = ReferenceFrameGraph::default();
    frames.insert_root(world)?;
    frames.insert_child(vehicle, world, RigidTransform::IDENTITY)?;

    let cabin_left = local_point(vehicle, DVec3::new(-8.0, 1.5, 2.0));
    let cabin_right = local_point(vehicle, DVec3::new(9.0, 4.0, -3.0));
    let expected_interior_distance = cabin_left
        .local_pose
        .translation
        .distance(cabin_right.local_pose.translation);
    let camera_local = RigidTransform::new(DVec3::new(-2.0, 5.0, 13.0), DQuat::IDENTITY)?;
    let table_local = RigidTransform::new(DVec3::new(3.0, 1.0, -2.0), DQuat::IDENTITY)?;
    let mut actor = FrameBoundPose::new(
        vehicle,
        RigidTransform::new(DVec3::new(0.0, 1.0, 0.0), DQuat::IDENTITY)?,
    );

    let mut max_interior_error: f64 = 0.0;
    let mut max_transfer_translation_error: f64 = 0.0;
    let mut max_transfer_rotation_error: f64 = 0.0;
    let mut max_camera_relative_error: f64 = 0.0;
    let mut non_finite_samples = 0;
    let mut transfer_count = 0;

    for step in 0..STEPS {
        let time = step as f64 * DELTA_SECONDS;
        let angle = time * 0.12;
        let authoritative = RigidTransform::new(
            base + DVec3::new(
                angle.cos() * 120.0,
                (time * 0.3).sin() * 0.5,
                angle.sin() * 120.0,
            ),
            DQuat::from_rotation_y(-angle),
        )?;
        let presentation_offset = RigidTransform::new(
            DVec3::new(0.0, (time * 1.7).sin() * 0.18, 0.0),
            (DQuat::from_rotation_z((time * 1.1).sin() * 0.025)
                * DQuat::from_rotation_x((time * 0.9).sin() * 0.015))
            .normalize(),
        )?;
        let vehicle_state =
            VehiclePoseState::new(authoritative).with_presentation_offset(presentation_offset);
        frames.set_pose_in_parent(vehicle, vehicle_state.authoritative_in_parent)?;

        let interior_distance = cabin_left
            .world_pose(&frames)?
            .translation
            .distance(cabin_right.world_pose(&frames)?.translation);
        max_interior_error =
            max_interior_error.max((interior_distance - expected_interior_distance).abs());

        let presentation_world = vehicle_state.presentation_in_parent();
        let camera_world = presentation_world.compose(camera_local);
        let table_world = presentation_world.compose(table_local);
        let relative_f64 = table_world.relative_to(camera_world).translation;
        let relative_f32 = CameraRelativePose::from_world(table_world, camera_world)
            .translation
            .as_dvec3();
        max_camera_relative_error =
            max_camera_relative_error.max(relative_f64.distance(relative_f32));

        if !interior_distance.is_finite() || !relative_f64.is_finite() || !relative_f32.is_finite()
        {
            non_finite_samples += 1;
        }

        if step == STEPS / 3 {
            let transfer = transfer_pose(actor, world, &frames)?;
            max_transfer_translation_error =
                max_transfer_translation_error.max(transfer.translation_error_meters);
            max_transfer_rotation_error =
                max_transfer_rotation_error.max(transfer.rotation_error_radians);
            actor = transfer.after;
            transfer_count += 1;
        }
        if step == STEPS * 2 / 3 {
            let transfer = transfer_pose(actor, vehicle, &frames)?;
            max_transfer_translation_error =
                max_transfer_translation_error.max(transfer.translation_error_meters);
            max_transfer_rotation_error =
                max_transfer_rotation_error.max(transfer.rotation_error_radians);
            actor = transfer.after;
            transfer_count += 1;
        }
    }

    let passed = max_interior_error <= INTERIOR_DISTANCE_LIMIT_METERS
        && max_transfer_translation_error <= TRANSFER_LIMIT_METERS
        && max_camera_relative_error <= CAMERA_RELATIVE_LIMIT_METERS
        && non_finite_samples == 0
        && transfer_count == 2;

    Ok(TraceReport {
        schema_version: 1,
        samples: STEPS,
        world_origin_meters: base.to_array(),
        max_interior_distance_error_meters: max_interior_error,
        max_frame_transfer_translation_error_meters: max_transfer_translation_error,
        max_frame_transfer_rotation_error_radians: max_transfer_rotation_error,
        max_camera_relative_quantization_error_meters: max_camera_relative_error,
        non_finite_samples,
        transfer_count,
        thresholds: Thresholds {
            max_interior_distance_error_meters: INTERIOR_DISTANCE_LIMIT_METERS,
            max_frame_transfer_translation_error_meters: TRANSFER_LIMIT_METERS,
            max_camera_relative_quantization_error_meters: CAMERA_RELATIVE_LIMIT_METERS,
            non_finite_samples: 0,
        },
        passed,
    })
}
