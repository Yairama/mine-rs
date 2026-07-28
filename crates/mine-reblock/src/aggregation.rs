use std::collections::{BTreeMap, BTreeSet};

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet, GridDefinition,
    MineError,
};
use mine_indexing::{ijk_to_linear, ijk_to_xyz, linear_to_ijk, xyz_to_ijk};
use serde::{Deserialize, Serialize};

use crate::internal::{
    AggregatedValue, AggregationBuffer, numeric_value_at, validate_superblock_grids,
};

/// Operación custom limitada para agregaciones numéricas declarativas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAggregationSpec {
    name: String,
    input_columns: Vec<ColumnId>,
    output_logical_type: ColumnLogicalType,
}

impl CustomAggregationSpec {
    /// Construye una especificación custom limitada.
    pub fn new(
        name: impl Into<String>,
        input_columns: Vec<ColumnId>,
        output_logical_type: ColumnLogicalType,
    ) -> Result<Self, MineError> {
        let name = name.into().trim().to_owned();

        if name.is_empty() {
            return Err(MineError::invalid_parameter(
                "name",
                "custom aggregation name must not be empty",
            ));
        }

        if input_columns.is_empty() {
            return Err(MineError::invalid_parameter(
                "input_columns",
                "custom aggregation must declare at least one input column",
            ));
        }

        Ok(Self {
            name,
            input_columns,
            output_logical_type,
        })
    }

    /// Devuelve el nombre lógico de la operación custom.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Devuelve las columnas requeridas por la operación.
    #[must_use]
    pub fn input_columns(&self) -> &[ColumnId] {
        &self.input_columns
    }

    /// Devuelve el tipo lógico esperado en la salida.
    #[must_use]
    pub const fn output_logical_type(&self) -> ColumnLogicalType {
        self.output_logical_type
    }
}

/// Operaciones declarativas disponibles para reblocking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationOperation {
    /// Suma una columna numérica.
    Sum {
        /// Columna fuente que se sumará.
        column: ColumnId,
    },
    /// Calcula promedio ponderado de una columna numérica.
    WeightedAverage {
        /// Columna de valores a promediar.
        value_column: ColumnId,
        /// Columna de pesos usada para el promedio.
        weight_column: ColumnId,
    },
    /// Devuelve el mínimo de una columna numérica.
    Minimum {
        /// Columna fuente sobre la que se calcula el mínimo.
        column: ColumnId,
    },
    /// Devuelve el máximo de una columna numérica.
    Maximum {
        /// Columna fuente sobre la que se calcula el máximo.
        column: ColumnId,
    },
    /// Devuelve el primer valor observado de una columna.
    First {
        /// Columna fuente cuyo primer valor se preserva.
        column: ColumnId,
    },
    /// Devuelve el valor más frecuente de una columna categórica.
    Majority {
        /// Columna categórica evaluada por mayoría.
        column: ColumnId,
    },
    /// Reserva una operación custom limitada para agregaciones numéricas futuras.
    CustomNumeric(CustomAggregationSpec),
}

/// Regla individual para producir una columna agregada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregationRule {
    output_column: ColumnId,
    operation: AggregationOperation,
}

impl AggregationRule {
    /// Construye una regla de suma.
    #[must_use]
    pub fn sum(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::Sum { column },
        }
    }

    /// Construye una regla de promedio ponderado.
    #[must_use]
    pub fn weighted_average(
        output_column: ColumnId,
        value_column: ColumnId,
        weight_column: ColumnId,
    ) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::WeightedAverage {
                value_column,
                weight_column,
            },
        }
    }

    /// Construye una regla de mínimo.
    #[must_use]
    pub fn minimum(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::Minimum { column },
        }
    }

    /// Construye una regla de máximo.
    #[must_use]
    pub fn maximum(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::Maximum { column },
        }
    }

    /// Construye una regla de primer valor.
    #[must_use]
    pub fn first(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::First { column },
        }
    }

    /// Construye una regla de mayoría.
    #[must_use]
    pub fn majority(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::Majority { column },
        }
    }

    /// Construye una regla custom numérica limitada.
    #[must_use]
    pub fn custom_numeric(output_column: ColumnId, spec: CustomAggregationSpec) -> Self {
        Self {
            output_column,
            operation: AggregationOperation::CustomNumeric(spec),
        }
    }

    /// Devuelve la columna de salida producida por la regla.
    #[must_use]
    pub const fn output_column(&self) -> &ColumnId {
        &self.output_column
    }

    /// Devuelve la operación declarada por la regla.
    #[must_use]
    pub const fn operation(&self) -> &AggregationOperation {
        &self.operation
    }
}

/// Conjunto declarativo de reglas que describen cómo agregar atributos durante reblocking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregationRules {
    rules: Vec<AggregationRule>,
}

impl AggregationRules {
    /// Construye un conjunto validando invariantes básicas del contrato.
    pub fn new(rules: Vec<AggregationRule>) -> Result<Self, MineError> {
        if rules.is_empty() {
            return Err(MineError::invalid_parameter(
                "rules",
                "aggregation rules must contain at least one rule",
            ));
        }

        let mut output_columns = BTreeSet::new();
        for rule in &rules {
            if !output_columns.insert(rule.output_column().as_str().to_owned()) {
                return Err(MineError::invalid_parameter(
                    "rules",
                    format!(
                        "aggregation output column `{}` is duplicated",
                        rule.output_column()
                    ),
                ));
            }
        }

        Ok(Self { rules })
    }

    /// Devuelve las reglas declaradas.
    #[must_use]
    pub fn rules(&self) -> &[AggregationRule] {
        &self.rules
    }

    /// Valida que todas las columnas requeridas existan y que cada operación sea segura.
    pub fn validate_against_schema(&self, schema: &ColumnSchemaSet) -> Result<(), MineError> {
        for rule in &self.rules {
            match rule.operation() {
                AggregationOperation::Sum { column } => {
                    ensure_numeric_column(schema, column, rule)?;
                    reject_intensive_sum(schema, column, rule)?;
                }
                AggregationOperation::Minimum { column } => {
                    ensure_numeric_column(schema, column, rule)?;
                    reject_tonnage_non_sum(schema, column, rule, "minimum")?;
                }
                AggregationOperation::Maximum { column } => {
                    ensure_numeric_column(schema, column, rule)?;
                    reject_tonnage_non_sum(schema, column, rule, "maximum")?;
                }
                AggregationOperation::WeightedAverage {
                    value_column,
                    weight_column,
                } => {
                    ensure_numeric_column(schema, value_column, rule)?;
                    ensure_numeric_column(schema, weight_column, rule)?;
                    reject_tonnage_non_sum(schema, value_column, rule, "weighted_average")?;
                }
                AggregationOperation::First { column } => {
                    ensure_column_exists(schema, column, rule)?;
                    reject_tonnage_non_sum(schema, column, rule, "first")?;
                }
                AggregationOperation::Majority { column } => {
                    let logical_type = ensure_column_exists(schema, column, rule)?;

                    if !matches!(
                        logical_type,
                        ColumnLogicalType::Boolean
                            | ColumnLogicalType::Integer
                            | ColumnLogicalType::Text
                    ) {
                        return Err(MineError::invalid_parameter(
                            "rules",
                            format!(
                                "majority aggregation for output `{}` requires boolean, integer or text input, but `{column}` is `{:?}`",
                                rule.output_column(),
                                logical_type
                            ),
                        ));
                    }
                }
                AggregationOperation::CustomNumeric(spec) => {
                    if !matches!(
                        spec.output_logical_type(),
                        ColumnLogicalType::Integer | ColumnLogicalType::Float
                    ) {
                        return Err(MineError::invalid_parameter(
                            "rules",
                            format!(
                                "custom aggregation `{}` must declare numeric output type",
                                spec.name()
                            ),
                        ));
                    }

                    for column in spec.input_columns() {
                        ensure_numeric_column(schema, column, rule)?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn reject_tonnage_non_sum(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &AggregationRule,
    operation: &str,
) -> Result<(), MineError> {
    let source_schema = schema.get(column).ok_or_else(|| {
        MineError::schema(format!(
            "aggregation rule for output `{}` requires column `{column}` but it is missing from the schema",
            rule.output_column()
        ))
    })?;

    if source_schema.mining_role() == ColumnMiningRole::Tonnage {
        return Err(MineError::invalid_parameter(
            "rules",
            format!(
                "{operation} aggregation for output `{}` cannot preserve conservative tonnage column `{column}`; use sum",
                rule.output_column()
            ),
        ));
    }

    Ok(())
}

fn reject_intensive_sum(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &AggregationRule,
) -> Result<(), MineError> {
    let source_schema = schema.get(column).ok_or_else(|| {
        MineError::schema(format!(
            "aggregation rule for output `{}` requires column `{column}` but it is missing from the schema",
            rule.output_column()
        ))
    })?;

    if matches!(
        source_schema.mining_role(),
        ColumnMiningRole::Grade | ColumnMiningRole::Density | ColumnMiningRole::Recovery
    ) {
        return Err(MineError::invalid_parameter(
            "rules",
            format!(
                "sum aggregation for output `{}` cannot preserve the `{:?}` role of intensive column `{column}`; use an explicit weighted average",
                rule.output_column(),
                source_schema.mining_role()
            ),
        ));
    }

    Ok(())
}

/// Resultado reusable de una agregación ponderada sobre variables continuas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedAggregation {
    /// Número total de filas inspeccionadas por la agregación.
    pub input_count: usize,
    /// Cantidad de filas omitidas por valores o pesos nulos.
    pub skipped_null_count: usize,
    /// Cantidad de filas no nulas consideradas por la agregación.
    pub contributing_count: usize,
    /// Suma total de pesos válidos.
    pub total_weight: f64,
    /// Suma ponderada acumulada.
    pub weighted_sum: f64,
    /// Promedio ponderado cuando el peso total es positivo.
    pub weighted_average: Option<f64>,
}

/// Agrega valores opcionales con pesos opcionales, omitiendo filas nulas.
pub fn aggregate_weighted_values(
    values: &[Option<f64>],
    weights: &[Option<f64>],
) -> Result<WeightedAggregation, MineError> {
    if values.len() != weights.len() {
        return Err(MineError::invalid_parameter(
            "weights",
            "weighted aggregation requires values and weights with the same length",
        ));
    }

    let mut skipped_null_count = 0_usize;
    let mut contributing_count = 0_usize;
    let mut total_weight = 0.0_f64;
    let mut weighted_sum = 0.0_f64;

    for (value, weight) in values.iter().zip(weights.iter()) {
        let (Some(value), Some(weight)) = (value, weight) else {
            skipped_null_count += 1;
            continue;
        };

        if !value.is_finite() {
            return Err(MineError::numeric(
                "weighted aggregation values must be finite",
            ));
        }

        if !weight.is_finite() || *weight < 0.0 {
            return Err(MineError::numeric(
                "weighted aggregation weights must be finite and greater than or equal to zero",
            ));
        }

        contributing_count += 1;
        total_weight += *weight;
        weighted_sum += *value * *weight;
    }

    Ok(WeightedAggregation {
        input_count: values.len(),
        skipped_null_count,
        contributing_count,
        total_weight,
        weighted_sum,
        weighted_average: (total_weight > 0.0).then_some(weighted_sum / total_weight),
    })
}

/// Agrega una columna numérica de `BlockModel` usando otra columna numérica como peso.
pub fn aggregate_weighted_column(
    model: &BlockModel,
    value_column: &ColumnId,
    weight_column: &ColumnId,
    row_indices: Option<&[usize]>,
) -> Result<WeightedAggregation, MineError> {
    let value_data = model.column(value_column).ok_or_else(|| {
        MineError::schema(format!(
            "weighted aggregation value column `{value_column}` does not exist in block model storage"
        ))
    })?;
    let weight_data = model.column(weight_column).ok_or_else(|| {
        MineError::schema(format!(
            "weighted aggregation weight column `{weight_column}` does not exist in block model storage"
        ))
    })?;
    let row_indices = row_indices
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| (0..model.block_count()).collect::<Vec<_>>());
    let mut values = Vec::with_capacity(row_indices.len());
    let mut weights = Vec::with_capacity(row_indices.len());

    for row_index in row_indices {
        values.push(numeric_value_at(value_data, row_index, value_column)?);
        weights.push(numeric_value_at(weight_data, row_index, weight_column)?);
    }

    aggregate_weighted_values(&values, &weights)
}

/// Reagrega un `BlockModel` sobre una grilla más gruesa usando reglas explícitas.
pub fn superblock(
    model: &BlockModel,
    target_grid: GridDefinition,
    rules: &AggregationRules,
) -> Result<BlockModel, MineError> {
    const GRID_TOLERANCE: f64 = 1e-9;

    rules.validate_against_schema(model.schema())?;
    validate_superblock_grids(model.grid(), &target_grid, GRID_TOLERANCE)?;

    let mut grouped_rows = BTreeMap::<usize, Vec<usize>>::new();
    for row_index in 0..model.block_count() {
        let source_linear_index = model.linear_index_at(row_index)?;
        let source_index = linear_to_ijk(model.grid(), source_linear_index)?;
        let center = ijk_to_xyz(model.grid(), source_index)?;
        let target_index = xyz_to_ijk(&target_grid, center, GRID_TOLERANCE)?;
        let target_linear_index = ijk_to_linear(&target_grid, target_index)?;
        grouped_rows
            .entry(target_linear_index)
            .or_default()
            .push(row_index);
    }

    let mut descriptors = rules
        .rules()
        .iter()
        .map(|rule| build_rule_descriptor(model.schema(), rule))
        .collect::<Result<Vec<_>, _>>()?;

    for row_indices in grouped_rows.values() {
        for descriptor in &mut descriptors {
            let aggregated_value = evaluate_rule(model, row_indices, &descriptor.rule)?;
            descriptor.buffer.push(aggregated_value)?;
        }
    }

    let output_schema = ColumnSchemaSet::from_columns(
        descriptors
            .iter()
            .map(|descriptor| descriptor.output_schema.clone())
            .collect::<Vec<_>>(),
    )?;
    let columns = descriptors
        .into_iter()
        .map(|descriptor| {
            (
                descriptor.rule.output_column().clone(),
                descriptor.buffer.finish(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let materialized_linear_indices = grouped_rows.keys().copied().collect::<Vec<_>>();

    if materialized_linear_indices.len() == target_grid.shape().total_cells() {
        BlockModel::new(
            target_grid,
            output_schema,
            model.metadata().clone(),
            columns,
        )
    } else {
        BlockModel::new_sparse(
            target_grid,
            output_schema,
            model.metadata().clone(),
            materialized_linear_indices,
            columns,
        )
    }
}

struct RuleDescriptor {
    rule: AggregationRule,
    output_schema: ColumnSchema,
    buffer: AggregationBuffer,
}

fn build_rule_descriptor(
    schema: &ColumnSchemaSet,
    rule: &AggregationRule,
) -> Result<RuleDescriptor, MineError> {
    let output_schema = match rule.operation() {
        AggregationOperation::Sum { column }
        | AggregationOperation::Minimum { column }
        | AggregationOperation::Maximum { column }
        | AggregationOperation::First { column }
        | AggregationOperation::Majority { column } => {
            let source_schema = schema.get(column).ok_or_else(|| {
                MineError::schema(format!(
                    "aggregation rule for output `{}` requires column `{column}` but it is missing from the schema",
                    rule.output_column()
                ))
            })?;

            ColumnSchema::new(
                rule.output_column().clone(),
                source_schema.logical_type(),
                source_schema.unit().cloned(),
                false,
                source_schema.mining_role(),
            )
        }
        AggregationOperation::WeightedAverage { value_column, .. } => {
            let source_schema = schema.get(value_column).ok_or_else(|| {
                MineError::schema(format!(
                    "aggregation rule for output `{}` requires column `{value_column}` but it is missing from the schema",
                    rule.output_column()
                ))
            })?;

            ColumnSchema::new(
                rule.output_column().clone(),
                ColumnLogicalType::Float,
                source_schema.unit().cloned(),
                false,
                source_schema.mining_role(),
            )
        }
        AggregationOperation::CustomNumeric(spec) => {
            return Err(MineError::invalid_parameter(
                "rules",
                format!(
                    "superblock does not execute custom aggregation `{}` yet",
                    spec.name()
                ),
            ));
        }
    };
    let buffer = AggregationBuffer::new(output_schema.logical_type())?;

    Ok(RuleDescriptor {
        rule: rule.clone(),
        output_schema,
        buffer,
    })
}

fn evaluate_rule(
    model: &BlockModel,
    row_indices: &[usize],
    rule: &AggregationRule,
) -> Result<AggregatedValue, MineError> {
    match rule.operation() {
        AggregationOperation::Sum { column } => aggregate_sum(model, column, row_indices),
        AggregationOperation::WeightedAverage {
            value_column,
            weight_column,
        } => {
            let aggregation =
                aggregate_weighted_column(model, value_column, weight_column, Some(row_indices))?;

            aggregation
                .weighted_average
                .map(AggregatedValue::Float)
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "weighted average for output `{}` has zero total weight",
                        rule.output_column()
                    ))
                })
        }
        AggregationOperation::Minimum { column } => aggregate_minimum(model, column, row_indices),
        AggregationOperation::Maximum { column } => aggregate_maximum(model, column, row_indices),
        AggregationOperation::First { column } => aggregate_first(model, column, row_indices),
        AggregationOperation::Majority { column } => aggregate_majority(model, column, row_indices),
        AggregationOperation::CustomNumeric(spec) => Err(MineError::invalid_parameter(
            "rules",
            format!(
                "superblock does not execute custom aggregation `{}` yet",
                spec.name()
            ),
        )),
    }
}

fn aggregate_sum(
    model: &BlockModel,
    column: &ColumnId,
    row_indices: &[usize],
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "superblock aggregation column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Integers(values) => Ok(AggregatedValue::Integer(
            row_indices.iter().map(|index| values[*index]).sum(),
        )),
        ColumnData::Floats(values) => Ok(AggregatedValue::Float(
            row_indices.iter().map(|index| values[*index]).sum(),
        )),
        _ => Err(MineError::invalid_parameter(
            "rules",
            format!("sum aggregation requires numeric column `{column}`"),
        )),
    }
}

fn aggregate_minimum(
    model: &BlockModel,
    column: &ColumnId,
    row_indices: &[usize],
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "superblock aggregation column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Integers(values) => Ok(AggregatedValue::Integer(
            row_indices
                .iter()
                .map(|index| values[*index])
                .min()
                .ok_or_else(|| MineError::validation("superblock group must not be empty"))?,
        )),
        ColumnData::Floats(values) => Ok(AggregatedValue::Float(
            row_indices
                .iter()
                .map(|index| values[*index])
                .min_by(f64::total_cmp)
                .ok_or_else(|| MineError::validation("superblock group must not be empty"))?,
        )),
        _ => Err(MineError::invalid_parameter(
            "rules",
            format!("minimum aggregation requires numeric column `{column}`"),
        )),
    }
}

fn aggregate_maximum(
    model: &BlockModel,
    column: &ColumnId,
    row_indices: &[usize],
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "superblock aggregation column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Integers(values) => Ok(AggregatedValue::Integer(
            row_indices
                .iter()
                .map(|index| values[*index])
                .max()
                .ok_or_else(|| MineError::validation("superblock group must not be empty"))?,
        )),
        ColumnData::Floats(values) => Ok(AggregatedValue::Float(
            row_indices
                .iter()
                .map(|index| values[*index])
                .max_by(f64::total_cmp)
                .ok_or_else(|| MineError::validation("superblock group must not be empty"))?,
        )),
        _ => Err(MineError::invalid_parameter(
            "rules",
            format!("maximum aggregation requires numeric column `{column}`"),
        )),
    }
}

fn aggregate_first(
    model: &BlockModel,
    column: &ColumnId,
    row_indices: &[usize],
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "superblock aggregation column `{column}` does not exist in block model storage"
        ))
    })?;
    let first_index = *row_indices
        .first()
        .ok_or_else(|| MineError::validation("superblock group must not be empty"))?;

    match column_data {
        ColumnData::Integers(values) => Ok(AggregatedValue::Integer(values[first_index])),
        ColumnData::Floats(values) => Ok(AggregatedValue::Float(values[first_index])),
        ColumnData::Booleans(values) => Ok(AggregatedValue::Boolean(values[first_index])),
        ColumnData::Texts(values) => Ok(AggregatedValue::Text(values[first_index].clone())),
    }
}

fn aggregate_majority(
    model: &BlockModel,
    column: &ColumnId,
    row_indices: &[usize],
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "superblock aggregation column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Booleans(values) => {
            let mut counts = BTreeMap::<bool, usize>::new();
            for row_index in row_indices {
                *counts.entry(values[*row_index]).or_insert(0) += 1;
            }
            Ok(AggregatedValue::Boolean(select_majority(counts)?))
        }
        ColumnData::Integers(values) => {
            let mut counts = BTreeMap::<i64, usize>::new();
            for row_index in row_indices {
                *counts.entry(values[*row_index]).or_insert(0) += 1;
            }
            Ok(AggregatedValue::Integer(select_majority(counts)?))
        }
        ColumnData::Texts(values) => {
            let mut counts = BTreeMap::<String, usize>::new();
            for row_index in row_indices {
                *counts.entry(values[*row_index].clone()).or_insert(0) += 1;
            }
            Ok(AggregatedValue::Text(select_majority(counts)?))
        }
        _ => Err(MineError::invalid_parameter(
            "rules",
            format!("majority aggregation requires categorical column `{column}`"),
        )),
    }
}

fn select_majority<T: Ord>(counts: BTreeMap<T, usize>) -> Result<T, MineError> {
    counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        .map(|(value, _)| value)
        .ok_or_else(|| MineError::validation("superblock group must not be empty"))
}

fn ensure_column_exists(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &AggregationRule,
) -> Result<ColumnLogicalType, MineError> {
    schema
        .get(column)
        .map(|column_schema| column_schema.logical_type())
        .ok_or_else(|| {
            MineError::schema(format!(
                "aggregation rule for output `{}` requires column `{column}` but it is missing from the schema",
                rule.output_column()
            ))
        })
}

fn ensure_numeric_column(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &AggregationRule,
) -> Result<(), MineError> {
    let logical_type = ensure_column_exists(schema, column, rule)?;

    if matches!(
        logical_type,
        ColumnLogicalType::Integer | ColumnLogicalType::Float
    ) {
        Ok(())
    } else {
        Err(MineError::invalid_parameter(
            "rules",
            format!(
                "aggregation rule for output `{}` requires numeric column `{column}`, but found `{:?}`",
                rule.output_column(),
                logical_type
            ),
        ))
    }
}
