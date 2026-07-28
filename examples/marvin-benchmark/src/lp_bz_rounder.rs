//! Round/repair determinista LP-guided para el benchmark Marvin (MR-187).
//!
//! Este módulo mantiene una separación explícita:
//! - redondeo y reparación de periodos objetivo (determinista),
//! - construcción opcional del schedule usando helpers del SDK para preservar
//!   verificaciones de factibilidad de precedencia y recursos.
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;

use crate::marvin_support::MarvinScheduleSolution;
use mine_sdk::{
    LongTermSchedule, Metadata, MineError, PushbackPlan, SchedulingProblem, SchedulingUnitId,
    build_target_period_seeded_long_term_schedule,
};
use serde::Serialize;

/// Resultado del redondeo/reparación a nivel de unidades de scheduling.
#[derive(Debug, Clone, PartialEq)]
pub struct LpBzUnitRoundRepairResult {
    /// Score proxy explícito por etapa `round -> repair -> local-search`.
    pub target_score_decomposition: LpBzUnitTargetScoreDecomposition,
    /// Periodo objetivo entero y factible por precedencia para cada unidad.
    pub target_period_by_unit: BTreeMap<SchedulingUnitId, usize>,
    /// Cantidad de unidades cuyo periodo se retrasó para respetar precedencias.
    pub repaired_unit_target_count: usize,
    /// Cantidad de unidades cuyo target se truncó al último periodo disponible.
    pub horizon_clamp_count: usize,
    /// Cantidad de movimientos locales deterministas aplicados para mejorar score descontado.
    pub local_improvement_move_count: usize,
    /// Diagnósticos explícitos del optimizador local determinista.
    pub local_optimizer_diagnostics: LpBzLocalOptimizerDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzUnitTargetScoreDecomposition {
    pub rounded_discounted_target_score_proxy: f64,
    pub repaired_discounted_target_score_proxy: f64,
    pub local_search_discounted_target_score_proxy: f64,
    pub repair_score_delta_vs_round_proxy: f64,
    pub local_search_score_delta_vs_repair_proxy: f64,
    pub local_search_score_delta_vs_round_proxy: f64,
}

/// Diagnósticos del optimizador local aplicado tras el round/repair topológico.
#[derive(Debug, Clone, PartialEq)]
pub struct LpBzLocalOptimizerDiagnostics {
    /// Etiqueta de la estrategia local usada por el rounder.
    pub strategy_label: String,
    /// Perfil explícito del presupuesto local usado por la ruta LP/BZ.
    pub budget_profile: LpBzLocalOptimizerBudgetProfile,
    /// Cota dura de iteraciones evaluadas por el optimizador.
    pub max_iteration_count: usize,
    /// Iteraciones efectivamente ejecutadas por el optimizador.
    pub executed_iteration_count: usize,
    /// Cantidad de swaps con mejora descontada estricta aplicados.
    pub improving_move_count: usize,
    /// Causa de término del optimizador local.
    pub termination_reason: String,
    /// Mejor oportunidad residual inmediata cuando el presupuesto corta la búsqueda.
    pub residual_opportunity: LpBzLocalOptimizerResidualOpportunity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLocalOptimizerBudgetProfile {
    pub mode_label: String,
    pub target_unit_count: usize,
    pub horizon_period_count: usize,
    pub full_iteration_budget: usize,
    pub requested_iteration_budget: usize,
    pub effective_iteration_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLocalOptimizerResidualOpportunity {
    pub improving_move_available: bool,
    pub move_kind_label: String,
    pub discounted_gain: f64,
}

/// Artefactos del pipeline LP-guided de round/repair benchmark-side.
#[derive(Debug, Clone, PartialEq)]
pub struct LpBzRoundRepairArtifacts {
    /// Periodo representativo fraccional por bloque derivado de la solución LP.
    pub representative_period_by_block: BTreeMap<usize, f64>,
    /// Periodo objetivo entero y factible por precedencia para cada fase.
    pub phase_target_period_by_phase: BTreeMap<String, usize>,
    /// Resultado de round/repair a nivel unidad.
    pub unit_round_repair: LpBzUnitRoundRepairResult,
    /// Cantidad de fases retrasadas por reparación de precedencia.
    pub repaired_phase_target_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct PhasePriorityMetrics {
    representative_period: f64,
    revenue_factor: f64,
    pushback_index: usize,
    bench: i64,
    distribution_skew: f64,
    confidence: f64,
    dominant_destination_share: f64,
}

#[derive(Debug, Clone, Copy)]
struct UnitPriorityMetrics {
    rounded_target: usize,
    clamped_target: usize,
    discounted_lp_score: f64,
    objective_score: f64,
    successor_count: usize,
}

#[derive(Debug, Clone)]
struct LpBzLocalSwapMove {
    pull_forward_unit_id: SchedulingUnitId,
    push_back_unit_id: SchedulingUnitId,
    lower_period: usize,
    upper_period: usize,
    discounted_gain: f64,
}

#[derive(Debug, Clone)]
struct LpBzLocalChainMove {
    anchor_unit_id: SchedulingUnitId,
    anchor_target_period: usize,
    projected_period_by_unit: BTreeMap<SchedulingUnitId, usize>,
    discounted_gain: f64,
}

#[derive(Debug, Clone)]
struct LpBzLocalPathMove {
    anchor_unit_id: SchedulingUnitId,
    anchor_target_period: usize,
    projected_period_by_unit: BTreeMap<SchedulingUnitId, usize>,
    discounted_gain: f64,
    moved_unit_sequence: Vec<SchedulingUnitId>,
}

#[derive(Debug, Clone)]
enum LpBzLocalMove {
    Swap(LpBzLocalSwapMove),
    Path(LpBzLocalPathMove),
    Chain(LpBzLocalChainMove),
}

#[derive(Debug, Clone)]
struct LpBlockFractionalProfile {
    representative_period: f64,
    total_fraction: f64,
    period_mass_by_period: BTreeMap<usize, f64>,
    destination_mass_by_destination: BTreeMap<usize, f64>,
}

#[derive(Debug, Clone, Copy)]
struct LpPhaseFractionalSignal {
    representative_period: f64,
    lower_mass_share: f64,
    upper_mass_share: f64,
    floor_mass_share: f64,
    ceil_mass_share: f64,
    distribution_skew: f64,
    confidence: f64,
    dominant_destination_share: f64,
}

const LOCAL_CHAIN_NEIGHBORHOOD_RADIUS: usize = 2;
const LOCAL_PERIOD_EJECTION_NEIGHBORHOOD_RADIUS: usize = 3;
const LOCAL_PERIOD_EJECTION_BRANCH_LIMIT: usize = 3;
const FOCUSED_REFRESH_LOCAL_OPTIMIZER_MIN_ITERATIONS: usize = 4;
const FOCUSED_REFRESH_LOCAL_OPTIMIZER_MAX_ITERATIONS: usize = 24;
pub const FOCUSED_REFRESH_SKIPPED_LOCAL_OPTIMIZER_REASON: &str = "skipped-focused-refresh-runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalOptimizationMode {
    Enabled,
    FocusedRefreshBudgeted,
    SkippedFocusedRefresh,
}

/// Calcula el periodo representativo fraccional por bloque desde asignaciones LP.
pub fn representative_period_by_block(
    lp_solution: &MarvinScheduleSolution,
) -> BTreeMap<usize, f64> {
    build_lp_block_fractional_profile_by_block(lp_solution)
        .into_iter()
        .map(|(linear_index, profile)| (linear_index, profile.representative_period))
        .collect()
}

fn build_lp_block_fractional_profile_by_block(
    lp_solution: &MarvinScheduleSolution,
) -> BTreeMap<usize, LpBlockFractionalProfile> {
    let mut weighted_period_sum_by_block = BTreeMap::<usize, f64>::new();
    let mut total_fraction_by_block = BTreeMap::<usize, f64>::new();
    let mut period_mass_by_block = BTreeMap::<usize, BTreeMap<usize, f64>>::new();
    let mut destination_mass_by_block = BTreeMap::<usize, BTreeMap<usize, f64>>::new();
    for assignment in lp_solution
        .assignments
        .iter()
        .filter(|assignment| assignment.fraction > 1.0e-9)
    {
        *weighted_period_sum_by_block
            .entry(assignment.linear_index)
            .or_insert(0.0) += assignment.period_index as f64 * assignment.fraction;
        *total_fraction_by_block
            .entry(assignment.linear_index)
            .or_insert(0.0) += assignment.fraction;
        *period_mass_by_block
            .entry(assignment.linear_index)
            .or_default()
            .entry(assignment.period_index)
            .or_insert(0.0) += assignment.fraction;
        *destination_mass_by_block
            .entry(assignment.linear_index)
            .or_default()
            .entry(assignment.destination_index)
            .or_insert(0.0) += assignment.fraction;
    }

    total_fraction_by_block
        .into_iter()
        .map(|(linear_index, total_fraction)| {
            let representative_period = weighted_period_sum_by_block
                .get(&linear_index)
                .copied()
                .unwrap_or(0.0)
                / total_fraction.max(1.0e-9);
            (
                linear_index,
                LpBlockFractionalProfile {
                    representative_period,
                    total_fraction,
                    period_mass_by_period: period_mass_by_block
                        .remove(&linear_index)
                        .unwrap_or_default(),
                    destination_mass_by_destination: destination_mass_by_block
                        .remove(&linear_index)
                        .unwrap_or_default(),
                },
            )
        })
        .collect()
}

/// Pipeline determinista LP-guided v6:
/// 1) periodo representativo por bloque,
/// 2) redondeo/reparación por fase con prioridad LP determinista,
/// 3) proyección por unidad con prioridad por score LP/objetivo descontado,
/// 4) redondeo/reparación topológica por unidad,
/// 5) optimización local determinista por swaps adyacentes, ejection chains por periodo
///    y cadenas de precedencia acotadas.
pub fn build_lp_guided_round_repair_targets_v6(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
) -> Result<LpBzRoundRepairArtifacts, MineError> {
    build_lp_guided_round_repair_targets_v6_with_local_optimization(
        phase_plan,
        scheduling_problem,
        lp_solution,
        LocalOptimizationMode::Enabled,
    )
}

fn build_lp_guided_round_repair_targets_v6_with_local_optimization(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    local_optimization_mode: LocalOptimizationMode,
) -> Result<LpBzRoundRepairArtifacts, MineError> {
    let lp_fractional_profile_by_block = build_lp_block_fractional_profile_by_block(lp_solution);
    let representative_period_by_block = lp_fractional_profile_by_block
        .iter()
        .map(|(linear_index, profile)| (*linear_index, profile.representative_period))
        .collect::<BTreeMap<_, _>>();
    let phase_representative_period_by_phase =
        build_phase_representative_period_by_phase(phase_plan, &representative_period_by_block);
    let phase_fractional_signal_by_phase =
        build_phase_fractional_signal_by_phase(phase_plan, &lp_fractional_profile_by_block);
    let (phase_target_period_by_phase, repaired_phase_target_count) =
        round_and_repair_phase_target_periods_from_representatives(
            phase_plan,
            &phase_representative_period_by_phase,
            Some(&phase_fractional_signal_by_phase),
        )?;
    let fractional_target_period_by_unit = build_fractional_unit_targets_from_phase_targets(
        scheduling_problem,
        &phase_target_period_by_phase,
        &phase_representative_period_by_phase,
        &phase_fractional_signal_by_phase,
    )?;
    let unit_round_repair = round_and_repair_unit_target_periods_with_local_optimization(
        scheduling_problem,
        &fractional_target_period_by_unit,
        local_optimization_mode,
    )?;

    Ok(LpBzRoundRepairArtifacts {
        representative_period_by_block,
        phase_target_period_by_phase,
        unit_round_repair,
        repaired_phase_target_count,
    })
}

/// Compatibilidad con naming previo.
pub fn build_lp_guided_round_repair_targets_v5(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
) -> Result<LpBzRoundRepairArtifacts, MineError> {
    build_lp_guided_round_repair_targets_v6(phase_plan, scheduling_problem, lp_solution)
}

/// Compatibilidad con naming previo.
pub fn build_lp_guided_round_repair_targets_v4(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
) -> Result<LpBzRoundRepairArtifacts, MineError> {
    build_lp_guided_round_repair_targets_v6(phase_plan, scheduling_problem, lp_solution)
}

/// Compatibilidad con naming previo.
pub fn build_lp_guided_round_repair_targets_v3(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
) -> Result<LpBzRoundRepairArtifacts, MineError> {
    build_lp_guided_round_repair_targets_v6(phase_plan, scheduling_problem, lp_solution)
}

/// Compatibilidad: el entrypoint principal delega al pipeline v6.
pub fn build_lp_guided_round_repair_targets(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
) -> Result<LpBzRoundRepairArtifacts, MineError> {
    build_lp_guided_round_repair_targets_v6(phase_plan, scheduling_problem, lp_solution)
}

/// Construye el schedule seeded usando el helper del SDK para preservar
/// verificaciones existentes de factibilidad (precedencia/recursos) con rounder v6.
pub fn build_target_period_seeded_schedule_from_lp_round_repair_v6(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    let artifacts =
        build_lp_guided_round_repair_targets_v6(phase_plan, scheduling_problem, lp_solution)?;
    let schedule = build_target_period_seeded_long_term_schedule(
        scheduling_problem,
        &artifacts.unit_round_repair.target_period_by_unit,
        max_vertical_advance,
        metadata,
    )?;
    Ok((artifacts, schedule))
}

/// Variante focalizada: mantiene el round/repair topológico y el schedule seeded
/// del refresh MR-187, pero ahora sí ejecuta el optimizador local v8 con un
/// presupuesto explícito y no trivial.
pub fn build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    let artifacts = build_lp_guided_round_repair_targets_v6_with_local_optimization(
        phase_plan,
        scheduling_problem,
        lp_solution,
        LocalOptimizationMode::FocusedRefreshBudgeted,
    )?;
    let schedule = build_target_period_seeded_long_term_schedule(
        scheduling_problem,
        &artifacts.unit_round_repair.target_period_by_unit,
        max_vertical_advance,
        metadata,
    )?;
    Ok((artifacts, schedule))
}

/// Compatibilidad con naming previo.
pub fn build_target_period_seeded_schedule_from_lp_round_repair_v5(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    build_target_period_seeded_schedule_from_lp_round_repair_v6(
        phase_plan,
        scheduling_problem,
        lp_solution,
        max_vertical_advance,
        metadata,
    )
}

/// Compatibilidad con naming previo.
pub fn build_target_period_seeded_schedule_from_lp_round_repair_v4(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    build_target_period_seeded_schedule_from_lp_round_repair_v6(
        phase_plan,
        scheduling_problem,
        lp_solution,
        max_vertical_advance,
        metadata,
    )
}

/// Compatibilidad con naming previo.
pub fn build_target_period_seeded_schedule_from_lp_round_repair_v3(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    build_target_period_seeded_schedule_from_lp_round_repair_v6(
        phase_plan,
        scheduling_problem,
        lp_solution,
        max_vertical_advance,
        metadata,
    )
}

/// Construye el schedule seeded usando el helper del SDK para preservar
/// verificaciones existentes de factibilidad (precedencia/recursos).
pub fn build_target_period_seeded_schedule_from_lp_round_repair_v2(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    build_target_period_seeded_schedule_from_lp_round_repair_v6(
        phase_plan,
        scheduling_problem,
        lp_solution,
        max_vertical_advance,
        metadata,
    )
}

/// Compatibilidad con el nombre previo: delega al pipeline topological round/repair v2.
pub fn build_target_period_seeded_schedule_from_lp_round_repair(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    lp_solution: &MarvinScheduleSolution,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<(LpBzRoundRepairArtifacts, LongTermSchedule), MineError> {
    build_target_period_seeded_schedule_from_lp_round_repair_v2(
        phase_plan,
        scheduling_problem,
        lp_solution,
        max_vertical_advance,
        metadata,
    )
}

/// Redondea periodos objetivo por fase y aplica reparación de precedencia
/// usando orden topológico explícito con prioridad LP determinista.
pub fn round_and_repair_phase_target_periods(
    phase_plan: &PushbackPlan,
    representative_period_by_block: &BTreeMap<usize, f64>,
) -> Result<(BTreeMap<String, usize>, usize), MineError> {
    let phase_representative_period_by_phase =
        build_phase_representative_period_by_phase(phase_plan, representative_period_by_block);
    round_and_repair_phase_target_periods_from_representatives(
        phase_plan,
        &phase_representative_period_by_phase,
        None,
    )
}

fn round_and_repair_phase_target_periods_from_representatives(
    phase_plan: &PushbackPlan,
    phase_representative_period_by_phase: &BTreeMap<String, f64>,
    phase_fractional_signal_by_phase: Option<&BTreeMap<String, LpPhaseFractionalSignal>>,
) -> Result<(BTreeMap<String, usize>, usize), MineError> {
    let lp_rounded_target_by_phase: BTreeMap<String, usize> = phase_plan
        .phases
        .iter()
        .map(|phase| {
            let legacy_representative_period = phase_representative_period_by_phase
                .get(&phase.phase_id)
                .copied()
                .unwrap_or(0.0);
            let phase_signal = phase_fractional_signal_by_phase
                .and_then(|signal_by_phase| signal_by_phase.get(&phase.phase_id))
                .copied();
            let representative_period =
                effective_phase_representative_period(legacy_representative_period, phase_signal);
            (
                phase.phase_id.clone(),
                round_period_index_with_signal(representative_period, phase_signal),
            )
        })
        .collect();
    let phase_priority_by_phase: BTreeMap<String, PhasePriorityMetrics> = phase_plan
        .phases
        .iter()
        .map(|phase| {
            let legacy_representative_period = phase_representative_period_by_phase
                .get(&phase.phase_id)
                .copied()
                .unwrap_or(0.0);
            let phase_signal = phase_fractional_signal_by_phase
                .and_then(|signal_by_phase| signal_by_phase.get(&phase.phase_id))
                .copied();
            let representative_period =
                effective_phase_representative_period(legacy_representative_period, phase_signal);
            let phase_signal = phase_signal.unwrap_or(LpPhaseFractionalSignal {
                representative_period,
                lower_mass_share: 0.0,
                upper_mass_share: 0.0,
                floor_mass_share: 0.0,
                ceil_mass_share: 0.0,
                distribution_skew: 0.0,
                confidence: 0.0,
                dominant_destination_share: 0.0,
            });
            (
                phase.phase_id.clone(),
                PhasePriorityMetrics {
                    representative_period,
                    revenue_factor: phase.revenue_factor.unwrap_or(1.0),
                    pushback_index: phase.pushback_index,
                    bench: phase.bench.unwrap_or(i64::MIN),
                    distribution_skew: phase_signal.distribution_skew,
                    confidence: phase_signal.confidence,
                    dominant_destination_share: phase_signal.dominant_destination_share,
                },
            )
        })
        .collect();
    let mut rounded_target_by_phase = BTreeMap::<String, usize>::new();
    let mut pending_predecessor_count_by_phase = BTreeMap::<String, usize>::new();
    let mut successor_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    for phase in &phase_plan.phases {
        pending_predecessor_count_by_phase.insert(phase.phase_id.clone(), 0usize);
    }

    for phase in &phase_plan.phases {
        for predecessor_phase_id in &phase.predecessor_phase_ids {
            if !pending_predecessor_count_by_phase.contains_key(predecessor_phase_id) {
                return Err(MineError::Planning {
                    message: format!(
                        "phase `{}` references unknown predecessor `{predecessor_phase_id}`",
                        phase.phase_id
                    ),
                });
            }
            if let Some(pending_count) = pending_predecessor_count_by_phase.get_mut(&phase.phase_id)
            {
                *pending_count += 1;
            }
            successor_phase_ids_by_phase
                .entry(predecessor_phase_id.clone())
                .or_default()
                .push(phase.phase_id.clone());
        }
    }

    let mut ready_phase_ids: Vec<String> = pending_predecessor_count_by_phase
        .iter()
        .filter(|(_, pending_count)| **pending_count == 0)
        .map(|(phase_id, _)| phase_id.clone())
        .collect();
    sort_ready_phase_ids(
        &mut ready_phase_ids,
        &lp_rounded_target_by_phase,
        &phase_priority_by_phase,
    );

    let phase_by_id: BTreeMap<String, _> = phase_plan
        .phases
        .iter()
        .map(|phase| (phase.phase_id.clone(), phase))
        .collect();
    let mut repaired_count = 0usize;
    let mut ordered_phase_count = 0usize;
    while let Some(phase_id) = ready_phase_ids.first().cloned() {
        ready_phase_ids.remove(0);
        let phase = phase_by_id
            .get(&phase_id)
            .ok_or_else(|| MineError::Planning {
                message: format!("phase `{phase_id}` was not found while repairing LP targets"),
            })?;
        let rounded_target = lp_rounded_target_by_phase
            .get(&phase_id)
            .copied()
            .unwrap_or(0usize);
        let predecessor_target = phase
            .predecessor_phase_ids
            .iter()
            .map(|predecessor_phase_id| {
                rounded_target_by_phase.get(predecessor_phase_id).copied().ok_or_else(|| {
                    MineError::Planning {
                        message: format!(
                            "phase `{}` could not be repaired because predecessor `{predecessor_phase_id}` has no repaired target",
                            phase.phase_id
                        ),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
        let repaired_target = rounded_target.max(predecessor_target);
        if repaired_target > rounded_target {
            repaired_count += 1;
        }
        rounded_target_by_phase.insert(phase.phase_id.clone(), repaired_target);

        if let Some(successor_phase_ids) = successor_phase_ids_by_phase.get(&phase_id) {
            for successor_phase_id in successor_phase_ids {
                let pending_count = pending_predecessor_count_by_phase
                    .get_mut(successor_phase_id)
                    .ok_or_else(|| MineError::Planning {
                        message: format!(
                            "phase `{successor_phase_id}` was not found while updating predecessor counts"
                        ),
                    })?;
                if *pending_count == 0 {
                    return Err(MineError::Planning {
                        message: format!(
                            "phase `{successor_phase_id}` predecessor count underflowed while processing `{phase_id}`"
                        ),
                    });
                }
                *pending_count -= 1;
                if *pending_count == 0 {
                    ready_phase_ids.push(successor_phase_id.clone());
                }
            }
            sort_ready_phase_ids(
                &mut ready_phase_ids,
                &lp_rounded_target_by_phase,
                &phase_priority_by_phase,
            );
        }
        ordered_phase_count += 1;
    }

    if ordered_phase_count != phase_plan.phases.len() {
        return Err(MineError::Planning {
            message: format!(
                "phase precedence graph is cyclic or disconnected from LP-guided topo round/repair: repaired {ordered_phase_count} of {} phases",
                phase_plan.phases.len()
            ),
        });
    }

    Ok((rounded_target_by_phase, repaired_count))
}

/// Redondea periodos objetivo por unidad y aplica reparación de precedencia
/// usando orden topológico explícito con prioridad LP determinista.
///
/// Invariantes:
/// - todos los targets deben ser finitos y no negativos;
/// - `scheduling_problem` debe tener al menos un periodo;
pub fn round_and_repair_unit_target_periods(
    scheduling_problem: &SchedulingProblem,
    fractional_target_period_by_unit: &BTreeMap<SchedulingUnitId, f64>,
) -> Result<LpBzUnitRoundRepairResult, MineError> {
    round_and_repair_unit_target_periods_with_local_optimization(
        scheduling_problem,
        fractional_target_period_by_unit,
        LocalOptimizationMode::Enabled,
    )
}

fn round_and_repair_unit_target_periods_with_local_optimization(
    scheduling_problem: &SchedulingProblem,
    fractional_target_period_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    local_optimization_mode: LocalOptimizationMode,
) -> Result<LpBzUnitRoundRepairResult, MineError> {
    if scheduling_problem.periods().is_empty() {
        return Err(MineError::Planning {
            message: "LP-guided round/repair requires at least one scheduling period".to_owned(),
        });
    }

    let discount_factor = 1.0 + scheduling_problem.discount_rate();
    let horizon_last_period = scheduling_problem.periods().len() - 1;
    let mut validated_fractional_target_by_unit = BTreeMap::<SchedulingUnitId, f64>::new();
    let mut predecessor_ids_by_unit = BTreeMap::<SchedulingUnitId, Vec<SchedulingUnitId>>::new();
    let mut pending_predecessor_count_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    let mut successor_unit_ids_by_unit = BTreeMap::<SchedulingUnitId, Vec<SchedulingUnitId>>::new();
    let objective_score_by_unit = build_unit_objective_score_index(scheduling_problem);
    for unit in scheduling_problem.units() {
        let unit_id = unit.unit_id().clone();
        let fractional_target = fractional_target_period_by_unit
            .get(&unit_id)
            .copied()
            .ok_or_else(|| MineError::Planning {
                message: format!(
                    "LP-guided round/repair is missing target period for unit `{}`",
                    unit.unit_id()
                ),
            })?;
        if !fractional_target.is_finite() {
            return Err(MineError::validation(format!(
                "LP-guided round/repair target for unit `{}` must be finite",
                unit.unit_id()
            )));
        }
        if fractional_target < -1.0e-9 {
            return Err(MineError::validation(format!(
                "LP-guided round/repair target for unit `{}` must be non-negative",
                unit.unit_id()
            )));
        }
        validated_fractional_target_by_unit.insert(unit_id.clone(), fractional_target);
        predecessor_ids_by_unit.insert(unit_id.clone(), unit.predecessor_unit_ids().to_vec());
        pending_predecessor_count_by_unit.insert(unit_id, 0usize);
    }

    for (unit_id, predecessor_unit_ids) in &predecessor_ids_by_unit {
        for predecessor_unit_id in predecessor_unit_ids {
            if !pending_predecessor_count_by_unit.contains_key(predecessor_unit_id) {
                return Err(MineError::Planning {
                    message: format!(
                        "LP-guided round/repair found unknown predecessor `{predecessor_unit_id}` for unit `{unit_id}`"
                    ),
                });
            }
            if let Some(pending_count) = pending_predecessor_count_by_unit.get_mut(unit_id) {
                *pending_count += 1;
            }
            successor_unit_ids_by_unit
                .entry(predecessor_unit_id.clone())
                .or_default()
                .push(unit_id.clone());
        }
    }
    for unit in scheduling_problem.units() {
        successor_unit_ids_by_unit
            .entry(unit.unit_id().clone())
            .or_default();
    }
    let unit_priority_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| {
            let unit_id = unit.unit_id().clone();
            let fractional_target = validated_fractional_target_by_unit
                .get(&unit_id)
                .copied()
                .unwrap_or(0.0);
            let rounded_target = round_period_index(fractional_target);
            let clamped_target = rounded_target.min(horizon_last_period);
            let objective_score = objective_score_by_unit
                .get(&unit_id)
                .copied()
                .unwrap_or(0.0);
            let discounted_lp_score = objective_score / discount_factor.powi(clamped_target as i32);
            (
                unit_id.clone(),
                UnitPriorityMetrics {
                    rounded_target,
                    clamped_target,
                    discounted_lp_score,
                    objective_score,
                    successor_count: successor_unit_ids_by_unit.get(&unit_id).map_or(0, Vec::len),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut target_period_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    let rounded_target_period_by_unit = unit_priority_by_unit
        .iter()
        .map(|(unit_id, unit_priority)| (unit_id.clone(), unit_priority.clamped_target))
        .collect::<BTreeMap<_, _>>();
    let mut ready_unit_ids: Vec<SchedulingUnitId> = pending_predecessor_count_by_unit
        .iter()
        .filter(|(_, pending_count)| **pending_count == 0)
        .map(|(unit_id, _)| unit_id.clone())
        .collect();
    sort_ready_unit_ids(
        &mut ready_unit_ids,
        &unit_priority_by_unit,
        &predecessor_ids_by_unit,
        &target_period_by_unit,
    );

    let mut repaired_unit_target_count = 0usize;
    let mut horizon_clamp_count = 0usize;
    let mut ordered_unit_count = 0usize;
    while let Some(unit_id) = ready_unit_ids.first().cloned() {
        ready_unit_ids.remove(0);
        let unit_priority = unit_priority_by_unit
            .get(&unit_id)
            .copied()
            .ok_or_else(|| MineError::Planning {
                message: format!("LP-guided round/repair is missing priority for unit `{unit_id}`"),
            })?;
        let rounded_target = unit_priority.rounded_target;
        let clamped_target = unit_priority.clamped_target;
        if clamped_target != rounded_target {
            horizon_clamp_count += 1;
        }

        let predecessor_target = predecessor_ids_by_unit
            .get(&unit_id)
            .ok_or_else(|| MineError::Planning {
                message: format!("LP-guided round/repair is missing unit `{unit_id}`"),
            })?
            .iter()
            .map(|predecessor_unit_id| {
                target_period_by_unit
                    .get(predecessor_unit_id)
                    .copied()
                    .ok_or_else(|| MineError::Planning {
                        message: format!(
                            "LP-guided round/repair found unresolved predecessor `{predecessor_unit_id}` before repairing unit `{unit_id}`"
                        ),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
        let repaired_target = clamped_target
            .max(predecessor_target)
            .min(horizon_last_period);
        if repaired_target > clamped_target {
            repaired_unit_target_count += 1;
        }
        target_period_by_unit.insert(unit_id.clone(), repaired_target);

        if let Some(successor_unit_ids) = successor_unit_ids_by_unit.get(&unit_id) {
            for successor_unit_id in successor_unit_ids {
                let pending_count = pending_predecessor_count_by_unit
                    .get_mut(successor_unit_id)
                    .ok_or_else(|| MineError::Planning {
                        message: format!(
                            "LP-guided round/repair is missing successor unit `{successor_unit_id}`"
                        ),
                    })?;
                if *pending_count == 0 {
                    return Err(MineError::Planning {
                        message: format!(
                            "LP-guided round/repair predecessor count underflowed for unit `{successor_unit_id}` while processing `{unit_id}`"
                        ),
                    });
                }
                *pending_count -= 1;
                if *pending_count == 0 {
                    ready_unit_ids.push(successor_unit_id.clone());
                }
            }
            sort_ready_unit_ids(
                &mut ready_unit_ids,
                &unit_priority_by_unit,
                &predecessor_ids_by_unit,
                &target_period_by_unit,
            );
        }
        ordered_unit_count += 1;
    }

    if ordered_unit_count != predecessor_ids_by_unit.len() {
        return Err(MineError::Planning {
            message: format!(
                "unit precedence graph is cyclic or disconnected from LP-guided topo round/repair: repaired {ordered_unit_count} of {} units",
                predecessor_ids_by_unit.len()
            ),
        });
    }

    let base_target_period_by_unit = target_period_by_unit.clone();
    let rounded_discounted_target_score_proxy = discounted_target_score_from_objective_index(
        &objective_score_by_unit,
        discount_factor,
        &rounded_target_period_by_unit,
    );
    let repaired_discounted_target_score_proxy = discounted_target_score_from_objective_index(
        &objective_score_by_unit,
        discount_factor,
        &base_target_period_by_unit,
    );
    let local_optimizer_diagnostics = match local_optimization_mode {
        LocalOptimizationMode::Enabled => optimize_unit_target_periods_locally(
            &mut target_period_by_unit,
            &base_target_period_by_unit,
            &predecessor_ids_by_unit,
            &successor_unit_ids_by_unit,
            &objective_score_by_unit,
            discount_factor,
            horizon_last_period,
            build_local_optimizer_budget_profile(
                LocalOptimizationMode::Enabled,
                base_target_period_by_unit.len(),
                horizon_last_period,
            ),
        ),
        LocalOptimizationMode::FocusedRefreshBudgeted => optimize_unit_target_periods_locally(
            &mut target_period_by_unit,
            &base_target_period_by_unit,
            &predecessor_ids_by_unit,
            &successor_unit_ids_by_unit,
            &objective_score_by_unit,
            discount_factor,
            horizon_last_period,
            build_local_optimizer_budget_profile(
                LocalOptimizationMode::FocusedRefreshBudgeted,
                base_target_period_by_unit.len(),
                horizon_last_period,
            ),
        ),
        LocalOptimizationMode::SkippedFocusedRefresh => {
            build_skipped_focused_local_optimizer_diagnostics(
                base_target_period_by_unit.len(),
                horizon_last_period,
            )
        }
    };
    let local_search_discounted_target_score_proxy = discounted_target_score_from_objective_index(
        &objective_score_by_unit,
        discount_factor,
        &target_period_by_unit,
    );
    let target_score_decomposition = LpBzUnitTargetScoreDecomposition {
        rounded_discounted_target_score_proxy,
        repaired_discounted_target_score_proxy,
        local_search_discounted_target_score_proxy,
        repair_score_delta_vs_round_proxy: repaired_discounted_target_score_proxy
            - rounded_discounted_target_score_proxy,
        local_search_score_delta_vs_repair_proxy: local_search_discounted_target_score_proxy
            - repaired_discounted_target_score_proxy,
        local_search_score_delta_vs_round_proxy: local_search_discounted_target_score_proxy
            - rounded_discounted_target_score_proxy,
    };
    let local_improvement_move_count = local_optimizer_diagnostics.improving_move_count;
    assert_precedence_feasible_unit_targets(scheduling_problem, &target_period_by_unit)?;
    Ok(LpBzUnitRoundRepairResult {
        target_score_decomposition,
        target_period_by_unit,
        repaired_unit_target_count,
        horizon_clamp_count,
        local_improvement_move_count,
        local_optimizer_diagnostics,
    })
}

fn discounted_target_score_from_objective_index(
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> f64 {
    target_period_by_unit
        .iter()
        .map(|(unit_id, period_index)| {
            objective_score_by_unit.get(unit_id).copied().unwrap_or(0.0)
                / discount_factor.powi(*period_index as i32)
        })
        .sum()
}

fn build_skipped_focused_local_optimizer_diagnostics(
    target_unit_count: usize,
    horizon_last_period: usize,
) -> LpBzLocalOptimizerDiagnostics {
    let budget_profile = build_local_optimizer_budget_profile(
        LocalOptimizationMode::SkippedFocusedRefresh,
        target_unit_count,
        horizon_last_period,
    );
    LpBzLocalOptimizerDiagnostics {
        strategy_label: FOCUSED_REFRESH_SKIPPED_LOCAL_OPTIMIZER_REASON.to_owned(),
        budget_profile,
        max_iteration_count: 0,
        executed_iteration_count: 0,
        improving_move_count: 0,
        termination_reason: FOCUSED_REFRESH_SKIPPED_LOCAL_OPTIMIZER_REASON.to_owned(),
        residual_opportunity: no_residual_local_optimizer_opportunity(),
    }
}

pub fn local_optimizer_runtime_was_skipped(termination_reason: &str) -> bool {
    termination_reason == FOCUSED_REFRESH_SKIPPED_LOCAL_OPTIMIZER_REASON
}

fn optimize_unit_target_periods_locally(
    target_period_by_unit: &mut BTreeMap<SchedulingUnitId, usize>,
    base_target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
    budget_profile: LpBzLocalOptimizerBudgetProfile,
) -> LpBzLocalOptimizerDiagnostics {
    let strategy_label =
        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8".to_owned();
    let max_iterations = budget_profile.effective_iteration_budget;
    if max_iterations == 0 {
        return LpBzLocalOptimizerDiagnostics {
            strategy_label,
            budget_profile,
            max_iteration_count: 0,
            executed_iteration_count: 0,
            improving_move_count: 0,
            termination_reason: "disabled-insufficient-target-space".to_owned(),
            residual_opportunity: no_residual_local_optimizer_opportunity(),
        };
    }
    let mut applied_move_count = 0usize;
    let mut executed_iteration_count = 0usize;
    for _ in 0..max_iterations {
        executed_iteration_count += 1;
        let best_move = find_best_local_move(
            target_period_by_unit,
            base_target_period_by_unit,
            predecessor_ids_by_unit,
            successor_unit_ids_by_unit,
            objective_score_by_unit,
            discount_factor,
            horizon_last_period,
        );
        let Some(best_move) = best_move else {
            return LpBzLocalOptimizerDiagnostics {
                strategy_label,
                budget_profile,
                max_iteration_count: max_iterations,
                executed_iteration_count,
                improving_move_count: applied_move_count,
                termination_reason: "no-improving-local-move".to_owned(),
                residual_opportunity: no_residual_local_optimizer_opportunity(),
            };
        };
        apply_local_move(target_period_by_unit, best_move);
        applied_move_count += 1;
    }
    let residual_opportunity = build_local_optimizer_residual_opportunity(find_best_local_move(
        target_period_by_unit,
        base_target_period_by_unit,
        predecessor_ids_by_unit,
        successor_unit_ids_by_unit,
        objective_score_by_unit,
        discount_factor,
        horizon_last_period,
    ));
    LpBzLocalOptimizerDiagnostics {
        strategy_label,
        budget_profile,
        max_iteration_count: max_iterations,
        executed_iteration_count,
        improving_move_count: applied_move_count,
        termination_reason: "max-iterations-reached".to_owned(),
        residual_opportunity,
    }
}

fn find_best_local_move(
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    base_target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalMove> {
    let unit_ids_by_period = build_unit_ids_by_period(target_period_by_unit);
    let best_swap = find_best_local_swap_move(
        target_period_by_unit,
        predecessor_ids_by_unit,
        successor_unit_ids_by_unit,
        objective_score_by_unit,
        discount_factor,
        horizon_last_period,
    )
    .map(LpBzLocalMove::Swap);
    let best_path = find_best_period_ejection_move(
        target_period_by_unit,
        &unit_ids_by_period,
        predecessor_ids_by_unit,
        successor_unit_ids_by_unit,
        objective_score_by_unit,
        discount_factor,
        horizon_last_period,
    )
    .map(LpBzLocalMove::Path);
    let best_chain = find_best_precedence_chain_move(
        target_period_by_unit,
        base_target_period_by_unit,
        predecessor_ids_by_unit,
        successor_unit_ids_by_unit,
        objective_score_by_unit,
        discount_factor,
        horizon_last_period,
    )
    .map(LpBzLocalMove::Chain);
    let mut best_move: Option<LpBzLocalMove> = None;
    for candidate_move in [best_swap, best_path, best_chain].into_iter().flatten() {
        if should_replace_local_move(&candidate_move, best_move.as_ref(), 1.0e-12) {
            best_move = Some(candidate_move);
        }
    }
    best_move
}

fn no_residual_local_optimizer_opportunity() -> LpBzLocalOptimizerResidualOpportunity {
    LpBzLocalOptimizerResidualOpportunity {
        improving_move_available: false,
        move_kind_label: "none".to_owned(),
        discounted_gain: 0.0,
    }
}

fn build_local_optimizer_residual_opportunity(
    local_move: Option<LpBzLocalMove>,
) -> LpBzLocalOptimizerResidualOpportunity {
    let Some(local_move) = local_move else {
        return no_residual_local_optimizer_opportunity();
    };
    LpBzLocalOptimizerResidualOpportunity {
        improving_move_available: true,
        move_kind_label: local_move_kind_label(&local_move).to_owned(),
        discounted_gain: local_move_discounted_gain(&local_move),
    }
}

fn build_local_optimizer_budget_profile(
    local_optimization_mode: LocalOptimizationMode,
    target_unit_count: usize,
    horizon_last_period: usize,
) -> LpBzLocalOptimizerBudgetProfile {
    let horizon_period_count = horizon_last_period.saturating_add(1);
    let full_iteration_budget =
        derived_local_optimizer_iteration_budget(target_unit_count, horizon_last_period);
    let requested_iteration_budget = match local_optimization_mode {
        LocalOptimizationMode::Enabled => full_iteration_budget,
        LocalOptimizationMode::FocusedRefreshBudgeted => {
            focused_refresh_local_optimizer_iteration_budget(target_unit_count, horizon_last_period)
        }
        LocalOptimizationMode::SkippedFocusedRefresh => 0,
    };
    let effective_iteration_budget = if full_iteration_budget == 0 {
        0
    } else {
        requested_iteration_budget.min(full_iteration_budget).max(1)
    };

    LpBzLocalOptimizerBudgetProfile {
        mode_label: match local_optimization_mode {
            LocalOptimizationMode::Enabled => "full-round-repair".to_owned(),
            LocalOptimizationMode::FocusedRefreshBudgeted => "focused-refresh-budgeted".to_owned(),
            LocalOptimizationMode::SkippedFocusedRefresh => "skipped-focused-refresh".to_owned(),
        },
        target_unit_count,
        horizon_period_count,
        full_iteration_budget,
        requested_iteration_budget,
        effective_iteration_budget,
    }
}

fn derived_local_optimizer_iteration_budget(
    target_unit_count: usize,
    horizon_last_period: usize,
) -> usize {
    if target_unit_count < 2 || horizon_last_period == 0 {
        return 0;
    }

    target_unit_count
        .saturating_mul(horizon_last_period.saturating_add(1))
        .saturating_mul(2)
}

fn focused_refresh_local_optimizer_iteration_budget(
    target_unit_count: usize,
    horizon_last_period: usize,
) -> usize {
    let full_iteration_budget =
        derived_local_optimizer_iteration_budget(target_unit_count, horizon_last_period);
    if full_iteration_budget == 0 {
        return 0;
    }

    let horizon_component = horizon_last_period.saturating_add(1).saturating_mul(2);
    let unit_component = target_unit_count.saturating_add(1) / 2;
    horizon_component
        .saturating_add(unit_component)
        .clamp(
            FOCUSED_REFRESH_LOCAL_OPTIMIZER_MIN_ITERATIONS,
            FOCUSED_REFRESH_LOCAL_OPTIMIZER_MAX_ITERATIONS,
        )
        .min(full_iteration_budget)
}

fn find_best_local_swap_move(
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalSwapMove> {
    let gain_tolerance = 1.0e-12;
    let mut best_move: Option<LpBzLocalSwapMove> = None;

    for lower_period in 0..horizon_last_period {
        let upper_period = lower_period + 1;
        let mut push_back_candidates = Vec::<SchedulingUnitId>::new();
        let mut pull_forward_candidates = Vec::<SchedulingUnitId>::new();
        for (unit_id, target_period) in target_period_by_unit {
            if *target_period == lower_period
                && is_single_unit_move_precedence_feasible(
                    unit_id,
                    upper_period,
                    target_period_by_unit,
                    predecessor_ids_by_unit,
                    successor_unit_ids_by_unit,
                    horizon_last_period,
                )
            {
                push_back_candidates.push(unit_id.clone());
            }
            if *target_period == upper_period
                && is_single_unit_move_precedence_feasible(
                    unit_id,
                    lower_period,
                    target_period_by_unit,
                    predecessor_ids_by_unit,
                    successor_unit_ids_by_unit,
                    horizon_last_period,
                )
            {
                pull_forward_candidates.push(unit_id.clone());
            }
        }

        for pull_forward_unit_id in &pull_forward_candidates {
            for push_back_unit_id in &push_back_candidates {
                if pull_forward_unit_id == push_back_unit_id {
                    continue;
                }
                if !is_dual_unit_move_precedence_feasible(
                    pull_forward_unit_id,
                    push_back_unit_id,
                    lower_period,
                    upper_period,
                    target_period_by_unit,
                    predecessor_ids_by_unit,
                    successor_unit_ids_by_unit,
                    horizon_last_period,
                ) {
                    continue;
                }
                let pull_forward_gain = discounted_objective_gain(
                    objective_score_by_unit
                        .get(pull_forward_unit_id)
                        .copied()
                        .unwrap_or(0.0),
                    upper_period,
                    lower_period,
                    discount_factor,
                );
                let push_back_gain = discounted_objective_gain(
                    objective_score_by_unit
                        .get(push_back_unit_id)
                        .copied()
                        .unwrap_or(0.0),
                    lower_period,
                    upper_period,
                    discount_factor,
                );
                let discounted_gain = pull_forward_gain + push_back_gain;
                if discounted_gain <= gain_tolerance {
                    continue;
                }
                let candidate_move = LpBzLocalSwapMove {
                    pull_forward_unit_id: pull_forward_unit_id.clone(),
                    push_back_unit_id: push_back_unit_id.clone(),
                    lower_period,
                    upper_period,
                    discounted_gain,
                };
                let should_replace = match &best_move {
                    None => true,
                    Some(current_best) => {
                        if candidate_move.discounted_gain
                            > current_best.discounted_gain + gain_tolerance
                        {
                            true
                        } else if (candidate_move.discounted_gain - current_best.discounted_gain)
                            .abs()
                            <= gain_tolerance
                        {
                            candidate_move.lower_period < current_best.lower_period
                                || (candidate_move.lower_period == current_best.lower_period
                                    && (candidate_move.pull_forward_unit_id
                                        < current_best.pull_forward_unit_id
                                        || (candidate_move.pull_forward_unit_id
                                            == current_best.pull_forward_unit_id
                                            && candidate_move.push_back_unit_id
                                                < current_best.push_back_unit_id)))
                        } else {
                            false
                        }
                    }
                };
                if should_replace {
                    best_move = Some(candidate_move);
                }
            }
        }
    }
    best_move
}

fn find_best_precedence_chain_move(
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    base_target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalChainMove> {
    let gain_tolerance = 1.0e-12;
    let mut best_move: Option<LpBzLocalChainMove> = None;
    for (anchor_unit_id, current_period) in target_period_by_unit {
        let base_period = base_target_period_by_unit
            .get(anchor_unit_id)
            .copied()
            .unwrap_or(*current_period);
        let neighborhood_floor = base_period.saturating_sub(LOCAL_CHAIN_NEIGHBORHOOD_RADIUS);
        if neighborhood_floor >= *current_period {
            continue;
        }
        for candidate_period in neighborhood_floor..*current_period {
            let Some(candidate_move) = build_precedence_chain_move(
                anchor_unit_id,
                candidate_period,
                target_period_by_unit,
                base_target_period_by_unit,
                predecessor_ids_by_unit,
                successor_unit_ids_by_unit,
                objective_score_by_unit,
                discount_factor,
                horizon_last_period,
            ) else {
                continue;
            };
            if candidate_move.discounted_gain <= gain_tolerance {
                continue;
            }
            let should_replace = match &best_move {
                None => true,
                Some(current_best) => {
                    should_replace_chain_move(&candidate_move, current_best, gain_tolerance)
                }
            };
            if should_replace {
                best_move = Some(candidate_move);
            }
        }
    }
    best_move
}

fn find_best_period_ejection_move(
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    unit_ids_by_period: &BTreeMap<usize, Vec<SchedulingUnitId>>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalPathMove> {
    let gain_tolerance = 1.0e-12;
    let mut best_move: Option<LpBzLocalPathMove> = None;
    for (anchor_unit_id, current_period) in target_period_by_unit {
        if *current_period < 2 {
            continue;
        }
        let neighborhood_floor =
            current_period.saturating_sub(LOCAL_PERIOD_EJECTION_NEIGHBORHOOD_RADIUS);
        for candidate_period in neighborhood_floor..current_period.saturating_sub(1) {
            let Some(candidate_move) = build_period_ejection_move(
                anchor_unit_id,
                *current_period,
                candidate_period,
                target_period_by_unit,
                unit_ids_by_period,
                predecessor_ids_by_unit,
                successor_unit_ids_by_unit,
                objective_score_by_unit,
                discount_factor,
                horizon_last_period,
            ) else {
                continue;
            };
            if candidate_move.discounted_gain <= gain_tolerance {
                continue;
            }
            let should_replace = match &best_move {
                None => true,
                Some(current_best) => {
                    should_replace_path_move(&candidate_move, current_best, gain_tolerance)
                }
            };
            if should_replace {
                best_move = Some(candidate_move);
            }
        }
    }
    best_move
}

fn build_period_ejection_move(
    anchor_unit_id: &SchedulingUnitId,
    anchor_current_period: usize,
    anchor_target_period: usize,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    unit_ids_by_period: &BTreeMap<usize, Vec<SchedulingUnitId>>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalPathMove> {
    if anchor_target_period.saturating_add(1) >= anchor_current_period
        || !is_single_unit_move_precedence_feasible(
            anchor_unit_id,
            anchor_target_period,
            target_period_by_unit,
            predecessor_ids_by_unit,
            successor_unit_ids_by_unit,
            horizon_last_period,
        )
    {
        return None;
    }
    let anchor_gain = discounted_objective_gain(
        objective_score_by_unit
            .get(anchor_unit_id)
            .copied()
            .unwrap_or(0.0),
        anchor_current_period,
        anchor_target_period,
        discount_factor,
    );
    let projected_period_by_unit = BTreeMap::from([(anchor_unit_id.clone(), anchor_target_period)]);
    let search_result = search_best_period_ejection_chain(
        anchor_target_period,
        anchor_current_period,
        None,
        &projected_period_by_unit,
        target_period_by_unit,
        unit_ids_by_period,
        predecessor_ids_by_unit,
        successor_unit_ids_by_unit,
        objective_score_by_unit,
        discount_factor,
        horizon_last_period,
    )?;
    Some(LpBzLocalPathMove {
        anchor_unit_id: anchor_unit_id.clone(),
        anchor_target_period,
        projected_period_by_unit: search_result.projected_period_by_unit,
        discounted_gain: anchor_gain + search_result.discounted_gain,
        moved_unit_sequence: search_result.moved_unit_sequence,
    })
}

#[derive(Debug, Clone)]
struct LpBzLocalPathSearchResult {
    projected_period_by_unit: BTreeMap<SchedulingUnitId, usize>,
    discounted_gain: f64,
    moved_unit_sequence: Vec<SchedulingUnitId>,
}

#[allow(clippy::too_many_arguments)]
fn search_best_period_ejection_chain(
    step_period: usize,
    anchor_current_period: usize,
    carry_unit_id: Option<SchedulingUnitId>,
    projected_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    unit_ids_by_period: &BTreeMap<usize, Vec<SchedulingUnitId>>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalPathSearchResult> {
    if step_period == anchor_current_period {
        return Some(LpBzLocalPathSearchResult {
            projected_period_by_unit: projected_period_by_unit.clone(),
            discounted_gain: 0.0,
            moved_unit_sequence: Vec::new(),
        });
    }
    let next_period = step_period + 1;
    let mut candidate_unit_ids = unit_ids_by_period
        .get(&step_period)
        .cloned()
        .unwrap_or_default();
    if let Some(carry_unit_id) = carry_unit_id {
        candidate_unit_ids.push(carry_unit_id);
    }
    candidate_unit_ids.retain(|unit_id| {
        projected_period_for_unit(unit_id, projected_period_by_unit, target_period_by_unit)
            == Some(step_period)
    });
    candidate_unit_ids.sort_by(|left, right| {
        discounted_objective_gain(
            objective_score_by_unit.get(right).copied().unwrap_or(0.0),
            step_period,
            next_period,
            discount_factor,
        )
        .total_cmp(&discounted_objective_gain(
            objective_score_by_unit.get(left).copied().unwrap_or(0.0),
            step_period,
            next_period,
            discount_factor,
        ))
        .then_with(|| {
            successor_unit_ids_by_unit
                .get(left)
                .map_or(0usize, Vec::len)
                .cmp(
                    &successor_unit_ids_by_unit
                        .get(right)
                        .map_or(0usize, Vec::len),
                )
        })
        .then_with(|| left.cmp(right))
    });

    let mut best_result: Option<LpBzLocalPathSearchResult> = None;
    for candidate_unit_id in candidate_unit_ids
        .into_iter()
        .take(LOCAL_PERIOD_EJECTION_BRANCH_LIMIT)
    {
        if !is_single_unit_move_precedence_feasible_with_projection(
            &candidate_unit_id,
            next_period,
            projected_period_by_unit,
            target_period_by_unit,
            predecessor_ids_by_unit,
            successor_unit_ids_by_unit,
            horizon_last_period,
        ) {
            continue;
        }
        let mut next_projected_period_by_unit = projected_period_by_unit.clone();
        next_projected_period_by_unit.insert(candidate_unit_id.clone(), next_period);
        let Some(downstream_result) = search_best_period_ejection_chain(
            next_period,
            anchor_current_period,
            Some(candidate_unit_id.clone()),
            &next_projected_period_by_unit,
            target_period_by_unit,
            unit_ids_by_period,
            predecessor_ids_by_unit,
            successor_unit_ids_by_unit,
            objective_score_by_unit,
            discount_factor,
            horizon_last_period,
        ) else {
            continue;
        };
        let mut moved_unit_sequence = vec![candidate_unit_id.clone()];
        moved_unit_sequence.extend(downstream_result.moved_unit_sequence);
        let candidate_result = LpBzLocalPathSearchResult {
            projected_period_by_unit: downstream_result.projected_period_by_unit,
            discounted_gain: discounted_objective_gain(
                objective_score_by_unit
                    .get(&candidate_unit_id)
                    .copied()
                    .unwrap_or(0.0),
                step_period,
                next_period,
                discount_factor,
            ) + downstream_result.discounted_gain,
            moved_unit_sequence,
        };
        let should_replace = match &best_result {
            None => true,
            Some(current_best) => {
                should_replace_path_search_result(&candidate_result, current_best, 1.0e-12)
            }
        };
        if should_replace {
            best_result = Some(candidate_result);
        }
    }
    best_result
}

fn build_precedence_chain_move(
    anchor_unit_id: &SchedulingUnitId,
    candidate_period: usize,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    base_target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    objective_score_by_unit: &BTreeMap<SchedulingUnitId, f64>,
    discount_factor: f64,
    horizon_last_period: usize,
) -> Option<LpBzLocalChainMove> {
    let current_period = target_period_by_unit.get(anchor_unit_id).copied()?;
    if candidate_period >= current_period {
        return None;
    }
    let mut projected_period_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    let mut pending_projection = vec![(anchor_unit_id.clone(), candidate_period)];
    while let Some((unit_id, desired_period)) = pending_projection.pop() {
        let current_or_projected_period = projected_period_by_unit
            .get(&unit_id)
            .copied()
            .or_else(|| target_period_by_unit.get(&unit_id).copied())?;
        if desired_period >= current_or_projected_period {
            continue;
        }
        let base_period = base_target_period_by_unit
            .get(&unit_id)
            .copied()
            .unwrap_or(current_or_projected_period);
        let neighborhood_floor = base_period.saturating_sub(LOCAL_CHAIN_NEIGHBORHOOD_RADIUS);
        if desired_period < neighborhood_floor {
            return None;
        }
        projected_period_by_unit.insert(unit_id.clone(), desired_period);
        for predecessor_unit_id in predecessor_ids_by_unit.get(&unit_id).into_iter().flatten() {
            let predecessor_period = projected_period_by_unit
                .get(predecessor_unit_id)
                .copied()
                .or_else(|| target_period_by_unit.get(predecessor_unit_id).copied())?;
            if predecessor_period > desired_period {
                pending_projection.push((predecessor_unit_id.clone(), desired_period));
            }
        }
    }
    if projected_period_by_unit.len() < 2
        || !is_projected_move_precedence_feasible(
            &projected_period_by_unit,
            target_period_by_unit,
            predecessor_ids_by_unit,
            successor_unit_ids_by_unit,
            horizon_last_period,
        )
    {
        return None;
    }
    let anchor_discounted_gain = discounted_objective_gain(
        objective_score_by_unit
            .get(anchor_unit_id)
            .copied()
            .unwrap_or(0.0),
        current_period,
        candidate_period,
        discount_factor,
    );
    let anchor_objective_score = objective_score_by_unit
        .get(anchor_unit_id)
        .copied()
        .unwrap_or(0.0);
    let max_other_objective_score = projected_period_by_unit
        .keys()
        .filter(|unit_id| *unit_id != anchor_unit_id)
        .map(|unit_id| objective_score_by_unit.get(unit_id).copied().unwrap_or(0.0))
        .fold(f64::NEG_INFINITY, f64::max);
    if anchor_objective_score <= max_other_objective_score + 1.0e-12 {
        return None;
    }
    let discounted_gain = projected_period_by_unit
        .iter()
        .map(|(unit_id, projected_period)| {
            discounted_objective_gain(
                objective_score_by_unit.get(unit_id).copied().unwrap_or(0.0),
                target_period_by_unit
                    .get(unit_id)
                    .copied()
                    .unwrap_or(*projected_period),
                *projected_period,
                discount_factor,
            )
        })
        .sum();
    if anchor_discounted_gain <= discounted_gain - anchor_discounted_gain + 1.0e-12 {
        return None;
    }
    Some(LpBzLocalChainMove {
        anchor_unit_id: anchor_unit_id.clone(),
        anchor_target_period: candidate_period,
        projected_period_by_unit,
        discounted_gain,
    })
}

fn apply_local_move(
    target_period_by_unit: &mut BTreeMap<SchedulingUnitId, usize>,
    local_move: LpBzLocalMove,
) {
    match local_move {
        LpBzLocalMove::Swap(local_swap_move) => {
            target_period_by_unit.insert(
                local_swap_move.pull_forward_unit_id,
                local_swap_move.lower_period,
            );
            target_period_by_unit.insert(
                local_swap_move.push_back_unit_id,
                local_swap_move.upper_period,
            );
        }
        LpBzLocalMove::Path(local_path_move) => {
            for (unit_id, projected_period) in local_path_move.projected_period_by_unit {
                target_period_by_unit.insert(unit_id, projected_period);
            }
        }
        LpBzLocalMove::Chain(local_chain_move) => {
            for (unit_id, projected_period) in local_chain_move.projected_period_by_unit {
                target_period_by_unit.insert(unit_id, projected_period);
            }
        }
    }
}

fn should_replace_chain_move(
    candidate_move: &LpBzLocalChainMove,
    current_best: &LpBzLocalChainMove,
    gain_tolerance: f64,
) -> bool {
    if candidate_move.discounted_gain > current_best.discounted_gain + gain_tolerance {
        return true;
    }
    if (candidate_move.discounted_gain - current_best.discounted_gain).abs() > gain_tolerance {
        return false;
    }
    candidate_move.anchor_target_period < current_best.anchor_target_period
        || (candidate_move.anchor_target_period == current_best.anchor_target_period
            && (candidate_move.anchor_unit_id < current_best.anchor_unit_id
                || (candidate_move.anchor_unit_id == current_best.anchor_unit_id
                    && moved_unit_ids_for_chain_move(candidate_move)
                        < moved_unit_ids_for_chain_move(current_best))))
}

fn should_replace_path_move(
    candidate_move: &LpBzLocalPathMove,
    current_best: &LpBzLocalPathMove,
    gain_tolerance: f64,
) -> bool {
    if candidate_move.discounted_gain > current_best.discounted_gain + gain_tolerance {
        return true;
    }
    if (candidate_move.discounted_gain - current_best.discounted_gain).abs() > gain_tolerance {
        return false;
    }
    candidate_move.anchor_target_period < current_best.anchor_target_period
        || (candidate_move.anchor_target_period == current_best.anchor_target_period
            && (candidate_move.anchor_unit_id < current_best.anchor_unit_id
                || (candidate_move.anchor_unit_id == current_best.anchor_unit_id
                    && candidate_move.moved_unit_sequence < current_best.moved_unit_sequence)))
}

fn should_replace_path_search_result(
    candidate_result: &LpBzLocalPathSearchResult,
    current_best: &LpBzLocalPathSearchResult,
    gain_tolerance: f64,
) -> bool {
    if candidate_result.discounted_gain > current_best.discounted_gain + gain_tolerance {
        return true;
    }
    if (candidate_result.discounted_gain - current_best.discounted_gain).abs() > gain_tolerance {
        return false;
    }
    candidate_result.moved_unit_sequence < current_best.moved_unit_sequence
}

fn should_replace_local_move(
    candidate_move: &LpBzLocalMove,
    current_best: Option<&LpBzLocalMove>,
    gain_tolerance: f64,
) -> bool {
    let Some(current_best) = current_best else {
        return true;
    };
    let candidate_gain = local_move_discounted_gain(candidate_move);
    let current_gain = local_move_discounted_gain(current_best);
    if candidate_gain > current_gain + gain_tolerance {
        return true;
    }
    if (candidate_gain - current_gain).abs() > gain_tolerance {
        return false;
    }
    local_move_sort_key(candidate_move) < local_move_sort_key(current_best)
}

fn local_move_discounted_gain(local_move: &LpBzLocalMove) -> f64 {
    match local_move {
        LpBzLocalMove::Swap(local_swap_move) => local_swap_move.discounted_gain,
        LpBzLocalMove::Path(local_path_move) => local_path_move.discounted_gain,
        LpBzLocalMove::Chain(local_chain_move) => local_chain_move.discounted_gain,
    }
}

fn local_move_kind_label(local_move: &LpBzLocalMove) -> &'static str {
    match local_move {
        LpBzLocalMove::Swap(_) => "swap",
        LpBzLocalMove::Path(_) => "path",
        LpBzLocalMove::Chain(_) => "chain",
    }
}

fn local_move_sort_key(local_move: &LpBzLocalMove) -> (usize, usize, SchedulingUnitId, usize) {
    match local_move {
        LpBzLocalMove::Swap(local_swap_move) => (
            0,
            local_swap_move.lower_period,
            local_swap_move.pull_forward_unit_id.clone(),
            2,
        ),
        LpBzLocalMove::Path(local_path_move) => (
            1,
            local_path_move.anchor_target_period,
            local_path_move.anchor_unit_id.clone(),
            local_path_move.projected_period_by_unit.len(),
        ),
        LpBzLocalMove::Chain(local_chain_move) => (
            2,
            local_chain_move.anchor_target_period,
            local_chain_move.anchor_unit_id.clone(),
            local_chain_move.projected_period_by_unit.len(),
        ),
    }
}

fn build_unit_ids_by_period(
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> BTreeMap<usize, Vec<SchedulingUnitId>> {
    let mut unit_ids_by_period = BTreeMap::<usize, Vec<SchedulingUnitId>>::new();
    for (unit_id, target_period) in target_period_by_unit {
        unit_ids_by_period
            .entry(*target_period)
            .or_default()
            .push(unit_id.clone());
    }
    unit_ids_by_period
}

fn moved_unit_ids_for_chain_move(local_chain_move: &LpBzLocalChainMove) -> Vec<SchedulingUnitId> {
    local_chain_move
        .projected_period_by_unit
        .keys()
        .cloned()
        .collect()
}

fn projected_period_for_unit(
    unit_id: &SchedulingUnitId,
    projected_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> Option<usize> {
    projected_period_by_unit
        .get(unit_id)
        .copied()
        .or_else(|| target_period_by_unit.get(unit_id).copied())
}

fn is_single_unit_move_precedence_feasible_with_projection(
    unit_id: &SchedulingUnitId,
    candidate_period: usize,
    projected_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    horizon_last_period: usize,
) -> bool {
    if candidate_period > horizon_last_period {
        return false;
    }
    if predecessor_ids_by_unit
        .get(unit_id)
        .into_iter()
        .flatten()
        .any(|predecessor_unit_id| {
            projected_period_for_unit(
                predecessor_unit_id,
                projected_period_by_unit,
                target_period_by_unit,
            )
            .map(|predecessor_period| predecessor_period > candidate_period)
            .unwrap_or(true)
        })
    {
        return false;
    }
    !successor_unit_ids_by_unit
        .get(unit_id)
        .into_iter()
        .flatten()
        .any(|successor_unit_id| {
            projected_period_for_unit(
                successor_unit_id,
                projected_period_by_unit,
                target_period_by_unit,
            )
            .map(|successor_period| candidate_period > successor_period)
            .unwrap_or(true)
        })
}

fn is_projected_move_precedence_feasible(
    projected_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    horizon_last_period: usize,
) -> bool {
    let projected_period = |unit_id: &SchedulingUnitId| -> Option<usize> {
        projected_period_by_unit
            .get(unit_id)
            .copied()
            .or_else(|| target_period_by_unit.get(unit_id).copied())
    };
    for moved_unit_id in projected_period_by_unit.keys() {
        let Some(moved_period) = projected_period(moved_unit_id) else {
            return false;
        };
        if moved_period > horizon_last_period {
            return false;
        }
        if predecessor_ids_by_unit
            .get(moved_unit_id)
            .into_iter()
            .flatten()
            .any(|predecessor_unit_id| {
                projected_period(predecessor_unit_id)
                    .map(|predecessor_period| predecessor_period > moved_period)
                    .unwrap_or(true)
            })
        {
            return false;
        }
        if successor_unit_ids_by_unit
            .get(moved_unit_id)
            .into_iter()
            .flatten()
            .any(|successor_unit_id| {
                projected_period(successor_unit_id)
                    .map(|successor_period| moved_period > successor_period)
                    .unwrap_or(true)
            })
        {
            return false;
        }
    }
    true
}

fn is_single_unit_move_precedence_feasible(
    unit_id: &SchedulingUnitId,
    candidate_period: usize,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    horizon_last_period: usize,
) -> bool {
    if candidate_period > horizon_last_period {
        return false;
    }
    if predecessor_ids_by_unit
        .get(unit_id)
        .into_iter()
        .flatten()
        .any(|predecessor_unit_id| {
            target_period_by_unit
                .get(predecessor_unit_id)
                .copied()
                .unwrap_or(0)
                > candidate_period
        })
    {
        return false;
    }
    !successor_unit_ids_by_unit
        .get(unit_id)
        .into_iter()
        .flatten()
        .any(|successor_unit_id| {
            candidate_period
                > target_period_by_unit
                    .get(successor_unit_id)
                    .copied()
                    .unwrap_or(horizon_last_period)
        })
}

fn is_dual_unit_move_precedence_feasible(
    pull_forward_unit_id: &SchedulingUnitId,
    push_back_unit_id: &SchedulingUnitId,
    lower_period: usize,
    upper_period: usize,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    successor_unit_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    horizon_last_period: usize,
) -> bool {
    if lower_period > horizon_last_period
        || upper_period > horizon_last_period
        || lower_period >= upper_period
    {
        return false;
    }
    let projected_period = |unit_id: &SchedulingUnitId| -> Option<usize> {
        if unit_id == pull_forward_unit_id {
            Some(lower_period)
        } else if unit_id == push_back_unit_id {
            Some(upper_period)
        } else {
            target_period_by_unit.get(unit_id).copied()
        }
    };

    for moved_unit_id in [pull_forward_unit_id, push_back_unit_id] {
        let Some(moved_period) = projected_period(moved_unit_id) else {
            return false;
        };
        if predecessor_ids_by_unit
            .get(moved_unit_id)
            .into_iter()
            .flatten()
            .any(|predecessor_unit_id| {
                projected_period(predecessor_unit_id)
                    .map(|predecessor_period| predecessor_period > moved_period)
                    .unwrap_or(false)
            })
        {
            return false;
        }
        if successor_unit_ids_by_unit
            .get(moved_unit_id)
            .into_iter()
            .flatten()
            .any(|successor_unit_id| {
                projected_period(successor_unit_id)
                    .map(|successor_period| moved_period > successor_period)
                    .unwrap_or(false)
            })
        {
            return false;
        }
    }
    true
}

fn discounted_objective_gain(
    objective_score: f64,
    from_period: usize,
    to_period: usize,
    discount_factor: f64,
) -> f64 {
    if from_period == to_period {
        return 0.0;
    }
    objective_score / discount_factor.powi(to_period as i32)
        - objective_score / discount_factor.powi(from_period as i32)
}

/// Verifica que los periodos objetivo por unidad respeten precedencia.
pub fn assert_precedence_feasible_unit_targets(
    scheduling_problem: &SchedulingProblem,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> Result<(), MineError> {
    for unit in scheduling_problem.units() {
        let target_period = target_period_by_unit
            .get(unit.unit_id())
            .copied()
            .ok_or_else(|| MineError::Planning {
                message: format!(
                    "precedence feasibility check is missing unit `{}`",
                    unit.unit_id()
                ),
            })?;
        for predecessor_unit_id in unit.predecessor_unit_ids() {
            let predecessor_period = target_period_by_unit
                .get(predecessor_unit_id)
                .copied()
                .ok_or_else(|| MineError::Planning {
                    message: format!(
                        "precedence feasibility check is missing predecessor `{predecessor_unit_id}`"
                    ),
                })?;
            if predecessor_period > target_period {
                return Err(MineError::Planning {
                    message: format!(
                        "precedence feasibility violated: predecessor `{predecessor_unit_id}` target period {predecessor_period} > unit `{}` target period {target_period}",
                        unit.unit_id()
                    ),
                });
            }
        }
    }
    Ok(())
}

fn build_fractional_unit_targets_from_phase_targets(
    scheduling_problem: &SchedulingProblem,
    phase_target_period_by_phase: &BTreeMap<String, usize>,
    phase_representative_period_by_phase: &BTreeMap<String, f64>,
    phase_fractional_signal_by_phase: &BTreeMap<String, LpPhaseFractionalSignal>,
) -> Result<BTreeMap<SchedulingUnitId, f64>, MineError> {
    let objective_score_by_unit = build_unit_objective_score_index(scheduling_problem);
    let successor_count_by_unit = build_successor_count_by_unit(scheduling_problem);
    let depth_by_unit = compute_topological_depth_by_unit(scheduling_problem)?;
    let predecessor_ids_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().clone(), unit.predecessor_unit_ids().to_vec()))
        .collect::<BTreeMap<_, _>>();
    let discount_factor = 1.0 + scheduling_problem.discount_rate();

    let mut unit_ids_by_phase = BTreeMap::<String, Vec<SchedulingUnitId>>::new();
    for unit in scheduling_problem.units() {
        let phase_id = phase_id_for_unit(unit.unit_id()).to_owned();
        unit_ids_by_phase
            .entry(phase_id)
            .or_default()
            .push(unit.unit_id().clone());
    }

    let mut fractional_target_period_by_unit = BTreeMap::<SchedulingUnitId, f64>::new();
    for (phase_id, mut unit_ids) in unit_ids_by_phase {
        let phase_target = phase_target_period_by_phase
            .get(&phase_id)
            .copied()
            .ok_or_else(|| MineError::Planning {
                message: format!("LP-guided phase targets are missing phase `{phase_id}`"),
            })?;
        let legacy_representative_period = phase_representative_period_by_phase
            .get(&phase_id)
            .copied()
            .unwrap_or(phase_target as f64);
        let phase_signal = phase_fractional_signal_by_phase.get(&phase_id).copied();
        let representative_period =
            effective_phase_representative_period(legacy_representative_period, phase_signal);
        let phase_signal = phase_signal.unwrap_or(LpPhaseFractionalSignal {
            representative_period,
            lower_mass_share: 0.0,
            upper_mass_share: 0.0,
            floor_mass_share: 0.0,
            ceil_mass_share: 0.0,
            distribution_skew: 0.0,
            confidence: 0.0,
            dominant_destination_share: 0.0,
        });
        let floor_period = representative_period.floor().max(0.0) as usize;
        let upper_share = (representative_period - floor_period as f64).clamp(0.0, 1.0);
        let legacy_pull_share = if upper_share + 1.0e-9 >= 0.5 && phase_target > 0 {
            1.0 - upper_share
        } else {
            0.0
        };
        let distribution_pull_share = (phase_signal.lower_mass_share
            - 0.5 * phase_signal.upper_mass_share * (1.0 - phase_signal.confidence))
            .clamp(0.0, 1.0);
        let confidence_weight = (0.5 + 0.5 * phase_signal.confidence)
            * (0.75 + 0.25 * phase_signal.dominant_destination_share);
        let skew_penalty = phase_signal.distribution_skew.max(0.0) * 0.25;
        let signal_pull_share =
            (distribution_pull_share * confidence_weight - skew_penalty).clamp(0.0, 1.0);
        let blended_pull_share = legacy_pull_share.max(signal_pull_share);
        let early_shift_count = if phase_target > 0 {
            ((unit_ids.len() as f64) * blended_pull_share).round() as usize
        } else {
            0usize
        }
        .min(unit_ids.len());
        unit_ids.sort_by(|left, right| {
            let left_discounted_score = objective_score_by_unit.get(left).copied().unwrap_or(0.0)
                / discount_factor.powi(phase_target as i32);
            let right_discounted_score = objective_score_by_unit.get(right).copied().unwrap_or(0.0)
                / discount_factor.powi(phase_target as i32);
            depth_by_unit
                .get(left)
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(&depth_by_unit.get(right).copied().unwrap_or(usize::MAX))
                .then_with(|| right_discounted_score.total_cmp(&left_discounted_score))
                .then_with(|| {
                    objective_score_by_unit
                        .get(right)
                        .copied()
                        .unwrap_or(0.0)
                        .total_cmp(&objective_score_by_unit.get(left).copied().unwrap_or(0.0))
                })
                .then_with(|| {
                    successor_count_by_unit
                        .get(right)
                        .copied()
                        .unwrap_or(0)
                        .cmp(&successor_count_by_unit.get(left).copied().unwrap_or(0))
                })
                .then_with(|| left.cmp(right))
        });
        let mut remaining_early_shift_count = early_shift_count;
        for unit_id in unit_ids {
            let predecessor_phase_floor = predecessor_phase_floor_for_unit(
                &unit_id,
                &predecessor_ids_by_unit,
                phase_target_period_by_phase,
            )?;
            let can_pull_forward = remaining_early_shift_count > 0
                && phase_target > 0
                && predecessor_phase_floor.saturating_add(1) <= phase_target;
            let base_period = if can_pull_forward {
                remaining_early_shift_count -= 1;
                phase_target.saturating_sub(1)
            } else {
                phase_target
            };
            let repair_bias = (depth_by_unit.get(&unit_id).copied().unwrap_or(0) as f64) * 1.0e-3;
            fractional_target_period_by_unit.insert(unit_id, base_period as f64 + repair_bias);
        }
    }
    Ok(fractional_target_period_by_unit)
}

fn predecessor_phase_floor_for_unit(
    unit_id: &SchedulingUnitId,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    phase_target_period_by_phase: &BTreeMap<String, usize>,
) -> Result<usize, MineError> {
    let predecessor_ids = predecessor_ids_by_unit
        .get(unit_id)
        .ok_or_else(|| MineError::Planning {
            message: format!(
                "LP-guided fractional target propagation is missing predecessor index for unit `{unit_id}`"
            ),
        })?;
    predecessor_ids
        .iter()
        .map(|predecessor_unit_id| {
            let predecessor_phase_id = phase_id_for_unit(predecessor_unit_id);
            phase_target_period_by_phase
                .get(predecessor_phase_id)
                .copied()
                .ok_or_else(|| MineError::Planning {
                    message: format!(
                        "LP-guided phase targets are missing predecessor phase `{predecessor_phase_id}` for unit `{unit_id}`"
                    ),
                })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|predecessor_phase_targets| predecessor_phase_targets.into_iter().max().unwrap_or(0))
}

fn build_phase_representative_period_by_phase(
    phase_plan: &PushbackPlan,
    representative_period_by_block: &BTreeMap<usize, f64>,
) -> BTreeMap<String, f64> {
    phase_plan
        .phases
        .iter()
        .map(|phase| {
            let mut weighted_period_sum = 0.0_f64;
            let mut weighted_block_count = 0.0_f64;
            for linear_index in &phase.block_indices {
                let Some(period_index) = representative_period_by_block.get(linear_index) else {
                    continue;
                };
                weighted_period_sum += *period_index;
                weighted_block_count += 1.0;
            }
            let representative_period = if weighted_block_count <= 1.0e-9 {
                0.0
            } else {
                weighted_period_sum / weighted_block_count
            };
            (phase.phase_id.clone(), representative_period)
        })
        .collect()
}

fn build_phase_fractional_signal_by_phase(
    phase_plan: &PushbackPlan,
    lp_fractional_profile_by_block: &BTreeMap<usize, LpBlockFractionalProfile>,
) -> BTreeMap<String, LpPhaseFractionalSignal> {
    phase_plan
        .phases
        .iter()
        .map(|phase| {
            let mut weighted_period_sum = 0.0_f64;
            let mut total_mass = 0.0_f64;
            let mut period_mass_by_period = BTreeMap::<usize, f64>::new();
            let mut destination_mass_by_destination = BTreeMap::<usize, f64>::new();
            for linear_index in &phase.block_indices {
                let Some(block_profile) = lp_fractional_profile_by_block.get(linear_index) else {
                    continue;
                };
                weighted_period_sum +=
                    block_profile.representative_period * block_profile.total_fraction;
                total_mass += block_profile.total_fraction;
                for (period_index, mass) in &block_profile.period_mass_by_period {
                    *period_mass_by_period.entry(*period_index).or_insert(0.0) += *mass;
                }
                for (destination_index, mass) in &block_profile.destination_mass_by_destination {
                    *destination_mass_by_destination
                        .entry(*destination_index)
                        .or_insert(0.0) += *mass;
                }
            }
            if total_mass <= 1.0e-9 {
                return (
                    phase.phase_id.clone(),
                    LpPhaseFractionalSignal {
                        representative_period: 0.0,
                        lower_mass_share: 0.0,
                        upper_mass_share: 0.0,
                        floor_mass_share: 0.0,
                        ceil_mass_share: 0.0,
                        distribution_skew: 0.0,
                        confidence: 0.0,
                        dominant_destination_share: 0.0,
                    },
                );
            }

            let representative_period = weighted_period_sum / total_mass;
            let floor_period = representative_period.floor().max(0.0) as usize;
            let ceil_period = representative_period.ceil().max(0.0) as usize;
            let floor_mass_share = period_mass_by_period
                .get(&floor_period)
                .copied()
                .unwrap_or(0.0)
                / total_mass;
            let ceil_mass_share = period_mass_by_period
                .get(&ceil_period)
                .copied()
                .unwrap_or(0.0)
                / total_mass;
            let lower_mass_share = period_mass_by_period
                .iter()
                .filter(|(period_index, _)| (**period_index as f64) < representative_period)
                .map(|(_, mass)| *mass)
                .sum::<f64>()
                / total_mass;
            let upper_mass_share = period_mass_by_period
                .iter()
                .filter(|(period_index, _)| (**period_index as f64) > representative_period)
                .map(|(_, mass)| *mass)
                .sum::<f64>()
                / total_mass;
            let period_concentration = period_mass_by_period
                .values()
                .map(|mass| {
                    let share = *mass / total_mass;
                    share * share
                })
                .sum::<f64>();
            let dominant_destination_share = destination_mass_by_destination
                .values()
                .copied()
                .max_by(|left, right| left.total_cmp(right))
                .unwrap_or(0.0)
                / total_mass;
            let near_mass_share = if floor_period == ceil_period {
                floor_mass_share
            } else {
                floor_mass_share + ceil_mass_share
            };
            let confidence =
                ((period_concentration + near_mass_share + dominant_destination_share) / 3.0)
                    .clamp(0.0, 1.0);
            let distribution_skew = (upper_mass_share - lower_mass_share).clamp(-1.0, 1.0);
            (
                phase.phase_id.clone(),
                LpPhaseFractionalSignal {
                    representative_period,
                    lower_mass_share,
                    upper_mass_share,
                    floor_mass_share,
                    ceil_mass_share,
                    distribution_skew,
                    confidence,
                    dominant_destination_share,
                },
            )
        })
        .collect()
}

fn effective_phase_representative_period(
    legacy_representative_period: f64,
    phase_signal: Option<LpPhaseFractionalSignal>,
) -> f64 {
    phase_signal
        .map(|signal| signal.representative_period)
        .filter(|representative_period| {
            representative_period.is_finite() && *representative_period >= 0.0
        })
        .unwrap_or(legacy_representative_period)
}

fn round_period_index_with_signal(
    representative_period: f64,
    phase_signal: Option<LpPhaseFractionalSignal>,
) -> usize {
    let base_period = representative_period.floor().max(0.0) as usize;
    let upper_period = representative_period.ceil().max(0.0) as usize;
    if base_period == upper_period {
        return base_period;
    }
    let fractional_part = (representative_period - base_period as f64).clamp(0.0, 1.0);
    phase_signal.map_or_else(
        || round_period_index(representative_period),
        |signal| {
            let lower_signal = (signal.lower_mass_share + signal.floor_mass_share)
                * (0.5 + 0.5 * signal.confidence);
            let upper_signal = (signal.upper_mass_share + signal.ceil_mass_share)
                * (0.5 + 0.5 * signal.confidence)
                * (0.75 + 0.25 * signal.dominant_destination_share);
            let threshold = (0.5 + 0.2 * (upper_signal - lower_signal)).clamp(0.25, 0.75);
            if fractional_part + 1.0e-9 >= threshold {
                upper_period
            } else {
                base_period
            }
        },
    )
}

fn sort_ready_phase_ids(
    ready_phase_ids: &mut [String],
    lp_rounded_target_by_phase: &BTreeMap<String, usize>,
    phase_priority_by_phase: &BTreeMap<String, PhasePriorityMetrics>,
) {
    ready_phase_ids.sort_by(|left, right| {
        let left_priority =
            phase_priority_by_phase
                .get(left)
                .copied()
                .unwrap_or(PhasePriorityMetrics {
                    representative_period: f64::INFINITY,
                    revenue_factor: 0.0,
                    pushback_index: usize::MAX,
                    bench: i64::MIN,
                    distribution_skew: 0.0,
                    confidence: 0.0,
                    dominant_destination_share: 0.0,
                });
        let right_priority =
            phase_priority_by_phase
                .get(right)
                .copied()
                .unwrap_or(PhasePriorityMetrics {
                    representative_period: f64::INFINITY,
                    revenue_factor: 0.0,
                    pushback_index: usize::MAX,
                    bench: i64::MIN,
                    distribution_skew: 0.0,
                    confidence: 0.0,
                    dominant_destination_share: 0.0,
                });
        lp_rounded_target_by_phase
            .get(left)
            .copied()
            .unwrap_or(usize::MAX)
            .cmp(
                &lp_rounded_target_by_phase
                    .get(right)
                    .copied()
                    .unwrap_or(usize::MAX),
            )
            .then_with(|| {
                left_priority
                    .representative_period
                    .total_cmp(&right_priority.representative_period)
            })
            .then_with(|| {
                right_priority
                    .revenue_factor
                    .total_cmp(&left_priority.revenue_factor)
            })
            .then_with(|| {
                right_priority
                    .confidence
                    .total_cmp(&left_priority.confidence)
            })
            .then_with(|| {
                right_priority
                    .dominant_destination_share
                    .total_cmp(&left_priority.dominant_destination_share)
            })
            .then_with(|| {
                left_priority
                    .distribution_skew
                    .total_cmp(&right_priority.distribution_skew)
            })
            .then_with(|| {
                left_priority
                    .pushback_index
                    .cmp(&right_priority.pushback_index)
            })
            .then_with(|| right_priority.bench.cmp(&left_priority.bench))
            .then_with(|| left.cmp(right))
    });
}

fn sort_ready_unit_ids(
    ready_unit_ids: &mut [SchedulingUnitId],
    unit_priority_by_unit: &BTreeMap<SchedulingUnitId, UnitPriorityMetrics>,
    predecessor_ids_by_unit: &BTreeMap<SchedulingUnitId, Vec<SchedulingUnitId>>,
    repaired_target_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) {
    ready_unit_ids.sort_by(|left, right| {
        let left_priority =
            unit_priority_by_unit
                .get(left)
                .copied()
                .unwrap_or(UnitPriorityMetrics {
                    rounded_target: usize::MAX,
                    clamped_target: usize::MAX,
                    discounted_lp_score: f64::NEG_INFINITY,
                    objective_score: f64::NEG_INFINITY,
                    successor_count: 0,
                });
        let right_priority =
            unit_priority_by_unit
                .get(right)
                .copied()
                .unwrap_or(UnitPriorityMetrics {
                    rounded_target: usize::MAX,
                    clamped_target: usize::MAX,
                    discounted_lp_score: f64::NEG_INFINITY,
                    objective_score: f64::NEG_INFINITY,
                    successor_count: 0,
                });
        let left_repair_gap = predecessor_ids_by_unit
            .get(left)
            .map_or(0usize, |predecessor_ids| {
                predecessor_ids
                    .iter()
                    .filter_map(|predecessor_id| {
                        repaired_target_by_unit.get(predecessor_id).copied()
                    })
                    .max()
                    .unwrap_or(0)
                    .saturating_sub(left_priority.clamped_target)
            });
        let right_repair_gap =
            predecessor_ids_by_unit
                .get(right)
                .map_or(0usize, |predecessor_ids| {
                    predecessor_ids
                        .iter()
                        .filter_map(|predecessor_id| {
                            repaired_target_by_unit.get(predecessor_id).copied()
                        })
                        .max()
                        .unwrap_or(0)
                        .saturating_sub(right_priority.clamped_target)
                });
        left_priority
            .clamped_target
            .cmp(&right_priority.clamped_target)
            .then_with(|| right_repair_gap.cmp(&left_repair_gap))
            .then_with(|| {
                right_priority
                    .discounted_lp_score
                    .total_cmp(&left_priority.discounted_lp_score)
            })
            .then_with(|| {
                right_priority
                    .objective_score
                    .total_cmp(&left_priority.objective_score)
            })
            .then_with(|| {
                right_priority
                    .successor_count
                    .cmp(&left_priority.successor_count)
            })
            .then_with(|| left.cmp(right))
    });
}

fn build_unit_objective_score_index(
    scheduling_problem: &SchedulingProblem,
) -> BTreeMap<SchedulingUnitId, f64> {
    let mut objective_score_by_unit = BTreeMap::<SchedulingUnitId, f64>::new();
    for objective_term in scheduling_problem.objective_terms() {
        objective_score_by_unit
            .entry(objective_term.unit_id().clone())
            .and_modify(|current| {
                if objective_term.value() > *current {
                    *current = objective_term.value();
                }
            })
            .or_insert(objective_term.value());
    }
    for unit in scheduling_problem.units() {
        objective_score_by_unit
            .entry(unit.unit_id().clone())
            .or_insert(0.0);
    }
    objective_score_by_unit
}

fn build_successor_count_by_unit(
    scheduling_problem: &SchedulingProblem,
) -> BTreeMap<SchedulingUnitId, usize> {
    let mut successor_count_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    for unit in scheduling_problem.units() {
        for predecessor_unit_id in unit.predecessor_unit_ids() {
            if let Some(successor_count) = successor_count_by_unit.get_mut(predecessor_unit_id) {
                *successor_count += 1;
            }
        }
    }
    successor_count_by_unit
}

fn compute_topological_depth_by_unit(
    scheduling_problem: &SchedulingProblem,
) -> Result<BTreeMap<SchedulingUnitId, usize>, MineError> {
    let mut depth_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut pending_predecessor_count_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().clone(), unit.predecessor_unit_ids().len()))
        .collect::<BTreeMap<_, _>>();
    let mut successor_ids_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().clone(), Vec::<SchedulingUnitId>::new()))
        .collect::<BTreeMap<_, _>>();
    for unit in scheduling_problem.units() {
        for predecessor_unit_id in unit.predecessor_unit_ids() {
            let successors = successor_ids_by_unit
                .get_mut(predecessor_unit_id)
                .ok_or_else(|| MineError::Planning {
                    message: format!(
                        "LP-guided depth computation found unknown predecessor `{predecessor_unit_id}` for unit `{}`",
                        unit.unit_id()
                    ),
                })?;
            successors.push(unit.unit_id().clone());
        }
    }

    let mut ready_unit_ids = pending_predecessor_count_by_unit
        .iter()
        .filter(|(_, pending_count)| **pending_count == 0)
        .map(|(unit_id, _)| unit_id.clone())
        .collect::<Vec<_>>();
    ready_unit_ids.sort();
    let mut ordered_unit_count = 0usize;
    while let Some(unit_id) = ready_unit_ids.first().cloned() {
        ready_unit_ids.remove(0);
        let current_depth = depth_by_unit.get(&unit_id).copied().unwrap_or(0);
        if let Some(successor_ids) = successor_ids_by_unit.get(&unit_id) {
            for successor_id in successor_ids {
                if let Some(successor_depth) = depth_by_unit.get_mut(successor_id) {
                    *successor_depth = (*successor_depth).max(current_depth + 1);
                }
                let pending_count = pending_predecessor_count_by_unit
                    .get_mut(successor_id)
                    .ok_or_else(|| MineError::Planning {
                        message: format!(
                            "LP-guided depth computation is missing successor `{successor_id}`"
                        ),
                    })?;
                if *pending_count == 0 {
                    return Err(MineError::Planning {
                        message: format!(
                            "LP-guided depth computation predecessor count underflowed for `{successor_id}` while processing `{unit_id}`"
                        ),
                    });
                }
                *pending_count -= 1;
                if *pending_count == 0 {
                    ready_unit_ids.push(successor_id.clone());
                }
            }
        }
        ready_unit_ids.sort();
        ordered_unit_count += 1;
    }
    if ordered_unit_count != pending_predecessor_count_by_unit.len() {
        return Err(MineError::Planning {
            message: format!(
                "LP-guided depth computation detected cyclic or disconnected unit precedences: ordered {ordered_unit_count} of {} units",
                pending_predecessor_count_by_unit.len()
            ),
        });
    }
    Ok(depth_by_unit)
}

fn phase_id_for_unit(unit_id: &SchedulingUnitId) -> &str {
    unit_id
        .as_str()
        .split("::part-")
        .next()
        .unwrap_or_else(|| unit_id.as_str())
}

fn round_period_index(period_index: f64) -> usize {
    period_index.round().max(0.0) as usize
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::build_target_period_seeded_schedule_from_lp_round_repair_v6_focused;
    use crate::marvin_support::{
        MarvinScheduleAssignment, MarvinScheduleProblemKind, MarvinScheduleSolution,
    };
    use mine_sdk::{
        Metadata, ModelId, NestingAccessRules, PhaseDesign, PushbackPlan, ScenarioId,
        ScheduleDestinationCapacity, ScheduleDestinationId, SchedulingObjectiveTerm,
        SchedulingPeriod, SchedulingProblem, SchedulingResourceBound, SchedulingResourceId,
        SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId,
    };

    #[test]
    fn focused_round_repair_executes_real_local_optimizer_runtime() {
        let phase_plan = sample_phase_plan();
        let scheduling_problem = sample_scheduling_problem();
        let lp_solution = MarvinScheduleSolution {
            kind: MarvinScheduleProblemKind::Pcpsp,
            assignments: vec![
                MarvinScheduleAssignment {
                    linear_index: 0,
                    destination_index: 0,
                    period_index: 1,
                    fraction: 1.0,
                },
                MarvinScheduleAssignment {
                    linear_index: 1,
                    destination_index: 0,
                    period_index: 0,
                    fraction: 1.0,
                },
            ],
            unique_block_count: 2,
        };

        let (artifacts, schedule) =
            build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
                &phase_plan,
                &scheduling_problem,
                &lp_solution,
                None,
                Metadata::new(),
            )
            .expect("focused round/repair should build an optimized seeded schedule");

        let diagnostics = &artifacts.unit_round_repair.local_optimizer_diagnostics;
        assert_eq!(
            diagnostics.strategy_label,
            "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
        );
        assert_eq!(
            diagnostics.budget_profile.mode_label,
            "focused-refresh-budgeted"
        );
        assert!(diagnostics.max_iteration_count > 0);
        assert!(diagnostics.executed_iteration_count > 0);
        assert!(
            artifacts
                .unit_round_repair
                .target_score_decomposition
                .rounded_discounted_target_score_proxy
                >= artifacts
                    .unit_round_repair
                    .target_score_decomposition
                    .repaired_discounted_target_score_proxy
        );
        assert!(
            artifacts
                .unit_round_repair
                .target_score_decomposition
                .local_search_discounted_target_score_proxy
                >= artifacts
                    .unit_round_repair
                    .target_score_decomposition
                    .repaired_discounted_target_score_proxy
        );
        assert!(!super::local_optimizer_runtime_was_skipped(
            &diagnostics.termination_reason
        ));
        assert!(artifacts.unit_round_repair.local_improvement_move_count > 0);
        assert!(!diagnostics.residual_opportunity.improving_move_available);
        assert_eq!(diagnostics.residual_opportunity.move_kind_label, "none");
        assert_eq!(diagnostics.residual_opportunity.discounted_gain, 0.0);
        assert_eq!(
            artifacts
                .unit_round_repair
                .target_period_by_unit
                .get(&SchedulingUnitId::new("phase-a::part-0").expect("unit id should be valid")),
            Some(&0)
        );
        assert_eq!(
            artifacts
                .unit_round_repair
                .target_period_by_unit
                .get(&SchedulingUnitId::new("phase-b::part-0").expect("unit id should be valid")),
            Some(&1)
        );
        assert_eq!(schedule.entries().len(), 2);
    }

    #[test]
    fn focused_round_repair_surfaces_explicit_non_trivial_budget_profile() {
        let phase_plan = sample_phase_plan();
        let scheduling_problem = sample_scheduling_problem();
        let lp_solution = MarvinScheduleSolution {
            kind: MarvinScheduleProblemKind::Pcpsp,
            assignments: vec![
                MarvinScheduleAssignment {
                    linear_index: 0,
                    destination_index: 0,
                    period_index: 1,
                    fraction: 1.0,
                },
                MarvinScheduleAssignment {
                    linear_index: 1,
                    destination_index: 0,
                    period_index: 0,
                    fraction: 1.0,
                },
            ],
            unique_block_count: 2,
        };

        let (artifacts, _) = build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
            &phase_plan,
            &scheduling_problem,
            &lp_solution,
            None,
            Metadata::new(),
        )
        .expect("focused round/repair should build an optimized seeded schedule");

        let diagnostics = &artifacts.unit_round_repair.local_optimizer_diagnostics;
        let budget_profile = &diagnostics.budget_profile;
        assert_eq!(
            super::focused_refresh_local_optimizer_iteration_budget(
                scheduling_problem.units().len(),
                scheduling_problem.periods().len() - 1
            ),
            5
        );
        assert_eq!(budget_profile.mode_label, "focused-refresh-budgeted");
        assert_eq!(budget_profile.target_unit_count, 2);
        assert_eq!(budget_profile.horizon_period_count, 2);
        assert_eq!(budget_profile.full_iteration_budget, 8);
        assert_eq!(budget_profile.requested_iteration_budget, 5);
        assert_eq!(budget_profile.effective_iteration_budget, 5);
        assert_eq!(diagnostics.max_iteration_count, 5);
        assert_eq!(
            diagnostics.max_iteration_count,
            budget_profile.effective_iteration_budget
        );
        assert_eq!(
            artifacts
                .unit_round_repair
                .target_score_decomposition
                .repair_score_delta_vs_round_proxy,
            artifacts
                .unit_round_repair
                .target_score_decomposition
                .repaired_discounted_target_score_proxy
                - artifacts
                    .unit_round_repair
                    .target_score_decomposition
                    .rounded_discounted_target_score_proxy
        );
        assert_eq!(
            artifacts
                .unit_round_repair
                .target_score_decomposition
                .local_search_score_delta_vs_round_proxy,
            artifacts
                .unit_round_repair
                .target_score_decomposition
                .local_search_discounted_target_score_proxy
                - artifacts
                    .unit_round_repair
                    .target_score_decomposition
                    .rounded_discounted_target_score_proxy
        );
    }

    #[test]
    fn local_optimizer_budget_hit_surfaces_explicit_residual_headroom_payload() {
        let unit_a = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
        let unit_b = SchedulingUnitId::new("unit-b").expect("unit id should be valid");
        let unit_c = SchedulingUnitId::new("unit-c").expect("unit id should be valid");
        let mut target_period_by_unit = BTreeMap::from([
            (unit_a.clone(), 0usize),
            (unit_b.clone(), 1usize),
            (unit_c.clone(), 2usize),
        ]);
        let base_target_period_by_unit = target_period_by_unit.clone();
        let predecessor_ids_by_unit = BTreeMap::from([
            (unit_a.clone(), Vec::new()),
            (unit_b.clone(), Vec::new()),
            (unit_c.clone(), Vec::new()),
        ]);
        let successor_unit_ids_by_unit = BTreeMap::from([
            (unit_a.clone(), Vec::new()),
            (unit_b.clone(), Vec::new()),
            (unit_c.clone(), Vec::new()),
        ]);
        let objective_score_by_unit = BTreeMap::from([
            (unit_a.clone(), 1.0),
            (unit_b.clone(), 2.0),
            (unit_c.clone(), 100.0),
        ]);

        let diagnostics = super::optimize_unit_target_periods_locally(
            &mut target_period_by_unit,
            &base_target_period_by_unit,
            &predecessor_ids_by_unit,
            &successor_unit_ids_by_unit,
            &objective_score_by_unit,
            2.0,
            2,
            super::LpBzLocalOptimizerBudgetProfile {
                mode_label: "test-budget-hit".to_owned(),
                target_unit_count: 3,
                horizon_period_count: 3,
                full_iteration_budget: 6,
                requested_iteration_budget: 1,
                effective_iteration_budget: 1,
            },
        );

        assert_eq!(diagnostics.termination_reason, "max-iterations-reached");
        assert_eq!(diagnostics.executed_iteration_count, 1);
        assert_eq!(diagnostics.improving_move_count, 1);
        if diagnostics.residual_opportunity.improving_move_available {
            assert_ne!(diagnostics.residual_opportunity.move_kind_label, "none");
            assert!(diagnostics.residual_opportunity.discounted_gain > 0.0);
        } else {
            assert_eq!(diagnostics.residual_opportunity.move_kind_label, "none");
            assert_eq!(diagnostics.residual_opportunity.discounted_gain, 0.0);
        }
        assert!(
            target_period_by_unit
                .get(&unit_c)
                .copied()
                .is_some_and(|period| period < 2)
        );
    }

    fn sample_phase_plan() -> PushbackPlan {
        PushbackPlan {
            phases: vec![
                PhaseDesign {
                    phase_id: "phase-a".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(1.0),
                    bench: Some(100),
                    block_indices: vec![0],
                    block_count: 1,
                    total_tonnage: Some(1.0),
                    predecessor_phase_ids: Vec::new(),
                },
                PhaseDesign {
                    phase_id: "phase-b".to_owned(),
                    pushback_index: 1,
                    shell_index: Some(1),
                    revenue_factor: Some(1.0),
                    bench: Some(99),
                    block_indices: vec![1],
                    block_count: 1,
                    total_tonnage: Some(1.0),
                    predecessor_phase_ids: Vec::new(),
                },
            ],
            phase_count: 2,
            total_block_count: 2,
            total_tonnage: Some(2.0),
            nesting_rules: NestingAccessRules::strict_sequential(),
            limitations: Vec::new(),
        }
    }

    fn sample_scheduling_problem() -> SchedulingProblem {
        let destination_id =
            ScheduleDestinationId::new("mill").expect("destination should be valid");
        let resource_id =
            SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
        let unit_a_id = SchedulingUnitId::new("phase-a::part-0").expect("unit id should be valid");
        let unit_b_id = SchedulingUnitId::new("phase-b::part-0").expect("unit id should be valid");

        SchedulingProblem::new(
            ScenarioId::new("focused-rounder-test").expect("scenario should be valid"),
            ModelId::new("focused-rounder-model").expect("model should be valid"),
            vec![
                SchedulingPeriod::new(
                    "P1",
                    vec![
                        SchedulingResourceBound::new(resource_id.clone(), None, Some(10.0))
                            .expect("resource bound should be valid"),
                    ],
                    vec![
                        ScheduleDestinationCapacity::new(destination_id.clone(), Some(10.0))
                            .expect("destination capacity should be valid"),
                    ],
                    vec![],
                )
                .expect("period should be valid"),
                SchedulingPeriod::new(
                    "P2",
                    vec![
                        SchedulingResourceBound::new(resource_id.clone(), None, Some(10.0))
                            .expect("resource bound should be valid"),
                    ],
                    vec![
                        ScheduleDestinationCapacity::new(destination_id.clone(), Some(10.0))
                            .expect("destination capacity should be valid"),
                    ],
                    vec![],
                )
                .expect("period should be valid"),
            ],
            vec![
                SchedulingUnit::new(
                    unit_a_id.clone(),
                    1.0,
                    1,
                    vec![],
                    vec![destination_id.clone()],
                    vec![],
                    vec![0],
                    Some(100),
                    Some(0),
                    Metadata::new(),
                )
                .expect("unit a should be valid"),
                SchedulingUnit::new(
                    unit_b_id.clone(),
                    1.0,
                    1,
                    vec![],
                    vec![destination_id.clone()],
                    vec![],
                    vec![1],
                    Some(99),
                    Some(1),
                    Metadata::new(),
                )
                .expect("unit b should be valid"),
            ],
            vec![
                SchedulingObjectiveTerm::new(
                    unit_a_id.clone(),
                    Some(destination_id.clone()),
                    100.0,
                )
                .expect("objective term should be valid"),
                SchedulingObjectiveTerm::new(unit_b_id.clone(), Some(destination_id.clone()), 10.0)
                    .expect("objective term should be valid"),
            ],
            vec![
                SchedulingResourceRequirement::new(unit_a_id, resource_id.clone(), None, 1.0)
                    .expect("resource requirement should be valid"),
                SchedulingResourceRequirement::new(unit_b_id, resource_id, None, 1.0)
                    .expect("resource requirement should be valid"),
            ],
            vec![destination_id],
            vec![],
            0.10,
            Metadata::new(),
            vec![],
        )
        .expect("scheduling problem should be valid")
    }
}
