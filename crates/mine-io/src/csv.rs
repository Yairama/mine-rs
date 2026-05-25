use super::*;

/// Nombres de columnas CSV usados para materializar índices `i/j/k`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvIndexColumns {
    i_column: String,
    j_column: String,
    k_column: String,
}

impl CsvIndexColumns {
    /// Construye nombres explícitos para las columnas de índice.
    pub fn new(
        i_column: impl Into<String>,
        j_column: impl Into<String>,
        k_column: impl Into<String>,
    ) -> Result<Self, MineError> {
        Ok(Self {
            i_column: validate_header_name("i_column", i_column.into())?,
            j_column: validate_header_name("j_column", j_column.into())?,
            k_column: validate_header_name("k_column", k_column.into())?,
        })
    }

    /// Devuelve el nombre de la columna `i`.
    #[must_use]
    pub fn i_column(&self) -> &str {
        &self.i_column
    }

    /// Devuelve el nombre de la columna `j`.
    #[must_use]
    pub fn j_column(&self) -> &str {
        &self.j_column
    }

    /// Devuelve el nombre de la columna `k`.
    #[must_use]
    pub fn k_column(&self) -> &str {
        &self.k_column
    }
}

/// Opciones mínimas para reconstruir un `BlockModel` desde CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct CsvReadOptions {
    grid: GridDefinition,
    schema: ColumnSchemaSet,
    metadata: Metadata,
    index_columns: CsvIndexColumns,
}

impl CsvReadOptions {
    /// Construye opciones explícitas de lectura CSV.
    #[must_use]
    pub fn new(
        grid: GridDefinition,
        schema: ColumnSchemaSet,
        metadata: Metadata,
        index_columns: CsvIndexColumns,
    ) -> Self {
        Self {
            grid,
            schema,
            metadata,
            index_columns,
        }
    }

    /// Devuelve la grilla del modelo.
    #[must_use]
    pub const fn grid(&self) -> &GridDefinition {
        &self.grid
    }

    /// Devuelve el schema esperado.
    #[must_use]
    pub const fn schema(&self) -> &ColumnSchemaSet {
        &self.schema
    }

    /// Devuelve la metadata a asociar al modelo cargado.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Devuelve los nombres de columnas de índice esperados.
    #[must_use]
    pub const fn index_columns(&self) -> &CsvIndexColumns {
        &self.index_columns
    }
}

/// Opciones mínimas para exportar un `BlockModel` a CSV.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsvWriteOptions {
    index_columns: CsvIndexColumns,
    selected_columns: Option<Vec<ColumnId>>,
}

impl CsvWriteOptions {
    /// Construye opciones de escritura CSV.
    pub fn new(
        index_columns: CsvIndexColumns,
        selected_columns: Option<Vec<ColumnId>>,
    ) -> Result<Self, MineError> {
        validate_selected_columns(selected_columns.as_deref())?;

        Ok(Self {
            index_columns,
            selected_columns,
        })
    }

    /// Devuelve los nombres de columnas de índice a escribir.
    #[must_use]
    pub const fn index_columns(&self) -> &CsvIndexColumns {
        &self.index_columns
    }

    /// Devuelve las columnas seleccionadas o `None` para escribir todo el schema.
    #[must_use]
    pub fn selected_columns(&self) -> Option<&[ColumnId]> {
        self.selected_columns.as_deref()
    }
}

/// Escribe un `BlockModel` regular a CSV con columnas `i/j/k` explícitas.
pub fn write_block_model_csv(
    model: &BlockModel,
    path: impl AsRef<Path>,
    options: &CsvWriteOptions,
) -> Result<(), MineError> {
    let selected_columns = resolve_selected_columns(model, options.selected_columns())?;
    let mut writer = WriterBuilder::new()
        .has_headers(true)
        .from_path(path.as_ref())
        .map_err(|error| io_error(format!("unable to open CSV writer: {error}")))?;

    let mut header = Vec::with_capacity(3 + selected_columns.len());
    header.push(options.index_columns().i_column().to_owned());
    header.push(options.index_columns().j_column().to_owned());
    header.push(options.index_columns().k_column().to_owned());
    header.extend(
        selected_columns
            .iter()
            .map(|column_id| column_id.to_string()),
    );
    writer
        .write_record(&header)
        .map_err(|error| io_error(format!("unable to write CSV header: {error}")))?;

    for row_index in 0..model.block_count() {
        let linear_index = model.linear_index_at(row_index)?;
        let index = linear_to_ijk(model.grid(), linear_index)?;
        let mut record = Vec::with_capacity(header.len());
        record.push(index.i().to_string());
        record.push(index.j().to_string());
        record.push(index.k().to_string());

        for column_id in &selected_columns {
            let column_data = model.column(column_id).ok_or_else(|| {
                MineError::schema(format!(
                    "column `{column_id}` is present in schema but missing from block model storage"
                ))
            })?;
            record.push(stringify_value(column_data, row_index, column_id)?);
        }

        writer
            .write_record(&record)
            .map_err(|error| io_error(format!("unable to write CSV row: {error}")))?;
    }

    writer
        .flush()
        .map_err(|error| io_error(format!("unable to flush CSV writer: {error}")))?;

    Ok(())
}

/// Lee un `BlockModel` regular desde CSV usando grilla, schema e índices explícitos.
pub fn read_block_model_csv(
    path: impl AsRef<Path>,
    options: &CsvReadOptions,
) -> Result<BlockModel, MineError> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path.as_ref())
        .map_err(|error| io_error(format!("unable to open CSV file: {error}")))?;
    let headers = reader
        .headers()
        .map_err(|error| io_error(format!("unable to read CSV header: {error}")))?
        .clone();
    let index_positions = IndexColumnPositions::from_headers(&headers, options.index_columns())?;
    let schema_positions = SchemaColumnPositions::from_headers(&headers, options.schema())?;
    let total_cells = options.grid().shape().total_cells();
    let mut occupied_rows = vec![false; total_cells];
    let mut columns = options
        .schema()
        .iter()
        .map(|(column_id, column_schema)| {
            (
                column_id.clone(),
                PendingColumnData::new(column_schema.logical_type(), total_cells),
            )
        })
        .collect::<Vec<_>>();

    for (row_offset, row_result) in reader.records().enumerate() {
        let row_number = row_offset + 2;
        let row = row_result
            .map_err(|error| io_error(format!("unable to read CSV row {row_number}: {error}")))?;
        let grid_index = parse_grid_index(&row, row_number, &index_positions)?;
        let linear_index = ijk_to_linear(options.grid(), grid_index)?;

        if occupied_rows[linear_index] {
            return Err(MineError::validation(format!(
                "CSV row {row_number} duplicates block index ({}, {}, {})",
                grid_index.i(),
                grid_index.j(),
                grid_index.k()
            )));
        }
        occupied_rows[linear_index] = true;

        for (column_id, pending_column) in &mut columns {
            let position = schema_positions.position(column_id).ok_or_else(|| {
                MineError::schema(format!("column `{column_id}` is missing from CSV header"))
            })?;
            let raw_value = value_at(&row, position, row_number, column_id.as_str())?;
            pending_column.set(linear_index, raw_value, row_number, column_id)?;
        }
    }

    if let Some(missing_linear_index) = occupied_rows.iter().position(|occupied| !occupied) {
        let missing_index = linear_to_ijk(options.grid(), missing_linear_index)?;
        return Err(MineError::validation(format!(
            "CSV data is missing block index ({}, {}, {})",
            missing_index.i(),
            missing_index.j(),
            missing_index.k()
        )));
    }

    let finalized_columns = columns
        .into_iter()
        .map(|(column_id, pending_column)| {
            pending_column
                .finalize(column_id.clone())
                .map(|column_data| (column_id, column_data))
        })
        .collect::<Result<_, _>>()?;

    BlockModel::new(
        options.grid().clone(),
        options.schema().clone(),
        options.metadata().clone(),
        finalized_columns,
    )
}
