#![allow(missing_docs)]
//! Puente Rust para exponer el SDK a la futura capa Python.

mod analytics;
mod binding;
mod blockmodel;
mod core;
mod tools;
mod validation;

use mine_sdk::MineError;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyModule;

pub use binding::{PythonBindingSurface, binding_surface};

create_exception!(_native, PyMineError, PyException);

pub(crate) fn to_py_mine_error(error: MineError) -> PyErr {
    PyErr::new::<PyMineError, _>(error.to_string())
}

/// Inicializa el módulo nativo importable como `miners._native`.
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("MineError", module.py().get_type::<PyMineError>())?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    binding::register(module)?;
    core::register(module)?;
    analytics::register(module)?;
    tools::register(module)?;
    validation::register(module)?;
    blockmodel::register(module)?;
    Ok(())
}
