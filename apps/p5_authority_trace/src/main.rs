#![forbid(unsafe_code)]

use p5_validation_harness::run_validation;
use std::{env, fs, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("P5 authority trace failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/p5-authority-trace.json"));
    let data_dir = output.with_extension("data");
    let report = run_validation(&data_dir, |step| {
        println!(
            "[{}] {}: {} — {}",
            if step.passed { "PASS" } else { "FAIL" },
            step.index,
            step.title,
            step.detail
        );
    })?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("P5 trace written to {}", output.display());
    if !report.all_passed {
        return Err("one or more P5 validation checks failed".into());
    }
    Ok(())
}
