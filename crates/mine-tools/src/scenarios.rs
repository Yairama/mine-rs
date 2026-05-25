use std::collections::{BTreeMap, BTreeSet};

use mine_sdk::{
    BenchParameters, Metadata, MiningScenario, ModelId, PeriodCashflowInput,
    ScenarioCashflowReport, ScenarioConstraints, ScenarioId, ScenarioPeriod,
    ScenarioPeriodCashflow, ScenarioRules, evaluate_scenario_cashflow,
};
use serde::{Deserialize, Serialize};

use crate::contract::{ToolDescriptor, ToolResponse};

pub(crate) const CREATE_SCENARIO_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "create_scenario",
    description: "Construye un escenario minero validado con periodos, reglas y restricciones.",
    input_version: "1",
    output_version: "1",
};

pub(crate) const EVALUATE_SCENARIO_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "evaluate_scenario",
    description: "Calcula cashflow y NPV de un escenario con inputs financieros explícitos.",
    input_version: "1",
    output_version: "1",
};

pub(crate) const COMPARE_SCENARIOS_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "compare_scenarios",
    description: "Compara dos evaluaciones de escenario y resume diferencias de cashflow y NPV.",
    input_version: "1",
    output_version: "1",
};

/// Periodo declarativo para construir un escenario desde `create_scenario`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateScenarioPeriodInput {
    /// Etiqueta legible del periodo.
    pub label: String,
    /// Target opcional de tonelaje.
    pub target_tonnage: Option<f64>,
    /// Target opcional de bloques.
    pub target_blocks: Option<usize>,
}

/// Entrada para `create_scenario`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateScenarioInput {
    /// Identificador estable del escenario.
    pub scenario_id: ScenarioId,
    /// Referencia al modelo base del escenario.
    pub model_id: ModelId,
    /// Periodos discretos del escenario.
    pub periods: Vec<CreateScenarioPeriodInput>,
    /// Columna categórica opcional usada como fase base.
    pub phase_column: Option<mine_sdk::ColumnId>,
    /// Parámetros opcionales para discretización por bancos.
    pub bench_parameters: Option<BenchParameters>,
    /// Restricción opcional de avance vertical máximo.
    pub max_vertical_advance: Option<f64>,
    /// Restricción opcional de máximo de fases activas.
    pub max_active_phases: Option<usize>,
    /// Supuestos serializables asociados al escenario.
    #[serde(default = "Metadata::new")]
    pub assumptions: Metadata,
}

/// Salida de `create_scenario`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateScenarioOutput {
    /// Escenario validado y serializable resultante.
    pub scenario: MiningScenario,
}

/// Entrada para `evaluate_scenario`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluateScenarioInput {
    /// Escenario previamente construido y validado.
    pub scenario: MiningScenario,
    /// Revenue y costo explícitos por periodo del escenario.
    pub period_inputs: Vec<PeriodCashflowInput>,
    /// Tasa de descuento por periodo.
    pub discount_rate_per_period: f64,
}

/// Salida de `evaluate_scenario`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluateScenarioOutput {
    /// Reporte financiero resultante.
    pub report: ScenarioCashflowReport,
}

/// Entrada para `compare_scenarios`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareScenariosInput {
    /// Evaluación base.
    pub base: ScenarioCashflowReport,
    /// Evaluación candidata.
    pub candidate: ScenarioCashflowReport,
}

/// Comparación por periodo entre dos escenarios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPeriodComparison {
    /// Etiqueta del periodo comparado.
    pub period_label: String,
    /// Cashflow base cuando existe el periodo.
    pub base_cashflow: Option<f64>,
    /// Cashflow candidato cuando existe el periodo.
    pub candidate_cashflow: Option<f64>,
    /// Delta de cashflow cuando ambos periodos existen.
    pub cashflow_delta: Option<f64>,
    /// Cashflow descontado base cuando existe el periodo.
    pub base_discounted_cashflow: Option<f64>,
    /// Cashflow descontado candidato cuando existe el periodo.
    pub candidate_discounted_cashflow: Option<f64>,
    /// Delta descontado cuando ambos periodos existen.
    pub discounted_cashflow_delta: Option<f64>,
}

/// Salida de `compare_scenarios`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareScenariosOutput {
    /// Escenario base comparado.
    pub base_scenario_id: String,
    /// Escenario candidato comparado.
    pub candidate_scenario_id: String,
    /// NPV del escenario base.
    pub base_npv: f64,
    /// NPV del escenario candidato.
    pub candidate_npv: f64,
    /// Delta `candidate - base` del NPV.
    pub npv_delta: f64,
    /// Cashflow total del escenario base.
    pub base_total_cashflow: f64,
    /// Cashflow total del escenario candidato.
    pub candidate_total_cashflow: f64,
    /// Delta `candidate - base` del cashflow total.
    pub total_cashflow_delta: f64,
    /// Identificador del escenario preferido por NPV cuando hay diferencia.
    pub preferred_scenario_id: Option<String>,
    /// Comparación por periodo.
    pub period_comparisons: Vec<ScenarioPeriodComparison>,
}

/// Construye un escenario minero validado a partir de reglas y periodos explícitos.
#[must_use]
pub fn create_scenario(input: &CreateScenarioInput) -> ToolResponse<CreateScenarioOutput> {
    let periods = match input
        .periods
        .iter()
        .map(|period| {
            ScenarioPeriod::new(
                period.label.clone(),
                period.target_tonnage,
                period.target_blocks,
            )
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(periods) => periods,
        Err(error) => return ToolResponse::failure(CREATE_SCENARIO_DESCRIPTOR, error),
    };
    let constraints =
        match ScenarioConstraints::new(input.max_vertical_advance, input.max_active_phases) {
            Ok(constraints) => constraints,
            Err(error) => return ToolResponse::failure(CREATE_SCENARIO_DESCRIPTOR, error),
        };

    match MiningScenario::new(
        input.scenario_id.clone(),
        input.model_id.clone(),
        periods,
        ScenarioRules::new(input.phase_column.clone(), input.bench_parameters.clone()),
        constraints,
        input.assumptions.clone(),
    ) {
        Ok(scenario) => ToolResponse::success(
            CREATE_SCENARIO_DESCRIPTOR,
            CreateScenarioOutput { scenario },
        ),
        Err(error) => ToolResponse::failure(CREATE_SCENARIO_DESCRIPTOR, error),
    }
}

/// Evalúa cashflow y NPV de un escenario con supuestos por periodo explícitos.
#[must_use]
pub fn evaluate_scenario(input: &EvaluateScenarioInput) -> ToolResponse<EvaluateScenarioOutput> {
    match evaluate_scenario_cashflow(
        &input.scenario,
        &input.period_inputs,
        input.discount_rate_per_period,
    ) {
        Ok(report) => ToolResponse::success(
            EVALUATE_SCENARIO_DESCRIPTOR,
            EvaluateScenarioOutput { report },
        ),
        Err(error) => ToolResponse::failure(EVALUATE_SCENARIO_DESCRIPTOR, error),
    }
}

/// Compara dos evaluaciones de escenario ya calculadas.
#[must_use]
pub fn compare_scenarios(input: &CompareScenariosInput) -> ToolResponse<CompareScenariosOutput> {
    ToolResponse::success(
        COMPARE_SCENARIOS_DESCRIPTOR,
        CompareScenariosOutput {
            base_scenario_id: input.base.scenario_id.clone(),
            candidate_scenario_id: input.candidate.scenario_id.clone(),
            base_npv: input.base.npv,
            candidate_npv: input.candidate.npv,
            npv_delta: input.candidate.npv - input.base.npv,
            base_total_cashflow: input.base.total_cashflow,
            candidate_total_cashflow: input.candidate.total_cashflow,
            total_cashflow_delta: input.candidate.total_cashflow - input.base.total_cashflow,
            preferred_scenario_id: preferred_scenario_id(&input.base, &input.candidate),
            period_comparisons: build_period_comparisons(
                &input.base.periods,
                &input.candidate.periods,
            ),
        },
    )
}

fn preferred_scenario_id(
    base: &ScenarioCashflowReport,
    candidate: &ScenarioCashflowReport,
) -> Option<String> {
    if candidate.npv > base.npv {
        Some(candidate.scenario_id.clone())
    } else if candidate.npv < base.npv {
        Some(base.scenario_id.clone())
    } else {
        None
    }
}

fn build_period_comparisons(
    base_periods: &[ScenarioPeriodCashflow],
    candidate_periods: &[ScenarioPeriodCashflow],
) -> Vec<ScenarioPeriodComparison> {
    let base_by_label = base_periods
        .iter()
        .map(|period| (period.period_label.as_str(), period))
        .collect::<BTreeMap<_, _>>();
    let candidate_by_label = candidate_periods
        .iter()
        .map(|period| (period.period_label.as_str(), period))
        .collect::<BTreeMap<_, _>>();
    let period_labels = base_by_label
        .keys()
        .chain(candidate_by_label.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    period_labels
        .into_iter()
        .map(|period_label| {
            let base = base_by_label.get(period_label).copied();
            let candidate = candidate_by_label.get(period_label).copied();

            ScenarioPeriodComparison {
                period_label: period_label.to_owned(),
                base_cashflow: base.map(|period| period.cashflow),
                candidate_cashflow: candidate.map(|period| period.cashflow),
                cashflow_delta: match (base, candidate) {
                    (Some(base), Some(candidate)) => Some(candidate.cashflow - base.cashflow),
                    _ => None,
                },
                base_discounted_cashflow: base.map(|period| period.discounted_cashflow),
                candidate_discounted_cashflow: candidate.map(|period| period.discounted_cashflow),
                discounted_cashflow_delta: match (base, candidate) {
                    (Some(base), Some(candidate)) => {
                        Some(candidate.discounted_cashflow - base.discounted_cashflow)
                    }
                    _ => None,
                },
            }
        })
        .collect()
}
