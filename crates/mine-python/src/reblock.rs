use mine_sdk::{AggregationRule, AggregationRules, ColumnId, DistributionRule, DistributionRules};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::{blockmodel::PyBlockModel, core::PyGridDefinition, to_py_mine_error};

fn column_id(name: &str) -> PyResult<ColumnId> {
    ColumnId::new(name).map_err(to_py_mine_error)
}

/// Regla declarativa de agregación construida desde Python y ejecutada en Rust.
#[pyclass(module = "miners._native", name = "AggregationRule")]
#[derive(Debug, Clone)]
pub(crate) struct PyAggregationRule {
    inner: AggregationRule,
}

#[pymethods]
impl PyAggregationRule {
    #[staticmethod]
    fn sum(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AggregationRule::sum(column_id(output_column)?, column_id(column)?),
        })
    }

    #[staticmethod]
    fn weighted_average(
        output_column: &str,
        value_column: &str,
        weight_column: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: AggregationRule::weighted_average(
                column_id(output_column)?,
                column_id(value_column)?,
                column_id(weight_column)?,
            ),
        })
    }

    #[staticmethod]
    fn minimum(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AggregationRule::minimum(column_id(output_column)?, column_id(column)?),
        })
    }

    #[staticmethod]
    fn maximum(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AggregationRule::maximum(column_id(output_column)?, column_id(column)?),
        })
    }

    #[staticmethod]
    fn first(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AggregationRule::first(column_id(output_column)?, column_id(column)?),
        })
    }

    #[staticmethod]
    fn majority(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AggregationRule::majority(column_id(output_column)?, column_id(column)?),
        })
    }
}

/// Regla declarativa de distribución construida desde Python y ejecutada en Rust.
#[pyclass(module = "miners._native", name = "DistributionRule")]
#[derive(Debug, Clone)]
pub(crate) struct PyDistributionRule {
    inner: DistributionRule,
}

#[pymethods]
impl PyDistributionRule {
    #[staticmethod]
    fn split_equally(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: DistributionRule::split_equally(column_id(output_column)?, column_id(column)?),
        })
    }

    #[staticmethod]
    fn replicate(output_column: &str, column: &str) -> PyResult<Self> {
        Ok(Self {
            inner: DistributionRule::replicate(column_id(output_column)?, column_id(column)?),
        })
    }
}

#[pyfunction(name = "superblock")]
fn py_superblock(
    model: &PyBlockModel,
    target_grid: &PyGridDefinition,
    rules: Vec<PyAggregationRule>,
) -> PyResult<PyBlockModel> {
    let rules = AggregationRules::new(rules.into_iter().map(|rule| rule.inner).collect())
        .map_err(to_py_mine_error)?;
    let model = mine_sdk::superblock(&model.inner, target_grid.inner.clone(), &rules)
        .map_err(to_py_mine_error)?;
    Ok(PyBlockModel::new_inner(model))
}

#[pyfunction(name = "subblock")]
fn py_subblock(
    model: &PyBlockModel,
    target_grid: &PyGridDefinition,
    rules: Vec<PyDistributionRule>,
) -> PyResult<PyBlockModel> {
    let rules = DistributionRules::new(rules.into_iter().map(|rule| rule.inner).collect())
        .map_err(to_py_mine_error)?;
    let model = mine_sdk::subblock(&model.inner, target_grid.inner.clone(), &rules)
        .map_err(to_py_mine_error)?;
    Ok(PyBlockModel::new_inner(model))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAggregationRule>()?;
    module.add_class::<PyDistributionRule>()?;
    module.add_function(wrap_pyfunction!(py_superblock, module)?)?;
    module.add_function(wrap_pyfunction!(py_subblock, module)?)?;
    Ok(())
}
