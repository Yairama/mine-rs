//! Solver exacto pequeño para `SchedulingProblem`.
//!
//! Esta ruta no intenta escalar a instancias industriales. Su objetivo es dar
//! una baseline exacta para fixtures pequeños, validar heurísticas y medir gaps
//! contra contratos CPIT/PCPSP ya normalizados.
//!
//! # References
//! - Caccetta, L., Hill, S. P. (2003). *An Application of Branch and Cut to
//!   Open Pit Mine Scheduling*. <https://doi.org/10.1007/A:1024835022186>
//! - Lambert, W. B., Brickey, A., Newman, A. M., Eurek, K. (2014).
//!   *Open-Pit Block-Sequencing Formulations: A Tutorial*.
//!   <https://doi.org/10.1287/inte.2013.0731>

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use mine_core::{Metadata, MetadataValue, MineError};

use crate::long_term_schedule::{
    LongTermSchedule, LongTermScheduleEntry, LongTermSchedulePeriodCapacity,
    LongTermScheduleStockpile, build_long_term_vertical_advance_violations,
};
use crate::scheduling_problem::{
    SchedulingObjectiveTerm, SchedulingProblem, SchedulingResourceId,
    SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId,
    destination_capacity_resource_id, stockpile_reclaim_capacity_resource_id,
};
use crate::{ScheduleDestinationId, ScheduleStockpileId, SchedulingPeriod};

const MAX_SMALL_SCHEDULING_UNITS: usize = 18;
const SCHEDULING_EPSILON: f64 = 1.0e-9;

/// Asignación exacta de una unidad a un periodo y destino opcional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmallSchedulingAssignment {
    unit_id: SchedulingUnitId,
    period_label: String,
    period_index: usize,
    destination_id: Option<ScheduleDestinationId>,
    stockpile_id: Option<ScheduleStockpileId>,
    stockpile_inventory_delta_tonnage: f64,
    objective_value: f64,
    discounted_objective_value: f64,
}

impl SmallSchedulingAssignment {
    /// Unidad asignada.
    #[must_use]
    pub fn unit_id(&self) -> &SchedulingUnitId {
        &self.unit_id
    }

    /// Etiqueta del periodo.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Índice del periodo.
    #[must_use]
    pub const fn period_index(&self) -> usize {
        self.period_index
    }

    /// Destino elegido, cuando aplica.
    #[must_use]
    pub fn destination_id(&self) -> Option<&ScheduleDestinationId> {
        self.destination_id.as_ref()
    }

    /// Stockpile elegido, cuando aplica.
    #[must_use]
    pub fn stockpile_id(&self) -> Option<&ScheduleStockpileId> {
        self.stockpile_id.as_ref()
    }

    /// Delta explícito de inventario aplicado cuando la asignación enruta a stockpile.
    #[must_use]
    pub const fn stockpile_inventory_delta_tonnage(&self) -> f64 {
        self.stockpile_inventory_delta_tonnage
    }

    /// Objetivo sin descuento del assignment.
    #[must_use]
    pub const fn objective_value(&self) -> f64 {
        self.objective_value
    }

    /// Objetivo descontado del assignment.
    #[must_use]
    pub const fn discounted_objective_value(&self) -> f64 {
        self.discounted_objective_value
    }
}

/// Uso observado de un recurso en un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmallSchedulingResourceUsage {
    resource_id: SchedulingResourceId,
    total: f64,
    min_total: Option<f64>,
    max_total: Option<f64>,
}

impl SmallSchedulingResourceUsage {
    /// Recurso reportado.
    #[must_use]
    pub fn resource_id(&self) -> &SchedulingResourceId {
        &self.resource_id
    }

    /// Uso total observado.
    #[must_use]
    pub const fn total(&self) -> f64 {
        self.total
    }

    /// Cota inferior configurada.
    #[must_use]
    pub const fn min_total(&self) -> Option<f64> {
        self.min_total
    }

    /// Cota superior configurada.
    #[must_use]
    pub const fn max_total(&self) -> Option<f64> {
        self.max_total
    }
}

/// Balance de inventario observado para un stockpile en un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmallSchedulingStockpileUsage {
    stockpile_id: ScheduleStockpileId,
    opening_tonnage: f64,
    inventory_delta_tonnage: f64,
    closing_tonnage: f64,
    max_inventory_tonnage: Option<f64>,
}

impl SmallSchedulingStockpileUsage {
    /// Stockpile reportado.
    #[must_use]
    pub fn stockpile_id(&self) -> &ScheduleStockpileId {
        &self.stockpile_id
    }

    /// Inventario de apertura del periodo.
    #[must_use]
    pub const fn opening_tonnage(&self) -> f64 {
        self.opening_tonnage
    }

    /// Delta neto de inventario aplicado en el periodo.
    #[must_use]
    pub const fn inventory_delta_tonnage(&self) -> f64 {
        self.inventory_delta_tonnage
    }

    /// Inventario de cierre del periodo.
    #[must_use]
    pub const fn closing_tonnage(&self) -> f64 {
        self.closing_tonnage
    }

    /// Límite máximo de inventario configurado para el periodo.
    #[must_use]
    pub const fn max_inventory_tonnage(&self) -> Option<f64> {
        self.max_inventory_tonnage
    }
}

/// Resumen de un periodo dentro de la solución exacta pequeña.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmallSchedulingPeriodSummary {
    period_label: String,
    assignment_count: usize,
    resource_usage: Vec<SmallSchedulingResourceUsage>,
    stockpile_usage: Vec<SmallSchedulingStockpileUsage>,
}

impl SmallSchedulingPeriodSummary {
    /// Etiqueta del periodo.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Cantidad de assignments en el periodo.
    #[must_use]
    pub const fn assignment_count(&self) -> usize {
        self.assignment_count
    }

    /// Uso de recursos del periodo.
    #[must_use]
    pub fn resource_usage(&self) -> &[SmallSchedulingResourceUsage] {
        &self.resource_usage
    }

    /// Balance de stockpiles observado durante el periodo.
    #[must_use]
    pub fn stockpile_usage(&self) -> &[SmallSchedulingStockpileUsage] {
        &self.stockpile_usage
    }
}

/// Resultado exacto pequeño.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmallSchedulingSolution {
    assignments: Vec<SmallSchedulingAssignment>,
    skipped_unit_ids: Vec<SchedulingUnitId>,
    total_objective_value: f64,
    total_discounted_objective_value: f64,
    periods: Vec<SmallSchedulingPeriodSummary>,
}

impl SmallSchedulingSolution {
    /// Assignments seleccionados.
    #[must_use]
    pub fn assignments(&self) -> &[SmallSchedulingAssignment] {
        &self.assignments
    }

    /// Unidades omitidas por la baseline exacta.
    #[must_use]
    pub fn skipped_unit_ids(&self) -> &[SchedulingUnitId] {
        &self.skipped_unit_ids
    }

    /// Objetivo total sin descuento.
    #[must_use]
    pub const fn total_objective_value(&self) -> f64 {
        self.total_objective_value
    }

    /// Objetivo total descontado.
    #[must_use]
    pub const fn total_discounted_objective_value(&self) -> f64 {
        self.total_discounted_objective_value
    }

    /// Resumen por periodo.
    #[must_use]
    pub fn periods(&self) -> &[SmallSchedulingPeriodSummary] {
        &self.periods
    }
}

/// Resuelve exactamente un `SchedulingProblem` pequeño mediante búsqueda exhaustiva.
///
/// La solución asigna cada unidad a lo sumo una vez, en un único periodo y con un
/// único destino opcional. Las precedencias se respetan en el sentido clásico:
/// una unidad solo puede minarse si todas sus predecesoras fueron minadas en el
/// mismo periodo o en uno anterior.
pub fn solve_small_scheduling_problem(
    problem: &SchedulingProblem,
) -> Result<SmallSchedulingSolution, MineError> {
    if problem.units().len() > MAX_SMALL_SCHEDULING_UNITS {
        return Err(MineError::invalid_parameter(
            "problem.units",
            "small exact scheduling supports at most 18 units",
        ));
    }

    let sorted_unit_indices = topological_unit_order(problem)?;
    let period_count = problem.periods().len();
    let period_resource_limits = problem
        .periods()
        .iter()
        .map(|period| {
            period
                .resource_bounds()
                .iter()
                .map(|bound| {
                    (
                        bound.resource_id().clone(),
                        (bound.min_total(), bound.max_total()),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    let mut usage_by_period = vec![BTreeMap::<SchedulingResourceId, f64>::new(); period_count];
    let objective_by_unit = index_objective_terms(problem.objective_terms());
    let requirements_by_unit = index_resource_requirements(problem.resource_requirements());
    let stockpile_opening_inventory = index_stockpile_opening_inventory(problem);
    let stockpile_inventory_limits = index_stockpile_inventory_limits(problem);
    let stockpile_reclaim_limits = index_stockpile_reclaim_limits(problem);
    let mut scheduled_period_by_unit = BTreeMap::<SchedulingUnitId, Option<usize>>::new();
    let mut assignments = Vec::<SmallSchedulingAssignment>::new();
    let mut stockpile_inventory_delta_by_period =
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); period_count];
    let mut stockpile_reclaims_by_period =
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); period_count];
    let mut best_solution = None::<SearchState>;

    search_exact_assignments(
        problem,
        &sorted_unit_indices,
        0,
        &period_resource_limits,
        &objective_by_unit,
        &requirements_by_unit,
        &stockpile_opening_inventory,
        &stockpile_inventory_limits,
        &stockpile_reclaim_limits,
        &mut usage_by_period,
        &mut stockpile_inventory_delta_by_period,
        &mut stockpile_reclaims_by_period,
        &mut scheduled_period_by_unit,
        &mut assignments,
        0.0,
        0.0,
        &mut best_solution,
    )?;

    let best_solution = best_solution.ok_or_else(|| MineError::Planning {
        message: "small exact scheduling did not find a feasible solution".to_owned(),
    })?;
    Ok(materialize_solution(
        problem,
        best_solution,
        &period_resource_limits,
    ))
}

/// Construye un schedule heurístico usando la frontera de unidades listas.
///
/// En cada periodo, el algoritmo elige iterativamente la opción factible con
/// mayor valor descontado por unidad de carga, respetando precedencias y cotas
/// superiores por recurso.
///
/// # References
/// - Tolwinski, B. (1996). *A scheduling algorithm for open pit mines*.
///   <https://doi.org/10.1093/imaman/7.3.247>
/// - Lambert, W. B., Brickey, A., Newman, A. M., Eurek, K. (2014).
///   *Open-Pit Block-Sequencing Formulations: A Tutorial*.
///   <https://doi.org/10.1287/inte.2013.0731>
/// - Cullenbine, C., Wood, R. K., Newman, A. M. (2011).
///   *A Sliding Time Window Heuristic for Open Pit Mine Block Sequencing*.
///   <https://doi.org/10.1007/s11590-011-0306-2>
pub fn build_ready_frontier_schedule(
    problem: &SchedulingProblem,
) -> Result<SmallSchedulingSolution, MineError> {
    let period_resource_limits = problem
        .periods()
        .iter()
        .map(|period| {
            period
                .resource_bounds()
                .iter()
                .map(|bound| {
                    (
                        bound.resource_id().clone(),
                        (bound.min_total(), bound.max_total()),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    let objective_by_unit = index_objective_terms(problem.objective_terms());
    let requirements_by_unit = index_resource_requirements(problem.resource_requirements());
    let stockpile_inventory_limits = index_stockpile_inventory_limits(problem);
    let stockpile_reclaim_limits = index_stockpile_reclaim_limits(problem);
    let stockpile_future_inventory_limits =
        build_future_stockpile_inventory_limits(&stockpile_inventory_limits);
    let mut stockpile_inventory_by_id = index_stockpile_opening_inventory(problem);
    let mut stockpile_reclaims_by_period =
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); problem.periods().len()];
    let mut usage_by_period =
        vec![BTreeMap::<SchedulingResourceId, f64>::new(); problem.periods().len()];
    let mut scheduled_period_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    let mut assignments = Vec::<SmallSchedulingAssignment>::new();
    let mut total_objective_value = 0.0;
    let mut total_discounted_objective_value = 0.0;

    for period_index in 0..problem.periods().len() {
        loop {
            let mut best_candidate = None::<FrontierCandidate>;
            for unit in problem
                .units()
                .iter()
                .filter(|unit| !scheduled_period_by_unit.contains_key(unit.unit_id()))
                .filter(|unit| {
                    unit.predecessor_unit_ids()
                        .iter()
                        .all(|predecessor| scheduled_period_by_unit.contains_key(predecessor))
                })
            {
                for option in build_unit_options(unit, &objective_by_unit, &requirements_by_unit) {
                    if !fits_upper_bounds(
                        period_index,
                        &option.requirements,
                        &usage_by_period,
                        &period_resource_limits,
                    ) {
                        continue;
                    }
                    if !fits_effective_stockpile_bounds(
                        period_index,
                        option.stockpile_inventory_delta_tonnage,
                        option.stockpile_id.as_ref(),
                        &stockpile_inventory_by_id,
                        &stockpile_reclaims_by_period,
                        &stockpile_future_inventory_limits,
                        &stockpile_reclaim_limits,
                    ) {
                        continue;
                    }

                    let discounted_objective_value = option.objective_value
                        / (1.0 + problem.discount_rate()).powi(period_index as i32);
                    let load = option
                        .requirements
                        .iter()
                        .map(|(_, amount)| *amount)
                        .sum::<f64>()
                        .max(unit.tonnage())
                        .max(1.0e-9);
                    let candidate = FrontierCandidate {
                        unit_id: unit.unit_id().clone(),
                        period_index,
                        destination_id: option.destination_id,
                        stockpile_id: option.stockpile_id,
                        stockpile_inventory_delta_tonnage: option.stockpile_inventory_delta_tonnage,
                        requirements: option.requirements,
                        objective_value: option.objective_value,
                        discounted_objective_value,
                        priority_distance: 0,
                        score: discounted_objective_value / load,
                    };
                    let replace = best_candidate.as_ref().is_none_or(|best| {
                        candidate.score > best.score
                            || (candidate.score == best.score
                                && (candidate.discounted_objective_value
                                    > best.discounted_objective_value
                                    || (candidate.discounted_objective_value
                                        == best.discounted_objective_value
                                        && candidate.unit_id < best.unit_id)))
                    });
                    if replace {
                        best_candidate = Some(candidate);
                    }
                }
            }

            let Some(best_candidate) = best_candidate else {
                break;
            };

            apply_requirements(
                best_candidate.period_index,
                &best_candidate.requirements,
                &mut usage_by_period,
                1.0,
            );
            apply_stockpile_inventory(
                best_candidate.stockpile_id.as_ref(),
                best_candidate.stockpile_inventory_delta_tonnage,
                &mut stockpile_inventory_by_id,
                1.0,
            );
            apply_stockpile_reclaim(
                best_candidate.period_index,
                best_candidate.stockpile_inventory_delta_tonnage,
                best_candidate.stockpile_id.as_ref(),
                &mut stockpile_reclaims_by_period,
                1.0,
            );
            scheduled_period_by_unit
                .insert(best_candidate.unit_id.clone(), best_candidate.period_index);
            total_objective_value += best_candidate.objective_value;
            total_discounted_objective_value += best_candidate.discounted_objective_value;
            assignments.push(SmallSchedulingAssignment {
                unit_id: best_candidate.unit_id,
                period_label: problem.periods()[best_candidate.period_index]
                    .period_label()
                    .to_owned(),
                period_index: best_candidate.period_index,
                destination_id: best_candidate.destination_id,
                stockpile_id: best_candidate.stockpile_id,
                stockpile_inventory_delta_tonnage: best_candidate.stockpile_inventory_delta_tonnage,
                objective_value: best_candidate.objective_value,
                discounted_objective_value: best_candidate.discounted_objective_value,
            });
        }
    }

    if !period_lower_bounds_satisfied(&usage_by_period, &period_resource_limits) {
        return Err(MineError::Planning {
            message: "ready frontier schedule did not satisfy all configured lower resource bounds"
                .to_owned(),
        });
    }

    Ok(materialize_solution(
        problem,
        SearchState {
            assignments,
            usage_by_period,
            total_objective_value,
            total_discounted_objective_value,
        },
        &period_resource_limits,
    ))
}

/// Construye un schedule heurístico seeded por un periodo objetivo por unidad.
///
/// Esta variante sigue la misma factibilidad que `build_ready_frontier_schedule`,
/// pero dentro de cada frontera lista prioriza primero la cercanía al periodo
/// objetivo provisto por `target_period_by_unit` y recién después el score
/// económico descontado por carga.
pub fn build_target_period_seeded_schedule(
    problem: &SchedulingProblem,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> Result<SmallSchedulingSolution, MineError> {
    let period_resource_limits = problem
        .periods()
        .iter()
        .map(|period| {
            period
                .resource_bounds()
                .iter()
                .map(|bound| {
                    (
                        bound.resource_id().clone(),
                        (bound.min_total(), bound.max_total()),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    let objective_by_unit = index_objective_terms(problem.objective_terms());
    let requirements_by_unit = index_resource_requirements(problem.resource_requirements());
    let stockpile_inventory_limits = index_stockpile_inventory_limits(problem);
    let stockpile_reclaim_limits = index_stockpile_reclaim_limits(problem);
    let stockpile_future_inventory_limits =
        build_future_stockpile_inventory_limits(&stockpile_inventory_limits);
    let mut stockpile_inventory_by_id = index_stockpile_opening_inventory(problem);
    let mut stockpile_reclaims_by_period =
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); problem.periods().len()];
    let mut usage_by_period =
        vec![BTreeMap::<SchedulingResourceId, f64>::new(); problem.periods().len()];
    let mut scheduled_period_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    let mut assignments = Vec::<SmallSchedulingAssignment>::new();
    let mut total_objective_value = 0.0;
    let mut total_discounted_objective_value = 0.0;

    for period_index in 0..problem.periods().len() {
        loop {
            let mut best_candidate = None::<FrontierCandidate>;
            for unit in problem
                .units()
                .iter()
                .filter(|unit| !scheduled_period_by_unit.contains_key(unit.unit_id()))
                .filter(|unit| {
                    unit.predecessor_unit_ids()
                        .iter()
                        .all(|predecessor| scheduled_period_by_unit.contains_key(predecessor))
                })
            {
                for option in build_unit_options(unit, &objective_by_unit, &requirements_by_unit) {
                    if !fits_upper_bounds(
                        period_index,
                        &option.requirements,
                        &usage_by_period,
                        &period_resource_limits,
                    ) {
                        continue;
                    }
                    if !fits_effective_stockpile_bounds(
                        period_index,
                        option.stockpile_inventory_delta_tonnage,
                        option.stockpile_id.as_ref(),
                        &stockpile_inventory_by_id,
                        &stockpile_reclaims_by_period,
                        &stockpile_future_inventory_limits,
                        &stockpile_reclaim_limits,
                    ) {
                        continue;
                    }

                    let discounted_objective_value = option.objective_value
                        / (1.0 + problem.discount_rate()).powi(period_index as i32);
                    let load = option
                        .requirements
                        .iter()
                        .map(|(_, amount)| *amount)
                        .sum::<f64>()
                        .max(unit.tonnage())
                        .max(1.0e-9);
                    let target_period_index = target_period_by_unit
                        .get(unit.unit_id())
                        .copied()
                        .unwrap_or(period_index);
                    let candidate = FrontierCandidate {
                        unit_id: unit.unit_id().clone(),
                        period_index,
                        destination_id: option.destination_id,
                        stockpile_id: option.stockpile_id,
                        stockpile_inventory_delta_tonnage: option.stockpile_inventory_delta_tonnage,
                        requirements: option.requirements,
                        objective_value: option.objective_value,
                        discounted_objective_value,
                        priority_distance: period_index.abs_diff(target_period_index),
                        score: discounted_objective_value / load,
                    };
                    let replace = best_candidate.as_ref().is_none_or(|best| {
                        candidate.priority_distance < best.priority_distance
                            || (candidate.priority_distance == best.priority_distance
                                && (candidate.score > best.score
                                    || (candidate.score == best.score
                                        && (candidate.discounted_objective_value
                                            > best.discounted_objective_value
                                            || (candidate.discounted_objective_value
                                                == best.discounted_objective_value
                                                && candidate.unit_id < best.unit_id)))))
                    });
                    if replace {
                        best_candidate = Some(candidate);
                    }
                }
            }

            let Some(best_candidate) = best_candidate else {
                break;
            };

            apply_requirements(
                best_candidate.period_index,
                &best_candidate.requirements,
                &mut usage_by_period,
                1.0,
            );
            apply_stockpile_inventory(
                best_candidate.stockpile_id.as_ref(),
                best_candidate.stockpile_inventory_delta_tonnage,
                &mut stockpile_inventory_by_id,
                1.0,
            );
            apply_stockpile_reclaim(
                best_candidate.period_index,
                best_candidate.stockpile_inventory_delta_tonnage,
                best_candidate.stockpile_id.as_ref(),
                &mut stockpile_reclaims_by_period,
                1.0,
            );
            scheduled_period_by_unit
                .insert(best_candidate.unit_id.clone(), best_candidate.period_index);
            total_objective_value += best_candidate.objective_value;
            total_discounted_objective_value += best_candidate.discounted_objective_value;
            assignments.push(SmallSchedulingAssignment {
                unit_id: best_candidate.unit_id,
                period_label: problem.periods()[best_candidate.period_index]
                    .period_label()
                    .to_owned(),
                period_index: best_candidate.period_index,
                destination_id: best_candidate.destination_id,
                stockpile_id: best_candidate.stockpile_id,
                stockpile_inventory_delta_tonnage: best_candidate.stockpile_inventory_delta_tonnage,
                objective_value: best_candidate.objective_value,
                discounted_objective_value: best_candidate.discounted_objective_value,
            });
        }
    }

    if !period_lower_bounds_satisfied(&usage_by_period, &period_resource_limits) {
        return Err(MineError::Planning {
            message:
                "target-period seeded schedule did not satisfy all configured lower resource bounds"
                    .to_owned(),
        });
    }

    Ok(materialize_solution(
        problem,
        SearchState {
            assignments,
            usage_by_period,
            total_objective_value,
            total_discounted_objective_value,
        },
        &period_resource_limits,
    ))
}

/// Construye un schedule rolling-horizon que resuelve exactamente un packing
/// pequeño por periodo sobre una ventana LP-guided de unidades ready.
///
/// En cada iteración selecciona hasta `candidate_window_size` unidades listas,
/// priorizadas por cercanía al periodo objetivo y score económico, y resuelve
/// exactamente qué subconjunto cabe en el periodo actual. Luego reabre la
/// frontera dentro del mismo periodo para permitir precedencias en el mismo
/// bucket temporal.
pub fn build_target_period_windowed_schedule(
    problem: &SchedulingProblem,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    candidate_window_size: usize,
) -> Result<SmallSchedulingSolution, MineError> {
    if candidate_window_size == 0 || candidate_window_size > MAX_SMALL_SCHEDULING_UNITS {
        return Err(MineError::invalid_parameter(
            "candidate_window_size",
            "must be between 1 and 18",
        ));
    }

    let period_resource_limits = problem
        .periods()
        .iter()
        .map(|period| {
            period
                .resource_bounds()
                .iter()
                .map(|bound| {
                    (
                        bound.resource_id().clone(),
                        (bound.min_total(), bound.max_total()),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    let objective_by_unit = index_objective_terms(problem.objective_terms());
    let requirements_by_unit = index_resource_requirements(problem.resource_requirements());
    let stockpile_inventory_limits = index_stockpile_inventory_limits(problem);
    let stockpile_reclaim_limits = index_stockpile_reclaim_limits(problem);
    let stockpile_future_inventory_limits =
        build_future_stockpile_inventory_limits(&stockpile_inventory_limits);
    let mut stockpile_inventory_by_id = index_stockpile_opening_inventory(problem);
    let mut stockpile_reclaims_by_period =
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); problem.periods().len()];
    let mut usage_by_period =
        vec![BTreeMap::<SchedulingResourceId, f64>::new(); problem.periods().len()];
    let mut scheduled_period_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    let mut assignments = Vec::<SmallSchedulingAssignment>::new();
    let mut total_objective_value = 0.0;
    let mut total_discounted_objective_value = 0.0;

    for period_index in 0..problem.periods().len() {
        loop {
            let ready_window = select_window_candidates(
                problem,
                period_index,
                candidate_window_size,
                target_period_by_unit,
                &objective_by_unit,
                &requirements_by_unit,
                &stockpile_inventory_by_id,
                &stockpile_reclaims_by_period,
                &stockpile_future_inventory_limits,
                &stockpile_reclaim_limits,
                &usage_by_period,
                &period_resource_limits,
                &scheduled_period_by_unit,
            );
            if ready_window.is_empty() {
                break;
            }

            let residual_period = build_residual_period_for_window(
                &problem.periods()[period_index],
                &usage_by_period[period_index],
                &stockpile_reclaims_by_period[period_index],
                &stockpile_future_inventory_limits[period_index],
                &stockpile_reclaim_limits[period_index],
            )?;
            let window_problem = build_window_subproblem(
                problem,
                &ready_window,
                residual_period,
                &stockpile_inventory_by_id,
            )?;
            let window_solution = solve_small_scheduling_problem(&window_problem)?;
            if window_solution.assignments().is_empty() {
                let Some(fallback_candidate) = best_candidate_for_unit(
                    problem,
                    problem
                        .units()
                        .iter()
                        .find(|unit| unit.unit_id() == &ready_window[0])
                        .expect("window candidate should exist"),
                    period_index,
                    &objective_by_unit,
                    &requirements_by_unit,
                    &stockpile_inventory_by_id,
                    &stockpile_reclaims_by_period,
                    &stockpile_future_inventory_limits,
                    &stockpile_reclaim_limits,
                    &usage_by_period,
                    &period_resource_limits,
                ) else {
                    break;
                };
                apply_requirements(
                    period_index,
                    &fallback_candidate.requirements,
                    &mut usage_by_period,
                    1.0,
                );
                apply_stockpile_inventory(
                    fallback_candidate.stockpile_id.as_ref(),
                    fallback_candidate.stockpile_inventory_delta_tonnage,
                    &mut stockpile_inventory_by_id,
                    1.0,
                );
                apply_stockpile_reclaim(
                    period_index,
                    fallback_candidate.stockpile_inventory_delta_tonnage,
                    fallback_candidate.stockpile_id.as_ref(),
                    &mut stockpile_reclaims_by_period,
                    1.0,
                );
                scheduled_period_by_unit.insert(fallback_candidate.unit_id.clone(), period_index);
                total_objective_value += fallback_candidate.objective_value;
                total_discounted_objective_value += fallback_candidate.discounted_objective_value;
                assignments.push(SmallSchedulingAssignment {
                    unit_id: fallback_candidate.unit_id,
                    period_label: problem.periods()[period_index].period_label().to_owned(),
                    period_index,
                    destination_id: fallback_candidate.destination_id,
                    stockpile_id: fallback_candidate.stockpile_id,
                    stockpile_inventory_delta_tonnage: fallback_candidate
                        .stockpile_inventory_delta_tonnage,
                    objective_value: fallback_candidate.objective_value,
                    discounted_objective_value: fallback_candidate.discounted_objective_value,
                });
                continue;
            }

            for assignment in window_solution.assignments() {
                let requirements = requirements_by_unit
                    .get(assignment.unit_id())
                    .map(|indexed| {
                        indexed
                            .iter()
                            .filter(|(_, destination_id, _)| {
                                destination_id.is_none()
                                    || *destination_id == assignment.destination_id().cloned()
                            })
                            .map(|(resource_id, _, amount)| (resource_id.clone(), *amount))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                apply_requirements(period_index, &requirements, &mut usage_by_period, 1.0);
                apply_stockpile_inventory(
                    assignment.stockpile_id(),
                    assignment.stockpile_inventory_delta_tonnage(),
                    &mut stockpile_inventory_by_id,
                    1.0,
                );
                apply_stockpile_reclaim(
                    period_index,
                    assignment.stockpile_inventory_delta_tonnage(),
                    assignment.stockpile_id(),
                    &mut stockpile_reclaims_by_period,
                    1.0,
                );
                scheduled_period_by_unit.insert(assignment.unit_id().clone(), period_index);
                total_objective_value += assignment.objective_value();
                total_discounted_objective_value += assignment.discounted_objective_value();
                assignments.push(SmallSchedulingAssignment {
                    unit_id: assignment.unit_id().clone(),
                    period_label: problem.periods()[period_index].period_label().to_owned(),
                    period_index,
                    destination_id: assignment.destination_id().cloned(),
                    stockpile_id: assignment.stockpile_id().cloned(),
                    stockpile_inventory_delta_tonnage: assignment
                        .stockpile_inventory_delta_tonnage(),
                    objective_value: assignment.objective_value(),
                    discounted_objective_value: assignment.discounted_objective_value(),
                });
            }
        }
    }

    if !period_lower_bounds_satisfied(&usage_by_period, &period_resource_limits) {
        return Err(MineError::Planning {
            message:
                "target-period windowed schedule did not satisfy all configured lower resource bounds"
                    .to_owned(),
        });
    }

    Ok(materialize_solution(
        problem,
        SearchState {
            assignments,
            usage_by_period,
            total_objective_value,
            total_discounted_objective_value,
        },
        &period_resource_limits,
    ))
}

/// Construye un `LongTermSchedule` usando la heurística `ready frontier` sobre
/// un `SchedulingProblem` con ruteo explícito por destino.
///
/// Si el contrato no declara requerimientos de tonelaje para mina, planta o
/// capacidades por destino, esta función deriva automáticamente los mínimos
/// necesarios a partir del tonelaje de cada unidad para que dichas cotas formen
/// parte de la decisión del scheduler y no queden relegadas a una evaluación
/// posterior.
///
/// # References
/// - Tolwinski, B. (1996). *A scheduling algorithm for open pit mines*.
///   <https://doi.org/10.1093/imaman/7.3.247>
/// - Lambert, W. B., Brickey, A., Newman, A. M., Eurek, K. (2014).
///   *Open-Pit Block-Sequencing Formulations: A Tutorial*.
///   <https://doi.org/10.1287/inte.2013.0731>
pub fn build_ready_frontier_long_term_schedule(
    problem: &SchedulingProblem,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<LongTermSchedule, MineError> {
    let enriched_problem = enrich_problem_for_ready_frontier(problem)?;
    let solution = build_ready_frontier_schedule(&enriched_problem)?;
    materialize_long_term_schedule(problem, &solution, max_vertical_advance, metadata)
}

/// Construye un `LongTermSchedule` seeded por periodos objetivo por unidad.
pub fn build_target_period_seeded_long_term_schedule(
    problem: &SchedulingProblem,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<LongTermSchedule, MineError> {
    let enriched_problem = enrich_problem_for_ready_frontier(problem)?;
    let solution = build_target_period_seeded_schedule(&enriched_problem, target_period_by_unit)?;
    materialize_long_term_schedule(problem, &solution, max_vertical_advance, metadata)
}

/// Construye un `LongTermSchedule` rolling-horizon sobre una ventana exacta
/// LP-guided por periodo.
pub fn build_target_period_windowed_long_term_schedule(
    problem: &SchedulingProblem,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    candidate_window_size: usize,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<LongTermSchedule, MineError> {
    let enriched_problem = enrich_problem_for_ready_frontier(problem)?;
    let solution = build_target_period_windowed_schedule(
        &enriched_problem,
        target_period_by_unit,
        candidate_window_size,
    )?;
    materialize_long_term_schedule(problem, &solution, max_vertical_advance, metadata)
}

#[derive(Clone)]
struct SearchState {
    assignments: Vec<SmallSchedulingAssignment>,
    usage_by_period: Vec<BTreeMap<SchedulingResourceId, f64>>,
    total_objective_value: f64,
    total_discounted_objective_value: f64,
}

type ObjectiveIndex = BTreeMap<SchedulingUnitId, BTreeMap<Option<ScheduleDestinationId>, f64>>;
type RequirementIndex =
    BTreeMap<SchedulingUnitId, Vec<(SchedulingResourceId, Option<ScheduleDestinationId>, f64)>>;
type StockpileInventoryLimitIndex = Vec<BTreeMap<ScheduleStockpileId, f64>>;

fn search_exact_assignments(
    problem: &SchedulingProblem,
    sorted_unit_indices: &[usize],
    depth: usize,
    period_resource_limits: &[BTreeMap<SchedulingResourceId, (Option<f64>, Option<f64>)>],
    objective_by_unit: &ObjectiveIndex,
    requirements_by_unit: &RequirementIndex,
    stockpile_opening_inventory: &BTreeMap<ScheduleStockpileId, f64>,
    stockpile_inventory_limits: &StockpileInventoryLimitIndex,
    stockpile_reclaim_limits: &StockpileInventoryLimitIndex,
    usage_by_period: &mut [BTreeMap<SchedulingResourceId, f64>],
    stockpile_inventory_delta_by_period: &mut [BTreeMap<ScheduleStockpileId, f64>],
    stockpile_reclaims_by_period: &mut [BTreeMap<ScheduleStockpileId, f64>],
    scheduled_period_by_unit: &mut BTreeMap<SchedulingUnitId, Option<usize>>,
    assignments: &mut Vec<SmallSchedulingAssignment>,
    current_total_objective: f64,
    current_total_discounted_objective: f64,
    best_solution: &mut Option<SearchState>,
) -> Result<(), MineError> {
    if depth == sorted_unit_indices.len() {
        if period_lower_bounds_satisfied(usage_by_period, period_resource_limits) {
            let candidate = SearchState {
                assignments: assignments.clone(),
                usage_by_period: usage_by_period.to_vec(),
                total_objective_value: current_total_objective,
                total_discounted_objective_value: current_total_discounted_objective,
            };
            let replace = best_solution.as_ref().is_none_or(|best| {
                candidate.total_discounted_objective_value > best.total_discounted_objective_value
            });
            if replace {
                *best_solution = Some(candidate);
            }
        }
        return Ok(());
    }

    let unit = &problem.units()[sorted_unit_indices[depth]];
    let predecessor_periods = predecessor_periods(unit, scheduled_period_by_unit)?;
    let unit_options = build_unit_options(unit, objective_by_unit, requirements_by_unit);

    scheduled_period_by_unit.insert(unit.unit_id().clone(), None);
    search_exact_assignments(
        problem,
        sorted_unit_indices,
        depth + 1,
        period_resource_limits,
        objective_by_unit,
        requirements_by_unit,
        stockpile_opening_inventory,
        stockpile_inventory_limits,
        stockpile_reclaim_limits,
        usage_by_period,
        stockpile_inventory_delta_by_period,
        stockpile_reclaims_by_period,
        scheduled_period_by_unit,
        assignments,
        current_total_objective,
        current_total_discounted_objective,
        best_solution,
    )?;
    scheduled_period_by_unit.remove(unit.unit_id());

    let Some(earliest_period_index) = predecessor_periods else {
        return Ok(());
    };

    for period_index in earliest_period_index..problem.periods().len() {
        for option in &unit_options {
            if !fits_upper_bounds(
                period_index,
                &option.requirements,
                usage_by_period,
                period_resource_limits,
            ) {
                continue;
            }
            if !fits_stockpile_inventory_bounds(
                period_index,
                option.stockpile_inventory_delta_tonnage,
                option.stockpile_id.as_ref(),
                stockpile_inventory_delta_by_period,
                stockpile_reclaims_by_period,
                stockpile_opening_inventory,
                stockpile_inventory_limits,
                stockpile_reclaim_limits,
            ) {
                continue;
            }

            apply_requirements(period_index, &option.requirements, usage_by_period, 1.0);
            apply_stockpile_inventory_delta(
                period_index,
                option.stockpile_inventory_delta_tonnage,
                option.stockpile_id.as_ref(),
                stockpile_inventory_delta_by_period,
                1.0,
            );
            apply_stockpile_reclaim(
                period_index,
                option.stockpile_inventory_delta_tonnage,
                option.stockpile_id.as_ref(),
                stockpile_reclaims_by_period,
                1.0,
            );
            let discounted_objective_value =
                option.objective_value / (1.0 + problem.discount_rate()).powi(period_index as i32);
            assignments.push(SmallSchedulingAssignment {
                unit_id: unit.unit_id().clone(),
                period_label: problem.periods()[period_index].period_label().to_owned(),
                period_index,
                destination_id: option.destination_id.clone(),
                stockpile_id: option.stockpile_id.clone(),
                stockpile_inventory_delta_tonnage: option.stockpile_inventory_delta_tonnage,
                objective_value: option.objective_value,
                discounted_objective_value,
            });
            scheduled_period_by_unit.insert(unit.unit_id().clone(), Some(period_index));

            search_exact_assignments(
                problem,
                sorted_unit_indices,
                depth + 1,
                period_resource_limits,
                objective_by_unit,
                requirements_by_unit,
                stockpile_opening_inventory,
                stockpile_inventory_limits,
                stockpile_reclaim_limits,
                usage_by_period,
                stockpile_inventory_delta_by_period,
                stockpile_reclaims_by_period,
                scheduled_period_by_unit,
                assignments,
                current_total_objective + option.objective_value,
                current_total_discounted_objective + discounted_objective_value,
                best_solution,
            )?;

            scheduled_period_by_unit.remove(unit.unit_id());
            assignments.pop();
            apply_stockpile_reclaim(
                period_index,
                option.stockpile_inventory_delta_tonnage,
                option.stockpile_id.as_ref(),
                stockpile_reclaims_by_period,
                -1.0,
            );
            apply_stockpile_inventory_delta(
                period_index,
                option.stockpile_inventory_delta_tonnage,
                option.stockpile_id.as_ref(),
                stockpile_inventory_delta_by_period,
                -1.0,
            );
            apply_requirements(period_index, &option.requirements, usage_by_period, -1.0);
        }
    }

    Ok(())
}

fn topological_unit_order(problem: &SchedulingProblem) -> Result<Vec<usize>, MineError> {
    let remaining = problem
        .units()
        .iter()
        .map(|unit| {
            (
                unit.unit_id().clone(),
                unit.predecessor_unit_ids()
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut remaining = remaining;
    let mut scheduled = BTreeSet::<SchedulingUnitId>::new();
    let mut order = Vec::<usize>::new();

    while !remaining.is_empty() {
        let ready_ids = remaining
            .iter()
            .filter(|(_, predecessors)| predecessors.is_subset(&scheduled))
            .map(|(unit_id, _)| unit_id.clone())
            .collect::<Vec<_>>();
        if ready_ids.is_empty() {
            return Err(MineError::Validation {
                message: "scheduling problem contains cyclic or unresolved precedences".to_owned(),
            });
        }

        for ready_id in ready_ids {
            let unit_index = problem
                .units()
                .iter()
                .position(|unit| unit.unit_id() == &ready_id)
                .expect("validated unit should exist");
            order.push(unit_index);
            scheduled.insert(ready_id.clone());
            remaining.remove(&ready_id);
        }
    }

    Ok(order)
}

pub(crate) fn enrich_problem_for_ready_frontier(
    problem: &SchedulingProblem,
) -> Result<SchedulingProblem, MineError> {
    let periods = problem
        .periods()
        .iter()
        .map(enrich_period_with_destination_capacity_resources)
        .collect::<Result<Vec<_>, _>>()?;
    let declared_resource_ids = periods
        .iter()
        .flat_map(SchedulingPeriod::resource_bounds)
        .map(|bound| bound.resource_id().clone())
        .collect::<BTreeSet<_>>();
    let mine_resource = declared_resource_ids
        .iter()
        .find(|resource_id| resource_id.as_str() == "mine_tonnage")
        .cloned();
    let plant_resource = declared_resource_ids
        .iter()
        .find(|resource_id| resource_id.as_str() == "plant_tonnage")
        .cloned();
    let mut resource_requirements = problem.resource_requirements().to_vec();
    let mut derived_requirements = false;

    for unit in problem.units() {
        if let Some(mine_resource) = &mine_resource
            && !unit.is_stockpile_reclaim()
            && !has_requirement_for_unit_resource(
                &resource_requirements,
                unit.unit_id(),
                mine_resource,
            )
        {
            resource_requirements.push(SchedulingResourceRequirement::new(
                unit.unit_id().clone(),
                mine_resource.clone(),
                None,
                unit.tonnage(),
            )?);
            derived_requirements = true;
        }

        if let Some(reclaim_tonnage) = unit.stockpile_reclaim_tonnage() {
            for stockpile_id in unit.eligible_stockpile_ids() {
                let reclaim_resource = stockpile_reclaim_capacity_resource_id(stockpile_id)?;
                if declared_resource_ids.contains(&reclaim_resource)
                    && !has_requirement_for_unit_resource(
                        &resource_requirements,
                        unit.unit_id(),
                        &reclaim_resource,
                    )
                {
                    resource_requirements.push(SchedulingResourceRequirement::new(
                        unit.unit_id().clone(),
                        reclaim_resource,
                        None,
                        reclaim_tonnage,
                    )?);
                    derived_requirements = true;
                }
            }
        }

        let candidate_destinations = candidate_destination_ids(problem, unit);
        if candidate_destinations.is_empty() {
            continue;
        }

        if let Some(plant_resource) = &plant_resource
            && !unit.is_stockpile_reclaim()
            && !has_requirement_for_unit_resource(
                &resource_requirements,
                unit.unit_id(),
                plant_resource,
            )
        {
            for destination_id in &candidate_destinations {
                resource_requirements.push(SchedulingResourceRequirement::new(
                    unit.unit_id().clone(),
                    plant_resource.clone(),
                    Some(destination_id.clone()),
                    unit.tonnage(),
                )?);
            }
            derived_requirements = true;
        }

        for destination_id in &candidate_destinations {
            let destination_resource = destination_capacity_resource_id(destination_id)?;
            if declared_resource_ids.contains(&destination_resource)
                && !has_requirement_for_unit_resource(
                    &resource_requirements,
                    unit.unit_id(),
                    &destination_resource,
                )
            {
                resource_requirements.push(SchedulingResourceRequirement::new(
                    unit.unit_id().clone(),
                    destination_resource,
                    Some(destination_id.clone()),
                    unit.tonnage(),
                )?);
                derived_requirements = true;
            }
        }
    }

    let mut limitations = problem.limitations().to_vec();
    if derived_requirements {
        limitations.push(
            "Ready-frontier long-term schedule auto-derived tonnage requirements for mine, plant, destination and stockpile reclaim capacities when the SchedulingProblem omitted them.".to_owned(),
        );
    }

    SchedulingProblem::new(
        problem.scenario_id().clone(),
        problem.model_id().clone(),
        periods,
        problem.units().to_vec(),
        problem.objective_terms().to_vec(),
        resource_requirements,
        problem.destination_ids().to_vec(),
        problem.stockpiles().to_vec(),
        problem.discount_rate(),
        problem.metadata().clone(),
        limitations,
    )
}

fn enrich_period_with_destination_capacity_resources(
    period: &SchedulingPeriod,
) -> Result<SchedulingPeriod, MineError> {
    let mut resource_bounds = period.resource_bounds().to_vec();
    for capacity in period.destination_capacities() {
        let Some(max_total) = capacity.max_tonnage() else {
            continue;
        };
        let resource_id = destination_capacity_resource_id(capacity.destination_id())?;
        if resource_bounds
            .iter()
            .any(|bound| bound.resource_id() == &resource_id)
        {
            continue;
        }
        resource_bounds.push(crate::SchedulingResourceBound::new(
            resource_id,
            None,
            Some(max_total),
        )?);
    }
    for capacity in period.stockpile_capacities() {
        let Some(max_total) = capacity.max_reclaim_tonnage() else {
            continue;
        };
        let resource_id = stockpile_reclaim_capacity_resource_id(capacity.stockpile_id())?;
        if resource_bounds
            .iter()
            .any(|bound| bound.resource_id() == &resource_id)
        {
            continue;
        }
        resource_bounds.push(crate::SchedulingResourceBound::new(
            resource_id,
            None,
            Some(max_total),
        )?);
    }

    SchedulingPeriod::new(
        period.period_label(),
        resource_bounds,
        period.destination_capacities().to_vec(),
        period.stockpile_capacities().to_vec(),
    )
}

fn candidate_destination_ids(
    problem: &SchedulingProblem,
    unit: &SchedulingUnit,
) -> Vec<ScheduleDestinationId> {
    let mut destination_ids = unit
        .eligible_destination_ids()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    for objective_term in problem.objective_terms() {
        if objective_term.unit_id() == unit.unit_id()
            && let Some(destination_id) = objective_term.destination_id()
        {
            destination_ids.insert(destination_id.clone());
        }
    }
    for requirement in problem.resource_requirements() {
        if requirement.unit_id() == unit.unit_id()
            && let Some(destination_id) = requirement.destination_id()
        {
            destination_ids.insert(destination_id.clone());
        }
    }

    destination_ids.into_iter().collect()
}

fn has_requirement_for_unit_resource(
    requirements: &[SchedulingResourceRequirement],
    unit_id: &SchedulingUnitId,
    resource_id: &SchedulingResourceId,
) -> bool {
    requirements.iter().any(|requirement| {
        requirement.unit_id() == unit_id && requirement.resource_id() == resource_id
    })
}

pub(crate) fn materialize_long_term_schedule(
    problem: &SchedulingProblem,
    solution: &SmallSchedulingSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<LongTermSchedule, MineError> {
    let units_by_id = problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().clone(), unit))
        .collect::<BTreeMap<_, _>>();
    let capacities = problem
        .periods()
        .iter()
        .map(long_term_capacity_from_period)
        .collect::<Result<Vec<_>, _>>()?;

    let mut assignments = solution.assignments().to_vec();
    assignments.sort_by(|left, right| {
        left.period_index()
            .cmp(&right.period_index())
            .then_with(|| left.unit_id().cmp(right.unit_id()))
    });

    let entries =
        assignments
            .into_iter()
            .map(|assignment| {
                let unit = units_by_id
                    .get(assignment.unit_id())
                    .copied()
                    .ok_or_else(|| MineError::Planning {
                        message: format!(
                            "scheduled unit `{}` is missing from the original SchedulingProblem",
                            assignment.unit_id()
                        ),
                    })?;
                if assignment.stockpile_inventory_delta_tonnage() < -SCHEDULING_EPSILON {
                    let destination_id = assignment.destination_id().cloned().ok_or_else(|| {
                        MineError::Planning {
                            message: format!(
                                "reclaim assignment `{}` is missing a final destination",
                                assignment.unit_id()
                            ),
                        }
                    })?;
                    let reclaim_stockpile_id =
                        assignment
                            .stockpile_id()
                            .cloned()
                            .ok_or_else(|| MineError::Planning {
                                message: format!(
                                    "reclaim assignment `{}` is missing its source stockpile",
                                    assignment.unit_id()
                                ),
                            })?;
                    LongTermScheduleEntry::new_with_reclaim(
                        assignment.period_label(),
                        Some(long_term_phase_id(unit)),
                        None,
                        None,
                        unit.tonnage(),
                        unit.block_count(),
                        Some(destination_id),
                        None,
                        Some(reclaim_stockpile_id),
                        unit.predecessor_unit_ids()
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                    )
                } else {
                    LongTermScheduleEntry::new(
                        assignment.period_label(),
                        Some(long_term_phase_id(unit)),
                        unit.shell_index(),
                        unit.bench(),
                        unit.tonnage(),
                        unit.block_count(),
                        assignment.destination_id().cloned(),
                        assignment.stockpile_id().cloned(),
                        unit.predecessor_unit_ids()
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                    )
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

    if entries.is_empty() {
        return Err(MineError::Planning {
            message: "ready frontier long-term schedule did not assign any unit".to_owned(),
        });
    }

    let mut violations = Vec::new();
    if let Some(max_vertical_advance) = max_vertical_advance {
        violations.extend(build_long_term_vertical_advance_violations(
            &entries,
            &capacities,
            max_vertical_advance,
        )?);
    }

    LongTermSchedule::new(
        problem.scenario_id().clone(),
        problem.model_id().clone(),
        entries,
        capacities,
        problem.stockpiles().to_vec(),
        violations,
        metadata,
    )
}

fn long_term_capacity_from_period(
    period: &SchedulingPeriod,
) -> Result<LongTermSchedulePeriodCapacity, MineError> {
    let max_mine_tonnage = period
        .resource_bounds()
        .iter()
        .find(|bound| bound.resource_id().as_str() == "mine_tonnage")
        .and_then(|bound| bound.max_total());
    let max_plant_tonnage = period
        .resource_bounds()
        .iter()
        .find(|bound| bound.resource_id().as_str() == "plant_tonnage")
        .and_then(|bound| bound.max_total());

    LongTermSchedulePeriodCapacity::new(
        period.period_label(),
        max_mine_tonnage,
        max_plant_tonnage,
        period.destination_capacities().to_vec(),
        period.stockpile_capacities().to_vec(),
    )
}

fn long_term_phase_id(unit: &SchedulingUnit) -> String {
    match unit.metadata().get("phase_id") {
        Some(MetadataValue::Text(phase_id)) => phase_id.clone(),
        _ => unit.unit_id().as_str().to_owned(),
    }
}

fn predecessor_periods(
    unit: &SchedulingUnit,
    scheduled_period_by_unit: &BTreeMap<SchedulingUnitId, Option<usize>>,
) -> Result<Option<usize>, MineError> {
    let mut earliest_period_index = 0usize;
    for predecessor_unit_id in unit.predecessor_unit_ids() {
        match scheduled_period_by_unit
            .get(predecessor_unit_id)
            .copied()
            .flatten()
        {
            Some(period_index) => {
                earliest_period_index = earliest_period_index.max(period_index);
            }
            None => return Ok(None),
        }
    }
    Ok(Some(earliest_period_index))
}

#[allow(clippy::too_many_arguments)]
fn select_window_candidates(
    problem: &SchedulingProblem,
    period_index: usize,
    candidate_window_size: usize,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    objective_by_unit: &ObjectiveIndex,
    requirements_by_unit: &RequirementIndex,
    stockpile_inventory_by_id: &BTreeMap<ScheduleStockpileId, f64>,
    stockpile_reclaims_by_period: &[BTreeMap<ScheduleStockpileId, f64>],
    stockpile_future_inventory_limits: &StockpileInventoryLimitIndex,
    stockpile_reclaim_limits: &StockpileInventoryLimitIndex,
    usage_by_period: &[BTreeMap<SchedulingResourceId, f64>],
    period_resource_limits: &[BTreeMap<SchedulingResourceId, (Option<f64>, Option<f64>)>],
    scheduled_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> Vec<SchedulingUnitId> {
    let mut ranked = problem
        .units()
        .iter()
        .filter(|unit| !scheduled_period_by_unit.contains_key(unit.unit_id()))
        .filter(|unit| {
            unit.predecessor_unit_ids()
                .iter()
                .all(|predecessor| scheduled_period_by_unit.contains_key(predecessor))
        })
        .filter_map(|unit| {
            let best_option = build_unit_options(unit, objective_by_unit, requirements_by_unit)
                .into_iter()
                .filter(|option| {
                    fits_upper_bounds(
                        period_index,
                        &option.requirements,
                        usage_by_period,
                        period_resource_limits,
                    ) && fits_effective_stockpile_bounds(
                        period_index,
                        option.stockpile_inventory_delta_tonnage,
                        option.stockpile_id.as_ref(),
                        stockpile_inventory_by_id,
                        stockpile_reclaims_by_period,
                        stockpile_future_inventory_limits,
                        stockpile_reclaim_limits,
                    )
                })
                .max_by(|left, right| {
                    let left_load = left
                        .requirements
                        .iter()
                        .map(|(_, amount)| *amount)
                        .sum::<f64>()
                        .max(unit.tonnage())
                        .max(1.0e-9);
                    let right_load = right
                        .requirements
                        .iter()
                        .map(|(_, amount)| *amount)
                        .sum::<f64>()
                        .max(unit.tonnage())
                        .max(1.0e-9);
                    let left_score = left.objective_value / left_load;
                    let right_score = right.objective_value / right_load;
                    left_score
                        .partial_cmp(&right_score)
                        .expect("window scores should be finite")
                        .then_with(|| {
                            left.objective_value
                                .partial_cmp(&right.objective_value)
                                .expect("window objective should be finite")
                        })
                })?;
            let load = best_option
                .requirements
                .iter()
                .map(|(_, amount)| *amount)
                .sum::<f64>()
                .max(unit.tonnage())
                .max(1.0e-9);
            let target_period = target_period_by_unit
                .get(unit.unit_id())
                .copied()
                .unwrap_or(period_index);
            Some(FrontierCandidate {
                unit_id: unit.unit_id().clone(),
                period_index,
                destination_id: best_option.destination_id,
                stockpile_id: best_option.stockpile_id,
                stockpile_inventory_delta_tonnage: best_option.stockpile_inventory_delta_tonnage,
                requirements: best_option.requirements,
                objective_value: best_option.objective_value,
                discounted_objective_value: best_option.objective_value
                    / (1.0 + problem.discount_rate()).powi(period_index as i32),
                priority_distance: period_index.abs_diff(target_period),
                score: best_option.objective_value / load,
            })
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        let left_tardiness = period_index.saturating_sub(
            target_period_by_unit
                .get(&left.unit_id)
                .copied()
                .unwrap_or(period_index),
        );
        let right_tardiness = period_index.saturating_sub(
            target_period_by_unit
                .get(&right.unit_id)
                .copied()
                .unwrap_or(period_index),
        );
        right_tardiness
            .cmp(&left_tardiness)
            .then_with(|| left.priority_distance.cmp(&right.priority_distance))
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .expect("window scores should be finite")
            })
            .then_with(|| left.unit_id.cmp(&right.unit_id))
    });

    ranked
        .into_iter()
        .take(candidate_window_size)
        .map(|candidate| candidate.unit_id)
        .collect()
}

fn best_candidate_for_unit(
    problem: &SchedulingProblem,
    unit: &SchedulingUnit,
    period_index: usize,
    objective_by_unit: &ObjectiveIndex,
    requirements_by_unit: &RequirementIndex,
    stockpile_inventory_by_id: &BTreeMap<ScheduleStockpileId, f64>,
    stockpile_reclaims_by_period: &[BTreeMap<ScheduleStockpileId, f64>],
    stockpile_future_inventory_limits: &StockpileInventoryLimitIndex,
    stockpile_reclaim_limits: &StockpileInventoryLimitIndex,
    usage_by_period: &[BTreeMap<SchedulingResourceId, f64>],
    period_resource_limits: &[BTreeMap<SchedulingResourceId, (Option<f64>, Option<f64>)>],
) -> Option<FrontierCandidate> {
    build_unit_options(unit, objective_by_unit, requirements_by_unit)
        .into_iter()
        .filter(|option| {
            fits_upper_bounds(
                period_index,
                &option.requirements,
                usage_by_period,
                period_resource_limits,
            ) && fits_effective_stockpile_bounds(
                period_index,
                option.stockpile_inventory_delta_tonnage,
                option.stockpile_id.as_ref(),
                stockpile_inventory_by_id,
                stockpile_reclaims_by_period,
                stockpile_future_inventory_limits,
                stockpile_reclaim_limits,
            )
        })
        .map(|option| {
            let discounted_objective_value =
                option.objective_value / (1.0 + problem.discount_rate()).powi(period_index as i32);
            let load = option
                .requirements
                .iter()
                .map(|(_, amount)| *amount)
                .sum::<f64>()
                .max(unit.tonnage())
                .max(1.0e-9);
            FrontierCandidate {
                unit_id: unit.unit_id().clone(),
                period_index,
                destination_id: option.destination_id,
                stockpile_id: option.stockpile_id,
                stockpile_inventory_delta_tonnage: option.stockpile_inventory_delta_tonnage,
                requirements: option.requirements,
                objective_value: option.objective_value,
                discounted_objective_value,
                priority_distance: 0,
                score: discounted_objective_value / load,
            }
        })
        .max_by(|left, right| {
            left.score
                .partial_cmp(&right.score)
                .expect("candidate scores should be finite")
                .then_with(|| {
                    left.discounted_objective_value
                        .partial_cmp(&right.discounted_objective_value)
                        .expect("candidate objective should be finite")
                })
        })
}

fn build_residual_period_for_window(
    period: &SchedulingPeriod,
    current_usage: &BTreeMap<SchedulingResourceId, f64>,
    current_reclaims: &BTreeMap<ScheduleStockpileId, f64>,
    effective_stockpile_inventory_limits: &BTreeMap<ScheduleStockpileId, f64>,
    stockpile_reclaim_limits: &BTreeMap<ScheduleStockpileId, f64>,
) -> Result<SchedulingPeriod, MineError> {
    let residual_bounds = period
        .resource_bounds()
        .iter()
        .map(|bound| {
            let residual_max_total = bound.max_total().map(|max_total| {
                (max_total
                    - current_usage
                        .get(bound.resource_id())
                        .copied()
                        .unwrap_or(0.0))
                .max(0.0)
            });
            crate::SchedulingResourceBound::new(
                bound.resource_id().clone(),
                None,
                residual_max_total,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let residual_stockpile_capacities = period
        .stockpile_capacities()
        .iter()
        .map(|capacity| {
            let max_inventory_tonnage = effective_stockpile_inventory_limits
                .get(capacity.stockpile_id())
                .copied()
                .or_else(|| capacity.max_inventory_tonnage());
            let max_reclaim_tonnage = stockpile_reclaim_limits
                .get(capacity.stockpile_id())
                .copied()
                .or_else(|| capacity.max_reclaim_tonnage())
                .map(|max_reclaim_tonnage| {
                    (max_reclaim_tonnage
                        - current_reclaims
                            .get(capacity.stockpile_id())
                            .copied()
                            .unwrap_or(0.0))
                    .max(0.0)
                });
            crate::ScheduleStockpileCapacity::new(
                capacity.stockpile_id().clone(),
                max_inventory_tonnage,
                max_reclaim_tonnage,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    SchedulingPeriod::new(
        period.period_label(),
        residual_bounds,
        period.destination_capacities().to_vec(),
        residual_stockpile_capacities,
    )
}

fn build_window_subproblem(
    problem: &SchedulingProblem,
    candidate_unit_ids: &[SchedulingUnitId],
    residual_period: SchedulingPeriod,
    stockpile_inventory_by_id: &BTreeMap<ScheduleStockpileId, f64>,
) -> Result<SchedulingProblem, MineError> {
    let candidate_ids = candidate_unit_ids.iter().cloned().collect::<BTreeSet<_>>();
    let units = problem
        .units()
        .iter()
        .filter(|unit| candidate_ids.contains(unit.unit_id()))
        .map(|unit| {
            SchedulingUnit::new(
                unit.unit_id().clone(),
                unit.tonnage(),
                unit.block_count(),
                Vec::new(),
                unit.eligible_destination_ids().to_vec(),
                unit.eligible_stockpile_ids().to_vec(),
                unit.block_indices().to_vec(),
                unit.bench(),
                unit.shell_index(),
                unit.metadata().clone(),
            )
            .and_then(|unit_clone| {
                unit_clone.with_stockpile_inventory_delta_tonnage(
                    unit.stockpile_inventory_delta_tonnage(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let objective_terms = problem
        .objective_terms()
        .iter()
        .filter(|term| candidate_ids.contains(term.unit_id()))
        .cloned()
        .collect::<Vec<_>>();
    let resource_requirements = problem
        .resource_requirements()
        .iter()
        .filter(|requirement| candidate_ids.contains(requirement.unit_id()))
        .cloned()
        .collect::<Vec<_>>();
    let stockpiles = problem
        .stockpiles()
        .iter()
        .map(|stockpile| {
            LongTermScheduleStockpile::new(
                stockpile.stockpile_id().clone(),
                stockpile_inventory_by_id
                    .get(stockpile.stockpile_id())
                    .copied()
                    .unwrap_or_else(|| stockpile.opening_tonnage()),
                stockpile.metadata().clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    SchedulingProblem::new(
        problem.scenario_id().clone(),
        problem.model_id().clone(),
        vec![residual_period],
        units,
        objective_terms,
        resource_requirements,
        problem.destination_ids().to_vec(),
        stockpiles,
        problem.discount_rate(),
        problem.metadata().clone(),
        problem.limitations().to_vec(),
    )
}

#[derive(Clone)]
struct UnitOption {
    destination_id: Option<ScheduleDestinationId>,
    stockpile_id: Option<ScheduleStockpileId>,
    stockpile_inventory_delta_tonnage: f64,
    objective_value: f64,
    requirements: Vec<(SchedulingResourceId, f64)>,
}

struct FrontierCandidate {
    unit_id: SchedulingUnitId,
    period_index: usize,
    destination_id: Option<ScheduleDestinationId>,
    stockpile_id: Option<ScheduleStockpileId>,
    stockpile_inventory_delta_tonnage: f64,
    requirements: Vec<(SchedulingResourceId, f64)>,
    objective_value: f64,
    discounted_objective_value: f64,
    priority_distance: usize,
    score: f64,
}

fn build_unit_options(
    unit: &SchedulingUnit,
    objective_by_unit: &ObjectiveIndex,
    requirements_by_unit: &RequirementIndex,
) -> Vec<UnitOption> {
    let objective_terms = objective_by_unit.get(unit.unit_id());
    let generic_objective = objective_terms
        .and_then(|terms| terms.get(&None))
        .copied()
        .unwrap_or(0.0);
    let mut destination_scope = BTreeSet::<Option<ScheduleDestinationId>>::new();
    destination_scope.insert(None);
    if let Some(objective_terms) = objective_terms {
        for destination_id in objective_terms.keys() {
            destination_scope.insert(destination_id.clone());
        }
    }
    for destination_id in unit.eligible_destination_ids() {
        destination_scope.insert(Some(destination_id.clone()));
    }
    if let Some(requirements) = requirements_by_unit.get(unit.unit_id()) {
        for (_, destination_id, _) in requirements {
            destination_scope.insert(destination_id.clone());
        }
    }
    let stockpile_inventory_delta_tonnage = unit.effective_stockpile_inventory_delta_tonnage();
    let is_reclaim_option = stockpile_inventory_delta_tonnage < -SCHEDULING_EPSILON;

    let destination_options = destination_scope
        .into_iter()
        .filter(|destination_id| {
            !is_reclaim_option
                && (destination_id.is_some()
                    || (unit.eligible_destination_ids().is_empty()
                        && unit.eligible_stockpile_ids().is_empty()))
        })
        .map(|destination_id| {
            let destination_objective = if destination_id.is_none() {
                0.0
            } else {
                objective_terms
                    .and_then(|terms| terms.get(&destination_id))
                    .copied()
                    .unwrap_or(0.0)
            };
            let requirements = requirements_by_unit
                .get(unit.unit_id())
                .map(|requirements| {
                    requirements
                        .iter()
                        .filter(|(_, requirement_destination_id, _)| {
                            requirement_destination_id.is_none()
                                || *requirement_destination_id == destination_id
                        })
                        .map(|(resource_id, _, amount)| (resource_id.clone(), *amount))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            UnitOption {
                destination_id,
                stockpile_id: None,
                stockpile_inventory_delta_tonnage: 0.0,
                objective_value: generic_objective + destination_objective,
                requirements,
            }
        })
        .collect::<Vec<_>>();
    let stockpile_options = if is_reclaim_option {
        unit.eligible_stockpile_ids()
            .iter()
            .cloned()
            .flat_map(|stockpile_id| {
                let objective_terms = objective_terms;
                destination_scope_for_reclaim(unit, requirements_by_unit, objective_by_unit)
                    .into_iter()
                    .map(move |destination_id| {
                        let destination_objective = objective_terms
                            .and_then(|terms| terms.get(&Some(destination_id.clone())))
                            .copied()
                            .unwrap_or(0.0);
                        let requirements = requirements_by_unit
                            .get(unit.unit_id())
                            .map(|requirements| {
                                requirements
                                    .iter()
                                    .filter(|(_, requirement_destination_id, _)| {
                                        requirement_destination_id.is_none()
                                            || *requirement_destination_id
                                                == Some(destination_id.clone())
                                    })
                                    .map(|(resource_id, _, amount)| (resource_id.clone(), *amount))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();

                        UnitOption {
                            destination_id: Some(destination_id.clone()),
                            stockpile_id: Some(stockpile_id.clone()),
                            stockpile_inventory_delta_tonnage,
                            objective_value: generic_objective + destination_objective,
                            requirements,
                        }
                    })
            })
            .collect::<Vec<_>>()
    } else {
        unit.eligible_stockpile_ids()
            .iter()
            .cloned()
            .map(|stockpile_id| {
                let requirements = requirements_by_unit
                    .get(unit.unit_id())
                    .map(|requirements| {
                        requirements
                            .iter()
                            .filter(|(_, requirement_destination_id, _)| {
                                requirement_destination_id.is_none()
                            })
                            .map(|(resource_id, _, amount)| (resource_id.clone(), *amount))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                UnitOption {
                    destination_id: None,
                    stockpile_id: Some(stockpile_id),
                    stockpile_inventory_delta_tonnage,
                    objective_value: generic_objective,
                    requirements,
                }
            })
            .collect::<Vec<_>>()
    };

    destination_options
        .into_iter()
        .chain(stockpile_options)
        .collect()
}

fn destination_scope_for_reclaim(
    unit: &SchedulingUnit,
    requirements_by_unit: &RequirementIndex,
    objective_by_unit: &ObjectiveIndex,
) -> Vec<ScheduleDestinationId> {
    let mut destination_scope = unit
        .eligible_destination_ids()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(objective_terms) = objective_by_unit.get(unit.unit_id()) {
        for destination_id in objective_terms.keys().flatten() {
            destination_scope.insert(destination_id.clone());
        }
    }
    if let Some(requirements) = requirements_by_unit.get(unit.unit_id()) {
        for (_, destination_id, _) in requirements {
            if let Some(destination_id) = destination_id {
                destination_scope.insert(destination_id.clone());
            }
        }
    }
    destination_scope.into_iter().collect()
}

fn index_stockpile_opening_inventory(
    problem: &SchedulingProblem,
) -> BTreeMap<ScheduleStockpileId, f64> {
    problem
        .stockpiles()
        .iter()
        .map(|stockpile| {
            (
                stockpile.stockpile_id().clone(),
                stockpile.opening_tonnage(),
            )
        })
        .collect()
}

fn index_stockpile_inventory_limits(problem: &SchedulingProblem) -> StockpileInventoryLimitIndex {
    problem
        .periods()
        .iter()
        .map(|period| {
            period
                .stockpile_capacities()
                .iter()
                .filter_map(|capacity| {
                    capacity
                        .max_inventory_tonnage()
                        .map(|limit| (capacity.stockpile_id().clone(), limit))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect()
}

fn index_stockpile_reclaim_limits(problem: &SchedulingProblem) -> StockpileInventoryLimitIndex {
    problem
        .periods()
        .iter()
        .map(|period| {
            period
                .stockpile_capacities()
                .iter()
                .filter_map(|capacity| {
                    capacity
                        .max_reclaim_tonnage()
                        .map(|limit| (capacity.stockpile_id().clone(), limit))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect()
}

fn build_future_stockpile_inventory_limits(
    stockpile_inventory_limits: &StockpileInventoryLimitIndex,
) -> StockpileInventoryLimitIndex {
    let mut future_limits =
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); stockpile_inventory_limits.len()];
    let mut next_limits = BTreeMap::<ScheduleStockpileId, f64>::new();
    for period_index in (0..stockpile_inventory_limits.len()).rev() {
        let mut period_limits = stockpile_inventory_limits[period_index].clone();
        for (stockpile_id, next_limit) in &next_limits {
            period_limits
                .entry(stockpile_id.clone())
                .and_modify(|limit| *limit = limit.min(*next_limit))
                .or_insert(*next_limit);
        }
        next_limits = period_limits.clone();
        future_limits[period_index] = period_limits;
    }
    future_limits
}

fn fits_stockpile_inventory_bounds(
    period_index: usize,
    inventory_delta_tonnage: f64,
    stockpile_id: Option<&ScheduleStockpileId>,
    stockpile_inventory_delta_by_period: &[BTreeMap<ScheduleStockpileId, f64>],
    stockpile_reclaims_by_period: &[BTreeMap<ScheduleStockpileId, f64>],
    stockpile_opening_inventory: &BTreeMap<ScheduleStockpileId, f64>,
    stockpile_inventory_limits: &StockpileInventoryLimitIndex,
    stockpile_reclaim_limits: &StockpileInventoryLimitIndex,
) -> bool {
    let Some(stockpile_id) = stockpile_id else {
        return true;
    };
    let mut inventory = stockpile_opening_inventory
        .get(stockpile_id)
        .copied()
        .unwrap_or(0.0);
    for current_period_index in 0..stockpile_inventory_limits.len() {
        inventory += stockpile_inventory_delta_by_period[current_period_index]
            .get(stockpile_id)
            .copied()
            .unwrap_or(0.0);
        if current_period_index == period_index {
            inventory += inventory_delta_tonnage;
        }
        if inventory < -SCHEDULING_EPSILON {
            return false;
        }
        if current_period_index >= period_index
            && let Some(limit) = stockpile_inventory_limits[current_period_index].get(stockpile_id)
            && inventory > *limit + SCHEDULING_EPSILON
        {
            return false;
        }
    }
    if inventory_delta_tonnage < -SCHEDULING_EPSILON
        && let Some(limit) = stockpile_reclaim_limits[period_index].get(stockpile_id)
        && stockpile_reclaims_by_period[period_index]
            .get(stockpile_id)
            .copied()
            .unwrap_or(0.0)
            + inventory_delta_tonnage.abs()
            > *limit + SCHEDULING_EPSILON
    {
        return false;
    }
    true
}

fn fits_effective_stockpile_bounds(
    period_index: usize,
    inventory_delta_tonnage: f64,
    stockpile_id: Option<&ScheduleStockpileId>,
    stockpile_inventory_by_id: &BTreeMap<ScheduleStockpileId, f64>,
    stockpile_reclaims_by_period: &[BTreeMap<ScheduleStockpileId, f64>],
    stockpile_future_inventory_limits: &StockpileInventoryLimitIndex,
    stockpile_reclaim_limits: &StockpileInventoryLimitIndex,
) -> bool {
    let Some(stockpile_id) = stockpile_id else {
        return true;
    };
    let Some(limit) = stockpile_future_inventory_limits[period_index].get(stockpile_id) else {
        return false;
    };
    let updated_inventory = stockpile_inventory_by_id
        .get(stockpile_id)
        .copied()
        .unwrap_or(0.0)
        + inventory_delta_tonnage;
    if updated_inventory < -SCHEDULING_EPSILON || updated_inventory > *limit + SCHEDULING_EPSILON {
        return false;
    }
    if inventory_delta_tonnage < -SCHEDULING_EPSILON
        && let Some(reclaim_limit) = stockpile_reclaim_limits[period_index].get(stockpile_id)
        && stockpile_reclaims_by_period[period_index]
            .get(stockpile_id)
            .copied()
            .unwrap_or(0.0)
            + inventory_delta_tonnage.abs()
            > *reclaim_limit + SCHEDULING_EPSILON
    {
        return false;
    }
    true
}

fn apply_stockpile_inventory_delta(
    period_index: usize,
    inventory_delta_tonnage: f64,
    stockpile_id: Option<&ScheduleStockpileId>,
    stockpile_inventory_delta_by_period: &mut [BTreeMap<ScheduleStockpileId, f64>],
    direction: f64,
) {
    let Some(stockpile_id) = stockpile_id else {
        return;
    };
    let updated_total = stockpile_inventory_delta_by_period[period_index]
        .get(stockpile_id)
        .copied()
        .unwrap_or(0.0)
        + direction * inventory_delta_tonnage;
    if updated_total.abs() <= SCHEDULING_EPSILON {
        stockpile_inventory_delta_by_period[period_index].remove(stockpile_id);
    } else {
        stockpile_inventory_delta_by_period[period_index]
            .insert(stockpile_id.clone(), updated_total);
    }
}

fn apply_stockpile_reclaim(
    period_index: usize,
    inventory_delta_tonnage: f64,
    stockpile_id: Option<&ScheduleStockpileId>,
    stockpile_reclaims_by_period: &mut [BTreeMap<ScheduleStockpileId, f64>],
    direction: f64,
) {
    if inventory_delta_tonnage >= -SCHEDULING_EPSILON {
        return;
    }
    let Some(stockpile_id) = stockpile_id else {
        return;
    };
    let updated_total = stockpile_reclaims_by_period[period_index]
        .get(stockpile_id)
        .copied()
        .unwrap_or(0.0)
        + direction * inventory_delta_tonnage.abs();
    if updated_total.abs() <= SCHEDULING_EPSILON {
        stockpile_reclaims_by_period[period_index].remove(stockpile_id);
    } else {
        stockpile_reclaims_by_period[period_index].insert(stockpile_id.clone(), updated_total);
    }
}

fn apply_stockpile_inventory(
    stockpile_id: Option<&ScheduleStockpileId>,
    inventory_delta_tonnage: f64,
    stockpile_inventory_by_id: &mut BTreeMap<ScheduleStockpileId, f64>,
    direction: f64,
) {
    let Some(stockpile_id) = stockpile_id else {
        return;
    };
    let updated_inventory = stockpile_inventory_by_id
        .get(stockpile_id)
        .copied()
        .unwrap_or(0.0)
        + direction * inventory_delta_tonnage;
    if updated_inventory.abs() <= SCHEDULING_EPSILON {
        stockpile_inventory_by_id.remove(stockpile_id);
    } else {
        stockpile_inventory_by_id.insert(stockpile_id.clone(), updated_inventory);
    }
}

fn fits_upper_bounds(
    period_index: usize,
    requirements: &[(SchedulingResourceId, f64)],
    usage_by_period: &[BTreeMap<SchedulingResourceId, f64>],
    period_resource_limits: &[BTreeMap<SchedulingResourceId, (Option<f64>, Option<f64>)>],
) -> bool {
    requirements.iter().all(|(resource_id, amount)| {
        let Some((_, max_total)) = period_resource_limits[period_index].get(resource_id) else {
            return false;
        };
        let current_usage = usage_by_period[period_index]
            .get(resource_id)
            .copied()
            .unwrap_or(0.0);
        max_total.is_none_or(|max_total| current_usage + amount <= max_total + 1.0e-9)
    })
}

fn apply_requirements(
    period_index: usize,
    requirements: &[(SchedulingResourceId, f64)],
    usage_by_period: &mut [BTreeMap<SchedulingResourceId, f64>],
    sign: f64,
) {
    for (resource_id, amount) in requirements {
        let entry = usage_by_period[period_index]
            .entry(resource_id.clone())
            .or_insert(0.0);
        *entry += sign * amount;
    }
}

fn period_lower_bounds_satisfied(
    usage_by_period: &[BTreeMap<SchedulingResourceId, f64>],
    period_resource_limits: &[BTreeMap<SchedulingResourceId, (Option<f64>, Option<f64>)>],
) -> bool {
    period_resource_limits
        .iter()
        .enumerate()
        .all(|(period_index, period_limits)| {
            period_limits.iter().all(|(resource_id, (min_total, _))| {
                min_total.is_none_or(|min_total| {
                    usage_by_period[period_index]
                        .get(resource_id)
                        .copied()
                        .unwrap_or(0.0)
                        >= min_total - 1.0e-9
                })
            })
        })
}

fn materialize_solution(
    problem: &SchedulingProblem,
    solution: SearchState,
    period_resource_limits: &[BTreeMap<SchedulingResourceId, (Option<f64>, Option<f64>)>],
) -> SmallSchedulingSolution {
    let stockpile_delta_by_period = solution.assignments.iter().fold(
        vec![BTreeMap::<ScheduleStockpileId, f64>::new(); problem.periods().len()],
        |mut indexed, assignment| {
            if let Some(stockpile_id) = assignment.stockpile_id() {
                *indexed[assignment.period_index()]
                    .entry(stockpile_id.clone())
                    .or_insert(0.0) += assignment.stockpile_inventory_delta_tonnage();
            }
            indexed
        },
    );
    let scheduled_unit_ids = solution
        .assignments
        .iter()
        .map(SmallSchedulingAssignment::unit_id)
        .cloned()
        .collect::<BTreeSet<_>>();
    let skipped_unit_ids = problem
        .units()
        .iter()
        .filter(|unit| !scheduled_unit_ids.contains(unit.unit_id()))
        .map(SchedulingUnit::unit_id)
        .cloned()
        .collect::<Vec<_>>();
    let mut stockpile_inventory_by_id = index_stockpile_opening_inventory(problem);
    let periods = problem
        .periods()
        .iter()
        .enumerate()
        .map(|(period_index, period)| {
            let capacity_by_stockpile = period
                .stockpile_capacities()
                .iter()
                .map(|capacity| {
                    (
                        capacity.stockpile_id().clone(),
                        capacity.max_inventory_tonnage(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let stockpile_ids = stockpile_inventory_by_id
                .keys()
                .chain(stockpile_delta_by_period[period_index].keys())
                .chain(capacity_by_stockpile.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            let stockpile_usage = stockpile_ids
                .into_iter()
                .filter_map(|stockpile_id| {
                    let opening_tonnage = stockpile_inventory_by_id
                        .get(&stockpile_id)
                        .copied()
                        .unwrap_or(0.0);
                    let inventory_delta_tonnage = stockpile_delta_by_period[period_index]
                        .get(&stockpile_id)
                        .copied()
                        .unwrap_or(0.0);
                    let closing_tonnage = opening_tonnage + inventory_delta_tonnage;
                    stockpile_inventory_by_id.insert(stockpile_id.clone(), closing_tonnage);

                    if opening_tonnage.abs() <= SCHEDULING_EPSILON
                        && inventory_delta_tonnage.abs() <= SCHEDULING_EPSILON
                        && !capacity_by_stockpile.contains_key(&stockpile_id)
                    {
                        None
                    } else {
                        let max_inventory_tonnage =
                            capacity_by_stockpile.get(&stockpile_id).copied().flatten();
                        Some(SmallSchedulingStockpileUsage {
                            stockpile_id,
                            opening_tonnage,
                            inventory_delta_tonnage,
                            closing_tonnage,
                            max_inventory_tonnage,
                        })
                    }
                })
                .collect();

            SmallSchedulingPeriodSummary {
                period_label: period.period_label().to_owned(),
                assignment_count: solution
                    .assignments
                    .iter()
                    .filter(|assignment| assignment.period_index() == period_index)
                    .count(),
                resource_usage: period_resource_limits[period_index]
                    .iter()
                    .map(
                        |(resource_id, (min_total, max_total))| SmallSchedulingResourceUsage {
                            resource_id: resource_id.clone(),
                            total: solution.usage_by_period[period_index]
                                .get(resource_id)
                                .copied()
                                .unwrap_or(0.0),
                            min_total: *min_total,
                            max_total: *max_total,
                        },
                    )
                    .collect(),
                stockpile_usage,
            }
        })
        .collect::<Vec<_>>();

    SmallSchedulingSolution {
        assignments: solution.assignments,
        skipped_unit_ids,
        total_objective_value: solution.total_objective_value,
        total_discounted_objective_value: solution.total_discounted_objective_value,
        periods,
    }
}

fn index_objective_terms(objective_terms: &[SchedulingObjectiveTerm]) -> ObjectiveIndex {
    let mut indexed =
        BTreeMap::<SchedulingUnitId, BTreeMap<Option<ScheduleDestinationId>, f64>>::new();
    for objective_term in objective_terms {
        indexed
            .entry(objective_term.unit_id().clone())
            .or_default()
            .insert(
                objective_term.destination_id().cloned(),
                objective_term.value(),
            );
    }
    indexed
}

fn index_resource_requirements(
    resource_requirements: &[SchedulingResourceRequirement],
) -> RequirementIndex {
    let mut indexed = BTreeMap::<
        SchedulingUnitId,
        Vec<(SchedulingResourceId, Option<ScheduleDestinationId>, f64)>,
    >::new();
    for requirement in resource_requirements {
        indexed
            .entry(requirement.unit_id().clone())
            .or_default()
            .push((
                requirement.resource_id().clone(),
                requirement.destination_id().cloned(),
                requirement.amount(),
            ));
    }
    indexed
}
