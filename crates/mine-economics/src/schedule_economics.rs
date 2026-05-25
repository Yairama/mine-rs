//! Evaluación económica determinista de `LongTermSchedule`.

use std::collections::{BTreeMap, BTreeSet};

use mine_blockmodel::ColumnData;
use mine_core::{Metadata, MineError, ModelId, ScenarioId};
use mine_planning::{
    LongTermSchedule, LongTermSchedulePeriodCapacity, MiningScenario, PushbackPlan,
    ScenarioConstraints, ScenarioPeriod, ScenarioRules, build_aggregated_long_term_schedule,
};
use serde::{Deserialize, Serialize};

use crate::{
    DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationRecovery,
    EconomicBlockModel, EconomicBlockModelConfig, PeriodCashflowInput, evaluate_scenario_cashflow,
};

#[derive(Debug, Clone)]
struct PhaseEconomicSummary {
    total_tonnage: f64,
    revenue: f64,
    cost: f64,
    destination_tonnage: BTreeMap<String, f64>,
    payable_metal: BTreeMap<String, f64>,
}

/// KPIs económicos agregados para un periodo del schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermSchedulePeriodEconomics {
    /// Etiqueta del periodo.
    pub period_label: String,
    /// Fases activas en el periodo.
    pub phase_ids: Vec<String>,
    /// Tonelaje asignado en el periodo.
    pub tonnage: f64,
    /// Cantidad agregada de bloques representados.
    pub block_count: usize,
    /// Revenue del periodo.
    pub revenue: f64,
    /// Costo del periodo.
    pub cost: f64,
    /// Cashflow no descontado.
    pub cashflow: f64,
    /// Factor de descuento aplicado.
    pub discount_factor: f64,
    /// Cashflow descontado.
    pub discounted_cashflow: f64,
    /// Tonelaje por destino económico seleccionado.
    pub destination_tonnage: BTreeMap<String, f64>,
    /// Metal pagable agregado por columna de ley.
    pub payable_metal: BTreeMap<String, f64>,
}

/// Reporte económico agregado de un `LongTermSchedule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleEconomicsReport {
    /// Identificador del escenario evaluado.
    pub scenario_id: String,
    /// Identificador del modelo evaluado.
    pub model_id: String,
    /// KPIs detallados por periodo.
    pub periods: Vec<LongTermSchedulePeriodEconomics>,
    /// Revenue total.
    pub total_revenue: f64,
    /// Costo total.
    pub total_cost: f64,
    /// Cashflow total sin descuento.
    pub total_cashflow: f64,
    /// Valor presente neto.
    pub npv: f64,
    /// Tasa de descuento por periodo.
    pub discount_rate_per_period: f64,
    /// Tonelaje total programado.
    pub total_tonnage: f64,
    /// Bloques totales representados.
    pub total_block_count: usize,
    /// Tonelaje agregado por destino.
    pub destination_tonnage: BTreeMap<String, f64>,
    /// Metal pagable agregado por columna.
    pub payable_metal: BTreeMap<String, f64>,
}

/// Resumen de riesgo para una métrica económica escalar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskMetricSummary {
    /// Cantidad de escenarios muestreados.
    pub sample_count: usize,
    /// Valor mínimo observado.
    pub min: f64,
    /// Valor máximo observado.
    pub max: f64,
    /// Valor promedio.
    pub mean: f64,
    /// Percentil 10 usando nearest-rank.
    pub p10: f64,
    /// Percentil 50 usando nearest-rank.
    pub p50: f64,
    /// Percentil 90 usando nearest-rank.
    pub p90: f64,
    /// Probabilidad empírica de downside (`value < 0`).
    pub downside_probability: f64,
    /// Conditional Value-at-Risk del 10% peor tramo.
    pub cvar10: f64,
}

/// Resumen de riesgo agregado para un conjunto de escenarios económicos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRiskReport {
    /// Identificadores de los escenarios muestreados.
    pub scenario_ids: Vec<String>,
    /// Método explícito usado para los cuantiles.
    pub quantile_method: String,
    /// Riesgo sobre NPV.
    pub npv: RiskMetricSummary,
    /// Riesgo sobre cashflow total.
    pub total_cashflow: RiskMetricSummary,
    /// Riesgo sobre tonelaje total programado.
    pub total_tonnage: RiskMetricSummary,
}

/// Evalúa KPIs económicos reproducibles para un `LongTermSchedule`.
///
/// La evaluación usa `PushbackPlan` para recuperar los bloques incluidos en cada fase y
/// prorratea revenue, costo y metal pagable según la fracción de tonelaje que cada periodo
/// asigna de la fase correspondiente.
pub fn evaluate_long_term_schedule_economics(
    schedule: &LongTermSchedule,
    phase_plan: &PushbackPlan,
    economic_model: &EconomicBlockModel,
    discount_rate_per_period: f64,
) -> Result<LongTermScheduleEconomicsReport, MineError> {
    let phase_summaries = build_phase_economic_summaries(phase_plan, economic_model)?;
    let scenario = schedule_to_scenario(schedule)?;

    let mut scheduled_phase_tonnage = BTreeMap::<String, f64>::new();
    let mut period_inputs = Vec::with_capacity(schedule.capacities().len());
    let mut period_economics = Vec::with_capacity(schedule.capacities().len());
    let mut total_tonnage = 0.0;
    let mut total_block_count = 0usize;
    let mut total_destination_tonnage = BTreeMap::<String, f64>::new();
    let mut total_payable_metal = BTreeMap::<String, f64>::new();

    for capacity in schedule.capacities() {
        let period_entries = schedule
            .entries()
            .iter()
            .filter(|entry| entry.period_label() == capacity.period_label())
            .collect::<Vec<_>>();

        let mut phase_ids = BTreeSet::new();
        let mut period_tonnage = 0.0;
        let mut period_block_count = 0usize;
        let mut period_revenue = 0.0;
        let mut period_cost = 0.0;
        let mut period_destination_tonnage = BTreeMap::<String, f64>::new();
        let mut period_payable_metal = BTreeMap::<String, f64>::new();

        for entry in period_entries {
            let phase_id = entry.phase_id().ok_or_else(|| MineError::Economics {
                message: format!(
                    "long-term schedule entry in period `{}` requires a phase_id for economic evaluation",
                    entry.period_label()
                ),
            })?;
            let phase_summary = phase_summaries.get(phase_id).ok_or_else(|| MineError::Economics {
                message: format!(
                    "schedule phase `{phase_id}` is missing from the pushback plan used for economic evaluation"
                ),
            })?;

            let already_scheduled = scheduled_phase_tonnage
                .get(phase_id)
                .copied()
                .unwrap_or(0.0);
            let updated_scheduled = already_scheduled + entry.tonnage();
            let tonnage_tolerance = phase_summary.total_tonnage.abs().max(1.0) * 1.0e-12;
            if updated_scheduled > phase_summary.total_tonnage + tonnage_tolerance {
                return Err(MineError::Economics {
                    message: format!(
                        "schedule phase `{phase_id}` assigns {} t but the phase only contains {} t",
                        updated_scheduled, phase_summary.total_tonnage
                    ),
                });
            }

            let share = if phase_summary.total_tonnage <= 0.0 {
                0.0
            } else {
                entry.tonnage() / phase_summary.total_tonnage
            };

            phase_ids.insert(phase_id.to_owned());
            period_tonnage += entry.tonnage();
            period_block_count += entry.block_count();
            period_revenue += phase_summary.revenue * share;
            period_cost += phase_summary.cost * share;

            accumulate_scaled_map(
                &mut period_destination_tonnage,
                &phase_summary.destination_tonnage,
                share,
            );
            accumulate_scaled_map(
                &mut period_payable_metal,
                &phase_summary.payable_metal,
                share,
            );
            scheduled_phase_tonnage.insert(phase_id.to_owned(), updated_scheduled);
        }

        total_tonnage += period_tonnage;
        total_block_count += period_block_count;
        accumulate_scaled_map(
            &mut total_destination_tonnage,
            &period_destination_tonnage,
            1.0,
        );
        accumulate_scaled_map(&mut total_payable_metal, &period_payable_metal, 1.0);

        period_inputs.push(PeriodCashflowInput::new(
            capacity.period_label(),
            period_revenue,
            period_cost,
        )?);
        period_economics.push((
            capacity.period_label().to_owned(),
            phase_ids.into_iter().collect::<Vec<_>>(),
            period_tonnage,
            period_block_count,
            period_revenue,
            period_cost,
            period_destination_tonnage,
            period_payable_metal,
        ));
    }

    let cashflow = evaluate_scenario_cashflow(&scenario, &period_inputs, discount_rate_per_period)?;
    let cashflow_by_period = cashflow
        .periods
        .iter()
        .map(|period| (period.period_label.clone(), period))
        .collect::<BTreeMap<_, _>>();

    let periods = period_economics
        .into_iter()
        .map(
            |(
                period_label,
                phase_ids,
                tonnage,
                block_count,
                revenue,
                cost,
                destination_tonnage,
                payable_metal,
            )| {
                let period_cashflow =
                    cashflow_by_period
                        .get(&period_label)
                        .copied()
                        .ok_or_else(|| MineError::Economics {
                            message: format!(
                                "cashflow output is missing schedule period `{period_label}`"
                            ),
                        })?;

                Ok(LongTermSchedulePeriodEconomics {
                    period_label,
                    phase_ids,
                    tonnage,
                    block_count,
                    revenue,
                    cost,
                    cashflow: period_cashflow.cashflow,
                    discount_factor: period_cashflow.discount_factor,
                    discounted_cashflow: period_cashflow.discounted_cashflow,
                    destination_tonnage,
                    payable_metal,
                })
            },
        )
        .collect::<Result<Vec<_>, MineError>>()?;

    Ok(LongTermScheduleEconomicsReport {
        scenario_id: schedule.scenario_id().to_string(),
        model_id: schedule.model_id().to_string(),
        periods,
        total_revenue: cashflow.total_revenue,
        total_cost: cashflow.total_cost,
        total_cashflow: cashflow.total_cashflow,
        npv: cashflow.npv,
        discount_rate_per_period: cashflow.discount_rate_per_period,
        total_tonnage,
        total_block_count,
        destination_tonnage: total_destination_tonnage,
        payable_metal: total_payable_metal,
    })
}

/// Resume riesgo económico sobre múltiples reportes de schedule.
pub fn summarize_long_term_schedule_risk(
    reports: &[LongTermScheduleEconomicsReport],
) -> Result<ScenarioRiskReport, MineError> {
    if reports.is_empty() {
        return Err(MineError::invalid_parameter(
            "reports",
            "risk summary requires at least one economic report",
        ));
    }

    Ok(ScenarioRiskReport {
        scenario_ids: reports
            .iter()
            .map(|report| report.scenario_id.clone())
            .collect(),
        quantile_method: "nearest-rank".to_owned(),
        npv: summarize_risk_metric(&reports.iter().map(|report| report.npv).collect::<Vec<_>>())?,
        total_cashflow: summarize_risk_metric(
            &reports
                .iter()
                .map(|report| report.total_cashflow)
                .collect::<Vec<_>>(),
        )?,
        total_tonnage: summarize_risk_metric(
            &reports
                .iter()
                .map(|report| report.total_tonnage)
                .collect::<Vec<_>>(),
        )?,
    })
}

fn build_phase_economic_summaries(
    phase_plan: &PushbackPlan,
    economic_model: &EconomicBlockModel,
) -> Result<BTreeMap<String, PhaseEconomicSummary>, MineError> {
    let summary_by_linear_index = economic_model
        .block_summaries()
        .iter()
        .map(|summary| (summary.linear_index, summary))
        .collect::<BTreeMap<_, _>>();
    let row_by_linear_index = build_row_index_map(economic_model)?;
    let grade_columns = economic_model
        .grade_columns()
        .iter()
        .map(|column| {
            let data = match economic_model.model().column(column) {
                Some(ColumnData::Floats(values)) => Ok(values.as_slice()),
                _ => Err(MineError::Economics {
                    message: format!(
                        "economic grade column `{}` is missing or not Float in the block model",
                        column.as_str()
                    ),
                }),
            }?;
            Ok((column.as_str().to_owned(), data))
        })
        .collect::<Result<BTreeMap<_, _>, MineError>>()?;

    phase_plan
        .phases
        .iter()
        .map(|phase| {
            let mut total_tonnage = 0.0;
            let mut revenue = 0.0;
            let mut cost = 0.0;
            let mut destination_tonnage = BTreeMap::<String, f64>::new();
            let mut payable_metal = BTreeMap::<String, f64>::new();

            for &linear_index in &phase.block_indices {
                let summary = summary_by_linear_index.get(&linear_index).copied().ok_or_else(|| {
                    MineError::Economics {
                        message: format!(
                            "phase `{}` references block `{linear_index}` that is missing from the economic block model",
                            phase.phase_id
                        ),
                    }
                })?;
                let destination = economic_model
                    .destinations()
                    .get(&summary.best_destination_id)
                    .ok_or_else(|| MineError::Economics {
                        message: format!(
                            "destination `{}` is missing from the economic assumptions",
                            summary.best_destination_id.as_str()
                        ),
                    })?;
                let row_index =
                    row_by_linear_index
                        .get(&linear_index)
                        .copied()
                        .ok_or_else(|| MineError::Economics {
                            message: format!(
                                "block `{linear_index}` cannot be mapped back to a materialized row"
                            ),
                        })?;

                total_tonnage += summary.tonnage;
                revenue += summary.nsr_per_tonne * summary.tonnage;
                cost += (summary.nsr_per_tonne - summary.margin_per_tonne) * summary.tonnage;
                *destination_tonnage
                    .entry(summary.best_destination_id.as_str().to_owned())
                    .or_insert(0.0) += summary.tonnage;

                for recovery in destination.recoveries() {
                    let metal_key = recovery.metal_column().as_str();
                    let payability = destination
                        .payabilities()
                        .iter()
                        .find(|payability| payability.metal_column() == recovery.metal_column())
                        .map(|payability| payability.payability_fraction())
                        .unwrap_or(1.0);
                    let grades = grade_columns.get(metal_key).ok_or_else(|| MineError::Economics {
                        message: format!(
                            "grade column `{metal_key}` required by destination `{}` is missing from the economic block model",
                            destination.id().as_str()
                        ),
                    })?;
                    let grade = grades.get(row_index).copied().ok_or_else(|| MineError::Economics {
                        message: format!(
                            "grade column `{metal_key}` is missing row `{row_index}` required for block `{linear_index}`"
                        ),
                    })?;

                    *payable_metal.entry(metal_key.to_owned()).or_insert(0.0) +=
                        summary.tonnage * grade * recovery.recovery_fraction() * payability;
                }
            }

            Ok((
                phase.phase_id.clone(),
                PhaseEconomicSummary {
                    total_tonnage,
                    revenue,
                    cost,
                    destination_tonnage,
                    payable_metal,
                },
            ))
        })
        .collect()
}

fn build_row_index_map(
    economic_model: &EconomicBlockModel,
) -> Result<BTreeMap<usize, usize>, MineError> {
    let mut row_by_linear_index = BTreeMap::new();
    for row_index in 0..economic_model.model().block_count() {
        let linear_index = economic_model.model().linear_index_at(row_index)?;
        row_by_linear_index.insert(linear_index, row_index);
    }
    Ok(row_by_linear_index)
}

fn schedule_to_scenario(schedule: &LongTermSchedule) -> Result<MiningScenario, MineError> {
    let periods = schedule
        .capacities()
        .iter()
        .map(|capacity| {
            ScenarioPeriod::new(capacity.period_label(), capacity.max_mine_tonnage(), None)
        })
        .collect::<Result<Vec<_>, _>>()?;

    MiningScenario::new(
        schedule.scenario_id().clone(),
        schedule.model_id().clone(),
        periods,
        ScenarioRules::default(),
        ScenarioConstraints::default(),
        Metadata::new(),
    )
}

fn accumulate_scaled_map(
    target: &mut BTreeMap<String, f64>,
    source: &BTreeMap<String, f64>,
    scale: f64,
) {
    for (key, value) in source {
        *target.entry(key.clone()).or_insert(0.0) += value * scale;
    }
}

fn summarize_risk_metric(values: &[f64]) -> Result<RiskMetricSummary, MineError> {
    if values.is_empty() {
        return Err(MineError::invalid_parameter(
            "values",
            "risk metric summary requires at least one value",
        ));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(MineError::invalid_parameter(
            "values",
            "risk metric summary requires finite values",
        ));
    }

    let mut ordered = values.to_vec();
    ordered.sort_by(|left, right| left.partial_cmp(right).expect("finite values must compare"));
    let sample_count = ordered.len();
    let min = ordered[0];
    let max = *ordered.last().expect("ordered values should not be empty");
    let mean = ordered.iter().sum::<f64>() / sample_count as f64;
    let p10 = nearest_rank_quantile(&ordered, 0.10);
    let p50 = nearest_rank_quantile(&ordered, 0.50);
    let p90 = nearest_rank_quantile(&ordered, 0.90);
    let downside_probability =
        ordered.iter().filter(|value| **value < 0.0).count() as f64 / sample_count as f64;
    let cvar_cutoff = ((sample_count as f64) * 0.10).ceil().max(1.0) as usize;
    let cvar10 = ordered.iter().take(cvar_cutoff).sum::<f64>() / cvar_cutoff as f64;

    Ok(RiskMetricSummary {
        sample_count,
        min,
        max,
        mean,
        p10,
        p50,
        p90,
        downside_probability,
        cvar10,
    })
}

fn nearest_rank_quantile(sorted_values: &[f64], quantile: f64) -> f64 {
    let rank = ((sorted_values.len() as f64) * quantile).ceil().max(1.0) as usize - 1;
    sorted_values[rank.min(sorted_values.len() - 1)]
}

/// Caso parametrizado para sensibilidad de scheduling/economía.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleSensitivityCase {
    /// Identificador estable del caso.
    pub case_id: String,
    /// Multiplicador opcional de precios.
    pub price_factor: Option<f64>,
    /// Multiplicador opcional de recoveries.
    pub recovery_factor: Option<f64>,
    /// Multiplicador opcional de costo de minado.
    pub mining_cost_factor: Option<f64>,
    /// Multiplicador opcional de costo de proceso.
    pub processing_cost_factor: Option<f64>,
    /// Multiplicador opcional de capacidad de mina por periodo.
    pub mine_capacity_factor: Option<f64>,
    /// Override opcional del avance vertical máximo.
    pub max_vertical_advance: Option<i64>,
}

impl LongTermScheduleSensitivityCase {
    /// Construye un caso de sensibilidad validado.
    pub fn new(
        case_id: impl Into<String>,
        price_factor: Option<f64>,
        recovery_factor: Option<f64>,
        mining_cost_factor: Option<f64>,
        processing_cost_factor: Option<f64>,
        mine_capacity_factor: Option<f64>,
        max_vertical_advance: Option<i64>,
    ) -> Result<Self, MineError> {
        let case_id = validate_case_id(case_id.into())?;
        validate_optional_factor("price_factor", price_factor)?;
        validate_optional_factor("recovery_factor", recovery_factor)?;
        validate_optional_factor("mining_cost_factor", mining_cost_factor)?;
        validate_optional_factor("processing_cost_factor", processing_cost_factor)?;
        validate_optional_factor("mine_capacity_factor", mine_capacity_factor)?;
        if let Some(max_vertical_advance) = max_vertical_advance
            && max_vertical_advance <= 0
        {
            return Err(MineError::invalid_parameter(
                "max_vertical_advance",
                "must be greater than zero when provided",
            ));
        }

        Ok(Self {
            case_id,
            price_factor,
            recovery_factor,
            mining_cost_factor,
            processing_cost_factor,
            mine_capacity_factor,
            max_vertical_advance,
        })
    }
}

/// Comparación por periodo entre el caso base y un candidato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPeriodComparison {
    /// Etiqueta del periodo.
    pub period_label: String,
    /// Tonelaje base.
    pub base_tonnage: Option<f64>,
    /// Tonelaje candidato.
    pub candidate_tonnage: Option<f64>,
    /// Delta de tonelaje `candidate - base`.
    pub tonnage_delta: Option<f64>,
    /// Cashflow base.
    pub base_cashflow: Option<f64>,
    /// Cashflow candidato.
    pub candidate_cashflow: Option<f64>,
    /// Delta de cashflow `candidate - base`.
    pub cashflow_delta: Option<f64>,
    /// Cashflow descontado base.
    pub base_discounted_cashflow: Option<f64>,
    /// Cashflow descontado candidato.
    pub candidate_discounted_cashflow: Option<f64>,
    /// Delta descontado `candidate - base`.
    pub discounted_cashflow_delta: Option<f64>,
}

/// Comparación completa entre el caso base y un candidato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioComparison {
    /// Identificador del caso candidato.
    pub case_id: String,
    /// Reporte económico completo del candidato.
    pub report: LongTermScheduleEconomicsReport,
    /// Delta de NPV `candidate - base`.
    pub npv_delta: f64,
    /// Delta de cashflow total `candidate - base`.
    pub total_cashflow_delta: f64,
    /// Delta de tonelaje total `candidate - base`.
    pub total_tonnage_delta: f64,
    /// Caso preferido por NPV.
    pub preferred_case_id: Option<String>,
    /// Comparaciones por periodo.
    pub period_comparisons: Vec<ScenarioPeriodComparison>,
}

/// Reporte serializable de comparación entre escenarios de sensibilidad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioComparisonReport {
    /// Identificador del caso base.
    pub base_case_id: String,
    /// Reporte económico del caso base.
    pub base_report: LongTermScheduleEconomicsReport,
    /// Casos comparados contra el base.
    pub comparisons: Vec<ScenarioComparison>,
}

/// Ejecuta un pack de sensibilidad sobre el scheduler agregado y la evaluación económica.
pub fn evaluate_long_term_schedule_sensitivity_pack(
    scenario_id: ScenarioId,
    model_id: ModelId,
    phase_plan: &PushbackPlan,
    capacities: Vec<LongTermSchedulePeriodCapacity>,
    max_vertical_advance: Option<i64>,
    economic_model: &EconomicBlockModel,
    discount_rate_per_period: f64,
    cases: &[LongTermScheduleSensitivityCase],
) -> Result<ScenarioComparisonReport, MineError> {
    let base_schedule = build_aggregated_long_term_schedule(
        scenario_id.clone(),
        model_id.clone(),
        phase_plan,
        capacities.clone(),
        max_vertical_advance,
        Metadata::new(),
    )?;
    let base_report = evaluate_long_term_schedule_economics(
        &base_schedule,
        phase_plan,
        economic_model,
        discount_rate_per_period,
    )?;
    let comparisons = cases
        .iter()
        .map(|case| {
            let candidate_capacities =
                scale_period_capacities(&capacities, case.mine_capacity_factor)?;
            let candidate_vertical_advance = case.max_vertical_advance.or(max_vertical_advance);
            let candidate_schedule = build_aggregated_long_term_schedule(
                scenario_id.clone(),
                model_id.clone(),
                phase_plan,
                candidate_capacities,
                candidate_vertical_advance,
                Metadata::new(),
            )?;
            let candidate_model = rebuild_economic_model_with_case(economic_model, case)?;
            let candidate_report = evaluate_long_term_schedule_economics(
                &candidate_schedule,
                phase_plan,
                &candidate_model,
                discount_rate_per_period,
            )?;

            Ok(compare_sensitivity_case(
                "base",
                &base_report,
                case,
                candidate_report,
            ))
        })
        .collect::<Result<Vec<_>, MineError>>()?;

    Ok(ScenarioComparisonReport {
        base_case_id: "base".to_owned(),
        base_report,
        comparisons,
    })
}

fn compare_sensitivity_case(
    base_case_id: &str,
    base_report: &LongTermScheduleEconomicsReport,
    case: &LongTermScheduleSensitivityCase,
    report: LongTermScheduleEconomicsReport,
) -> ScenarioComparison {
    ScenarioComparison {
        case_id: case.case_id.clone(),
        npv_delta: report.npv - base_report.npv,
        total_cashflow_delta: report.total_cashflow - base_report.total_cashflow,
        total_tonnage_delta: report.total_tonnage - base_report.total_tonnage,
        preferred_case_id: preferred_case_id(base_case_id, base_report, case, &report),
        period_comparisons: build_period_comparisons(&base_report.periods, &report.periods),
        report,
    }
}

fn preferred_case_id(
    base_case_id: &str,
    base_report: &LongTermScheduleEconomicsReport,
    case: &LongTermScheduleSensitivityCase,
    candidate_report: &LongTermScheduleEconomicsReport,
) -> Option<String> {
    if candidate_report.npv > base_report.npv {
        Some(case.case_id.clone())
    } else if candidate_report.npv < base_report.npv {
        Some(base_case_id.to_owned())
    } else {
        None
    }
}

fn build_period_comparisons(
    base_periods: &[LongTermSchedulePeriodEconomics],
    candidate_periods: &[LongTermSchedulePeriodEconomics],
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
                base_tonnage: base.map(|period| period.tonnage),
                candidate_tonnage: candidate.map(|period| period.tonnage),
                tonnage_delta: match (base, candidate) {
                    (Some(base), Some(candidate)) => Some(candidate.tonnage - base.tonnage),
                    _ => None,
                },
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

fn rebuild_economic_model_with_case(
    base_model: &EconomicBlockModel,
    case: &LongTermScheduleSensitivityCase,
) -> Result<EconomicBlockModel, MineError> {
    let destinations = base_model
        .destinations()
        .destinations()
        .iter()
        .map(|destination| adjust_destination(destination, case))
        .collect::<Result<Vec<_>, _>>()?;

    EconomicBlockModel::build(
        base_model.model().clone(),
        EconomicBlockModelConfig {
            tonnage_column: base_model.tonnage_column().clone(),
            grade_columns: base_model.grade_columns().to_vec(),
            destinations: DestinationAssumptionSet::new(destinations)?,
        },
    )
}

fn adjust_destination(
    destination: &DestinationAssumptions,
    case: &LongTermScheduleSensitivityCase,
) -> Result<DestinationAssumptions, MineError> {
    let adjusted_recoveries = destination
        .recoveries()
        .iter()
        .map(|recovery| adjust_recovery(recovery, case.recovery_factor))
        .collect::<Result<Vec<_>, _>>()?;
    let adjusted_prices = destination
        .price_per_metal_unit()
        .iter()
        .map(|(metal, price)| Ok((metal.clone(), price * case.price_factor.unwrap_or(1.0))))
        .collect::<Result<BTreeMap<_, _>, MineError>>()?;
    let adjusted_capacity = DestinationCapacity::new(
        destination.capacity().max_tonnes_per_period(),
        destination.capacity().tonnage_unit().clone(),
    )?;

    DestinationAssumptions::new(
        destination.id().clone(),
        destination.kind(),
        destination.mining_cost_per_tonne() * case.mining_cost_factor.unwrap_or(1.0),
        destination.processing_cost_per_tonne() * case.processing_cost_factor.unwrap_or(1.0),
        adjusted_recoveries,
        destination.payabilities().to_vec(),
        adjusted_capacity,
        adjusted_prices,
    )
}

fn adjust_recovery(
    recovery: &DestinationRecovery,
    factor: Option<f64>,
) -> Result<DestinationRecovery, MineError> {
    DestinationRecovery::new(
        recovery.metal_column().clone(),
        recovery.recovery_fraction() * factor.unwrap_or(1.0),
    )
}

fn scale_period_capacities(
    capacities: &[LongTermSchedulePeriodCapacity],
    mine_capacity_factor: Option<f64>,
) -> Result<Vec<LongTermSchedulePeriodCapacity>, MineError> {
    capacities
        .iter()
        .map(|capacity| {
            LongTermSchedulePeriodCapacity::new(
                capacity.period_label(),
                scale_optional(capacity.max_mine_tonnage(), mine_capacity_factor),
                capacity.max_plant_tonnage(),
                capacity.destination_capacities().to_vec(),
                capacity.stockpile_capacities().to_vec(),
            )
        })
        .collect()
}

fn scale_optional(value: Option<f64>, factor: Option<f64>) -> Option<f64> {
    match (value, factor) {
        (Some(value), Some(factor)) => Some(value * factor),
        (value, None) => value,
        (None, Some(_)) => None,
    }
}

fn validate_case_id(case_id: String) -> Result<String, MineError> {
    if case_id.trim().is_empty() {
        return Err(MineError::invalid_parameter(
            "case_id",
            "must not be empty or whitespace only",
        ));
    }
    if case_id.trim() != case_id {
        return Err(MineError::invalid_parameter(
            "case_id",
            "must not contain leading or trailing whitespace",
        ));
    }
    Ok(case_id)
}

fn validate_optional_factor(parameter: &'static str, factor: Option<f64>) -> Result<(), MineError> {
    if let Some(factor) = factor
        && (!factor.is_finite() || factor <= 0.0)
    {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than zero when provided",
        ));
    }
    Ok(())
}
