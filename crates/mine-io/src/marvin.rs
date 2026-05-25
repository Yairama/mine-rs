use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, Metadata, MetadataValue, MineError,
};
use mine_indexing::{GridIndex, ijk_to_linear};

use crate::io_error;

const MARVIN_BLOCKS_EXPECTED_FIELDS: usize = 8;

#[derive(Debug, Clone)]
struct MarvinBlockRow {
    source_block_id: i64,
    i: usize,
    j: usize,
    k: usize,
    field_4: f64,
    field_5: f64,
    field_6: f64,
    field_7: f64,
    linear_index: usize,
}

/// Lee un archivo `marvin.blocks` como `BlockModel` sparse usando una grilla unitaria derivada de
/// las columnas enteras `i/j/k` detectadas en el artefacto.
pub fn read_marvin_blocks(path: impl AsRef<Path>) -> Result<BlockModel, MineError> {
    let rows = read_marvin_rows(path.as_ref())?;

    if rows.is_empty() {
        return Err(io_error(
            "marvin.blocks file must contain at least one data row",
        ));
    }

    let max_i = rows
        .iter()
        .map(|row| row.i)
        .max()
        .expect("rows should not be empty");
    let max_j = rows
        .iter()
        .map(|row| row.j)
        .max()
        .expect("rows should not be empty");
    let max_k = rows
        .iter()
        .map(|row| row.k)
        .max()
        .expect("rows should not be empty");
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0)?,
        BlockDimensions::new(1.0, 1.0, 1.0)?,
        GridShape::new(max_i + 1, max_j + 1, max_k + 1)?,
        None,
    )?;
    let mut rows = rows
        .into_iter()
        .map(|mut row| {
            row.linear_index = ijk_to_linear(&grid, GridIndex::new(row.i, row.j, row.k))?;
            Ok(row)
        })
        .collect::<Result<Vec<_>, MineError>>()?;
    rows.sort_by_key(|row| row.linear_index);

    for window in rows.windows(2) {
        if window[0].linear_index == window[1].linear_index {
            return Err(MineError::validation(format!(
                "marvin.blocks contains duplicate sparse index ({}, {}, {})",
                window[1].i, window[1].j, window[1].k
            )));
        }
    }

    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("source_block_id")?,
            ColumnLogicalType::Integer,
            None,
            false,
            ColumnMiningRole::Other,
        ),
        ColumnSchema::new(
            ColumnId::new("field_4")?,
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Other,
        ),
        ColumnSchema::new(
            ColumnId::new("field_5")?,
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Other,
        ),
        ColumnSchema::new(
            ColumnId::new("field_6")?,
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Other,
        ),
        ColumnSchema::new(
            ColumnId::new("field_7")?,
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Other,
        ),
    ])?;
    let metadata = Metadata::from_entries(vec![
        (
            "benchmark_family".to_owned(),
            MetadataValue::Text("marvin".to_owned()),
        ),
        (
            "source_format".to_owned(),
            MetadataValue::Text("marvin.blocks".to_owned()),
        ),
        (
            "grid_assumption".to_owned(),
            MetadataValue::Text("unit-grid-from-i-j-k".to_owned()),
        ),
        (
            "semantics_verified".to_owned(),
            MetadataValue::Text("false".to_owned()),
        ),
    ])?;
    let materialized_linear_indices = rows.iter().map(|row| row.linear_index).collect::<Vec<_>>();
    let columns = BTreeMap::from([
        (
            ColumnId::new("source_block_id")?,
            ColumnData::Integers(rows.iter().map(|row| row.source_block_id).collect()),
        ),
        (
            ColumnId::new("field_4")?,
            ColumnData::Floats(rows.iter().map(|row| row.field_4).collect()),
        ),
        (
            ColumnId::new("field_5")?,
            ColumnData::Floats(rows.iter().map(|row| row.field_5).collect()),
        ),
        (
            ColumnId::new("field_6")?,
            ColumnData::Floats(rows.iter().map(|row| row.field_6).collect()),
        ),
        (
            ColumnId::new("field_7")?,
            ColumnData::Floats(rows.iter().map(|row| row.field_7).collect()),
        ),
    ]);

    BlockModel::new_sparse(grid, schema, metadata, materialized_linear_indices, columns)
}

fn read_marvin_rows(path: &Path) -> Result<Vec<MarvinBlockRow>, MineError> {
    let file = File::open(path)
        .map_err(|error| io_error(format!("unable to open marvin.blocks file: {error}")))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();

    for (row_offset, line_result) in reader.lines().enumerate() {
        let row_number = row_offset + 1;
        let line = line_result.map_err(|error| {
            io_error(format!(
                "unable to read marvin.blocks row {row_number}: {error}"
            ))
        })?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() != MARVIN_BLOCKS_EXPECTED_FIELDS {
            return Err(io_error(format!(
                "marvin.blocks row {row_number} must contain exactly {MARVIN_BLOCKS_EXPECTED_FIELDS} whitespace-delimited values"
            )));
        }

        rows.push(MarvinBlockRow {
            source_block_id: parse_i64_field(fields[0], row_number, "field_0")?,
            i: parse_usize_field(fields[1], row_number, "field_1")?,
            j: parse_usize_field(fields[2], row_number, "field_2")?,
            k: parse_usize_field(fields[3], row_number, "field_3")?,
            field_4: parse_f64_field(fields[4], row_number, "field_4")?,
            field_5: parse_f64_field(fields[5], row_number, "field_5")?,
            field_6: parse_f64_field(fields[6], row_number, "field_6")?,
            field_7: parse_f64_field(fields[7], row_number, "field_7")?,
            linear_index: 0,
        });
    }

    Ok(rows)
}

fn parse_i64_field(value: &str, row_number: usize, field_name: &str) -> Result<i64, MineError> {
    value.parse::<i64>().map_err(|error| {
        io_error(format!(
            "marvin.blocks row {row_number} contains invalid integer in {field_name}: {error}"
        ))
    })
}

fn parse_usize_field(value: &str, row_number: usize, field_name: &str) -> Result<usize, MineError> {
    value.parse::<usize>().map_err(|error| {
        io_error(format!(
            "marvin.blocks row {row_number} contains invalid grid index in {field_name}: {error}"
        ))
    })
}

fn parse_f64_field(value: &str, row_number: usize, field_name: &str) -> Result<f64, MineError> {
    let parsed = value.parse::<f64>().map_err(|error| {
        io_error(format!(
            "marvin.blocks row {row_number} contains invalid float in {field_name}: {error}"
        ))
    })?;

    if !parsed.is_finite() {
        return Err(io_error(format!(
            "marvin.blocks row {row_number} contains non-finite float in {field_name}"
        )));
    }

    Ok(parsed)
}
