//! Fórmulas explícitas de Net Smelter Return (NSR) y Equivalent Value (EV).
//!
//! Estas fórmulas calculan el valor neto por tonelada de mineral basándose en
//! leyes, precios, recoveries, payabilities y costos de tratamiento. El resultado
//! es reproducible, serializable y auditaable: no hay defaults ocultos.
//!
//! # Fórmulas principales
//!
//! Para un metal `i` con ley `g_i`, precio `p_i`, recovery `r_i`,
//! payability `f_i` y costo de tratamiento `tc_i` (por unidad de metal):
//!
//! ```text
//! NSR_i = g_i × r_i × f_i × (p_i - tc_i)
//! NSR_total = Σ NSR_i
//! ```
//!
//! El Equivalent Value (EV) normaliza el NSR contra un metal de referencia:
//!
//! ```text
//! EV = NSR_total / (r_ref × f_ref × (p_ref - tc_ref))
//! ```
//!
//! Esto produce una "ley equivalente" en unidades del metal de referencia.

use std::collections::BTreeMap;

use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

/// Insumos por metal para el cálculo de NSR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NsrMetalInput {
    /// Identificador de la columna de ley en el block model.
    pub metal_column: ColumnId,
    /// Ley del bloque (en la unidad especificada por el supuesto económico).
    pub grade: f64,
    /// Recovery metalúrgica (fracción 0–1).
    pub recovery: f64,
    /// Payability (fracción 0–1 del precio de mercado efectivamente cobrada).
    pub payability: f64,
    /// Precio de mercado por unidad de metal recuperado.
    pub price_per_unit: f64,
    /// Costo de tratamiento por unidad de metal recuperado (refining charges, etc.).
    pub treatment_cost_per_unit: f64,
}

/// Resultado del cálculo NSR para un bloque y un conjunto de metales.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NsrResult {
    /// NSR individual por metal (keyed por `metal_column.as_str()`).
    pub nsr_by_metal: BTreeMap<String, f64>,
    /// NSR total por tonelada (suma de contributions individuales).
    pub total_nsr_per_tonne: f64,
}

impl NsrResult {
    /// Retorna el NSR de un metal específico, o `None` si no existe.
    #[must_use]
    pub fn nsr_for(&self, metal: &ColumnId) -> Option<f64> {
        self.nsr_by_metal.get(metal.as_str()).copied()
    }
}

/// Parámetros para el Equivalent Value (EV) respecto a un metal de referencia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvParameters {
    /// Metal de referencia para normalizar el NSR total.
    pub reference_metal: ColumnId,
    /// Recovery del metal de referencia (fracción 0–1).
    pub reference_recovery: f64,
    /// Payability del metal de referencia (fracción 0–1).
    pub reference_payability: f64,
    /// Precio del metal de referencia por unidad.
    pub reference_price_per_unit: f64,
    /// Costo de tratamiento del metal de referencia por unidad.
    pub reference_treatment_cost_per_unit: f64,
}

/// Resultado del cálculo de Equivalent Value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvResult {
    /// NSR total utilizado como numerador.
    pub total_nsr_per_tonne: f64,
    /// Factor de normalización del metal de referencia (denominador).
    pub reference_factor: f64,
    /// Equivalent value en unidades del metal de referencia.
    pub equivalent_value: f64,
}

/// Calcula el NSR por tonelada para un bloque dado un conjunto de insumos por metal.
///
/// # Errores
///
/// Retorna error si:
/// - alguna ley, precio o recovery es no finita
/// - recovery o payability están fuera de [0.0, 1.0]
/// - el precio de mercado es negativo o cero
pub fn compute_nsr(inputs: &[NsrMetalInput]) -> Result<NsrResult, MineError> {
    let mut nsr_by_metal = BTreeMap::new();
    let mut total = 0.0_f64;

    for input in inputs {
        validate_nsr_input(input)?;
        let nsr_i = input.grade
            * input.recovery
            * input.payability
            * (input.price_per_unit - input.treatment_cost_per_unit);
        nsr_by_metal.insert(input.metal_column.as_str().to_owned(), nsr_i);
        total += nsr_i;
    }

    Ok(NsrResult {
        nsr_by_metal,
        total_nsr_per_tonne: total,
    })
}

/// Calcula el Equivalent Value (EV) en unidades del metal de referencia.
///
/// `nsr_total` es el NSR total por tonelada (puede provenir de `compute_nsr`).
///
/// # Errores
///
/// Retorna error si:
/// - el factor de referencia es cero o negativo (división inválida)
/// - los parámetros del metal de referencia son inválidos
pub fn compute_ev(nsr_total: f64, params: &EvParameters) -> Result<EvResult, MineError> {
    if !nsr_total.is_finite() {
        return Err(MineError::invalid_parameter(
            "nsr_total",
            "NSR total must be finite",
        ));
    }
    if !params.reference_recovery.is_finite()
        || !(0.0..=1.0).contains(&params.reference_recovery)
    {
        return Err(MineError::invalid_parameter(
            "reference_recovery",
            "reference recovery must be finite and between 0.0 and 1.0",
        ));
    }
    if !params.reference_payability.is_finite()
        || !(0.0..=1.0).contains(&params.reference_payability)
    {
        return Err(MineError::invalid_parameter(
            "reference_payability",
            "reference payability must be finite and between 0.0 and 1.0",
        ));
    }
    if !params.reference_price_per_unit.is_finite() || params.reference_price_per_unit <= 0.0 {
        return Err(MineError::invalid_parameter(
            "reference_price_per_unit",
            "reference price must be finite and positive",
        ));
    }
    if !params.reference_treatment_cost_per_unit.is_finite() {
        return Err(MineError::invalid_parameter(
            "reference_treatment_cost_per_unit",
            "reference treatment cost must be finite",
        ));
    }

    let reference_factor = params.reference_recovery
        * params.reference_payability
        * (params.reference_price_per_unit - params.reference_treatment_cost_per_unit);

    if reference_factor <= 0.0 {
        return Err(MineError::invalid_parameter(
            "reference_factor",
            "reference factor (recovery × payability × net_price) must be positive",
        ));
    }

    let equivalent_value = nsr_total / reference_factor;

    Ok(EvResult {
        total_nsr_per_tonne: nsr_total,
        reference_factor,
        equivalent_value,
    })
}

fn validate_nsr_input(input: &NsrMetalInput) -> Result<(), MineError> {
    if !input.grade.is_finite() {
        return Err(MineError::invalid_parameter(
            "grade",
            "grade must be finite",
        ));
    }
    if !input.recovery.is_finite() || !(0.0..=1.0).contains(&input.recovery) {
        return Err(MineError::invalid_parameter(
            "recovery",
            "recovery must be finite and between 0.0 and 1.0",
        ));
    }
    if !input.payability.is_finite() || !(0.0..=1.0).contains(&input.payability) {
        return Err(MineError::invalid_parameter(
            "payability",
            "payability must be finite and between 0.0 and 1.0",
        ));
    }
    if !input.price_per_unit.is_finite() || input.price_per_unit <= 0.0 {
        return Err(MineError::invalid_parameter(
            "price_per_unit",
            "price per unit must be finite and positive",
        ));
    }
    if !input.treatment_cost_per_unit.is_finite() {
        return Err(MineError::invalid_parameter(
            "treatment_cost_per_unit",
            "treatment cost per unit must be finite",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mine_core::ColumnId;

    use super::*;

    fn cu_input(grade: f64) -> NsrMetalInput {
        NsrMetalInput {
            metal_column: ColumnId::new("cu").expect("column id should be valid"),
            grade,
            recovery: 0.88,
            payability: 0.97,
            price_per_unit: 9000.0,
            treatment_cost_per_unit: 150.0,
        }
    }

    fn au_input(grade: f64) -> NsrMetalInput {
        NsrMetalInput {
            metal_column: ColumnId::new("au").expect("column id should be valid"),
            grade,
            recovery: 0.75,
            payability: 0.995,
            price_per_unit: 60000.0,
            treatment_cost_per_unit: 0.0,
        }
    }

    #[test]
    fn nsr_single_metal_computes_correctly() {
        // NSR_cu = 1.0 × 0.88 × 0.97 × (9000 - 150)
        //        = 0.88 × 0.97 × 8850
        //        = 7556.04
        let inputs = vec![cu_input(1.0)];
        let result = compute_nsr(&inputs).expect("nsr should succeed");
        let expected = 0.88 * 0.97 * (9000.0 - 150.0);
        assert!(
            (result.total_nsr_per_tonne - expected).abs() < 1e-6,
            "expected {expected}, got {}",
            result.total_nsr_per_tonne
        );
    }

    #[test]
    fn nsr_zero_grade_gives_zero_contribution() {
        let inputs = vec![cu_input(0.0)];
        let result = compute_nsr(&inputs).expect("nsr should succeed");
        assert_eq!(result.total_nsr_per_tonne, 0.0);
    }

    #[test]
    fn nsr_polymetallic_sums_contributions() {
        let inputs = vec![cu_input(0.5), au_input(0.0003)];
        let result = compute_nsr(&inputs).expect("nsr should succeed");

        let cu = 0.5 * 0.88 * 0.97 * (9000.0 - 150.0);
        let au = 0.0003 * 0.75 * 0.995 * 60000.0;
        let expected = cu + au;

        assert!(
            (result.total_nsr_per_tonne - expected).abs() < 1e-6,
            "expected {expected}, got {}",
            result.total_nsr_per_tonne
        );
        assert!(result.nsr_by_metal.contains_key("cu"));
        assert!(result.nsr_by_metal.contains_key("au"));
    }

    #[test]
    fn nsr_rejects_invalid_recovery() {
        let mut bad = cu_input(1.0);
        bad.recovery = 1.5;
        assert!(compute_nsr(&[bad]).is_err());
    }

    #[test]
    fn nsr_rejects_non_finite_grade() {
        let mut bad = cu_input(f64::NAN);
        assert!(compute_nsr(&[bad.clone()]).is_err());
        bad.grade = f64::INFINITY;
        assert!(compute_nsr(&[bad]).is_err());
    }

    #[test]
    fn nsr_rejects_zero_price() {
        let mut bad = cu_input(1.0);
        bad.price_per_unit = 0.0;
        assert!(compute_nsr(&[bad]).is_err());
    }

    #[test]
    fn ev_computes_correctly() {
        let inputs = vec![cu_input(0.5), au_input(0.0003)];
        let nsr = compute_nsr(&inputs).expect("nsr should succeed");

        let params = EvParameters {
            reference_metal: ColumnId::new("cu").expect("column id should be valid"),
            reference_recovery: 0.88,
            reference_payability: 0.97,
            reference_price_per_unit: 9000.0,
            reference_treatment_cost_per_unit: 150.0,
        };

        let ev = compute_ev(nsr.total_nsr_per_tonne, &params).expect("ev should succeed");

        let reference_factor = 0.88 * 0.97 * (9000.0 - 150.0);
        let expected_ev = nsr.total_nsr_per_tonne / reference_factor;

        assert!(
            (ev.equivalent_value - expected_ev).abs() < 1e-9,
            "expected {expected_ev}, got {}",
            ev.equivalent_value
        );
        assert!((ev.reference_factor - reference_factor).abs() < 1e-9);
    }

    #[test]
    fn ev_rejects_zero_reference_factor() {
        // Si el precio neto del metal de referencia es 0, el denominador es 0
        let params = EvParameters {
            reference_metal: ColumnId::new("cu").expect("column id should be valid"),
            reference_recovery: 0.88,
            reference_payability: 0.97,
            reference_price_per_unit: 150.0, // igual que treatment_cost → net = 0
            reference_treatment_cost_per_unit: 150.0,
        };
        assert!(compute_ev(1000.0, &params).is_err());
    }

    #[test]
    fn nsr_empty_inputs_returns_zero() {
        let result = compute_nsr(&[]).expect("empty nsr should succeed");
        assert_eq!(result.total_nsr_per_tonne, 0.0);
        assert!(result.nsr_by_metal.is_empty());
    }
}
