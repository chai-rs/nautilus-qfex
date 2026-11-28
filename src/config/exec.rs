//! Execution client configuration.

use std::{collections::HashMap, fmt, path::PathBuf};

use nautilus_model::identifiers::{AccountId, InstrumentId};
use rust_decimal::Decimal;

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
    /// Asks the venue to cancel every open order of the account when the trade socket drops.
    /// Sent after each login, so also after a reconnect.
    pub cancel_on_disconnect: bool,
    /// Leverage per instrument, set at connect. A refusal is logged and does not stop the
    /// connect: the venue refuses a change while the symbol has an open order or a position.
    pub leverages: HashMap<InstrumentId, Decimal>,
    /// Appends every REST answer and trade socket frame to this file, one JSON line each,
    /// without the key, the secret or any signature.
    pub record_path: Option<PathBuf>,
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
            leverages: HashMap::new(),
            record_path: None,
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
            .field("leverages", &self.leverages)
            .field("record_path", &self.record_path)
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

    #[test]
    fn debug_shows_the_record_path() {
        let config = QfexExecClientConfig {
            api_secret: Some("not-a-real-secret".to_string()),
            record_path: Some(PathBuf::from("recordings/uat-step-3.jsonl")),
            ..Default::default()
        };
        let debug = format!("{config:?}");
        assert!(debug.contains("recordings/uat-step-3.jsonl"));
        assert!(!debug.contains("not-a-real-secret"));
    }
}
