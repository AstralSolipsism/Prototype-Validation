#![forbid(unsafe_code)]

use p7_validation_harness::{ValidationStep, run_validation};
use serde_json::json;
use std::{env, fs, io, path::PathBuf};

#[derive(Clone, Debug)]
struct Options {
    auto: bool,
    data_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_options()?;
    fs::create_dir_all(&options.data_dir)?;
    println!("P7 multiplayer interest / region authority validation");
    println!("Data directory: {}", options.data_dir.display());
    println!("The scenario uses four independent client projections, three static regions and one mobile ship region.");
    println!();

    let auto = options.auto;
    let report = run_validation(|step| display_step(step, auto))?;
    let report_path = options.data_dir.join("p7-validation-report.json");
    let interests_path = options.data_dir.join("p7-client-interests.json");
    let transfers_path = options.data_dir.join("p7-transfer-evidence.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        &interests_path,
        serde_json::to_vec_pretty(&report.first_run.projections)?,
    )?;
    fs::write(
        &transfers_path,
        serde_json::to_vec_pretty(&json!({
            "transfer": &report.first_run.transfer,
            "mobile": &report.first_run.mobile,
            "owners_before_rebalance": &report.first_run.owners_before_rebalance,
            "owners_after_rebalance": &report.first_run.owners_after_rebalance,
            "stale_writer_rejected": report.first_run.stale_writer_rejected,
            "parallel": &report.first_run.parallel,
            "hotspot": &report.first_run.hotspot,
        }))?,
    )?;

    println!();
    println!("P7 report: {}", report_path.display());
    println!("Interest evidence: {}", interests_path.display());
    println!("Transfer/authority evidence: {}", transfers_path.display());
    println!("Checks: {}/{}", report.checks.iter().filter(|step| step.passed).count(), report.checks.len());
    println!("Fingerprints match: {}", report.fingerprints_match);
    println!("Final verdict: {}", if report.all_passed { "PASS" } else { "FAIL" });
    if !report.all_passed {
        std::process::exit(1);
    }
    Ok(())
}

fn parse_options() -> Result<Options, String> {
    let mut auto = false;
    let mut data_dir = PathBuf::from("p7-data");
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--auto" => auto = true,
            "--data-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--data-dir requires a path".to_owned())?;
                data_dir = PathBuf::from(value);
            }
            "--help" | "-h" => {
                println!("p7_manual_validation [--auto] [--data-dir PATH]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Options { auto, data_dir })
}

fn display_step(step: &ValidationStep, auto: bool) {
    println!(
        "[{}] {:02}. {}",
        if step.passed { "PASS" } else { "FAIL" },
        step.index,
        step.title
    );
    println!("       {}", step.detail);
    if !auto {
        println!("       Press Enter to continue.");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}
