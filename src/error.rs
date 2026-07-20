use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum AppError {
    InvalidConfiguration(String),
    CurlSetup {
        operation: &'static str,
        source: curl::Error,
    },
    TimingRead {
        field: &'static str,
        source: curl::Error,
    },
    Internal(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => write!(f, "invalid configuration: {message}"),
            Self::CurlSetup { operation, source } => {
                write!(f, "failed to configure curl option {operation}: {source}")
            }
            Self::TimingRead { field, source } => {
                write!(f, "failed to read curl timing field {field}: {source}")
            }
            Self::Internal(message) => write!(f, "internal error: {message}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurlSetup { source, .. } | Self::TimingRead { source, .. } => Some(source),
            Self::InvalidConfiguration(_) | Self::Internal(_) => None,
        }
    }
}
