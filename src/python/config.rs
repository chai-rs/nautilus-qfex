//! Python bindings of the client configurations.

use nautilus_model::identifiers::AccountId;
use pyo3::pymethods;

use crate::{
    common::enums::QfexEnvironment,
    config::{data::QfexDataClientConfig, exec::QfexExecClientConfig},
};

#[pymethods]
impl QfexDataClientConfig {
    #[new]
    #[pyo3(signature = (
        environment = None,
        base_url_http = None,
        base_url_ws = None,
        proxy_url = None,
        http_timeout_secs = None,
        api_version = None,
    ))]
    fn py_new(
        environment: Option<QfexEnvironment>,
        base_url_http: Option<String>,
        base_url_ws: Option<String>,
        proxy_url: Option<String>,
        http_timeout_secs: Option<u64>,
        api_version: Option<String>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            environment: environment.unwrap_or(defaults.environment),
            base_url_http,
            base_url_ws,
            proxy_url,
            http_timeout_secs: http_timeout_secs.unwrap_or(defaults.http_timeout_secs),
            api_version,
        }
    }

    #[getter]
    fn environment(&self) -> QfexEnvironment {
        self.environment
    }

    #[getter]
    fn base_url_http(&self) -> Option<String> {
        self.base_url_http.clone()
    }

    #[getter]
    fn base_url_ws(&self) -> Option<String> {
        self.base_url_ws.clone()
    }

    #[getter]
    fn http_timeout_secs(&self) -> u64 {
        self.http_timeout_secs
    }

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl QfexExecClientConfig {
    /// Without `api_key` and `api_secret` the client reads them from the environment.
    #[new]
    #[pyo3(signature = (
        account_id = None,
        environment = None,
        base_url_http = None,
        base_url_ws = None,
        proxy_url = None,
        http_timeout_secs = None,
        api_version = None,
        api_key = None,
        api_secret = None,
        cancel_on_disconnect = None,
    ))]
    #[expect(clippy::too_many_arguments)]
    fn py_new(
        account_id: Option<AccountId>,
        environment: Option<QfexEnvironment>,
        base_url_http: Option<String>,
        base_url_ws: Option<String>,
        proxy_url: Option<String>,
        http_timeout_secs: Option<u64>,
        api_version: Option<String>,
        api_key: Option<String>,
        api_secret: Option<String>,
        cancel_on_disconnect: Option<bool>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            environment: environment.unwrap_or(defaults.environment),
            base_url_http,
            base_url_ws,
            proxy_url,
            http_timeout_secs: http_timeout_secs.unwrap_or(defaults.http_timeout_secs),
            api_version,
            account_id: account_id.unwrap_or(defaults.account_id),
            api_key,
            api_secret,
            cancel_on_disconnect: cancel_on_disconnect.unwrap_or(defaults.cancel_on_disconnect),
        }
    }

    #[getter]
    fn environment(&self) -> QfexEnvironment {
        self.environment
    }

    #[getter]
    fn account_id(&self) -> AccountId {
        self.account_id
    }

    #[getter]
    fn cancel_on_disconnect(&self) -> bool {
        self.cancel_on_disconnect
    }

    // No getter for `api_key` and `api_secret`; `__repr__` redacts them.
    fn __repr__(&self) -> String {
        format!("{self:?}")
    }
}
