use std::collections::BTreeMap;

use crate::stats::MetricStats;

#[derive(Debug, Clone)]
pub struct TimingSample {
    pub dns_us: u64,
    pub tcp_us: u64,
    pub tls_us: Option<u64>,
    pub ttfb_us: u64,
    pub download_us: u64,
    pub total_us: u64,
}

#[derive(Debug, Clone)]
pub struct ResponseMetadata {
    pub status_code: u32,
    pub primary_ip: Option<String>,
    pub downloaded_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct SuccessfulAttempt {
    pub attempt: u32,
    pub timing: TimingSample,
    pub response: ResponseMetadata,
}

#[derive(Debug, Clone)]
pub struct HttpFailureAttempt {
    pub attempt: u32,
    pub timing: TimingSample,
    pub response: ResponseMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransportFailureKind {
    Dns,
    Connect,
    Timeout,
    Tls,
    Redirect,
    Receive,
    Other,
}

impl TransportFailureKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dns => "DNS",
            Self::Connect => "Connect",
            Self::Timeout => "Timeout",
            Self::Tls => "TLS",
            Self::Redirect => "Redirect",
            Self::Receive => "Receive",
            Self::Other => "Other",
        }
    }

    pub fn verbose_label(&self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::Connect => "connect",
            Self::Timeout => "timeout",
            Self::Tls => "tls",
            Self::Redirect => "redirect",
            Self::Receive => "receive",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransportFailureAttempt {
    pub attempt: u32,
    pub kind: TransportFailureKind,
    pub message: String,
    pub elapsed_us: u64,
}

#[derive(Debug, Clone)]
pub enum AttemptResult {
    Success(SuccessfulAttempt),
    HttpFailure(HttpFailureAttempt),
    TransportFailure(TransportFailureAttempt),
}

#[derive(Debug, Clone)]
pub struct LatencySummary {
    pub dns: MetricStats,
    pub tcp: MetricStats,
    pub tls: Option<MetricStats>,
    pub ttfb: MetricStats,
    pub download: MetricStats,
    pub total: MetricStats,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub target: String,
    pub requested_attempts: u32,
    pub successful_attempts: u32,
    pub failed_attempts: u32,
    pub failure_rate: f64,
    pub latency: Option<LatencySummary>,
    pub status_counts: BTreeMap<u32, u32>,
    pub transport_failure_counts: BTreeMap<TransportFailureKind, u32>,
    pub attempts: Vec<AttemptResult>,
    pub follow_redirects: bool,
    pub is_https: bool,
}
