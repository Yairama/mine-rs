//! IO determinista para block models.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use ::csv::{ReaderBuilder, StringRecord, WriterBuilder};
use ::parquet::arrow::ArrowWriter;
use ::parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use arrow::array::{Array, ArrayRef, BooleanArray, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use mine_blockmodel::{
    BlockModel, ColumnData, ExperimentalVariogram, ExperimentalVariogramLagRow,
    experimental_variogram_from_lag_rows,
};
use mine_core::{
    ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet, GridDefinition,
    Metadata, MineError,
};
use mine_indexing::{GridIndex, ijk_to_linear, ijk_to_xyz, linear_to_ijk};
use serde::{Deserialize, Serialize};

const PARQUET_GRID_METADATA_KEY: &str = "mine_rs:grid_definition";
const PARQUET_SCHEMA_METADATA_KEY: &str = "mine_rs:column_schema";
const PARQUET_MODEL_METADATA_KEY: &str = "mine_rs:model_metadata";

mod csv;
mod inference;
mod internal;
mod marvin;
mod parquet;
mod variogram;
mod vtu;
mod vulcan;

pub use csv::{
    CsvIndexColumns, CsvReadOptions, CsvWriteOptions, read_block_model_csv, write_block_model_csv,
};
pub use inference::{
    InferredModelSchema, SchemaInferenceHints, SchemaInferenceWarning, SchemaInferenceWarningCode,
    infer_csv_schema, infer_parquet_schema,
};
pub(crate) use internal::*;
pub use marvin::read_marvin_blocks;
pub use parquet::{
    block_model_from_record_batch, block_model_to_record_batch, read_block_model_parquet,
    write_block_model_parquet,
};
pub use variogram::{
    experimental_variogram_from_record_batch, experimental_variogram_to_record_batch,
    read_experimental_variogram_json, read_experimental_variogram_parquet,
    write_experimental_variogram_json, write_experimental_variogram_parquet,
};
pub use vtu::{VtuWriteOptions, write_block_model_vtu};
pub use vulcan::{
    VulcanBooleanFormat, VulcanCoordinateColumns, VulcanCsvWriteOptions,
    write_block_model_vulcan_csv,
};
