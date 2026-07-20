use crate::cli::ProbeConfig;
use crate::model::{AttemptResult, LatencySummary, RunSummary, TimingSample};

#[derive(Debug, Clone)]
pub struct MetricStats {
    pub samples: usize,
    pub min_us: u64,
    pub max_us: u64,
    pub average_us: f64,
    pub median_us: f64,
    pub p95_us: u64,
    pub p99_us: u64,
}

pub fn summarize(values: &[u64]) -> Option<MetricStats> {
    if values.is_empty() {
        return None;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let samples = sorted.len();
    let min_us = sorted[0];
    let max_us = sorted[samples - 1];
    let average_us = sorted.iter().map(|value| *value as f64).sum::<f64>() / samples as f64;
    let median_us = if samples % 2 == 1 {
        sorted[samples / 2] as f64
    } else {
        (sorted[samples / 2 - 1] as f64 + sorted[samples / 2] as f64) / 2.0
    };

    Some(MetricStats {
        samples,
        min_us,
        max_us,
        average_us,
        median_us,
        p95_us: nearest_rank(&sorted, 0.95),
        p99_us: nearest_rank(&sorted, 0.99),
    })
}

pub fn nearest_rank(values_sorted: &[u64], percentile: f64) -> u64 {
    debug_assert!(!values_sorted.is_empty());
    debug_assert!(percentile.is_finite());
    debug_assert!((0.0..=1.0).contains(&percentile));

    let rank = (percentile * values_sorted.len() as f64).ceil() as usize;
    let index = rank.max(1) - 1;
    values_sorted[index]
}

pub fn build_run_summary(config: &ProbeConfig, attempts: Vec<AttemptResult>) -> RunSummary {
    let requested_attempts = config.count;
    let successful_attempts = attempts
        .iter()
        .filter(|attempt| matches!(attempt, AttemptResult::Success(_)))
        .count() as u32;
    let failed_attempts = requested_attempts.saturating_sub(successful_attempts);
    let failure_rate = if requested_attempts == 0 {
        0.0
    } else {
        failed_attempts as f64 / requested_attempts as f64 * 100.0
    };

    let mut status_counts = std::collections::BTreeMap::new();
    let mut transport_failure_counts = std::collections::BTreeMap::new();
    let mut successful_timings = Vec::new();

    for attempt in &attempts {
        match attempt {
            AttemptResult::Success(success) => {
                *status_counts
                    .entry(success.response.status_code)
                    .or_insert(0) += 1;
                successful_timings.push(success.timing.clone());
            }
            AttemptResult::HttpFailure(failure) => {
                *status_counts
                    .entry(failure.response.status_code)
                    .or_insert(0) += 1;
            }
            AttemptResult::TransportFailure(failure) => {
                *transport_failure_counts
                    .entry(failure.kind.clone())
                    .or_insert(0) += 1;
            }
        }
    }

    let latency = summarize_latency(&successful_timings, config.is_https);

    RunSummary {
        target: config.url.clone(),
        requested_attempts,
        successful_attempts,
        failed_attempts,
        failure_rate,
        latency,
        status_counts,
        transport_failure_counts,
        attempts,
        follow_redirects: config.follow_redirects,
        is_https: config.is_https,
    }
}

fn summarize_latency(timings: &[TimingSample], is_https: bool) -> Option<LatencySummary> {
    if timings.is_empty() {
        return None;
    }

    let dns = collect(timings, |timing| timing.dns_us);
    let tcp = collect(timings, |timing| timing.tcp_us);
    let ttfb = collect(timings, |timing| timing.ttfb_us);
    let download = collect(timings, |timing| timing.download_us);
    let total = collect(timings, |timing| timing.total_us);
    let tls_values = timings
        .iter()
        .filter_map(|timing| timing.tls_us)
        .collect::<Vec<_>>();

    Some(LatencySummary {
        dns: summarize(&dns)?,
        tcp: summarize(&tcp)?,
        tls: if is_https {
            summarize(&tls_values)
        } else {
            None
        },
        ttfb: summarize(&ttfb)?,
        download: summarize(&download)?,
        total: summarize(&total)?,
    })
}

fn collect<F>(timings: &[TimingSample], mut f: F) -> Vec<u64>
where
    F: FnMut(&TimingSample) -> u64,
{
    timings.iter().map(&mut f).collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::model::{
        HttpFailureAttempt, ResponseMetadata, SuccessfulAttempt, TransportFailureAttempt,
        TransportFailureKind,
    };

    #[test]
    fn empty_input_returns_none() {
        assert!(summarize(&[]).is_none());
    }

    #[test]
    fn one_value_has_all_fields_equal() {
        let stats = summarize(&[42]).unwrap();
        assert_eq!(stats.min_us, 42);
        assert_eq!(stats.max_us, 42);
        assert_eq!(stats.average_us, 42.0);
        assert_eq!(stats.median_us, 42.0);
        assert_eq!(stats.p95_us, 42);
        assert_eq!(stats.p99_us, 42);
    }

    #[test]
    fn calculates_odd_median() {
        assert_eq!(summarize(&[3, 1, 2]).unwrap().median_us, 2.0);
    }

    #[test]
    fn calculates_even_median() {
        assert_eq!(summarize(&[4, 1, 2, 3]).unwrap().median_us, 2.5);
    }

    #[test]
    fn average_preserves_fractional_result() {
        assert_eq!(summarize(&[1, 2]).unwrap().average_us, 1.5);
    }

    #[test]
    fn nearest_rank_p95_for_100_values() {
        let values = (1..=100).collect::<Vec<_>>();
        assert_eq!(nearest_rank(&values, 0.95), 95);
    }

    #[test]
    fn nearest_rank_p99_for_100_values() {
        let values = (1..=100).collect::<Vec<_>>();
        assert_eq!(nearest_rank(&values, 0.99), 99);
    }

    #[test]
    fn p99_for_fewer_than_100_values_can_equal_max() {
        assert_eq!(summarize(&[1, 2, 3]).unwrap().p99_us, 3);
    }

    #[test]
    fn unsorted_input_is_handled() {
        assert_eq!(summarize(&[10, 1, 5]).unwrap().min_us, 1);
    }

    #[test]
    fn duplicate_values_are_handled() {
        assert_eq!(summarize(&[5, 5, 5]).unwrap().p95_us, 5);
    }

    #[test]
    fn zero_duration_values_are_handled() {
        assert_eq!(summarize(&[0, 0, 1]).unwrap().min_us, 0);
    }

    #[test]
    fn very_large_values_do_not_overflow_integer_sum() {
        let stats = summarize(&[u64::MAX, u64::MAX]).unwrap();
        assert!(stats.average_us > 0.0);
    }

    #[test]
    fn builds_all_successful_summary() {
        let summary = build_run_summary(&config(2, true), vec![success(1, 200), success(2, 200)]);
        assert_eq!(summary.successful_attempts, 2);
        assert_eq!(summary.failed_attempts, 0);
        assert_eq!(summary.failure_rate, 0.0);
        assert!(summary.latency.is_some());
    }

    #[test]
    fn builds_mixed_status_summary() {
        let summary = build_run_summary(
            &config(2, true),
            vec![success(1, 200), http_failure(2, 503)],
        );
        assert_eq!(summary.successful_attempts, 1);
        assert_eq!(summary.failed_attempts, 1);
        assert_eq!(summary.failure_rate, 50.0);
        assert_eq!(summary.status_counts.get(&200), Some(&1));
        assert_eq!(summary.status_counts.get(&503), Some(&1));
        assert_eq!(summary.latency.unwrap().total.samples, 1);
    }

    #[test]
    fn builds_transport_failures_only_summary() {
        let summary = build_run_summary(&config(1, true), vec![transport_failure(1)]);
        assert_eq!(summary.successful_attempts, 0);
        assert_eq!(summary.failed_attempts, 1);
        assert!(summary.latency.is_none());
        assert_eq!(
            summary
                .transport_failure_counts
                .get(&TransportFailureKind::Timeout),
            Some(&1)
        );
    }

    fn config(count: u32, is_https: bool) -> ProbeConfig {
        ProbeConfig {
            url: if is_https {
                "https://example.com".to_string()
            } else {
                "http://example.com".to_string()
            },
            count,
            timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(3),
            interval: Duration::ZERO,
            follow_redirects: false,
            verbose: false,
            is_https,
        }
    }

    fn timing() -> TimingSample {
        TimingSample {
            dns_us: 1,
            tcp_us: 2,
            tls_us: Some(3),
            ttfb_us: 4,
            download_us: 5,
            total_us: 15,
        }
    }

    fn response(status_code: u32) -> ResponseMetadata {
        ResponseMetadata {
            status_code,
            primary_ip: Some("127.0.0.1".to_string()),
            downloaded_bytes: 2,
        }
    }

    fn success(attempt: u32, status_code: u32) -> AttemptResult {
        AttemptResult::Success(SuccessfulAttempt {
            attempt,
            timing: timing(),
            response: response(status_code),
        })
    }

    fn http_failure(attempt: u32, status_code: u32) -> AttemptResult {
        AttemptResult::HttpFailure(HttpFailureAttempt {
            attempt,
            timing: timing(),
            response: response(status_code),
        })
    }

    fn transport_failure(attempt: u32) -> AttemptResult {
        AttemptResult::TransportFailure(TransportFailureAttempt {
            attempt,
            kind: TransportFailureKind::Timeout,
            message: "timeout".to_string(),
            elapsed_us: 100,
        })
    }
}
