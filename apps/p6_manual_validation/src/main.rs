#![forbid(unsafe_code)]

use p6_validation_harness::run_validation;
use std::{env, io, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("\nP6 MANUAL VALIDATION FAILED TO COMPLETE: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut auto = false;
    let mut data_dir = PathBuf::from("p6-data");
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--auto" => auto = true,
            "--data-dir" => {
                data_dir = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--data-dir requires a path")?;
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    println!("P6 simulation tiers / event calendar / million-person validation");
    println!("Data directory: {}", data_dir.display());
    println!("This run constructs 1,000,000 Cold, 10,000 Warm and 300 Hot people.");
    println!("It performs two independent deterministic scale runs.");
    println!("The console may be busy for several seconds while the compact population is built.\n");

    let report = run_validation(&data_dir, |step| {
        println!(
            "[{}] {:02}. {}\n      {}",
            if step.passed { "PASS" } else { "FAIL" },
            step.index,
            step.title,
            step.detail
        );
        if !auto {
            println!("      Press Enter to continue...");
            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);
        }
    })?;

    println!("\nP6 summary");
    println!("  all_passed: {}", report.all_passed);
    println!(
        "  model memory: {:.2} MiB",
        report.first_run.memory.total_mebibytes()
    );
    println!("  first run: {} ms", report.first_run.timings_ms.total);
    println!("  repeat run: {} ms", report.repeat_run.timings_ms.total);
    println!("  fingerprint: {:?}", report.first_run.fingerprint);
    println!("  report: {}", report.report_file);
    println!("  transition evidence: {}", report.transition_file);

    if !report.all_passed {
        return Err("one or more P6 checks failed".into());
    }
    Ok(())
}
