#![forbid(unsafe_code)]

use p1_scenario::{TravelDirection, grammar_at, route};
use scroll_camera_core::{CameraPose, CameraRigConfig, CameraRigState, ViewSide};
use serde::Serialize;
use std::{env, fs, path::PathBuf};

const SAMPLE_RATE_HZ: f64 = 60.0;
const ROUTE_SPEED_MPS: f64 = 7.0;

#[derive(Debug, Serialize)]
struct TraceReport {
    schema_version: u32,
    stage: &'static str,
    status: &'static str,
    route_length_m: f64,
    thresholds: TraceThresholds,
    passes: Vec<PassMetrics>,
    observations: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct TraceThresholds {
    max_camera_speed_mps: f64,
    max_camera_angular_speed_deg_s: f64,
    min_subject_distance_m: f64,
    max_subject_distance_m: f64,
}

#[derive(Debug, Serialize)]
struct PassMetrics {
    direction: &'static str,
    samples: usize,
    duration_s: f64,
    max_camera_speed_mps: f64,
    max_camera_angular_speed_deg_s: f64,
    min_subject_distance_m: f64,
    max_subject_distance_m: f64,
    all_values_finite: bool,
    passed: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = generate_report()?;
    let json = serde_json::to_string_pretty(&report)?;

    if let Some(path) = env::args_os().nth(1) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, format!("{json}\n"))?;
        println!("wrote {}", path.display());
    } else {
        println!("{json}");
    }

    if report.status != "automated-thresholds-passed" {
        std::process::exit(2);
    }
    Ok(())
}

fn generate_report() -> Result<TraceReport, Box<dyn std::error::Error>> {
    let route = route()?;
    let thresholds = TraceThresholds {
        max_camera_speed_mps: 45.0,
        max_camera_angular_speed_deg_s: 100.0,
        min_subject_distance_m: 15.0,
        max_subject_distance_m: 30.0,
    };
    let passes = vec![
        trace_pass(TravelDirection::Forward, thresholds)?,
        trace_pass(TravelDirection::Reverse, thresholds)?,
    ];
    let passed = passes.iter().all(|pass| pass.passed);

    Ok(TraceReport {
        schema_version: 1,
        stage: "P1-complex-scroll-camera",
        status: if passed {
            "automated-thresholds-passed"
        } else {
            "automated-thresholds-failed"
        },
        route_length_m: route.total_length(),
        thresholds,
        passes,
        observations: vec![
            "This report validates kinematic continuity in both directions.",
            "It does not prove occlusion, composition, motion comfort, or visual correctness.",
            "P1 remains open until GPU review and screenshot regression are complete.",
        ],
    })
}

fn trace_pass(
    direction: TravelDirection,
    thresholds: TraceThresholds,
) -> Result<PassMetrics, Box<dyn std::error::Error>> {
    let route = route()?;
    let step = 1.0 / SAMPLE_RATE_HZ;
    let duration = route.total_length() / ROUTE_SPEED_MPS;
    let samples = (duration * SAMPLE_RATE_HZ).ceil() as usize + 1;
    let start_distance = match direction {
        TravelDirection::Forward => 0.0,
        TravelDirection::Reverse => route.total_length(),
    };
    let start_frame = route.sample(start_distance)?;
    let mut rig = CameraRigState::new(
        CameraRigConfig::default(),
        start_frame.tangent * direction.sign(),
        ViewSide::Left,
    )?;

    let mut previous_pose: Option<CameraPose> = None;
    let mut max_speed: f64 = 0.0;
    let mut max_angular_speed: f64 = 0.0;
    let mut min_distance = f64::INFINITY;
    let mut max_distance: f64 = 0.0;
    let mut all_finite = true;

    for index in 0..samples {
        let travelled = index as f64 * step * ROUTE_SPEED_MPS;
        let distance = match direction {
            TravelDirection::Forward => travelled.min(route.total_length()),
            TravelDirection::Reverse => (route.total_length() - travelled).max(0.0),
        };
        let frame = route.sample(distance)?;
        let subject = frame.position + glam::DVec3::Y * 0.75;
        let pose = rig.update(
            subject,
            frame.tangent * direction.sign(),
            grammar_at(distance, route.total_length(), direction),
            step,
        )?;

        let subject_distance = pose.position.distance(subject);
        min_distance = min_distance.min(subject_distance);
        max_distance = max_distance.max(subject_distance);
        all_finite &= pose.position.is_finite() && pose.target.is_finite() && pose.up.is_finite();

        if let Some(previous) = previous_pose {
            max_speed = max_speed.max(previous.position.distance(pose.position) / step);
            let dot = previous.forward().dot(pose.forward()).clamp(-1.0, 1.0);
            max_angular_speed = max_angular_speed.max(dot.acos().to_degrees() / step);
        }
        previous_pose = Some(pose);
    }

    let passed = all_finite
        && max_speed <= thresholds.max_camera_speed_mps
        && max_angular_speed <= thresholds.max_camera_angular_speed_deg_s
        && min_distance >= thresholds.min_subject_distance_m
        && max_distance <= thresholds.max_subject_distance_m;

    Ok(PassMetrics {
        direction: direction.label(),
        samples,
        duration_s: duration,
        max_camera_speed_mps: max_speed,
        max_camera_angular_speed_deg_s: max_angular_speed,
        min_subject_distance_m: min_distance,
        max_subject_distance_m: max_distance,
        all_values_finite: all_finite,
        passed,
    })
}
