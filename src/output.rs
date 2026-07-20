use crate::model::{AttemptResult, RunSummary};
use crate::stats::MetricStats;

pub fn print_report(summary: &RunSummary) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    write_report(summary, &mut stdout)
}

pub fn write_report<W: std::io::Write>(
    summary: &RunSummary,
    writer: &mut W,
) -> std::io::Result<()> {
    writeln!(writer, "HTTP Latency Probe")?;
    writeln!(writer, "Target:           {}", summary.target)?;
    writeln!(
        writer,
        "Measurement:      fresh DNS and connection per attempt"
    )?;
    writeln!(writer, "Attempts:         {}", summary.requested_attempts)?;
    writeln!(writer, "Successful:       {}", summary.successful_attempts)?;
    writeln!(writer, "Failed:           {}", summary.failed_attempts)?;
    writeln!(writer, "Failure rate:     {:.2}%", summary.failure_rate)?;
    writeln!(
        writer,
        "HTTP redirects:   {}",
        if summary.follow_redirects {
            "enabled"
        } else {
            "disabled"
        }
    )?;
    writeln!(writer)?;

    if let Some(latency) = &summary.latency {
        writeln!(
            writer,
            "Latency statistics for successful requests (milliseconds)"
        )?;
        writeln!(writer)?;
        writeln!(
            writer,
            "{:<16} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
            "Phase", "Min", "Avg", "Median", "P95", "P99", "Max"
        )?;
        print_metric_row(writer, "DNS", &latency.dns)?;
        print_metric_row(writer, "TCP connection", &latency.tcp)?;
        if summary.is_https {
            if let Some(tls) = &latency.tls {
                print_metric_row(writer, "TLS handshake", tls)?;
            } else {
                print_empty_metric_row(writer, "TLS handshake")?;
            }
        } else {
            print_empty_metric_row(writer, "TLS handshake")?;
        }
        print_metric_row(writer, "TTFB", &latency.ttfb)?;
        print_metric_row(writer, "Body transfer", &latency.download)?;
        print_metric_row(writer, "Total", &latency.total)?;
    } else {
        writeln!(
            writer,
            "Latency statistics: unavailable because no request succeeded."
        )?;
    }

    if summary.status_counts.len() > 1 {
        writeln!(writer)?;
        writeln!(writer, "HTTP statuses")?;
        for (status, count) in &summary.status_counts {
            writeln!(writer, "{status:<16} {count:>9}")?;
        }
    }

    writeln!(writer)?;
    if summary.failed_attempts == 0 {
        writeln!(writer, "Failures: none")?;
    } else {
        writeln!(writer, "Failures")?;
        for (status, count) in &summary.status_counts {
            if !(200..=299).contains(status) {
                writeln!(writer, "HTTP {status:<11} {count:>9}")?;
            }
        }
        for (kind, count) in &summary.transport_failure_counts {
            writeln!(writer, "{:<16} {:>9}", kind.label(), count)?;
        }
    }

    Ok(())
}

pub fn print_verbose_attempt(
    result: &AttemptResult,
    requested_attempts: u32,
) -> std::io::Result<()> {
    let mut stderr = std::io::stderr();
    write_verbose_attempt(result, requested_attempts, &mut stderr)
}

pub fn write_verbose_attempt<W: std::io::Write>(
    result: &AttemptResult,
    requested_attempts: u32,
    writer: &mut W,
) -> std::io::Result<()> {
    match result {
        AttemptResult::Success(attempt) => writeln!(
            writer,
            "[{}/{}] success status={} total={} ip={} bytes={}",
            attempt.attempt,
            requested_attempts,
            attempt.response.status_code,
            format_ms_u64(attempt.timing.total_us),
            attempt.response.primary_ip.as_deref().unwrap_or("-"),
            attempt.response.downloaded_bytes
        ),
        AttemptResult::HttpFailure(attempt) => writeln!(
            writer,
            "[{}/{}] failure status={} total={} ip={} bytes={}",
            attempt.attempt,
            requested_attempts,
            attempt.response.status_code,
            format_ms_u64(attempt.timing.total_us),
            attempt.response.primary_ip.as_deref().unwrap_or("-"),
            attempt.response.downloaded_bytes
        ),
        AttemptResult::TransportFailure(attempt) => writeln!(
            writer,
            "[{}/{}] failure kind={} elapsed={} error={:?}",
            attempt.attempt,
            requested_attempts,
            attempt.kind.verbose_label(),
            format_ms_u64(attempt.elapsed_us),
            attempt.message
        ),
    }
}

pub fn format_ms_u64(microseconds: u64) -> String {
    format!("{:.3}ms", microseconds as f64 / 1000.0)
}

fn format_table_ms_u64(microseconds: u64) -> String {
    format!("{:.3}", microseconds as f64 / 1000.0)
}

fn format_table_ms_f64(microseconds: f64) -> String {
    format!("{:.3}", microseconds / 1000.0)
}

fn print_metric_row<W: std::io::Write>(
    writer: &mut W,
    label: &str,
    stats: &MetricStats,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "{:<16} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        label,
        format_table_ms_u64(stats.min_us),
        format_table_ms_f64(stats.average_us),
        format_table_ms_f64(stats.median_us),
        format_table_ms_u64(stats.p95_us),
        format_table_ms_u64(stats.p99_us),
        format_table_ms_u64(stats.max_us)
    )
}

fn print_empty_metric_row<W: std::io::Write>(writer: &mut W, label: &str) -> std::io::Result<()> {
    writeln!(
        writer,
        "{:<16} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        label, "-", "-", "-", "-", "-", "-"
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::RunSummary;

    #[test]
    fn prints_no_success_message() {
        let summary = RunSummary {
            target: "http://example.com".to_string(),
            requested_attempts: 1,
            successful_attempts: 0,
            failed_attempts: 1,
            failure_rate: 100.0,
            latency: None,
            status_counts: BTreeMap::from([(503, 1)]),
            transport_failure_counts: BTreeMap::new(),
            attempts: Vec::new(),
            follow_redirects: false,
            is_https: false,
        };
        let mut output = Vec::new();
        write_report(&summary, &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Latency statistics: unavailable"));
        assert!(output.contains("HTTP 503"));
    }

    #[test]
    fn formats_verbose_transport_failure() {
        let result = AttemptResult::TransportFailure(crate::model::TransportFailureAttempt {
            attempt: 1,
            kind: crate::model::TransportFailureKind::Timeout,
            message: "timed out".to_string(),
            elapsed_us: 1234,
        });
        let mut output = Vec::new();
        write_verbose_attempt(&result, 2, &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("[1/2] failure kind=timeout"));
        assert!(output.contains("elapsed=1.234ms"));
    }
}
