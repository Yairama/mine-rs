//! Artefacto integrado `EconomicBlockModel`.
//!
//! Combina un `BlockModel` con supuestos de destinos y los valores económicos
//! derivados por bloque. Sirve como input estándar para pit, scheduling y
//! evaluación económica.
//!
//! El artefacto preserva:
//! - El block model original con todas sus columnas.
//! - Los supuestos de destinos usados para la valuación.
//! - El mejor destino y el valor de bloque para cada bloque materializado.
//! - Las leyes usadas para el cálculo (por metal, por bloque).

use std::collections::BTreeMap;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, ColumnLogicalType, MineError};
use serde::{Deserialize, Serialize};

use crate::{
    DestinationAssumptionSet, DestinationId,
    block_valuation::{BlockGrades, value_block_by_destinations},
};

/// Configuración para construir un `EconomicBlockModel`.
#[derive(Debug, Clone)]
pub struct EconomicBlockModelConfig {
    /// Columna que contiene el tonelaje por bloque.
    pub tonnage_column: ColumnId,
    /// Columnas de ley por metal para la valuación (clave: nombre de columna de ley).
    pub grade_columns: Vec<ColumnId>,
    /// Supuestos de destinos.
    pub destinations: DestinationAssumptionSet,
}

/// Resultado económico derivado para un bloque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockEconomicSummary {
    /// Índice lineal del bloque en el grid.
    pub linear_index: usize,
    /// Tonelaje del bloque.
    pub tonnage: f64,
    /// Mejor destino para este bloque.
    pub best_destination_id: DestinationId,
    /// Valor de bloque en el mejor destino.
    pub block_value: f64,
    /// NSR por tonelada en el mejor destino.
    pub nsr_per_tonne: f64,
    /// Margen por tonelada en el mejor destino.
    pub margin_per_tonne: f64,
}

/// Artefacto integrado que combina block model, supuestos y valuación económica.
///
/// El artefacto no modifica el block model subyacente; los resultados económicos
/// se almacenan separadamente y son accesibles por índice lineal.
#[derive(Debug, Clone)]
pub struct EconomicBlockModel {
    model: BlockModel,
    destinations: DestinationAssumptionSet,
    block_summaries: Vec<BlockEconomicSummary>,
    tonnage_column: ColumnId,
    grade_columns: Vec<ColumnId>,
}

impl EconomicBlockModel {
    /// Construye el `EconomicBlockModel` valuando cada bloque materializado.
    ///
    /// Para cada bloque, extrae el tonelaje y las leyes de las columnas especificadas,
    /// evalúa todos los destinos y registra el mejor.
    ///
    /// # Errores
    ///
    /// Retorna error si:
    /// - la columna de tonelaje no existe o no es de tipo float
    /// - alguna columna de ley no existe o no es de tipo float
    /// - la valuación falla (destinos vacíos, precios inválidos, etc.)
    pub fn build(
        model: BlockModel,
        config: EconomicBlockModelConfig,
    ) -> Result<Self, MineError> {
        validate_float_column(&model, &config.tonnage_column, "tonnage_column")?;
        for grade_col in &config.grade_columns {
            validate_float_column(&model, grade_col, "grade_column")?;
        }

        let tonnage_data = get_float_column(&model, &config.tonnage_column)?;
        let mut grade_data: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for grade_col in &config.grade_columns {
            let data = get_float_column(&model, grade_col)?;
            grade_data.insert(grade_col.as_str().to_owned(), data);
        }

        let n_blocks = tonnage_data.len();
        let mut block_summaries = Vec::with_capacity(n_blocks);

        for (pos, &tonnage) in tonnage_data.iter().enumerate() {
            let linear_index = model.linear_index_at(pos)
                .expect("position must be within materialized bounds");

            let mut grades = BTreeMap::new();
            for (metal_key, data) in &grade_data {
                grades.insert(metal_key.clone(), data[pos]);
            }

            let block = BlockGrades::new(tonnage.max(0.0), grades)?;
            let valuation = value_block_by_destinations(&block, &config.destinations)?;

            let best = valuation
                .by_destination
                .iter()
                .find(|d| d.destination_id == valuation.best_destination_id)
                .expect("best_destination_id must be in by_destination");

            block_summaries.push(BlockEconomicSummary {
                linear_index,
                tonnage,
                best_destination_id: best.destination_id.clone(),
                block_value: best.block_value,
                nsr_per_tonne: best.nsr_per_tonne,
                margin_per_tonne: best.margin_per_tonne,
            });
        }

        Ok(Self {
            model,
            destinations: config.destinations,
            block_summaries,
            tonnage_column: config.tonnage_column,
            grade_columns: config.grade_columns,
        })
    }

    /// Referencia al block model subyacente.
    #[must_use]
    pub fn model(&self) -> &BlockModel {
        &self.model
    }

    /// Supuestos de destinos usados para la valuación.
    #[must_use]
    pub fn destinations(&self) -> &DestinationAssumptionSet {
        &self.destinations
    }

    /// Resultados económicos para todos los bloques materializados.
    #[must_use]
    pub fn block_summaries(&self) -> &[BlockEconomicSummary] {
        &self.block_summaries
    }

    /// Resultado económico de un bloque por posición en el array materializado.
    #[must_use]
    pub fn summary_at(&self, pos: usize) -> Option<&BlockEconomicSummary> {
        self.block_summaries.get(pos)
    }

    /// Columna de tonelaje usada para la valuación.
    #[must_use]
    pub fn tonnage_column(&self) -> &ColumnId {
        &self.tonnage_column
    }

    /// Columnas de ley usadas para la valuación.
    #[must_use]
    pub fn grade_columns(&self) -> &[ColumnId] {
        &self.grade_columns
    }

    /// Número de bloques materializados.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.block_summaries.len()
    }

    /// Valor total de todos los bloques (suma de `block_value`).
    #[must_use]
    pub fn total_block_value(&self) -> f64 {
        self.block_summaries.iter().map(|s| s.block_value).sum()
    }

    /// Número de bloques con valor positivo (profitable).
    #[must_use]
    pub fn profitable_block_count(&self) -> usize {
        self.block_summaries
            .iter()
            .filter(|s| s.block_value > 0.0)
            .count()
    }
}

fn validate_float_column(
    model: &BlockModel,
    col: &ColumnId,
    param_name: &'static str,
) -> Result<(), MineError> {
    match model.column(col) {
        None => Err(MineError::invalid_parameter(
            param_name,
            format!("column `{col}` not found in block model"),
        )),
        Some(data) if data.logical_type() != ColumnLogicalType::Float => {
            Err(MineError::invalid_parameter(
                param_name,
                format!("column `{col}` must be of type Float"),
            ))
        }
        _ => Ok(()),
    }
}

fn get_float_column(model: &BlockModel, col: &ColumnId) -> Result<Vec<f64>, MineError> {
    match model.column(col) {
        Some(ColumnData::Floats(v)) => Ok(v.clone()),
        _ => Err(MineError::invalid_parameter(
            "column",
            format!("column `{col}` not found or not Float"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_blockmodel::{BlockModel, ColumnData};
    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
    };

    use crate::{
        DestinationAssumptions, DestinationAssumptionSet, DestinationCapacity, DestinationId,
        DestinationKind, DestinationPayability, DestinationRecovery,
    };

    use super::*;

    fn small_grid() -> GridDefinition {
        let origin = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let shape = GridShape::new(2, 2, 1).expect("shape should be valid");
        GridDefinition::new(origin, dims, shape, None).expect("grid should be valid")
    }

    fn small_model(cu_grades: Vec<f64>, tonnages: Vec<f64>) -> BlockModel {
        let grid = small_grid();
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let ton = ColumnId::new("ton").expect("column id should be valid");
        let unit_pct = MeasurementUnit::new("%").expect("% is valid");
        let unit_t = MeasurementUnit::new("t").expect("t is valid");

        let schema = ColumnSchemaSet::from_columns(vec![
            ColumnSchema::new(cu.clone(), ColumnLogicalType::Float, Some(unit_pct), false, ColumnMiningRole::Grade),
            ColumnSchema::new(ton.clone(), ColumnLogicalType::Float, Some(unit_t), false, ColumnMiningRole::Tonnage),
        ])
        .expect("schema should be valid");

        let mut cols = BTreeMap::new();
        cols.insert(cu, ColumnData::Floats(cu_grades));
        cols.insert(ton, ColumnData::Floats(tonnages));

        BlockModel::new(grid, schema, Metadata::new(), cols).expect("model should be valid")
    }

    fn two_destination_set() -> DestinationAssumptionSet {
        let cu = ColumnId::new("cu").expect("column id should be valid");

        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            2.0,
            8.0,
            vec![DestinationRecovery::new(cu.clone(), 0.88).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 0.97).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid")).expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9000.0)]),
        )
        .expect("mill should be valid");

        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid")).expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");

        DestinationAssumptionSet::new(vec![mill, waste]).expect("set should be valid")
    }

    #[test]
    fn economic_block_model_builds_with_correct_count() {
        let model = small_model(vec![0.5, 0.0, 1.2, 0.0], vec![1000.0; 4]);
        let config = EconomicBlockModelConfig {
            tonnage_column: ColumnId::new("ton").expect("valid"),
            grade_columns: vec![ColumnId::new("cu").expect("valid")],
            destinations: two_destination_set(),
        };

        let ebm = EconomicBlockModel::build(model, config).expect("ebm should build");
        assert_eq!(ebm.block_count(), 4);
    }

    #[test]
    fn high_grade_blocks_select_mill() {
        // Grades: 0.5, 0.0, 1.2, 0.0
        // High-grade blocks (0.5, 1.2) should select mill; zero-grade should select waste
        let model = small_model(vec![0.5, 0.0, 1.2, 0.0], vec![1000.0; 4]);
        let config = EconomicBlockModelConfig {
            tonnage_column: ColumnId::new("ton").expect("valid"),
            grade_columns: vec![ColumnId::new("cu").expect("valid")],
            destinations: two_destination_set(),
        };

        let ebm = EconomicBlockModel::build(model, config).expect("ebm should build");

        let s0 = ebm.summary_at(0).expect("block 0 should exist");
        let s1 = ebm.summary_at(1).expect("block 1 should exist");
        let s2 = ebm.summary_at(2).expect("block 2 should exist");

        assert_eq!(s0.best_destination_id.as_str(), "mill");
        assert_eq!(s1.best_destination_id.as_str(), "waste"); // zero grade
        assert_eq!(s2.best_destination_id.as_str(), "mill");
    }

    #[test]
    fn total_block_value_equals_sum() {
        let model = small_model(vec![0.5, 0.2, 1.2, 0.1], vec![1000.0; 4]);
        let config = EconomicBlockModelConfig {
            tonnage_column: ColumnId::new("ton").expect("valid"),
            grade_columns: vec![ColumnId::new("cu").expect("valid")],
            destinations: two_destination_set(),
        };

        let ebm = EconomicBlockModel::build(model, config).expect("ebm should build");

        let manual_sum: f64 = ebm.block_summaries().iter().map(|s| s.block_value).sum();
        assert!((ebm.total_block_value() - manual_sum).abs() < 1e-9);
    }

    #[test]
    fn rejects_missing_tonnage_column() {
        let model = small_model(vec![0.5, 0.5, 0.5, 0.5], vec![1000.0; 4]);
        let config = EconomicBlockModelConfig {
            tonnage_column: ColumnId::new("missing_col").expect("valid"),
            grade_columns: vec![ColumnId::new("cu").expect("valid")],
            destinations: two_destination_set(),
        };

        assert!(EconomicBlockModel::build(model, config).is_err());
    }
}
