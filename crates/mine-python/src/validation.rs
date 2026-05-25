use mine_sdk::{
    Coordinate3D, ValidationIssue, ValidationIssueCode, ValidationReport, ValidationSeverity,
    validate_duplicate_block_coordinates, validate_duplicate_block_indices,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule};

use crate::{PyMineError, core::PyGridDefinition, to_py_mine_error};

/// Issue de validación expuesto a Python.
#[pyclass(module = "miners._native", name = "ValidationIssue")]
#[derive(Debug, Clone)]
pub(crate) struct PyValidationIssue {
    inner: ValidationIssue,
}

impl PyValidationIssue {
    fn new_inner(inner: ValidationIssue) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyValidationIssue {
    #[getter]
    fn severity(&self) -> &'static str {
        severity_name(self.inner.severity)
    }

    #[getter]
    fn code(&self) -> &'static str {
        issue_code_name(self.inner.code)
    }

    #[getter]
    fn message(&self) -> String {
        self.inner.message.clone()
    }

    #[getter]
    fn location(&self) -> Option<String> {
        self.inner.location.clone()
    }

    #[getter]
    fn affected_count(&self) -> Option<usize> {
        self.inner.affected_count
    }

    #[getter]
    fn recommendation(&self) -> Option<String> {
        self.inner.recommendation.clone()
    }
}

/// Reporte de validación expuesto a Python.
#[pyclass(module = "miners._native", name = "ValidationReport")]
#[derive(Debug, Clone)]
pub(crate) struct PyValidationReport {
    inner: ValidationReport,
}

impl PyValidationReport {
    pub(crate) fn new_inner(inner: ValidationReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyValidationReport {
    fn has_errors(&self) -> bool {
        self.inner.has_errors()
    }

    fn error_count(&self) -> usize {
        self.inner.error_count()
    }

    fn warning_count(&self) -> usize {
        self.inner.warning_count()
    }

    fn issues(&self) -> Vec<PyValidationIssue> {
        self.inner
            .issues
            .iter()
            .cloned()
            .map(PyValidationIssue::new_inner)
            .collect()
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|error| PyErr::new::<PyMineError, _>(error.to_string()))
    }

    fn to_pandas(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        validation_report_to_pandas(py, &self.inner)
    }
}

#[pyfunction(name = "validate_duplicate_indices")]
fn py_validate_duplicate_indices(indices: Vec<(usize, usize, usize)>) -> PyValidationReport {
    let indices = indices
        .into_iter()
        .map(|(i, j, k)| mine_sdk::GridIndex::new(i, j, k))
        .collect::<Vec<_>>();

    PyValidationReport::new_inner(validate_duplicate_block_indices(&indices))
}

#[pyfunction(name = "validate_duplicate_coordinates", signature = (grid, coordinates, tolerance=1e-9))]
fn py_validate_duplicate_coordinates(
    grid: &PyGridDefinition,
    coordinates: Vec<(f64, f64, f64)>,
    tolerance: f64,
) -> PyResult<PyValidationReport> {
    let coordinates = coordinates
        .into_iter()
        .map(|(x, y, z)| Coordinate3D::new(x, y, z).map_err(to_py_mine_error))
        .collect::<PyResult<Vec<_>>>()?;

    Ok(PyValidationReport::new_inner(
        validate_duplicate_block_coordinates(&grid.inner, &coordinates, tolerance)
            .map_err(to_py_mine_error)?,
    ))
}

fn validation_report_to_pandas(py: Python<'_>, report: &ValidationReport) -> PyResult<Py<PyAny>> {
    let pandas = import_pandas(py, "tabular validation reports")?;
    let data = PyDict::new(py);

    data.set_item(
        "severity",
        report
            .issues
            .iter()
            .map(|issue| severity_name(issue.severity))
            .collect::<Vec<_>>(),
    )?;
    data.set_item(
        "code",
        report
            .issues
            .iter()
            .map(|issue| issue_code_name(issue.code))
            .collect::<Vec<_>>(),
    )?;
    data.set_item(
        "message",
        report
            .issues
            .iter()
            .map(|issue| issue.message.clone())
            .collect::<Vec<_>>(),
    )?;
    data.set_item(
        "location",
        report
            .issues
            .iter()
            .map(|issue| issue.location.clone())
            .collect::<Vec<_>>(),
    )?;
    data.set_item(
        "affected_count",
        report
            .issues
            .iter()
            .map(|issue| issue.affected_count)
            .collect::<Vec<_>>(),
    )?;
    data.set_item(
        "recommendation",
        report
            .issues
            .iter()
            .map(|issue| issue.recommendation.clone())
            .collect::<Vec<_>>(),
    )?;

    Ok(pandas.call_method1("DataFrame", (data,))?.unbind())
}

fn import_pandas<'py>(py: Python<'py>, purpose: &str) -> PyResult<Bound<'py, PyModule>> {
    py.import("pandas").map_err(|_| {
        PyErr::new::<PyMineError, _>(format!(
            "pandas is required to {purpose}; install the Python dependency first"
        ))
    })
}

fn severity_name(severity: ValidationSeverity) -> &'static str {
    match severity {
        ValidationSeverity::Error => "error",
        ValidationSeverity::Warning => "warning",
        ValidationSeverity::Info => "info",
    }
}

fn issue_code_name(code: ValidationIssueCode) -> &'static str {
    match code {
        ValidationIssueCode::MissingRequiredColumn => "missing_required_column",
        ValidationIssueCode::WrongLogicalType => "wrong_logical_type",
        ValidationIssueCode::MissingMeasurementUnit => "missing_measurement_unit",
        ValidationIssueCode::GridIndexRoundtripMismatch => "grid_index_roundtrip_mismatch",
        ValidationIssueCode::UnsupportedRotatedGrid => "unsupported_rotated_grid",
        ValidationIssueCode::MissingBlocksDetected => "missing_blocks_detected",
        ValidationIssueCode::DuplicateBlockDetected => "duplicate_block_detected",
        ValidationIssueCode::IncompleteExtent => "incomplete_extent",
        ValidationIssueCode::DisplacedExtent => "displaced_extent",
        ValidationIssueCode::OversizedExtent => "oversized_extent",
        ValidationIssueCode::NonFiniteGradeValue => "non_finite_grade_value",
        ValidationIssueCode::InvalidTonnageValue => "invalid_tonnage_value",
        ValidationIssueCode::InvalidDensityValue => "invalid_density_value",
        ValidationIssueCode::InvalidRecoveryValue => "invalid_recovery_value",
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyValidationIssue>()?;
    module.add_class::<PyValidationReport>()?;
    module.add_function(wrap_pyfunction!(py_validate_duplicate_indices, module)?)?;
    module.add_function(wrap_pyfunction!(py_validate_duplicate_coordinates, module)?)?;
    Ok(())
}
