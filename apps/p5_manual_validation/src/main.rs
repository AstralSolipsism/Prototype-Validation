#![forbid(unsafe_code)]

use p5_validation_harness::run_validation;
use std::{env, io, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("\nP5 MANUAL VALIDATION FAILED TO COMPLETE: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let automatic = args.iter().any(|argument| argument == "--auto");
    let data_dir = args
        .windows(2)
        .find(|pair| pair[0] == "--data-dir")
        .map(|pair| PathBuf::from(&pair[1]))
        .unwrap_or_else(|| PathBuf::from("p5-data"));

    println!("P5 authoritative server / persistence / recovery validation");
    println!("Data directory: {}", data_dir.display());
    println!("The program uses one real loopback TCP server and two logical TCP clients.");
    if !automatic {
        println!("Press Enter after reading each result to continue.\n");
    }

    let report = run_validation(&data_dir, |step| {
        println!(
            "\n[{}] STEP {} — {}\n{}",
            if step.passed { "PASS" } else { "FAIL" },
            step.index,
            step.title,
            step.detail
        );
        if !automatic {
            println!("Press Enter to continue...");
            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);
        }
    })?;

    println!("\n============================================================");
    println!(
        "FINAL RESULT: {}",
        if report.all_passed { "PASS" } else { "FAIL" }
    );
    println!("Final revision: {}", report.final_revision.0);
    println!("Journal records: {}", report.journal_records);
    println!("Duplicate receipts: {}", report.duplicate_receipts);
    println!("Rejected receipts: {}", report.rejected_receipts);
    println!(
        "Buffered out-of-order commands: {}",
        report.buffered_out_of_order_commands
    );
    println!("Snapshot: {}", report.snapshot_file);
    println!("Journal: {}", report.journal_file);
    println!(
        "Report: {}",
        data_dir.join("p5-validation-report.json").display()
    );
    println!("============================================================");

    if !report.all_passed {
        return Err("one or more manual validation checks failed".into());
    }
    Ok(())
}
