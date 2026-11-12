//! Error types for the QFEX HTTP client.

use std::time::Duration;

use nautilus_network::http::HttpClientError;
use thiserror::Error;

/// Result type for QFEX HTTP operations.
pub type QfexHttpResult<T> = Result<T, QfexHttpError>;

/// Errors returned by the QFEX HTTP client.
#[derive(Debug, Clone, Error)]
pub enum QfexHttpError {
    /// Transport failure or timeout.
    #[error("network error: {0}")]
    Network(String),
    /// Non-success HTTP status without a venue error body.
    #[error("HTTP {status}: {body}")]
    Http {
        status: u16,
        body: String,
        retry_after: Option<Duration>,
    },
    /// HTTP 429.
    #[error("rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        retry_after: Option<Duration>,
    },
    /// Venue error body (`application/problem+json`): `code` is its `title`, `message` its
    /// `detail`.
    #[error("venue error {code}: {message}")]
    Venue { code: String, message: String },
    /// The request was rejected locally, before any call to the venue.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The response could not be decoded or converted.
    #[error("parse error: {0}")]
    Parse(String),
}

impl QfexHttpError {
    pub(super) const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Http { retry_after, .. } | Self::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

impl From<HttpClientError> for QfexHttpError {
    fn from(error: HttpClientError) -> Self {
        Self::Network(error.to_string())
    }
}

impl From<serde_json::Error> for QfexHttpError {
    fn from(error: serde_json::Error) -> Self {
        Self::Parse(error.to_string())
    }
}

impl From<anyhow::Error> for QfexHttpError {
    fn from(error: anyhow::Error) -> Self {
        Self::Parse(error.to_string())
    }
}

/// Returns whether a failed request may be sent again.
#[must_use]
pub fn should_retry_qfex_http_error(error: &QfexHttpError) -> bool {
    match error {
        QfexHttpError::Network(_) | QfexHttpError::RateLimit { .. } => true,
        QfexHttpError::Http { status, .. } => *status >= 500,
        _ => false,
    }
}

/// Builds the error the retry manager reports when it gives up on time.
#[must_use]
pub fn create_qfex_http_timeout_error(message: String) -> QfexHttpError {
    QfexHttpError::Network(message)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn http(status: u16) -> QfexHttpError {
        QfexHttpError::Http {
            status,
            body: String::new(),
            retry_after: Some(Duration::from_secs(2)),
        }
    }

    fn venue() -> QfexHttpError {
        QfexHttpError::Venue {
            code: "Bad Request".to_string(),
            message: String::new(),
        }
    }

    #[rstest]
    #[case(QfexHttpError::Network(String::new()), true)]
    #[case(QfexHttpError::RateLimit { message: String::new(), retry_after: None }, true)]
    #[case(http(503), true)]
    #[case(http(404), false)]
    #[case(venue(), false)]
    #[case(QfexHttpError::InvalidRequest(String::new()), false)]
    #[case(QfexHttpError::Parse(String::new()), false)]
    fn test_should_retry(#[case] error: QfexHttpError, #[case] expected: bool) {
        assert_eq!(should_retry_qfex_http_error(&error), expected);
    }

    #[rstest]
    fn test_retry_after_only_on_http_and_rate_limit() {
        assert_eq!(http(503).retry_after(), Some(Duration::from_secs(2)));
        assert_eq!(venue().retry_after(), None);
    }
}
