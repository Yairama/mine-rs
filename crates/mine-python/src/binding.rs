use mine_sdk::public_layers;
use mine_tools::initial_tool_catalog;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use serde::Serialize;

/// Describe la superficie mínima que conectará bindings Python con el SDK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PythonBindingSurface {
    /// Nombre estable de la capa `mine-python`.
    pub binding_layer: String,
    /// Nombres de las capas del SDK visibles desde Python.
    pub sdk_layers: Vec<String>,
    /// Nombre de la capa de tools disponible para futuras exposiciones.
    pub tool_layer: String,
    /// Tools deterministas disponibles en el catálogo actual.
    pub available_tools: Vec<String>,
}

/// Devuelve la superficie inicial de bindings para futuras integraciones Python.
#[must_use]
pub fn binding_surface() -> PythonBindingSurface {
    let catalog = initial_tool_catalog();

    PythonBindingSurface {
        binding_layer: "mine-python".to_owned(),
        sdk_layers: public_layers()
            .iter()
            .map(|layer| layer.name.to_owned())
            .collect(),
        tool_layer: catalog.tool_layer.name.to_owned(),
        available_tools: catalog
            .available_tools
            .iter()
            .map(|tool| tool.name.to_owned())
            .collect(),
    }
}

/// Representa la misma superficie mínima, pero expuesta como objeto Python.
#[pyclass(module = "miners._native")]
#[derive(Debug, Clone)]
pub(crate) struct PyBindingSurface {
    #[pyo3(get)]
    pub binding_layer: String,
    #[pyo3(get)]
    pub sdk_layers: Vec<String>,
    #[pyo3(get)]
    pub tool_layer: String,
    #[pyo3(get)]
    pub available_tools: Vec<String>,
}

impl From<PythonBindingSurface> for PyBindingSurface {
    fn from(surface: PythonBindingSurface) -> Self {
        Self {
            binding_layer: surface.binding_layer,
            sdk_layers: surface.sdk_layers,
            tool_layer: surface.tool_layer,
            available_tools: surface.available_tools,
        }
    }
}

#[pymethods]
impl PyBindingSurface {
    fn __repr__(&self) -> String {
        format!(
            "PythonBindingSurface(binding_layer={:?}, sdk_layers={:?}, tool_layer={:?}, available_tools={:?})",
            self.binding_layer, self.sdk_layers, self.tool_layer, self.available_tools
        )
    }
}

#[pyfunction(name = "binding_surface")]
fn py_binding_surface() -> PyBindingSurface {
    binding_surface().into()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBindingSurface>()?;
    module.add_function(wrap_pyfunction!(py_binding_surface, module)?)?;
    Ok(())
}
