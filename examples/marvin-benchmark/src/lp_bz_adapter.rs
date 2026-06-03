//! Adaptador LP/BZ angosto y explícitamente acotado al benchmark Marvin.
//!
//! Está pensado para reuso interno desde otros bins del paquete `marvin-benchmark`
//! sin exponer artefactos grandes del kernel o del rounder.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;

use mine_sdk::{LongTermSchedule, Metadata, MineError, PushbackPlan, SchedulingProblem};
use serde::Serialize;

use crate::lp_bz_bound::{LpBzBoundArtifact, LpBzInputArtifact, compute_lp_bz_bound_artifacts};
use crate::lp_bz_lp_kernel::{
    LpBzCutSolveDiagnostics, LpBzLpKernelArtifact, LpBzLpSolveArtifact, LpBzLpSolveStatus,
    LpBzPrecedenceSolveDiagnostics, build_lp_bz_lp_kernel_artifact, solve_lp_bz_lp_kernel_artifact,
};
use crate::lp_bz_rounder::{
    build_target_period_seeded_schedule_from_lp_round_repair_v6_focused,
    local_optimizer_runtime_was_skipped, representative_period_by_block,
};
use crate::lp_bz_runtime_budget::{
    LpBzLocalOptimizerRuntimeBudgetContract, build_lp_bz_local_optimizer_runtime_budget_contract,
    validate_lp_bz_local_optimizer_runtime_budget_contract,
};
use crate::marvin_support::{MarvinScheduleProblem, MarvinScheduleSolution};

pub const MARVIN_FOCUSED_LP_BZ_ADAPTER_SCOPE: &str = "marvin-focused-lp-bz-adapter";

const MARVIN_SCOPED_LIMITATION: &str = "This adapter is intentionally Marvin-scoped inside examples/marvin-benchmark; MineLib aliases may reuse the normalized Marvin contracts here, but this path must not be read as a generic LP/BZ adapter for every MineLib dataset yet.";
const FOCUSED_ROUND_REPAIR_LIMITATION: &str = "The focused LP/BZ candidate comes from build_target_period_seeded_schedule_from_lp_round_repair_v6_focused, which now keeps the benchmark-side local optimizer on an explicit single-iteration deterministic guardrail; treat it as the optimized benchmark-side LP/BZ candidate while native solve comparability remains tracked separately.";

#[derive(Debug)]
pub struct MarvinLpBzAdapterResult {
    pub summary: MarvinLpBzAdapterSummary,
    pub seeded_schedule: LongTermSchedule,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarvinLpBzAdapterSummary {
    pub scope_label: String,
    pub lp_relaxation_assignment_count: usize,
    pub lp_relaxation_unique_block_count: usize,
    pub representative_period_block_count: usize,
    pub seeded_schedule_entry_count: usize,
    pub seeded_schedule_violation_count: usize,
    pub lp_bz_inputs: LpBzInputArtifact,
    pub lp_bz_bound: LpBzBoundArtifact,
    pub lp_bz_lp_kernel: MarvinLpBzAdapterLpKernelSummary,
    pub lp_bz_lp_solve: MarvinLpBzAdapterLpSolveSummary,
    pub lp_bz_round_repair: MarvinLpBzAdapterRoundRepairSummary,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarvinLpBzAdapterLpKernelSummary {
    pub kernel_label: String,
    pub variable_count: usize,
    pub non_zero_objective_coefficient_count: usize,
    pub capacity_row_count: usize,
    pub activation_row_count: usize,
    pub precedence_row_count: usize,
    pub access_unit_profile_count: usize,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarvinLpBzAdapterLpSolveSummary {
    pub solver_label: String,
    pub solve_status: LpBzLpSolveStatus,
    pub discounted_objective_bound: Option<f64>,
    pub active_variable_count: usize,
    pub min_positive_variable_value: Option<f64>,
    pub max_variable_value: Option<f64>,
    pub precedence_diagnostics: LpBzPrecedenceSolveDiagnostics,
    pub cut_diagnostics: LpBzCutSolveDiagnostics,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarvinLpBzAdapterRoundRepairSummary {
    pub rounder_strategy_label: String,
    pub focused_round_repair: bool,
    pub local_optimization_skipped: bool,
    pub local_optimizer_runtime_budget_contract: LpBzLocalOptimizerRuntimeBudgetContract,
    pub local_optimizer_strategy_label: String,
    pub local_optimizer_termination_reason: String,
    pub local_optimizer_executed_iteration_count: usize,
    pub local_optimizer_improving_move_count: usize,
    pub repaired_phase_target_count: usize,
    pub repaired_unit_target_count: usize,
    pub horizon_clamp_count: usize,
    pub phase_target_count: usize,
    pub unit_target_count: usize,
    pub limitations: Vec<String>,
}

pub fn run_marvin_focused_lp_bz_adapter(
    phase_plan: &PushbackPlan,
    scheduling_problem: &SchedulingProblem,
    marvin_problem: &MarvinScheduleProblem,
    lp_relaxation_solution: &MarvinScheduleSolution,
    lp_relaxation_reference_path: &Path,
    repo_root: &Path,
    unit_granularity_label: &str,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<MarvinLpBzAdapterResult, MineError> {
    let lp_representative_period_by_block = representative_period_by_block(lp_relaxation_solution);
    let (round_repair_artifacts, seeded_schedule) =
        build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
            phase_plan,
            scheduling_problem,
            lp_relaxation_solution,
            max_vertical_advance,
            metadata,
        )?;

    let lp_bz_bound_artifacts = compute_lp_bz_bound_artifacts(
        marvin_problem,
        lp_relaxation_solution,
        lp_relaxation_reference_path,
        repo_root,
        scheduling_problem.units().len(),
        scheduling_problem
            .units()
            .iter()
            .map(|unit| unit.predecessor_unit_ids().len())
            .sum(),
        unit_granularity_label,
    )?;
    let lp_bz_lp_kernel_artifact = build_lp_bz_lp_kernel_artifact(scheduling_problem)?;
    validate_lp_bz_artifact_coherence(
        &lp_bz_bound_artifacts.lp_bz_inputs,
        &lp_bz_bound_artifacts.lp_bz_bound_artifact,
        &lp_bz_lp_kernel_artifact,
    )?;
    let lp_bz_lp_solve_artifact = solve_lp_bz_lp_kernel_artifact(&lp_bz_lp_kernel_artifact)?;
    let local_optimizer_runtime_budget_contract =
        build_lp_bz_local_optimizer_runtime_budget_contract(
            &round_repair_artifacts
                .unit_round_repair
                .local_optimizer_diagnostics
                .strategy_label,
            round_repair_artifacts
                .unit_round_repair
                .local_optimizer_diagnostics
                .max_iteration_count,
            round_repair_artifacts
                .unit_round_repair
                .local_optimizer_diagnostics
                .executed_iteration_count,
            &round_repair_artifacts
                .unit_round_repair
                .local_optimizer_diagnostics
                .termination_reason,
        );
    validate_lp_bz_local_optimizer_runtime_budget_contract(
        &local_optimizer_runtime_budget_contract,
    )
    .map_err(MineError::validation)?;

    let round_repair_limitations = vec![FOCUSED_ROUND_REPAIR_LIMITATION.to_owned()];
    let lp_bz_round_repair = MarvinLpBzAdapterRoundRepairSummary {
        rounder_strategy_label: "lp-bz-rounder-v6-focused-seeded".to_owned(),
        focused_round_repair: true,
        local_optimization_skipped: local_optimizer_runtime_was_skipped(
            &round_repair_artifacts
                .unit_round_repair
                .local_optimizer_diagnostics
                .termination_reason,
        ),
        local_optimizer_runtime_budget_contract,
        local_optimizer_strategy_label: round_repair_artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .strategy_label
            .clone(),
        local_optimizer_termination_reason: round_repair_artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .termination_reason
            .clone(),
        local_optimizer_executed_iteration_count: round_repair_artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .executed_iteration_count,
        local_optimizer_improving_move_count: round_repair_artifacts
            .unit_round_repair
            .local_improvement_move_count,
        repaired_phase_target_count: round_repair_artifacts.repaired_phase_target_count,
        repaired_unit_target_count: round_repair_artifacts
            .unit_round_repair
            .repaired_unit_target_count,
        horizon_clamp_count: round_repair_artifacts.unit_round_repair.horizon_clamp_count,
        phase_target_count: round_repair_artifacts.phase_target_period_by_phase.len(),
        unit_target_count: round_repair_artifacts
            .unit_round_repair
            .target_period_by_unit
            .len(),
        limitations: round_repair_limitations.clone(),
    };
    validate_lp_bz_round_repair_runtime_budget_contract(&lp_bz_round_repair)?;
    let summary = MarvinLpBzAdapterSummary {
        scope_label: MARVIN_FOCUSED_LP_BZ_ADAPTER_SCOPE.to_owned(),
        lp_relaxation_assignment_count: lp_relaxation_solution.assignments.len(),
        lp_relaxation_unique_block_count: lp_relaxation_solution.unique_block_count,
        representative_period_block_count: lp_representative_period_by_block.len(),
        seeded_schedule_entry_count: seeded_schedule.entries().len(),
        seeded_schedule_violation_count: seeded_schedule.violations().len(),
        lp_bz_inputs: lp_bz_bound_artifacts.lp_bz_inputs,
        lp_bz_bound: lp_bz_bound_artifacts.lp_bz_bound_artifact,
        lp_bz_lp_kernel: build_lp_kernel_summary(&lp_bz_lp_kernel_artifact),
        lp_bz_lp_solve: build_lp_solve_summary(&lp_bz_lp_solve_artifact),
        lp_bz_round_repair,
        limitations: merge_limitations([
            vec![MARVIN_SCOPED_LIMITATION.to_owned()],
            round_repair_limitations,
            lp_bz_lp_kernel_artifact.limitations.clone(),
            lp_bz_lp_solve_artifact.limitations.clone(),
        ]),
    };

    Ok(MarvinLpBzAdapterResult {
        summary,
        seeded_schedule,
    })
}

fn build_lp_kernel_summary(artifact: &LpBzLpKernelArtifact) -> MarvinLpBzAdapterLpKernelSummary {
    MarvinLpBzAdapterLpKernelSummary {
        kernel_label: artifact.kernel_label.clone(),
        variable_count: artifact.variable_index.variable_count,
        non_zero_objective_coefficient_count: artifact.objective.summary.non_zero_coefficient_count,
        capacity_row_count: artifact.constraints.summary.capacity_row_count,
        activation_row_count: artifact.constraints.summary.activation_row_count,
        precedence_row_count: artifact.constraints.summary.precedence_row_count,
        access_unit_profile_count: artifact.access.unit_profile_count,
        limitations: artifact.limitations.clone(),
    }
}

fn build_lp_solve_summary(artifact: &LpBzLpSolveArtifact) -> MarvinLpBzAdapterLpSolveSummary {
    MarvinLpBzAdapterLpSolveSummary {
        solver_label: artifact.solver_label.clone(),
        solve_status: artifact.solve_status,
        discounted_objective_bound: artifact.discounted_objective_bound,
        active_variable_count: artifact.active_variable_count,
        min_positive_variable_value: artifact.min_positive_variable_value,
        max_variable_value: artifact.max_variable_value,
        precedence_diagnostics: artifact.precedence_diagnostics.clone(),
        cut_diagnostics: artifact.cut_diagnostics.clone(),
        limitations: artifact.limitations.clone(),
    }
}

fn merge_limitations(groups: impl IntoIterator<Item = Vec<String>>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for group in groups {
        for limitation in group {
            if seen.insert(limitation.clone()) {
                merged.push(limitation);
            }
        }
    }
    merged
}

fn validate_lp_bz_round_repair_runtime_budget_contract(
    round_repair: &MarvinLpBzAdapterRoundRepairSummary,
) -> Result<(), MineError> {
    validate_lp_bz_local_optimizer_runtime_budget_contract(
        &round_repair.local_optimizer_runtime_budget_contract,
    )
    .map_err(MineError::validation)?;
    if round_repair
        .local_optimizer_runtime_budget_contract
        .strategy_label
        != round_repair.local_optimizer_strategy_label
        || round_repair
            .local_optimizer_runtime_budget_contract
            .executed_iteration_count
            != round_repair.local_optimizer_executed_iteration_count
        || round_repair
            .local_optimizer_runtime_budget_contract
            .termination_reason
            != round_repair.local_optimizer_termination_reason
    {
        return Err(MineError::validation(
            "Focused LP/BZ adapter round-repair summary must keep strategy, iterations and termination metadata aligned with the explicit runtime budget contract."
                .to_owned(),
        ));
    }
    if round_repair.local_optimization_skipped
        != local_optimizer_runtime_was_skipped(&round_repair.local_optimizer_termination_reason)
    {
        return Err(MineError::validation(
            "Focused LP/BZ adapter round-repair summary must derive `local_optimization_skipped` from the surfaced termination reason."
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_lp_bz_artifact_coherence(
    lp_bz_inputs: &LpBzInputArtifact,
    lp_bz_bound_artifact: &LpBzBoundArtifact,
    lp_bz_lp_kernel_artifact: &LpBzLpKernelArtifact,
) -> Result<(), MineError> {
    let normalized_period_count = lp_bz_inputs.problem_normalization.period_count;
    let normalized_destination_count = lp_bz_inputs.problem_normalization.destination_count;
    let normalized_unit_count = lp_bz_inputs.precedence_units.unit_count;
    let normalized_discount_rate = lp_bz_inputs.problem_normalization.discount_rate;

    if lp_bz_bound_artifact.period_count != normalized_period_count {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_bound_artifact.period_count={} but lp_bz_inputs.problem_normalization.period_count={normalized_period_count}",
            lp_bz_bound_artifact.period_count
        )));
    }
    if lp_bz_bound_artifact.destination_count != normalized_destination_count {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_bound_artifact.destination_count={} but lp_bz_inputs.problem_normalization.destination_count={normalized_destination_count}",
            lp_bz_bound_artifact.destination_count
        )));
    }
    if lp_bz_bound_artifact.unit_count != normalized_unit_count {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_bound_artifact.unit_count={} but lp_bz_inputs.precedence_units.unit_count={normalized_unit_count}",
            lp_bz_bound_artifact.unit_count
        )));
    }
    if lp_bz_lp_kernel_artifact.period_count != normalized_period_count {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_lp_kernel_artifact.period_count={} but lp_bz_inputs.problem_normalization.period_count={normalized_period_count}",
            lp_bz_lp_kernel_artifact.period_count
        )));
    }
    if lp_bz_lp_kernel_artifact.destination_count != normalized_destination_count {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_lp_kernel_artifact.destination_count={} but lp_bz_inputs.problem_normalization.destination_count={normalized_destination_count}",
            lp_bz_lp_kernel_artifact.destination_count
        )));
    }
    if lp_bz_lp_kernel_artifact.unit_count != normalized_unit_count {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_lp_kernel_artifact.unit_count={} but lp_bz_inputs.precedence_units.unit_count={normalized_unit_count}",
            lp_bz_lp_kernel_artifact.unit_count
        )));
    }
    if (lp_bz_lp_kernel_artifact.discount_rate - normalized_discount_rate).abs() > 1.0e-12 {
        return Err(MineError::validation(format!(
            "LP/BZ adapter coherence error: lp_bz_lp_kernel_artifact.discount_rate={} but lp_bz_inputs.problem_normalization.discount_rate={normalized_discount_rate}",
            lp_bz_lp_kernel_artifact.discount_rate
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MarvinLpBzAdapterRoundRepairSummary, validate_lp_bz_round_repair_runtime_budget_contract,
    };
    use crate::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract;

    #[test]
    fn round_repair_runtime_budget_contract_accepts_budget_hit_without_skip_regression() {
        let round_repair = MarvinLpBzAdapterRoundRepairSummary {
            rounder_strategy_label: "lp-bz-rounder-v6-focused-seeded".to_owned(),
            focused_round_repair: true,
            local_optimization_skipped: false,
            local_optimizer_runtime_budget_contract:
                build_lp_bz_local_optimizer_runtime_budget_contract(
                    "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8",
                    12,
                    12,
                    "max-iterations-reached",
                ),
            local_optimizer_strategy_label:
                "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
                    .to_owned(),
            local_optimizer_termination_reason: "max-iterations-reached".to_owned(),
            local_optimizer_executed_iteration_count: 12,
            local_optimizer_improving_move_count: 1,
            repaired_phase_target_count: 4,
            repaired_unit_target_count: 8,
            horizon_clamp_count: 0,
            phase_target_count: 4,
            unit_target_count: 12,
            limitations: Vec::new(),
        };

        validate_lp_bz_round_repair_runtime_budget_contract(&round_repair)
            .expect("budget-hit runtime contract should validate");
        assert_eq!(
            round_repair
                .local_optimizer_runtime_budget_contract
                .execution_state,
            "budget-hit"
        );
        assert!(!round_repair.local_optimization_skipped);
    }
}
