//! Python bindings: configurations and factories.
//!
//! The pattern is the one of `nautilus-lighter`. No stub generation: the stubs
//! serve the official `nautilus_trader` wheel only.

pub mod config;
pub mod factories;

use nautilus_common::factories::{ClientConfig, DataClientFactory, ExecutionClientFactory};
use nautilus_core::python::{to_pyruntime_err, to_pyvalue_err};
use nautilus_system::get_global_pyo3_registry;
use pyo3::prelude::*;

use crate::{
    common::{consts::QFEX, enums::QfexEnvironment},
    config::{data::QfexDataClientConfig, exec::QfexExecClientConfig},
    factories::{data::QfexDataClientFactory, exec::QfexExecutionClientFactory},
};

#[expect(clippy::needless_pass_by_value)]
fn extract_data_factory(
    py: Python<'_>,
    factory: Py<PyAny>,
) -> PyResult<Box<dyn DataClientFactory>> {
    match factory.extract::<QfexDataClientFactory>(py) {
        Ok(f) => Ok(Box::new(f)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract QfexDataClientFactory: {e}"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn extract_exec_factory(
    py: Python<'_>,
    factory: Py<PyAny>,
) -> PyResult<Box<dyn ExecutionClientFactory>> {
    match factory.extract::<QfexExecutionClientFactory>(py) {
        Ok(f) => Ok(Box::new(f)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract QfexExecutionClientFactory: {e}"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn extract_data_config(py: Python<'_>, config: Py<PyAny>) -> PyResult<Box<dyn ClientConfig>> {
    match config.extract::<QfexDataClientConfig>(py) {
        Ok(c) => Ok(Box::new(c)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract QfexDataClientConfig: {e}"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn extract_exec_config(py: Python<'_>, config: Py<PyAny>) -> PyResult<Box<dyn ClientConfig>> {
    match config.extract::<QfexExecClientConfig>(py) {
        Ok(c) => Ok(Box::new(c)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract QfexExecClientConfig: {e}"
        ))),
    }
}

/// Adds the QFEX classes to `m` and registers the extractors of the live node.
///
/// # Errors
///
/// Returns an error if a class or an extractor cannot be registered.
#[pymodule]
pub fn qfex(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add(stringify!(QFEX), QFEX)?;
    m.add_class::<QfexEnvironment>()?;
    m.add_class::<QfexDataClientConfig>()?;
    m.add_class::<QfexDataClientFactory>()?;
    m.add_class::<QfexExecClientConfig>()?;
    m.add_class::<QfexExecutionClientFactory>()?;

    let registry = get_global_pyo3_registry();
    registry
        .register_factory_extractor(QFEX.to_string(), extract_data_factory)
        .map_err(to_pyruntime_err)?;
    registry
        .register_exec_factory_extractor(QFEX.to_string(), extract_exec_factory)
        .map_err(to_pyruntime_err)?;
    registry
        .register_config_extractor("QfexDataClientConfig".to_string(), extract_data_config)
        .map_err(to_pyruntime_err)?;
    registry
        .register_config_extractor("QfexExecClientConfig".to_string(), extract_exec_config)
        .map_err(to_pyruntime_err)?;
    Ok(())
}
