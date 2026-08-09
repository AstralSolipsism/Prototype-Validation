#![forbid(unsafe_code)]

// Headless evidence entrypoint for the P7 region-authority stop line.
use p7_validation_harness::run_validation;
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("p7-region-authority-trace.json"));
    if let Some(parent) = output.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let report = run_validation(|_| {})?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("P7 trace written to {}", output.display());
    println!(
        "checks={}/{}",
        report.checks.iter().filter(|step| step.passed).count(),
        report.checks.len()
    );
    println!("all_passed={}", report.all_passed);
    println!("fingerprints_match={}", report.fingerprints_match);
    Ok(())
}
