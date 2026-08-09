#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use simulation_scale_core::{ScaleConfig, ScaleError, ScaleRunSummary, run_scale_scenario};
use std::{fs, path::Path};
use thiserror::Error;

const MAX_MODEL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SINGLE_RUN_MS: u128 = 120_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationStep {
    pub index: usize,
    pub title: String,
    pub detail: String,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct P6ValidationReport {
    pub schema_version: u32,
    pub checks: Vec<ValidationStep>,
    pub first_run: ScaleRunSummary,
    pub repeat_run: ScaleRunSummary,
    pub fingerprints_match: bool,
    pub report_file: String,
    pub transition_file: String,
    pub all_passed: bool,
}

pub fn run_validation(
    root: impl AsRef<Path>,
    mut observe: impl FnMut(&ValidationStep),
) -> Result<P6ValidationReport, HarnessError> {
    let root = root.as_ref();
    fs::create_dir_all(root)?;
    let config = ScaleConfig::acceptance();
    let first = run_scale_scenario(config)?;
    let repeat = run_scale_scenario(config)?;
    let mut checks = Vec::new();

    record(
        &mut checks,
        &mut observe,
        "population tiers are materialized",
        format!(
            "cold={}, warm={}, hot={}",
            first.config.cold_population,
            first.config.warm_population,
            first.config.hot_population
        ),
        first.config.cold_population == 1_000_000
            && first.config.warm_population == 10_000
            && first.config.hot_population == 300,
    );

    record(
        &mut checks,
        &mut observe,
        "compact model-owned memory budget",
        format!(
            "model-owned bytes={} ({:.2} MiB), gate={} MiB",
            first.memory.total_model_bytes,
            first.memory.total_mebibytes(),
            MAX_MODEL_BYTES / 1024 / 1024
        ),
        first.memory.total_model_bytes <= MAX_MODEL_BYTES,
    );

    let expected_cold_events = first.config.cold_population as u64
        * u64::from(first.config.accelerated_days);
    record(
        &mut checks,
        &mut observe,
        "cold population uses due buckets",
        format!(
            "{} due settlements, {} full-population scans",
            first.cold_metrics.due_events, first.cold_metrics.full_population_scans
        ),
        first.cold_metrics.due_events == expected_cold_events
            && first.cold_metrics.full_population_scans == 0,
    );

    let warm_upper_bound = first.config.warm_population as u64
        * u64::from(first.config.accelerated_days)
        * 8;
    record(
        &mut checks,
        &mut observe,
        "warm population uses an event calendar",
        format!(
            "{} due events, {} full-population scans",
            first.warm_metrics.events_processed, first.warm_metrics.full_population_scans
        ),
        first.warm_metrics.events_processed > first.config.warm_population as u64
            && first.warm_metrics.events_processed < warm_upper_bound
            && first.warm_metrics.full_population_scans == 0,
    );

    let expected_hot_updates = first.config.hot_population as u64
        * u64::from(first.config.hot_ticks);
    record(
        &mut checks,
        &mut observe,
        "hot population runs fixed-step simulation",
        format!(
            "{} updates at {} Hz-equivalent fixed steps",
            first.hot_metrics.fixed_step_updates,
            simulation_scale_core::HOT_TICKS_PER_SECOND
        ),
        first.hot_metrics.fixed_step_updates == expected_hot_updates,
    );

    record(
        &mut checks,
        &mut observe,
        "accelerated time does not scan one million people per second",
        format!(
            "actual cold settlements={} versus hypothetical per-second updates={}",
            first.actual_cold_due_events, first.hypothetical_per_second_cold_updates
        ),
        u128::from(first.actual_cold_due_events) * 1_000
            < first.hypothetical_per_second_cold_updates,
    );

    record(
        &mut checks,
        &mut observe,
        "tier round trip preserves persistent facts",
        format!(
            "{} people completed Cold→Warm→Hot→Warm→Cold; {} summaries changed",
            first.transition.people_exercised, first.transition.summaries_changed
        ),
        first.transition.people_exercised == first.config.transition_people
            && first.transition.facts_preserved
            && first.transition.summaries_changed == first.config.transition_people
            && first.transition.final_cold_count == first.config.cold_population,
    );

    record(
        &mut checks,
        &mut observe,
        "hot runtime cache is disposable and rebuildable",
        format!(
            "cleared={}, rebuilt={}, valid={}",
            first.hot_cache_entries_cleared,
            first.hot_cache_entries_rebuilt,
            first.hot_caches_valid_after_rebuild
        ),
        first.hot_cache_entries_cleared == first.config.hot_population
            && first.hot_cache_entries_rebuilt == first.config.hot_population
            && first.hot_caches_valid_after_rebuild,
    );

    let internal_updates = first.cold_metrics.due_events
        + first.warm_metrics.events_processed
        + first.hot_metrics.fixed_step_updates;
    record(
        &mut checks,
        &mut observe,
        "history stores significant events rather than every micro-update",
        format!(
            "history events={} across {} internal updates",
            first.history_events, internal_updates
        ),
        first.history_events > 0
            && (first.history_events as u64).saturating_mul(1_000) < internal_updates,
    );

    let fingerprints_match = first.fingerprint == repeat.fingerprint;
    record(
        &mut checks,
        &mut observe,
        "independent runs are deterministic",
        format!(
            "first={:?}, repeat={:?}",
            first.fingerprint, repeat.fingerprint
        ),
        fingerprints_match,
    );

    record(
        &mut checks,
        &mut observe,
        "scale run completes within the prototype budget",
        format!(
            "first={} ms, repeat={} ms, per-run gate={} ms",
            first.timings_ms.total, repeat.timings_ms.total, MAX_SINGLE_RUN_MS
        ),
        first.timings_ms.total <= MAX_SINGLE_RUN_MS
            && repeat.timings_ms.total <= MAX_SINGLE_RUN_MS,
    );

    record(
        &mut checks,
        &mut observe,
        "repeat run reproduces scale metrics",
        format!(
            "cold={}, warm={}, hot={}, history={}",
            repeat.cold_metrics.due_events,
            repeat.warm_metrics.events_processed,
            repeat.hot_metrics.fixed_step_updates,
            repeat.history_events
        ),
        first.cold_metrics == repeat.cold_metrics
            && first.warm_metrics == repeat.warm_metrics
            && first.hot_metrics == repeat.hot_metrics
            && first.transition == repeat.transition
            && first.history_events == repeat.history_events,
    );

    let report_file = root.join("p6-validation-report.json");
    let transition_file = root.join("p6-tier-transition.json");
    let all_passed = checks.iter().all(|check| check.passed);
    let report = P6ValidationReport {
        schema_version: 1,
        checks,
        first_run: first,
        repeat_run: repeat,
        fingerprints_match,
        report_file: report_file.display().to_string(),
        transition_file: transition_file.display().to_string(),
        all_passed,
    };

    fs::write(&transition_file, serde_json::to_vec_pretty(&report.first_run.transition)?)?;
    fs::write(&report_file, serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

fn record(
    checks: &mut Vec<ValidationStep>,
    observe: &mut impl FnMut(&ValidationStep),
    title: &str,
    detail: String,
    passed: bool,
) {
    let step = ValidationStep {
        index: checks.len() + 1,
        title: title.to_owned(),
        detail,
        passed,
    };
    observe(&step);
    checks.push(step);
}

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("P6 scale simulation failed: {0}")]
    Scale(#[from] ScaleError),
    #[error("P6 report I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("P6 report JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn report_schema_round_trips_on_a_small_direct_core_run() {
        let run = run_scale_scenario(ScaleConfig::small_test()).expect("small run");
        let encoded = serde_json::to_vec(&run).expect("encode");
        let decoded: ScaleRunSummary = serde_json::from_slice(&encoded).expect("decode");
        assert_eq!(decoded.fingerprint, run.fingerprint);
    }

    #[test]
    fn acceptance_harness_writes_files_when_explicitly_requested() {
        if std::env::var_os("P6_RUN_FULL_HARNESS_TEST").is_none() {
            return;
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("p6-harness-{nonce}"));
        let report = run_validation(&root, |_| {}).expect("full report");
        assert!(report.all_passed);
        assert!(root.join("p6-validation-report.json").is_file());
        assert!(root.join("p6-tier-transition.json").is_file());
        let _ = fs::remove_dir_all(root);
    }
}
