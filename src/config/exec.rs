//! Execution client configuration.

use std::fmt;

use nautilus_model::identifiers::AccountId;

use crate::common::{consts::HTTP_TIMEOUT_SECS, enums::QfexEnvironment};

/// Configuration of the QFEX execution client.
#[derive(Clone)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_qfex", from_py_object)
)]
pub struct QfexExecClientConfig {
    pub environment: QfexEnvironment,
    /// Replaces the REST base URL of the environment.
    pub base_url_http: Option<String>,
    /// Replaces the trade WebSocket URL of the environment.
    pub base_url_ws: Option<String>,
    pub proxy_url: Option<String>,
    pub http_timeout_secs: u64,
    /// Replaces the REST `api_version`; `None` uses the version this adapter was written for.
    pub api_version: Option<String>,
    pub account_id: AccountId,
    /// API public key; falls back to the environment variable.
    pub api_key: Option<String>,
    /// HMAC secret; falls back to the environment variable.
    pub api_secret: Option<String>,
    /// Asks the venue to cancel every open order when the trade socket drops.
    pub cancel_on_disconnect: bool,
}

impl Default for QfexExecClientConfig {
    fn default() -> Self {
        Self {
            environment: QfexEnvironment::default(),
            base_url_http: None,
            base_url_ws: None,
            proxy_url: None,
            http_timeout_secs: HTTP_TIMEOUT_SECS,
            api_version: None,
            account_id: AccountId::from("QFEX-001"),
            api_key: None,
            api_secret: None,
            cancel_on_disconnect: false,
        }
    }
}

// Manual impl so the secret can never reach logs.
impl fmt::Debug for QfexExecClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QfexExecClientConfig")
            .field("environment", &self.environment)
            .field("base_url_http", &self.base_url_http)
            .field("base_url_ws", &self.base_url_ws)
            .field("account_id", &self.account_id)
            .field("api_key", &self.api_key)
            .field(
                "api_secret",
                &self.api_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("cancel_on_disconnect", &self.cancel_on_disconnect)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_secret() {
        let config = QfexExecClientConfig {
            api_secret: Some("not-a-real-secret".to_string()),
            ..Default::default()
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("not-a-real-secret"));
        assert!(debug.contains("<redacted>"));
    }
}
