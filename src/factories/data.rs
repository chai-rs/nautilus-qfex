//! Data client factory.

use std::{cell::RefCell, rc::Rc};

use nautilus_common::{
    cache::CacheView,
    clients::DataClient,
    clock::Clock,
    factories::{ClientConfig, DataClientFactory},
};
use nautilus_model::identifiers::ClientId;

use crate::{common::consts::QFEX, config::data::QfexDataClientConfig, data::QfexDataClient};

/// Creates [`QfexDataClient`]s for the live node.
#[derive(Debug, Clone, Default)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_qfex", from_py_object)
)]
pub struct QfexDataClientFactory;

impl QfexDataClientFactory {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl DataClientFactory for QfexDataClientFactory {
    fn create(
        &self,
        name: &str,
        config: &dyn ClientConfig,
        _cache: CacheView,
        _clock: Rc<RefCell<dyn Clock>>,
    ) -> anyhow::Result<Box<dyn DataClient>> {
        let config = config
            .as_any()
            .downcast_ref::<QfexDataClientConfig>()
            .ok_or_else(|| {
                anyhow::anyhow!("Invalid config type for QfexDataClientFactory: {config:?}")
            })?
            .clone();
        Ok(Box::new(QfexDataClient::new(ClientId::from(name), config)?))
    }

    fn name(&self) -> &'static str {
        QFEX
    }

    fn config_type(&self) -> &'static str {
        stringify!(QfexDataClientConfig)
    }
}
