use super::*;

#[derive(Debug, Clone)]
pub(crate) struct IndexColumnPositions {
    i: usize,
    j: usize,
    k: usize,
}

impl IndexColumnPositions {
    pub(crate) fn from_headers(
        headers: &StringRecord,
        columns: &CsvIndexColumns,
    ) -> Result<Self, MineError> {
        Ok(Self {
            i: find_header_position(headers, columns.i_column())?,
            j: find_header_position(headers, columns.j_column())?,
            k: find_header_position(headers, columns.k_column())?,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaColumnPositions(Vec<(ColumnId, usize)>);

impl SchemaColumnPositions {
    pub(crate) fn from_headers(
        headers: &StringRecord,
        schema: &ColumnSchemaSet,
    ) -> Result<Self, MineError> {
        let mut positions = Vec::with_capacity(schema.len());

        for (column_id, _) in schema.iter() {
            positions.push((
                column_id.clone(),
                find_header_position(headers, column_id.as_str())?,
            ));
        }

        Ok(Self(positions))
    }

    pub(crate) fn position(&self, column_id: &ColumnId) -> Option<usize> {
        self.0
            .iter()
            .find_map(|(candidate, position)| (candidate == column_id).then_some(*position))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PendingColumnData {
    Integers(Vec<Option<i64>>),
    Floats(Vec<Option<f64>>),
    Booleans(Vec<Option<bool>>),
    Texts(Vec<Option<String>>),
}

impl PendingColumnData {
    pub(crate) fn new(logical_type: ColumnLogicalType, length: usize) -> Self {
        match logical_type {
            ColumnLogicalType::Integer => Self::Integers(vec![None; length]),
            ColumnLogicalType::Float => Self::Floats(vec![None; length]),
            ColumnLogicalType::Boolean => Self::Booleans(vec![None; length]),
            ColumnLogicalType::Text => Self::Texts(vec![None; length]),
        }
    }

    pub(crate) fn set(
        &mut self,
        linear_index: usize,
        raw_value: &str,
        row_number: usize,
        column_id: &ColumnId,
    ) -> Result<(), MineError> {
        match self {
            Self::Integers(values) => {
                values[linear_index] = Some(parse_integer(raw_value, row_number, column_id)?);
            }
            Self::Floats(values) => {
                values[linear_index] = Some(parse_float(raw_value, row_number, column_id)?);
            }
            Self::Booleans(values) => {
                values[linear_index] = Some(parse_boolean(raw_value, row_number, column_id)?);
            }
            Self::Texts(values) => values[linear_index] = Some(raw_value.to_owned()),
        }

        Ok(())
    }

    pub(crate) fn set_from_array(
        &mut self,
        linear_index: usize,
        array: &dyn Array,
        row_index: usize,
        column_id: &ColumnId,
    ) -> Result<(), MineError> {
        match self {
            Self::Integers(values) => {
                let array = array.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
                    MineError::schema(format!(
                        "column `{column_id}` is not stored as Int64 in Parquet"
                    ))
                })?;
                ensure_not_null(array, row_index, column_id.as_str(), "Parquet")?;
                values[linear_index] = Some(array.value(row_index));
            }
            Self::Floats(values) => {
                let array = array
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| {
                        MineError::schema(format!(
                            "column `{column_id}` is not stored as Float64 in Parquet"
                        ))
                    })?;
                ensure_not_null(array, row_index, column_id.as_str(), "Parquet")?;
                values[linear_index] = Some(array.value(row_index));
            }
            Self::Booleans(values) => {
                let array = array
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .ok_or_else(|| {
                        MineError::schema(format!(
                            "column `{column_id}` is not stored as Boolean in Parquet"
                        ))
                    })?;
                ensure_not_null(array, row_index, column_id.as_str(), "Parquet")?;
                values[linear_index] = Some(array.value(row_index));
            }
            Self::Texts(values) => {
                let array = array
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| {
                        MineError::schema(format!(
                            "column `{column_id}` is not stored as Utf8 in Parquet"
                        ))
                    })?;
                ensure_not_null(array, row_index, column_id.as_str(), "Parquet")?;
                values[linear_index] = Some(array.value(row_index).to_owned());
            }
        }

        Ok(())
    }

    pub(crate) fn finalize(self, column_id: ColumnId) -> Result<ColumnData, MineError> {
        match self {
            Self::Integers(values) => {
                Ok(ColumnData::Integers(finalize_values(values, &column_id)?))
            }
            Self::Floats(values) => Ok(ColumnData::Floats(finalize_values(values, &column_id)?)),
            Self::Booleans(values) => {
                Ok(ColumnData::Booleans(finalize_values(values, &column_id)?))
            }
            Self::Texts(values) => Ok(ColumnData::Texts(finalize_values(values, &column_id)?)),
        }
    }
}

pub(crate) fn validate_header_name(
    parameter: &'static str,
    value: String,
) -> Result<String, MineError> {
    if value.trim().is_empty() {
        return Err(MineError::invalid_parameter(
            parameter,
            "must not be empty or whitespace only",
        ));
    }

    if value.trim() != value {
        return Err(MineError::invalid_parameter(
            parameter,
            "must not contain leading or trailing whitespace",
        ));
    }

    Ok(value)
}

pub(crate) fn validate_selected_columns(
    selected_columns: Option<&[ColumnId]>,
) -> Result<(), MineError> {
    let Some(selected_columns) = selected_columns else {
        return Ok(());
    };

    let mut seen = BTreeSet::new();
    for column_id in selected_columns {
        if !seen.insert(column_id.as_str()) {
            return Err(MineError::invalid_parameter(
                "selected_columns",
                format!("column `{column_id}` is duplicated"),
            ));
        }
    }

    Ok(())
}

pub(crate) fn resolve_selected_columns(
    model: &BlockModel,
    selected_columns: Option<&[ColumnId]>,
) -> Result<Vec<ColumnId>, MineError> {
    match selected_columns {
        Some(selected_columns) => selected_columns
            .iter()
            .map(|column_id| {
                if model.schema().get(column_id).is_none() {
                    Err(MineError::schema(format!(
                        "column `{column_id}` does not exist in block model schema"
                    )))
                } else {
                    Ok(column_id.clone())
                }
            })
            .collect(),
        None => Ok(model
            .schema()
            .iter()
            .map(|(column_id, _)| column_id.clone())
            .collect()),
    }
}

pub(crate) fn stringify_value(
    column_data: &ColumnData,
    row_index: usize,
    column_id: &ColumnId,
) -> Result<String, MineError> {
    match column_data {
        ColumnData::Integers(values) => {
            values
                .get(row_index)
                .map(ToString::to_string)
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "row index `{row_index}` is out of bounds for integer column `{column_id}`"
                    ))
                })
        }
        ColumnData::Floats(values) => {
            values
                .get(row_index)
                .map(ToString::to_string)
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "row index `{row_index}` is out of bounds for float column `{column_id}`"
                    ))
                })
        }
        ColumnData::Booleans(values) => {
            values
                .get(row_index)
                .map(ToString::to_string)
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "row index `{row_index}` is out of bounds for boolean column `{column_id}`"
                    ))
                })
        }
        ColumnData::Texts(values) => values.get(row_index).cloned().ok_or_else(|| {
            MineError::validation(format!(
                "row index `{row_index}` is out of bounds for text column `{column_id}`"
            ))
        }),
    }
}

pub(crate) fn stringify_vulcan_value(
    column_data: &ColumnData,
    row_index: usize,
    column_id: &ColumnId,
    boolean_format: VulcanBooleanFormat,
) -> Result<String, MineError> {
    match column_data {
        ColumnData::Booleans(values) => values
            .get(row_index)
            .map(|value| match boolean_format {
                VulcanBooleanFormat::ZeroOne => {
                    if *value {
                        "1".to_owned()
                    } else {
                        "0".to_owned()
                    }
                }
                VulcanBooleanFormat::TrueFalse => value.to_string(),
            })
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{row_index}` is out of bounds for boolean column `{column_id}`"
                ))
            }),
        _ => stringify_value(column_data, row_index, column_id),
    }
}

pub(crate) fn ensure_dense_model_for_columnar_export(
    model: &BlockModel,
    operation: &'static str,
) -> Result<(), MineError> {
    if model.is_sparse() {
        Err(MineError::validation(format!(
            "{operation} does not support sparse block models yet"
        )))
    } else {
        Ok(())
    }
}

pub(crate) fn parse_grid_index(
    row: &StringRecord,
    row_number: usize,
    positions: &IndexColumnPositions,
) -> Result<GridIndex, MineError> {
    Ok(GridIndex::new(
        parse_usize(
            value_at(row, positions.i, row_number, "i")?,
            row_number,
            "i",
        )?,
        parse_usize(
            value_at(row, positions.j, row_number, "j")?,
            row_number,
            "j",
        )?,
        parse_usize(
            value_at(row, positions.k, row_number, "k")?,
            row_number,
            "k",
        )?,
    ))
}

pub(crate) fn find_header_position(
    headers: &StringRecord,
    expected: &str,
) -> Result<usize, MineError> {
    headers
        .iter()
        .position(|header| header == expected)
        .ok_or_else(|| MineError::schema(format!("column `{expected}` is missing from CSV header")))
}

pub(crate) fn value_at<'a>(
    row: &'a StringRecord,
    position: usize,
    row_number: usize,
    column_name: &str,
) -> Result<&'a str, MineError> {
    row.get(position).ok_or_else(|| {
        MineError::validation(format!(
            "CSV row {row_number} does not contain column `{column_name}`"
        ))
    })
}

pub(crate) fn parse_usize(
    value: &str,
    row_number: usize,
    column_name: &str,
) -> Result<usize, MineError> {
    value.parse::<usize>().map_err(|error| {
        MineError::schema(format!(
            "CSV row {row_number} has invalid `{column_name}` value `{value}`: {error}"
        ))
    })
}

pub(crate) fn parse_integer(
    value: &str,
    row_number: usize,
    column_id: &ColumnId,
) -> Result<i64, MineError> {
    value.parse::<i64>().map_err(|error| {
        MineError::schema(format!(
            "CSV row {row_number} has invalid integer for column `{column_id}`: {error}"
        ))
    })
}

pub(crate) fn parse_float(
    value: &str,
    row_number: usize,
    column_id: &ColumnId,
) -> Result<f64, MineError> {
    value.parse::<f64>().map_err(|error| {
        MineError::schema(format!(
            "CSV row {row_number} has invalid float for column `{column_id}`: {error}"
        ))
    })
}

pub(crate) fn parse_boolean(
    value: &str,
    row_number: usize,
    column_id: &ColumnId,
) -> Result<bool, MineError> {
    value.parse::<bool>().map_err(|error| {
        MineError::schema(format!(
            "CSV row {row_number} has invalid boolean for column `{column_id}`: {error}"
        ))
    })
}

pub(crate) fn finalize_values<T>(
    values: Vec<Option<T>>,
    column_id: &ColumnId,
) -> Result<Vec<T>, MineError> {
    values
        .into_iter()
        .enumerate()
        .map(|(linear_index, value)| {
            value.ok_or_else(|| {
                MineError::validation(format!(
                    "column `{column_id}` is missing a value for linear index `{linear_index}`"
                ))
            })
        })
        .collect()
}

pub(crate) fn required_int64_column<'a>(
    batch: &'a RecordBatch,
    column_name: &str,
) -> Result<&'a Int64Array, MineError> {
    let position = batch.schema().index_of(column_name).map_err(|_| {
        MineError::schema(format!(
            "column `{column_name}` is missing from Parquet data"
        ))
    })?;

    batch
        .column(position)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| {
            MineError::schema(format!(
                "column `{column_name}` is not stored as Int64 in Parquet"
            ))
        })
}

pub(crate) fn ensure_not_null(
    array: &dyn Array,
    row_index: usize,
    column_name: &str,
    format_name: &str,
) -> Result<(), MineError> {
    if array.is_null(row_index) {
        Err(MineError::validation(format!(
            "{format_name} row {row_index} contains null for column `{column_name}`"
        )))
    } else {
        Ok(())
    }
}

pub(crate) fn usize_to_i64(value: usize, axis: &'static str) -> Result<i64, MineError> {
    i64::try_from(value).map_err(|_| {
        MineError::grid(format!(
            "grid index `{axis}={value}` cannot be represented as Int64 for Parquet"
        ))
    })
}

pub(crate) fn i64_to_usize(value: i64, axis: &'static str) -> Result<usize, MineError> {
    usize::try_from(value).map_err(|_| {
        MineError::grid(format!(
            "Parquet index `{axis}={value}` cannot be represented as usize"
        ))
    })
}

pub(crate) fn json_error(error: serde_json::Error) -> MineError {
    MineError::schema(format!("unable to serialize Parquet metadata: {error}"))
}

pub(crate) fn io_error(message: impl Into<String>) -> MineError {
    MineError::Io {
        message: message.into(),
    }
}
