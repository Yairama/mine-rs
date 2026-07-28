use std::collections::HashMap;

use mine_sdk::{
    ColumnId, ColumnSchemaSet, CsvIndexColumns, CsvReadOptions, CsvWriteOptions, MineError,
    read_block_model_csv, read_block_model_parquet, write_block_model_csv,
    write_block_model_parquet,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::{
    blockmodel::{PyBlockModel, build_metadata},
    core::{PyColumnSchema, PyGridDefinition},
    to_py_mine_error,
};

fn csv_index_columns(columns: (String, String, String)) -> PyResult<CsvIndexColumns> {
    CsvIndexColumns::new(columns.0, columns.1, columns.2).map_err(to_py_mine_error)
}

#[pyfunction(name = "read_csv", signature = (path, grid, schema, metadata=None, index_columns=("i".to_owned(), "j".to_owned(), "k".to_owned())))]
fn py_read_csv(
    path: &str,
    grid: &PyGridDefinition,
    schema: Vec<PyColumnSchema>,
    metadata: Option<HashMap<String, String>>,
    index_columns: (String, String, String),
) -> PyResult<PyBlockModel> {
    let schema =
        ColumnSchemaSet::from_columns(schema.into_iter().map(|column| column.inner).collect())
            .map_err(to_py_mine_error)?;
    let options = CsvReadOptions::new(
        grid.inner.clone(),
        schema,
        build_metadata(metadata).map_err(to_py_mine_error)?,
        csv_index_columns(index_columns)?,
    );
    let model = read_block_model_csv(path, &options).map_err(to_py_mine_error)?;
    Ok(PyBlockModel::new_inner(model))
}

#[pyfunction(name = "write_csv", signature = (model, path, index_columns=("i".to_owned(), "j".to_owned(), "k".to_owned()), columns=None))]
fn py_write_csv(
    model: &PyBlockModel,
    path: &str,
    index_columns: (String, String, String),
    columns: Option<Vec<String>>,
) -> PyResult<()> {
    if model.inner.is_sparse() {
        return Err(to_py_mine_error(MineError::invalid_parameter(
            "model",
            "CSV writing requires a dense BlockModel because CSV reading does not reconstruct sparse layouts",
        )));
    }

    let selected_columns = columns
        .map(|columns| {
            columns
                .into_iter()
                .map(|column| ColumnId::new(column).map_err(to_py_mine_error))
                .collect::<PyResult<Vec<_>>>()
        })
        .transpose()?;
    let options = CsvWriteOptions::new(csv_index_columns(index_columns)?, selected_columns)
        .map_err(to_py_mine_error)?;
    write_block_model_csv(&model.inner, path, &options).map_err(to_py_mine_error)
}

#[pyfunction(name = "read_parquet")]
fn py_read_parquet(path: &str) -> PyResult<PyBlockModel> {
    let model = read_block_model_parquet(path).map_err(to_py_mine_error)?;
    Ok(PyBlockModel::new_inner(model))
}

#[pyfunction(name = "write_parquet")]
fn py_write_parquet(model: &PyBlockModel, path: &str) -> PyResult<()> {
    write_block_model_parquet(&model.inner, path).map_err(to_py_mine_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(py_read_csv, module)?)?;
    module.add_function(wrap_pyfunction!(py_write_csv, module)?)?;
    module.add_function(wrap_pyfunction!(py_read_parquet, module)?)?;
    module.add_function(wrap_pyfunction!(py_write_parquet, module)?)?;
    Ok(())
}
