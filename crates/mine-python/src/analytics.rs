use mine_sdk::{
    BasicStatistics, ColumnNullCount, GradeTonnagePoint, GroupedStatistics, ModelSummary,
    WeightedGradeStatistic,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::core::{logical_type_name, mining_role_name};

/// Columna resumida para inspección en Python.
#[pyclass(module = "miners._native", name = "ColumnSummary")]
#[derive(Debug, Clone)]
pub(crate) struct PyColumnSummary {
    inner: mine_sdk::ColumnSummary,
}

impl PyColumnSummary {
    fn new_inner(inner: mine_sdk::ColumnSummary) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyColumnSummary {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.to_string()
    }

    #[getter]
    fn logical_type(&self) -> &'static str {
        logical_type_name(self.inner.logical_type)
    }

    #[getter]
    fn unit(&self) -> Option<String> {
        self.inner
            .unit
            .as_ref()
            .map(|unit| unit.as_str().to_owned())
    }

    #[getter]
    fn nullable(&self) -> bool {
        self.inner.nullable
    }

    #[getter]
    fn mining_role(&self) -> &'static str {
        mining_role_name(self.inner.mining_role)
    }

    #[getter]
    fn row_count(&self) -> usize {
        self.inner.row_count
    }

    #[getter]
    fn null_count(&self) -> usize {
        self.inner.null_count
    }

    #[getter]
    fn approximate_memory_bytes(&self) -> usize {
        self.inner.approximate_memory_bytes
    }
}

/// Summary de block model expuesto a Python.
#[pyclass(module = "miners._native", name = "ModelSummary")]
#[derive(Debug, Clone)]
pub(crate) struct PyModelSummary {
    inner: ModelSummary,
}

impl PyModelSummary {
    pub(crate) fn new_inner(inner: ModelSummary) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyModelSummary {
    #[getter]
    fn block_count(&self) -> usize {
        self.inner.block_count
    }

    #[getter]
    fn column_count(&self) -> usize {
        self.inner.column_count
    }

    #[getter]
    fn shape(&self) -> (usize, usize, usize) {
        (
            self.inner.grid_shape.nx(),
            self.inner.grid_shape.ny(),
            self.inner.grid_shape.nz(),
        )
    }

    #[getter]
    fn rotation_degrees(&self) -> Option<f64> {
        self.inner.rotation_degrees
    }

    #[getter]
    fn approximate_memory_bytes(&self) -> usize {
        self.inner.approximate_memory_bytes
    }

    #[getter]
    fn metadata_keys(&self) -> Vec<String> {
        self.inner.metadata_keys.clone()
    }

    fn columns(&self) -> Vec<PyColumnSummary> {
        self.inner
            .columns
            .iter()
            .cloned()
            .map(PyColumnSummary::new_inner)
            .collect()
    }

    fn extent(&self) -> ((f64, f64, f64), (f64, f64, f64)) {
        (
            (
                self.inner.extent.minimum.x(),
                self.inner.extent.minimum.y(),
                self.inner.extent.minimum.z(),
            ),
            (
                self.inner.extent.maximum.x(),
                self.inner.extent.maximum.y(),
                self.inner.extent.maximum.z(),
            ),
        )
    }
}

/// Conteo de nulos por columna expuesto a Python.
#[pyclass(module = "miners._native", name = "ColumnNullCount")]
#[derive(Debug, Clone)]
pub(crate) struct PyColumnNullCount {
    inner: ColumnNullCount,
}

impl PyColumnNullCount {
    fn new_inner(inner: ColumnNullCount) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyColumnNullCount {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.to_string()
    }

    #[getter]
    fn null_count(&self) -> usize {
        self.inner.null_count
    }
}

/// Estadística ponderada de ley expuesta a Python.
#[pyclass(module = "miners._native", name = "WeightedGradeStatistic")]
#[derive(Debug, Clone)]
pub(crate) struct PyWeightedGradeStatistic {
    inner: WeightedGradeStatistic,
}

impl PyWeightedGradeStatistic {
    fn new_inner(inner: WeightedGradeStatistic) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyWeightedGradeStatistic {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.to_string()
    }

    #[getter]
    fn unit(&self) -> Option<String> {
        self.inner
            .unit
            .as_ref()
            .map(|unit| unit.as_str().to_owned())
    }

    #[getter]
    fn average_grade(&self) -> Option<f64> {
        self.inner.average_grade
    }

    #[getter]
    fn contained_metal(&self) -> Option<f64> {
        self.inner.contained_metal
    }
}

/// Estadísticas básicas del modelo expuestas a Python.
#[pyclass(module = "miners._native", name = "BasicStatistics")]
#[derive(Debug, Clone)]
pub(crate) struct PyBasicStatistics {
    inner: BasicStatistics,
}

impl PyBasicStatistics {
    pub(crate) fn new_inner(inner: BasicStatistics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBasicStatistics {
    #[getter]
    fn block_count(&self) -> usize {
        self.inner.block_count
    }

    #[getter]
    fn tonnage_column(&self) -> String {
        self.inner.tonnage_column.to_string()
    }

    #[getter]
    fn total_tonnage(&self) -> f64 {
        self.inner.total_tonnage
    }

    fn null_counts(&self) -> Vec<PyColumnNullCount> {
        self.inner
            .null_counts
            .iter()
            .cloned()
            .map(PyColumnNullCount::new_inner)
            .collect()
    }

    fn grade_statistics(&self) -> Vec<PyWeightedGradeStatistic> {
        self.inner
            .grade_statistics
            .iter()
            .cloned()
            .map(PyWeightedGradeStatistic::new_inner)
            .collect()
    }
}

/// Estadísticas agrupadas expuestas a Python.
#[pyclass(module = "miners._native", name = "GroupedStatistics")]
#[derive(Debug, Clone)]
pub(crate) struct PyGroupedStatistics {
    inner: GroupedStatistics,
}

impl PyGroupedStatistics {
    pub(crate) fn new_inner(inner: GroupedStatistics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyGroupedStatistics {
    #[getter]
    fn group_by(&self) -> String {
        self.inner.group_by.to_string()
    }

    #[getter]
    fn group_value(&self) -> String {
        self.inner.group_value.clone()
    }

    #[getter]
    fn block_count(&self) -> usize {
        self.inner.block_count
    }

    #[getter]
    fn tonnage_column(&self) -> String {
        self.inner.tonnage_column.to_string()
    }

    #[getter]
    fn total_tonnage(&self) -> f64 {
        self.inner.total_tonnage
    }

    fn grade_statistics(&self) -> Vec<PyWeightedGradeStatistic> {
        self.inner
            .grade_statistics
            .iter()
            .cloned()
            .map(PyWeightedGradeStatistic::new_inner)
            .collect()
    }
}

/// Punto de curva ley-tonelaje expuesto a Python.
#[pyclass(module = "miners._native", name = "GradeTonnagePoint")]
#[derive(Debug, Clone)]
pub(crate) struct PyGradeTonnagePoint {
    inner: GradeTonnagePoint,
}

impl PyGradeTonnagePoint {
    pub(crate) fn new_inner(inner: GradeTonnagePoint) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyGradeTonnagePoint {
    #[getter]
    fn cutoff(&self) -> f64 {
        self.inner.cutoff
    }

    #[getter]
    fn block_count(&self) -> usize {
        self.inner.block_count
    }

    #[getter]
    fn tonnage(&self) -> f64 {
        self.inner.tonnage
    }

    #[getter]
    fn average_grade(&self) -> Option<f64> {
        self.inner.average_grade
    }

    #[getter]
    fn contained_metal(&self) -> Option<f64> {
        self.inner.contained_metal
    }

    #[getter]
    fn tonnage_percentage(&self) -> Option<f64> {
        self.inner.tonnage_percentage
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyColumnSummary>()?;
    module.add_class::<PyModelSummary>()?;
    module.add_class::<PyColumnNullCount>()?;
    module.add_class::<PyWeightedGradeStatistic>()?;
    module.add_class::<PyBasicStatistics>()?;
    module.add_class::<PyGroupedStatistics>()?;
    module.add_class::<PyGradeTonnagePoint>()?;
    Ok(())
}
