use super::*;

/// Escribe un `ExperimentalVariogram` a JSON usando filas planas por lag.
pub fn write_experimental_variogram_json(
    variogram: &ExperimentalVariogram,
    path: impl AsRef<Path>,
) -> Result<(), MineError> {
    let file = File::create(path.as_ref())
        .map_err(|error| io_error(format!("unable to create variogram JSON file: {error}")))?;
    serde_json::to_writer_pretty(file, &variogram.lag_rows())
        .map_err(|error| io_error(format!("unable to write variogram JSON: {error}")))?;
    Ok(())
}

/// Lee un `ExperimentalVariogram` desde JSON usando filas planas por lag.
pub fn read_experimental_variogram_json(
    path: impl AsRef<Path>,
) -> Result<ExperimentalVariogram, MineError> {
    let file = File::open(path.as_ref())
        .map_err(|error| io_error(format!("unable to open variogram JSON file: {error}")))?;
    let rows = serde_json::from_reader::<_, Vec<ExperimentalVariogramLagRow>>(file)
        .map_err(|error| io_error(format!("unable to decode variogram JSON rows: {error}")))?;
    experimental_variogram_from_lag_rows(&rows)
}

/// Convierte un `ExperimentalVariogram` a `RecordBatch` Arrow plano por lag.
pub fn experimental_variogram_to_record_batch(
    variogram: &ExperimentalVariogram,
) -> Result<RecordBatch, MineError> {
    let rows = variogram.lag_rows();
    let schema = Arc::new(Schema::new(vec![
        Field::new("column_id", DataType::Utf8, false),
        Field::new("domain", DataType::Utf8, true),
        Field::new("direction_x", DataType::Float64, true),
        Field::new("direction_y", DataType::Float64, true),
        Field::new("direction_z", DataType::Float64, true),
        Field::new("angular_tolerance_degrees", DataType::Float64, true),
        Field::new("bandwidth", DataType::Float64, true),
        Field::new("lag_size", DataType::Float64, false),
        Field::new("lag_count", DataType::Int64, false),
        Field::new("lag_tolerance", DataType::Float64, false),
        Field::new("sample_count", DataType::Int64, false),
        Field::new("lag_index", DataType::Int64, false),
        Field::new("lag_center", DataType::Float64, false),
        Field::new("pair_count", DataType::Int64, false),
        Field::new("average_distance", DataType::Float64, true),
        Field::new("semivariance", DataType::Float64, true),
    ]));
    let arrays: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(
            rows.iter()
                .map(|row| row.column_id.to_string())
                .collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(StringArray::from(
            rows.iter()
                .map(|row| row.domain.clone())
                .collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.direction_x).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.direction_y).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.direction_z).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter()
                .map(|row| row.angular_tolerance_degrees)
                .collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.bandwidth).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.lag_size).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Int64Array::from(
            rows.iter()
                .map(|row| usize_to_i64(row.lag_count, "lag_count"))
                .collect::<Result<Vec<_>, _>>()?,
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.lag_tolerance).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Int64Array::from(
            rows.iter()
                .map(|row| usize_to_i64(row.sample_count, "sample_count"))
                .collect::<Result<Vec<_>, _>>()?,
        )) as ArrayRef,
        Arc::new(Int64Array::from(
            rows.iter()
                .map(|row| usize_to_i64(row.lag_index, "lag_index"))
                .collect::<Result<Vec<_>, _>>()?,
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.lag_center).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Int64Array::from(
            rows.iter()
                .map(|row| usize_to_i64(row.pair_count, "pair_count"))
                .collect::<Result<Vec<_>, _>>()?,
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter()
                .map(|row| row.average_distance)
                .collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(Float64Array::from(
            rows.iter().map(|row| row.semivariance).collect::<Vec<_>>(),
        )) as ArrayRef,
    ];

    RecordBatch::try_new(schema, arrays)
        .map_err(|error| MineError::schema(format!("unable to build variogram batch: {error}")))
}

/// Reconstruye un `ExperimentalVariogram` desde un `RecordBatch` Arrow plano por lag.
pub fn experimental_variogram_from_record_batch(
    batch: &RecordBatch,
) -> Result<ExperimentalVariogram, MineError> {
    let column_ids = batch
        .column(
            batch.schema().index_of("column_id").map_err(|_| {
                MineError::schema("column `column_id` is missing from Parquet data")
            })?,
        )
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| MineError::schema("column `column_id` is not stored as Utf8 in Parquet"))?;
    let domains = batch
        .column(
            batch
                .schema()
                .index_of("domain")
                .map_err(|_| MineError::schema("column `domain` is missing from Parquet data"))?,
        )
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| MineError::schema("column `domain` is not stored as Utf8 in Parquet"))?;
    let direction_x =
        batch
            .column(batch.schema().index_of("direction_x").map_err(|_| {
                MineError::schema("column `direction_x` is missing from Parquet data")
            })?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                MineError::schema("column `direction_x` is not stored as Float64 in Parquet")
            })?;
    let direction_y =
        batch
            .column(batch.schema().index_of("direction_y").map_err(|_| {
                MineError::schema("column `direction_y` is missing from Parquet data")
            })?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                MineError::schema("column `direction_y` is not stored as Float64 in Parquet")
            })?;
    let direction_z =
        batch
            .column(batch.schema().index_of("direction_z").map_err(|_| {
                MineError::schema("column `direction_z` is missing from Parquet data")
            })?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                MineError::schema("column `direction_z` is not stored as Float64 in Parquet")
            })?;
    let angular_tolerance_degrees = batch
        .column(
            batch
                .schema()
                .index_of("angular_tolerance_degrees")
                .map_err(|_| {
                    MineError::schema(
                        "column `angular_tolerance_degrees` is missing from Parquet data",
                    )
                })?,
        )
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| {
            MineError::schema(
                "column `angular_tolerance_degrees` is not stored as Float64 in Parquet",
            )
        })?;
    let bandwidth =
        batch
            .column(batch.schema().index_of("bandwidth").map_err(|_| {
                MineError::schema("column `bandwidth` is missing from Parquet data")
            })?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                MineError::schema("column `bandwidth` is not stored as Float64 in Parquet")
            })?;
    let lag_size = batch
        .column(
            batch
                .schema()
                .index_of("lag_size")
                .map_err(|_| MineError::schema("column `lag_size` is missing from Parquet data"))?,
        )
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| {
            MineError::schema("column `lag_size` is not stored as Float64 in Parquet")
        })?;
    let lag_count = required_int64_column(batch, "lag_count")?;
    let lag_tolerance = batch
        .column(batch.schema().index_of("lag_tolerance").map_err(|_| {
            MineError::schema("column `lag_tolerance` is missing from Parquet data")
        })?)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| {
            MineError::schema("column `lag_tolerance` is not stored as Float64 in Parquet")
        })?;
    let sample_count = required_int64_column(batch, "sample_count")?;
    let lag_index = required_int64_column(batch, "lag_index")?;
    let lag_center =
        batch
            .column(batch.schema().index_of("lag_center").map_err(|_| {
                MineError::schema("column `lag_center` is missing from Parquet data")
            })?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                MineError::schema("column `lag_center` is not stored as Float64 in Parquet")
            })?;
    let pair_count = required_int64_column(batch, "pair_count")?;
    let average_distance = batch
        .column(batch.schema().index_of("average_distance").map_err(|_| {
            MineError::schema("column `average_distance` is missing from Parquet data")
        })?)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| {
            MineError::schema("column `average_distance` is not stored as Float64 in Parquet")
        })?;
    let semivariance =
        batch
            .column(batch.schema().index_of("semivariance").map_err(|_| {
                MineError::schema("column `semivariance` is missing from Parquet data")
            })?)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                MineError::schema("column `semivariance` is not stored as Float64 in Parquet")
            })?;

    let mut rows = Vec::with_capacity(batch.num_rows());
    for row_index in 0..batch.num_rows() {
        ensure_not_null(column_ids, row_index, "column_id", "Parquet")?;
        ensure_not_null(lag_size, row_index, "lag_size", "Parquet")?;
        ensure_not_null(lag_count, row_index, "lag_count", "Parquet")?;
        ensure_not_null(lag_tolerance, row_index, "lag_tolerance", "Parquet")?;
        ensure_not_null(sample_count, row_index, "sample_count", "Parquet")?;
        ensure_not_null(lag_index, row_index, "lag_index", "Parquet")?;
        ensure_not_null(lag_center, row_index, "lag_center", "Parquet")?;
        ensure_not_null(pair_count, row_index, "pair_count", "Parquet")?;

        rows.push(ExperimentalVariogramLagRow {
            column_id: ColumnId::new(column_ids.value(row_index))?,
            domain: (!domains.is_null(row_index)).then(|| domains.value(row_index).to_owned()),
            direction_x: (!direction_x.is_null(row_index)).then(|| direction_x.value(row_index)),
            direction_y: (!direction_y.is_null(row_index)).then(|| direction_y.value(row_index)),
            direction_z: (!direction_z.is_null(row_index)).then(|| direction_z.value(row_index)),
            angular_tolerance_degrees: (!angular_tolerance_degrees.is_null(row_index))
                .then(|| angular_tolerance_degrees.value(row_index)),
            bandwidth: (!bandwidth.is_null(row_index)).then(|| bandwidth.value(row_index)),
            lag_size: lag_size.value(row_index),
            lag_count: i64_to_usize(lag_count.value(row_index), "lag_count")?,
            lag_tolerance: lag_tolerance.value(row_index),
            sample_count: i64_to_usize(sample_count.value(row_index), "sample_count")?,
            lag_index: i64_to_usize(lag_index.value(row_index), "lag_index")?,
            lag_center: lag_center.value(row_index),
            pair_count: i64_to_usize(pair_count.value(row_index), "pair_count")?,
            average_distance: (!average_distance.is_null(row_index))
                .then(|| average_distance.value(row_index)),
            semivariance: (!semivariance.is_null(row_index)).then(|| semivariance.value(row_index)),
        });
    }

    experimental_variogram_from_lag_rows(&rows)
}

/// Escribe un `ExperimentalVariogram` a Parquet usando una tabla plana por lag.
pub fn write_experimental_variogram_parquet(
    variogram: &ExperimentalVariogram,
    path: impl AsRef<Path>,
) -> Result<(), MineError> {
    let batch = experimental_variogram_to_record_batch(variogram)?;
    let file = File::create(path.as_ref())
        .map_err(|error| io_error(format!("unable to create variogram Parquet file: {error}")))?;
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None).map_err(|error| {
        io_error(format!(
            "unable to create variogram Parquet writer: {error}"
        ))
    })?;

    writer
        .write(&batch)
        .map_err(|error| io_error(format!("unable to write variogram Parquet batch: {error}")))?;
    writer
        .close()
        .map_err(|error| io_error(format!("unable to close variogram Parquet writer: {error}")))?;
    Ok(())
}

/// Lee un `ExperimentalVariogram` desde Parquet usando una tabla plana por lag.
pub fn read_experimental_variogram_parquet(
    path: impl AsRef<Path>,
) -> Result<ExperimentalVariogram, MineError> {
    let file = File::open(path.as_ref())
        .map_err(|error| io_error(format!("unable to open variogram Parquet file: {error}")))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|error| io_error(format!("unable to open variogram Parquet file: {error}")))?;
    let reader = builder
        .build()
        .map_err(|error| io_error(format!("unable to build variogram Parquet reader: {error}")))?;
    let batches = reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(format!("unable to read variogram Parquet batches: {error}")))?;
    if batches.is_empty() {
        return Err(MineError::validation(
            "variogram Parquet file does not contain any record batches",
        ));
    }
    if batches.len() > 1 {
        return Err(MineError::validation(
            "variogram Parquet reader returned multiple batches; a single batch is expected",
        ));
    }

    experimental_variogram_from_record_batch(&batches[0])
}
