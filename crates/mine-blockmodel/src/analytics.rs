use std::collections::BTreeMap;

use mine_core::{ColumnId, ColumnMiningRole, MeasurementUnit, MineError};
use serde::{Deserialize, Serialize};

use crate::{BlockModel, ColumnData};

/// Conteo de nulos por columna dentro del modelo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnNullCount {
    /// Nombre de la columna.
    pub name: ColumnId,
    /// Cantidad de valores nulos observados.
    pub null_count: usize,
}

/// Estadística ponderada para una columna de ley.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedGradeStatistic {
    /// Nombre de la columna de ley.
    pub name: ColumnId,
    /// Unidad declarada en el schema.
    pub unit: Option<MeasurementUnit>,
    /// Ley media ponderada por tonelaje cuando el denominador es positivo.
    pub average_grade: Option<f64>,
    /// Metal contenido cuando la unidad puede convertirse a fracción.
    pub contained_metal: Option<f64>,
}

/// Estadísticas básicas del modelo calculadas con una columna de tonelaje explícita.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicStatistics {
    /// Cantidad de bloques considerados.
    pub block_count: usize,
    /// Columna de tonelaje usada como peso.
    pub tonnage_column: ColumnId,
    /// Tonelaje total acumulado.
    pub total_tonnage: f64,
    /// Conteo de nulos por columna.
    pub null_counts: Vec<ColumnNullCount>,
    /// Estadísticas ponderadas de columnas de ley.
    pub grade_statistics: Vec<WeightedGradeStatistic>,
}

/// Estadísticas agregadas para un grupo categórico del modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupedStatistics {
    /// Columna usada para agrupar.
    pub group_by: ColumnId,
    /// Valor del grupo.
    pub group_value: String,
    /// Cantidad de bloques dentro del grupo.
    pub block_count: usize,
    /// Columna de tonelaje usada como peso.
    pub tonnage_column: ColumnId,
    /// Tonelaje total del grupo.
    pub total_tonnage: f64,
    /// Estadísticas ponderadas de columnas de ley dentro del grupo.
    pub grade_statistics: Vec<WeightedGradeStatistic>,
}

/// Punto de una curva ley-tonelaje calculada para un cutoff explícito.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeTonnagePoint {
    /// Ley de corte aplicada.
    pub cutoff: f64,
    /// Cantidad de bloques por encima del cutoff.
    pub block_count: usize,
    /// Tonelaje acumulado por encima del cutoff.
    pub tonnage: f64,
    /// Ley media ponderada por tonelaje para los bloques seleccionados.
    pub average_grade: Option<f64>,
    /// Metal contenido cuando la unidad puede convertirse a fracción.
    pub contained_metal: Option<f64>,
    /// Porcentaje de tonelaje retenido respecto del total del modelo.
    pub tonnage_percentage: Option<f64>,
}

impl BlockModel {
    /// Calcula estadísticas básicas usando una columna de tonelaje explícita.
    pub fn basic_statistics(
        &self,
        tonnage_column: &ColumnId,
    ) -> Result<BasicStatistics, MineError> {
        let indices = (0..self.block_count()).collect::<Vec<_>>();
        build_basic_statistics(self, tonnage_column, &indices)
    }

    /// Agrupa estadísticas por una columna categórica usando un tonelaje explícito.
    pub fn grouped_statistics(
        &self,
        group_by: &ColumnId,
        tonnage_column: &ColumnId,
    ) -> Result<Vec<GroupedStatistics>, MineError> {
        let group_values = categorical_group_values(self, group_by)?;
        let mut grouped_indices = BTreeMap::<String, Vec<usize>>::new();

        for (index, value) in group_values.into_iter().enumerate() {
            grouped_indices.entry(value).or_default().push(index);
        }

        let mut grouped_statistics = Vec::with_capacity(grouped_indices.len());

        for (group_value, indices) in grouped_indices {
            let statistics = build_basic_statistics(self, tonnage_column, &indices)?;

            grouped_statistics.push(GroupedStatistics {
                group_by: group_by.clone(),
                group_value,
                block_count: statistics.block_count,
                tonnage_column: statistics.tonnage_column,
                total_tonnage: statistics.total_tonnage,
                grade_statistics: statistics.grade_statistics,
            });
        }

        Ok(grouped_statistics)
    }

    /// Calcula una curva ley-tonelaje para una columna de ley y una columna de tonelaje.
    pub fn grade_tonnage_curve(
        &self,
        grade_column: &ColumnId,
        tonnage_column: &ColumnId,
        cutoffs: &[f64],
    ) -> Result<Vec<GradeTonnagePoint>, MineError> {
        let mut ordered_cutoffs = cutoffs.to_vec();

        if ordered_cutoffs.iter().any(|cutoff| !cutoff.is_finite()) {
            return Err(MineError::invalid_parameter(
                "cutoffs",
                "grade-tonnage cutoffs must be finite numeric values",
            ));
        }

        ordered_cutoffs.sort_by(f64::total_cmp);
        ordered_cutoffs.dedup_by(|left, right| left.total_cmp(right).is_eq());

        let grade_values = float_column(self, grade_column, "grade")?;
        let tonnage_values = float_column(self, tonnage_column, "tonnage")?;
        let total_tonnage = tonnage_values.iter().sum::<f64>();
        let metal_factor = self
            .schema
            .get(grade_column)
            .and_then(|column_schema| grade_to_fraction_factor(column_schema.unit()));
        let mut curve = Vec::with_capacity(ordered_cutoffs.len());

        for cutoff in ordered_cutoffs {
            let selected_indices = grade_values
                .iter()
                .enumerate()
                .filter_map(|(index, grade)| (*grade >= cutoff).then_some(index))
                .collect::<Vec<_>>();
            let tonnage = selected_indices
                .iter()
                .map(|index| tonnage_values[*index])
                .sum::<f64>();
            let weighted_grade_sum = selected_indices
                .iter()
                .map(|index| grade_values[*index] * tonnage_values[*index])
                .sum::<f64>();

            curve.push(GradeTonnagePoint {
                cutoff,
                block_count: selected_indices.len(),
                tonnage,
                average_grade: (tonnage > 0.0).then_some(weighted_grade_sum / tonnage),
                contained_metal: metal_factor.map(|factor| weighted_grade_sum * factor),
                tonnage_percentage: (total_tonnage > 0.0)
                    .then_some((tonnage / total_tonnage) * 100.0),
            });
        }

        Ok(curve)
    }
}

fn build_basic_statistics(
    model: &BlockModel,
    tonnage_column: &ColumnId,
    indices: &[usize],
) -> Result<BasicStatistics, MineError> {
    let tonnage_values = float_column(model, tonnage_column, "tonnage")?;
    let total_tonnage = indices.iter().map(|index| tonnage_values[*index]).sum();
    let mut grade_statistics = Vec::new();

    for (column_id, column_schema) in model.schema.iter() {
        if column_schema.mining_role() != ColumnMiningRole::Grade {
            continue;
        }

        let grade_values = float_column(model, column_id, "grade")?;
        let weighted_grade_sum = indices
            .iter()
            .map(|index| grade_values[*index] * tonnage_values[*index])
            .sum::<f64>();
        let average_grade = (total_tonnage > 0.0).then_some(weighted_grade_sum / total_tonnage);
        let contained_metal = grade_to_fraction_factor(column_schema.unit())
            .map(|factor| weighted_grade_sum * factor);

        grade_statistics.push(WeightedGradeStatistic {
            name: column_id.clone(),
            unit: column_schema.unit().cloned(),
            average_grade,
            contained_metal,
        });
    }

    let null_counts = model
        .schema
        .iter()
        .map(|(column_id, _)| ColumnNullCount {
            name: column_id.clone(),
            null_count: 0,
        })
        .collect();

    Ok(BasicStatistics {
        block_count: indices.len(),
        tonnage_column: tonnage_column.clone(),
        total_tonnage,
        null_counts,
        grade_statistics,
    })
}

fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
    purpose: &str,
) -> Result<&'a [f64], MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` does not exist in block model storage"
        )));
    };

    let ColumnData::Floats(values) = column_data else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` must be a float column"
        )));
    };

    Ok(values)
}

fn categorical_group_values(
    model: &BlockModel,
    group_by: &ColumnId,
) -> Result<Vec<String>, MineError> {
    let Some(column_data) = model.column(group_by) else {
        return Err(MineError::schema(format!(
            "group column `{group_by}` does not exist in block model storage"
        )));
    };

    match column_data {
        ColumnData::Texts(values) => Ok(values.clone()),
        ColumnData::Integers(values) => Ok(values.iter().map(ToString::to_string).collect()),
        ColumnData::Booleans(values) => Ok(values.iter().map(ToString::to_string).collect()),
        ColumnData::Floats(_) => Err(MineError::schema(format!(
            "group column `{group_by}` must be categorical (text, integer or boolean)"
        ))),
    }
}

fn grade_to_fraction_factor(unit: Option<&MeasurementUnit>) -> Option<f64> {
    let unit = unit?.as_str().to_ascii_lowercase();

    if unit.starts_with('%') {
        return Some(0.01);
    }

    if unit == "ppm" {
        return Some(1e-6);
    }

    None
}
