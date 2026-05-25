use super::*;

/// Escribe un `BlockModel` regular a Parquet preservando grilla, schema y metadata.
pub fn write_block_model_parquet(
    model: &BlockModel,
    path: impl AsRef<Path>,
) -> Result<(), MineError> {
    let batch = block_model_to_record_batch(model)?;
    let file = File::create(path.as_ref())?;
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None)
        .map_err(|error| io_error(format!("unable to create Parquet writer: {error}")))?;

    writer
        .write(&batch)
        .map_err(|error| io_error(format!("unable to write Parquet batch: {error}")))?;
    writer
        .close()
        .map_err(|error| io_error(format!("unable to close Parquet writer: {error}")))?;

    Ok(())
}

/// Lee un `BlockModel` regular desde Parquet usando metadata embebida del archivo.
pub fn read_block_model_parquet(path: impl AsRef<Path>) -> Result<BlockModel, MineError> {
    let file = File::open(path.as_ref())?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|error| io_error(format!("unable to open Parquet file: {error}")))?;
    let schema = builder.schema().clone();
    let reader = builder
        .build()
        .map_err(|error| io_error(format!("unable to build Parquet reader: {error}")))?;
    let batches = reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(format!("unable to read Parquet batches: {error}")))?;

    block_model_from_record_batches(schema, &batches, "Parquet")
}

/// Convierte un `BlockModel` a `RecordBatch` Arrow.
///
/// Actualmente esta conversión materializa arrays Arrow nuevos para cada columna, por lo que
/// copia los datos desde el storage columnar actual del modelo.
pub fn block_model_to_record_batch(model: &BlockModel) -> Result<RecordBatch, MineError> {
    ensure_dense_model_for_columnar_export(model, "block_model_to_record_batch")?;
    let schema = Arc::new(build_parquet_schema(model)?);
    let index_arrays = build_parquet_index_arrays(model.grid())?;
    let mut arrays: Vec<ArrayRef> = vec![index_arrays.0, index_arrays.1, index_arrays.2];

    for (column_id, _) in model.schema().iter() {
        let column_data = model.column(column_id).ok_or_else(|| {
            MineError::schema(format!(
                "column `{column_id}` is present in schema but missing from block model storage"
            ))
        })?;
        arrays.push(column_data_to_array(column_data));
    }

    RecordBatch::try_new(schema, arrays)
        .map_err(|error| MineError::schema(format!("unable to build Parquet batch: {error}")))
}

/// Reconstruye un `BlockModel` desde un `RecordBatch` Arrow con metadata `mine-rs`.
///
/// Actualmente la importación copia valores Arrow a vectores Rust tipados para preservar el
/// storage columnar actual del SDK.
pub fn block_model_from_record_batch(batch: &RecordBatch) -> Result<BlockModel, MineError> {
    block_model_from_record_batches(batch.schema(), std::slice::from_ref(batch), "Arrow")
}

fn build_parquet_schema(model: &BlockModel) -> Result<Schema, MineError> {
    let mut fields = vec![
        Field::new("i", DataType::Int64, false),
        Field::new("j", DataType::Int64, false),
        Field::new("k", DataType::Int64, false),
    ];

    for (column_id, column_schema) in model.schema().iter() {
        fields.push(Field::new(
            column_id.as_str(),
            logical_type_to_arrow(column_schema.logical_type()),
            false,
        ));
    }

    let stored_schema = model
        .schema()
        .iter()
        .map(|(_, column_schema)| column_schema.clone())
        .collect::<Vec<_>>();
    let metadata = HashMap::from([
        (
            PARQUET_GRID_METADATA_KEY.to_owned(),
            serde_json::to_string(model.grid()).map_err(json_error)?,
        ),
        (
            PARQUET_SCHEMA_METADATA_KEY.to_owned(),
            serde_json::to_string(&stored_schema).map_err(json_error)?,
        ),
        (
            PARQUET_MODEL_METADATA_KEY.to_owned(),
            serde_json::to_string(model.metadata()).map_err(json_error)?,
        ),
    ]);

    Ok(Schema::new(fields).with_metadata(metadata))
}

fn build_parquet_index_arrays(
    grid: &GridDefinition,
) -> Result<(ArrayRef, ArrayRef, ArrayRef), MineError> {
    let mut i_values = Vec::with_capacity(grid.shape().total_cells());
    let mut j_values = Vec::with_capacity(grid.shape().total_cells());
    let mut k_values = Vec::with_capacity(grid.shape().total_cells());

    for linear_index in 0..grid.shape().total_cells() {
        let index = linear_to_ijk(grid, linear_index)?;
        i_values.push(usize_to_i64(index.i(), "i")?);
        j_values.push(usize_to_i64(index.j(), "j")?);
        k_values.push(usize_to_i64(index.k(), "k")?);
    }

    Ok((
        Arc::new(Int64Array::from(i_values)) as ArrayRef,
        Arc::new(Int64Array::from(j_values)) as ArrayRef,
        Arc::new(Int64Array::from(k_values)) as ArrayRef,
    ))
}

fn column_data_to_array(column_data: &ColumnData) -> ArrayRef {
    match column_data {
        ColumnData::Integers(values) => Arc::new(Int64Array::from(values.clone())) as ArrayRef,
        ColumnData::Floats(values) => Arc::new(Float64Array::from(values.clone())) as ArrayRef,
        ColumnData::Booleans(values) => Arc::new(BooleanArray::from(values.clone())) as ArrayRef,
        ColumnData::Texts(values) => Arc::new(StringArray::from(values.clone())) as ArrayRef,
    }
}

pub(super) fn decode_parquet_model_metadata(
    schema: &SchemaRef,
) -> Result<(GridDefinition, ColumnSchemaSet, Metadata), MineError> {
    let metadata = schema.metadata();
    let grid = decode_required_schema_metadata::<GridDefinition>(
        metadata,
        PARQUET_GRID_METADATA_KEY,
        "grid definition",
    )?;
    let stored_schema = decode_required_schema_metadata::<Vec<ColumnSchema>>(
        metadata,
        PARQUET_SCHEMA_METADATA_KEY,
        "column schema",
    )?;
    let model_metadata = decode_required_schema_metadata::<Metadata>(
        metadata,
        PARQUET_MODEL_METADATA_KEY,
        "model metadata",
    )?;
    let column_schema = ColumnSchemaSet::from_columns(stored_schema)?;

    Ok((grid, column_schema, model_metadata))
}

fn decode_required_schema_metadata<T>(
    metadata: &HashMap<String, String>,
    key: &'static str,
    label: &'static str,
) -> Result<T, MineError>
where
    T: serde::de::DeserializeOwned,
{
    let value = metadata
        .get(key)
        .ok_or_else(|| MineError::schema(format!("Arrow schema is missing `{label}` metadata")))?;

    serde_json::from_str(value).map_err(|error| {
        MineError::schema(format!(
            "unable to decode embedded `{label}` metadata: {error}"
        ))
    })
}

fn materialize_record_batch(
    batch: &RecordBatch,
    grid: &GridDefinition,
    column_schema: &ColumnSchemaSet,
    columns: &mut [(ColumnId, PendingColumnData)],
    occupied_rows: &mut [bool],
    row_offset: usize,
    format_name: &str,
) -> Result<(), MineError> {
    let i_values = required_int64_column(batch, "i")?;
    let j_values = required_int64_column(batch, "j")?;
    let k_values = required_int64_column(batch, "k")?;

    for row_index in 0..batch.num_rows() {
        ensure_not_null(i_values, row_index, "i", format_name)?;
        ensure_not_null(j_values, row_index, "j", format_name)?;
        ensure_not_null(k_values, row_index, "k", format_name)?;
        let grid_index = GridIndex::new(
            i64_to_usize(i_values.value(row_index), "i")?,
            i64_to_usize(j_values.value(row_index), "j")?,
            i64_to_usize(k_values.value(row_index), "k")?,
        );
        let linear_index = ijk_to_linear(grid, grid_index)?;
        let absolute_row = row_offset + row_index + 1;

        if occupied_rows[linear_index] {
            return Err(MineError::validation(format!(
                "{format_name} row {absolute_row} duplicates block index ({}, {}, {})",
                grid_index.i(),
                grid_index.j(),
                grid_index.k()
            )));
        }
        occupied_rows[linear_index] = true;

        for (column_id, pending_column) in columns.iter_mut() {
            let position = batch.schema().index_of(column_id.as_str()).map_err(|_| {
                MineError::schema(format!(
                    "column `{column_id}` is missing from {format_name} data"
                ))
            })?;
            pending_column.set_from_array(
                linear_index,
                batch.column(position).as_ref(),
                row_index,
                column_id,
            )?;
        }
    }

    for (column_id, _) in column_schema.iter() {
        if batch.schema().index_of(column_id.as_str()).is_err() {
            return Err(MineError::schema(format!(
                "column `{column_id}` is missing from {format_name} data"
            )));
        }
    }

    Ok(())
}

fn block_model_from_record_batches(
    schema: SchemaRef,
    batches: &[RecordBatch],
    format_name: &str,
) -> Result<BlockModel, MineError> {
    let (grid, column_schema, metadata) = decode_parquet_model_metadata(&schema)?;
    let total_cells = grid.shape().total_cells();
    let mut occupied_rows = vec![false; total_cells];
    let mut columns = column_schema
        .iter()
        .map(|(column_id, column_schema)| {
            (
                column_id.clone(),
                PendingColumnData::new(column_schema.logical_type(), total_cells),
            )
        })
        .collect::<Vec<_>>();
    let mut row_offset = 0_usize;

    for batch in batches {
        materialize_record_batch(
            batch,
            &grid,
            &column_schema,
            &mut columns,
            &mut occupied_rows,
            row_offset,
            format_name,
        )?;
        row_offset += batch.num_rows();
    }

    if let Some(missing_linear_index) = occupied_rows.iter().position(|occupied| !occupied) {
        let missing_index = linear_to_ijk(&grid, missing_linear_index)?;
        return Err(MineError::validation(format!(
            "{format_name} data is missing block index ({}, {}, {})",
            missing_index.i(),
            missing_index.j(),
            missing_index.k()
        )));
    }

    let finalized_columns: BTreeMap<ColumnId, ColumnData> = columns
        .into_iter()
        .map(|(column_id, pending_column)| {
            pending_column
                .finalize(column_id.clone())
                .map(|column_data| (column_id, column_data))
        })
        .collect::<Result<_, _>>()?;

    BlockModel::new(grid, column_schema, metadata, finalized_columns)
}

fn logical_type_to_arrow(logical_type: ColumnLogicalType) -> DataType {
    match logical_type {
        ColumnLogicalType::Integer => DataType::Int64,
        ColumnLogicalType::Float => DataType::Float64,
        ColumnLogicalType::Boolean => DataType::Boolean,
        ColumnLogicalType::Text => DataType::Utf8,
    }
}
