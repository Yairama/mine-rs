//! Valorización multi-destino por bloque.
//!
//! Calcula el valor económico de un bloque para cada destino disponible,
//! aplicando las fórmulas NSR con los supuestos específicos del destino.
//! La selección del mejor destino queda explícita y auditaable.

use std::collections::BTreeMap;

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::{
    DestinationAssumptionSet, DestinationId,
    nsr::{NsrMetalInput, compute_nsr},
};

/// Valores de ley por metal para un bloque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockGrades {
    /// Tonelaje del bloque.
    pub tonnage: f64,
    /// Ley por metal (clave: nombre de columna, valor: ley).
    pub grades: BTreeMap<String, f64>,
}

impl BlockGrades {
    /// Construye los valores de un bloque.
    ///
    /// # Errores
    ///
    /// Retorna error si el tonelaje no es finito o es negativo.
    pub fn new(tonnage: f64, grades: BTreeMap<String, f64>) -> Result<Self, MineError> {
        if !tonnage.is_finite() || tonnage < 0.0 {
            return Err(MineError::invalid_parameter(
                "tonnage",
                "block tonnage must be finite and non-negative",
            ));
        }
        Ok(Self { tonnage, grades })
    }
}

/// Resultado de la valorización de un bloque en un destino específico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDestinationValue {
    /// Identificador del destino evaluado.
    pub destination_id: DestinationId,
    /// NSR por tonelada en este destino.
    pub nsr_per_tonne: f64,
    /// Costo de minado por tonelada incluido en el destino.
    pub mining_cost_per_tonne: f64,
    /// Costo downstream por tonelada posterior al minado.
    pub downstream_cost_per_tonne: f64,
    /// Costo total por tonelada (mining + processing).
    pub total_cost_per_tonne: f64,
    /// Margen por tonelada (NSR - costo).
    pub margin_per_tonne: f64,
    /// Valor total del bloque (margin × tonnage).
    pub block_value: f64,
}

/// Resultado de valorizar un bloque contra todos los destinos disponibles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiDestinationBlockValuation {
    /// Resultados por destino (ordenados por `destination_id`).
    pub by_destination: Vec<BlockDestinationValue>,
    /// Identificador del destino con mayor valor de bloque.
    pub best_destination_id: DestinationId,
    /// Valor máximo del bloque (en el mejor destino).
    pub max_block_value: f64,
}

impl MultiDestinationBlockValuation {
    /// Retorna el resultado para un destino específico, o `None` si no está en la evaluación.
    #[must_use]
    pub fn for_destination(&self, id: &DestinationId) -> Option<&BlockDestinationValue> {
        self.by_destination.iter().find(|d| &d.destination_id == id)
    }
}

/// Valoriza un bloque contra todos los destinos del conjunto de supuestos.
///
/// Para cada destino, calcula el NSR usando los precios, recoveries y payabilities
/// del destino, luego resta los costos para obtener el valor de bloque.
///
/// El mejor destino es el que maximiza el valor total (puede ser negativo).
///
/// # Errores
///
/// Retorna error si:
/// - el conjunto de destinos está vacío
/// - el tonelaje del bloque es inválido
/// - los metales declarados en los supuestos no tienen datos de ley en el bloque
pub fn value_block_by_destinations(
    block: &BlockGrades,
    destinations: &DestinationAssumptionSet,
) -> Result<MultiDestinationBlockValuation, MineError> {
    let all = destinations.destinations();
    if all.is_empty() {
        return Err(MineError::invalid_parameter(
            "destinations",
            "destination set must contain at least one destination",
        ));
    }

    let mut results: Vec<BlockDestinationValue> = Vec::with_capacity(all.len());

    for dest in all {
        let mut nsr_inputs: Vec<NsrMetalInput> = Vec::new();

        for recovery in dest.recoveries() {
            let metal_key = recovery.metal_column().as_str();
            let grade = *block.grades.get(metal_key).unwrap_or(&0.0);

            let payability = dest
                .payabilities()
                .iter()
                .find(|p| p.metal_column() == recovery.metal_column())
                .map(|p: &crate::DestinationPayability| p.payability_fraction())
                .unwrap_or(1.0);

            let price = dest
                .price_per_metal_unit()
                .get(metal_key)
                .copied()
                .ok_or_else(|| {
                    MineError::invalid_parameter(
                        "price_per_metal_unit",
                        format!(
                            "no price configured for metal `{metal_key}` in destination `{}`",
                            dest.id().as_str()
                        ),
                    )
                })?;

            nsr_inputs.push(NsrMetalInput {
                metal_column: recovery.metal_column().clone(),
                grade,
                recovery: recovery.recovery_fraction(),
                payability,
                price_per_unit: price,
                treatment_cost_per_unit: 0.0,
            });
        }

        let nsr = compute_nsr(&nsr_inputs)?;
        let nsr_per_tonne = nsr.total_nsr_per_tonne;
        let mining_cost = dest.mining_cost_per_tonne();
        let downstream_cost = dest.downstream_cost_per_tonne();
        let total_cost = mining_cost + downstream_cost;
        let margin = nsr_per_tonne - total_cost;
        let block_value = margin * block.tonnage;

        results.push(BlockDestinationValue {
            destination_id: dest.id().clone(),
            nsr_per_tonne,
            mining_cost_per_tonne: mining_cost,
            downstream_cost_per_tonne: downstream_cost,
            total_cost_per_tonne: total_cost,
            margin_per_tonne: margin,
            block_value,
        });
    }

    let best = results
        .iter()
        .max_by(|a, b| a.block_value.partial_cmp(&b.block_value).unwrap())
        .expect("results is non-empty");

    let best_destination_id = best.destination_id.clone();
    let max_block_value = best.block_value;

    results.sort_by(|a, b| a.destination_id.cmp(&b.destination_id));

    Ok(MultiDestinationBlockValuation {
        by_destination: results,
        best_destination_id,
        max_block_value,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::{ColumnId, MeasurementUnit};

    use crate::{
        DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
        DestinationKind, DestinationPayability, DestinationRecovery,
    };

    use super::*;

    fn mill_dest() -> DestinationAssumptions {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            2.0,
            8.0,
            vec![DestinationRecovery::new(cu.clone(), 0.88).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 0.97).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9000.0)]),
        )
        .expect("destination should be valid")
    }

    fn waste_dest() -> DestinationAssumptions {
        DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("destination should be valid")
    }

    fn block_with_grade(cu_grade: f64, tonnage: f64) -> BlockGrades {
        BlockGrades::new(tonnage, BTreeMap::from([("cu".to_owned(), cu_grade)]))
            .expect("block grades should be valid")
    }

    #[test]
    fn high_grade_block_selects_mill() {
        let dests = DestinationAssumptionSet::new(vec![mill_dest(), waste_dest()])
            .expect("set should be valid");

        let block = block_with_grade(1.0, 1000.0);
        let result = value_block_by_destinations(&block, &dests).expect("valuation should succeed");

        assert_eq!(result.best_destination_id.as_str(), "mill");
        // NSR_mill = 1.0 × 0.88 × 0.97 × 9000 = 7682.4, minus cost 10.0 = margin 7672.4 × 1000
        assert!(result.max_block_value > 0.0);
    }

    #[test]
    fn zero_grade_block_selects_waste() {
        let dests = DestinationAssumptionSet::new(vec![mill_dest(), waste_dest()])
            .expect("set should be valid");

        // Grade 0: mill NSR = 0, margin = -10.0 (cost). Waste margin = -2.5. Waste wins.
        let block = block_with_grade(0.0, 1000.0);
        let result = value_block_by_destinations(&block, &dests).expect("valuation should succeed");

        assert_eq!(result.best_destination_id.as_str(), "waste");
    }

    #[test]
    fn block_value_is_margin_times_tonnage() {
        let dests = DestinationAssumptionSet::new(vec![mill_dest()]).expect("set should be valid");

        let block = block_with_grade(0.5, 2000.0);
        let result = value_block_by_destinations(&block, &dests).expect("valuation should succeed");

        let mill_result = result
            .for_destination(&DestinationId::new("mill").expect("valid"))
            .expect("mill should be found");

        let expected_nsr = 0.5 * 0.88 * 0.97 * 9000.0;
        assert_eq!(mill_result.mining_cost_per_tonne, 2.0);
        assert_eq!(mill_result.downstream_cost_per_tonne, 8.0);
        assert_eq!(mill_result.total_cost_per_tonne, 10.0);
        let expected_margin = expected_nsr - 10.0;
        let expected_value = expected_margin * 2000.0;

        assert!((mill_result.block_value - expected_value).abs() < 1e-4);
    }

    #[test]
    fn empty_destination_set_returns_error() {
        let dests =
            DestinationAssumptionSet::new(vec![]).expect("empty set should be constructible");
        let block = block_with_grade(1.0, 1000.0);
        assert!(value_block_by_destinations(&block, &dests).is_err());
    }

    #[test]
    fn block_with_no_matching_grades_uses_zero_grade() {
        let dests = DestinationAssumptionSet::new(vec![mill_dest()]).expect("set should be valid");

        // Block has no "cu" grade — should default to 0.0, not error.
        let block = BlockGrades::new(500.0, BTreeMap::new()).expect("block should be valid");
        let result = value_block_by_destinations(&block, &dests).expect("valuation should succeed");

        let mill = result
            .for_destination(&DestinationId::new("mill").expect("valid"))
            .expect("mill should be found");

        assert_eq!(mill.nsr_per_tonne, 0.0);
    }
}
