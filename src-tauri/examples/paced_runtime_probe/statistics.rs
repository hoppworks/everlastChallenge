//! Descriptive statistics only; qualification and warmup selection belong to the caller.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub count: usize,
    pub median_nanos: f64,
    pub p95_nanos: u128,
    pub min_nanos: u128,
    pub max_nanos: u128,
    pub range_nanos: u128,
}

/// Includes every measured run or refuses the entire summary. The caller
/// excludes warmup before calling and decides whether run count qualifies.
/// Median converts each middle integer before averaging, avoiding u128 sum
/// overflow. Runtime durations <=120 seconds are below 2^53 nanoseconds and
/// retain exact integer (and half-nanosecond median) precision in f64.
pub fn summarize(samples: &[Option<u128>]) -> Option<Summary> {
    let mut values: Vec<_> = samples.iter().copied().collect::<Option<_>>()?;
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let count = values.len();
    let middle = count / 2;
    let median_nanos = if count.is_multiple_of(2) {
        values[middle - 1] as f64 / 2.0 + values[middle] as f64 / 2.0
    } else {
        values[middle] as f64
    };
    // ceil(95*n/100) == n-floor(n/20), without multiplying usize.
    let p95_nanos = values[count - count / 20 - 1];
    let min_nanos = values[0];
    let max_nanos = values[count - 1];
    Some(Summary {
        count,
        median_nanos,
        p95_nanos,
        min_nanos,
        max_nanos,
        range_nanos: max_nanos - min_nanos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_median_keeps_half_nanoseconds_without_integer_addition_overflow()
    -> Result<(), &'static str> {
        let summary = summarize(&[Some(4), Some(1), Some(3), Some(2)]).ok_or("missing summary")?;
        assert_eq!(summary.count, 4);
        assert_eq!(summary.median_nanos, 2.5);
        assert_eq!(summary.p95_nanos, 4);
        assert_eq!(summary.min_nanos, 1);
        assert_eq!(summary.max_nanos, 4);
        assert_eq!(summary.range_nanos, 3);
        let large = summarize(&[Some(u128::MAX), Some(u128::MAX)]).ok_or("large summary")?;
        assert!(large.median_nanos.is_finite());
        Ok(())
    }
    #[test]
    fn nearest_rank_p95_for_thirty_runs_is_the_twenty_ninth_order_statistic()
    -> Result<(), &'static str> {
        let runs: Vec<_> = (1..=30).rev().map(Some).collect();
        let summary = summarize(&runs).ok_or("summary")?;
        assert_eq!(summary.count, 30);
        assert_eq!(summary.median_nanos, 15.5);
        assert_eq!(summary.p95_nanos, 29);
        assert_eq!(summary.range_nanos, 29);
        Ok(())
    }

    #[test]
    fn odd_and_single_runs_are_descriptive_without_qualification() -> Result<(), &'static str> {
        let odd = summarize(&[Some(100), Some(10), Some(30)]).ok_or("odd summary")?;
        assert_eq!(odd.median_nanos, 30.0);
        assert_eq!(odd.p95_nanos, 100);
        assert_eq!(odd.range_nanos, 90);
        let single = summarize(&[Some(0)]).ok_or("single summary")?;
        assert_eq!(single.count, 1);
        assert_eq!(single.median_nanos, 0.0);
        assert_eq!(single.p95_nanos, 0);
        assert_eq!(single.min_nanos, 0);
        assert_eq!(single.max_nanos, 0);
        assert_eq!(single.range_nanos, 0);
        Ok(())
    }

    #[test]
    fn empty_or_any_failed_run_refuses_the_entire_summary() {
        for runs in [
            vec![],
            vec![None],
            vec![Some(1), None, Some(3)],
            vec![None, Some(1)],
            vec![Some(1), None],
        ] {
            assert!(summarize(&runs).is_none());
        }
    }
}
