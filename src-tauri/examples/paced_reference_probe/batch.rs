//! Fixed orchestration and all-or-nothing aggregate at the existing run seam.

use crate::{manifest::Case, runner, statistics};
use serde::Serialize;
pub const CASES: usize = 6;
pub const RUNS_PER_CASE: u32 = 30;
pub const TOTAL_RUNS: usize = 1 + CASES * RUNS_PER_CASE as usize;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub id: String,
    pub fixture_index: usize,
    pub fixture_id: String,
    pub fixture_sha256: String,
    pub language: String,
    pub duration_seconds: u32,
    pub warmup: bool,
    pub run_index: u32,
}
pub struct Output {
    pub slot: Slot,
    pub output: runner::Output,
}
fn slot(case: &Case, fixture_index: usize, run_index: u32) -> Slot {
    let warmup = run_index == 0;
    Slot {
        id: if warmup {
            format!("warmup-{}", case.id)
        } else {
            format!("run-{}-{run_index:02}", case.id)
        },
        fixture_index,
        fixture_id: case.id.clone(),
        fixture_sha256: case.audio.sha256.clone(),
        language: case.language.clone(),
        duration_seconds: case.seconds,
        warmup,
        run_index,
    }
}
/// Input is the canonical six-case result of manifest::parse. Run zero is one
/// warmup on DE5; each canonical case then has all thirty measured slots.
fn plan(cases: &[Case]) -> Vec<Slot> {
    let mut slots = Vec::with_capacity(TOTAL_RUNS);
    if let Some(first) = cases.first() {
        slots.push(slot(first, 0, 0));
    }
    for (index, case) in cases.iter().enumerate() {
        slots.extend((1..=RUNS_PER_CASE).map(|run| slot(case, index, run)));
    }
    slots
}
/// Execute is the existing runner::one in the CLI, and a controlled complete
/// run outcome in orchestration tests. No retry, new warmup, or subset filter.
pub fn run(
    cases: &[Case],
    initial_failure: Option<&str>,
    mut execute: impl FnMut(&Slot) -> runner::Output,
) -> Vec<Output> {
    let mut blocked = initial_failure.map(str::to_owned);
    plan(cases)
        .into_iter()
        .map(|slot| {
            let mut output = if let Some(reason) = &blocked {
                runner::unavailable(
                    slot.id.clone(),
                    slot.warmup,
                    format!("not executed: {reason}"),
                )
            } else {
                execute(&slot)
            };
            if output.observation.status == "ok"
                && (output.observation.id != slot.id
                    || output.observation.warmup != slot.warmup
                    || output.observation.stop_to_text_nanos.is_none()
                    || output.text.as_ref().is_none_or(|s| s.is_empty()))
            {
                output.observation.status = "failed";
                output.observation.error =
                    Some("run output violates its identity/final-text/timing contract".into());
            }
            if blocked.is_none() && output.observation.status != "ok" {
                blocked =
                    Some(output.observation.error.clone().unwrap_or_else(|| {
                        format!("run {} failed without an error detail", slot.id)
                    }));
            }
            Output { slot, output }
        })
        .collect()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseSummary {
    pub fixture_id: String,
    pub fixture_sha256: String,
    pub language: String,
    pub duration_seconds: u32,
    pub statistics: statistics::Summary,
    pub median_limit_passed: bool,
    pub p95_limit_passed: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aggregate {
    pub l_nanos: f64,
    pub absolute_limits_passed: bool,
    pub cases: Vec<CaseSummary>,
}
/// Called only after actual cleanup. A failed warmup, missing/foreign/failed
/// slot, or unconfirmed join refuses every summary, including otherwise complete
/// early cases. There is no statistic over a successful subset.
pub fn aggregate(cases: &[Case], rows: &[Output], joined: bool) -> Option<Aggregate> {
    if !joined || cases.len() != CASES || rows.len() != TOTAL_RUNS {
        return None;
    }
    let expected = plan(cases);
    if rows.iter().zip(&expected).any(|(row, slot)| {
        row.slot != *slot
            || row.output.observation.id != slot.id
            || row.output.observation.warmup != slot.warmup
            || row.output.observation.status != "ok"
            || row.output.observation.stop_to_text_nanos.is_none()
            || row.output.text.as_ref().is_none_or(|s| s.is_empty())
    }) {
        return None;
    }
    let mut summaries = Vec::with_capacity(CASES);
    for (index, case) in cases.iter().enumerate() {
        let values = rows
            .iter()
            .filter(|row| !row.slot.warmup && row.slot.fixture_index == index)
            .map(|row| row.output.observation.stop_to_text_nanos)
            .collect::<Vec<_>>();
        if values.len() != RUNS_PER_CASE as usize {
            return None;
        }
        let statistics = statistics::summarize(&values)?;
        let median_limit_passed = statistics.median_nanos <= 1_000_000_000.0;
        let p95_limit_passed = statistics.p95_nanos <= 1_500_000_000;
        summaries.push(CaseSummary {
            fixture_id: case.id.clone(),
            fixture_sha256: case.audio.sha256.clone(),
            language: case.language.clone(),
            duration_seconds: case.seconds,
            statistics,
            median_limit_passed,
            p95_limit_passed,
        });
    }
    Some(Aggregate {
        l_nanos: summaries
            .iter()
            .map(|case| case.statistics.median_nanos)
            .sum::<f64>()
            / CASES as f64,
        absolute_limits_passed: summaries
            .iter()
            .all(|case| case.median_limit_passed && case.p95_limit_passed),
        cases: summaries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    fn cases() -> Result<Vec<manifest::Case>, String> {
        manifest::parse(include_bytes!(
            "../../../tests/fixtures/corpus/central-reference-v1.json"
        ))
    }
    fn success(id: String, warmup: bool, latency: u128) -> crate::runner::Output {
        let mut result = crate::runner::unavailable(id, warmup, "unused".into());
        result.observation.status = "ok";
        result.observation.error = None;
        result.observation.stop_to_text_nanos = Some(latency);
        result.text = Some("test hypothesis".into());
        result
    }
    #[test]
    fn one_warmup_and_exactly_thirty_per_case_use_one_execution_sequence()
    -> Result<(), Box<dyn std::error::Error>> {
        let cases = cases()?;
        let mut calls = 0;
        let rows = run(&cases, None, |slot| {
            calls += 1;
            success(
                slot.id.clone(),
                slot.warmup,
                if slot.warmup {
                    999_000_000_000
                } else {
                    u128::from(slot.run_index) * 1_000_000
                },
            )
        });
        assert_eq!(calls, 181);
        assert_eq!(rows.len(), 181);
        assert!(rows[0].slot.warmup);
        assert_eq!(rows[0].slot.fixture_index, 0);
        assert_eq!(rows.iter().filter(|r| r.slot.warmup).count(), 1);
        for i in 0..6 {
            assert_eq!(
                rows.iter()
                    .filter(|r| !r.slot.warmup && r.slot.fixture_index == i)
                    .count(),
                30
            );
        }
        let stats = aggregate(&cases, &rows, true).ok_or("complete summary missing")?;
        assert_eq!(stats.cases.len(), 6);
        assert_eq!(stats.l_nanos, 15_500_000.0);
        assert!(stats.absolute_limits_passed);
        for case in stats.cases {
            assert_eq!(case.statistics.count, 30);
            assert_eq!(case.statistics.p95_nanos, 29_000_000);
        }
        assert!(
            aggregate(&cases, &rows, false).is_none(),
            "a successful subset cannot substitute for join"
        );
        Ok(())
    }
    #[test]
    fn failed_or_initially_unavailable_runs_retain_every_slot_without_retries_or_statistics()
    -> Result<(), Box<dyn std::error::Error>> {
        let cases = cases()?;
        let mut calls = 0;
        let rows = run(&cases, None, |slot| {
            calls += 1;
            if calls == 33 {
                let mut output = crate::runner::unavailable(
                    slot.id.clone(),
                    slot.warmup,
                    "specific run failure".into(),
                );
                output.observation.status = "failed";
                output
            } else {
                success(slot.id.clone(), slot.warmup, 1)
            }
        });
        assert_eq!(calls, 33);
        assert_eq!(rows.len(), 181);
        assert_eq!(rows[32].output.observation.status, "failed");
        assert!(
            rows.iter()
                .skip(33)
                .all(|r| r.output.observation.status == "unavailable")
        );
        assert!(
            rows[32]
                .output
                .observation
                .error
                .as_deref()
                .is_some_and(|e| e.contains("specific run failure"))
        );
        assert!(aggregate(&cases, &rows, true).is_none());
        let rows = run(&cases, Some("model initialization failed"), |_| {
            calls += 1;
            success("not-called".into(), false, 1)
        });
        assert_eq!(calls, 33);
        assert_eq!(rows.len(), 181);
        assert!(
            rows.iter()
                .all(|r| r.output.observation.status == "unavailable")
        );
        assert!(aggregate(&cases, &rows, true).is_none());
        Ok(())
    }
    #[test]
    fn absolute_limits_are_per_case_and_missing_foreign_or_duplicate_slots_refuse_all_statistics()
    -> Result<(), Box<dyn std::error::Error>> {
        let cases = cases()?;
        let mut rows = run(&cases, None, |slot| {
            let latency = match slot.fixture_index {
                0 => 1_000_000_000,
                1 => {
                    if slot.run_index >= 29 {
                        1_500_000_000
                    } else {
                        1_000_000_000
                    }
                }
                2 => 1_000_000_001,
                3 => {
                    if slot.run_index >= 29 {
                        1_500_000_001
                    } else {
                        1_000_000_000
                    }
                }
                _ => 100_000_000,
            };
            success(slot.id.clone(), slot.warmup, latency)
        });
        let summary = aggregate(&cases, &rows, true).ok_or("complete summary")?;
        assert!(!summary.absolute_limits_passed);
        assert!(summary.cases[0].median_limit_passed && summary.cases[0].p95_limit_passed);
        assert!(summary.cases[1].median_limit_passed && summary.cases[1].p95_limit_passed);
        assert!(!summary.cases[2].median_limit_passed && summary.cases[2].p95_limit_passed);
        assert!(summary.cases[3].median_limit_passed && !summary.cases[3].p95_limit_passed);
        assert!(aggregate(&cases, &rows[..180], true).is_none());
        rows[3].output.observation.stop_to_text_nanos = None;
        assert!(aggregate(&cases, &rows, true).is_none());
        rows[3].output.observation.stop_to_text_nanos = Some(1_000_000_000);
        rows[3].slot = rows[2].slot.clone();
        assert!(aggregate(&cases, &rows, true).is_none());
        Ok(())
    }
}
