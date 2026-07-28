use std::collections::{BTreeMap, BTreeSet};

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet, GridDefinition,
    MineError,
};
use mine_indexing::{ijk_to_linear, ijk_to_xyz, linear_to_ijk, xyz_to_ijk};
use serde::{Deserialize, Serialize};

use crate::internal::{AggregatedValue, AggregationBuffer, validate_subblock_grids};

/// Operaciones declarativas disponibles para subblocking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistributionOperation {
    /// Divide un valor conservativo de punto flotante en partes iguales entre los subbloques.
    SplitEqually {
        /// Columna fuente cuyo valor total se reparte entre hijos.
        column: ColumnId,
    },
    /// Replica el valor del bloque padre en cada subbloque.
    Replicate {
        /// Columna fuente cuyo valor se copia a cada hijo.
        column: ColumnId,
    },
}

/// Regla individual para producir una columna al subdividir bloques.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionRule {
    output_column: ColumnId,
    operation: DistributionOperation,
}

impl DistributionRule {
    /// Construye una regla que divide un valor conservativo en partes iguales.
    #[must_use]
    pub fn split_equally(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: DistributionOperation::SplitEqually { column },
        }
    }

    /// Construye una regla que replica el valor del bloque padre.
    #[must_use]
    pub fn replicate(output_column: ColumnId, column: ColumnId) -> Self {
        Self {
            output_column,
            operation: DistributionOperation::Replicate { column },
        }
    }

    /// Devuelve la columna de salida producida por la regla.
    #[must_use]
    pub const fn output_column(&self) -> &ColumnId {
        &self.output_column
    }

    /// Devuelve la operación declarada por la regla.
    #[must_use]
    pub const fn operation(&self) -> &DistributionOperation {
        &self.operation
    }
}

/// Conjunto declarativo de reglas que describen cómo distribuir atributos durante subblocking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionRules {
    rules: Vec<DistributionRule>,
}

impl DistributionRules {
    /// Construye un conjunto validando invariantes básicas del contrato.
    pub fn new(rules: Vec<DistributionRule>) -> Result<Self, MineError> {
        if rules.is_empty() {
            return Err(MineError::invalid_parameter(
                "rules",
                "distribution rules must contain at least one rule",
            ));
        }

        let mut output_columns = BTreeSet::new();
        for rule in &rules {
            if !output_columns.insert(rule.output_column().as_str().to_owned()) {
                return Err(MineError::invalid_parameter(
                    "rules",
                    format!(
                        "distribution output column `{}` is duplicated",
                        rule.output_column()
                    ),
                ));
            }
        }

        Ok(Self { rules })
    }

    /// Devuelve las reglas declaradas.
    #[must_use]
    pub fn rules(&self) -> &[DistributionRule] {
        &self.rules
    }

    /// Valida que todas las columnas requeridas existan y que cada operación sea segura.
    pub fn validate_against_schema(&self, schema: &ColumnSchemaSet) -> Result<(), MineError> {
        for rule in &self.rules {
            match rule.operation() {
                DistributionOperation::SplitEqually { column } => {
                    let logical_type = ensure_distribution_column_exists(schema, column, rule)?;
                    if logical_type != ColumnLogicalType::Float {
                        return Err(MineError::invalid_parameter(
                            "rules",
                            format!(
                                "split_equally distribution for output `{}` requires float column `{column}`, but found `{:?}`",
                                rule.output_column(),
                                logical_type
                            ),
                        ));
                    }
                    reject_intensive_split(schema, column, rule)?;
                }
                DistributionOperation::Replicate { column } => {
                    ensure_distribution_column_exists(schema, column, rule)?;
                    reject_conservative_replication(schema, column, rule)?;
                }
            }
        }

        Ok(())
    }
}

fn reject_intensive_split(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &DistributionRule,
) -> Result<(), MineError> {
    let source_schema = schema.get(column).ok_or_else(|| {
        MineError::schema(format!(
            "distribution rule for output `{}` requires column `{column}` but it is missing from the schema",
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
                "split_equally distribution for output `{}` cannot preserve the `{:?}` role of intensive column `{column}`; use replicate",
                rule.output_column(),
                source_schema.mining_role()
            ),
        ));
    }

    Ok(())
}

fn reject_conservative_replication(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &DistributionRule,
) -> Result<(), MineError> {
    let source_schema = schema.get(column).ok_or_else(|| {
        MineError::schema(format!(
            "distribution rule for output `{}` requires column `{column}` but it is missing from the schema",
            rule.output_column()
        ))
    })?;

    if source_schema.mining_role() == ColumnMiningRole::Tonnage {
        return Err(MineError::invalid_parameter(
            "rules",
            format!(
                "replicate distribution for output `{}` would duplicate conservative tonnage column `{column}`; use split_equally",
                rule.output_column()
            ),
        ));
    }

    Ok(())
}

/// Subdivide un `BlockModel` hacia una grilla más fina usando reglas explícitas.
pub fn subblock(
    model: &BlockModel,
    target_grid: GridDefinition,
    rules: &DistributionRules,
) -> Result<BlockModel, MineError> {
    const GRID_TOLERANCE: f64 = 1e-9;

    rules.validate_against_schema(model.schema())?;
    let children_per_parent = validate_subblock_grids(model.grid(), &target_grid, GRID_TOLERANCE)?;

    let mut source_rows_by_linear_index = BTreeMap::<usize, usize>::new();
    for row_index in 0..model.block_count() {
        source_rows_by_linear_index.insert(model.linear_index_at(row_index)?, row_index);
    }

    let mut descriptors = rules
        .rules()
        .iter()
        .map(|rule| build_distribution_descriptor(model.schema(), rule))
        .collect::<Result<Vec<_>, _>>()?;
    let mut materialized_linear_indices = Vec::new();

    for target_linear_index in 0..target_grid.shape().total_cells() {
        let target_index = linear_to_ijk(&target_grid, target_linear_index)?;
        let center = ijk_to_xyz(&target_grid, target_index)?;
        let source_index = xyz_to_ijk(model.grid(), center, GRID_TOLERANCE)?;
        let source_linear_index = ijk_to_linear(model.grid(), source_index)?;
        let Some(&source_row_index) = source_rows_by_linear_index.get(&source_linear_index) else {
            continue;
        };

        materialized_linear_indices.push(target_linear_index);
        for descriptor in &mut descriptors {
            let distributed_value = distribute_rule(
                model,
                source_row_index,
                children_per_parent,
                &descriptor.rule,
            )?;
            descriptor.buffer.push(distributed_value)?;
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

struct DistributionDescriptor {
    rule: DistributionRule,
    output_schema: ColumnSchema,
    buffer: AggregationBuffer,
}

fn build_distribution_descriptor(
    schema: &ColumnSchemaSet,
    rule: &DistributionRule,
) -> Result<DistributionDescriptor, MineError> {
    let output_schema = match rule.operation() {
        DistributionOperation::SplitEqually { column }
        | DistributionOperation::Replicate { column } => {
            let source_schema = schema.get(column).ok_or_else(|| {
                MineError::schema(format!(
                    "distribution rule for output `{}` requires column `{column}` but it is missing from the schema",
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
    };
    let buffer = AggregationBuffer::new(output_schema.logical_type())?;

    Ok(DistributionDescriptor {
        rule: rule.clone(),
        output_schema,
        buffer,
    })
}

fn distribute_rule(
    model: &BlockModel,
    source_row_index: usize,
    children_per_parent: usize,
    rule: &DistributionRule,
) -> Result<AggregatedValue, MineError> {
    match rule.operation() {
        DistributionOperation::SplitEqually { column } => {
            distribute_split_equally(model, column, source_row_index, children_per_parent)
        }
        DistributionOperation::Replicate { column } => {
            distribute_replicated(model, column, source_row_index)
        }
    }
}

fn distribute_split_equally(
    model: &BlockModel,
    column: &ColumnId,
    source_row_index: usize,
    children_per_parent: usize,
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "subblock distribution column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Floats(values) => {
            let value = *values.get(source_row_index).ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{source_row_index}` is outside column `{column}`"
                ))
            })?;
            if !value.is_finite() {
                return Err(MineError::numeric(
                    "subblock split_equally requires finite source values",
                ));
            }

            Ok(AggregatedValue::Float(value / children_per_parent as f64))
        }
        _ => Err(MineError::invalid_parameter(
            "rules",
            format!("split_equally distribution requires float column `{column}`"),
        )),
    }
}

fn distribute_replicated(
    model: &BlockModel,
    column: &ColumnId,
    source_row_index: usize,
) -> Result<AggregatedValue, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "subblock distribution column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Integers(values) => values
            .get(source_row_index)
            .copied()
            .map(AggregatedValue::Integer)
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{source_row_index}` is outside column `{column}`"
                ))
            }),
        ColumnData::Floats(values) => values
            .get(source_row_index)
            .copied()
            .map(AggregatedValue::Float)
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{source_row_index}` is outside column `{column}`"
                ))
            }),
        ColumnData::Booleans(values) => values
            .get(source_row_index)
            .copied()
            .map(AggregatedValue::Boolean)
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{source_row_index}` is outside column `{column}`"
                ))
            }),
        ColumnData::Texts(values) => values
            .get(source_row_index)
            .cloned()
            .map(AggregatedValue::Text)
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{source_row_index}` is outside column `{column}`"
                ))
            }),
    }
}

fn ensure_distribution_column_exists(
    schema: &ColumnSchemaSet,
    column: &ColumnId,
    rule: &DistributionRule,
) -> Result<ColumnLogicalType, MineError> {
    schema
        .get(column)
        .map(|column_schema| column_schema.logical_type())
        .ok_or_else(|| {
            MineError::schema(format!(
                "distribution rule for output `{}` requires column `{column}` but it is missing from the schema",
                rule.output_column()
            ))
        })
}
