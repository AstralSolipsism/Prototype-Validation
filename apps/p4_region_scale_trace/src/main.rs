#![forbid(unsafe_code)]

use p4_region_scale_scenario::generate_region_scale_world;
use region_scale_core::validate_world;
use std::{env, fs, path::PathBuf};

fn main() {
    let output = env::args_os().nth(1).map(PathBuf::from);
    let world = generate_region_scale_world().expect("region-scale world");
    let report = validate_world(&world).expect("region-scale validation");
    assert!(
        report.all_passed(),
        "failed checks: {:?}",
        report
            .checks
            .iter()
            .filter(|check| !check.passed)
            .collect::<Vec<_>>()
    );
    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(path, json).expect("write report");
    } else {
        println!("{json}");
    }
}
