use std::time::Duration;

use clap::Parser;

use crate::error::AppError;

pub const MAX_COUNT: u32 = 1_000_000;
pub const MAX_DURATION_MS: u64 = 600_000;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "http-probe",
    version,
    about = "Probe an HTTP endpoint repeatedly and report latency statistics"
)]
pub struct Cli {
    #[arg(value_name = "URL", help = "HTTP or HTTPS endpoint to probe")]
    pub url: String,

    #[arg(
        short = 'c',
        long,
        default_value_t = 10,
        value_name = "COUNT",
        help = "Number of requests"
    )]
    pub count: u32,

    #[arg(
        long,
        default_value_t = 5000,
        value_name = "MILLISECONDS",
        help = "Total timeout per request"
    )]
    pub timeout_ms: u64,

    #[arg(
        long,
        default_value_t = 3000,
        value_name = "MILLISECONDS",
        help = "Connection timeout per request"
    )]
    pub connect_timeout_ms: u64,

    #[arg(
        long,
        default_value_t = 0,
        value_name = "MILLISECONDS",
        help = "Delay between requests"
    )]
    pub interval_ms: u64,

    #[arg(long, help = "Follow HTTP redirects, up to 10")]
    pub follow_redirects: bool,

    #[arg(short = 'v', long, help = "Print every attempt to stderr")]
    pub verbose: bool,
}

#[derive(Debug, Clone)]
pub struct ProbeConfig {
    pub url: String,
    pub count: u32,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub interval: Duration,
    pub follow_redirects: bool,
    pub verbose: bool,
    pub is_https: bool,
}

impl Cli {
    pub fn validate(self) -> Result<ProbeConfig, AppError> {
        if self.url.is_empty() {
            return Err(AppError::InvalidConfiguration(
                "URL must not be empty".to_string(),
            ));
        }

        let is_https = if self.url.starts_with("https://") {
            true
        } else if self.url.starts_with("http://") {
            false
        } else {
            return Err(AppError::InvalidConfiguration(
                "URL must start with http:// or https://".to_string(),
            ));
        };

        if self.count == 0 || self.count > MAX_COUNT {
            return Err(AppError::InvalidConfiguration(format!(
                "count must be in the range 1..={MAX_COUNT}"
            )));
        }

        validate_duration("timeout-ms", self.timeout_ms, 1, MAX_DURATION_MS)?;
        validate_duration(
            "connect-timeout-ms",
            self.connect_timeout_ms,
            1,
            MAX_DURATION_MS,
        )?;
        validate_duration("interval-ms", self.interval_ms, 0, MAX_DURATION_MS)?;

        if self.connect_timeout_ms > self.timeout_ms {
            return Err(AppError::InvalidConfiguration(
                "connect-timeout-ms must not exceed timeout-ms".to_string(),
            ));
        }

        Ok(ProbeConfig {
            url: self.url,
            count: self.count,
            timeout: Duration::from_millis(self.timeout_ms),
            connect_timeout: Duration::from_millis(self.connect_timeout_ms),
            interval: Duration::from_millis(self.interval_ms),
            follow_redirects: self.follow_redirects,
            verbose: self.verbose,
            is_https,
        })
    }
}

fn validate_duration(name: &str, value: u64, min: u64, max: u64) -> Result<(), AppError> {
    if value < min || value > max {
        return Err(AppError::InvalidConfiguration(format!(
            "{name} must be in the range {min}..={max}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(url: &str) -> Cli {
        Cli {
            url: url.to_string(),
            count: 10,
            timeout_ms: 5000,
            connect_timeout_ms: 3000,
            interval_ms: 0,
            follow_redirects: false,
            verbose: false,
        }
    }

    #[test]
    fn accepts_http() {
        assert!(!cli("http://example.com").validate().unwrap().is_https);
    }

    #[test]
    fn accepts_https() {
        assert!(cli("https://example.com").validate().unwrap().is_https);
    }

    #[test]
    fn rejects_ftp() {
        assert!(cli("ftp://example.com").validate().is_err());
    }

    #[test]
    fn rejects_missing_scheme() {
        assert!(cli("example.com").validate().is_err());
    }

    #[test]
    fn rejects_zero_count() {
        let mut args = cli("https://example.com");
        args.count = 0;
        assert!(args.validate().is_err());
    }

    #[test]
    fn rejects_excessive_count() {
        let mut args = cli("https://example.com");
        args.count = MAX_COUNT + 1;
        assert!(args.validate().is_err());
    }

    #[test]
    fn rejects_zero_timeout() {
        let mut args = cli("https://example.com");
        args.timeout_ms = 0;
        assert!(args.validate().is_err());
    }

    #[test]
    fn rejects_connect_timeout_greater_than_total_timeout() {
        let mut args = cli("https://example.com");
        args.timeout_ms = 1000;
        args.connect_timeout_ms = 1001;
        assert!(args.validate().is_err());
    }

    #[test]
    fn accepts_zero_interval() {
        let mut args = cli("https://example.com");
        args.interval_ms = 0;
        assert!(args.validate().is_ok());
    }

    #[test]
    fn accepts_redirect_flag() {
        let mut args = cli("https://example.com");
        args.follow_redirects = true;
        assert!(args.validate().unwrap().follow_redirects);
    }
}
