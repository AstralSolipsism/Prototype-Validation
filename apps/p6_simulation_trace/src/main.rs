#![forbid(unsafe_code)]

use p6_validation_harness::run_validation;
use std::{env, fs, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("P6 simulation trace failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("p6-simulation-scale-trace.json"));
    let root = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("p6-simulation-scale-trace.data");
    let report = run_validation(&root, |step| {
        println!(
            "[{}] {} — {}",
            if step.passed { "PASS" } else { "FAIL" },
            step.title,
            step.detail
        );
    })?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("P6 trace written to {}", output.display());
    if !report.all_passed {
        return Err("P6 report contains failed checks".into());
    }
    Ok(())
}
