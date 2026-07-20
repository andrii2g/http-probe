use std::time::{Duration, Instant};

use curl::easy::{Easy2, Handler, WriteError};

use crate::cli::ProbeConfig;
use crate::error::AppError;
use crate::model::{
    AttemptResult, HttpFailureAttempt, ResponseMetadata, SuccessfulAttempt, TimingSample,
    TransportFailureAttempt, TransportFailureKind,
};

#[derive(Debug, Default)]
struct DiscardBody {
    bytes_received: u64,
}

impl Handler for DiscardBody {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.bytes_received = self.bytes_received.saturating_add(data.len() as u64);
        Ok(data.len())
    }
}

pub fn probe_once(config: &ProbeConfig, attempt: u32) -> Result<AttemptResult, AppError> {
    let mut easy = Easy2::new(DiscardBody::default());
    configure_handle(&mut easy, config)?;

    let started = Instant::now();
    match easy.perform() {
        Ok(()) => {
            let timing = read_timing(&easy, config.is_https)?;
            let response = read_response_metadata(&easy)?;
            if (200..=299).contains(&response.status_code) {
                Ok(AttemptResult::Success(SuccessfulAttempt {
                    attempt,
                    timing,
                    response,
                }))
            } else {
                Ok(AttemptResult::HttpFailure(HttpFailureAttempt {
                    attempt,
                    timing,
                    response,
                }))
            }
        }
        Err(error) => Ok(AttemptResult::TransportFailure(TransportFailureAttempt {
            attempt,
            kind: classify_curl_error(&error),
            message: error.to_string(),
            elapsed_us: duration_to_micros(started.elapsed()),
        })),
    }
}

fn configure_handle(easy: &mut Easy2<DiscardBody>, config: &ProbeConfig) -> Result<(), AppError> {
    easy.url(&config.url)
        .map_err(|source| setup_error("url", source))?;
    easy.get(true)
        .map_err(|source| setup_error("get", source))?;
    easy.useragent(concat!("http-probe/", env!("CARGO_PKG_VERSION")))
        .map_err(|source| setup_error("useragent", source))?;
    easy.timeout(config.timeout)
        .map_err(|source| setup_error("timeout", source))?;
    easy.connect_timeout(config.connect_timeout)
        .map_err(|source| setup_error("connect_timeout", source))?;
    easy.follow_location(config.follow_redirects)
        .map_err(|source| setup_error("follow_location", source))?;
    easy.max_redirections(10)
        .map_err(|source| setup_error("max_redirections", source))?;
    easy.fresh_connect(true)
        .map_err(|source| setup_error("fresh_connect", source))?;
    easy.forbid_reuse(true)
        .map_err(|source| setup_error("forbid_reuse", source))?;
    easy.dns_cache_timeout(Duration::ZERO)
        .map_err(|source| setup_error("dns_cache_timeout", source))?;
    easy.nosignal(true)
        .map_err(|source| setup_error("nosignal", source))?;

    Ok(())
}

fn setup_error(operation: &'static str, source: curl::Error) -> AppError {
    AppError::CurlSetup { operation, source }
}

pub fn build_timing_sample(
    name_lookup: Duration,
    connect: Duration,
    app_connect: Duration,
    pre_transfer: Duration,
    start_transfer: Duration,
    total: Duration,
    is_https: bool,
) -> TimingSample {
    TimingSample {
        dns_us: duration_to_micros(name_lookup),
        tcp_us: duration_to_micros(connect.saturating_sub(name_lookup)),
        tls_us: is_https.then_some(duration_to_micros(app_connect.saturating_sub(connect))),
        ttfb_us: duration_to_micros(start_transfer.saturating_sub(pre_transfer)),
        download_us: duration_to_micros(total.saturating_sub(start_transfer)),
        total_us: duration_to_micros(total),
    }
}

fn read_timing(easy: &Easy2<DiscardBody>, is_https: bool) -> Result<TimingSample, AppError> {
    let name_lookup = easy
        .namelookup_time()
        .map_err(|source| timing_error("namelookup_time", source))?;
    let connect = easy
        .connect_time()
        .map_err(|source| timing_error("connect_time", source))?;
    let app_connect = easy
        .appconnect_time()
        .map_err(|source| timing_error("appconnect_time", source))?;
    let pre_transfer = easy
        .pretransfer_time()
        .map_err(|source| timing_error("pretransfer_time", source))?;
    let start_transfer = easy
        .starttransfer_time()
        .map_err(|source| timing_error("starttransfer_time", source))?;
    let total = easy
        .total_time()
        .map_err(|source| timing_error("total_time", source))?;

    Ok(build_timing_sample(
        name_lookup,
        connect,
        app_connect,
        pre_transfer,
        start_transfer,
        total,
        is_https,
    ))
}

fn timing_error(field: &'static str, source: curl::Error) -> AppError {
    AppError::TimingRead { field, source }
}

fn read_response_metadata(easy: &Easy2<DiscardBody>) -> Result<ResponseMetadata, AppError> {
    let status_code = easy
        .response_code()
        .map_err(|source| AppError::TimingRead {
            field: "response_code",
            source,
        })?;
    let primary_ip = easy.primary_ip().ok().and_then(|ip| ip.map(ToOwned::to_owned));
    let downloaded_bytes = easy.get_ref().bytes_received;

    Ok(ResponseMetadata {
        status_code,
        primary_ip,
        downloaded_bytes,
    })
}

fn classify_curl_error(error: &curl::Error) -> TransportFailureKind {
    if error.is_couldnt_resolve_host() || error.is_couldnt_resolve_proxy() {
        TransportFailureKind::Dns
    } else if error.is_couldnt_connect() {
        TransportFailureKind::Connect
    } else if error.is_operation_timedout() {
        TransportFailureKind::Timeout
    } else if error.is_ssl_connect_error()
        || error.is_ssl_peer_certificate()
        || error.is_ssl_cacert_badfile()
        || error.is_peer_failed_verification()
    {
        TransportFailureKind::Tls
    } else if error.is_too_many_redirects() {
        TransportFailureKind::Redirect
    } else if error.is_recv_error() || error.is_got_nothing() {
        TransportFailureKind::Receive
    } else {
        TransportFailureKind::Other
    }
}

pub fn duration_to_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_normal_http_timings() {
        let timing = build_timing_sample(
            Duration::from_micros(10),
            Duration::from_micros(30),
            Duration::from_micros(30),
            Duration::from_micros(40),
            Duration::from_micros(90),
            Duration::from_micros(120),
            false,
        );
        assert_eq!(timing.dns_us, 10);
        assert_eq!(timing.tcp_us, 20);
        assert_eq!(timing.tls_us, None);
        assert_eq!(timing.ttfb_us, 50);
        assert_eq!(timing.download_us, 30);
        assert_eq!(timing.total_us, 120);
    }

    #[test]
    fn decomposes_normal_https_timings() {
        let timing = build_timing_sample(
            Duration::from_micros(10),
            Duration::from_micros(30),
            Duration::from_micros(60),
            Duration::from_micros(70),
            Duration::from_micros(90),
            Duration::from_micros(120),
            true,
        );
        assert_eq!(timing.tls_us, Some(30));
    }

    #[test]
    fn equal_cumulative_fields_do_not_panic() {
        let timing = build_timing_sample(
            Duration::from_micros(10),
            Duration::from_micros(10),
            Duration::from_micros(10),
            Duration::from_micros(10),
            Duration::from_micros(10),
            Duration::from_micros(10),
            true,
        );
        assert_eq!(timing.tcp_us, 0);
        assert_eq!(timing.tls_us, Some(0));
        assert_eq!(timing.ttfb_us, 0);
        assert_eq!(timing.download_us, 0);
    }

    #[test]
    fn out_of_order_cumulative_values_saturate() {
        let timing = build_timing_sample(
            Duration::from_micros(30),
            Duration::from_micros(10),
            Duration::from_micros(5),
            Duration::from_micros(90),
            Duration::from_micros(50),
            Duration::from_micros(40),
            true,
        );
        assert_eq!(timing.tcp_us, 0);
        assert_eq!(timing.tls_us, Some(0));
        assert_eq!(timing.ttfb_us, 0);
        assert_eq!(timing.download_us, 0);
    }

    #[test]
    fn converts_microseconds() {
        assert_eq!(duration_to_micros(Duration::from_micros(12_345)), 12_345);
    }
}
