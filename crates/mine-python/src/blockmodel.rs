use std::collections::{BTreeMap, HashMap};

use mine_sdk::{
    BlockModel, BlockModelValidationExt, ColumnData, ColumnId, ColumnLogicalType, ColumnSchema,
    ColumnSchemaSet, Metadata, MetadataValue, RequiredColumn, ValidationOptions,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule};

use crate::{
    PyMineError,
    analytics::{PyBasicStatistics, PyGradeTonnagePoint, PyGroupedStatistics, PyModelSummary},
    core::{PyColumnSchema, PyGridDefinition, parse_logical_type},
    to_py_mine_error,
    validation::PyValidationReport,
};

/// BlockModel expuesto a Python.
#[pyclass(module = "miners._native", name = "BlockModel")]
#[derive(Debug, Clone)]
pub(crate) struct PyBlockModel {
    pub(crate) inner: BlockModel,
}

#[pymethods]
impl PyBlockModel {
    #[staticmethod]
    #[pyo3(signature = (dataframe, grid, schema, metadata=None))]
    fn from_pandas(
        dataframe: &Bound<'_, PyAny>,
        grid: &PyGridDefinition,
        schema: Vec<PyColumnSchema>,
        metadata: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        let schema_columns = schema
            .into_iter()
            .map(|column| column.inner)
            .collect::<Vec<_>>();
        let metadata = build_metadata(metadata).map_err(to_py_mine_error)?;
        let columns = build_columns_from_pandas(dataframe, &schema_columns)?;
        let schema_set = ColumnSchemaSet::from_columns(schema_columns).map_err(to_py_mine_error)?;
        let model = BlockModel::new(grid.inner.clone(), schema_set, metadata, columns)
            .map_err(to_py_mine_error)?;

        Ok(Self { inner: model })
    }

    #[staticmethod]
    #[pyo3(signature = (
        grid,
        schema,
        metadata=None,
        float_columns=None,
        integer_columns=None,
        boolean_columns=None
    ))]
    fn from_numpy(
        py: Python<'_>,
        grid: &PyGridDefinition,
        schema: Vec<PyColumnSchema>,
        metadata: Option<HashMap<String, String>>,
        float_columns: Option<HashMap<String, Py<PyAny>>>,
        integer_columns: Option<HashMap<String, Py<PyAny>>>,
        boolean_columns: Option<HashMap<String, Py<PyAny>>>,
    ) -> PyResult<Self> {
        let schema_columns = schema
            .into_iter()
            .map(|column| column.inner)
            .collect::<Vec<_>>();
        let metadata = build_metadata(metadata).map_err(to_py_mine_error)?;
        let columns =
            build_columns_from_numpy(py, float_columns, integer_columns, boolean_columns)?;
        let schema_set = ColumnSchemaSet::from_columns(schema_columns).map_err(to_py_mine_error)?;
        let model = BlockModel::new(grid.inner.clone(), schema_set, metadata, columns)
            .map_err(to_py_mine_error)?;

        Ok(Self { inner: model })
    }

    #[new]
    #[pyo3(signature = (
        grid,
        schema,
        metadata=None,
        float_columns=None,
        integer_columns=None,
        boolean_columns=None,
        text_columns=None,
        materialized_linear_indices=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        grid: &PyGridDefinition,
        schema: Vec<PyColumnSchema>,
        metadata: Option<HashMap<String, String>>,
        float_columns: Option<HashMap<String, Vec<f64>>>,
        integer_columns: Option<HashMap<String, Vec<i64>>>,
        boolean_columns: Option<HashMap<String, Vec<bool>>>,
        text_columns: Option<HashMap<String, Vec<String>>>,
        materialized_linear_indices: Option<Vec<usize>>,
    ) -> PyResult<Self> {
        let schema_set =
            ColumnSchemaSet::from_columns(schema.into_iter().map(|column| column.inner).collect())
                .map_err(to_py_mine_error)?;
        let metadata = build_metadata(metadata).map_err(to_py_mine_error)?;
        let columns = build_columns(
            float_columns,
            integer_columns,
            boolean_columns,
            text_columns,
        )
        .map_err(to_py_mine_error)?;

        let model = match materialized_linear_indices {
            Some(materialized_linear_indices) => BlockModel::new_sparse(
                grid.inner.clone(),
                schema_set,
                metadata,
                materialized_linear_indices,
                columns,
            ),
            None => BlockModel::new(grid.inner.clone(), schema_set, metadata, columns),
        }
        .map_err(to_py_mine_error)?;

        Ok(Self { inner: model })
    }

    fn block_count(&self) -> usize {
        self.inner.block_count()
    }

    fn summary(&self) -> PyResult<PyModelSummary> {
        Ok(PyModelSummary::new_inner(
            self.inner.summary().map_err(to_py_mine_error)?,
        ))
    }

    fn basic_statistics(&self, tonnage_column: &str) -> PyResult<PyBasicStatistics> {
        Ok(PyBasicStatistics::new_inner(
            self.inner
                .basic_statistics(&ColumnId::new(tonnage_column).map_err(to_py_mine_error)?)
                .map_err(to_py_mine_error)?,
        ))
    }

    fn grouped_statistics(
        &self,
        group_by: &str,
        tonnage_column: &str,
    ) -> PyResult<Vec<PyGroupedStatistics>> {
        Ok(self
            .inner
            .grouped_statistics(
                &ColumnId::new(group_by).map_err(to_py_mine_error)?,
                &ColumnId::new(tonnage_column).map_err(to_py_mine_error)?,
            )
            .map_err(to_py_mine_error)?
            .into_iter()
            .map(PyGroupedStatistics::new_inner)
            .collect())
    }

    fn grade_tonnage(
        &self,
        grade_column: &str,
        tonnage_column: &str,
        cutoffs: Vec<f64>,
    ) -> PyResult<Vec<PyGradeTonnagePoint>> {
        Ok(self
            .inner
            .grade_tonnage_curve(
                &ColumnId::new(grade_column).map_err(to_py_mine_error)?,
                &ColumnId::new(tonnage_column).map_err(to_py_mine_error)?,
                &cutoffs,
            )
            .map_err(to_py_mine_error)?
            .into_iter()
            .map(PyGradeTonnagePoint::new_inner)
            .collect())
    }

    #[pyo3(signature = (required_columns=None, tolerance=1e-9, validate_schema=true, validate_grid=true, validate_missing_blocks=true, validate_extents=true, validate_values=true, allow_sparse=false))]
    #[allow(clippy::too_many_arguments)]
    fn validate(
        &self,
        required_columns: Option<Vec<(String, String)>>,
        tolerance: f64,
        validate_schema: bool,
        validate_grid: bool,
        validate_missing_blocks: bool,
        validate_extents: bool,
        validate_values: bool,
        allow_sparse: bool,
    ) -> PyResult<PyValidationReport> {
        let required_columns = required_columns
            .unwrap_or_default()
            .into_iter()
            .map(|(name, logical_type)| {
                Ok(RequiredColumn::new(
                    ColumnId::new(name).map_err(to_py_mine_error)?,
                    parse_logical_type(&logical_type)?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let options = ValidationOptions::new()
            .with_required_columns(required_columns)
            .with_tolerance(tolerance)
            .map_err(to_py_mine_error)?
            .with_schema_validation(validate_schema)
            .with_regular_grid_validation(validate_grid)
            .with_missing_block_validation(validate_missing_blocks)
            .with_extent_validation(validate_extents)
            .with_value_validation(validate_values)
            .with_sparse_allowed(allow_sparse);

        Ok(PyValidationReport::new_inner(
            self.inner.validate_with_options(&options),
        ))
    }

    #[pyo3(signature = (columns=None))]
    fn to_pandas(&self, py: Python<'_>, columns: Option<Vec<String>>) -> PyResult<Py<PyAny>> {
        block_model_to_pandas(py, &self.inner, columns)
    }

    #[pyo3(signature = (columns=None))]
    fn to_numpy(&self, py: Python<'_>, columns: Option<Vec<String>>) -> PyResult<Py<PyAny>> {
        block_model_to_numpy(py, &self.inner, columns)
    }
}

fn build_metadata(
    metadata: Option<HashMap<String, String>>,
) -> Result<Metadata, mine_sdk::MineError> {
    Metadata::from_entries(
        metadata
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| (key, MetadataValue::Text(value))),
    )
}

fn build_columns(
    float_columns: Option<HashMap<String, Vec<f64>>>,
    integer_columns: Option<HashMap<String, Vec<i64>>>,
    boolean_columns: Option<HashMap<String, Vec<bool>>>,
    text_columns: Option<HashMap<String, Vec<String>>>,
) -> Result<BTreeMap<ColumnId, ColumnData>, mine_sdk::MineError> {
    let mut columns = BTreeMap::new();

    for (name, values) in float_columns.unwrap_or_default() {
        columns.insert(ColumnId::new(name)?, ColumnData::Floats(values));
    }

    for (name, values) in integer_columns.unwrap_or_default() {
        columns.insert(ColumnId::new(name)?, ColumnData::Integers(values));
    }

    for (name, values) in boolean_columns.unwrap_or_default() {
        columns.insert(ColumnId::new(name)?, ColumnData::Booleans(values));
    }

    for (name, values) in text_columns.unwrap_or_default() {
        columns.insert(ColumnId::new(name)?, ColumnData::Texts(values));
    }

    Ok(columns)
}

fn build_columns_from_pandas(
    dataframe: &Bound<'_, PyAny>,
    schema: &[ColumnSchema],
) -> PyResult<BTreeMap<ColumnId, ColumnData>> {
    let mut columns = BTreeMap::new();

    for column_schema in schema {
        let column_name = column_schema.name().as_str();
        let series = dataframe.get_item(column_name).map_err(|_| {
            PyErr::new::<PyMineError, _>(format!(
                "pandas dataframe is missing required column `{column_name}`"
            ))
        })?;
        let values = series.call_method0("tolist")?;
        let column_data = match column_schema.logical_type() {
            ColumnLogicalType::Integer => ColumnData::Integers(values.extract::<Vec<i64>>()?),
            ColumnLogicalType::Float => ColumnData::Floats(values.extract::<Vec<f64>>()?),
            ColumnLogicalType::Boolean => ColumnData::Booleans(values.extract::<Vec<bool>>()?),
            ColumnLogicalType::Text => ColumnData::Texts(values.extract::<Vec<String>>()?),
        };

        columns.insert(column_schema.name().clone(), column_data);
    }

    Ok(columns)
}

fn build_columns_from_numpy(
    py: Python<'_>,
    float_columns: Option<HashMap<String, Py<PyAny>>>,
    integer_columns: Option<HashMap<String, Py<PyAny>>>,
    boolean_columns: Option<HashMap<String, Py<PyAny>>>,
) -> PyResult<BTreeMap<ColumnId, ColumnData>> {
    let mut columns = BTreeMap::new();

    for (name, values) in float_columns.unwrap_or_default() {
        columns.insert(
            ColumnId::new(name).map_err(to_py_mine_error)?,
            ColumnData::Floats(extract_numpy_values::<f64>(values.bind(py), "float")?),
        );
    }

    for (name, values) in integer_columns.unwrap_or_default() {
        columns.insert(
            ColumnId::new(name).map_err(to_py_mine_error)?,
            ColumnData::Integers(extract_numpy_values::<i64>(values.bind(py), "integer")?),
        );
    }

    for (name, values) in boolean_columns.unwrap_or_default() {
        columns.insert(
            ColumnId::new(name).map_err(to_py_mine_error)?,
            ColumnData::Booleans(extract_numpy_values::<bool>(values.bind(py), "boolean")?),
        );
    }

    Ok(columns)
}

fn extract_numpy_values<T>(values: &Bound<'_, PyAny>, logical_type: &str) -> PyResult<Vec<T>>
where
    for<'py> T: FromPyObject<'py>,
{
    values
        .call_method0("tolist")
        .map_err(|_| {
            PyErr::new::<PyMineError, _>(format!(
                "{logical_type} numpy columns must provide an array-like object with tolist()"
            ))
        })?
        .extract::<Vec<T>>()
}

fn block_model_to_pandas(
    py: Python<'_>,
    model: &BlockModel,
    columns: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let pandas = import_pandas(py, "convert block models to pandas")?;
    let selected_columns = resolve_pandas_columns(model, columns)?;
    let data = PyDict::new(py);

    for column_id in selected_columns {
        let Some(column_data) = model.column(&column_id) else {
            return Err(PyErr::new::<PyMineError, _>(format!(
                "column `{column_id}` does not exist in block model storage"
            )));
        };

        match column_data {
            ColumnData::Integers(values) => data.set_item(column_id.as_str(), values.clone())?,
            ColumnData::Floats(values) => data.set_item(column_id.as_str(), values.clone())?,
            ColumnData::Booleans(values) => data.set_item(column_id.as_str(), values.clone())?,
            ColumnData::Texts(values) => data.set_item(column_id.as_str(), values.clone())?,
        }
    }

    Ok(pandas.call_method1("DataFrame", (data,))?.unbind())
}

fn block_model_to_numpy(
    py: Python<'_>,
    model: &BlockModel,
    columns: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let numpy = import_numpy(py, "convert block models to numpy")?;
    let selected_columns = resolve_numpy_columns(model, columns)?;
    let data = PyDict::new(py);

    for column_id in selected_columns {
        let Some(column_data) = model.column(&column_id) else {
            return Err(PyErr::new::<PyMineError, _>(format!(
                "column `{column_id}` does not exist in block model storage"
            )));
        };

        let array = match column_data {
            ColumnData::Integers(values) => numpy.call_method1("array", (values.clone(),))?,
            ColumnData::Floats(values) => numpy.call_method1("array", (values.clone(),))?,
            ColumnData::Booleans(values) => numpy.call_method1("array", (values.clone(),))?,
            ColumnData::Texts(_) => {
                return Err(PyErr::new::<PyMineError, _>(format!(
                    "column `{column_id}` has logical type `text`; use to_pandas() for text data"
                )));
            }
        };

        data.set_item(column_id.as_str(), array)?;
    }

    Ok(data.into_any().unbind())
}

fn resolve_pandas_columns(
    model: &BlockModel,
    columns: Option<Vec<String>>,
) -> PyResult<Vec<ColumnId>> {
    if let Some(columns) = columns {
        return columns
            .into_iter()
            .map(|column| ColumnId::new(column).map_err(to_py_mine_error))
            .collect();
    }

    Ok(model
        .schema()
        .iter()
        .map(|(column_id, _)| column_id.clone())
        .collect())
}

fn resolve_numpy_columns(
    model: &BlockModel,
    columns: Option<Vec<String>>,
) -> PyResult<Vec<ColumnId>> {
    let selected_columns = if let Some(columns) = columns {
        columns
            .into_iter()
            .map(|column| ColumnId::new(column).map_err(to_py_mine_error))
            .collect::<PyResult<Vec<_>>>()?
    } else {
        model
            .schema()
            .iter()
            .filter(|(_, column_schema)| {
                matches!(
                    column_schema.logical_type(),
                    ColumnLogicalType::Integer
                        | ColumnLogicalType::Float
                        | ColumnLogicalType::Boolean
                )
            })
            .map(|(column_id, _)| column_id.clone())
            .collect()
    };

    for column_id in &selected_columns {
        let Some(column_schema) = model.schema().get(column_id) else {
            return Err(PyErr::new::<PyMineError, _>(format!(
                "column `{column_id}` is not declared in the block model schema"
            )));
        };

        if column_schema.logical_type() == ColumnLogicalType::Text {
            return Err(PyErr::new::<PyMineError, _>(format!(
                "column `{column_id}` has logical type `text`; use to_pandas() for text data"
            )));
        }
    }

    Ok(selected_columns)
}

fn import_pandas<'py>(py: Python<'py>, purpose: &str) -> PyResult<Bound<'py, PyModule>> {
    py.import("pandas").map_err(|_| {
        PyErr::new::<PyMineError, _>(format!(
            "pandas is required to {purpose}; install the Python dependency first"
        ))
    })
}

fn import_numpy<'py>(py: Python<'py>, purpose: &str) -> PyResult<Bound<'py, PyModule>> {
    py.import("numpy").map_err(|_| {
        PyErr::new::<PyMineError, _>(format!(
            "numpy is required to {purpose}; install the Python dependency first"
        ))
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBlockModel>()?;
    Ok(())
}
