//! Data client configuration.

use std::any::Any;

use nautilus_common::factories::ClientConfig;
use serde::{Deserialize, Serialize};

use crate::common::{consts::HTTP_TIMEOUT_SECS, enums::QfexEnvironment};

/// Configuration of the QFEX data client. Market data is public: no credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_qfex", from_py_object)
)]
pub struct QfexDataClientConfig {
    pub environment: QfexEnvironment,
    /// Replaces the REST base URL of the environment.
    pub base_url_http: Option<String>,
    /// Replaces the WebSocket URL of the environment.
    pub base_url_ws: Option<String>,
    pub proxy_url: Option<String>,
    pub http_timeout_secs: u64,
    /// Replaces the REST `api_version`; `None` uses the version this adapter was written for.
    pub api_version: Option<String>,
}

impl Default for QfexDataClientConfig {
    fn default() -> Self {
        Self {
            environment: QfexEnvironment::default(),
            base_url_http: None,
            base_url_ws: None,
            proxy_url: None,
            http_timeout_secs: HTTP_TIMEOUT_SECS,
            api_version: None,
        }
    }
}

impl ClientConfig for QfexDataClientConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
