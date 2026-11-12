//! Python binding tests: an embedded interpreter, no network.

#![cfg(feature = "python")]

use nautilus_model::identifiers::{AccountId, InstrumentId};
use nautilus_qfex::{
    common::{consts::QFEX, enums::QfexEnvironment},
    config::{data::QfexDataClientConfig, exec::QfexExecClientConfig},
    factories::{data::QfexDataClientFactory, exec::QfexExecutionClientFactory},
    python,
};
use nautilus_system::get_global_pyo3_registry;
use pyo3::{
    Bound, Py, Python,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyModule},
};
use rust_decimal::Decimal;

fn module(py: Python<'_>) -> Bound<'_, PyModule> {
    let module = PyModule::new(py, "qfex").unwrap();
    python::qfex(&module).unwrap();
    module
}

// One test: the global registry accepts each extractor once per process.
#[test]
fn module_registers_classes_and_extractors() {
    Python::initialize();
    Python::attach(|py| {
        let module = module(py);
        configs_construct_from_python(&module);
        factories_and_configs_extract_through_the_registry(py);
    });
}

fn configs_construct_from_python(module: &Bound<'_, PyModule>) {
    let py = module.py();
    let data = module
        .getattr("QfexDataClientConfig")
        .unwrap()
        .call0()
        .unwrap()
        .extract::<QfexDataClientConfig>()
        .unwrap();
    assert_eq!(data.environment, QfexEnvironment::Mainnet);
    assert_eq!(data.http_timeout_secs, 10);

    let exec_class = module.getattr("QfexExecClientConfig").unwrap();
    let kwargs = PyDict::new(py);
    kwargs
        .set_item(
            "account_id",
            Py::new(py, AccountId::from("QFEX-007")).unwrap(),
        )
        .unwrap();
    kwargs
        .set_item(
            "environment",
            module
                .getattr("QfexEnvironment")
                .unwrap()
                .getattr("UAT")
                .unwrap(),
        )
        .unwrap();
    kwargs.set_item("api_key", "test-public-key").unwrap();
    kwargs.set_item("api_secret", "not-a-real-secret").unwrap();
    kwargs.set_item("cancel_on_disconnect", true).unwrap();
    let aapl = InstrumentId::from("AAPL-USD-PERP.QFEX");
    let leverages = PyDict::new(py);
    leverages
        .set_item(Py::new(py, aapl).unwrap(), Decimal::from(5))
        .unwrap();
    kwargs.set_item("leverages", leverages).unwrap();

    let object = exec_class.call((), Some(&kwargs)).unwrap();
    let repr = object.repr().unwrap().to_string();
    assert!(!repr.contains("not-a-real-secret"), "{repr}");
    let exec = object.extract::<QfexExecClientConfig>().unwrap();
    assert_eq!(exec.account_id, AccountId::from("QFEX-007"));
    assert_eq!(exec.environment, QfexEnvironment::Uat);
    assert_eq!(exec.api_secret.as_deref(), Some("not-a-real-secret"));
    assert!(exec.cancel_on_disconnect);
    assert_eq!(exec.leverages, [(aapl, Decimal::from(5))].into());
    assert_eq!(object.getattr("leverages").unwrap().len().unwrap(), 1);
    assert_eq!(exec.http_timeout_secs, 10);

    kwargs.set_item("environment", "MOON").unwrap();
    assert!(exec_class.call((), Some(&kwargs)).is_err());
}

fn factories_and_configs_extract_through_the_registry(py: Python<'_>) {
    let registry = get_global_pyo3_registry();

    let factory = Py::new(py, QfexDataClientFactory::new())
        .unwrap()
        .into_any();
    let factory = registry.extract_factory(py, factory).unwrap();
    assert_eq!(factory.name(), QFEX);
    assert_eq!(factory.config_type(), "QfexDataClientConfig");

    let factory = Py::new(py, QfexExecutionClientFactory).unwrap().into_any();
    let factory = registry.extract_exec_factory(py, factory).unwrap();
    assert_eq!(factory.name(), QFEX);
    assert_eq!(factory.config_type(), "QfexExecClientConfig");

    let config = QfexDataClientConfig {
        http_timeout_secs: 7,
        ..Default::default()
    };
    let config = Py::new(py, config).unwrap().into_any();
    let config = registry.extract_config(py, config).unwrap();
    let config = config
        .as_any()
        .downcast_ref::<QfexDataClientConfig>()
        .unwrap();
    assert_eq!(config.http_timeout_secs, 7);

    let config = QfexExecClientConfig {
        cancel_on_disconnect: true,
        ..Default::default()
    };
    let config = Py::new(py, config).unwrap().into_any();
    let config = registry.extract_config(py, config).unwrap();
    let config = config
        .as_any()
        .downcast_ref::<QfexExecClientConfig>()
        .unwrap();
    assert!(config.cancel_on_disconnect);
}
