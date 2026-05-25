use super::*;

/// Hints explícitos para inferir schema sin asumir roles mineros críticos en silencio.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaInferenceHints {
    /// Columnas `i/j/k` si el archivo las usa como índices espaciales.
    pub index_columns: Option<CsvIndexColumns>,
    /// Columnas confirmadas como leyes.
    pub grade_columns: Vec<ColumnId>,
    /// Columna confirmada como tonelaje.
    pub tonnage_column: Option<ColumnId>,
    /// Columna confirmada como banco.
    pub bench_column: Option<ColumnId>,
    /// Columna confirmada como fase.
    pub phase_column: Option<ColumnId>,
    /// Columna confirmada como dominio.
    pub domain_column: Option<ColumnId>,
}

/// Código estructurado de warning durante inferencia de schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaInferenceWarningCode {
    /// La fuente parece tener columnas espaciales pero no se confirmó su uso.
    SpatialColumnsRequireConfirmation,
    /// Hay una columna candidata a tonelaje sin confirmación explícita.
    TonnageColumnRequiresConfirmation,
    /// Hay una columna numérica continua que podría usarse como ley.
    GradeColumnRequiresConfirmation,
}

/// Warning estructurado emitido por la inferencia controlada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaInferenceWarning {
    /// Código estable del warning.
    pub code: SchemaInferenceWarningCode,
    /// Columna asociada cuando aplica.
    pub column: Option<ColumnId>,
    /// Mensaje legible para usuarios y capas superiores.
    pub message: String,
}

/// Resultado de inferencia controlada de schema.
#[derive(Debug, Clone, PartialEq)]
pub struct InferredModelSchema {
    schema: ColumnSchemaSet,
    warnings: Vec<SchemaInferenceWarning>,
}

impl InferredModelSchema {
    /// Construye un resultado de inferencia controlada.
    #[must_use]
    pub fn new(schema: ColumnSchemaSet, warnings: Vec<SchemaInferenceWarning>) -> Self {
        Self { schema, warnings }
    }

    /// Devuelve el schema inferido.
    #[must_use]
    pub const fn schema(&self) -> &ColumnSchemaSet {
        &self.schema
    }

    /// Devuelve los warnings emitidos durante la inferencia.
    #[must_use]
    pub fn warnings(&self) -> &[SchemaInferenceWarning] {
        &self.warnings
    }
}

/// Infiere un schema desde CSV sin asumir roles críticos silenciosamente.
pub fn infer_csv_schema(
    path: impl AsRef<Path>,
    hints: &SchemaInferenceHints,
) -> Result<InferredModelSchema, MineError> {
    validate_inference_hints(hints)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path.as_ref())
        .map_err(|error| io_error(format!("unable to open CSV file: {error}")))?;
    let headers = reader
        .headers()
        .map_err(|error| io_error(format!("unable to read CSV header: {error}")))?
        .clone();
    let attribute_columns = infer_attribute_columns_from_headers(&headers, hints)?;
    let mut states = attribute_columns
        .iter()
        .map(|(column_id, position)| (*position, column_id.clone(), CsvInferenceState::default()))
        .collect::<Vec<_>>();

    for row_result in reader.records() {
        let row =
            row_result.map_err(|error| io_error(format!("unable to read CSV row: {error}")))?;
        for (position, _, state) in &mut states {
            let value = row
                .get(*position)
                .ok_or_else(|| MineError::validation("CSV row is shorter than the header"))?;
            state.observe(value);
        }
    }

    let inferred_columns = states
        .into_iter()
        .map(|(_, column_id, state)| {
            build_inferred_column(column_id, state.logical_type(), state.nullable, hints)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let warnings = build_inference_warnings(
        inferred_columns
            .iter()
            .map(|column| (column.name().clone(), column.logical_type()))
            .collect::<Vec<_>>(),
        hints,
    );

    Ok(InferredModelSchema::new(
        ColumnSchemaSet::from_columns(inferred_columns)?,
        warnings,
    ))
}

/// Infiere o recupera un schema desde Parquet sin asumir roles críticos silenciosamente.
pub fn infer_parquet_schema(
    path: impl AsRef<Path>,
    hints: &SchemaInferenceHints,
) -> Result<InferredModelSchema, MineError> {
    validate_inference_hints(hints)?;
    let file = File::open(path.as_ref())?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|error| io_error(format!("unable to open Parquet file: {error}")))?;
    let schema = builder.schema().clone();

    if schema.metadata().contains_key(PARQUET_SCHEMA_METADATA_KEY) {
        let (_, recovered_schema, _) = crate::parquet::decode_parquet_model_metadata(&schema)?;
        return Ok(InferredModelSchema::new(recovered_schema, Vec::new()));
    }

    let attribute_fields = infer_attribute_fields_from_schema(&schema, hints)?;
    let inferred_columns = attribute_fields
        .into_iter()
        .map(|field| {
            build_inferred_column(
                ColumnId::new(field.name())?,
                arrow_field_to_logical_type(field.data_type())?,
                field.is_nullable(),
                hints,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let warnings = build_inference_warnings(
        inferred_columns
            .iter()
            .map(|column| (column.name().clone(), column.logical_type()))
            .collect::<Vec<_>>(),
        hints,
    );

    Ok(InferredModelSchema::new(
        ColumnSchemaSet::from_columns(inferred_columns)?,
        warnings,
    ))
}

#[derive(Debug, Clone, Copy, Default)]
struct CsvInferenceState {
    nullable: bool,
    saw_non_empty: bool,
    all_boolean: bool,
    all_integer: bool,
    all_float: bool,
}

impl CsvInferenceState {
    fn observe(&mut self, value: &str) {
        if !self.saw_non_empty {
            self.all_boolean = true;
            self.all_integer = true;
            self.all_float = true;
        }

        let value = value.trim();
        if value.is_empty() {
            self.nullable = true;
            return;
        }

        self.saw_non_empty = true;
        self.all_boolean &= parse_boolean_like(value);
        self.all_integer &= value.parse::<i64>().is_ok();
        self.all_float &= value.parse::<f64>().is_ok();
    }

    fn logical_type(self) -> ColumnLogicalType {
        if !self.saw_non_empty {
            ColumnLogicalType::Text
        } else if self.all_boolean {
            ColumnLogicalType::Boolean
        } else if self.all_integer {
            ColumnLogicalType::Integer
        } else if self.all_float {
            ColumnLogicalType::Float
        } else {
            ColumnLogicalType::Text
        }
    }
}

fn validate_inference_hints(hints: &SchemaInferenceHints) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();

    for column_id in &hints.grade_columns {
        if !seen.insert(column_id.as_str().to_owned()) {
            return Err(MineError::invalid_parameter(
                "grade_columns",
                format!("column `{column_id}` is duplicated in inference hints"),
            ));
        }
    }

    for (parameter, column_id) in [
        ("tonnage_column", hints.tonnage_column.as_ref()),
        ("bench_column", hints.bench_column.as_ref()),
        ("phase_column", hints.phase_column.as_ref()),
        ("domain_column", hints.domain_column.as_ref()),
    ] {
        if let Some(column_id) = column_id
            && !seen.insert(column_id.as_str().to_owned())
        {
            return Err(MineError::invalid_parameter(
                parameter,
                format!("column `{column_id}` is duplicated across inference hints"),
            ));
        }
    }

    Ok(())
}

fn infer_attribute_columns_from_headers(
    headers: &StringRecord,
    hints: &SchemaInferenceHints,
) -> Result<Vec<(ColumnId, usize)>, MineError> {
    let excluded_headers = excluded_header_names(hints);
    let mut columns = Vec::new();

    for (position, header) in headers.iter().enumerate() {
        if excluded_headers.contains(header) {
            continue;
        }

        columns.push((ColumnId::new(header)?, position));
    }

    validate_required_hint_headers(headers, hints)?;

    Ok(columns)
}

fn infer_attribute_fields_from_schema(
    schema: &SchemaRef,
    hints: &SchemaInferenceHints,
) -> Result<Vec<Field>, MineError> {
    let excluded_headers = excluded_header_names(hints);
    let fields = schema
        .fields()
        .iter()
        .filter(|field| !excluded_headers.contains(field.name().as_str()))
        .map(|field| field.as_ref().clone())
        .collect::<Vec<_>>();

    validate_required_hint_fields(schema, hints)?;

    Ok(fields)
}

fn excluded_header_names(hints: &SchemaInferenceHints) -> BTreeSet<String> {
    hints
        .index_columns
        .as_ref()
        .map(|columns| {
            BTreeSet::from([
                columns.i_column().to_owned(),
                columns.j_column().to_owned(),
                columns.k_column().to_owned(),
            ])
        })
        .unwrap_or_default()
}

fn validate_required_hint_headers(
    headers: &StringRecord,
    hints: &SchemaInferenceHints,
) -> Result<(), MineError> {
    if let Some(index_columns) = &hints.index_columns {
        for column_name in [
            index_columns.i_column(),
            index_columns.j_column(),
            index_columns.k_column(),
        ] {
            find_header_position(headers, column_name)?;
        }
    }

    for column_id in all_hint_columns(hints) {
        find_header_position(headers, column_id.as_str())?;
    }

    Ok(())
}

fn validate_required_hint_fields(
    schema: &SchemaRef,
    hints: &SchemaInferenceHints,
) -> Result<(), MineError> {
    if let Some(index_columns) = &hints.index_columns {
        for column_name in [
            index_columns.i_column(),
            index_columns.j_column(),
            index_columns.k_column(),
        ] {
            schema.field_with_name(column_name).map_err(|_| {
                MineError::schema(format!(
                    "column `{column_name}` is missing from Parquet data"
                ))
            })?;
        }
    }

    for column_id in all_hint_columns(hints) {
        schema.field_with_name(column_id.as_str()).map_err(|_| {
            MineError::schema(format!("column `{column_id}` is missing from Parquet data"))
        })?;
    }

    Ok(())
}

fn all_hint_columns(hints: &SchemaInferenceHints) -> Vec<&ColumnId> {
    let mut columns = hints.grade_columns.iter().collect::<Vec<_>>();

    if let Some(column_id) = hints.tonnage_column.as_ref() {
        columns.push(column_id);
    }
    if let Some(column_id) = hints.bench_column.as_ref() {
        columns.push(column_id);
    }
    if let Some(column_id) = hints.phase_column.as_ref() {
        columns.push(column_id);
    }
    if let Some(column_id) = hints.domain_column.as_ref() {
        columns.push(column_id);
    }

    columns
}

fn build_inferred_column(
    column_id: ColumnId,
    logical_type: ColumnLogicalType,
    nullable: bool,
    hints: &SchemaInferenceHints,
) -> Result<ColumnSchema, MineError> {
    Ok(ColumnSchema::new(
        column_id.clone(),
        logical_type,
        None,
        nullable,
        inferred_role_for_column(&column_id, hints),
    ))
}

fn inferred_role_for_column(
    column_id: &ColumnId,
    hints: &SchemaInferenceHints,
) -> ColumnMiningRole {
    if hints
        .grade_columns
        .iter()
        .any(|candidate| candidate == column_id)
    {
        ColumnMiningRole::Grade
    } else if hints
        .tonnage_column
        .as_ref()
        .is_some_and(|candidate| candidate == column_id)
    {
        ColumnMiningRole::Tonnage
    } else if hints
        .bench_column
        .as_ref()
        .is_some_and(|candidate| candidate == column_id)
    {
        ColumnMiningRole::Bench
    } else if hints
        .phase_column
        .as_ref()
        .is_some_and(|candidate| candidate == column_id)
    {
        ColumnMiningRole::Phase
    } else if hints
        .domain_column
        .as_ref()
        .is_some_and(|candidate| candidate == column_id)
    {
        ColumnMiningRole::Domain
    } else {
        ColumnMiningRole::Other
    }
}

fn build_inference_warnings(
    inferred_columns: Vec<(ColumnId, ColumnLogicalType)>,
    hints: &SchemaInferenceHints,
) -> Vec<SchemaInferenceWarning> {
    let mut warnings = Vec::new();

    if hints.index_columns.is_none() {
        warnings.push(SchemaInferenceWarning {
            code: SchemaInferenceWarningCode::SpatialColumnsRequireConfirmation,
            column: None,
            message: "spatial columns must be confirmed explicitly; no index columns were provided"
                .to_owned(),
        });
    }

    if hints.tonnage_column.is_none()
        && let Some((column_id, _)) = inferred_columns.iter().find(|(column_id, logical_type)| {
            matches!(
                logical_type,
                ColumnLogicalType::Integer | ColumnLogicalType::Float
            ) && looks_like_tonnage(column_id.as_str())
        })
    {
        warnings.push(SchemaInferenceWarning {
            code: SchemaInferenceWarningCode::TonnageColumnRequiresConfirmation,
            column: Some(column_id.clone()),
            message: format!(
                "column `{column_id}` looks like tonnage and requires explicit confirmation"
            ),
        });
    }

    if hints.grade_columns.is_empty() {
        for (column_id, logical_type) in inferred_columns {
            if logical_type == ColumnLogicalType::Float {
                warnings.push(SchemaInferenceWarning {
                    code: SchemaInferenceWarningCode::GradeColumnRequiresConfirmation,
                    column: Some(column_id.clone()),
                    message: format!(
                        "float column `{column_id}` could be a grade or other critical metric; confirm it explicitly before relying on mining-role semantics"
                    ),
                });
            }
        }
    }

    warnings
}

fn looks_like_tonnage(column_name: &str) -> bool {
    let normalized = column_name.to_ascii_lowercase();
    ["tonnage", "tonnes", "tons", "tons_mined", "ton"]
        .into_iter()
        .any(|candidate| normalized.contains(candidate))
}

fn parse_boolean_like(value: &str) -> bool {
    value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false")
}

fn arrow_field_to_logical_type(data_type: &DataType) -> Result<ColumnLogicalType, MineError> {
    match data_type {
        DataType::Boolean => Ok(ColumnLogicalType::Boolean),
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => Ok(ColumnLogicalType::Integer),
        DataType::Float16 | DataType::Float32 | DataType::Float64 => Ok(ColumnLogicalType::Float),
        DataType::Utf8 | DataType::LargeUtf8 => Ok(ColumnLogicalType::Text),
        other => Err(MineError::schema(format!(
            "Arrow field type `{other:?}` is not supported by schema inference yet"
        ))),
    }
}
