use super::*;

/// Nombres de columnas espaciales para exportes estilo Vulcan CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulcanCoordinateColumns {
    x_column: String,
    y_column: String,
    z_column: String,
}

impl VulcanCoordinateColumns {
    /// Construye nombres explícitos para las columnas espaciales.
    pub fn new(
        x_column: impl Into<String>,
        y_column: impl Into<String>,
        z_column: impl Into<String>,
    ) -> Result<Self, MineError> {
        Ok(Self {
            x_column: validate_header_name("x_column", x_column.into())?,
            y_column: validate_header_name("y_column", y_column.into())?,
            z_column: validate_header_name("z_column", z_column.into())?,
        })
    }

    /// Devuelve el nombre de la columna `x`.
    #[must_use]
    pub fn x_column(&self) -> &str {
        &self.x_column
    }

    /// Devuelve el nombre de la columna `y`.
    #[must_use]
    pub fn y_column(&self) -> &str {
        &self.y_column
    }

    /// Devuelve el nombre de la columna `z`.
    #[must_use]
    pub fn z_column(&self) -> &str {
        &self.z_column
    }
}

/// Representación booleana usada por el exporter compatible con Vulcan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VulcanBooleanFormat {
    /// Exporta `0` y `1`.
    ZeroOne,
    /// Exporta `false` y `true`.
    TrueFalse,
}

/// Opciones configurables para exportar CSV compatibles con workflows Vulcan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulcanCsvWriteOptions {
    coordinate_columns: VulcanCoordinateColumns,
    index_columns: Option<CsvIndexColumns>,
    selected_columns: Option<Vec<ColumnId>>,
    column_aliases: BTreeMap<ColumnId, String>,
    boolean_format: VulcanBooleanFormat,
}

impl VulcanCsvWriteOptions {
    /// Construye opciones del exporter Vulcan CSV.
    pub fn new(
        coordinate_columns: VulcanCoordinateColumns,
        index_columns: Option<CsvIndexColumns>,
        selected_columns: Option<Vec<ColumnId>>,
        column_aliases: BTreeMap<ColumnId, String>,
        boolean_format: VulcanBooleanFormat,
    ) -> Result<Self, MineError> {
        validate_selected_columns(selected_columns.as_deref())?;
        validate_vulcan_aliases(&column_aliases)?;

        Ok(Self {
            coordinate_columns,
            index_columns,
            selected_columns,
            column_aliases,
            boolean_format,
        })
    }

    /// Devuelve los nombres de columnas espaciales.
    #[must_use]
    pub const fn coordinate_columns(&self) -> &VulcanCoordinateColumns {
        &self.coordinate_columns
    }

    /// Devuelve las columnas de índice opcionales.
    #[must_use]
    pub const fn index_columns(&self) -> Option<&CsvIndexColumns> {
        self.index_columns.as_ref()
    }

    /// Devuelve las columnas seleccionadas o `None` para exportar todo el schema.
    #[must_use]
    pub fn selected_columns(&self) -> Option<&[ColumnId]> {
        self.selected_columns.as_deref()
    }

    /// Devuelve los aliases de columnas configurados.
    #[must_use]
    pub const fn column_aliases(&self) -> &BTreeMap<ColumnId, String> {
        &self.column_aliases
    }

    /// Devuelve el formato booleano configurado.
    #[must_use]
    pub const fn boolean_format(&self) -> VulcanBooleanFormat {
        self.boolean_format
    }
}

/// Escribe un `BlockModel` a CSV con convenciones configurables para workflows Vulcan.
pub fn write_block_model_vulcan_csv(
    model: &BlockModel,
    path: impl AsRef<Path>,
    options: &VulcanCsvWriteOptions,
) -> Result<(), MineError> {
    let selected_columns = resolve_selected_columns(model, options.selected_columns())?;
    let header = build_vulcan_header(options, &selected_columns)?;
    let mut writer = WriterBuilder::new()
        .has_headers(true)
        .from_path(path.as_ref())
        .map_err(|error| io_error(format!("unable to open Vulcan CSV writer: {error}")))?;

    writer
        .write_record(&header)
        .map_err(|error| io_error(format!("unable to write Vulcan CSV header: {error}")))?;

    for row_index in 0..model.block_count() {
        let linear_index = model.linear_index_at(row_index)?;
        let grid_index = linear_to_ijk(model.grid(), linear_index)?;
        let center = ijk_to_xyz(model.grid(), grid_index)?;
        let mut record = Vec::with_capacity(header.len());
        record.push(center.x().to_string());
        record.push(center.y().to_string());
        record.push(center.z().to_string());

        if options.index_columns().is_some() {
            record.push(grid_index.i().to_string());
            record.push(grid_index.j().to_string());
            record.push(grid_index.k().to_string());
        }

        for column_id in &selected_columns {
            let column_data = model.column(column_id).ok_or_else(|| {
                MineError::schema(format!(
                    "column `{column_id}` is present in schema but missing from block model storage"
                ))
            })?;
            record.push(stringify_vulcan_value(
                column_data,
                row_index,
                column_id,
                options.boolean_format(),
            )?);
        }

        writer
            .write_record(&record)
            .map_err(|error| io_error(format!("unable to write Vulcan CSV row: {error}")))?;
    }

    writer
        .flush()
        .map_err(|error| io_error(format!("unable to flush Vulcan CSV writer: {error}")))?;

    Ok(())
}

fn validate_vulcan_aliases(column_aliases: &BTreeMap<ColumnId, String>) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();

    for alias in column_aliases.values() {
        let alias = validate_header_name("column_alias", alias.clone())?;

        if !seen.insert(alias) {
            return Err(MineError::invalid_parameter(
                "column_aliases",
                "Vulcan CSV aliases must be unique",
            ));
        }
    }

    Ok(())
}

fn build_vulcan_header(
    options: &VulcanCsvWriteOptions,
    selected_columns: &[ColumnId],
) -> Result<Vec<String>, MineError> {
    let mut header = Vec::with_capacity(
        3 + usize::from(options.index_columns().is_some()) * 3 + selected_columns.len(),
    );
    let mut seen = BTreeSet::new();
    let coordinate_columns = options.coordinate_columns();

    for column_name in [
        coordinate_columns.x_column(),
        coordinate_columns.y_column(),
        coordinate_columns.z_column(),
    ] {
        if !seen.insert(column_name.to_owned()) {
            return Err(MineError::invalid_parameter(
                "coordinate_columns",
                "Vulcan CSV coordinate columns must be unique",
            ));
        }
        header.push(column_name.to_owned());
    }

    if let Some(index_columns) = options.index_columns() {
        for column_name in [
            index_columns.i_column(),
            index_columns.j_column(),
            index_columns.k_column(),
        ] {
            if !seen.insert(column_name.to_owned()) {
                return Err(MineError::invalid_parameter(
                    "index_columns",
                    "Vulcan CSV header names must be unique",
                ));
            }
            header.push(column_name.to_owned());
        }
    }

    for column_id in selected_columns {
        let column_name = options
            .column_aliases()
            .get(column_id)
            .map_or_else(|| column_id.to_string(), Clone::clone);

        if !seen.insert(column_name.clone()) {
            return Err(MineError::invalid_parameter(
                "column_aliases",
                format!("Vulcan CSV header `{column_name}` is duplicated"),
            ));
        }

        header.push(column_name);
    }

    Ok(header)
}
