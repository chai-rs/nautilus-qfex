//! Python bindings of the client factories.

use pyo3::prelude::*;

use crate::{
    common::consts::QFEX,
    factories::{data::QfexDataClientFactory, exec::QfexExecutionClientFactory},
};

#[pymethods]
impl QfexDataClientFactory {
    #[new]
    fn py_new() -> Self {
        Self
    }

    #[pyo3(name = "name")]
    fn py_name(&self) -> &'static str {
        QFEX
    }
}

#[pymethods]
impl QfexExecutionClientFactory {
    #[new]
    fn py_new() -> Self {
        Self
    }

    #[pyo3(name = "name")]
    fn py_name(&self) -> &'static str {
        QFEX
    }
}
