//! Ejecuta una validacion multi-mine del scheduler sobre instancias MineLib abiertas.
//!
//! Uso:
//!   cargo run -p marvin-benchmark --bin multi_mine_scheduler [output_path]
//!
//! Si no se especifica `output_path`, el reporte se escribe en
//! `datasets/benchmarks/outputs/multi-mine-scheduling-report.json`.
//! Las rutas CLI relativas se rebasan contra la raíz del repo para evitar fallos sensibles al cwd.

#[path = "../benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
#[path = "../benchmark_path_policy.rs"]
mod benchmark_path_policy;
#[path = "../comparability_gap_support.rs"]
mod comparability_gap_support;
#[path = "../lp_bz_adapter.rs"]
mod lp_bz_adapter;
#[path = "../lp_bz_bound.rs"]
mod lp_bz_bound;
#[path = "../lp_bz_lp_kernel.rs"]
mod lp_bz_lp_kernel;
#[path = "../lp_bz_promotion_readiness.rs"]
mod lp_bz_promotion_readiness;
#[path = "../lp_bz_rounder.rs"]
mod lp_bz_rounder;
#[path = "../lp_bz_runtime_budget.rs"]
mod lp_bz_runtime_budget;
#[path = "../marvin_support.rs"]
mod marvin_support;
#[path = "../minelib_scheduling_support.rs"]
mod minelib_scheduling_support;
#[path = "../pushback_bench_localized_cut_support.rs"]
mod pushback_bench_localized_cut_support;
#[path = "../temporal_routing_promotion_gate.rs"]
mod temporal_routing_promotion_gate;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use benchmark_blocks_support::read_benchmark_blocks;
use benchmark_path_policy::BenchmarkPathPolicy;
use comparability_gap_support::{
    ComparabilityGapSource, ComparabilityGapSummary, derive_comparability_gaps,
    validate_comparability_gap_contract_consistency,
};
use lp_bz_adapter::{MarvinLpBzAdapterSummary, run_marvin_focused_lp_bz_adapter};
use lp_bz_promotion_readiness::{
    LpBzPromotionReadinessSummary, build_lp_bz_promotion_readiness_summary,
    validate_lp_bz_promotion_readiness_summary,
};
use lp_bz_runtime_budget::validate_lp_bz_local_optimizer_runtime_budget_contract;
use marvin_support::{
    MinelibScheduleAssignment, MinelibScheduleProblem, MinelibScheduleSolution,
    MinelibScheduleSolutionSummary, read_minelib_cpit_problem, read_minelib_cpit_solution,
    read_minelib_lp_cpit_solution, read_minelib_lp_pcpsp_solution, read_minelib_pcpsp_problem,
    read_minelib_pcpsp_solution, read_minelib_precedence_graph, read_minelib_upit_block_values,
    summarize_minelib_schedule_solution,
};
use mine_sdk::{
    ColumnId, DecomposedSchedulingConfig, Metadata, MineError, NumericMetricComparisonReport,
    PushbackPlan, SchedulingProblem, compare_named_numeric_metrics,
    solve_decomposed_scheduling_problem,
};
use minelib_scheduling_support::{
    MARVIN_PREFERRED_NESTED_SHELL_FACTOR_COUNT, MARVIN_SELECTED_BLOCK_SOURCE,
    MarvinPreferredNestedShellFamilyContract, MinelibResourceRole, REFERENCE_SELECTED_BLOCK_SOURCE,
    build_candidate_period_memberships, build_linear_index_float_lookup,
    build_linear_index_to_row_index, build_marvin_phase_plan_from_revenue_factor_shells,
    build_marvin_preferred_nested_shell_family_contract,
    build_preferred_phase_plan_for_minelib_scheduling,
    build_scheduling_problem_from_minelib_problem,
};
use pushback_bench_localized_cut_support::{
    MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE,
    MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL, MARVIN_MR187_PAPERLIKE_CANDIDATE_ROLE,
    MARVIN_MR187_PROMOTED_FAMILY_IS_ACTIVE_CANDIDATE,
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL,
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_FAMILY_LABEL,
    PushbackBenchLocalizedCutAccessPolicySummary, PushbackBenchLocalizedCutBuildArtifacts,
    PushbackBenchLocalizedCutBuildConfig, PushbackBenchLocalizedCutFrontProgression,
    PushbackBenchLocalizedCutPredecessorLinkPolicy, PushbackBenchLocalizedCutRefinementDiagnostics,
    PushbackBenchLocalizedCutUnitFamilyTraceability,
    build_marvin_mr187_promoted_pushback_bench_localized_cut_contract_surfaces,
    build_pushback_bench_localized_cut_benchmark_artifacts,
    format_promoted_lp_bz_bibliographic_gap_summary, format_promoted_lp_bz_family_status_summary,
    format_promoted_pushback_bench_localized_cut_family_summary,
    format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary,
    validate_promoted_pushback_bench_localized_cut_access_law_contract,
    validate_promoted_pushback_bench_localized_cut_unit_family_traceability,
};
#[cfg(test)]
use pushback_bench_localized_cut_support::{
    build_promoted_pushback_bench_localized_cut_unit_family_traceability,
    summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law,
};
use serde::Serialize;
use temporal_routing_promotion_gate::{
    TemporalRoutingPromotionGateSummary, build_temporal_routing_promotion_gate_summary,
    validate_temporal_routing_promotion_gate_summary,
};

const NESTED_SHELL_PROBE_FACTOR_COUNT: usize = MARVIN_PREFERRED_NESTED_SHELL_FACTOR_COUNT;
const LP_BZ_UNIT_GRANULARITY_LABEL: &str =
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_FAMILY_LABEL;
const LP_BZ_CUT_BUILDER_LABEL: &str = MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL;
const LP_BZ_COMPETITIVE_READINESS_CRITERIA_VERSION: &str = "mr205-v3";
const MCLAUGHLIN_LIMIT_BENCHMARK_CUT_CONTRACT_VERSION: &str = "mr207-v2";
const MCLAUGHLIN_LIMIT_LP_BZ_SIDECAR_VERSION: &str = "mr207-v1";
const MCLAUGHLIN_LIMIT_PROMOTION_CHECKLIST_VERSION: &str = "mr207-v6";
const MCLAUGHLIN_LIMIT_BENCHMARK_CUT_UNIT_FAMILY_LABEL: &str =
    "mclaughlin-limit-pushback-bench-localized-cut-phase";
const MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILDER_LABEL: &str =
    "mclaughlin-limit-pushback-bench-localized-mining-cuts";
const MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_LABEL: &str = "front3-ar2.0-span2-n6-limit";
const MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG: PushbackBenchLocalizedCutBuildConfig =
    PushbackBenchLocalizedCutBuildConfig {
        max_front_count: 3,
        min_aspect_ratio: 2.0,
        min_dominant_span: 2,
        include_touching_neighbors: true,
        max_local_predecessor_count: Some(6),
        predecessor_cut_link_policy:
            PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
        front_progression:
            PushbackBenchLocalizedCutFrontProgression::PreferredThreeFrontCumulativeTargetsWithUniformFallback {
                label: "uniform-33-67-100",
                cumulative_tonnage_targets: [1.0 / 3.0, 2.0 / 3.0, 1.0],
            },
    };
const LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION: &str =
    "proxy-covers-measured-ready-frontier-gap";
const LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER: &str =
    "proxy-covered-measured-ready-frontier-gap";
const LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE: &str =
    "need-schedule-level-ready-frontier-proof";
const LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS: &str = "diagnostic-only";
const MARVIN_SIDECAR_TRACEABILITY_FIELD_SUFFIXES: &[&str] = &[
    "selected_block_provenance.selected_block_source",
    "selected_block_provenance.selected_block_count",
    "preferred_phase_plan_proxy.aggregation_strategy",
    "preferred_phase_plan_proxy.preferred_nested_shell_factor_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_realized_shell_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_access_mode",
    "preferred_phase_plan_proxy.preferred_phase_count",
    "localized_cut_builder_provenance.localized_cut_builder_label",
    "localized_cut_builder_provenance.localized_cut_builder_build_label",
    "localized_cut_builder_provenance.scaffold_unit_family_label",
    "localized_cut_builder_provenance.promoted_unit_family_label",
    "localized_cut_builder_provenance.front_progression_label",
    "localized_cut_builder_provenance.promoted_cut_phase_count",
    "localized_cut_builder_provenance.scheduling_unit_count",
];
const MARVIN_SIDECAR_RUNTIME_CONTRACT_FIELD_SUFFIXES: &[&str] = &[
    "summary.lp_bz_lp_kernel.kernel_label",
    "summary.lp_bz_lp_solve.solve_status",
    "summary.lp_bz_lp_solve.precedence_diagnostics.strategy",
    "summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness",
    "summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points",
    "summary.lp_bz_lp_solve.precedence_diagnostics.enforced_precedence_rows",
    "summary.lp_bz_lp_solve.precedence_diagnostics.total_precedence_rows",
    "summary.lp_bz_lp_solve.precedence_diagnostics.skipped_precedence_rows",
    "summary.lp_bz_round_repair.local_optimizer_budget_profile.mode_label",
    "summary.lp_bz_round_repair.local_optimizer_budget_profile.full_iteration_budget",
    "summary.lp_bz_round_repair.local_optimizer_budget_profile.requested_iteration_budget",
    "summary.lp_bz_round_repair.local_optimizer_budget_profile.effective_iteration_budget",
    "summary.lp_bz_round_repair.target_score_decomposition.rounded_discounted_target_score_proxy",
    "summary.lp_bz_round_repair.target_score_decomposition.repaired_discounted_target_score_proxy",
    "summary.lp_bz_round_repair.target_score_decomposition.local_search_discounted_target_score_proxy",
    "summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_repair_proxy",
    "summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_round_proxy",
    "summary.lp_bz_round_repair.competitive_probe.improvement_status",
    "summary.lp_bz_round_repair.competitive_probe.competitive_budget_profile.mode_label",
    "summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.execution_state",
    "summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.budget_hit",
    "summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.summary",
    "summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_termination_reason",
    "summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_executed_iteration_count",
    "summary.lp_bz_round_repair.competitive_probe.competitive_local_search_discounted_target_score_proxy",
    "summary.lp_bz_round_repair.competitive_probe.local_search_score_delta_vs_focused_proxy",
    "summary.lp_bz_round_repair.competitive_probe.target_period_change_count_vs_focused",
    "summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state",
    "summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit",
    "summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.summary",
    "summary.lp_bz_round_repair.local_optimizer_residual_opportunity.improving_move_available",
    "summary.lp_bz_round_repair.local_optimizer_residual_opportunity.move_kind_label",
    "summary.lp_bz_round_repair.local_optimizer_residual_opportunity.discounted_gain",
    "competitive_ready_frontier_probe.driver_targeting_status",
    "competitive_ready_frontier_probe.closure_status",
    "competitive_ready_frontier_probe.competitive_probe_proxy_gap_closure_share",
    "competitive_ready_frontier_probe.residual_ready_frontier_gap_after_competitive_probe_proxy",
    "competitive_ready_frontier_probe.empirical_dominant_blocker",
    "competitive_ready_frontier_probe.empirical_dominant_blocker_summary",
    "competitive_ready_frontier_probe.empirical_driver_evidence_summary",
    "competitive_ready_frontier_probe.empirical_driver_evidence",
    "competitive_ready_frontier_probe.budget_coverage_experiment",
    "competitive_ready_frontier_probe.residual_interpretation",
    "competitive_ready_frontier_probe.residual_interpretation_summary",
    "competitive_ready_frontier_probe.dominant_residual_driver",
    "competitive_ready_frontier_probe.dominant_residual_driver_summary",
    "competitive_ready_frontier_probe.next_step_evidence",
    "competitive_ready_frontier_probe.next_step_evidence_summary",
    "competitive_ready_frontier_probe.parity_claim_status",
    "competitive_ready_frontier_probe.parity_claim_summary",
    "competitive_ready_frontier_probe.remaining_blocker_count",
    "competitive_ready_frontier_probe.remaining_blockers_summary",
    "competitive_ready_frontier_probe.remaining_blockers",
    "competitive_ready_frontier_probe.readiness_criteria_version",
    "competitive_ready_frontier_probe.readiness_state",
    "competitive_ready_frontier_probe.readiness_summary",
    "competitive_ready_frontier_probe.readiness_blocked_criteria_count",
    "competitive_ready_frontier_probe.readiness_criteria",
    "lp_bz_promotion_readiness.summary",
    "lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract.execution_state",
    "lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract.budget_hit",
];
const MARVIN_SIDECAR_PROMOTION_GATE_FIELD_SUFFIXES: &[&str] = &[
    "candidate_vs_reference_period_alignment",
    "candidate_vs_reference_destination_membership",
    "temporal_routing_promotion_gate",
];
const PRIMARY_UNIT_FAMILY_TRACEABILITY_FIELD_SUFFIXES: &[&str] = &[
    "unit_family_label",
    "unit_family_role",
    "literature_alignment_label",
    "selected_block_provenance.selected_block_source",
    "selected_block_provenance.selected_block_count",
    "selected_block_provenance.selected_block_provenance_summary",
    "selected_block_provenance.selected_block_provenance_chain",
    "preferred_phase_plan_proxy.aggregation_strategy",
    "preferred_phase_plan_proxy.preferred_phase_count",
    "preferred_phase_plan_proxy.unique_shell_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_factor_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_realized_shell_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_access_mode",
    "benchmark_side_evidence.benchmark_scope_label",
    "benchmark_side_evidence.mining_unit_evidence_summary",
    "benchmark_side_evidence.cut_evidence_label",
    "benchmark_side_evidence.cut_evidence_summary",
    "benchmark_side_evidence.benchmark_cut_refinement.contract_label",
    "benchmark_side_evidence.benchmark_cut_refinement.contract_version",
    "benchmark_side_evidence.benchmark_cut_refinement.contract_status",
    "benchmark_side_evidence.benchmark_cut_refinement.scope_label",
    "benchmark_side_evidence.benchmark_cut_refinement.source_unit_family_label",
    "benchmark_side_evidence.benchmark_cut_refinement.refined_unit_family_label",
    "benchmark_side_evidence.benchmark_cut_refinement.localized_cut_builder_label",
    "benchmark_side_evidence.benchmark_cut_refinement.build_label",
    "benchmark_side_evidence.benchmark_cut_refinement.scheduling_unit_count",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.max_front_count",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.min_aspect_ratio",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.min_dominant_span",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.include_touching_neighbors",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.max_local_predecessor_count",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.predecessor_cut_link_policy",
    "benchmark_side_evidence.benchmark_cut_refinement.build_config_summary.front_progression_label",
    "benchmark_side_evidence.benchmark_cut_refinement.phase_refinement_diagnostics",
    "benchmark_side_evidence.benchmark_cut_refinement.disclosure_summary",
    "benchmark_side_evidence.cut_readiness.readiness_label",
    "benchmark_side_evidence.cut_readiness.readiness_summary",
    "benchmark_side_evidence.cut_readiness.phase_count",
    "benchmark_side_evidence.cut_readiness.shell_bench_phase_count",
    "benchmark_side_evidence.cut_readiness.predecessor_traced_phase_count",
    "benchmark_side_evidence.cut_readiness.multi_block_phase_count",
    "benchmark_side_evidence.cut_readiness.refinement_candidate_phase_count",
    "benchmark_side_evidence.future_scaffold.scaffold_label",
    "benchmark_side_evidence.future_scaffold.scaffold_role",
    "benchmark_side_evidence.future_scaffold.source_unit_family_label",
    "benchmark_side_evidence.future_scaffold.scaffold_summary",
    "benchmark_side_evidence.future_scaffold.readiness_dependency_label",
    "benchmark_side_evidence.future_scaffold.target_contracts",
    "benchmark_side_evidence.future_scaffold.outstanding_gap_labels",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_contract_status",
    "benchmark_side_evidence.future_scaffold.promotion_path_summary",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_ready",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_blocking_prerequisite_ids",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_ready",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.rule_id",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.rule_label",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.target_contract",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.evaluation_mode",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.status",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.required_prerequisite_ids",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.blocking_prerequisite_ids",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.summary",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule.evidence_fields",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.rule_id",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.rule_label",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.target_contract",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.evaluation_mode",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.status",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.required_prerequisite_ids",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.blocking_prerequisite_ids",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.summary",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule.evidence_fields",
    "benchmark_side_evidence.future_scaffold.refinement_candidate_phase_count",
    "benchmark_side_evidence.future_scaffold.variant_scope_label",
    "benchmark_side_evidence.future_scaffold.variant_scope_summary",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_exit_criteria",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_exit_criteria",
    "benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites",
    "benchmark_side_evidence.future_scaffold.lp_bz_sidecar_prerequisites",
    "benchmark_side_evidence.sidecar_evidence_label",
    "benchmark_side_evidence.sidecar_evidence_summary",
    "scheduling_unit_count",
];

fn lp_bz_cut_scheduling_limitation_note() -> String {
    let promoted_family_status = format_promoted_lp_bz_family_status_summary(
        LP_BZ_UNIT_GRANULARITY_LABEL,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL,
    );
    format!(
        "Marvin LP/BZ sidecar keeps {promoted_family_status}; its audited `cut_access_law` and provenance/input-aggregation clause remain benchmark-side sidecar evidence (`selected_block_provenance`, `preferred_phase_plan_proxy`, `localized_cut_builder_provenance`) rather than shared/core scheduling logic, so the route still remains exploratory-local evidence rather than a closure-grade mining-cut workflow. Benchmark-side maturity is effectively exhausted at this contract level: moving beyond `exploratory-local` now requires a shared/core-side or protocol-level mining-cut input contract, not another sidecar-only heuristic."
    )
}

fn mclaughlin_limit_benchmark_cut_limitation_note() -> String {
    format!(
        "`mclaughlin-limit` benchmark-cut refinement keeps builder `{}` / build `{}` strictly benchmark-side and limit-only: it promotes the cut layer beyond readiness-only, but it still relies on proxy shell×bench inputs and does not by itself establish literature-grade mining-cut comparability or promote `mclaughlin-full` out of stress-only scope.",
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILDER_LABEL, MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_LABEL,
    )
}

fn marvin_sidecar_traceability_field_paths(prefix: &str) -> Vec<String> {
    MARVIN_SIDECAR_TRACEABILITY_FIELD_SUFFIXES
        .iter()
        .map(|suffix| format!("{prefix}.{suffix}"))
        .collect()
}

fn marvin_sidecar_runtime_contract_field_paths(prefix: &str) -> Vec<String> {
    MARVIN_SIDECAR_RUNTIME_CONTRACT_FIELD_SUFFIXES
        .iter()
        .map(|suffix| format!("{prefix}.{suffix}"))
        .collect()
}

fn marvin_sidecar_promotion_gate_field_paths(prefix: &str) -> Vec<String> {
    MARVIN_SIDECAR_PROMOTION_GATE_FIELD_SUFFIXES
        .iter()
        .map(|suffix| format!("{prefix}.{suffix}"))
        .collect()
}

fn primary_unit_family_traceability_field_paths(prefix: &str) -> Vec<String> {
    PRIMARY_UNIT_FAMILY_TRACEABILITY_FIELD_SUFFIXES
        .iter()
        .map(|suffix| format!("{prefix}.{suffix}"))
        .collect()
}

fn lp_bz_lp_solve_status_label(status: lp_bz_lp_kernel::LpBzLpSolveStatus) -> &'static str {
    match status {
        lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal => "optimal",
        lp_bz_lp_kernel::LpBzLpSolveStatus::Infeasible => "infeasible",
        lp_bz_lp_kernel::LpBzLpSolveStatus::Unbounded => "unbounded",
        lp_bz_lp_kernel::LpBzLpSolveStatus::Skipped => "skipped",
    }
}

fn build_lp_bz_competitive_empirical_driver_assessment(
    adapter_summary: &MarvinLpBzAdapterSummary,
    competitive_probe_proxy_gap_closure_share: f64,
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
) -> LpBzCompetitiveEmpiricalDriverAssessment {
    let precedence_diagnostics = &adapter_summary.lp_bz_lp_solve.precedence_diagnostics;
    let coverage_basis_points = precedence_diagnostics.coverage_basis_points.unwrap_or(0);
    let coverage_shortfall_basis_points = 10_000u16.saturating_sub(coverage_basis_points);
    let precedence_blocking = precedence_diagnostics.coverage_completeness
        != lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Complete
        || coverage_shortfall_basis_points > 0;
    let precedence_summary = if precedence_blocking {
        format!(
            "Precedence coverage remains a measured blocker at {} ({} bps shortfall; enforced {}/{} rows, skipped {}).",
            lp_bz_precedence_coverage_label(precedence_diagnostics.coverage_basis_points),
            coverage_shortfall_basis_points,
            precedence_diagnostics.enforced_precedence_rows,
            precedence_diagnostics.total_precedence_rows,
            precedence_diagnostics.skipped_precedence_rows,
        )
    } else {
        format!(
            "Precedence coverage is complete at {} (0 bps shortfall; enforced {}/{} rows, skipped {}), so precedence coverage is not the observed blocker.",
            lp_bz_precedence_coverage_label(precedence_diagnostics.coverage_basis_points),
            precedence_diagnostics.enforced_precedence_rows,
            precedence_diagnostics.total_precedence_rows,
            precedence_diagnostics.skipped_precedence_rows,
        )
    };

    let focused_runtime_budget = &adapter_summary
        .lp_bz_round_repair
        .local_optimizer_runtime_budget_contract;
    let competitive_probe = &adapter_summary.lp_bz_round_repair.competitive_probe;
    let competitive_runtime_budget =
        &competitive_probe.competitive_local_optimizer_runtime_budget_contract;
    let competitive_budget_hit = competitive_runtime_budget.budget_hit;
    let budget_depletion_blocking = focused_runtime_budget.budget_hit || competitive_budget_hit;
    let focused_budget_usage = format!(
        "{}/{} ({:.2}%)",
        focused_runtime_budget.executed_iteration_count,
        focused_runtime_budget.max_iteration_count,
        lp_bz_iteration_budget_usage_percent(
            focused_runtime_budget.executed_iteration_count,
            focused_runtime_budget.max_iteration_count,
        ),
    );
    let competitive_budget_usage = format!(
        "{}/{} ({:.2}%)",
        competitive_runtime_budget.executed_iteration_count,
        competitive_runtime_budget.max_iteration_count,
        lp_bz_iteration_budget_usage_percent(
            competitive_runtime_budget.executed_iteration_count,
            competitive_runtime_budget.max_iteration_count,
        ),
    );
    let budget_summary = if budget_depletion_blocking {
        format!(
            "Budget depletion is observed: focused local search is `{}` after {}, and competitive probe is `{}` after {}.",
            focused_runtime_budget.execution_state,
            focused_budget_usage,
            competitive_runtime_budget.execution_state,
            competitive_budget_usage,
        )
    } else {
        format!(
            "Budget depletion is not observed: focused local search is `{}` after {}, and competitive probe is `{}` after {}.",
            focused_runtime_budget.execution_state,
            focused_budget_usage,
            competitive_runtime_budget.execution_state,
            competitive_budget_usage,
        )
    };

    let target_score_decomposition = &adapter_summary
        .lp_bz_round_repair
        .target_score_decomposition;
    let round_to_repair_delta = target_score_decomposition.repair_score_delta_vs_round_proxy;
    let local_search_vs_repair_delta =
        target_score_decomposition.local_search_score_delta_vs_repair_proxy;
    let local_search_vs_round_delta =
        target_score_decomposition.local_search_score_delta_vs_round_proxy;
    let strategy_mismatch_blocking =
        residual_ready_frontier_gap_after_competitive_probe_proxy > 1.0e-9;
    let strategy_summary = if strategy_mismatch_blocking {
        format!(
            "Round/repair/local-search mismatch remains measured: round proxy {:.6} -> repair {:.6} ({:+.6}), local search {:.6} ({:+.6} vs repair, {:+.6} vs round), and the competitive probe still leaves residual ready_frontier gap {:.6} after {:.2}% proxy closure.",
            target_score_decomposition.rounded_discounted_target_score_proxy,
            target_score_decomposition.repaired_discounted_target_score_proxy,
            round_to_repair_delta,
            target_score_decomposition.local_search_discounted_target_score_proxy,
            local_search_vs_repair_delta,
            local_search_vs_round_delta,
            residual_ready_frontier_gap_after_competitive_probe_proxy.max(0.0),
            competitive_probe_proxy_gap_closure_share * 100.0,
        )
    } else {
        format!(
            "Round/repair/local-search mismatch is not the observed blocker: round proxy {:.6} -> repair {:.6} ({:+.6}), local search {:.6} ({:+.6} vs repair, {:+.6} vs round), and the competitive probe fully covers the measured ready_frontier gap.",
            target_score_decomposition.rounded_discounted_target_score_proxy,
            target_score_decomposition.repaired_discounted_target_score_proxy,
            round_to_repair_delta,
            target_score_decomposition.local_search_discounted_target_score_proxy,
            local_search_vs_repair_delta,
            local_search_vs_round_delta,
        )
    };

    let empirical_driver_evidence = vec![
        LpBzEmpiricalDriverEvidence {
            driver_id: "precedence-coverage".to_owned(),
            status: if precedence_blocking {
                "blocking".to_owned()
            } else {
                "cleared".to_owned()
            },
            summary: precedence_summary.clone(),
            evidence_fields: vec![
                "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.enforced_precedence_rows"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.total_precedence_rows"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.skipped_precedence_rows"
                    .to_owned(),
            ],
        },
        LpBzEmpiricalDriverEvidence {
            driver_id: "budget-depletion".to_owned(),
            status: if budget_depletion_blocking {
                "blocking".to_owned()
            } else {
                "cleared".to_owned()
            },
            summary: budget_summary.clone(),
            evidence_fields: vec![
                "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.summary"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_budget_profile.mode_label"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.execution_state"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.budget_hit"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.summary"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_termination_reason"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_executed_iteration_count"
                    .to_owned(),
            ],
        },
        LpBzEmpiricalDriverEvidence {
            driver_id: "round-repair-local-search-mismatch".to_owned(),
            status: if strategy_mismatch_blocking {
                "blocking".to_owned()
            } else {
                "cleared".to_owned()
            },
            summary: strategy_summary.clone(),
            evidence_fields: vec![
                "lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.rounded_discounted_target_score_proxy"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.repaired_discounted_target_score_proxy"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_discounted_target_score_proxy"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_repair_proxy"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_round_proxy"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.competitive_probe_proxy_gap_closure_share"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_ready_frontier_gap_after_competitive_probe_proxy"
                    .to_owned(),
            ],
        },
    ];

    let empirical_dominant_blocker = if precedence_blocking {
        "precedence-coverage"
    } else if budget_depletion_blocking {
        "budget-depletion"
    } else if strategy_mismatch_blocking {
        "round-repair-local-search-mismatch"
    } else {
        "schedule-level-proof-only"
    }
    .to_owned();
    let empirical_dominant_blocker_summary = match empirical_dominant_blocker.as_str() {
        "precedence-coverage" => format!(
            "Empirical driver read selects precedence coverage as the dominant blocker. {precedence_summary} {budget_summary} {strategy_summary}"
        ),
        "budget-depletion" => format!(
            "Empirical driver read selects budget depletion as the dominant blocker after clearing precedence coverage. {budget_summary} {strategy_summary}"
        ),
        "round-repair-local-search-mismatch" => format!(
            "Empirical driver read selects round/repair/local-search mismatch as the dominant blocker because precedence coverage is cleared and budget depletion is not observed. {strategy_summary}"
        ),
        _ => format!(
            "Empirical driver read leaves only schedule-level proof: precedence coverage is cleared, budget depletion is not observed, and the benchmark-side round/repair/local-search mismatch no longer explains the residual."
        ),
    };
    let empirical_driver_evidence_summary = format!(
        "Empirical driver evidence statuses: precedence-coverage=`{}`, budget-depletion=`{}`, round-repair-local-search-mismatch=`{}`; dominant blocker=`{}`.",
        empirical_driver_evidence[0].status,
        empirical_driver_evidence[1].status,
        empirical_driver_evidence[2].status,
        empirical_dominant_blocker,
    );

    LpBzCompetitiveEmpiricalDriverAssessment {
        empirical_dominant_blocker,
        empirical_dominant_blocker_summary,
        empirical_driver_evidence_summary,
        empirical_driver_evidence,
    }
}

fn lp_bz_iteration_budget_usage_percent(executed_iteration_count: usize, budget: usize) -> f64 {
    if budget == 0 {
        0.0
    } else {
        executed_iteration_count as f64 * 100.0 / budget as f64
    }
}

fn build_lp_bz_budget_coverage_experiment_summary(
    ready_frontier_discounted_objective: f64,
    focused_candidate_discounted_objective: f64,
    adapter_summary: &MarvinLpBzAdapterSummary,
    empirical_dominant_blocker: &str,
    parity_claim_status: &str,
) -> LpBzBudgetCoverageExperimentSummary {
    let precedence_diagnostics = &adapter_summary.lp_bz_lp_solve.precedence_diagnostics;
    let focused_runtime_budget = &adapter_summary
        .lp_bz_round_repair
        .local_optimizer_runtime_budget_contract;
    let competitive_probe = &adapter_summary.lp_bz_round_repair.competitive_probe;
    let competitive_runtime_budget =
        &competitive_probe.competitive_local_optimizer_runtime_budget_contract;
    let competitive_probe_discounted_objective = focused_candidate_discounted_objective
        + competitive_probe.local_search_score_delta_vs_focused_proxy;
    let residual_ready_frontier_gap_after_competitive_probe_proxy =
        ready_frontier_discounted_objective - competitive_probe_discounted_objective;
    let focused_budget_usage = format!(
        "{}/{} ({:.2}%)",
        focused_runtime_budget.executed_iteration_count,
        focused_runtime_budget.max_iteration_count,
        lp_bz_iteration_budget_usage_percent(
            focused_runtime_budget.executed_iteration_count,
            focused_runtime_budget.max_iteration_count,
        ),
    );
    let competitive_budget_usage = format!(
        "{}/{} ({:.2}%)",
        competitive_runtime_budget.executed_iteration_count,
        competitive_runtime_budget.max_iteration_count,
        lp_bz_iteration_budget_usage_percent(
            competitive_runtime_budget.executed_iteration_count,
            competitive_runtime_budget.max_iteration_count,
        ),
    );
    let focused_ready_frontier_gap =
        ready_frontier_discounted_objective - focused_candidate_discounted_objective;
    let comparison = LpBzBudgetCoverageExperimentComparison {
        focused_budget_usage: focused_budget_usage.clone(),
        competitive_budget_usage: competitive_budget_usage.clone(),
        focused_candidate_discounted_objective,
        competitive_probe_candidate_discounted_objective: competitive_probe_discounted_objective,
        proxy_objective_delta_vs_focused: competitive_probe
            .local_search_score_delta_vs_focused_proxy,
        focused_ready_frontier_gap,
        competitive_probe_ready_frontier_gap:
            residual_ready_frontier_gap_after_competitive_probe_proxy,
    };
    let evidence_fields = vec![
        "lp_bz_baseline.candidate_pcpsp_summary.discounted_objective".to_owned(),
        "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.execution_state"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.budget_hit"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.summary"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_executed_iteration_count"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_budget_profile.effective_iteration_budget"
            .to_owned(),
        "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.local_search_score_delta_vs_focused_proxy"
            .to_owned(),
        "lp_bz_baseline.competitive_ready_frontier_probe.empirical_dominant_blocker".to_owned(),
        "lp_bz_baseline.competitive_ready_frontier_probe.residual_ready_frontier_gap_after_competitive_probe_proxy"
            .to_owned(),
        "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status".to_owned(),
    ];
    let (experiment_status, recommended_next_action, summary) = match empirical_dominant_blocker {
        "precedence-coverage" => (
            "precedence-coverage-expansion-first".to_owned(),
            "expand-precedence-coverage-before-budget-rerun".to_owned(),
            format!(
                "Precedence coverage remains incomplete at {} (enforced {}/{} rows, skipped {}), so the smallest honest follow-up is a precedence-coverage expansion before treating budget results as decisive. The current budget expansion comparison still moves the LP/BZ proxy candidate {:.6} -> {:.6} ({:+.6}), but parity_claim_status stays `{parity_claim_status}` until coverage clears and schedule-level proof exists.",
                lp_bz_precedence_coverage_label(precedence_diagnostics.coverage_basis_points),
                precedence_diagnostics.enforced_precedence_rows,
                precedence_diagnostics.total_precedence_rows,
                precedence_diagnostics.skipped_precedence_rows,
                focused_candidate_discounted_objective,
                competitive_probe_discounted_objective,
                competitive_probe.local_search_score_delta_vs_focused_proxy,
            ),
        ),
        "budget-depletion" => (
            "budget-expansion-changes-proxy-candidate".to_owned(),
            "prioritize-budget-expansion-follow-up".to_owned(),
            format!(
                "Budget expansion changes the LP/BZ proxy candidate: focused local search used {} with execution_state=`{}`, while the competitive probe used {} with execution_state=`{}` and moves the proxy objective {:.6} -> {:.6} ({:+.6}), shrinking the ready_frontier gap {:.6} -> {:.6}. This reduces uncertainty toward a budget blocker, but parity_claim_status remains `{parity_claim_status}`.",
                focused_budget_usage,
                focused_runtime_budget.execution_state,
                competitive_budget_usage,
                competitive_runtime_budget.execution_state,
                focused_candidate_discounted_objective,
                competitive_probe_discounted_objective,
                competitive_probe.local_search_score_delta_vs_focused_proxy,
                focused_ready_frontier_gap,
                residual_ready_frontier_gap_after_competitive_probe_proxy,
            ),
        ),
        "schedule-level-proof-only" => (
            "neither-budget-nor-coverage-dominates".to_owned(),
            "request-schedule-level-ready-frontier-proof".to_owned(),
            format!(
                "Precedence coverage is {} and budget depletion is not observed (focused {} / `{}`, competitive {} / `{}`). The budget-expanded probe only moves the LP/BZ proxy candidate {:.6} -> {:.6} ({:+.6}) and leaves residual ready_frontier gap {:.6}, so neither extra precedence coverage nor the current budget expansion is the dominant uncertainty; the surfaced blocker remains `{empirical_dominant_blocker}` while parity_claim_status stays `{parity_claim_status}`.",
                lp_bz_precedence_coverage_label(precedence_diagnostics.coverage_basis_points),
                focused_budget_usage,
                focused_runtime_budget.execution_state,
                competitive_budget_usage,
                competitive_runtime_budget.execution_state,
                focused_candidate_discounted_objective,
                competitive_probe_discounted_objective,
                competitive_probe.local_search_score_delta_vs_focused_proxy,
                residual_ready_frontier_gap_after_competitive_probe_proxy.max(0.0),
            ),
        ),
        _ => (
            "neither-budget-nor-coverage-dominates".to_owned(),
            "prioritize-candidate-improvement-evidence".to_owned(),
            format!(
                "Precedence coverage is {} and budget depletion is not observed (focused {} / `{}`, competitive {} / `{}`). The budget-expanded probe only moves the LP/BZ proxy candidate {:.6} -> {:.6} ({:+.6}) and leaves residual ready_frontier gap {:.6}, so neither extra precedence coverage nor the current budget expansion is the dominant uncertainty; the surfaced blocker remains `{empirical_dominant_blocker}` while parity_claim_status stays `{parity_claim_status}`.",
                lp_bz_precedence_coverage_label(precedence_diagnostics.coverage_basis_points),
                focused_budget_usage,
                focused_runtime_budget.execution_state,
                competitive_budget_usage,
                competitive_runtime_budget.execution_state,
                focused_candidate_discounted_objective,
                competitive_probe_discounted_objective,
                competitive_probe.local_search_score_delta_vs_focused_proxy,
                residual_ready_frontier_gap_after_competitive_probe_proxy.max(0.0),
            ),
        ),
    };

    LpBzBudgetCoverageExperimentSummary {
        experiment_status,
        recommended_next_action,
        comparison,
        summary,
        evidence_fields,
    }
}

fn lp_bz_precedence_strategy_label(
    strategy: lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy,
) -> &'static str {
    match strategy {
        lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy::None => "none",
        lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy::FullPerPeriod => "full_per_period",
        lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy::HybridCheckpoint => "hybrid_checkpoint",
    }
}

fn lp_bz_precedence_coverage_completeness_label(
    completeness: lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness,
) -> &'static str {
    match completeness {
        lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::NotApplicable => "not_applicable",
        lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Partial => "partial",
        lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Complete => "complete",
    }
}

fn lp_bz_precedence_coverage_label(coverage_basis_points: Option<u16>) -> String {
    match coverage_basis_points {
        Some(coverage_basis_points) => format!("{:.2}%", f64::from(coverage_basis_points) / 100.0),
        None => "n/a".to_owned(),
    }
}

fn lp_bz_precedence_runtime_summary(
    diagnostics: &lp_bz_lp_kernel::LpBzPrecedenceSolveDiagnostics,
) -> String {
    format!(
        "{} precedence coverage `{}` ({}; enforced {}/{} rows, skipped {})",
        lp_bz_precedence_strategy_label(diagnostics.strategy),
        lp_bz_precedence_coverage_completeness_label(diagnostics.coverage_completeness),
        lp_bz_precedence_coverage_label(diagnostics.coverage_basis_points),
        diagnostics.enforced_precedence_rows,
        diagnostics.total_precedence_rows,
        diagnostics.skipped_precedence_rows,
    )
}

#[derive(Debug, Clone, Copy)]
struct DatasetConfig {
    dataset_id: &'static str,
    instance_id: &'static str,
    instance_variant: &'static str,
    benchmark_family: &'static str,
    literature_reference_instance: &'static str,
    same_literature_variant: bool,
    blocks_file: &'static str,
    selected_block_source: &'static str,
    cpit_problem_file: &'static str,
    selected_block_solution_file: &'static str,
    pcpsp_problem_file: &'static str,
    pcpsp_solution_file: &'static str,
    precedence_file: &'static str,
    upit_objective_file: &'static str,
    lp_cpit_solution_file: Option<&'static str>,
    lp_pcpsp_solution_file: Option<&'static str>,
    tonnage_column: &'static str,
    nested_shell_probe_enabled: bool,
    resource_roles: &'static [(usize, MinelibResourceRole)],
}

const DATASETS: [DatasetConfig; 3] = [
    DatasetConfig {
        dataset_id: "marvin",
        instance_id: "marvin-local",
        instance_variant: "canonical",
        benchmark_family: "marvin",
        literature_reference_instance: "marvin",
        same_literature_variant: true,
        blocks_file: "marvin.blocks",
        selected_block_source: MARVIN_SELECTED_BLOCK_SOURCE,
        cpit_problem_file: "marvin.cpit",
        selected_block_solution_file: "marvin_cpit_gmunoz120723.sol",
        pcpsp_problem_file: "marvin.pcpsp",
        pcpsp_solution_file: "marvin_pcpsp_gmunoz120723.sol",
        precedence_file: "marvin.prec",
        upit_objective_file: "marvin.upit",
        lp_cpit_solution_file: Some("marvin.LPcpit"),
        lp_pcpsp_solution_file: Some("marvin.LPpcpsp"),
        tonnage_column: "field_4",
        nested_shell_probe_enabled: true,
        resource_roles: &[
            (0, MinelibResourceRole::MineTonnage),
            (1, MinelibResourceRole::PlantTonnage),
        ],
    },
    DatasetConfig {
        dataset_id: "mclaughlin-limit",
        instance_id: "mclaughlin-limit-local",
        instance_variant: "limit",
        benchmark_family: "mclaughlin-limit",
        literature_reference_instance: "mclaughlin-limit",
        same_literature_variant: true,
        blocks_file: "mclaughlin_limit.blocks",
        selected_block_source: REFERENCE_SELECTED_BLOCK_SOURCE,
        cpit_problem_file: "mclaughlin_limit.cpit",
        selected_block_solution_file: "mclaughlin_limit_cpit_gmunoz120723.sol",
        pcpsp_problem_file: "mclaughlin_limit.pcpsp",
        pcpsp_solution_file: "mclaughlin_limit_pcpsp_gmunoz120723.sol",
        precedence_file: "mclaughlin_limit.prec",
        upit_objective_file: "mclaughlin_limit.upit",
        lp_cpit_solution_file: Some("mclaughlin_limit.LPcpit"),
        lp_pcpsp_solution_file: None,
        tonnage_column: "field_5",
        nested_shell_probe_enabled: true,
        resource_roles: &[(0, MinelibResourceRole::PlantTonnage)],
    },
    DatasetConfig {
        dataset_id: "mclaughlin",
        instance_id: "mclaughlin-full-local",
        instance_variant: "full",
        benchmark_family: "mclaughlin",
        literature_reference_instance: "mclaughlin-limit",
        same_literature_variant: false,
        blocks_file: "mclaughlin.blocks",
        selected_block_source: REFERENCE_SELECTED_BLOCK_SOURCE,
        cpit_problem_file: "mclaughlin.cpit",
        selected_block_solution_file: "mclaughlin_cpit_gmunoz120723.sol",
        pcpsp_problem_file: "mclaughlin.pcpsp",
        pcpsp_solution_file: "mclaughlin_pcpsp_gmunoz120723.sol",
        precedence_file: "mclaughlin.prec",
        upit_objective_file: "mclaughlin.upit",
        lp_cpit_solution_file: Some("mclaughlin.LPcpit"),
        lp_pcpsp_solution_file: None,
        tonnage_column: "field_5",
        nested_shell_probe_enabled: false,
        resource_roles: &[(0, MinelibResourceRole::PlantTonnage)],
    },
];

#[derive(Debug, Serialize)]
struct MultiMineSchedulingReport {
    reference: String,
    output_path: String,
    common_pipeline: Vec<String>,
    benchmark_contract_audit: BenchmarkContractAudit,
    diagnostics_schema: BenchmarkDiagnosticsSchema,
    datasets: Vec<DatasetSchedulingReport>,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DatasetSchedulingReport {
    dataset_id: String,
    instance_id: String,
    instance_variant: String,
    literature_reference_instance: String,
    same_literature_variant: bool,
    comparison_classification: String,
    comparability_gap_contract: Vec<ComparabilityGapSummary>,
    comparability_gaps: Vec<String>,
    dataset_dir: String,
    blocks_path: String,
    selected_block_source: String,
    selected_block_provenance_summary: String,
    selected_block_provenance_chain: Vec<String>,
    selected_block_solution_path: String,
    pcpsp_problem_path: String,
    pcpsp_solution_path: String,
    tonnage_column: String,
    aggregation_strategy: String,
    preferred_nested_shell_family_contract: Option<MarvinPreferredNestedShellFamilyContract>,
    primary_unit_family_traceability: PrimaryUnitFamilyTraceability,
    #[serde(skip_serializing_if = "Option::is_none")]
    marvin_paperlike_pipeline_checklist: Option<MarvinPaperlikePipelineChecklist>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mclaughlin_limit_promotion_checklist: Option<MclaughlinLimitPromotionChecklist>,
    benchmark_contract_roles: Vec<String>,
    diagnostic_groups_present: Vec<String>,
    resource_roles: Vec<ResourceRoleReport>,
    same_core_api: bool,
    problem_summary: ProblemSummary,
    reference_summary: MinelibScheduleSolutionSummary,
    staged_relaxation_references: Vec<RelaxationReferenceSummary>,
    reference_period_routed_baseline: TemporalBaselineSummary,
    nested_shell_bench_probe: Option<NestedShellProbeSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lp_bz_baseline: Option<LpBzBaselineSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mclaughlin_limit_lp_bz_sidecar: Option<MclaughlinLimitLpBzSidecarSummary>,
    candidate_summary: CandidateSchedulingSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_period_alignment: PeriodAlignmentSummary,
    candidate_vs_reference_destination_membership: CompactPeriodMembershipComparison,
    temporal_routing_promotion_gate: TemporalRoutingPromotionGateSummary,
}

#[derive(Debug, Serialize)]
struct BenchmarkContractAudit {
    audit_scope: String,
    modules: Vec<BenchmarkContractModuleSummary>,
    promotion_rules: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BenchmarkContractModuleSummary {
    module_path: String,
    contract_role: String,
    scope_label: String,
    maturity_label: String,
    report_surface: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BenchmarkDiagnosticsSchema {
    schema_version: String,
    classification_labels: Vec<String>,
    required_groups: Vec<BenchmarkDiagnosticsGroup>,
}

#[derive(Debug, Serialize)]
struct BenchmarkDiagnosticsGroup {
    group_name: String,
    fields: Vec<String>,
    sourced_from: Vec<String>,
    intent: String,
}

#[derive(Debug, Clone, Serialize)]
struct MarvinPaperlikePipelineChecklist {
    pipeline_label: String,
    checklist_version: String,
    items: Vec<MarvinPaperlikePipelineChecklistItem>,
}

#[derive(Debug, Clone, Serialize)]
struct MarvinPaperlikePipelineChecklistItem {
    contract_id: String,
    contract_label: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MclaughlinLimitPromotionChecklist {
    checklist_label: String,
    checklist_version: String,
    items: Vec<MclaughlinLimitPromotionChecklistItem>,
}

#[derive(Debug, Clone, Serialize)]
struct MclaughlinLimitPromotionChecklistItem {
    contract_id: String,
    contract_label: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SelectedBlockProvenanceTraceability {
    selected_block_source: String,
    selected_block_count: usize,
    selected_block_provenance_summary: String,
    selected_block_provenance_chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PreferredPhasePlanTraceability {
    aggregation_strategy: String,
    preferred_phase_count: usize,
    unique_shell_count: Option<usize>,
    preferred_nested_shell_factor_count: Option<usize>,
    preferred_nested_shell_realized_shell_count: Option<usize>,
    preferred_nested_shell_access_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkCutReadinessTraceability {
    readiness_label: String,
    readiness_summary: String,
    phase_count: usize,
    shell_bench_phase_count: usize,
    predecessor_traced_phase_count: usize,
    multi_block_phase_count: usize,
    refinement_candidate_phase_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkSideScaffoldTraceability {
    scaffold_label: String,
    scaffold_role: String,
    source_unit_family_label: String,
    scaffold_summary: String,
    readiness_dependency_label: String,
    target_contracts: Vec<String>,
    outstanding_gap_labels: Vec<String>,
    benchmark_cut_contract_status: String,
    lp_bz_sidecar_contract_status: String,
    promotion_path_summary: String,
    benchmark_cut_promotion_ready: bool,
    benchmark_cut_blocking_prerequisite_ids: Vec<String>,
    lp_bz_sidecar_promotion_ready: bool,
    lp_bz_sidecar_blocking_prerequisite_ids: Vec<String>,
    benchmark_cut_promotion_rule: BenchmarkSidePromotionRule,
    lp_bz_sidecar_promotion_rule: BenchmarkSidePromotionRule,
    refinement_candidate_phase_count: usize,
    variant_scope_label: String,
    variant_scope_summary: String,
    benchmark_cut_exit_criteria: Vec<BenchmarkSidePromotionExitCriterion>,
    lp_bz_sidecar_exit_criteria: Vec<BenchmarkSidePromotionExitCriterion>,
    benchmark_cut_prerequisites: Vec<BenchmarkSidePromotionPrerequisite>,
    lp_bz_sidecar_prerequisites: Vec<BenchmarkSidePromotionPrerequisite>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkSidePromotionPrerequisite {
    prerequisite_id: String,
    prerequisite_label: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkSidePromotionRule {
    rule_id: String,
    rule_label: String,
    target_contract: String,
    evaluation_mode: String,
    status: String,
    required_prerequisite_ids: Vec<String>,
    blocking_prerequisite_ids: Vec<String>,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkSidePromotionExitCriterion {
    criterion_id: String,
    criterion_label: String,
    target_contract: String,
    evaluation_mode: String,
    expected_state: String,
    current_state: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkSideEvidenceTraceability {
    benchmark_scope_label: String,
    mining_unit_evidence_summary: String,
    cut_evidence_label: String,
    cut_evidence_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    benchmark_cut_refinement: Option<MclaughlinLimitBenchmarkCutRefinementSummary>,
    cut_readiness: Option<BenchmarkCutReadinessTraceability>,
    future_scaffold: Option<BenchmarkSideScaffoldTraceability>,
    sidecar_evidence_label: String,
    sidecar_evidence_summary: String,
}

#[derive(Debug, Clone, Serialize)]
struct MclaughlinLimitBenchmarkCutBuildConfigSummary {
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    include_touching_neighbors: bool,
    max_local_predecessor_count: Option<usize>,
    predecessor_cut_link_policy: String,
    front_progression_label: String,
}

#[derive(Debug, Clone, Serialize)]
struct MclaughlinLimitBenchmarkCutRefinementSummary {
    contract_label: String,
    contract_version: String,
    contract_status: String,
    scope_label: String,
    source_unit_family_label: String,
    refined_unit_family_label: String,
    localized_cut_builder_label: String,
    build_label: String,
    scheduling_unit_count: usize,
    build_config_summary: MclaughlinLimitBenchmarkCutBuildConfigSummary,
    phase_refinement_diagnostics: PushbackBenchLocalizedCutRefinementDiagnostics,
    disclosure_summary: String,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PrimaryUnitFamilyTraceability {
    unit_family_label: String,
    unit_family_role: String,
    literature_alignment_label: String,
    selected_block_provenance: SelectedBlockProvenanceTraceability,
    preferred_phase_plan_proxy: PreferredPhasePlanTraceability,
    benchmark_side_evidence: BenchmarkSideEvidenceTraceability,
    scheduling_unit_count: usize,
}

#[derive(Debug, Serialize)]
struct ResourceRoleReport {
    resource_index: usize,
    role: String,
}

#[derive(Debug, Serialize)]
struct ProblemSummary {
    period_count: usize,
    destination_count: usize,
    resource_constraint_count: usize,
    discount_rate: f64,
}

#[derive(Debug, Serialize)]
struct CandidateSchedulingSummary {
    selected_block_count: usize,
    phase_count: usize,
    scheduling_unit_count: usize,
    temporal_candidate_objective: f64,
    temporal_candidate_discounted_objective: f64,
    routed_schedule_entry_count: usize,
    final_schedule_entry_count: usize,
    final_schedule_violation_count: usize,
    candidate_pcpsp_summary: MinelibScheduleSolutionSummary,
}

#[derive(Debug, Serialize)]
struct TemporalBaselineSummary {
    baseline_name: String,
    candidate_pcpsp_summary: MinelibScheduleSolutionSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_period_alignment: PeriodAlignmentSummary,
}

#[derive(Debug, Serialize)]
struct NestedShellProbeSummary {
    aggregation_strategy: String,
    revenue_factor_count: usize,
    unique_shell_count: usize,
    limitations: Vec<String>,
    candidate_summary: CandidateSchedulingSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_period_alignment: PeriodAlignmentSummary,
}

#[derive(Debug, Serialize)]
struct LpBzBaselineSummary {
    phase_plan_builder_label: String,
    unit_granularity_label: String,
    promoted_build_label: String,
    paperlike_candidate_role: String,
    local_optimizer_scaffold_unit_family_label: String,
    local_optimizer_scaffold_role: String,
    unit_family_traceability: PushbackBenchLocalizedCutUnitFamilyTraceability,
    cut_access_law: PushbackBenchLocalizedCutAccessPolicySummary,
    phase_refinement_diagnostics: PushbackBenchLocalizedCutRefinementDiagnostics,
    summary: MarvinLpBzAdapterSummary,
    competitive_ready_frontier_probe: LpBzCompetitiveReadyFrontierProbeSummary,
    lp_bz_promotion_readiness: LpBzPromotionReadinessSummary,
    candidate_pcpsp_summary: MinelibScheduleSolutionSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_period_alignment: PeriodAlignmentSummary,
    candidate_vs_reference_destination_membership: CompactPeriodMembershipComparison,
    temporal_routing_promotion_gate: TemporalRoutingPromotionGateSummary,
}

#[derive(Debug, Serialize)]
struct MclaughlinLimitLpBzSidecarSummary {
    sidecar_label: String,
    sidecar_version: String,
    sidecar_status: String,
    scope_label: String,
    objective_alignment_label: String,
    unit_family_label: String,
    kernel_label: String,
    solver_label: String,
    solve_status: lp_bz_lp_kernel::LpBzLpSolveStatus,
    scheduling_unit_count: usize,
    variable_count: usize,
    active_variable_count: usize,
    discounted_objective_bound: Option<f64>,
    candidate_discounted_objective: f64,
    reference_discounted_objective: f64,
    bound_to_candidate_absolute_gap: Option<f64>,
    bound_to_reference_absolute_gap: Option<f64>,
    precedence_diagnostics: lp_bz_lp_kernel::LpBzPrecedenceSolveDiagnostics,
    cut_diagnostics: lp_bz_lp_kernel::LpBzCutSolveDiagnostics,
    completeness_summary: String,
    disclosure_summary: String,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LpBzCompetitiveReadyFrontierProbeSummary {
    driver_targeting_status: String,
    closure_status: String,
    ready_frontier_discounted_objective: f64,
    focused_candidate_discounted_objective: f64,
    focused_candidate_vs_ready_frontier_objective_gap: f64,
    competitive_probe_proxy_gap_closure: f64,
    competitive_probe_proxy_gap_closure_share: f64,
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    empirical_dominant_blocker: String,
    empirical_dominant_blocker_summary: String,
    empirical_driver_evidence_summary: String,
    empirical_driver_evidence: Vec<LpBzEmpiricalDriverEvidence>,
    budget_coverage_experiment: LpBzBudgetCoverageExperimentSummary,
    residual_interpretation: String,
    residual_interpretation_summary: String,
    dominant_residual_driver: String,
    dominant_residual_driver_summary: String,
    next_step_evidence: String,
    next_step_evidence_summary: String,
    parity_claim_status: String,
    parity_claim_summary: String,
    remaining_blocker_count: usize,
    remaining_blockers_summary: String,
    remaining_blockers: Vec<LpBzReadyFrontierParityBlocker>,
    readiness_criteria_version: String,
    readiness_state: String,
    readiness_summary: String,
    readiness_blocked_criteria_count: usize,
    readiness_criteria: Vec<LpBzCompetitiveReadinessCriterion>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct LpBzReadyFrontierParityBlocker {
    blocker_id: String,
    blocker_label: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct LpBzCompetitiveReadinessCriterion {
    criterion_id: String,
    criterion_label: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct LpBzEmpiricalDriverEvidence {
    driver_id: String,
    status: String,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LpBzCompetitiveEmpiricalDriverAssessment {
    empirical_dominant_blocker: String,
    empirical_dominant_blocker_summary: String,
    empirical_driver_evidence_summary: String,
    empirical_driver_evidence: Vec<LpBzEmpiricalDriverEvidence>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct LpBzBudgetCoverageExperimentComparison {
    focused_budget_usage: String,
    competitive_budget_usage: String,
    focused_candidate_discounted_objective: f64,
    competitive_probe_candidate_discounted_objective: f64,
    proxy_objective_delta_vs_focused: f64,
    focused_ready_frontier_gap: f64,
    competitive_probe_ready_frontier_gap: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct LpBzBudgetCoverageExperimentSummary {
    experiment_status: String,
    recommended_next_action: String,
    comparison: LpBzBudgetCoverageExperimentComparison,
    summary: String,
    evidence_fields: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RelaxationReferenceSummary {
    label: String,
    problem_kind: String,
    problem_path: String,
    solution_path: String,
    directly_comparable_to_pcpsp: bool,
    summary: MinelibScheduleSolutionSummary,
}

#[derive(Debug, Serialize)]
struct CompactPeriodMembershipComparison {
    shared_assignments: usize,
    reference_only_assignment_count: usize,
    candidate_only_assignment_count: usize,
    jaccard_index: f64,
    reference_only_assignment_examples: Vec<(String, usize)>,
    candidate_only_assignment_examples: Vec<(String, usize)>,
}

#[derive(Debug, Serialize)]
struct PeriodAlignmentSummary {
    shared_block_count: usize,
    reference_only_block_count: usize,
    candidate_only_block_count: usize,
    exact_period_match_count: usize,
    earlier_than_reference_count: usize,
    later_than_reference_count: usize,
    mean_absolute_period_delta: f64,
    max_absolute_period_delta: f64,
    largest_absolute_period_delta_examples: Vec<(usize, usize, usize)>,
}

#[derive(Debug)]
struct MultiMineSchedulerCli {
    output_path: PathBuf,
}

fn parse_multi_mine_scheduler_cli(
    path_policy: &BenchmarkPathPolicy,
) -> Result<MultiMineSchedulerCli, MineError> {
    parse_multi_mine_scheduler_cli_args(path_policy, env::args_os().skip(1))
}

fn parse_multi_mine_scheduler_cli_args<I, S>(
    path_policy: &BenchmarkPathPolicy,
    args: I,
) -> Result<MultiMineSchedulerCli, MineError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut positional_args = Vec::new();

    for arg in args.into_iter().map(Into::into) {
        let arg_text = arg.to_string_lossy();
        if matches!(arg_text.as_ref(), "--quiet" | "-q") {
            continue;
        }
        if arg_text.starts_with('-') {
            return Err(MineError::validation(format!(
                "Unknown option `{arg_text}`. Supported arguments: optional positional `output_path`."
            )));
        }
        positional_args.push(path_policy.resolve_cli_path(Path::new(&arg)));
    }

    if positional_args.len() > 1 {
        return Err(MineError::validation(format!(
            "Expected at most 1 positional argument (`output_path`), received {}.",
            positional_args.len()
        )));
    }

    let output_path = positional_args.pop().unwrap_or_else(|| {
        path_policy
            .outputs_dir()
            .join("multi-mine-scheduling-report.json")
    });

    Ok(MultiMineSchedulerCli { output_path })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path_policy = BenchmarkPathPolicy::discover()?;
    let repo_root = path_policy.repo_root().to_path_buf();
    let cli = parse_multi_mine_scheduler_cli(&path_policy)?;
    let output_path = cli.output_path;
    let datasets = DATASETS
        .iter()
        .map(|config| build_dataset_report(&repo_root, config))
        .collect::<Result<Vec<_>, _>>()?;

    let report = MultiMineSchedulingReport {
        reference: "Espinoza et al. (2013) MineLib [R29] https://doi.org/10.1007/s10479-012-1258-3".to_owned(),
        output_path: output_path.display().to_string(),
        common_pipeline: vec![
            "read_benchmark_blocks(...)".to_owned(),
            "read_minelib_cpit_problem(...)".to_owned(),
            "read_minelib_cpit_solution(...)".to_owned(),
            "read_minelib_pcpsp_problem(...)".to_owned(),
            "build dataset-aware phase plan (reference-period × bench or safe nested-shell primary)".to_owned(),
            "build_scheduling_problem_from_minelib_problem(...)".to_owned(),
            "solve_decomposed_scheduling_problem(DecomposedSchedulingConfig::ready_frontier(), ...)".to_owned(),
        ],
        benchmark_contract_audit: build_benchmark_contract_audit(),
        diagnostics_schema: build_benchmark_diagnostics_schema(),
        datasets,
        limitations: vec![
            "Marvin now reports an explicit benchmark-side selected-block contract (`marvin-paperlike-v2-shells-pushbacks-mining-cuts`) over the chain `shells -> pushbacks -> mining-cuts -> scheduling`, while `mclaughlin-limit` now reports the explicit open-UPIT shell contract `mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases` and keeps `mclaughlin-full` on the staged CPIT fallback.".to_owned(),
            "The benchmark no longer depends uniformly on staged reference-period bands: Marvin uses bounded revenue/cost-aware shells, `mclaughlin-limit` uses open-UPIT shell × bench pushback-equivalent units, and only `mclaughlin-full` stays on the explicit `reference-period-bench` fallback until a comparable shell route exists.".to_owned(),
            format!(
                "For Marvin, the report now promotes a bounded {NESTED_SHELL_PROBE_FACTOR_COUNT}-factor nested-shell × bench primary route built from revenue/cost-aware factor scenarios; equivalent factor-aware probes for other datasets still depend on better economic semantics than the open `*.upit` net values alone."
            ),
            "Resource semantics that MineLib leaves to dataset metadata are injected through dataset config (for example, Marvin uses mine+plant capacities while McLaughlin only stages plant capacity).".to_owned(),
            "When staged LP references exist, the report versions them explicitly; only LPpcpsp references are directly comparable to the PCPSP objective, while LPcpit remains a relaxation on the pit-limit problem.".to_owned(),
            "The report includes both mclaughlin-limit and mclaughlin-full; only the limit variant can be aligned directly to the most common MineLib scheduling tables in the literature, while `mclaughlin-full` is carried explicitly as a stress-only local variant.".to_owned(),
        ],
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
    println!("{}", output_path.display());
    Ok(())
}

fn build_dataset_report(
    repo_root: &Path,
    config: &DatasetConfig,
) -> Result<DatasetSchedulingReport, Box<dyn std::error::Error>> {
    let path_policy = BenchmarkPathPolicy::from_repo_root(repo_root.to_path_buf());
    let dataset_dir = path_policy.dataset_dir(config.dataset_id);
    let references_dir = path_policy.references_dir(&dataset_dir);
    let blocks_path = dataset_dir.join(config.blocks_file);
    let cpit_problem_path = references_dir.join(config.cpit_problem_file);
    let selected_block_solution_path = references_dir.join(config.selected_block_solution_file);
    let pcpsp_problem_path = references_dir.join(config.pcpsp_problem_file);
    let pcpsp_solution_path = references_dir.join(config.pcpsp_solution_file);
    let precedence_path = references_dir.join(config.precedence_file);
    let upit_objective_path = references_dir.join(config.upit_objective_file);
    let model = read_benchmark_blocks(&blocks_path, config.benchmark_family)?;
    let cpit_problem = read_minelib_cpit_problem(&cpit_problem_path, &model)?;
    let selected_solution = read_minelib_cpit_solution(&selected_block_solution_path, &model)?;
    let selected_linear_indices = selected_solution
        .assignments
        .iter()
        .filter(|assignment| assignment.fraction > 1.0e-9)
        .map(|assignment| assignment.linear_index)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_block_count = selected_linear_indices.len();
    let pcpsp_problem = read_minelib_pcpsp_problem(&pcpsp_problem_path, &model)?;
    let pcpsp_solution = read_minelib_pcpsp_solution(&pcpsp_solution_path, &model)?;
    let reference_summary = summarize_minelib_schedule_solution(&pcpsp_problem, &pcpsp_solution)?;
    let tonnage_column = ColumnId::new(config.tonnage_column)?;
    let linear_index_to_row_index = build_linear_index_to_row_index(&model)?;
    let primary_precedence_graph = if config.nested_shell_probe_enabled {
        Some(read_minelib_precedence_graph(&precedence_path, &model)?)
    } else {
        None
    };
    let primary_upit_block_values = if config.nested_shell_probe_enabled {
        Some(
            read_minelib_upit_block_values(&upit_objective_path, &model)?
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
        )
    } else {
        None
    };
    let preferred_phase_plan = build_preferred_phase_plan_for_minelib_scheduling(
        config.dataset_id,
        config.nested_shell_probe_enabled,
        &model,
        &linear_index_to_row_index,
        &selected_solution.assignments,
        primary_precedence_graph.as_ref(),
        primary_upit_block_values.as_ref(),
        &tonnage_column,
        NESTED_SHELL_PROBE_FACTOR_COUNT,
    )?;
    let phase_plan = preferred_phase_plan.phase_plan;
    let phase_plan_metadata = preferred_phase_plan.metadata;
    let preferred_nested_shell_family_contract = phase_plan_metadata
        .marvin_nested_shell_family_contract
        .clone();
    let effective_selected_block_source = phase_plan_metadata.selected_block_source.clone();
    let effective_selected_block_summary = phase_plan_metadata
        .selected_block_provenance_summary
        .clone();
    let effective_selected_block_chain =
        phase_plan_metadata.selected_block_provenance_chain.clone();
    let effective_selected_block_count = if phase_plan_metadata.nested_shell_primary {
        phase_plan.total_block_count
    } else {
        selected_block_count
    };
    let resource_roles = config
        .resource_roles
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    let scheduling_problem = build_scheduling_problem_from_minelib_problem(
        &phase_plan,
        &pcpsp_problem,
        config.dataset_id,
        &resource_roles,
        &phase_plan_metadata.descriptive_note,
    )?;
    let artifacts = solve_decomposed_scheduling_problem(
        &scheduling_problem,
        &DecomposedSchedulingConfig::ready_frontier(),
        Metadata::new(),
    )?;
    let candidate_period_memberships = build_candidate_period_memberships(
        &linear_index_to_row_index,
        &model,
        &phase_plan,
        artifacts.final_schedule(),
        &tonnage_column,
    )?;
    let candidate_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &candidate_period_memberships)?;
    let candidate_pcpsp_summary =
        summarize_minelib_schedule_solution(&pcpsp_problem, &candidate_solution)?;
    let mclaughlin_limit_benchmark_cut_refinement =
        build_mclaughlin_limit_benchmark_cut_refinement(
            config,
            &phase_plan_metadata.aggregation_strategy,
            &model,
            &phase_plan,
            &pcpsp_problem,
            &resource_roles,
            &tonnage_column,
        )?;
    let mclaughlin_limit_lp_bz_sidecar = build_mclaughlin_limit_lp_bz_sidecar(
        config,
        &phase_plan_metadata.aggregation_strategy,
        &scheduling_problem,
        candidate_pcpsp_summary.discounted_objective,
        reference_summary.discounted_objective,
    )?;
    let reference_period_routed_baseline_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &build_period_memberships(&selected_solution),
    )?;
    let reference_period_routed_baseline_summary = summarize_minelib_schedule_solution(
        &pcpsp_problem,
        &reference_period_routed_baseline_solution,
    )?;
    let staged_relaxation_references = build_relaxation_reference_summaries(
        &model,
        &cpit_problem,
        &cpit_problem_path,
        &pcpsp_problem,
        &pcpsp_problem_path,
        &references_dir,
        config,
    )?;
    let lp_bz_baseline = build_lp_bz_baseline(
        repo_root,
        &references_dir,
        config,
        &model,
        &phase_plan,
        &pcpsp_problem,
        &pcpsp_solution,
        &resource_roles,
        &linear_index_to_row_index,
        &tonnage_column,
        candidate_pcpsp_summary.discounted_objective,
        effective_selected_block_count,
        &effective_selected_block_source,
        &phase_plan_metadata.aggregation_strategy,
        preferred_nested_shell_family_contract.as_ref(),
    )?;
    let candidate_summary = CandidateSchedulingSummary {
        selected_block_count,
        phase_count: phase_plan.phase_count,
        scheduling_unit_count: scheduling_problem.units().len(),
        temporal_candidate_objective: artifacts.temporal_candidate().total_objective_value(),
        temporal_candidate_discounted_objective: artifacts
            .temporal_candidate()
            .total_discounted_objective_value(),
        routed_schedule_entry_count: artifacts.routed_schedule().entries().len(),
        final_schedule_entry_count: artifacts.final_schedule().entries().len(),
        final_schedule_violation_count: artifacts.final_schedule().violations().len(),
        candidate_pcpsp_summary: candidate_pcpsp_summary.clone(),
    };
    let primary_unit_family_traceability = build_primary_unit_family_traceability(
        config,
        &effective_selected_block_source,
        effective_selected_block_count,
        &effective_selected_block_summary,
        &effective_selected_block_chain,
        &phase_plan_metadata.aggregation_strategy,
        &phase_plan,
        phase_plan.phase_count,
        phase_plan_metadata.unique_shell_count,
        preferred_nested_shell_family_contract.as_ref(),
        lp_bz_baseline.is_some(),
        mclaughlin_limit_benchmark_cut_refinement.as_ref(),
        mclaughlin_limit_lp_bz_sidecar.as_ref(),
        candidate_summary.scheduling_unit_count,
    );
    let nested_shell_bench_probe = if phase_plan_metadata.nested_shell_primary {
        None
    } else {
        build_nested_shell_probe(
            &model,
            &references_dir,
            &precedence_path,
            &upit_objective_path,
            &pcpsp_problem,
            &pcpsp_solution,
            &linear_index_to_row_index,
            &tonnage_column,
            &resource_roles,
            config,
        )?
    };
    let mut comparability_gap_contract = vec![
        if phase_plan_metadata.nested_shell_primary {
            ComparabilityGapSummary {
                gap_id: if config.dataset_id == "marvin" {
                    "selected-block-source-benchmark-shell-contract".to_owned()
                } else {
                    "selected-block-source-open-upit-shell-contract".to_owned()
                },
                gap_source: ComparabilityGapSource::InputProtocol,
                summary: if config.dataset_id == "marvin" {
                    format!(
                        "selected-block provenance is now explicit as `{effective_selected_block_source}` with chain `{}`, but the shell admission contract is still a bounded benchmark-side reconstruction rather than a paper-reproduced shell/pushback generator",
                        effective_selected_block_chain.join(" -> "),
                    )
                } else {
                    format!(
                        "selected-block provenance is now explicit as `{effective_selected_block_source}` with chain `{}`; the benchmark routes that shell family through pushback-equivalent shell × bench phases before scheduling, and `primary_unit_family_traceability` now fixes the remaining benchmark-side mining-cut/sidecar status as `cut_evidence_label = \"{}\"` plus `sidecar_evidence_label = \"{}\"`. The route still remains a bounded benchmark-side proxy rather than a paper-reproduced pushback/mining-unit generator",
                        effective_selected_block_chain.join(" -> "),
                        primary_unit_family_traceability
                            .benchmark_side_evidence
                            .cut_evidence_label,
                        primary_unit_family_traceability
                            .benchmark_side_evidence
                            .sidecar_evidence_label,
                    )
                },
                evidence_fields: if config.dataset_id == "marvin" {
                    vec![
                        "selected_block_source".to_owned(),
                        "selected_block_provenance_summary".to_owned(),
                        "selected_block_provenance_chain".to_owned(),
                        "preferred_nested_shell_family_contract".to_owned(),
                    ]
                } else {
                    primary_unit_family_traceability_field_paths("primary_unit_family_traceability")
                },
            }
        } else {
            ComparabilityGapSummary {
                gap_id: "selected-block-source-staged-cpit".to_owned(),
                gap_source: ComparabilityGapSource::InputProtocol,
                summary: "selected blocks are seeded from a staged CPIT reference instead of a paper-reproduced shell/pushback generation pipeline".to_owned(),
                evidence_fields: vec![
                    "selected_block_source".to_owned(),
                    "selected_block_solution_path".to_owned(),
                ],
            }
        },
        aggregation_comparability_gap_summary(
            config.dataset_id,
            &phase_plan_metadata.aggregation_strategy,
            preferred_nested_shell_family_contract.as_ref(),
            Some(&primary_unit_family_traceability),
            nested_shell_bench_probe.is_some(),
        ),
        temporal_solver_comparability_gap_summary(
            lp_bz_baseline.as_ref(),
            mclaughlin_limit_lp_bz_sidecar.as_ref(),
            Some(&primary_unit_family_traceability),
        ),
    ];
    if !config.same_literature_variant {
        push_unique_comparability_gap_summary(
            &mut comparability_gap_contract,
            ComparabilityGapSummary {
                gap_id: "literature-instance-variant-mismatch".to_owned(),
                gap_source: ComparabilityGapSource::InstanceVariant,
                summary: format!(
                    "the executed instance variant `{}` does not match the literature target `{}` and should be read as a local stress benchmark rather than a direct literature comparison",
                    config.instance_variant, config.literature_reference_instance
                ),
                evidence_fields: vec![
                    "instance_variant".to_owned(),
                    "literature_reference_instance".to_owned(),
                ],
            },
        );
    }
    for (index, limitation) in phase_plan_metadata.limitations.iter().enumerate() {
        push_unique_comparability_gap_summary(
            &mut comparability_gap_contract,
            ComparabilityGapSummary {
                gap_id: format!("aggregation-limitation-{}", index + 1),
                gap_source: ComparabilityGapSource::AggregationFormulation,
                summary: limitation.clone(),
                evidence_fields: vec![
                    "aggregation_strategy".to_owned(),
                    "preferred_nested_shell_family_contract".to_owned(),
                ],
            },
        );
    }
    if let Some(lp_bz_baseline) = lp_bz_baseline
        .as_ref()
        .filter(|_| config.dataset_id == "marvin")
    {
        push_unique_comparability_gap_summary(
            &mut comparability_gap_contract,
            marvin_input_aggregation_traceability_gap_summary(lp_bz_baseline),
        );
    }
    let comparability_gaps = derive_comparability_gaps(&comparability_gap_contract);
    validate_comparability_gap_contract_consistency(
        &comparability_gap_contract,
        &comparability_gaps,
        &format!("{} dataset report", config.dataset_id),
    )
    .map_err(mine_sdk::MineError::validation)?;
    let comparison_classification = if comparability_gaps.is_empty() {
        "paper-comparable"
    } else {
        "exploratory-local"
    };
    let candidate_vs_reference_period_alignment =
        compare_period_alignment(&pcpsp_solution, &candidate_solution);
    let candidate_vs_reference_destination_membership = compare_period_memberships(
        &build_period_destination_memberships(&pcpsp_solution),
        &build_period_destination_memberships(&candidate_solution),
    );
    let temporal_routing_promotion_gate = build_temporal_routing_promotion_gate_summary(
        candidate_pcpsp_summary.discounted_objective,
        reference_summary.discounted_objective,
        candidate_pcpsp_summary.used_period_count,
        reference_summary.used_period_count,
        candidate_vs_reference_period_alignment.mean_absolute_period_delta,
        candidate_vs_reference_period_alignment.earlier_than_reference_count,
        candidate_vs_reference_destination_membership.jaccard_index,
    );
    validate_temporal_routing_promotion_gate_summary(&temporal_routing_promotion_gate)
        .map_err(mine_sdk::MineError::validation)?;
    let marvin_paperlike_pipeline_checklist = if config.dataset_id == "marvin" {
        preferred_nested_shell_family_contract
            .as_ref()
            .zip(lp_bz_baseline.as_ref())
            .map(|(preferred_nested_shell_family_contract, lp_bz_baseline)| {
                build_marvin_paperlike_pipeline_checklist(
                    preferred_nested_shell_family_contract,
                    lp_bz_baseline,
                    comparison_classification,
                    &comparability_gaps,
                )
            })
    } else {
        None
    };
    let mclaughlin_limit_promotion_checklist = if config.dataset_id == "mclaughlin-limit" {
        Some(build_mclaughlin_limit_promotion_checklist(
            &primary_unit_family_traceability,
            comparison_classification,
            &comparability_gaps,
            &temporal_routing_promotion_gate,
        ))
    } else {
        None
    };
    let benchmark_contract_roles = build_dataset_contract_roles(
        config,
        &phase_plan_metadata.aggregation_strategy,
        nested_shell_bench_probe.is_some(),
        lp_bz_baseline.is_some(),
        mclaughlin_limit_benchmark_cut_refinement.is_some(),
        mclaughlin_limit_lp_bz_sidecar.is_some(),
    );
    let diagnostic_groups_present = build_dataset_diagnostic_groups(
        nested_shell_bench_probe.is_some(),
        lp_bz_baseline.is_some(),
        marvin_paperlike_pipeline_checklist.is_some(),
        mclaughlin_limit_promotion_checklist.is_some(),
    );

    Ok(DatasetSchedulingReport {
        dataset_id: config.dataset_id.to_owned(),
        instance_id: config.instance_id.to_owned(),
        instance_variant: config.instance_variant.to_owned(),
        literature_reference_instance: config.literature_reference_instance.to_owned(),
        same_literature_variant: config.same_literature_variant,
        comparison_classification: comparison_classification.to_owned(),
        comparability_gap_contract,
        comparability_gaps,
        dataset_dir: dataset_dir.display().to_string(),
        blocks_path: blocks_path.display().to_string(),
        selected_block_source: effective_selected_block_source,
        selected_block_provenance_summary: effective_selected_block_summary,
        selected_block_provenance_chain: effective_selected_block_chain,
        selected_block_solution_path: selected_block_solution_path.display().to_string(),
        pcpsp_problem_path: pcpsp_problem_path.display().to_string(),
        pcpsp_solution_path: pcpsp_solution_path.display().to_string(),
        tonnage_column: config.tonnage_column.to_owned(),
        aggregation_strategy: phase_plan_metadata.aggregation_strategy,
        preferred_nested_shell_family_contract,
        primary_unit_family_traceability,
        marvin_paperlike_pipeline_checklist,
        mclaughlin_limit_promotion_checklist,
        benchmark_contract_roles,
        diagnostic_groups_present,
        resource_roles: config
            .resource_roles
            .iter()
            .map(|(resource_index, role)| ResourceRoleReport {
                resource_index: *resource_index,
                role: format!("{role:?}"),
            })
            .collect(),
        same_core_api: true,
        problem_summary: ProblemSummary {
            period_count: pcpsp_problem.period_count,
            destination_count: pcpsp_problem.destination_count,
            resource_constraint_count: pcpsp_problem.resource_constraint_count,
            discount_rate: pcpsp_problem.discount_rate,
        },
        staged_relaxation_references,
        reference_period_routed_baseline: TemporalBaselineSummary {
            baseline_name: "cpit-period-routed".to_owned(),
            candidate_pcpsp_summary: reference_period_routed_baseline_summary.clone(),
            candidate_vs_reference_metrics: compare_named_numeric_metrics(
                &solution_metric_map(&reference_summary),
                &solution_metric_map(&reference_period_routed_baseline_summary),
                &BTreeMap::new(),
            ),
            candidate_vs_reference_period_alignment: compare_period_alignment(
                &pcpsp_solution,
                &reference_period_routed_baseline_solution,
            ),
        },
        nested_shell_bench_probe,
        lp_bz_baseline,
        mclaughlin_limit_lp_bz_sidecar,
        candidate_vs_reference_metrics: compare_named_numeric_metrics(
            &solution_metric_map(&reference_summary),
            &solution_metric_map(&candidate_pcpsp_summary),
            &BTreeMap::new(),
        ),
        candidate_vs_reference_period_alignment,
        candidate_vs_reference_destination_membership,
        temporal_routing_promotion_gate,
        reference_summary,
        candidate_summary,
    })
}

fn push_unique_comparability_gap_summary(
    comparability_gap_contract: &mut Vec<ComparabilityGapSummary>,
    gap_summary: ComparabilityGapSummary,
) {
    if !comparability_gap_contract
        .iter()
        .any(|existing| existing.summary == gap_summary.summary)
    {
        comparability_gap_contract.push(gap_summary);
    }
}

#[cfg(test)]
fn aggregation_comparability_gap(
    dataset_id: &str,
    aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
    primary_unit_family_traceability: Option<&PrimaryUnitFamilyTraceability>,
    has_nested_shell_bench_probe: bool,
) -> String {
    aggregation_comparability_gap_summary(
        dataset_id,
        aggregation_strategy,
        preferred_nested_shell_family_contract,
        primary_unit_family_traceability,
        has_nested_shell_bench_probe,
    )
    .summary
}

fn aggregation_comparability_gap_summary(
    dataset_id: &str,
    aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
    primary_unit_family_traceability: Option<&PrimaryUnitFamilyTraceability>,
    has_nested_shell_bench_probe: bool,
) -> ComparabilityGapSummary {
    if let Some(preferred_nested_shell_family_contract) = preferred_nested_shell_family_contract {
        let summary = match preferred_nested_shell_family_contract.realized_shell_count {
            Some(realized_shell_count) => format!(
                "the main candidate now uses {aggregation_strategy} units backed by {realized_shell_count} bounded shells from a {}-factor {} family, but the shell family is still a reproducible revenue/cost-aware proxy rather than a paper-reproduced pushback pipeline",
                preferred_nested_shell_family_contract.revenue_factor_count,
                preferred_nested_shell_family_contract
                    .shell_access_mode
                    .label()
            ),
            None => format!(
                "the main candidate now uses {aggregation_strategy} units from a {}-factor {} family, but the shell family is still a bounded reproducible proxy built from revenue/cost-aware factor scenarios rather than a paper-reproduced pushback pipeline",
                preferred_nested_shell_family_contract.revenue_factor_count,
                preferred_nested_shell_family_contract
                    .shell_access_mode
                    .label()
            ),
        };
        ComparabilityGapSummary {
            gap_id: "aggregation-proxy-family".to_owned(),
            gap_source: ComparabilityGapSource::AggregationFormulation,
            summary,
            evidence_fields: vec![
                "aggregation_strategy".to_owned(),
                "preferred_nested_shell_family_contract".to_owned(),
            ],
        }
    } else if dataset_id == "mclaughlin-limit" && aggregation_strategy == "nested-shell-bench" {
        let summary = if let Some(primary_unit_family_traceability) =
            primary_unit_family_traceability
        {
            let cut_readiness_clause = if let Some(cut_readiness) = primary_unit_family_traceability
                .benchmark_side_evidence
                .cut_readiness
                .as_ref()
            {
                format!(
                    " The report now also publishes benchmark-side cut readiness `{}` over {}/{} shell×bench phases, with {} multi-block refinement candidates before any localized-cut builder exists.",
                    cut_readiness.readiness_label,
                    cut_readiness.shell_bench_phase_count,
                    cut_readiness.phase_count,
                    cut_readiness.refinement_candidate_phase_count,
                )
            } else {
                String::new()
            };
            let scaffold_clause = if let Some(future_scaffold) = primary_unit_family_traceability
                .benchmark_side_evidence
                .future_scaffold
                .as_ref()
            {
                format!(
                    " The same report now also publishes future scaffold `{}` targeting {} while keeping outstanding gaps [{}] explicit.",
                    future_scaffold.scaffold_label,
                    future_scaffold.target_contracts.join(", "),
                    future_scaffold.outstanding_gap_labels.join(", "),
                )
            } else {
                String::new()
            };
            format!(
                "the main candidate now uses nested-shell × bench units rebuilt from open `*.upit` block values plus benchmark precedence as explicit pushback-equivalent mining units: `selected_block_source = \"{}\"` lifts {} selected blocks through {} shell×bench phases and {} scheduling units while keeping literature alignment `{}`.{}{} The report now also fixes the remaining benchmark-side cut/sidecar evidence as `cut_evidence_label = \"{}\"` and `sidecar_evidence_label = \"{}\"`. The family remains a bounded reproducible proxy rather than a paper-reproduced pushback/mining-unit pipeline.",
                primary_unit_family_traceability
                    .selected_block_provenance
                    .selected_block_source,
                primary_unit_family_traceability
                    .selected_block_provenance
                    .selected_block_count,
                primary_unit_family_traceability
                    .preferred_phase_plan_proxy
                    .preferred_phase_count,
                primary_unit_family_traceability.scheduling_unit_count,
                primary_unit_family_traceability.literature_alignment_label,
                cut_readiness_clause,
                scaffold_clause,
                primary_unit_family_traceability
                    .benchmark_side_evidence
                    .cut_evidence_label,
                primary_unit_family_traceability
                    .benchmark_side_evidence
                    .sidecar_evidence_label,
            )
        } else {
            "the main candidate now uses nested-shell × bench units rebuilt from open `*.upit` block values plus benchmark precedence as explicit pushback-equivalent mining units, but that family remains a bounded reproducible proxy rather than a paper-reproduced pushback/mining-unit pipeline".to_owned()
        };
        ComparabilityGapSummary {
            gap_id: "aggregation-open-upit-proxy-family".to_owned(),
            gap_source: ComparabilityGapSource::AggregationFormulation,
            summary,
            evidence_fields: primary_unit_family_traceability_field_paths(
                "primary_unit_family_traceability",
            ),
        }
    } else if has_nested_shell_bench_probe {
        ComparabilityGapSummary {
            gap_id: "reference-period-bench-primary-routing".to_owned(),
            gap_source: ComparabilityGapSource::AggregationFormulation,
            summary: "the main candidate still uses reference-period × bench units; a separate bounded nested-shell × bench probe is reported, but it is not yet the primary paper-comparable pipeline".to_owned(),
            evidence_fields: vec![
                "aggregation_strategy".to_owned(),
                "nested_shell_bench_probe".to_owned(),
            ],
        }
    } else {
        ComparabilityGapSummary {
            gap_id: "reference-period-bench-cpit-membership-proxy".to_owned(),
            gap_source: ComparabilityGapSource::AggregationFormulation,
            summary: "reference-period × bench units are still derived from staged CPIT memberships rather than from nested-shell pushbacks or literature-grade mining cuts".to_owned(),
            evidence_fields: vec![
                "aggregation_strategy".to_owned(),
                "selected_block_source".to_owned(),
            ],
        }
    }
}

#[cfg(test)]
fn temporal_solver_comparability_gap(lp_bz_baseline: Option<&LpBzBaselineSummary>) -> String {
    temporal_solver_comparability_gap_summary(lp_bz_baseline, None, None).summary
}

fn temporal_solver_comparability_gap_summary(
    lp_bz_baseline: Option<&LpBzBaselineSummary>,
    mclaughlin_limit_lp_bz_sidecar: Option<&MclaughlinLimitLpBzSidecarSummary>,
    primary_unit_family_traceability: Option<&PrimaryUnitFamilyTraceability>,
) -> ComparabilityGapSummary {
    if let Some(lp_bz_baseline) = lp_bz_baseline {
        let bibliographic_gap_ids = lp_bz_baseline
            .cut_access_law
            .bibliographic_gap_contract
            .iter()
            .map(|gap| gap.gap_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let summary = if MARVIN_MR187_PROMOTED_FAMILY_IS_ACTIVE_CANDIDATE {
            format!(
                "the main candidate now runs the Marvin-scoped focused LP/BZ route rebuilt on benchmark-side {LP_BZ_UNIT_GRANULARITY_LABEL} units as the single paper-like candidate family, and its audited `cut_access_law` now separates inter-phase/inter-cut release, local predecessor filtering, intra-phase progression, a benchmark-side partial ramp-access proxy, an explicit working-width proxy, a benchmark-side partial lineage / bench-continuity proxy, a benchmark-side partial complete-cut-design proxy and a structured bibliographic gap contract [{bibliographic_gap_ids}]. {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL} remains just a {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE}, while the active candidate still reports an explicit bounded local-optimizer runtime contract that distinguishes completed execution from budget hits or skips. ramp access, working width, lineage / bench continuity and complete cut design all remain benchmark-side partial proxies, so this remains exploratory-local evidence rather than a closure-grade literature workflow; the next credibility jump now depends on a shared/core-side or protocol-level mining-cut input contract rather than more sidecar-local tuning"
            )
        } else {
            format!(
                "the main candidate still uses ready_frontier; `lp_bz_baseline` only adds a Marvin-scoped focused LP/BZ sidecar rebuilt on benchmark-side {LP_BZ_UNIT_GRANULARITY_LABEL} units as the single paper-like candidate family, and its audited `cut_access_law` now separates inter-phase/inter-cut release, local predecessor filtering, intra-phase progression, a benchmark-side partial ramp-access proxy, an explicit working-width proxy, a benchmark-side partial lineage / bench-continuity proxy, a benchmark-side partial complete-cut-design proxy and a structured bibliographic gap contract [{bibliographic_gap_ids}]. {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL} remains just a {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE}, while the promoted sidecar now reports an explicit bounded local-optimizer runtime contract that distinguishes completed execution from budget hits or skips. ramp access, working width, lineage / bench continuity and complete cut design all remain benchmark-side partial proxies, so this remains exploratory-local evidence rather than a closure-grade literature workflow; the next credibility jump now depends on a shared/core-side or protocol-level mining-cut input contract rather than more sidecar-local tuning"
            )
        };
        ComparabilityGapSummary {
            gap_id: "lp-bz-temporal-solver-route".to_owned(),
            gap_source: ComparabilityGapSource::RelaxationModel,
            summary,
            evidence_fields: vec![
                "lp_bz_baseline".to_owned(),
                "lp_bz_baseline.cut_access_law.bibliographic_gap_contract".to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract"
                    .to_owned(),
            ],
        }
    } else if let Some(mclaughlin_limit_lp_bz_sidecar) = mclaughlin_limit_lp_bz_sidecar {
        let mut evidence_fields = vec![
            "candidate_summary".to_owned(),
            "reference_period_routed_baseline".to_owned(),
            "mclaughlin_limit_lp_bz_sidecar".to_owned(),
        ];
        if primary_unit_family_traceability.is_some() {
            evidence_fields.extend(primary_unit_family_traceability_field_paths(
                "primary_unit_family_traceability",
            ));
        }
        ComparabilityGapSummary {
            gap_id: "partial-lp-bz-sidecar".to_owned(),
            gap_source: ComparabilityGapSource::RelaxationModel,
            summary: format!(
                "the temporal solver still runs ready_frontier, but `mclaughlin-limit` now exposes `{}` as a benchmark-side LP/BZ sidecar with status `{}` and {}. {} The artifact is objective-aligned to PCPSP on the active shell×bench units, yet it remains a relaxed benchmark-side kernel without benchmark-side mining cuts or schedule-proof semantics, so the route stays exploratory-local.",
                mclaughlin_limit_lp_bz_sidecar.sidecar_label,
                mclaughlin_limit_lp_bz_sidecar.sidecar_status,
                mclaughlin_limit_lp_bz_sidecar.completeness_summary,
                mclaughlin_limit_lp_bz_sidecar.disclosure_summary,
            ),
            evidence_fields,
        }
    } else {
        ComparabilityGapSummary {
            gap_id: "missing-lp-bz-sidecar".to_owned(),
            gap_source: ComparabilityGapSource::RelaxationModel,
            summary: if let Some(primary_unit_family_traceability) =
                primary_unit_family_traceability
            {
                let cut_readiness_clause = if let Some(cut_readiness) =
                    primary_unit_family_traceability
                        .benchmark_side_evidence
                        .cut_readiness
                        .as_ref()
                {
                    format!(
                        " benchmark-side cut readiness `{}` keeps {}/{} shell×bench phases auditable and {} multi-block refinement candidates explicit, while",
                        cut_readiness.readiness_label,
                        cut_readiness.shell_bench_phase_count,
                        cut_readiness.phase_count,
                        cut_readiness.refinement_candidate_phase_count,
                    )
                } else {
                    String::new()
                };
                let scaffold_clause = if let Some(future_scaffold) =
                    primary_unit_family_traceability
                        .benchmark_side_evidence
                        .future_scaffold
                        .as_ref()
                {
                    format!(
                        " future scaffold `{}` is already published for {} while keeping outstanding gaps [{}], and",
                        future_scaffold.scaffold_label,
                        future_scaffold.target_contracts.join(", "),
                        future_scaffold.outstanding_gap_labels.join(", "),
                    )
                } else {
                    String::new()
                };
                format!(
                    "the temporal solver is still ready_frontier and no LP/BZ sidecar is available on this dataset:{}{} `cut_evidence_label = \"{}\"` and `sidecar_evidence_label = \"{}\"` keep the remaining benchmark-side cut/sidecar gap explicit for the active `{}` unit family, so the benchmark still lacks an LP/BZ-guided baseline with rounding or another literature-grade workflow",
                    cut_readiness_clause,
                    scaffold_clause,
                    primary_unit_family_traceability
                        .benchmark_side_evidence
                        .cut_evidence_label,
                    primary_unit_family_traceability
                        .benchmark_side_evidence
                        .sidecar_evidence_label,
                    primary_unit_family_traceability.unit_family_label,
                )
            } else {
                "the temporal solver is still ready_frontier and no LP/BZ sidecar is available on this dataset, so the benchmark still lacks an LP/BZ-guided baseline with rounding or another literature-grade workflow".to_owned()
            },
            evidence_fields: if primary_unit_family_traceability.is_some() {
                let mut evidence_fields = vec![
                    "candidate_summary".to_owned(),
                    "reference_period_routed_baseline".to_owned(),
                ];
                evidence_fields.extend(primary_unit_family_traceability_field_paths(
                    "primary_unit_family_traceability",
                ));
                evidence_fields
            } else {
                vec![
                    "candidate_summary".to_owned(),
                    "reference_period_routed_baseline".to_owned(),
                ]
            },
        }
    }
}

fn marvin_input_aggregation_traceability_gap_summary(
    lp_bz_baseline: &LpBzBaselineSummary,
) -> ComparabilityGapSummary {
    let mut evidence_fields =
        marvin_sidecar_traceability_field_paths("lp_bz_baseline.unit_family_traceability");
    evidence_fields
        .push("lp_bz_baseline.phase_refinement_diagnostics.total_cut_phase_count".to_owned());
    evidence_fields
        .push("lp_bz_baseline.summary.lp_bz_inputs.precedence_units.unit_count".to_owned());
    evidence_fields.push("preferred_nested_shell_family_contract".to_owned());
    ComparabilityGapSummary {
        gap_id: "marvin-input-aggregation-traceability".to_owned(),
        gap_source: ComparabilityGapSource::InputProtocol,
        summary: format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary(
            &lp_bz_baseline.unit_family_traceability,
            lp_bz_baseline
                .phase_refinement_diagnostics
                .total_cut_phase_count,
            lp_bz_baseline
                .summary
                .lp_bz_inputs
                .precedence_units
                .unit_count,
        ),
        evidence_fields,
    }
}

fn build_benchmark_contract_audit() -> BenchmarkContractAudit {
    BenchmarkContractAudit {
        audit_scope: "marvin-benchmark benchmark-side support modules and report wiring".to_owned(),
        modules: vec![
            BenchmarkContractModuleSummary {
                module_path: "examples/marvin-benchmark/src/minelib_scheduling_support.rs".to_owned(),
                contract_role: "dataset-aware phase-plan selection and scheduling normalization".to_owned(),
                scope_label: "benchmark-shared".to_owned(),
                maturity_label: "reusable benchmark contract".to_owned(),
                report_surface: {
                    let mut report_surface = vec![
                        "datasets[*].aggregation_strategy".to_owned(),
                        "datasets[*].selected_block_source".to_owned(),
                        "datasets[*].selected_block_provenance_summary".to_owned(),
                        "datasets[*].selected_block_provenance_chain".to_owned(),
                        "datasets[*].preferred_nested_shell_family_contract".to_owned(),
                        "datasets[*].primary_unit_family_traceability".to_owned(),
                        "datasets[*].marvin_paperlike_pipeline_checklist".to_owned(),
                        "datasets[*].mclaughlin_limit_promotion_checklist".to_owned(),
                        "datasets[*].benchmark_contract_roles".to_owned(),
                        "datasets[*].comparability_gaps".to_owned(),
                        "datasets[*].temporal_routing_promotion_gate".to_owned(),
                    ];
                    report_surface.extend(primary_unit_family_traceability_field_paths(
                        "datasets[*].primary_unit_family_traceability",
                    ));
                    report_surface
                },
                limitations: vec![
                    "still relies on bounded nested-shell proxies for Marvin and on open-UPIT shell/pushback-equivalent proxies for mclaughlin-limit rather than first-principles paper-grade pushback generation".to_owned(),
                ],
            },
            BenchmarkContractModuleSummary {
                module_path: "examples/marvin-benchmark/src/pushback_bench_localized_cut_support.rs"
                    .to_owned(),
                contract_role:
                    "shared localized-cut builder, access-law summary and refinement diagnostics"
                        .to_owned(),
                scope_label: "benchmark-shared".to_owned(),
                maturity_label: "reusable benchmark contract".to_owned(),
                report_surface: {
                    let mut report_surface = marvin_sidecar_traceability_field_paths(
                        "datasets[*].lp_bz_baseline.unit_family_traceability",
                    );
                    report_surface.extend([
                        "datasets[*].lp_bz_baseline.cut_access_law".to_owned(),
                        "datasets[*].lp_bz_baseline.phase_refinement_diagnostics".to_owned(),
                        "datasets[*].lp_bz_baseline.lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract".to_owned(),
                        "datasets[*].lp_bz_baseline.temporal_routing_promotion_gate".to_owned(),
                        "datasets[*].marvin_paperlike_pipeline_checklist".to_owned(),
                        "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.benchmark_cut_refinement".to_owned(),
                    ]);
                    report_surface
                },
                limitations: vec![
                    lp_bz_cut_scheduling_limitation_note(),
                    mclaughlin_limit_benchmark_cut_limitation_note(),
                ],
            },
            BenchmarkContractModuleSummary {
                module_path: "examples/marvin-benchmark/src/lp_bz_adapter.rs".to_owned(),
                contract_role: "Marvin-scoped LP/BZ + rounding sidecar summary".to_owned(),
                scope_label: "marvin-only".to_owned(),
                maturity_label: "exploratory sidecar".to_owned(),
                report_surface: vec![
                    "datasets[*].lp_bz_baseline.summary".to_owned(),
                    "datasets[*].lp_bz_baseline.lp_bz_promotion_readiness".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_kernel.kernel_label".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.solve_status".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.strategy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.enforced_precedence_rows".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.total_precedence_rows".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.skipped_precedence_rows".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.rounded_discounted_target_score_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.repaired_discounted_target_score_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_discounted_target_score_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_repair_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_round_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.improvement_status".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_budget_profile.mode_label".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.execution_state".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.budget_hit".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.summary".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_termination_reason".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_executed_iteration_count".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_search_discounted_target_score_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.local_search_score_delta_vs_focused_proxy".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.target_period_change_count_vs_focused".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_residual_opportunity.improving_move_available".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_residual_opportunity.move_kind_label".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_residual_opportunity.discounted_gain".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.closure_status".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.competitive_probe_proxy_gap_closure_share".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.empirical_dominant_blocker".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.empirical_dominant_blocker_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.empirical_driver_evidence_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.empirical_driver_evidence".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.budget_coverage_experiment".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.remaining_blocker_count".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_criteria_version".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_state".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_summary".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_blocked_criteria_count".to_owned(),
                    "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_criteria".to_owned(),
                    "datasets[*].lp_bz_baseline.candidate_vs_reference_metrics".to_owned(),
                    "datasets[*].lp_bz_baseline.temporal_routing_promotion_gate".to_owned(),
                ],
                limitations: vec![
                    "adapter remains exploratory-local evidence and should not be treated as shared/core scheduling logic".to_owned(),
                ],
            },
            BenchmarkContractModuleSummary {
                module_path: "examples/marvin-benchmark/src/main.rs".to_owned(),
                contract_role: "focused Marvin sweep harness and promotion sandbox".to_owned(),
                scope_label: "marvin research harness".to_owned(),
                maturity_label: "experimental probe".to_owned(),
                report_surface: vec![
                    "datasets/benchmarks/marvin/outputs/mr187-focused-refresh-report.json".to_owned(),
                    "datasets/benchmarks/marvin/outputs/mr187-focused-refresh-report.json::paperlike_pipeline_checklist".to_owned(),
                    "datasets/benchmarks/marvin/outputs/mr187-focused-refresh-report.json::lp_bz_promotion_readiness".to_owned(),
                    "datasets/benchmarks/marvin/outputs/mr187-focused-refresh-report.json::runtime_profile.promoted_local_optimizer_runtime_budget_contract".to_owned(),
                ],
                limitations: vec![
                    format!(
                        "sweeps and promotion candidates remain research evidence until they are re-expressed through shared benchmark contracts; {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL} stays a local optimizer scaffold and {LP_BZ_UNIT_GRANULARITY_LABEL} stays exploratory-local"
                    ),
                ],
            },
            BenchmarkContractModuleSummary {
                module_path: "examples/marvin-benchmark/src/bin/multi_mine_scheduler.rs".to_owned(),
                contract_role: "multi-mine report composition and comparability classification"
                    .to_owned(),
                scope_label: "benchmark reporting".to_owned(),
                maturity_label: "report composition contract".to_owned(),
                report_surface: vec![
                    "benchmark_contract_audit".to_owned(),
                    "diagnostics_schema".to_owned(),
                    "datasets[*].comparison_classification".to_owned(),
                    "datasets[*].primary_unit_family_traceability".to_owned(),
                    "datasets[*].marvin_paperlike_pipeline_checklist".to_owned(),
                    "datasets[*].mclaughlin_limit_lp_bz_sidecar".to_owned(),
                    "datasets[*].mclaughlin_limit_promotion_checklist".to_owned(),
                    "datasets[*].temporal_routing_promotion_gate".to_owned(),
                ],
                limitations: vec![
                    "the report can formalize evidence and gaps, including the benchmark-side sidecar-only provenance/input-aggregation clause, but it does not upgrade exploratory-local methods into literature-grade pipelines by itself".to_owned(),
                ],
            },
        ],
        promotion_rules: vec![
            "Only reusable benchmark contracts should flow into shared report surfaces; exploratory sidecars must stay explicitly labeled.".to_owned(),
            "Benchmark-side modules may become primary baselines only when their units, access law and financial assumptions are paper-comparable.".to_owned(),
            "MR-206 requires candidate promotion to clear explicit temporal/routing thresholds as well as NPV: used_period_count delta, mean_absolute_period_delta, earlier_than_reference_count and (period,destination) similarity are versioned in `temporal_routing_promotion_gate`.".to_owned(),
            format!(
                "For MR-187.A/B, `{LP_BZ_UNIT_GRANULARITY_LABEL}` is the only benchmark-side paper-like candidate family, its audited `cut_access_law` stays exploratory-local until ramp access, working width and lineage / bench continuity mature beyond benchmark-side partial coverage and complete cut design exists, and `{MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL}` must remain labeled as a {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE}."
            ),
            "For MR-207, `mclaughlin-limit` may only advance through benchmark-side promotion when `pushback-equivalent-bench-cut-readiness` remains auditable, the remaining cut-refinement / LP-BZ prerequisites and exit criteria stay versioned explicitly in the scaffold contract, benchmark-side mining-cut refinement is versioned explicitly, and any LP/BZ sidecar input contract is published without reclassifying `mclaughlin-full` away from stress-only local status.".to_owned(),
            "Marvin-specific support must remain outside core crates even when promoted inside benchmark reporting.".to_owned(),
        ],
    }
}

fn build_benchmark_diagnostics_schema() -> BenchmarkDiagnosticsSchema {
    BenchmarkDiagnosticsSchema {
        schema_version: "v2".to_owned(),
        classification_labels: vec![
            "paper-comparable".to_owned(),
            "exploratory-local".to_owned(),
            "smoke-test".to_owned(),
        ],
        required_groups: vec![
            BenchmarkDiagnosticsGroup {
                group_name: "comparability".to_owned(),
                fields: vec![
                    "comparison_classification".to_owned(),
                    "comparability_gaps".to_owned(),
                    "temporal_routing_promotion_gate".to_owned(),
                    "instance_variant".to_owned(),
                    "literature_reference_instance".to_owned(),
                    "aggregation_strategy".to_owned(),
                    "preferred_nested_shell_family_contract".to_owned(),
                    "selected_block_source".to_owned(),
                    "selected_block_provenance_summary".to_owned(),
                    "selected_block_provenance_chain".to_owned(),
                    "primary_unit_family_traceability".to_owned(),
                ],
                sourced_from: vec![
                    "datasets[*]".to_owned(),
                    "datasets[*].reference_period_routed_baseline".to_owned(),
                ],
                intent: "Explain whether a run is paper-comparable or still exploratory-local before any objective comparison.".to_owned(),
            },
            BenchmarkDiagnosticsGroup {
                group_name: "objective-and-schedule-summary".to_owned(),
                fields: vec![
                    "problem_summary".to_owned(),
                    "reference_summary".to_owned(),
                    "candidate_summary".to_owned(),
                    "candidate_vs_reference_metrics".to_owned(),
                ],
                sourced_from: vec!["datasets[*]".to_owned()],
                intent: "Track discounted objective, schedule size, unit count and basic solution totals.".to_owned(),
            },
            BenchmarkDiagnosticsGroup {
                group_name: "temporal-and-membership-alignment".to_owned(),
                fields: vec![
                    "candidate_vs_reference_period_alignment".to_owned(),
                    "candidate_vs_reference_destination_membership".to_owned(),
                    "temporal_routing_promotion_gate".to_owned(),
                    "lp_bz_baseline.temporal_routing_promotion_gate".to_owned(),
                    "reference_period_routed_baseline.candidate_vs_reference_period_alignment"
                        .to_owned(),
                ],
                sourced_from: vec![
                    "datasets[*]".to_owned(),
                    "datasets[*].reference_period_routed_baseline".to_owned(),
                    "datasets[*].lp_bz_baseline".to_owned(),
                ],
                intent: "Show whether gains or regressions come from period moves, destination drift or pure membership changes.".to_owned(),
            },
            BenchmarkDiagnosticsGroup {
                group_name: "paperlike-pipeline-checklist".to_owned(),
                fields: {
                    let mut fields =
                        marvin_sidecar_traceability_field_paths("lp_bz_baseline.unit_family_traceability");
                    fields.extend([
                        "marvin_paperlike_pipeline_checklist".to_owned(),
                        "selected_block_provenance_summary".to_owned(),
                        "selected_block_provenance_chain".to_owned(),
                        "preferred_nested_shell_family_contract".to_owned(),
                        "lp_bz_baseline.cut_access_law".to_owned(),
                        "lp_bz_baseline.phase_refinement_diagnostics".to_owned(),
                        "lp_bz_baseline.lp_bz_promotion_readiness".to_owned(),
                        "lp_bz_baseline.lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract".to_owned(),
                        "lp_bz_baseline.temporal_routing_promotion_gate".to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_summary"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.budget_coverage_experiment"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blocker_count"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers_summary"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.readiness_criteria_version"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.readiness_state"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.readiness_summary"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.readiness_blocked_criteria_count"
                            .to_owned(),
                        "lp_bz_baseline.competitive_ready_frontier_probe.readiness_criteria"
                            .to_owned(),
                    ]);
                    fields.extend(marvin_sidecar_runtime_contract_field_paths("lp_bz_baseline"));
                    fields
                },
                sourced_from: vec![
                    "datasets[*]".to_owned(),
                    "datasets[*].lp_bz_baseline".to_owned(),
                ],
                intent: "Compress the active Marvin paper-like benchmark contracts into an auditable checklist instead of relying on scattered narrative assumptions.".to_owned(),
            },
            BenchmarkDiagnosticsGroup {
                group_name: "mclaughlin-limit-promotion-checklist".to_owned(),
                fields: vec![
                    "mclaughlin_limit_promotion_checklist".to_owned(),
                    "mclaughlin_limit_lp_bz_sidecar".to_owned(),
                    "comparison_classification".to_owned(),
                    "comparability_gap_contract".to_owned(),
                    "comparability_gaps".to_owned(),
                    "benchmark_contract_roles".to_owned(),
                    "instance_variant".to_owned(),
                    "literature_reference_instance".to_owned(),
                    "same_literature_variant".to_owned(),
                    "selected_block_source".to_owned(),
                    "selected_block_provenance_summary".to_owned(),
                    "selected_block_provenance_chain".to_owned(),
                    "primary_unit_family_traceability".to_owned(),
                    "temporal_routing_promotion_gate".to_owned(),
                ],
                sourced_from: vec!["datasets[*]".to_owned()],
                intent: "Make the future promotion path for `mclaughlin-limit` explicit while cut refinement remains pending and the first LP/BZ sidecar stays benchmark-side partial.".to_owned(),
            },
            BenchmarkDiagnosticsGroup {
                group_name: "phase-plan-and-access-law".to_owned(),
                fields: {
                    let mut fields =
                        marvin_sidecar_traceability_field_paths("lp_bz_baseline.unit_family_traceability");
                    fields.extend([
                        "benchmark_contract_roles".to_owned(),
                        "diagnostic_groups_present".to_owned(),
                        "selected_block_provenance_summary".to_owned(),
                        "selected_block_provenance_chain".to_owned(),
                        "primary_unit_family_traceability".to_owned(),
                        "preferred_nested_shell_family_contract".to_owned(),
                        "nested_shell_bench_probe".to_owned(),
                        "lp_bz_baseline.cut_access_law".to_owned(),
                        "lp_bz_baseline.phase_refinement_diagnostics".to_owned(),
                        "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract"
                            .to_owned(),
                    ]);
                    fields
                },
                sourced_from: vec![
                    "datasets[*]".to_owned(),
                    "datasets[*].lp_bz_baseline".to_owned(),
                ],
                intent: "Expose the active unit family plus the layered release/filter/progression access-law contract and refinement behavior behind each benchmark result.".to_owned(),
            },
            BenchmarkDiagnosticsGroup {
                group_name: "relaxations-and-sidecars".to_owned(),
                fields: vec![
                    "staged_relaxation_references".to_owned(),
                    "lp_bz_baseline.summary".to_owned(),
                    "lp_bz_baseline.lp_bz_promotion_readiness".to_owned(),
                    "lp_bz_baseline.temporal_routing_promotion_gate".to_owned(),
                    "mclaughlin_limit_lp_bz_sidecar".to_owned(),
                    "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract"
                        .to_owned(),
                ],
                sourced_from: vec![
                    "datasets[*].staged_relaxation_references".to_owned(),
                    "datasets[*].lp_bz_baseline".to_owned(),
                ],
                intent: "Separate CPIT/LP references from LP/BZ sidecars so bounds, relaxations and feasible schedules are not conflated.".to_owned(),
            },
        ],
    }
}

fn build_dataset_contract_roles(
    config: &DatasetConfig,
    aggregation_strategy: &str,
    has_nested_shell_bench_probe: bool,
    has_lp_bz_baseline: bool,
    has_mclaughlin_limit_benchmark_cut_refinement: bool,
    has_mclaughlin_limit_lp_bz_sidecar: bool,
) -> Vec<String> {
    let mut roles = vec![
        "dataset-aware-phase-plan-selector".to_owned(),
        "multi-mine-report-classifier".to_owned(),
        format!("primary-unit-family:{aggregation_strategy}"),
    ];
    if has_nested_shell_bench_probe {
        roles.push("bounded-nested-shell-bench-probe".to_owned());
    }
    if has_lp_bz_baseline {
        roles.push(if MARVIN_MR187_PROMOTED_FAMILY_IS_ACTIVE_CANDIDATE {
            "marvin-lp-bz-active-candidate".to_owned()
        } else {
            "marvin-lp-bz-sidecar".to_owned()
        });
        roles.push(format!(
            "paperlike-candidate-family:{LP_BZ_UNIT_GRANULARITY_LABEL}"
        ));
        roles.push(format!(
            "local-optimizer-scaffold:{MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL}"
        ));
        roles.push(format!(
            "promoted-build-label:{MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL}"
        ));
    }
    if has_mclaughlin_limit_benchmark_cut_refinement {
        roles.push("mclaughlin-limit-benchmark-cut-refinement".to_owned());
    }
    if has_mclaughlin_limit_lp_bz_sidecar {
        roles.push("mclaughlin-limit-lp-bz-bound-sidecar".to_owned());
    }
    if config.dataset_id == "mclaughlin-limit" && aggregation_strategy == "nested-shell-bench" {
        roles.push("mclaughlin-limit-pushback-equivalent-routing".to_owned());
        roles.push("mclaughlin-limit-cut-sidecar-scaffold".to_owned());
    }
    if !config.same_literature_variant {
        roles.push("stress-only-local-variant".to_owned());
    }
    if config.dataset_id == "marvin" {
        roles.push("marvin-focused-research-harness".to_owned());
    }
    roles
}

fn build_primary_unit_family_traceability(
    config: &DatasetConfig,
    selected_block_source: &str,
    selected_block_count: usize,
    selected_block_provenance_summary: &str,
    selected_block_provenance_chain: &[String],
    aggregation_strategy: &str,
    phase_plan: &PushbackPlan,
    phase_count: usize,
    unique_shell_count: Option<usize>,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
    has_lp_bz_baseline: bool,
    mclaughlin_limit_benchmark_cut_refinement: Option<
        &MclaughlinLimitBenchmarkCutRefinementSummary,
    >,
    mclaughlin_limit_lp_bz_sidecar: Option<&MclaughlinLimitLpBzSidecarSummary>,
    scheduling_unit_count: usize,
) -> PrimaryUnitFamilyTraceability {
    let literature_alignment_label = if config.same_literature_variant {
        "literature-target-variant"
    } else {
        "stress-only-local-variant"
    };
    let unit_family_role = if config.dataset_id == "mclaughlin-limit"
        && aggregation_strategy == "nested-shell-bench"
    {
        "pushback-equivalent shell×bench routing"
    } else if preferred_nested_shell_family_contract.is_some() {
        "bounded nested-shell × bench routing"
    } else if !config.same_literature_variant {
        "stress-only reference-period × bench fallback"
    } else {
        "reference-period × bench routing"
    };
    let cut_readiness =
        build_benchmark_cut_readiness_traceability(config, aggregation_strategy, phase_plan);
    let future_scaffold = build_benchmark_side_scaffold_traceability(
        config,
        aggregation_strategy,
        &cut_readiness,
        mclaughlin_limit_benchmark_cut_refinement,
        mclaughlin_limit_lp_bz_sidecar,
    );
    let benchmark_side_evidence = if config.dataset_id == "mclaughlin-limit"
        && aggregation_strategy == "nested-shell-bench"
    {
        let (sidecar_evidence_label, sidecar_evidence_summary) = if let Some(
            mclaughlin_limit_lp_bz_sidecar,
        ) =
            mclaughlin_limit_lp_bz_sidecar
        {
            (
                "mclaughlin-limit-lp-bz-bound-sidecar".to_owned(),
                mclaughlin_limit_lp_bz_sidecar.disclosure_summary.clone(),
            )
        } else {
            (
                    "no-lp-bz-sidecar".to_owned(),
                    "No LP/BZ sidecar is currently versioned for `mclaughlin-limit`; the temporal route remains `ready_frontier` over the pushback-equivalent unit family.".to_owned(),
                )
        };
        BenchmarkSideEvidenceTraceability {
            benchmark_scope_label: "benchmark-side-comparable-proxy".to_owned(),
            mining_unit_evidence_summary: "Open `*.upit` net values plus benchmark precedence rebuild bounded shells that compress into pushback-equivalent shell×bench mining units before scheduling.".to_owned(),
            cut_evidence_label: if mclaughlin_limit_benchmark_cut_refinement.is_some() {
                "mclaughlin-limit-benchmark-cut-refinement".to_owned()
            } else {
                "no-benchmark-cut-refinement".to_owned()
            },
            cut_evidence_summary: if let Some(cut_contract) = mclaughlin_limit_benchmark_cut_refinement {
                cut_contract.disclosure_summary.clone()
            } else {
                "No benchmark-side localized-cut/mining-cut builder is wired for `mclaughlin-limit` yet; the remaining cut-side gap is therefore narrowed to auditable shell×bench refinement readiness rather than an unqualified absence of structure.".to_owned()
            },
            benchmark_cut_refinement: mclaughlin_limit_benchmark_cut_refinement.cloned(),
            cut_readiness,
            future_scaffold,
            sidecar_evidence_label,
            sidecar_evidence_summary,
        }
    } else if config.dataset_id == "marvin" && has_lp_bz_baseline {
        BenchmarkSideEvidenceTraceability {
            benchmark_scope_label: "benchmark-side-paperlike-sidecar".to_owned(),
            mining_unit_evidence_summary: "The primary benchmark family stays bounded nested-shell × bench, while the localized-cut/LP-BZ sidecar publishes a separate benchmark-side paper-like scaffold.".to_owned(),
            cut_evidence_label: "marvin-localized-cut-sidecar".to_owned(),
            cut_evidence_summary: "Localized cuts are benchmark-side sidecar evidence rebuilt from the primary shell×bench family; they remain outside shared/core scheduling logic.".to_owned(),
            benchmark_cut_refinement: None,
            cut_readiness: None,
            future_scaffold: None,
            sidecar_evidence_label: "marvin-lp-bz-sidecar".to_owned(),
            sidecar_evidence_summary: "An audited Marvin-only LP/BZ sidecar is versioned as exploratory-local evidence, not as a literature-grade shared workflow.".to_owned(),
        }
    } else if !config.same_literature_variant {
        BenchmarkSideEvidenceTraceability {
            benchmark_scope_label: "stress-only-local-variant".to_owned(),
            mining_unit_evidence_summary: "This route is kept only as a local stress benchmark and does not claim literature-grade mining-unit comparability.".to_owned(),
            cut_evidence_label: "stress-only-no-cut-contract".to_owned(),
            cut_evidence_summary: "No benchmark-side mining-cut contract is claimed for the stress-only `mclaughlin-full` path.".to_owned(),
            benchmark_cut_refinement: None,
            cut_readiness: None,
            future_scaffold: None,
            sidecar_evidence_label: "stress-only-no-sidecar".to_owned(),
            sidecar_evidence_summary: "No LP/BZ sidecar is tracked for the stress-only path because the variant is intentionally excluded from literature-grade comparability claims.".to_owned(),
        }
    } else {
        BenchmarkSideEvidenceTraceability {
            benchmark_scope_label: "benchmark-shared-routing".to_owned(),
            mining_unit_evidence_summary: "Scheduling currently relies on the reported benchmark routing family without an extra cut-sidecar scaffold.".to_owned(),
            cut_evidence_label: "no-benchmark-cut-refinement".to_owned(),
            cut_evidence_summary: "No additional benchmark-side mining-cut refinement is versioned for this routing family.".to_owned(),
            benchmark_cut_refinement: None,
            cut_readiness: None,
            future_scaffold: None,
            sidecar_evidence_label: "no-lp-bz-sidecar".to_owned(),
            sidecar_evidence_summary: "No LP/BZ sidecar is versioned for this routing family.".to_owned(),
        }
    };
    PrimaryUnitFamilyTraceability {
        unit_family_label: aggregation_strategy.to_owned(),
        unit_family_role: unit_family_role.to_owned(),
        literature_alignment_label: literature_alignment_label.to_owned(),
        selected_block_provenance: SelectedBlockProvenanceTraceability {
            selected_block_source: selected_block_source.to_owned(),
            selected_block_count,
            selected_block_provenance_summary: selected_block_provenance_summary.to_owned(),
            selected_block_provenance_chain: selected_block_provenance_chain.to_vec(),
        },
        preferred_phase_plan_proxy: PreferredPhasePlanTraceability {
            aggregation_strategy: aggregation_strategy.to_owned(),
            preferred_phase_count: phase_count,
            unique_shell_count,
            preferred_nested_shell_factor_count: preferred_nested_shell_family_contract
                .map(|contract| contract.revenue_factor_count),
            preferred_nested_shell_realized_shell_count: preferred_nested_shell_family_contract
                .and_then(|contract| contract.realized_shell_count),
            preferred_nested_shell_access_mode: preferred_nested_shell_family_contract
                .map(|contract| contract.shell_access_mode.label().to_owned()),
        },
        benchmark_side_evidence,
        scheduling_unit_count,
    }
}

fn build_benchmark_cut_readiness_traceability(
    config: &DatasetConfig,
    aggregation_strategy: &str,
    phase_plan: &PushbackPlan,
) -> Option<BenchmarkCutReadinessTraceability> {
    if config.dataset_id != "mclaughlin-limit" || aggregation_strategy != "nested-shell-bench" {
        return None;
    }

    let phase_count = phase_plan.phase_count;
    let shell_bench_phase_count = phase_plan
        .phases
        .iter()
        .filter(|phase| phase.shell_index.is_some() && phase.bench.is_some())
        .count();
    let predecessor_traced_phase_count = phase_plan
        .phases
        .iter()
        .filter(|phase| !phase.predecessor_phase_ids.is_empty())
        .count();
    let multi_block_phase_count = phase_plan
        .phases
        .iter()
        .filter(|phase| phase.block_count > 1)
        .count();
    let refinement_candidate_phase_count = phase_plan
        .phases
        .iter()
        .filter(|phase| {
            phase.shell_index.is_some() && phase.bench.is_some() && phase.block_count > 1
        })
        .count();

    Some(BenchmarkCutReadinessTraceability {
        readiness_label: "pushback-equivalent-bench-cut-readiness".to_owned(),
        readiness_summary: format!(
            "{} of {} preferred phases already keep explicit shell+bench provenance, {} keep predecessor lineage, and {} remain multi-block shell×bench refinement candidates for any future benchmark-side mining-cut builder.",
            shell_bench_phase_count,
            phase_count,
            predecessor_traced_phase_count,
            refinement_candidate_phase_count,
        ),
        phase_count,
        shell_bench_phase_count,
        predecessor_traced_phase_count,
        multi_block_phase_count,
        refinement_candidate_phase_count,
    })
}

fn build_benchmark_side_scaffold_traceability(
    config: &DatasetConfig,
    aggregation_strategy: &str,
    cut_readiness: &Option<BenchmarkCutReadinessTraceability>,
    mclaughlin_limit_benchmark_cut_refinement: Option<
        &MclaughlinLimitBenchmarkCutRefinementSummary,
    >,
    mclaughlin_limit_lp_bz_sidecar: Option<&MclaughlinLimitLpBzSidecarSummary>,
) -> Option<BenchmarkSideScaffoldTraceability> {
    if config.dataset_id != "mclaughlin-limit" || aggregation_strategy != "nested-shell-bench" {
        return None;
    }

    let cut_readiness = cut_readiness.as_ref()?;
    let benchmark_cut_contract_status = if mclaughlin_limit_benchmark_cut_refinement.is_some() {
        "benchmark-side-implemented"
    } else {
        "scaffold-only-not-implemented"
    };
    let lp_bz_partial_bound_available = mclaughlin_limit_lp_bz_sidecar.is_some();
    let lp_bz_sidecar_contract_status = if lp_bz_partial_bound_available {
        "partial-bound-available"
    } else {
        "scaffold-only-not-implemented"
    };
    let variant_scope_summary = "`mclaughlin-full` remains outside this scaffold: only the literature-target `mclaughlin-limit` route may consume these future benchmark-side cut-refinement / LP-BZ sidecar contracts, while the full variant stays stress-only.".to_owned();
    let benchmark_cut_prerequisites = vec![
        BenchmarkSidePromotionPrerequisite {
            prerequisite_id: "pushback-equivalent-bench-cut-readiness".to_owned(),
            prerequisite_label: "Pushback-equivalent bench-cut readiness".to_owned(),
            status: "audited".to_owned(),
            summary: format!(
                "Readiness `{}` already keeps {}/{} shell×bench phases auditable with {} multi-block refinement candidates before any localized-cut builder exists.",
                cut_readiness.readiness_label,
                cut_readiness.shell_bench_phase_count,
                cut_readiness.phase_count,
                cut_readiness.refinement_candidate_phase_count,
            ),
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.cut_readiness"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.readiness_dependency_label"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionPrerequisite {
            prerequisite_id: "benchmark-side-mining-cut-refinement-implementation".to_owned(),
            prerequisite_label: "Benchmark-side mining-cut refinement implementation".to_owned(),
            status: if let Some(cut_contract) = mclaughlin_limit_benchmark_cut_refinement {
                cut_contract.contract_status.clone()
            } else {
                "blocked".to_owned()
            },
            summary: if let Some(cut_contract) = mclaughlin_limit_benchmark_cut_refinement {
                format!(
                    "Benchmark-side mining-cut refinement is now versioned as `{}` / build `{}` with contract version `{}`. {}",
                    cut_contract.localized_cut_builder_label,
                    cut_contract.build_label,
                    cut_contract.contract_version,
                    cut_contract.disclosure_summary,
                )
            } else {
                "No benchmark-side mining-cut builder is versioned yet on top of the audited shell×bench family, so the cut contract remains scaffold-only.".to_owned()
            },
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.benchmark_cut_refinement"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.target_contracts"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.outstanding_gap_labels"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                    .to_owned(),
            ],
        },
    ];
    let lp_bz_sidecar_prerequisites = vec![
        BenchmarkSidePromotionPrerequisite {
            prerequisite_id: "pushback-equivalent-bench-cut-readiness".to_owned(),
            prerequisite_label: "Pushback-equivalent bench-cut readiness".to_owned(),
            status: "audited".to_owned(),
            summary: format!(
                "The same readiness `{}` keeps the future LP/BZ input family anchored to audited shell×bench phases instead of an implicit proxy.",
                cut_readiness.readiness_label,
            ),
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.cut_readiness"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.readiness_dependency_label"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionPrerequisite {
            prerequisite_id: "benchmark-side-mining-cut-refinement".to_owned(),
            prerequisite_label: "Benchmark-side mining-cut refinement".to_owned(),
            status: if mclaughlin_limit_benchmark_cut_refinement.is_some() {
                "benchmark-side-implemented".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: if let Some(cut_contract) = mclaughlin_limit_benchmark_cut_refinement {
                format!(
                    "Upstream cut refinement is now versioned as `{}` / build `{}` with status `{}`; the LP/BZ sidecar can now depend on an explicit benchmark-side cut layer instead of bypassing it.",
                    cut_contract.localized_cut_builder_label,
                    cut_contract.build_label,
                    cut_contract.contract_status,
                )
            } else {
                "A benchmark-side mining-cut refinement contract must be versioned first; otherwise any LP/BZ sidecar input family would still bypass the missing cut layer.".to_owned()
            },
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.benchmark_cut_refinement"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionPrerequisite {
            prerequisite_id: "lp-bz-sidecar-input-family-implementation".to_owned(),
            prerequisite_label: "LP/BZ sidecar input family implementation".to_owned(),
            status: if lp_bz_sidecar_contract_status == "benchmark-side-implemented" {
                "benchmark-side-implemented".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: if let Some(mclaughlin_limit_lp_bz_sidecar) = mclaughlin_limit_lp_bz_sidecar {
                format!(
                    "A first benchmark-side `mclaughlin-limit` LP/BZ sidecar is now versioned as `{}` with artifact status `{}` and contract step `{}`: {} This unlocks benchmark-side MR-206 auditing, but the route stays blocked from promotion because benchmark-side cut refinement is still scaffold-only and the sidecar remains partial diagnostic evidence rather than `benchmark-side-implemented`.",
                    mclaughlin_limit_lp_bz_sidecar.sidecar_label,
                    mclaughlin_limit_lp_bz_sidecar.sidecar_status,
                    lp_bz_sidecar_contract_status,
                    mclaughlin_limit_lp_bz_sidecar.completeness_summary,
                )
            } else {
                "No `mclaughlin-limit` LP/BZ sidecar candidate is versioned yet, so the sidecar contract remains scaffold-only and MR-206 stays advisory here.".to_owned()
            },
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.target_contracts"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_contract_status"
                    .to_owned(),
                "mclaughlin_limit_lp_bz_sidecar".to_owned(),
                "temporal_routing_promotion_gate".to_owned(),
            ],
        },
        BenchmarkSidePromotionPrerequisite {
            prerequisite_id: "stress-only-full-variant-separation".to_owned(),
            prerequisite_label: "Stress-only full variant separation".to_owned(),
            status: "audited".to_owned(),
            summary: variant_scope_summary.clone(),
            evidence_fields: vec![
                "instance_variant".to_owned(),
                "same_literature_variant".to_owned(),
                "benchmark_contract_roles".to_owned(),
            ],
        },
    ];
    let benchmark_cut_blocking_prerequisite_ids =
        blocking_scaffold_prerequisite_ids(&benchmark_cut_prerequisites);
    let benchmark_cut_promotion_ready = benchmark_cut_blocking_prerequisite_ids.is_empty();
    let lp_bz_sidecar_blocking_prerequisite_ids =
        blocking_scaffold_prerequisite_ids(&lp_bz_sidecar_prerequisites);
    let lp_bz_sidecar_promotion_ready = lp_bz_sidecar_blocking_prerequisite_ids.is_empty();
    let benchmark_cut_exit_criteria = vec![
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "publish-benchmark-cut-readiness-traceability".to_owned(),
            criterion_label: "Publish benchmark-side cut-readiness traceability".to_owned(),
            target_contract: "benchmark-side-mining-cut-refinement".to_owned(),
            evaluation_mode: "traceability-surface-must-remain-audited".to_owned(),
            expected_state:
                "cut_readiness is present with audited shell×bench lineage and refinement candidates"
                    .to_owned(),
            current_state: format!(
                "cut_readiness `{}` keeps {}/{} shell×bench phases auditable and {} refinement candidates visible",
                cut_readiness.readiness_label,
                cut_readiness.shell_bench_phase_count,
                cut_readiness.phase_count,
                cut_readiness.refinement_candidate_phase_count,
            ),
            status: "satisfied".to_owned(),
            summary: "The current benchmark-side proxy already exposes the readiness traceability needed to test any future promotion against explicit shell×bench evidence instead of an implicit family label.".to_owned(),
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.cut_readiness"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.readiness_dependency_label"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "version-benchmark-cut-refinement-contract".to_owned(),
            criterion_label: "Version benchmark-side mining-cut refinement contract".to_owned(),
            target_contract: "benchmark-side-mining-cut-refinement".to_owned(),
            evaluation_mode: "contract-status-must-equal-benchmark-side-implemented".to_owned(),
            expected_state:
                "benchmark_cut_contract_status = benchmark-side-implemented".to_owned(),
            current_state: format!(
                "benchmark_cut_contract_status = {}",
                benchmark_cut_contract_status,
            ),
            status: if benchmark_cut_contract_status == "benchmark-side-implemented" {
                "satisfied".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: if let Some(cut_contract) = mclaughlin_limit_benchmark_cut_refinement {
                format!(
                    "Benchmark-side cut refinement is now versioned with `{}` / build `{}` and contract version `{}`; the contract-status gate is satisfied, although overall comparability still remains benchmark-side only.",
                    cut_contract.localized_cut_builder_label,
                    cut_contract.build_label,
                    cut_contract.contract_version,
                )
            } else {
                "Promotion cannot move beyond scaffold-only while the cut-refinement contract itself is still absent from the benchmark-side report surface.".to_owned()
            },
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.target_contracts"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "clear-benchmark-cut-promotion-rule".to_owned(),
            criterion_label: "Clear benchmark-cut promotion rule".to_owned(),
            target_contract: "benchmark-side-mining-cut-refinement".to_owned(),
            evaluation_mode: "promotion-flag-and-rule-status-must-both-clear".to_owned(),
            expected_state:
                "benchmark_cut_promotion_ready = true and benchmark_cut_promotion_rule.status = ready"
                    .to_owned(),
            current_state: format!(
                "benchmark_cut_promotion_ready = {}, rule status = {}, blockers [{}]",
                benchmark_cut_promotion_ready,
                if benchmark_cut_promotion_ready {
                    "ready"
                } else {
                    "blocked"
                },
                benchmark_cut_blocking_prerequisite_ids.join(", "),
            ),
            status: if benchmark_cut_promotion_ready {
                "satisfied".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: "The promotion flag stays testable on its own: the contract cannot clear until every blocked benchmark-side prerequisite disappears from the scaffold.".to_owned(),
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_ready"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_blocking_prerequisite_ids"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule"
                    .to_owned(),
            ],
        },
    ];
    let lp_bz_sidecar_exit_criteria = vec![
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "preserve-limit-only-scope".to_owned(),
            criterion_label: "Preserve limit-only benchmark scope".to_owned(),
            target_contract: "lp-bz-sidecar-input-family".to_owned(),
            evaluation_mode: "variant-scope-must-remain-limit-only".to_owned(),
            expected_state:
                "variant_scope_label = mclaughlin-limit-only-scaffold and mclaughlin-full stays stress-only"
                    .to_owned(),
            current_state: "variant_scope_label = mclaughlin-limit-only-scaffold".to_owned(),
            status: "satisfied".to_owned(),
            summary: "Any future LP/BZ promotion stays benchmark-side only for `mclaughlin-limit`; `mclaughlin-full` remains explicitly out-of-scope as stress-only.".to_owned(),
            evidence_fields: vec![
                "instance_variant".to_owned(),
                "same_literature_variant".to_owned(),
                "benchmark_contract_roles".to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.variant_scope_label"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "promote-benchmark-cut-refinement-first".to_owned(),
            criterion_label: "Promote benchmark-side cut refinement first".to_owned(),
            target_contract: "lp-bz-sidecar-input-family".to_owned(),
            evaluation_mode: "upstream-cut-contract-must-clear-before-sidecar".to_owned(),
            expected_state:
                "benchmark_cut_contract_status = benchmark-side-implemented".to_owned(),
            current_state: format!(
                "benchmark_cut_contract_status = {}",
                benchmark_cut_contract_status,
            ),
            status: if benchmark_cut_contract_status == "benchmark-side-implemented" {
                "satisfied".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: if benchmark_cut_contract_status == "benchmark-side-implemented" {
                "The upstream benchmark-side cut layer is now versioned, so the LP/BZ sidecar may be evaluated against an explicit cut contract instead of a readiness-only scaffold.".to_owned()
            } else {
                "The sidecar input family cannot be promoted while the upstream benchmark-side cut layer is still scaffold-only.".to_owned()
            },
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites"
                    .to_owned(),
            ],
        },
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "version-mclaughlin-limit-lp-bz-sidecar".to_owned(),
            criterion_label: "Version mclaughlin-limit LP/BZ sidecar input family".to_owned(),
            target_contract: "lp-bz-sidecar-input-family".to_owned(),
            evaluation_mode: "contract-status-must-equal-benchmark-side-implemented".to_owned(),
            expected_state:
                "lp_bz_sidecar_contract_status = benchmark-side-implemented".to_owned(),
            current_state: format!(
                "lp_bz_sidecar_contract_status = {}",
                lp_bz_sidecar_contract_status,
            ),
            status: if lp_bz_sidecar_contract_status == "benchmark-side-implemented" {
                "satisfied".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: if let Some(mclaughlin_limit_lp_bz_sidecar) = mclaughlin_limit_lp_bz_sidecar {
                format!(
                    "The report now promotes `{}` from scaffold-only to benchmark-side contract step `{}`, which makes MR-206 auditable on `mclaughlin-limit`. Promotion still stays blocked because this sidecar step remains partial diagnostic evidence rather than `benchmark-side-implemented`.",
                    mclaughlin_limit_lp_bz_sidecar.sidecar_label,
                    lp_bz_sidecar_contract_status,
                )
            } else {
                "The report still has no `mclaughlin-limit` LP/BZ sidecar candidate, so promotion remains blocked even before temporal/routing gates become binding.".to_owned()
            },
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_contract_status"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.sidecar_evidence_label"
                    .to_owned(),
                "mclaughlin_limit_lp_bz_sidecar".to_owned(),
            ],
        },
        BenchmarkSidePromotionExitCriterion {
            criterion_id: "clear-lp-bz-sidecar-promotion-rule".to_owned(),
            criterion_label: "Clear LP/BZ sidecar promotion rule".to_owned(),
            target_contract: "lp-bz-sidecar-input-family".to_owned(),
            evaluation_mode: "promotion-flag-and-rule-status-must-both-clear".to_owned(),
            expected_state:
                "lp_bz_sidecar_promotion_ready = true and lp_bz_sidecar_promotion_rule.status = ready"
                    .to_owned(),
            current_state: format!(
                "lp_bz_sidecar_promotion_ready = {}, rule status = {}, blockers [{}]",
                lp_bz_sidecar_promotion_ready,
                if lp_bz_sidecar_promotion_ready {
                    "ready"
                } else {
                    "blocked"
                },
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
            ),
            status: if lp_bz_sidecar_promotion_ready {
                "satisfied".to_owned()
            } else {
                "blocked".to_owned()
            },
            summary: "This keeps the sidecar promotion exit condition machine-checkable on the benchmark-side contract itself; MR-206 remains a later gate once a real sidecar exists.".to_owned(),
            evidence_fields: vec![
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_ready"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids"
                    .to_owned(),
                "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule"
                    .to_owned(),
            ],
        },
    ];
    let benchmark_cut_promotion_rule = BenchmarkSidePromotionRule {
        rule_id: "benchmark-side-mining-cut-refinement-promotion-rule".to_owned(),
        rule_label: "Benchmark-side mining-cut refinement promotion rule".to_owned(),
        target_contract: "benchmark-side-mining-cut-refinement".to_owned(),
        evaluation_mode: "all-listed-prerequisites-must-clear".to_owned(),
        status: if benchmark_cut_promotion_ready {
            "ready".to_owned()
        } else {
            "blocked".to_owned()
        },
        required_prerequisite_ids: benchmark_cut_prerequisites
            .iter()
            .map(|prerequisite| prerequisite.prerequisite_id.clone())
            .collect(),
        blocking_prerequisite_ids: benchmark_cut_blocking_prerequisite_ids.clone(),
        summary: format!(
            "Rule `benchmark-side-mining-cut-refinement-promotion-rule` keeps `{}` on evaluation mode `all-listed-prerequisites-must-clear`: contract status `{}`, blocked prerequisites [{}], exit criteria [{}], and no promotion may occur until every blocked prerequisite disappears from the benchmark-side contract surface.",
            "benchmark-side-mining-cut-refinement",
            benchmark_cut_contract_status,
            benchmark_cut_blocking_prerequisite_ids.join(", "),
            format_scaffold_exit_criteria_statuses(&benchmark_cut_exit_criteria),
        ),
        evidence_fields: vec![
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_exit_criteria"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_ready"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_blocking_prerequisite_ids"
                .to_owned(),
        ],
    };
    let lp_bz_sidecar_promotion_rule = BenchmarkSidePromotionRule {
        rule_id: "lp-bz-sidecar-input-family-promotion-rule".to_owned(),
        rule_label: "LP/BZ sidecar input family promotion rule".to_owned(),
        target_contract: "lp-bz-sidecar-input-family".to_owned(),
        evaluation_mode:
            "all-listed-prerequisites-must-clear; temporal-routing-gate-becomes-binding-once-sidecar-exists"
                .to_owned(),
        status: if lp_bz_sidecar_promotion_ready {
            "ready".to_owned()
        } else {
            "blocked".to_owned()
        },
        required_prerequisite_ids: lp_bz_sidecar_prerequisites
            .iter()
            .map(|prerequisite| prerequisite.prerequisite_id.clone())
            .collect(),
        blocking_prerequisite_ids: lp_bz_sidecar_blocking_prerequisite_ids.clone(),
        summary: if mclaughlin_limit_lp_bz_sidecar.is_some() {
            format!(
                "Rule `lp-bz-sidecar-input-family-promotion-rule` keeps `{}` on evaluation mode `all-listed-prerequisites-must-clear; temporal-routing-gate-becomes-binding-once-sidecar-exists`: contract status `{}`, blocked prerequisites [{}], exit criteria [{}], and MR-206 is now auditable against the partial benchmark-side sidecar step while promotion remains blocked by the missing benchmark-side cut contract and the sidecar not yet reaching `benchmark-side-implemented`.",
                "lp-bz-sidecar-input-family",
                lp_bz_sidecar_contract_status,
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
            )
        } else {
            format!(
                "Rule `lp-bz-sidecar-input-family-promotion-rule` keeps `{}` on evaluation mode `all-listed-prerequisites-must-clear; temporal-routing-gate-becomes-binding-once-sidecar-exists`: contract status `{}`, blocked prerequisites [{}], exit criteria [{}], and MR-206 remains advisory until a real `mclaughlin-limit` sidecar candidate exists.",
                "lp-bz-sidecar-input-family",
                lp_bz_sidecar_contract_status,
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
            )
        },
        evidence_fields: vec![
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_contract_status"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_prerequisites"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_exit_criteria"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_ready"
                .to_owned(),
            "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids"
                .to_owned(),
            "temporal_routing_promotion_gate".to_owned(),
        ],
    };
    Some(BenchmarkSideScaffoldTraceability {
        scaffold_label: "mclaughlin-limit-cut-sidecar-scaffold".to_owned(),
        scaffold_role: "future-mining-cut-or-lp-bz-input-contract".to_owned(),
        source_unit_family_label: aggregation_strategy.to_owned(),
        scaffold_summary: if let Some(cut_contract) = mclaughlin_limit_benchmark_cut_refinement {
            if let Some(mclaughlin_limit_lp_bz_sidecar) = mclaughlin_limit_lp_bz_sidecar {
                format!(
                    "The active `{aggregation_strategy}` family now feeds implemented benchmark-side cut contract `{}` / build `{}` over `{}` localized-cut units; readiness `{}` still keeps {}/{} source shell×bench phases auditable, and LP/BZ sidecar `{}` remains at contract step `{}` with blocked LP/BZ prerequisites [{}] because it is still partial diagnostic evidence only.",
                    cut_contract.localized_cut_builder_label,
                    cut_contract.build_label,
                    cut_contract.refined_unit_family_label,
                    cut_readiness.readiness_label,
                    cut_readiness.shell_bench_phase_count,
                    cut_readiness.phase_count,
                    mclaughlin_limit_lp_bz_sidecar.sidecar_label,
                    lp_bz_sidecar_contract_status,
                    lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                )
            } else {
                format!(
                    "The active `{aggregation_strategy}` family now feeds implemented benchmark-side cut contract `{}` / build `{}` over `{}` localized-cut units; readiness `{}` remains the audited upstream bridge from {}/{} shell×bench phases, while LP/BZ support is still absent and therefore stays scaffold-only.",
                    cut_contract.localized_cut_builder_label,
                    cut_contract.build_label,
                    cut_contract.refined_unit_family_label,
                    cut_readiness.readiness_label,
                    cut_readiness.shell_bench_phase_count,
                    cut_readiness.phase_count,
                )
            }
        } else if let Some(mclaughlin_limit_lp_bz_sidecar) = mclaughlin_limit_lp_bz_sidecar {
            format!(
                "The active `{aggregation_strategy}` family is kept as a benchmark-side scaffold for future mining-cut refinement and/or LP/BZ sidecar support on `mclaughlin-limit`: readiness `{}` keeps {}/{} shell×bench phases auditable and {} multi-block refinement candidates explicit, no localized-cut builder is versioned yet, and the first LP/BZ sidecar `{}` now upgrades the LP/BZ branch from scaffold-only to contract step `{}`; blocked cut prerequisites [{}], blocked LP/BZ prerequisites [{}], cut exit criteria [{}] and LP/BZ exit criteria [{}] stay explicit because the sidecar remains partial benchmark-side evidence only.",
                cut_readiness.readiness_label,
                cut_readiness.shell_bench_phase_count,
                cut_readiness.phase_count,
                cut_readiness.refinement_candidate_phase_count,
                mclaughlin_limit_lp_bz_sidecar.sidecar_label,
                lp_bz_sidecar_contract_status,
                benchmark_cut_blocking_prerequisite_ids.join(", "),
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                format_scaffold_exit_criteria_statuses(&benchmark_cut_exit_criteria),
                format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
            )
        } else {
            format!(
                "The active `{aggregation_strategy}` family is kept as a benchmark-side scaffold for future mining-cut refinement and/or LP/BZ sidecar support on `mclaughlin-limit`: readiness `{}` keeps {}/{} shell×bench phases auditable and {} multi-block refinement candidates explicit, but no localized-cut builder or LP/BZ sidecar is versioned yet; blocked cut prerequisites [{}], blocked LP/BZ prerequisites [{}], cut exit criteria [{}] and LP/BZ exit criteria [{}] stay explicit until those benchmark-side contracts exist.",
                cut_readiness.readiness_label,
                cut_readiness.shell_bench_phase_count,
                cut_readiness.phase_count,
                cut_readiness.refinement_candidate_phase_count,
                benchmark_cut_blocking_prerequisite_ids.join(", "),
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                format_scaffold_exit_criteria_statuses(&benchmark_cut_exit_criteria),
                format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
            )
        },
        readiness_dependency_label: cut_readiness.readiness_label.clone(),
        target_contracts: vec![
            "benchmark-side-mining-cut-refinement".to_owned(),
            "lp-bz-sidecar-input-family".to_owned(),
        ],
        outstanding_gap_labels: vec![
            if mclaughlin_limit_benchmark_cut_refinement.is_some() {
                "benchmark-cut-remains-benchmark-side".to_owned()
            } else {
                "no-benchmark-cut-refinement".to_owned()
            },
            if mclaughlin_limit_lp_bz_sidecar.is_some() {
                "partial-lp-bz-bound-sidecar".to_owned()
            } else {
                "no-lp-bz-sidecar".to_owned()
            },
        ],
        benchmark_cut_contract_status: benchmark_cut_contract_status.to_owned(),
        lp_bz_sidecar_contract_status: lp_bz_sidecar_contract_status.to_owned(),
        promotion_path_summary: if let Some(cut_contract) =
            mclaughlin_limit_benchmark_cut_refinement
        {
            if mclaughlin_limit_lp_bz_sidecar.is_some() {
                format!(
                    "Future promotion remains benchmark-side only: cut contract `{}` is now `{}` with cut promotion ready = {}, LP/BZ branch remains `{}` with LP/BZ promotion ready = {}, blocked LP/BZ prerequisites [{}], LP/BZ exit criteria [{}], and `mclaughlin-full` remains explicitly out-of-scope as stress-only.",
                    cut_contract.contract_label,
                    benchmark_cut_contract_status,
                    benchmark_cut_promotion_ready,
                    lp_bz_sidecar_contract_status,
                    lp_bz_sidecar_promotion_ready,
                    lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                    format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
                )
            } else {
                format!(
                    "Future promotion remains benchmark-side only: cut contract `{}` is now `{}` with cut promotion ready = {}, while LP/BZ support is still scaffold-only with remaining blocked prerequisites [{}]; `mclaughlin-full` remains explicitly out-of-scope as stress-only.",
                    cut_contract.contract_label,
                    benchmark_cut_contract_status,
                    benchmark_cut_promotion_ready,
                    lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                )
            }
        } else if mclaughlin_limit_lp_bz_sidecar.is_some() {
            format!(
                "Future promotion remains benchmark-side only: `{}` is the audited readiness gate, `{}` must land before `{}` can move beyond contract step `{}`, cut promotion ready = {}, LP/BZ promotion ready = {}, remaining blocked cut prerequisites [{}], remaining blocked LP/BZ prerequisites [{}], cut exit criteria [{}], LP/BZ exit criteria [{}], MR-206 is now auditable but still partial/diagnostic-only, and `mclaughlin-full` remains explicitly out-of-scope as stress-only.",
                cut_readiness.readiness_label,
                "benchmark-side-mining-cut-refinement",
                "lp-bz-sidecar-input-family",
                lp_bz_sidecar_contract_status,
                benchmark_cut_promotion_ready,
                lp_bz_sidecar_promotion_ready,
                benchmark_cut_blocking_prerequisite_ids.join(", "),
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                format_scaffold_exit_criteria_statuses(&benchmark_cut_exit_criteria),
                format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
            )
        } else {
            format!(
                "Future promotion remains benchmark-side only: `{}` is the audited readiness gate, `{}` must land before `{}` becomes meaningful, cut promotion ready = {}, LP/BZ promotion ready = {}, remaining blocked cut prerequisites [{}], remaining blocked LP/BZ prerequisites [{}], cut exit criteria [{}], LP/BZ exit criteria [{}], both target contracts stay scaffold-only until that implementation work is versioned, and `mclaughlin-full` remains explicitly out-of-scope as stress-only.",
                cut_readiness.readiness_label,
                "benchmark-side-mining-cut-refinement",
                "lp-bz-sidecar-input-family",
                benchmark_cut_promotion_ready,
                lp_bz_sidecar_promotion_ready,
                benchmark_cut_blocking_prerequisite_ids.join(", "),
                lp_bz_sidecar_blocking_prerequisite_ids.join(", "),
                format_scaffold_exit_criteria_statuses(&benchmark_cut_exit_criteria),
                format_scaffold_exit_criteria_statuses(&lp_bz_sidecar_exit_criteria),
            )
        },
        benchmark_cut_promotion_ready,
        benchmark_cut_blocking_prerequisite_ids,
        lp_bz_sidecar_promotion_ready,
        lp_bz_sidecar_blocking_prerequisite_ids,
        benchmark_cut_promotion_rule,
        lp_bz_sidecar_promotion_rule,
        refinement_candidate_phase_count: cut_readiness.refinement_candidate_phase_count,
        variant_scope_label: "mclaughlin-limit-only-scaffold".to_owned(),
        variant_scope_summary,
        benchmark_cut_exit_criteria,
        lp_bz_sidecar_exit_criteria,
        benchmark_cut_prerequisites,
        lp_bz_sidecar_prerequisites,
    })
}

fn format_scaffold_prerequisite_statuses(
    prerequisites: &[BenchmarkSidePromotionPrerequisite],
) -> String {
    prerequisites
        .iter()
        .map(|prerequisite| format!("{}={}", prerequisite.prerequisite_id, prerequisite.status))
        .collect::<Vec<_>>()
        .join(", ")
}

fn blocking_scaffold_prerequisite_ids(
    prerequisites: &[BenchmarkSidePromotionPrerequisite],
) -> Vec<String> {
    prerequisites
        .iter()
        .filter(|prerequisite| prerequisite.status == "blocked")
        .map(|prerequisite| prerequisite.prerequisite_id.clone())
        .collect()
}

fn format_scaffold_exit_criteria_statuses(
    criteria: &[BenchmarkSidePromotionExitCriterion],
) -> String {
    criteria
        .iter()
        .map(|criterion| format!("{}={}", criterion.criterion_id, criterion.status))
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_marvin_paperlike_pipeline_checklist(
    preferred_nested_shell_family_contract: &MarvinPreferredNestedShellFamilyContract,
    lp_bz_baseline: &LpBzBaselineSummary,
    comparison_classification: &str,
    comparability_gaps: &[String],
) -> MarvinPaperlikePipelineChecklist {
    let shell_summary = match preferred_nested_shell_family_contract.realized_shell_count {
        Some(realized_shell_count) => format!(
            "Nested-shell × bench primary routing stays on the bounded {}-factor {} family with {realized_shell_count} realized shells.",
            preferred_nested_shell_family_contract.revenue_factor_count,
            preferred_nested_shell_family_contract
                .shell_access_mode
                .label(),
        ),
        None => format!(
            "Nested-shell × bench primary routing stays on the bounded {}-factor {} family.",
            preferred_nested_shell_family_contract.revenue_factor_count,
            preferred_nested_shell_family_contract
                .shell_access_mode
                .label(),
        ),
    };
    let readiness = &lp_bz_baseline.phase_refinement_diagnostics;
    let readiness_summary = format!(
        "Localized-cut diagnostics refined {}/{} shell×bench phases ({} single-component cases), max_cut_count_per_base_phase={} and {} readiness buckets.",
        readiness.refined_base_phase_count,
        readiness.base_phase_count,
        readiness.refined_single_component_phase_count,
        readiness.max_cut_count_per_base_phase,
        readiness.readiness_reason_histogram.len(),
    );
    let access_law = &lp_bz_baseline.cut_access_law;
    let unit_family_traceability = &lp_bz_baseline.unit_family_traceability;
    let lp_kernel = &lp_bz_baseline.summary.lp_bz_lp_kernel;
    let lp_solve = &lp_bz_baseline.summary.lp_bz_lp_solve;
    let runtime_budget_contract = &lp_bz_baseline
        .summary
        .lp_bz_round_repair
        .local_optimizer_runtime_budget_contract;
    let promoted_family_summary = format_promoted_pushback_bench_localized_cut_family_summary(
        unit_family_traceability,
        &lp_bz_baseline.unit_granularity_label,
        &lp_bz_baseline.promoted_build_label,
        &lp_bz_baseline.local_optimizer_scaffold_unit_family_label,
        access_law,
    );
    let bibliographic_gap_ids = access_law
        .bibliographic_gap_contract
        .iter()
        .map(|gap| gap.gap_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let bibliographic_gap_summary = format_promoted_lp_bz_bibliographic_gap_summary(
        "lp_bz_baseline.cut_access_law.bibliographic_gap_contract",
        access_law.bibliographic_gap_contract.len(),
        &bibliographic_gap_ids,
    );
    let input_aggregation_traceability_summary =
        format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary(
            unit_family_traceability,
            lp_bz_baseline
                .phase_refinement_diagnostics
                .total_cut_phase_count,
            lp_bz_baseline
                .summary
                .lp_bz_inputs
                .precedence_units
                .unit_count,
        );
    let mut input_aggregation_traceability_fields =
        marvin_sidecar_traceability_field_paths("lp_bz_baseline.unit_family_traceability");
    input_aggregation_traceability_fields
        .push("lp_bz_baseline.phase_refinement_diagnostics.total_cut_phase_count".to_owned());
    input_aggregation_traceability_fields
        .push("lp_bz_baseline.summary.lp_bz_inputs.precedence_units.unit_count".to_owned());
    input_aggregation_traceability_fields.push("preferred_nested_shell_family_contract".to_owned());
    let mut promoted_family_evidence_fields =
        marvin_sidecar_traceability_field_paths("lp_bz_baseline.unit_family_traceability");
    promoted_family_evidence_fields.extend([
        "lp_bz_baseline.unit_granularity_label".to_owned(),
        "lp_bz_baseline.promoted_build_label".to_owned(),
        "lp_bz_baseline.local_optimizer_scaffold_unit_family_label".to_owned(),
        "lp_bz_baseline.cut_access_law".to_owned(),
        "lp_bz_baseline.lp_bz_promotion_readiness".to_owned(),
        "lp_bz_baseline.lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract"
            .to_owned(),
    ]);
    let runtime_contract_summary = format!(
        "Kernel `{}` reports native LP solve status `{}` with {} for the promoted LP/BZ route, and the explicit local-optimizer runtime contract stays `{}` with `budget_hit={}`. {}",
        lp_kernel.kernel_label,
        lp_bz_lp_solve_status_label(lp_solve.solve_status),
        lp_bz_precedence_runtime_summary(&lp_solve.precedence_diagnostics),
        runtime_budget_contract.execution_state,
        runtime_budget_contract.budget_hit,
        runtime_budget_contract.summary,
    );
    let promotion_gate_summary = format!(
        "MR-206 temporal/routing gate `{}` stays `{}` (ΔNPV = {:.3}); thresholds require |Δ used_period_count| <= {}, mean_absolute_period_delta <= {:.1}, earlier_than_reference_count <= {} and (period,destination) similarity >= {:.2}. {}",
        lp_bz_baseline.temporal_routing_promotion_gate.gate_version,
        lp_bz_baseline
            .temporal_routing_promotion_gate
            .promotion_decision,
        lp_bz_baseline
            .temporal_routing_promotion_gate
            .npv_delta_vs_reference,
        lp_bz_baseline
            .temporal_routing_promotion_gate
            .thresholds
            .max_used_period_count_delta,
        lp_bz_baseline
            .temporal_routing_promotion_gate
            .thresholds
            .max_mean_absolute_period_delta,
        lp_bz_baseline
            .temporal_routing_promotion_gate
            .thresholds
            .max_earlier_than_reference_count,
        lp_bz_baseline
            .temporal_routing_promotion_gate
            .thresholds
            .min_period_destination_similarity,
        lp_bz_baseline.temporal_routing_promotion_gate.summary,
    );
    let classification_summary = format!(
        "The Marvin dataset stays `{comparison_classification}` with {} explicit comparability gaps, so the paper-like path remains audited benchmark evidence rather than a silent literature-grade promotion. At the current contract level the benchmark-side maturity is intentionally treated as exhausted: moving past this point requires a shared/core-side or protocol-level mining-cut input contract, not just another local benchmark heuristic.",
        comparability_gaps.len(),
    );
    MarvinPaperlikePipelineChecklist {
        pipeline_label: "marvin-paperlike-pipeline".to_owned(),
        checklist_version: "mr189-v2".to_owned(),
        items: vec![
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "preferred-shell-family-contract".to_owned(),
                contract_label: "Preferred nested-shell family".to_owned(),
                status: "audited".to_owned(),
                summary: shell_summary,
                evidence_fields: vec!["preferred_nested_shell_family_contract".to_owned()],
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "input-aggregation-traceability".to_owned(),
                contract_label: "Input/aggregation traceability".to_owned(),
                status: "audited".to_owned(),
                summary: input_aggregation_traceability_summary,
                evidence_fields: input_aggregation_traceability_fields,
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "localized-cut-readiness-diagnostics".to_owned(),
                contract_label: "Localized-cut readiness diagnostics".to_owned(),
                status: "audited".to_owned(),
                summary: readiness_summary,
                evidence_fields: vec!["lp_bz_baseline.phase_refinement_diagnostics".to_owned()],
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "promoted-paperlike-lp-bz-family".to_owned(),
                contract_label: "Promoted paper-like LP/BZ family".to_owned(),
                status: "audited".to_owned(),
                summary: promoted_family_summary,
                evidence_fields: promoted_family_evidence_fields,
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "promoted-lp-bz-runtime-solve-path".to_owned(),
                contract_label: "Promoted LP/BZ runtime / solve path".to_owned(),
                status: "audited".to_owned(),
                summary: runtime_contract_summary,
                evidence_fields: marvin_sidecar_runtime_contract_field_paths("lp_bz_baseline"),
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "temporal-routing-promotion-gate".to_owned(),
                contract_label: "Temporal/routing promotion gate".to_owned(),
                status: "audited".to_owned(),
                summary: promotion_gate_summary,
                evidence_fields: marvin_sidecar_promotion_gate_field_paths("lp_bz_baseline"),
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "localized-cut-bibliographic-gap-contract".to_owned(),
                contract_label: "Localized-cut bibliographic gap contract".to_owned(),
                status: "audited".to_owned(),
                summary: bibliographic_gap_summary,
                evidence_fields: vec![
                    "lp_bz_baseline.cut_access_law.bibliographic_gap_contract".to_owned(),
                    "lp_bz_baseline.cut_access_law.ramp_access_contract".to_owned(),
                    "lp_bz_baseline.cut_access_law.working_width_contract".to_owned(),
                    "lp_bz_baseline.cut_access_law.lineage_bench_continuity_contract".to_owned(),
                    "lp_bz_baseline.cut_access_law.complete_cut_design_contract".to_owned(),
                    "lp_bz_baseline.cut_access_law.missing_bibliographic_terms".to_owned(),
                ],
            },
            MarvinPaperlikePipelineChecklistItem {
                contract_id: "comparability-classification".to_owned(),
                contract_label: "Comparability classification".to_owned(),
                status: "audited".to_owned(),
                summary: classification_summary,
                evidence_fields: vec![
                    "comparison_classification".to_owned(),
                    "comparability_gaps".to_owned(),
                ],
            },
        ],
    }
}

fn build_mclaughlin_limit_promotion_checklist(
    primary_unit_family_traceability: &PrimaryUnitFamilyTraceability,
    comparison_classification: &str,
    comparability_gaps: &[String],
    temporal_routing_promotion_gate: &TemporalRoutingPromotionGateSummary,
) -> MclaughlinLimitPromotionChecklist {
    let cut_readiness = primary_unit_family_traceability
        .benchmark_side_evidence
        .cut_readiness
        .as_ref();
    let future_scaffold = primary_unit_family_traceability
        .benchmark_side_evidence
        .future_scaffold
        .as_ref();
    let cut_readiness_summary = cut_readiness.map_or_else(
        || {
            "No pushback-equivalent bench-cut readiness traceability is published, so promotion cannot proceed beyond the current proxy family.".to_owned()
        },
        |cut_readiness| {
            format!(
                "Readiness `{}` keeps {}/{} shell×bench phases auditable, {} phases with predecessor lineage, and {} multi-block refinement candidates ready for any future benchmark-side mining-cut builder.",
                cut_readiness.readiness_label,
                cut_readiness.shell_bench_phase_count,
                cut_readiness.phase_count,
                cut_readiness.predecessor_traced_phase_count,
                cut_readiness.refinement_candidate_phase_count,
            )
        },
    );
    let benchmark_cut_scaffold_summary = future_scaffold.map_or_else(
        || {
            "No benchmark-side scaffold is published for mclaughlin-limit cut refinement yet.".to_owned()
        },
        |future_scaffold| {
            format!(
                "Scaffold `{}` keeps target contract `{}` benchmark-side only with status `{}` while outstanding gaps stay [{}], cut promotion ready = {}, blocked cut prerequisites are [{}], cut prerequisites remain [{}], cut exit criteria remain [{}], and rule `{}` stays `{}` under mode `{}`.",
                future_scaffold.scaffold_label,
                "benchmark-side-mining-cut-refinement",
                future_scaffold.benchmark_cut_contract_status,
                future_scaffold.outstanding_gap_labels.join(", "),
                future_scaffold.benchmark_cut_promotion_ready,
                future_scaffold
                    .benchmark_cut_blocking_prerequisite_ids
                    .join(", "),
                format_scaffold_prerequisite_statuses(&future_scaffold.benchmark_cut_prerequisites),
                format_scaffold_exit_criteria_statuses(&future_scaffold.benchmark_cut_exit_criteria),
                future_scaffold.benchmark_cut_promotion_rule.rule_id,
                future_scaffold.benchmark_cut_promotion_rule.status,
                future_scaffold.benchmark_cut_promotion_rule.evaluation_mode,
            )
        },
    );
    let lp_bz_scaffold_summary = future_scaffold.map_or_else(
        || {
            "No LP/BZ sidecar scaffold is published for mclaughlin-limit yet.".to_owned()
        },
        |future_scaffold| {
            if future_scaffold.lp_bz_sidecar_contract_status == "partial-bound-available"
            {
                format!(
                    "The same scaffold `{}` keeps `{}` at status `{}` with sidecar promotion ready = {}, blocked LP/BZ prerequisites [{}], sidecar prerequisites [{}], sidecar exit criteria [{}], and rule `{}` staying `{}` under mode `{}`; MR-206 gate `{}` is now auditable (`{}`), and the contract has advanced beyond scaffold-only, but promotion still cannot overclaim comparability because benchmark-side cut refinement is still missing and the current sidecar is only partial diagnostic evidence on `mclaughlin-limit`.",
                    future_scaffold.scaffold_label,
                    "lp-bz-sidecar-input-family",
                    future_scaffold.lp_bz_sidecar_contract_status,
                    future_scaffold.lp_bz_sidecar_promotion_ready,
                    future_scaffold
                        .lp_bz_sidecar_blocking_prerequisite_ids
                        .join(", "),
                    format_scaffold_prerequisite_statuses(
                        &future_scaffold.lp_bz_sidecar_prerequisites
                    ),
                    format_scaffold_exit_criteria_statuses(
                        &future_scaffold.lp_bz_sidecar_exit_criteria
                    ),
                    future_scaffold.lp_bz_sidecar_promotion_rule.rule_id,
                    future_scaffold.lp_bz_sidecar_promotion_rule.status,
                    future_scaffold.lp_bz_sidecar_promotion_rule.evaluation_mode,
                    temporal_routing_promotion_gate.gate_version,
                    temporal_routing_promotion_gate.promotion_decision,
                )
            } else {
                format!(
                    "The same scaffold `{}` keeps `{}` at status `{}` with sidecar promotion ready = {}, blocked LP/BZ prerequisites [{}], sidecar prerequisites [{}], sidecar exit criteria [{}], and rule `{}` staying `{}` under mode `{}`; MR-206 gate `{}` remains `{}` today, but that gate is still only advisory until an actual LP/BZ-side candidate exists on `mclaughlin-limit`.",
                    future_scaffold.scaffold_label,
                    "lp-bz-sidecar-input-family",
                    future_scaffold.lp_bz_sidecar_contract_status,
                    future_scaffold.lp_bz_sidecar_promotion_ready,
                    future_scaffold
                        .lp_bz_sidecar_blocking_prerequisite_ids
                        .join(", "),
                    format_scaffold_prerequisite_statuses(
                        &future_scaffold.lp_bz_sidecar_prerequisites
                    ),
                    format_scaffold_exit_criteria_statuses(
                        &future_scaffold.lp_bz_sidecar_exit_criteria
                    ),
                    future_scaffold.lp_bz_sidecar_promotion_rule.rule_id,
                    future_scaffold.lp_bz_sidecar_promotion_rule.status,
                    future_scaffold.lp_bz_sidecar_promotion_rule.evaluation_mode,
                    temporal_routing_promotion_gate.gate_version,
                    temporal_routing_promotion_gate.promotion_decision,
                )
            }
        },
    );
    let input_traceability_summary = if primary_unit_family_traceability
        .benchmark_side_evidence
        .benchmark_cut_refinement
        .is_some()
    {
        format!(
            "`selected_block_source = \"{}\"` keeps {} selected blocks on literature-target variant `{}` and lifts them through {} phase-plan proxy units into {} scheduling units before the report-specific benchmark-side cut contract and LP/BZ sidecar layers are evaluated.",
            primary_unit_family_traceability
                .selected_block_provenance
                .selected_block_source,
            primary_unit_family_traceability
                .selected_block_provenance
                .selected_block_count,
            primary_unit_family_traceability.literature_alignment_label,
            primary_unit_family_traceability
                .preferred_phase_plan_proxy
                .preferred_phase_count,
            primary_unit_family_traceability.scheduling_unit_count,
        )
    } else {
        format!(
            "`selected_block_source = \"{}\"` keeps {} selected blocks on literature-target variant `{}` and lifts them through {} phase-plan proxy units into {} scheduling units before any mining-cut refinement or LP/BZ sidecar exists.",
            primary_unit_family_traceability
                .selected_block_provenance
                .selected_block_source,
            primary_unit_family_traceability
                .selected_block_provenance
                .selected_block_count,
            primary_unit_family_traceability.literature_alignment_label,
            primary_unit_family_traceability
                .preferred_phase_plan_proxy
                .preferred_phase_count,
            primary_unit_family_traceability.scheduling_unit_count,
        )
    };
    let classification_summary = format!(
        "`mclaughlin-limit` stays `{comparison_classification}` with {} explicit comparability gaps; promotion remains blocked until benchmark-side cut refinement and/or LP/BZ sidecar work replaces labels `{}` / `{}` with versioned support.",
        comparability_gaps.len(),
        primary_unit_family_traceability
            .benchmark_side_evidence
            .cut_evidence_label,
        primary_unit_family_traceability
            .benchmark_side_evidence
            .sidecar_evidence_label,
    );
    MclaughlinLimitPromotionChecklist {
        checklist_label: "mclaughlin-limit-promotion-path".to_owned(),
        checklist_version: MCLAUGHLIN_LIMIT_PROMOTION_CHECKLIST_VERSION.to_owned(),
        items: vec![
            MclaughlinLimitPromotionChecklistItem {
                contract_id: "pushback-equivalent-input-traceability".to_owned(),
                contract_label: "Pushback-equivalent input traceability".to_owned(),
                status: "audited".to_owned(),
                summary: input_traceability_summary,
                evidence_fields: vec![
                    "selected_block_source".to_owned(),
                    "selected_block_provenance_summary".to_owned(),
                    "selected_block_provenance_chain".to_owned(),
                    "primary_unit_family_traceability.selected_block_provenance.selected_block_source"
                        .to_owned(),
                    "primary_unit_family_traceability.selected_block_provenance.selected_block_count"
                        .to_owned(),
                    "primary_unit_family_traceability.preferred_phase_plan_proxy.preferred_phase_count"
                        .to_owned(),
                    "primary_unit_family_traceability.scheduling_unit_count".to_owned(),
                ],
            },
            MclaughlinLimitPromotionChecklistItem {
                contract_id: "pushback-equivalent-bench-cut-readiness".to_owned(),
                contract_label: "Pushback-equivalent bench-cut readiness".to_owned(),
                status: "audited".to_owned(),
                summary: cut_readiness_summary,
                evidence_fields: vec![
                    "primary_unit_family_traceability.benchmark_side_evidence.cut_evidence_label"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.cut_readiness"
                        .to_owned(),
                ],
            },
            MclaughlinLimitPromotionChecklistItem {
                contract_id: "benchmark-side-mining-cut-refinement".to_owned(),
                contract_label: "Benchmark-side mining-cut refinement".to_owned(),
                status: future_scaffold
                    .map(|future_scaffold| future_scaffold.benchmark_cut_contract_status.clone())
                    .unwrap_or_else(|| "scaffold-only".to_owned()),
                summary: benchmark_cut_scaffold_summary,
                evidence_fields: vec![
                    "primary_unit_family_traceability.benchmark_side_evidence.benchmark_cut_refinement"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.scaffold_label"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.target_contracts"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_ready"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_blocking_prerequisite_ids"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_exit_criteria"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.outstanding_gap_labels"
                        .to_owned(),
                ],
            },
            MclaughlinLimitPromotionChecklistItem {
                contract_id: "lp-bz-sidecar-input-family".to_owned(),
                contract_label: "Future LP/BZ sidecar input family".to_owned(),
                status: if future_scaffold
                    .map(|future_scaffold| {
                        future_scaffold.lp_bz_sidecar_contract_status
                            == "partial-bound-available"
                    })
                    .unwrap_or(false)
                {
                    "partial-bound-available".to_owned()
                } else {
                    "scaffold-only".to_owned()
                },
                summary: lp_bz_scaffold_summary,
                evidence_fields: vec![
                    "mclaughlin_limit_lp_bz_sidecar".to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.sidecar_evidence_label"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.scaffold_label"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_contract_status"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_ready"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_exit_criteria"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_prerequisites"
                        .to_owned(),
                    "temporal_routing_promotion_gate".to_owned(),
                ],
            },
            MclaughlinLimitPromotionChecklistItem {
                contract_id: "stress-only-full-variant-separation".to_owned(),
                contract_label: "Stress-only full variant separation".to_owned(),
                status: "audited".to_owned(),
                summary: "`mclaughlin-full` remains outside this promotion path: the checklist applies only to the literature-target `mclaughlin-limit` route, while the full dataset stays an explicit stress-only local variant.".to_owned(),
                evidence_fields: vec![
                    "instance_variant".to_owned(),
                    "literature_reference_instance".to_owned(),
                    "same_literature_variant".to_owned(),
                    "benchmark_contract_roles".to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.variant_scope_label"
                        .to_owned(),
                    "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.variant_scope_summary"
                        .to_owned(),
                ],
            },
            MclaughlinLimitPromotionChecklistItem {
                contract_id: "comparability-classification".to_owned(),
                contract_label: "Comparability classification".to_owned(),
                status: "blocked".to_owned(),
                summary: classification_summary,
                evidence_fields: vec![
                    "comparison_classification".to_owned(),
                    "comparability_gap_contract".to_owned(),
                    "comparability_gaps".to_owned(),
                ],
            },
        ],
    }
}

fn build_dataset_diagnostic_groups(
    has_nested_shell_bench_probe: bool,
    has_lp_bz_baseline: bool,
    has_marvin_paperlike_pipeline_checklist: bool,
    has_mclaughlin_limit_promotion_checklist: bool,
) -> Vec<String> {
    let mut groups = vec![
        "comparability".to_owned(),
        "objective-and-schedule-summary".to_owned(),
        "temporal-and-membership-alignment".to_owned(),
        "relaxations-and-sidecars".to_owned(),
    ];
    if has_nested_shell_bench_probe || has_lp_bz_baseline {
        groups.push("phase-plan-and-access-law".to_owned());
    }
    if has_marvin_paperlike_pipeline_checklist {
        groups.push("paperlike-pipeline-checklist".to_owned());
    }
    if has_mclaughlin_limit_promotion_checklist {
        groups.push("mclaughlin-limit-promotion-checklist".to_owned());
    }
    groups
}

#[cfg(test)]
struct MineRsEndToEndArtifacts {
    phase_plan: mine_sdk::PushbackPlan,
}

#[cfg(test)]
fn build_mine_rs_end_to_end_artifacts(
    model: &mine_sdk::BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    _marvin_problem: &MinelibScheduleProblem,
) -> Result<MineRsEndToEndArtifacts, mine_sdk::MineError> {
    let preferred_shell_family =
        build_marvin_preferred_nested_shell_family_contract(NESTED_SHELL_PROBE_FACTOR_COUNT)?;
    let phase_plan = build_marvin_phase_plan_from_revenue_factor_shells(
        model,
        precedence_graph,
        &preferred_shell_family.revenue_factors,
        preferred_shell_family.shell_access_mode.nesting_rules(),
        "Marvin test shell×bench phase plan uses the bounded benchmark-side nested-shell builder.",
    )?
    .phase_plan;
    Ok(MineRsEndToEndArtifacts { phase_plan })
}

#[cfg(test)]
fn build_phase_scheduling_problem_from_marvin_problem(
    _model: &mine_sdk::BlockModel,
    phase_plan: &mine_sdk::PushbackPlan,
    marvin_problem: &MinelibScheduleProblem,
) -> Result<mine_sdk::SchedulingProblem, mine_sdk::MineError> {
    let resource_roles = BTreeMap::from([
        (0usize, MinelibResourceRole::MineTonnage),
        (1usize, MinelibResourceRole::PlantTonnage),
    ]);
    build_scheduling_problem_from_minelib_problem(
        phase_plan,
        marvin_problem,
        "marvin",
        &resource_roles,
        "Marvin test scheduling problem keeps the benchmark-side shell×bench aggregation.",
    )
}

fn solution_metric_map(summary: &MinelibScheduleSolutionSummary) -> BTreeMap<String, f64> {
    BTreeMap::from([
        (
            "discounted_objective".to_owned(),
            summary.discounted_objective,
        ),
        (
            "assignment_count".to_owned(),
            summary.assignment_count as f64,
        ),
        (
            "unique_block_count".to_owned(),
            summary.unique_block_count as f64,
        ),
        (
            "used_period_count".to_owned(),
            summary.used_period_count as f64,
        ),
        (
            "used_destination_count".to_owned(),
            summary.used_destination_count as f64,
        ),
    ])
}

fn build_period_destination_memberships(
    solution: &MinelibScheduleSolution,
) -> BTreeMap<String, BTreeSet<usize>> {
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();
    for assignment in &solution.assignments {
        memberships
            .entry(format!(
                "P{:02}.D{:02}",
                assignment.period_index + 1,
                assignment.destination_index
            ))
            .or_default()
            .insert(assignment.linear_index);
    }
    memberships
}

fn build_period_memberships(
    solution: &MinelibScheduleSolution,
) -> BTreeMap<String, BTreeSet<usize>> {
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();
    for assignment in &solution.assignments {
        memberships
            .entry(format!("P{:02}", assignment.period_index + 1))
            .or_default()
            .insert(assignment.linear_index);
    }
    memberships
}

fn build_relaxation_reference_summaries(
    model: &mine_sdk::BlockModel,
    cpit_problem: &MinelibScheduleProblem,
    cpit_problem_path: &Path,
    pcpsp_problem: &MinelibScheduleProblem,
    pcpsp_problem_path: &Path,
    references_dir: &Path,
    config: &DatasetConfig,
) -> Result<Vec<RelaxationReferenceSummary>, mine_sdk::MineError> {
    let mut summaries = Vec::new();

    if let Some(lp_cpit_solution_file) = config.lp_cpit_solution_file {
        let solution_path = references_dir.join(lp_cpit_solution_file);
        let solution = read_minelib_lp_cpit_solution(&solution_path, model)?;
        summaries.push(RelaxationReferenceSummary {
            label: "lp-cpit".to_owned(),
            problem_kind: "CPIT".to_owned(),
            problem_path: cpit_problem_path.display().to_string(),
            solution_path: solution_path.display().to_string(),
            directly_comparable_to_pcpsp: false,
            summary: summarize_minelib_schedule_solution(cpit_problem, &solution)?,
        });
    }

    if let Some(lp_pcpsp_solution_file) = config.lp_pcpsp_solution_file {
        let solution_path = references_dir.join(lp_pcpsp_solution_file);
        let solution = read_minelib_lp_pcpsp_solution(&solution_path, model)?;
        summaries.push(RelaxationReferenceSummary {
            label: "lp-pcpsp".to_owned(),
            problem_kind: "PCPSP".to_owned(),
            problem_path: pcpsp_problem_path.display().to_string(),
            solution_path: solution_path.display().to_string(),
            directly_comparable_to_pcpsp: true,
            summary: summarize_minelib_schedule_solution(pcpsp_problem, &solution)?,
        });
    }

    Ok(summaries)
}

#[allow(clippy::too_many_arguments)]
fn build_localized_cut_sidecar_artifacts(
    config: &DatasetConfig,
    model: &mine_sdk::BlockModel,
    base_phase_plan: &mine_sdk::PushbackPlan,
    pcpsp_problem: &MinelibScheduleProblem,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    tonnage_column: &ColumnId,
    build_config: PushbackBenchLocalizedCutBuildConfig,
    limitation_note: &str,
) -> Result<PushbackBenchLocalizedCutBuildArtifacts<mine_sdk::SchedulingProblem>, mine_sdk::MineError>
{
    let tonnage_by_linear_index = build_linear_index_float_lookup(model, tonnage_column)?;
    build_pushback_bench_localized_cut_benchmark_artifacts(
        model,
        base_phase_plan,
        &tonnage_by_linear_index,
        build_config,
        |phase_plan| {
            build_scheduling_problem_from_minelib_problem(
                phase_plan,
                pcpsp_problem,
                config.dataset_id,
                resource_roles,
                limitation_note,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn build_marvin_lp_bz_sidecar_artifacts(
    config: &DatasetConfig,
    model: &mine_sdk::BlockModel,
    base_phase_plan: &mine_sdk::PushbackPlan,
    pcpsp_problem: &MinelibScheduleProblem,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    tonnage_column: &ColumnId,
) -> Result<PushbackBenchLocalizedCutBuildArtifacts<mine_sdk::SchedulingProblem>, mine_sdk::MineError>
{
    build_localized_cut_sidecar_artifacts(
        config,
        model,
        base_phase_plan,
        pcpsp_problem,
        resource_roles,
        tonnage_column,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        &lp_bz_cut_scheduling_limitation_note(),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_mclaughlin_limit_benchmark_cut_refinement(
    config: &DatasetConfig,
    aggregation_strategy: &str,
    model: &mine_sdk::BlockModel,
    base_phase_plan: &mine_sdk::PushbackPlan,
    pcpsp_problem: &MinelibScheduleProblem,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    tonnage_column: &ColumnId,
) -> Result<Option<MclaughlinLimitBenchmarkCutRefinementSummary>, mine_sdk::MineError> {
    if config.dataset_id != "mclaughlin-limit"
        || aggregation_strategy != "nested-shell-bench"
        || !config.same_literature_variant
    {
        return Ok(None);
    }

    let cut_artifacts = build_localized_cut_sidecar_artifacts(
        config,
        model,
        base_phase_plan,
        pcpsp_problem,
        resource_roles,
        tonnage_column,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG,
        &mclaughlin_limit_benchmark_cut_limitation_note(),
    )?;
    let diagnostics = cut_artifacts.phase_refinement_diagnostics;
    let scheduling_unit_count = cut_artifacts.benchmark.scheduling_problem.units().len();
    let disclosure_summary = format!(
        "`mclaughlin-limit` now versions benchmark-side mining-cut refinement `{}` / build `{}` on top of `{}`: {} base shell×bench phases refine into {} localized-cut phases (+{}), producing {} scheduling units under `max_front_count = {}`, `min_aspect_ratio = {:.1}`, `min_dominant_span = {}`, `max_local_predecessor_count = {:?}`, predecessor-link policy `{}` and front progression `{}`. This remains a limit-only benchmark-side contract over proxy shell×bench inputs rather than a literature-grade mining-cut generator.",
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILDER_LABEL,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_LABEL,
        aggregation_strategy,
        diagnostics.base_phase_count,
        diagnostics.total_cut_phase_count,
        diagnostics.additional_phase_count,
        scheduling_unit_count,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.max_front_count,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.min_aspect_ratio,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.min_dominant_span,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.max_local_predecessor_count,
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG
            .predecessor_cut_link_policy
            .label(),
        MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG
            .front_progression
            .label(),
    );

    Ok(Some(MclaughlinLimitBenchmarkCutRefinementSummary {
        contract_label: "benchmark-side-mining-cut-refinement".to_owned(),
        contract_version: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_CONTRACT_VERSION.to_owned(),
        contract_status: "benchmark-side-implemented".to_owned(),
        scope_label: "mclaughlin-limit-only".to_owned(),
        source_unit_family_label: aggregation_strategy.to_owned(),
        refined_unit_family_label: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_UNIT_FAMILY_LABEL.to_owned(),
        localized_cut_builder_label: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILDER_LABEL.to_owned(),
        build_label: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_LABEL.to_owned(),
        scheduling_unit_count,
        build_config_summary: MclaughlinLimitBenchmarkCutBuildConfigSummary {
            max_front_count: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.max_front_count,
            min_aspect_ratio: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.min_aspect_ratio,
            min_dominant_span: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.min_dominant_span,
            include_touching_neighbors: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.include_touching_neighbors,
            max_local_predecessor_count: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG.max_local_predecessor_count,
            predecessor_cut_link_policy: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG
                .predecessor_cut_link_policy
                .label()
                .to_owned(),
            front_progression_label: MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_CONFIG
                .front_progression
                .label()
                .to_owned(),
        },
        phase_refinement_diagnostics: diagnostics,
        disclosure_summary,
        limitations: vec![
            mclaughlin_limit_benchmark_cut_limitation_note(),
            "The cut contract is benchmark-side only: it improves the auditable cut layer, but comparability remains exploratory-local until the upstream input/protocol gap and the LP/BZ sidecar maturity gap both close.".to_owned(),
        ],
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_lp_bz_baseline(
    repo_root: &Path,
    references_dir: &Path,
    config: &DatasetConfig,
    model: &mine_sdk::BlockModel,
    base_phase_plan: &mine_sdk::PushbackPlan,
    pcpsp_problem: &MinelibScheduleProblem,
    pcpsp_solution: &MinelibScheduleSolution,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    linear_index_to_row_index: &BTreeMap<usize, usize>,
    tonnage_column: &ColumnId,
    ready_frontier_discounted_objective: f64,
    selected_block_count: usize,
    selected_block_source: &str,
    preferred_phase_plan_aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
) -> Result<Option<LpBzBaselineSummary>, mine_sdk::MineError> {
    if !supports_lp_bz_baseline(config) {
        return Ok(None);
    }
    let Some(lp_pcpsp_solution_file) = config.lp_pcpsp_solution_file else {
        return Ok(None);
    };

    let lp_pcpsp_solution_path = references_dir.join(lp_pcpsp_solution_file);
    let lp_pcpsp_solution = read_minelib_lp_pcpsp_solution(&lp_pcpsp_solution_path, model)?;
    let cut_artifacts = build_marvin_lp_bz_sidecar_artifacts(
        config,
        model,
        base_phase_plan,
        pcpsp_problem,
        resource_roles,
        tonnage_column,
    )?;
    let result = run_marvin_focused_lp_bz_adapter(
        &cut_artifacts.benchmark.phase_plan,
        &cut_artifacts.benchmark.scheduling_problem,
        pcpsp_problem,
        &lp_pcpsp_solution,
        &lp_pcpsp_solution_path,
        repo_root,
        LP_BZ_UNIT_GRANULARITY_LABEL,
        None,
        Metadata::new(),
    )?;
    let candidate_period_memberships = build_candidate_period_memberships(
        linear_index_to_row_index,
        model,
        &cut_artifacts.benchmark.phase_plan,
        &result.seeded_schedule,
        tonnage_column,
    )?;
    let candidate_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &candidate_period_memberships)?;
    let candidate_pcpsp_summary =
        summarize_minelib_schedule_solution(pcpsp_problem, &candidate_solution)?;
    let reference_summary = summarize_minelib_schedule_solution(pcpsp_problem, pcpsp_solution)?;
    let candidate_vs_reference_period_alignment =
        compare_period_alignment(pcpsp_solution, &candidate_solution);
    let candidate_vs_reference_destination_membership = compare_period_memberships(
        &build_period_destination_memberships(pcpsp_solution),
        &build_period_destination_memberships(&candidate_solution),
    );
    let temporal_routing_promotion_gate = build_temporal_routing_promotion_gate_summary(
        candidate_pcpsp_summary.discounted_objective,
        reference_summary.discounted_objective,
        candidate_pcpsp_summary.used_period_count,
        reference_summary.used_period_count,
        candidate_vs_reference_period_alignment.mean_absolute_period_delta,
        candidate_vs_reference_period_alignment.earlier_than_reference_count,
        candidate_vs_reference_destination_membership.jaccard_index,
    );
    let promoted_contract_surfaces =
        build_marvin_mr187_promoted_pushback_bench_localized_cut_contract_surfaces(
            selected_block_source,
            selected_block_count,
            preferred_phase_plan_aggregation_strategy,
            preferred_nested_shell_family_contract,
            LP_BZ_UNIT_GRANULARITY_LABEL,
            &cut_artifacts.phase_refinement_diagnostics,
            result.summary.lp_bz_inputs.precedence_units.unit_count,
        );
    let competitive_ready_frontier_probe = build_lp_bz_competitive_ready_frontier_probe_summary(
        ready_frontier_discounted_objective,
        candidate_pcpsp_summary.discounted_objective,
        &result.summary,
    );
    let baseline = LpBzBaselineSummary {
        phase_plan_builder_label: LP_BZ_CUT_BUILDER_LABEL.to_owned(),
        unit_granularity_label: LP_BZ_UNIT_GRANULARITY_LABEL.to_owned(),
        promoted_build_label: promoted_contract_surfaces.promoted_build_label.to_owned(),
        paperlike_candidate_role: MARVIN_MR187_PAPERLIKE_CANDIDATE_ROLE.to_owned(),
        local_optimizer_scaffold_unit_family_label:
            MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL.to_owned(),
        local_optimizer_scaffold_role: MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE.to_owned(),
        unit_family_traceability: promoted_contract_surfaces.unit_family_traceability,
        cut_access_law: promoted_contract_surfaces.access_law,
        phase_refinement_diagnostics: cut_artifacts.phase_refinement_diagnostics,
        competitive_ready_frontier_probe,
        lp_bz_promotion_readiness: build_lp_bz_promotion_readiness_summary(
            "exploratory-local",
            LP_BZ_UNIT_GRANULARITY_LABEL,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
            !MARVIN_MR187_PROMOTED_FAMILY_IS_ACTIVE_CANDIDATE,
            result.summary.lp_bz_lp_solve.solve_status
                == lp_bz_lp_kernel::LpBzLpSolveStatus::Skipped,
            &result
                .summary
                .lp_bz_round_repair
                .local_optimizer_runtime_budget_contract,
        ),
        summary: result.summary,
        candidate_pcpsp_summary: candidate_pcpsp_summary.clone(),
        candidate_vs_reference_metrics: compare_named_numeric_metrics(
            &solution_metric_map(&reference_summary),
            &solution_metric_map(&candidate_pcpsp_summary),
            &BTreeMap::new(),
        ),
        candidate_vs_reference_period_alignment,
        candidate_vs_reference_destination_membership,
        temporal_routing_promotion_gate,
    };
    validate_promoted_pushback_bench_localized_cut_access_law_contract(
        &baseline.cut_access_law,
        &baseline.promoted_build_label,
    )?;
    validate_promoted_pushback_bench_localized_cut_unit_family_traceability(
        &baseline.unit_family_traceability,
        selected_block_source,
        selected_block_count,
        preferred_phase_plan_aggregation_strategy,
        MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL,
        LP_BZ_UNIT_GRANULARITY_LABEL,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
    )?;
    validate_lp_bz_local_optimizer_runtime_budget_contract(
        &baseline
            .summary
            .lp_bz_round_repair
            .local_optimizer_runtime_budget_contract,
    )
    .map_err(mine_sdk::MineError::validation)?;
    validate_lp_bz_promotion_readiness_summary(&baseline.lp_bz_promotion_readiness)
        .map_err(mine_sdk::MineError::validation)?;
    validate_temporal_routing_promotion_gate_summary(&baseline.temporal_routing_promotion_gate)
        .map_err(mine_sdk::MineError::validation)?;
    validate_lp_bz_baseline_runtime_budget_contract(&baseline)?;
    Ok(Some(baseline))
}

fn build_mclaughlin_limit_lp_bz_sidecar(
    config: &DatasetConfig,
    aggregation_strategy: &str,
    scheduling_problem: &SchedulingProblem,
    candidate_discounted_objective: f64,
    reference_discounted_objective: f64,
) -> Result<Option<MclaughlinLimitLpBzSidecarSummary>, mine_sdk::MineError> {
    if config.dataset_id != "mclaughlin-limit"
        || aggregation_strategy != "nested-shell-bench"
        || !config.same_literature_variant
    {
        return Ok(None);
    }

    let kernel_artifact = lp_bz_lp_kernel::build_lp_bz_lp_kernel_artifact(scheduling_problem)?;
    let lp_solve_artifact = lp_bz_lp_kernel::solve_lp_bz_lp_kernel_artifact(&kernel_artifact)?;
    let discounted_objective_bound = lp_solve_artifact.discounted_objective_bound;
    let bound_to_candidate_absolute_gap =
        discounted_objective_bound.map(|bound| bound - candidate_discounted_objective);
    let bound_to_reference_absolute_gap =
        discounted_objective_bound.map(|bound| bound - reference_discounted_objective);
    let completeness_summary = format!(
        "{}",
        lp_bz_precedence_runtime_summary(&lp_solve_artifact.precedence_diagnostics)
    );
    let sidecar_status = match (
        lp_solve_artifact.solve_status,
        lp_solve_artifact
            .precedence_diagnostics
            .coverage_completeness,
    ) {
        (
            lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal,
            lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Complete,
        ) => "benchmark-side-complete-relaxed-kernel-bound",
        (lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal, _) => {
            "benchmark-side-partial-relaxed-kernel-bound"
        }
        _ => "benchmark-side-audited-kernel-without-bound",
    };
    let disclosure_summary = match discounted_objective_bound {
        Some(bound) => format!(
            "A `mclaughlin-limit`-only benchmark-side LP/BZ sidecar now solves the relaxed shell×bench kernel `{}` with status `{}` and {}. Discounted objective bound = {:.3}, bound→candidate gap = {:.3}, bound→reference gap = {:.3}. This remains diagnostic-only bound-like evidence on the relaxed benchmark-side kernel; it does not prove mining-cut comparability and a larger LP bound must not be read as proof that the integer schedule is good.",
            kernel_artifact.kernel_label,
            lp_bz_lp_solve_status_label(lp_solve_artifact.solve_status),
            completeness_summary,
            bound,
            bound - candidate_discounted_objective,
            bound - reference_discounted_objective,
        ),
        None => format!(
            "A `mclaughlin-limit`-only benchmark-side LP/BZ sidecar now audits the relaxed shell×bench kernel `{}` with status `{}` and {}, but no explicit discounted objective bound is available from this solve result. The artifact is still diagnostic-only and does not upgrade the route to mining-cut or literature-grade comparability.",
            kernel_artifact.kernel_label,
            lp_bz_lp_solve_status_label(lp_solve_artifact.solve_status),
            completeness_summary,
        ),
    };
    let mut limitations = lp_solve_artifact.limitations.clone();
    limitations.push(
        "This sidecar is scoped strictly to `mclaughlin-limit`; `mclaughlin-full` remains stress-only and out of LP/BZ promotion scope.".to_owned(),
    );
    limitations.push(
        "The relaxed-kernel bound is benchmark-side evidence only; it should not be interpreted as proof that the current integer schedule is high quality or close to a literature-grade optimum.".to_owned(),
    );

    Ok(Some(MclaughlinLimitLpBzSidecarSummary {
        sidecar_label: "mclaughlin-limit-benchmark-lp-bz-kernel".to_owned(),
        sidecar_version: MCLAUGHLIN_LIMIT_LP_BZ_SIDECAR_VERSION.to_owned(),
        sidecar_status: sidecar_status.to_owned(),
        scope_label: "mclaughlin-limit-only".to_owned(),
        objective_alignment_label: "pcpsp-objective-aligned-relaxed-kernel".to_owned(),
        unit_family_label: aggregation_strategy.to_owned(),
        kernel_label: kernel_artifact.kernel_label,
        solver_label: lp_solve_artifact.solver_label.clone(),
        solve_status: lp_solve_artifact.solve_status,
        scheduling_unit_count: scheduling_problem.units().len(),
        variable_count: lp_solve_artifact.variable_count,
        active_variable_count: lp_solve_artifact.active_variable_count,
        discounted_objective_bound,
        candidate_discounted_objective,
        reference_discounted_objective,
        bound_to_candidate_absolute_gap,
        bound_to_reference_absolute_gap,
        precedence_diagnostics: lp_solve_artifact.precedence_diagnostics,
        cut_diagnostics: lp_solve_artifact.cut_diagnostics,
        completeness_summary,
        disclosure_summary,
        limitations,
    }))
}

fn supports_lp_bz_baseline(config: &DatasetConfig) -> bool {
    config.dataset_id == "marvin" && config.benchmark_family == "marvin"
}

fn validate_lp_bz_baseline_runtime_budget_contract(
    baseline: &LpBzBaselineSummary,
) -> Result<(), mine_sdk::MineError> {
    if baseline
        .summary
        .lp_bz_round_repair
        .local_optimizer_runtime_budget_contract
        != baseline
            .lp_bz_promotion_readiness
            .local_optimizer_runtime_budget_contract
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar must keep the explicit local optimizer runtime budget contract aligned between the sidecar summary and promotion readiness."
                .to_owned(),
        ));
    }
    let runtime_budget_contract = &baseline
        .summary
        .lp_bz_round_repair
        .local_optimizer_runtime_budget_contract;
    let round_repair = &baseline.summary.lp_bz_round_repair;
    if runtime_budget_contract.strategy_label != round_repair.local_optimizer_strategy_label
        || runtime_budget_contract.executed_iteration_count
            != round_repair.local_optimizer_executed_iteration_count
        || runtime_budget_contract.termination_reason
            != round_repair.local_optimizer_termination_reason
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar must keep the adapter-owned explicit local optimizer runtime budget contract consistent with the promoted round-repair diagnostics."
                .to_owned(),
        ));
    }
    if runtime_budget_contract.max_iteration_count
        != round_repair
            .local_optimizer_budget_profile
            .effective_iteration_budget
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar must keep the surfaced effective local-search budget aligned with the promoted runtime budget contract."
                .to_owned(),
        ));
    }
    if round_repair.local_optimization_skipped
        != lp_bz_rounder::local_optimizer_runtime_was_skipped(
            &round_repair.local_optimizer_termination_reason,
        )
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar must derive `local_optimization_skipped` from the surfaced promoted termination reason."
                .to_owned(),
        ));
    }
    if runtime_budget_contract.execution_state == "budget-hit"
        && (round_repair.local_optimization_skipped
            || baseline
                .lp_bz_promotion_readiness
                .summary
                .contains("local optimization is skipped"))
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar must distinguish budget-hit termination from skipped local optimization in the readiness summary."
                .to_owned(),
        ));
    }
    validate_lp_bz_competitive_ready_frontier_probe_summary(baseline)?;
    Ok(())
}

fn build_lp_bz_competitive_ready_frontier_probe_summary(
    ready_frontier_discounted_objective: f64,
    focused_candidate_discounted_objective: f64,
    adapter_summary: &MarvinLpBzAdapterSummary,
) -> LpBzCompetitiveReadyFrontierProbeSummary {
    let competitive_probe = &adapter_summary.lp_bz_round_repair.competitive_probe;
    let focused_candidate_vs_ready_frontier_objective_gap =
        ready_frontier_discounted_objective - focused_candidate_discounted_objective;
    let competitive_probe_proxy_gap_closure =
        competitive_probe.local_search_score_delta_vs_focused_proxy;
    let competitive_probe_proxy_gap_closure_share =
        if focused_candidate_vs_ready_frontier_objective_gap <= 1.0e-9 {
            0.0
        } else {
            competitive_probe_proxy_gap_closure / focused_candidate_vs_ready_frontier_objective_gap
        };
    let residual_ready_frontier_gap_after_competitive_probe_proxy =
        focused_candidate_vs_ready_frontier_objective_gap - competitive_probe_proxy_gap_closure;
    let empirical_driver_assessment = build_lp_bz_competitive_empirical_driver_assessment(
        adapter_summary,
        competitive_probe_proxy_gap_closure_share,
        residual_ready_frontier_gap_after_competitive_probe_proxy,
    );
    let parity_claim_status = LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS;
    let budget_coverage_experiment = build_lp_bz_budget_coverage_experiment_summary(
        ready_frontier_discounted_objective,
        focused_candidate_discounted_objective,
        adapter_summary,
        empirical_driver_assessment
            .empirical_dominant_blocker
            .as_str(),
        parity_claim_status,
    );
    let residual_interpretation = classify_lp_bz_ready_frontier_probe_residual_interpretation(
        residual_ready_frontier_gap_after_competitive_probe_proxy,
        &competitive_probe.competitive_local_optimizer_residual_opportunity,
    );
    let dominant_residual_driver = classify_lp_bz_ready_frontier_probe_dominant_residual_driver(
        residual_ready_frontier_gap_after_competitive_probe_proxy,
        &competitive_probe.competitive_local_optimizer_residual_opportunity,
    );
    let next_step_evidence = classify_lp_bz_ready_frontier_probe_next_step_evidence(
        residual_ready_frontier_gap_after_competitive_probe_proxy,
        &competitive_probe.competitive_local_optimizer_residual_opportunity,
    );
    let residual_interpretation_summary =
        summarize_lp_bz_ready_frontier_probe_residual_interpretation(
            residual_interpretation,
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    let dominant_residual_driver_summary =
        summarize_lp_bz_ready_frontier_probe_dominant_residual_driver(
            dominant_residual_driver,
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    let next_step_evidence_summary = summarize_lp_bz_ready_frontier_probe_next_step_evidence(
        next_step_evidence,
        residual_ready_frontier_gap_after_competitive_probe_proxy,
        &competitive_probe.competitive_local_optimizer_residual_opportunity,
    );
    let parity_claim_summary =
        summarize_lp_bz_ready_frontier_probe_parity_claim_status(parity_claim_status);
    let remaining_blockers = build_lp_bz_ready_frontier_probe_remaining_blockers(
        dominant_residual_driver,
        dominant_residual_driver_summary.as_str(),
        residual_interpretation,
        residual_interpretation_summary.as_str(),
        next_step_evidence,
        next_step_evidence_summary.as_str(),
        parity_claim_summary.as_str(),
    );
    let remaining_blockers_summary = summarize_lp_bz_ready_frontier_probe_remaining_blockers(
        residual_interpretation_summary.as_str(),
        parity_claim_status,
        &remaining_blockers,
    );
    let readiness_criteria = build_lp_bz_competitive_readiness_criteria(
        dominant_residual_driver,
        dominant_residual_driver_summary.as_str(),
        residual_interpretation,
        residual_interpretation_summary.as_str(),
        next_step_evidence,
        next_step_evidence_summary.as_str(),
        parity_claim_summary.as_str(),
    );
    let readiness_blocked_criteria_count =
        count_blocked_lp_bz_competitive_readiness_criteria(&readiness_criteria);
    let readiness_state =
        classify_lp_bz_competitive_readiness_state(readiness_blocked_criteria_count);
    let readiness_summary = summarize_lp_bz_competitive_readiness(
        readiness_state,
        readiness_blocked_criteria_count,
        remaining_blockers_summary.as_str(),
        parity_claim_summary.as_str(),
    );

    LpBzCompetitiveReadyFrontierProbeSummary {
        driver_targeting_status: classify_lp_bz_ready_frontier_probe_driver_targeting_status(
            focused_candidate_vs_ready_frontier_objective_gap,
        )
        .to_owned(),
        closure_status: classify_lp_bz_ready_frontier_probe_closure_status(
            focused_candidate_vs_ready_frontier_objective_gap,
            competitive_probe_proxy_gap_closure,
            competitive_probe
                .competitive_local_optimizer_residual_opportunity
                .improving_move_available,
        )
        .to_owned(),
        ready_frontier_discounted_objective,
        focused_candidate_discounted_objective,
        focused_candidate_vs_ready_frontier_objective_gap,
        competitive_probe_proxy_gap_closure,
        competitive_probe_proxy_gap_closure_share,
        residual_ready_frontier_gap_after_competitive_probe_proxy,
        empirical_dominant_blocker: empirical_driver_assessment.empirical_dominant_blocker,
        empirical_dominant_blocker_summary: empirical_driver_assessment
            .empirical_dominant_blocker_summary,
        empirical_driver_evidence_summary: empirical_driver_assessment
            .empirical_driver_evidence_summary,
        empirical_driver_evidence: empirical_driver_assessment.empirical_driver_evidence,
        budget_coverage_experiment,
        residual_interpretation: residual_interpretation.to_owned(),
        residual_interpretation_summary,
        dominant_residual_driver: dominant_residual_driver.to_owned(),
        dominant_residual_driver_summary,
        next_step_evidence: next_step_evidence.to_owned(),
        next_step_evidence_summary,
        parity_claim_status: parity_claim_status.to_owned(),
        parity_claim_summary,
        remaining_blocker_count: remaining_blockers.len(),
        remaining_blockers_summary,
        remaining_blockers,
        readiness_criteria_version: LP_BZ_COMPETITIVE_READINESS_CRITERIA_VERSION.to_owned(),
        readiness_state: readiness_state.to_owned(),
        readiness_summary,
        readiness_blocked_criteria_count,
        readiness_criteria,
    }
}

fn summarize_lp_bz_competitive_readiness_exit_shape() -> String {
    format!(
        "residual_interpretation=`{LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION}`, dominant_residual_driver=`{LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER}`, next_step_evidence=`{LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE}`, parity_claim_status=`{LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS}`"
    )
}

fn lp_bz_competitive_readiness_exit_shape_is_cleared(
    residual_interpretation: &str,
    dominant_residual_driver: &str,
    next_step_evidence: &str,
) -> bool {
    residual_interpretation == LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION
        && dominant_residual_driver == LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER
        && next_step_evidence == LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE
}

fn summarize_lp_bz_ready_frontier_probe_parity_claim_status(parity_claim_status: &str) -> String {
    match parity_claim_status {
        LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS => format!(
            "Benchmark-side LP/BZ vs ready_frontier remains diagnostic-only: even if {} clears, the competitive probe still provides auditable benchmark-side evidence rather than schedule-level proof of parity or material competitiveness.",
            summarize_lp_bz_competitive_readiness_exit_shape()
        ),
        _ => "Benchmark-side LP/BZ vs ready_frontier claim status is unspecified.".to_owned(),
    }
}

fn build_lp_bz_ready_frontier_probe_remaining_blockers(
    dominant_residual_driver: &str,
    dominant_residual_driver_summary: &str,
    residual_interpretation: &str,
    residual_interpretation_summary: &str,
    next_step_evidence: &str,
    next_step_evidence_summary: &str,
    parity_claim_summary: &str,
) -> Vec<LpBzReadyFrontierParityBlocker> {
    let mut blockers = Vec::new();
    let exit_shape_summary = summarize_lp_bz_competitive_readiness_exit_shape();
    let exit_shape_cleared = lp_bz_competitive_readiness_exit_shape_is_cleared(
        residual_interpretation,
        dominant_residual_driver,
        next_step_evidence,
    );

    if dominant_residual_driver != LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER {
        blockers.push(LpBzReadyFrontierParityBlocker {
            blocker_id: "dominant-residual-driver".to_owned(),
            blocker_label: "Dominant residual driver still blocks benchmark-side material competitiveness"
                .to_owned(),
            status: "active".to_owned(),
            summary: format!(
                "Competitive-readiness exit criterion requires dominant_residual_driver=`{LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER}`, but current value is `{dominant_residual_driver}`. {dominant_residual_driver_summary}"
            ),
            evidence_fields: vec![
                "lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver_summary"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation_summary"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_ready_frontier_gap_after_competitive_probe_proxy".to_owned(),
            ],
        });
    }

    if next_step_evidence != LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE {
        blockers.push(LpBzReadyFrontierParityBlocker {
            blocker_id: "next-step-evidence".to_owned(),
            blocker_label:
                "Next benchmark-side evidence still exceeds the schedule-proof-only exit criterion"
                    .to_owned(),
            status: "active".to_owned(),
            summary: format!(
                "Competitive-readiness exit criterion requires next_step_evidence=`{LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE}`, but current value is `{next_step_evidence}`. {next_step_evidence_summary}"
            ),
            evidence_fields: vec![
                "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence".to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence_summary"
                    .to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.local_search_score_delta_vs_focused_proxy".to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_residual_opportunity.improving_move_available".to_owned(),
                "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_residual_opportunity.discounted_gain".to_owned(),
            ],
        });
    }

    blockers.push(LpBzReadyFrontierParityBlocker {
        blocker_id: "schedule-level-ready-frontier-proof".to_owned(),
        blocker_label:
            "Schedule-level ready_frontier proof still gates any material competitiveness claim"
                .to_owned(),
        status: "active".to_owned(),
        summary: if exit_shape_cleared {
            format!(
                "{parity_claim_summary} Benchmark-side competitive-readiness exit shape is satisfied ({exit_shape_summary}); schedule-level ready_frontier proof is the only remaining blocker. Residual read: {residual_interpretation_summary}"
            )
        } else {
            format!(
                "{parity_claim_summary} Benchmark-side competitive-readiness exit shape is not yet satisfied ({exit_shape_summary}); current statuses are residual_interpretation=`{residual_interpretation}`, dominant_residual_driver=`{dominant_residual_driver}`, next_step_evidence=`{next_step_evidence}`. Residual read: {residual_interpretation_summary}"
            )
        },
        evidence_fields: vec![
            "lp_bz_baseline.competitive_ready_frontier_probe.closure_status".to_owned(),
            "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation".to_owned(),
            "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation_summary"
                .to_owned(),
            "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence".to_owned(),
            "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence_summary".to_owned(),
            "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status".to_owned(),
            "lp_bz_baseline.competitive_ready_frontier_probe.readiness_state".to_owned(),
        ],
    });

    if exit_shape_cleared && blockers.len() != 1 {
        return vec![
            blockers
                .into_iter()
                .find(|blocker| blocker.blocker_id == "schedule-level-ready-frontier-proof")
                .expect("schedule-level parity proof blocker should always be present"),
        ];
    }

    blockers
}

fn summarize_lp_bz_ready_frontier_probe_remaining_blockers(
    residual_interpretation_summary: &str,
    parity_claim_status: &str,
    blockers: &[LpBzReadyFrontierParityBlocker],
) -> String {
    let blocker_labels = blockers
        .iter()
        .map(|blocker| blocker.blocker_label.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "LP/BZ vs ready_frontier remains {parity_claim_status} with {} active benchmark-side blocker(s): {blocker_labels}. Competitive-readiness exit shape requires {}. Residual interpretation: {residual_interpretation_summary}",
        blockers.len(),
        summarize_lp_bz_competitive_readiness_exit_shape()
    )
}

fn build_lp_bz_competitive_readiness_criteria(
    dominant_residual_driver: &str,
    dominant_residual_driver_summary: &str,
    residual_interpretation: &str,
    residual_interpretation_summary: &str,
    next_step_evidence: &str,
    next_step_evidence_summary: &str,
    parity_claim_summary: &str,
) -> Vec<LpBzCompetitiveReadinessCriterion> {
    vec![
        LpBzCompetitiveReadinessCriterion {
            criterion_id: "dominant-residual-driver-cleared".to_owned(),
            criterion_label: format!(
                "dominant_residual_driver must equal `{LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER}` before LP/BZ can be treated as benchmark-side materially competitive"
            ),
            status: if dominant_residual_driver
                == LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER
            {
                "ready"
            } else {
                "blocked"
            }
            .to_owned(),
            summary: if dominant_residual_driver
                == LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER
            {
                format!(
                    "dominant_residual_driver=`{LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER}`; this benchmark-side material-competitiveness exit criterion is satisfied and only schedule-level proof can remain."
                )
            } else {
                format!(
                    "Expected dominant_residual_driver=`{LP_BZ_COMPETITIVE_READY_EXIT_DOMINANT_RESIDUAL_DRIVER}`, but current value is `{dominant_residual_driver}`. {dominant_residual_driver_summary}"
                )
            },
            evidence_fields: vec![
                "lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver_summary"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers".to_owned(),
            ],
        },
        LpBzCompetitiveReadinessCriterion {
            criterion_id: "residual-interpretation-cleared".to_owned(),
            criterion_label: format!(
                "residual_interpretation must equal `{LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION}` before only schedule-level proof remains"
            ),
            status: if residual_interpretation
                == LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION
            {
                "ready"
            } else {
                "blocked"
            }
            .to_owned(),
            summary: if residual_interpretation
                == LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION
            {
                format!(
                    "residual_interpretation=`{LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION}`; benchmark-side follow-up can now collapse to schedule-level proof."
                )
            } else {
                format!(
                    "Expected residual_interpretation=`{LP_BZ_COMPETITIVE_READY_EXIT_RESIDUAL_INTERPRETATION}`, but current value is `{residual_interpretation}`. {residual_interpretation_summary}"
                )
            },
            evidence_fields: vec![
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation_summary"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.residual_ready_frontier_gap_after_competitive_probe_proxy".to_owned(),
            ],
        },
        LpBzCompetitiveReadinessCriterion {
            criterion_id: "next-step-evidence-narrows-to-schedule-proof".to_owned(),
            criterion_label: format!(
                "next_step_evidence must equal `{LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE}` before the benchmark-side exit checklist is clear"
            ),
            status: if next_step_evidence == LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE {
                "ready"
            } else {
                "blocked"
            }
            .to_owned(),
            summary: if next_step_evidence == LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE {
                format!(
                    "next_step_evidence=`{LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE}`; the remaining evidence request is now schedule-level ready_frontier proof rather than another benchmark-side candidate-improvement explanation."
                )
            } else {
                format!(
                    "Expected next_step_evidence=`{LP_BZ_COMPETITIVE_READY_EXIT_NEXT_STEP_EVIDENCE}`, but current value is `{next_step_evidence}`. {next_step_evidence_summary}"
                )
            },
            evidence_fields: vec![
                "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence".to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence_summary"
                    .to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers".to_owned(),
            ],
        },
        LpBzCompetitiveReadinessCriterion {
            criterion_id: "parity-claim-guardrail".to_owned(),
            criterion_label: format!(
                "parity_claim_status must stay `{LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS}` until schedule-level proof exists"
            ),
            status: "guardrail-active".to_owned(),
            summary: format!(
                "{parity_claim_summary} The benchmark-side exit checklist can clear before schedule-level proof, but parity_claim_status must remain `{LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS}`."
            ),
            evidence_fields: vec![
                "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status".to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_summary".to_owned(),
                "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers_summary"
                    .to_owned(),
            ],
        },
    ]
}

fn count_blocked_lp_bz_competitive_readiness_criteria(
    criteria: &[LpBzCompetitiveReadinessCriterion],
) -> usize {
    criteria
        .iter()
        .filter(|criterion| criterion.status == "blocked")
        .count()
}

fn classify_lp_bz_competitive_readiness_state(blocked_criteria_count: usize) -> &'static str {
    if blocked_criteria_count == 0 {
        "benchmark-side-ready-for-schedule-proof"
    } else {
        "benchmark-side-not-ready"
    }
}

fn summarize_lp_bz_competitive_readiness(
    readiness_state: &str,
    blocked_criteria_count: usize,
    remaining_blockers_summary: &str,
    parity_claim_summary: &str,
) -> String {
    match readiness_state {
        "benchmark-side-ready-for-schedule-proof" => format!(
            "Benchmark-side LP/BZ candidate satisfies the current competitive-readiness criteria ({}) so schedule-level ready_frontier proof is the only remaining step before any competitive claim could move beyond diagnostics. {parity_claim_summary}",
            summarize_lp_bz_competitive_readiness_exit_shape()
        ),
        _ => format!(
            "Benchmark-side LP/BZ candidate still fails {blocked_criteria_count} competitive-readiness criterion/criteria before schedule-level proof would be the only missing step. The benchmark-side exit shape remains {}. {remaining_blockers_summary}",
            summarize_lp_bz_competitive_readiness_exit_shape()
        ),
    }
}

fn classify_lp_bz_ready_frontier_probe_driver_targeting_status(
    focused_candidate_vs_ready_frontier_objective_gap: f64,
) -> &'static str {
    if focused_candidate_vs_ready_frontier_objective_gap > 1.0e-9 {
        "candidate-vs-ready-frontier-gap-active"
    } else {
        "focused-candidate-matches-or-beats-ready-frontier"
    }
}

fn classify_lp_bz_ready_frontier_probe_closure_status(
    focused_candidate_vs_ready_frontier_objective_gap: f64,
    competitive_probe_proxy_gap_closure: f64,
    competitive_probe_residual_headroom_available: bool,
) -> &'static str {
    if focused_candidate_vs_ready_frontier_objective_gap <= 1.0e-9 {
        "ready-frontier-gap-not-active"
    } else if competitive_probe_proxy_gap_closure
        >= focused_candidate_vs_ready_frontier_objective_gap - 1.0e-9
    {
        "competitive-probe-proxy-closes-ready-frontier-gap"
    } else if competitive_probe_proxy_gap_closure > 1.0e-9 {
        "competitive-probe-proxy-partially-closes-ready-frontier-gap"
    } else if competitive_probe_residual_headroom_available {
        "competitive-probe-still-has-headroom-below-ready-frontier-gap"
    } else {
        "competitive-probe-does-not-close-ready-frontier-gap"
    }
}

fn lp_bz_ready_frontier_probe_residual_components(
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> (f64, f64) {
    let residual_ready_frontier_gap =
        residual_ready_frontier_gap_after_competitive_probe_proxy.max(0.0);
    let residual_competitive_headroom =
        if competitive_probe_residual_opportunity.improving_move_available {
            competitive_probe_residual_opportunity
                .discounted_gain
                .max(0.0)
        } else {
            0.0
        };
    (residual_ready_frontier_gap, residual_competitive_headroom)
}

fn classify_lp_bz_ready_frontier_probe_residual_interpretation(
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> &'static str {
    let (residual_ready_frontier_gap, residual_competitive_headroom) =
        lp_bz_ready_frontier_probe_residual_components(
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            competitive_probe_residual_opportunity,
        );

    if residual_ready_frontier_gap <= 1.0e-9 {
        "proxy-covers-measured-ready-frontier-gap"
    } else if residual_competitive_headroom <= 1.0e-9 {
        "residual-gap-persists-without-probe-headroom"
    } else if residual_ready_frontier_gap > residual_competitive_headroom + 1.0e-9 {
        "residual-gap-still-exceeds-probe-headroom"
    } else if residual_competitive_headroom > residual_ready_frontier_gap + 1.0e-9 {
        "probe-headroom-could-still-cover-residual-gap"
    } else {
        "residual-gap-and-probe-headroom-are-balanced"
    }
}

fn summarize_lp_bz_ready_frontier_probe_residual_interpretation(
    residual_interpretation: &str,
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> String {
    let (residual_ready_frontier_gap, residual_competitive_headroom) =
        lp_bz_ready_frontier_probe_residual_components(
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            competitive_probe_residual_opportunity,
        );
    let move_kind_label = competitive_probe_residual_opportunity
        .move_kind_label
        .as_str();

    match residual_interpretation {
        "proxy-covers-measured-ready-frontier-gap" => format!(
            "Competitive probe proxy covers the measured ready_frontier gap (residual gap {:.6}), but benchmark-side evidence still needs a schedule-level comparison before claiming parity.",
            residual_ready_frontier_gap
        ),
        "residual-gap-persists-without-probe-headroom" => format!(
            "Residual ready_frontier gap {:.6} remains after the competitive probe proxy, and the probe reports no further improving move.",
            residual_ready_frontier_gap
        ),
        "residual-gap-still-exceeds-probe-headroom" => format!(
            "Residual ready_frontier gap {:.6} still exceeds the competitive probe's remaining {move_kind_label} headroom {:.6}, so the measured gap remains materially open.",
            residual_ready_frontier_gap, residual_competitive_headroom
        ),
        "probe-headroom-could-still-cover-residual-gap" => format!(
            "Competitive probe residual {move_kind_label} headroom {:.6} still exceeds the residual ready_frontier gap {:.6}, so benchmark-side evidence suggests further probe follow-through could cover the measured gap.",
            residual_competitive_headroom, residual_ready_frontier_gap
        ),
        _ => format!(
            "Residual ready_frontier gap {:.6} and competitive probe {move_kind_label} headroom {:.6} are balanced, so benchmark-side evidence cannot yet separate gap persistence from remaining probe headroom.",
            residual_ready_frontier_gap, residual_competitive_headroom
        ),
    }
}

fn classify_lp_bz_ready_frontier_probe_dominant_residual_driver(
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> &'static str {
    let (residual_ready_frontier_gap, residual_competitive_headroom) =
        lp_bz_ready_frontier_probe_residual_components(
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            competitive_probe_residual_opportunity,
        );

    if residual_ready_frontier_gap <= 1.0e-9 {
        "proxy-covered-measured-ready-frontier-gap"
    } else if residual_competitive_headroom <= 1.0e-9
        || residual_ready_frontier_gap > residual_competitive_headroom + 1.0e-9
    {
        "remaining-ready-frontier-gap"
    } else if residual_competitive_headroom > residual_ready_frontier_gap + 1.0e-9 {
        "competitive-probe-residual-headroom"
    } else {
        "split-between-ready-frontier-gap-and-residual-headroom"
    }
}

fn summarize_lp_bz_ready_frontier_probe_dominant_residual_driver(
    dominant_residual_driver: &str,
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> String {
    let (residual_ready_frontier_gap, residual_competitive_headroom) =
        lp_bz_ready_frontier_probe_residual_components(
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            competitive_probe_residual_opportunity,
        );
    let move_kind_label = competitive_probe_residual_opportunity
        .move_kind_label
        .as_str();

    match dominant_residual_driver {
        "proxy-covered-measured-ready-frontier-gap" => format!(
            "Competitive probe proxy covers the measured ready_frontier gap (residual gap {:.6}), but the benchmark still needs schedule-level proof before claiming parity.",
            residual_ready_frontier_gap
        ),
        "remaining-ready-frontier-gap" if residual_competitive_headroom <= 1.0e-9 => format!(
            "Residual ready_frontier gap {:.6} remains the dominant measured blocker; the competitive probe reports no improving move.",
            residual_ready_frontier_gap
        ),
        "remaining-ready-frontier-gap" => format!(
            "Residual ready_frontier gap {:.6} remains the dominant measured blocker, ahead of the competitive probe's remaining {move_kind_label} headroom {:.6}.",
            residual_ready_frontier_gap, residual_competitive_headroom
        ),
        "competitive-probe-residual-headroom" => format!(
            "Competitive probe residual {move_kind_label} headroom {:.6} is now the dominant benchmark-side follow-up signal, versus residual ready_frontier gap {:.6}.",
            residual_competitive_headroom, residual_ready_frontier_gap
        ),
        _ => format!(
            "Residual ready_frontier gap {:.6} and competitive probe {move_kind_label} headroom {:.6} remain effectively tied as benchmark-side blockers.",
            residual_ready_frontier_gap, residual_competitive_headroom
        ),
    }
}

fn classify_lp_bz_ready_frontier_probe_next_step_evidence(
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> &'static str {
    let (residual_ready_frontier_gap, residual_competitive_headroom) =
        lp_bz_ready_frontier_probe_residual_components(
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            competitive_probe_residual_opportunity,
        );

    if residual_ready_frontier_gap <= 1.0e-9 {
        "need-schedule-level-ready-frontier-proof"
    } else if residual_competitive_headroom <= 1.0e-9 {
        "need-new-candidate-evidence-beyond-current-probe"
    } else if residual_ready_frontier_gap > residual_competitive_headroom + 1.0e-9 {
        "need-broader-candidate-improvement-than-current-probe"
    } else if residual_competitive_headroom > residual_ready_frontier_gap + 1.0e-9 {
        "need-probe-headroom-converted-into-candidate-improvement"
    } else {
        "need-probe-and-candidate-follow-through"
    }
}

fn summarize_lp_bz_ready_frontier_probe_next_step_evidence(
    next_step_evidence: &str,
    residual_ready_frontier_gap_after_competitive_probe_proxy: f64,
    competitive_probe_residual_opportunity: &lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity,
) -> String {
    let (residual_ready_frontier_gap, residual_competitive_headroom) =
        lp_bz_ready_frontier_probe_residual_components(
            residual_ready_frontier_gap_after_competitive_probe_proxy,
            competitive_probe_residual_opportunity,
        );
    let move_kind_label = competitive_probe_residual_opportunity
        .move_kind_label
        .as_str();

    match next_step_evidence {
        "need-schedule-level-ready-frontier-proof" => format!(
            "The competitive probe proxy closes the measured ready_frontier gap, so the next benchmark-side evidence is a schedule-level ready_frontier comparison before claiming competitive parity."
        ),
        "need-new-candidate-evidence-beyond-current-probe" => format!(
            "Residual ready_frontier gap {:.6} remains after the current probe exhausted its headroom, so the next evidence must come from a stronger candidate path than the current probe.",
            residual_ready_frontier_gap
        ),
        "need-broader-candidate-improvement-than-current-probe" => format!(
            "Residual ready_frontier gap {:.6} still exceeds remaining {move_kind_label} headroom {:.6}, so the next evidence must show a broader candidate improvement than the current probe can explain.",
            residual_ready_frontier_gap, residual_competitive_headroom
        ),
        "need-probe-headroom-converted-into-candidate-improvement" => format!(
            "Remaining competitive probe {move_kind_label} headroom {:.6} exceeds residual ready_frontier gap {:.6}, so the next evidence is to convert that benchmark-side headroom into an actual candidate improvement.",
            residual_competitive_headroom, residual_ready_frontier_gap
        ),
        _ => format!(
            "Residual ready_frontier gap {:.6} and competitive probe {move_kind_label} headroom {:.6} are balanced, so the next evidence must show both probe follow-through and candidate-level improvement.",
            residual_ready_frontier_gap, residual_competitive_headroom
        ),
    }
}

fn validate_lp_bz_competitive_ready_frontier_probe_summary(
    baseline: &LpBzBaselineSummary,
) -> Result<(), mine_sdk::MineError> {
    let probe = &baseline.competitive_ready_frontier_probe;
    let focused_candidate_discounted_objective =
        baseline.candidate_pcpsp_summary.discounted_objective;
    let competitive_probe = &baseline.summary.lp_bz_round_repair.competitive_probe;
    let expected_empirical_driver_assessment = build_lp_bz_competitive_empirical_driver_assessment(
        &baseline.summary,
        probe.competitive_probe_proxy_gap_closure_share,
        probe.residual_ready_frontier_gap_after_competitive_probe_proxy,
    );
    let expected_gap =
        probe.ready_frontier_discounted_objective - focused_candidate_discounted_objective;
    if (probe.focused_candidate_discounted_objective - focused_candidate_discounted_objective).abs()
        > 1.0e-9
        || (probe.focused_candidate_vs_ready_frontier_objective_gap - expected_gap).abs() > 1.0e-9
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the focused candidate objective and ready-frontier gap aligned with the surfaced candidate summary."
                .to_owned(),
        ));
    }
    if (probe.competitive_probe_proxy_gap_closure
        - competitive_probe.local_search_score_delta_vs_focused_proxy)
        .abs()
        > 1.0e-9
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must reuse the adapter competitive probe headroom delta without drift."
                .to_owned(),
        ));
    }
    let expected_share = if expected_gap <= 1.0e-9 {
        0.0
    } else {
        competitive_probe.local_search_score_delta_vs_focused_proxy / expected_gap
    };
    if (probe.competitive_probe_proxy_gap_closure_share - expected_share).abs() > 1.0e-9 {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the proxy gap-closure share aligned with the surfaced ready-frontier gap."
                .to_owned(),
        ));
    }
    let expected_residual_gap =
        expected_gap - competitive_probe.local_search_score_delta_vs_focused_proxy;
    if (probe.residual_ready_frontier_gap_after_competitive_probe_proxy - expected_residual_gap)
        .abs()
        > 1.0e-9
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the residual ready-frontier gap aligned with the surfaced focused gap and competitive headroom."
                .to_owned(),
        ));
    }
    if probe.empirical_dominant_blocker
        != expected_empirical_driver_assessment.empirical_dominant_blocker
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the empirical dominant blocker aligned with precedence coverage, budget depletion and round/repair/local-search evidence."
                .to_owned(),
        ));
    }
    if probe.empirical_dominant_blocker_summary
        != expected_empirical_driver_assessment.empirical_dominant_blocker_summary
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the empirical dominant blocker summary aligned with the surfaced evidence."
                .to_owned(),
        ));
    }
    if probe.empirical_driver_evidence_summary
        != expected_empirical_driver_assessment.empirical_driver_evidence_summary
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the empirical driver evidence summary aligned with the surfaced evidence statuses."
                .to_owned(),
        ));
    }
    if probe.empirical_driver_evidence
        != expected_empirical_driver_assessment.empirical_driver_evidence
    {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the empirical driver evidence checklist aligned with precedence coverage, budget depletion and round/repair/local-search evidence."
                .to_owned(),
        ));
    }
    let expected_budget_coverage_experiment = build_lp_bz_budget_coverage_experiment_summary(
        probe.ready_frontier_discounted_objective,
        focused_candidate_discounted_objective,
        &baseline.summary,
        expected_empirical_driver_assessment
            .empirical_dominant_blocker
            .as_str(),
        LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS,
    );
    if probe.budget_coverage_experiment != expected_budget_coverage_experiment {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the budget/coverage experiment summary aligned with the surfaced before/after evidence."
                .to_owned(),
        ));
    }
    let expected_residual_interpretation =
        classify_lp_bz_ready_frontier_probe_residual_interpretation(
            expected_residual_gap,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    if probe.residual_interpretation != expected_residual_interpretation {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the residual interpretation aligned with the surfaced residual gap and residual headroom."
                .to_owned(),
        ));
    }
    let expected_residual_interpretation_summary =
        summarize_lp_bz_ready_frontier_probe_residual_interpretation(
            expected_residual_interpretation,
            expected_residual_gap,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    if probe.residual_interpretation_summary != expected_residual_interpretation_summary {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the residual interpretation summary aligned with the surfaced residual gap and residual headroom."
                .to_owned(),
        ));
    }
    let expected_driver_targeting_status =
        classify_lp_bz_ready_frontier_probe_driver_targeting_status(expected_gap);
    if probe.driver_targeting_status != expected_driver_targeting_status {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the driver-targeting status aligned with the focused candidate vs ready-frontier objective gap."
                .to_owned(),
        ));
    }
    let expected_closure_status = classify_lp_bz_ready_frontier_probe_closure_status(
        expected_gap,
        competitive_probe.local_search_score_delta_vs_focused_proxy,
        competitive_probe
            .competitive_local_optimizer_residual_opportunity
            .improving_move_available,
    );
    if probe.closure_status != expected_closure_status {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the closure status aligned with the surfaced ready-frontier gap and competitive headroom."
                .to_owned(),
        ));
    }
    let expected_dominant_residual_driver =
        classify_lp_bz_ready_frontier_probe_dominant_residual_driver(
            expected_residual_gap,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    if probe.dominant_residual_driver != expected_dominant_residual_driver {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the dominant residual driver aligned with the surfaced residual gap and residual headroom."
                .to_owned(),
        ));
    }
    let expected_dominant_residual_driver_summary =
        summarize_lp_bz_ready_frontier_probe_dominant_residual_driver(
            expected_dominant_residual_driver,
            expected_residual_gap,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    if probe.dominant_residual_driver_summary != expected_dominant_residual_driver_summary {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the dominant residual driver summary aligned with the surfaced residual gap and residual headroom."
                .to_owned(),
        ));
    }
    let expected_next_step_evidence = classify_lp_bz_ready_frontier_probe_next_step_evidence(
        expected_residual_gap,
        &competitive_probe.competitive_local_optimizer_residual_opportunity,
    );
    if probe.next_step_evidence != expected_next_step_evidence {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the next-step evidence status aligned with the surfaced residual gap and residual headroom."
                .to_owned(),
        ));
    }
    let expected_next_step_evidence_summary =
        summarize_lp_bz_ready_frontier_probe_next_step_evidence(
            expected_next_step_evidence,
            expected_residual_gap,
            &competitive_probe.competitive_local_optimizer_residual_opportunity,
        );
    if probe.next_step_evidence_summary != expected_next_step_evidence_summary {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the next-step evidence summary aligned with the surfaced residual gap and residual headroom."
                .to_owned(),
        ));
    }
    let expected_parity_claim_status = LP_BZ_COMPETITIVE_READY_EXIT_PARITY_CLAIM_STATUS;
    if probe.parity_claim_status != expected_parity_claim_status {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the parity claim status explicitly diagnostic-only."
                .to_owned(),
        ));
    }
    let expected_parity_claim_summary =
        summarize_lp_bz_ready_frontier_probe_parity_claim_status(expected_parity_claim_status);
    if probe.parity_claim_summary != expected_parity_claim_summary {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the parity claim summary aligned with the diagnostic-only contract."
                .to_owned(),
        ));
    }
    let expected_remaining_blockers = build_lp_bz_ready_frontier_probe_remaining_blockers(
        expected_dominant_residual_driver,
        expected_dominant_residual_driver_summary.as_str(),
        expected_residual_interpretation,
        expected_residual_interpretation_summary.as_str(),
        expected_next_step_evidence,
        expected_next_step_evidence_summary.as_str(),
        expected_parity_claim_summary.as_str(),
    );
    if probe.remaining_blockers != expected_remaining_blockers {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the remaining-blocker checklist aligned with the surfaced residual driver, interpretation and next-step evidence."
                .to_owned(),
        ));
    }
    if probe.remaining_blocker_count != expected_remaining_blockers.len() {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the remaining blocker count aligned with the checklist entries."
                .to_owned(),
        ));
    }
    let expected_remaining_blockers_summary =
        summarize_lp_bz_ready_frontier_probe_remaining_blockers(
            expected_residual_interpretation_summary.as_str(),
            expected_parity_claim_status,
            &expected_remaining_blockers,
        );
    if probe.remaining_blockers_summary != expected_remaining_blockers_summary {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the remaining blockers summary aligned with the surfaced checklist."
                .to_owned(),
        ));
    }
    if probe.readiness_criteria_version != LP_BZ_COMPETITIVE_READINESS_CRITERIA_VERSION {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the benchmark-side competitive-readiness criteria version explicit."
                .to_owned(),
        ));
    }
    let expected_readiness_criteria = build_lp_bz_competitive_readiness_criteria(
        expected_dominant_residual_driver,
        expected_dominant_residual_driver_summary.as_str(),
        expected_residual_interpretation,
        expected_residual_interpretation_summary.as_str(),
        expected_next_step_evidence,
        expected_next_step_evidence_summary.as_str(),
        expected_parity_claim_summary.as_str(),
    );
    if probe.readiness_criteria != expected_readiness_criteria {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the benchmark-side readiness criteria aligned with the surfaced blocker checklist."
                .to_owned(),
        ));
    }
    let expected_readiness_blocked_criteria_count =
        count_blocked_lp_bz_competitive_readiness_criteria(&expected_readiness_criteria);
    if probe.readiness_blocked_criteria_count != expected_readiness_blocked_criteria_count {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the blocked readiness-criteria count aligned with the surfaced readiness checklist."
                .to_owned(),
        ));
    }
    let expected_readiness_state =
        classify_lp_bz_competitive_readiness_state(expected_readiness_blocked_criteria_count);
    if probe.readiness_state != expected_readiness_state {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the readiness state aligned with the surfaced readiness checklist."
                .to_owned(),
        ));
    }
    let expected_readiness_summary = summarize_lp_bz_competitive_readiness(
        expected_readiness_state,
        expected_readiness_blocked_criteria_count,
        expected_remaining_blockers_summary.as_str(),
        expected_parity_claim_summary.as_str(),
    );
    if probe.readiness_summary != expected_readiness_summary {
        return Err(mine_sdk::MineError::validation(
            "multi-mine LP/BZ sidecar competitive ready-frontier probe must keep the readiness summary aligned with the surfaced readiness checklist."
                .to_owned(),
        ));
    }
    Ok(())
}

fn build_nested_shell_probe(
    model: &mine_sdk::BlockModel,
    _references_dir: &Path,
    precedence_path: &Path,
    _upit_objective_path: &Path,
    pcpsp_problem: &MinelibScheduleProblem,
    pcpsp_solution: &MinelibScheduleSolution,
    linear_index_to_row_index: &BTreeMap<usize, usize>,
    tonnage_column: &ColumnId,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    config: &DatasetConfig,
) -> Result<Option<NestedShellProbeSummary>, mine_sdk::MineError> {
    if !config.nested_shell_probe_enabled {
        return Ok(None);
    }

    let precedence_graph = read_minelib_precedence_graph(precedence_path, model)?;
    let preferred_shell_family =
        build_marvin_preferred_nested_shell_family_contract(NESTED_SHELL_PROBE_FACTOR_COUNT)?;
    let shell_artifacts = if config.dataset_id == "marvin" {
        build_marvin_phase_plan_from_revenue_factor_shells(
            model,
            &precedence_graph,
            &preferred_shell_family.revenue_factors,
            preferred_shell_family.shell_access_mode.nesting_rules(),
            &format!(
                "Nested-shell × bench probe for {} uses a bounded {}-factor revenue/cost-aware sweep rebuilt from Marvin benchmark columns; this is a reproducible stepping stone, not yet the final paper-calibrated pushback family.",
                config.dataset_id, NESTED_SHELL_PROBE_FACTOR_COUNT
            ),
        )?
    } else {
        return Ok(None);
    };
    let scheduling_problem = build_scheduling_problem_from_minelib_problem(
        &shell_artifacts.phase_plan,
        pcpsp_problem,
        config.dataset_id,
        resource_roles,
        &format!(
            "Nested-shell × bench probe for {} uses a bounded {}-factor revenue sweep over open `*.upit` + `*.prec` artifacts before routing.",
            config.dataset_id, NESTED_SHELL_PROBE_FACTOR_COUNT
        ),
    )?;
    let scheduling_artifacts = solve_decomposed_scheduling_problem(
        &scheduling_problem,
        &DecomposedSchedulingConfig::ready_frontier(),
        Metadata::new(),
    )?;
    let candidate_period_memberships = build_candidate_period_memberships(
        linear_index_to_row_index,
        model,
        &shell_artifacts.phase_plan,
        scheduling_artifacts.final_schedule(),
        tonnage_column,
    )?;
    let candidate_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &candidate_period_memberships)?;
    let candidate_pcpsp_summary =
        summarize_minelib_schedule_solution(pcpsp_problem, &candidate_solution)?;
    let mut limitations = shell_artifacts.phase_plan.limitations.clone();
    if shell_artifacts.shell_set.unique_shell_count <= 1 {
        limitations.push(
            "The bounded sweep collapsed to a single shell on this dataset, indicating that scaling already-net block values from open `*.upit` artifacts is not enough to reproduce a nested-shell family; a revenue/cost split or factor-specific weight construction is still needed."
                .to_owned(),
        );
    }

    Ok(Some(NestedShellProbeSummary {
        aggregation_strategy: preferred_shell_family.aggregation_strategy.clone(),
        revenue_factor_count: preferred_shell_family.revenue_factor_count,
        unique_shell_count: shell_artifacts.shell_set.unique_shell_count,
        limitations,
        candidate_summary: CandidateSchedulingSummary {
            selected_block_count: shell_artifacts.phase_plan.total_block_count,
            phase_count: shell_artifacts.phase_plan.phase_count,
            scheduling_unit_count: scheduling_problem.units().len(),
            temporal_candidate_objective: scheduling_artifacts
                .temporal_candidate()
                .total_objective_value(),
            temporal_candidate_discounted_objective: scheduling_artifacts
                .temporal_candidate()
                .total_discounted_objective_value(),
            routed_schedule_entry_count: scheduling_artifacts.routed_schedule().entries().len(),
            final_schedule_entry_count: scheduling_artifacts.final_schedule().entries().len(),
            final_schedule_violation_count: scheduling_artifacts
                .final_schedule()
                .violations()
                .len(),
            candidate_pcpsp_summary: candidate_pcpsp_summary.clone(),
        },
        candidate_vs_reference_metrics: compare_named_numeric_metrics(
            &solution_metric_map(&summarize_minelib_schedule_solution(
                pcpsp_problem,
                pcpsp_solution,
            )?),
            &solution_metric_map(&candidate_pcpsp_summary),
            &BTreeMap::new(),
        ),
        candidate_vs_reference_period_alignment: compare_period_alignment(
            pcpsp_solution,
            &candidate_solution,
        ),
    }))
}

fn compare_period_memberships(
    reference: &BTreeMap<String, BTreeSet<usize>>,
    candidate: &BTreeMap<String, BTreeSet<usize>>,
) -> CompactPeriodMembershipComparison {
    let reference_assignments = reference
        .iter()
        .flat_map(|(period_label, blocks)| {
            blocks
                .iter()
                .map(move |linear_index| (period_label.clone(), *linear_index))
        })
        .collect::<BTreeSet<_>>();
    let candidate_assignments = candidate
        .iter()
        .flat_map(|(period_label, blocks)| {
            blocks
                .iter()
                .map(move |linear_index| (period_label.clone(), *linear_index))
        })
        .collect::<BTreeSet<_>>();
    let shared_assignments = reference_assignments
        .intersection(&candidate_assignments)
        .count();
    let reference_only = reference_assignments
        .difference(&candidate_assignments)
        .cloned()
        .collect::<Vec<_>>();
    let candidate_only = candidate_assignments
        .difference(&reference_assignments)
        .cloned()
        .collect::<Vec<_>>();
    let union = reference_assignments.len() + candidate_only.len();
    let jaccard_index = if union == 0 {
        1.0
    } else {
        shared_assignments as f64 / union as f64
    };

    CompactPeriodMembershipComparison {
        shared_assignments,
        reference_only_assignment_count: reference_only.len(),
        candidate_only_assignment_count: candidate_only.len(),
        jaccard_index,
        reference_only_assignment_examples: reference_only.into_iter().take(10).collect(),
        candidate_only_assignment_examples: candidate_only.into_iter().take(10).collect(),
    }
}

fn compare_period_alignment(
    reference: &MinelibScheduleSolution,
    candidate: &MinelibScheduleSolution,
) -> PeriodAlignmentSummary {
    let reference_periods = representative_period_by_block(reference);
    let candidate_periods = representative_period_by_block(candidate);
    let shared_blocks = reference_periods
        .keys()
        .filter(|linear_index| candidate_periods.contains_key(linear_index))
        .copied()
        .collect::<Vec<_>>();
    let reference_only_block_count = reference_periods.len().saturating_sub(shared_blocks.len());
    let candidate_only_block_count = candidate_periods.len().saturating_sub(shared_blocks.len());
    let mut exact_period_match_count = 0usize;
    let mut earlier_than_reference_count = 0usize;
    let mut later_than_reference_count = 0usize;
    let mut absolute_period_delta_sum = 0.0;
    let mut max_absolute_period_delta = 0.0_f64;
    let mut largest_absolute_period_delta_examples = shared_blocks
        .iter()
        .map(|linear_index| {
            let reference_period_index =
                representative_period_index(reference_periods[linear_index]);
            let candidate_period_index =
                representative_period_index(candidate_periods[linear_index]);
            let absolute_period_delta = reference_period_index.abs_diff(candidate_period_index);
            (
                *linear_index,
                reference_period_index,
                candidate_period_index,
                absolute_period_delta,
            )
        })
        .collect::<Vec<_>>();

    for (_, reference_period_index, candidate_period_index, absolute_period_delta) in
        &largest_absolute_period_delta_examples
    {
        if reference_period_index == candidate_period_index {
            exact_period_match_count += 1;
        } else if candidate_period_index < reference_period_index {
            earlier_than_reference_count += 1;
        } else {
            later_than_reference_count += 1;
        }
        absolute_period_delta_sum += *absolute_period_delta as f64;
        max_absolute_period_delta = max_absolute_period_delta.max(*absolute_period_delta as f64);
    }

    largest_absolute_period_delta_examples
        .sort_by(|left, right| right.3.cmp(&left.3).then_with(|| left.0.cmp(&right.0)));

    PeriodAlignmentSummary {
        shared_block_count: shared_blocks.len(),
        reference_only_block_count,
        candidate_only_block_count,
        exact_period_match_count,
        earlier_than_reference_count,
        later_than_reference_count,
        mean_absolute_period_delta: if shared_blocks.is_empty() {
            0.0
        } else {
            absolute_period_delta_sum / shared_blocks.len() as f64
        },
        max_absolute_period_delta,
        largest_absolute_period_delta_examples: largest_absolute_period_delta_examples
            .into_iter()
            .take(10)
            .map(
                |(linear_index, reference_period_index, candidate_period_index, _)| {
                    (linear_index, reference_period_index, candidate_period_index)
                },
            )
            .collect(),
    }
}

fn representative_period_by_block(solution: &MinelibScheduleSolution) -> BTreeMap<usize, f64> {
    solution
        .assignments
        .iter()
        .filter(|assignment| assignment.fraction > 1.0e-9)
        .fold(
            BTreeMap::<usize, (f64, f64)>::new(),
            |mut acc, assignment| {
                let entry = acc.entry(assignment.linear_index).or_insert((0.0, 0.0));
                entry.0 += assignment.period_index as f64 * assignment.fraction;
                entry.1 += assignment.fraction;
                acc
            },
        )
        .into_iter()
        .map(|(linear_index, (weighted_period_sum, total_fraction))| {
            (
                linear_index,
                weighted_period_sum / total_fraction.max(1.0e-9),
            )
        })
        .collect()
}

fn representative_period_index(period_index: f64) -> usize {
    period_index.round().max(0.0) as usize
}

fn build_candidate_pcpsp_solution(
    problem: &MinelibScheduleProblem,
    period_memberships: &BTreeMap<String, BTreeSet<usize>>,
) -> Result<MinelibScheduleSolution, mine_sdk::MineError> {
    let objective_lookup = problem.objective_terms.iter().fold(
        BTreeMap::<usize, Vec<(usize, f64)>>::new(),
        |mut acc, term| {
            acc.entry(term.linear_index)
                .or_default()
                .push((term.destination_index, term.objective_value));
            acc
        },
    );
    let resource_coefficients = problem.resource_coefficients.iter().fold(
        BTreeMap::<(usize, usize, usize), f64>::new(),
        |mut acc, coefficient| {
            acc.insert(
                (
                    coefficient.linear_index,
                    coefficient.destination_index,
                    coefficient.resource_index,
                ),
                coefficient.coefficient,
            );
            acc
        },
    );

    let mut assignments = Vec::new();
    for period_index in 0..problem.period_count {
        let period_label = format!("P{:02}", period_index + 1);
        let mut blocks = period_memberships
            .get(&period_label)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| {
            let left_best = objective_lookup
                .get(left)
                .and_then(|destinations| {
                    destinations
                        .iter()
                        .map(|(_, objective)| *objective)
                        .max_by(|a, b| a.partial_cmp(b).expect("objective should be finite"))
                })
                .unwrap_or(f64::NEG_INFINITY);
            let right_best = objective_lookup
                .get(right)
                .and_then(|destinations| {
                    destinations
                        .iter()
                        .map(|(_, objective)| *objective)
                        .max_by(|a, b| a.partial_cmp(b).expect("objective should be finite"))
                })
                .unwrap_or(f64::NEG_INFINITY);
            right_best
                .partial_cmp(&left_best)
                .expect("objective should be finite")
                .then_with(|| left.cmp(right))
        });

        let mut remaining_limits = problem
            .resource_constraint_limits
            .iter()
            .filter(|limit| limit.period_index == period_index && limit.relation == 'L')
            .map(|limit| (limit.resource_index, limit.limit))
            .collect::<BTreeMap<_, _>>();

        for linear_index in blocks {
            let mut destinations =
                objective_lookup
                    .get(&linear_index)
                    .cloned()
                    .ok_or_else(|| {
                        mine_sdk::MineError::validation(format!(
                            "missing MineLib objective terms for candidate block `{linear_index}`"
                        ))
                    })?;
            destinations.sort_by(|left, right| {
                right
                    .1
                    .partial_cmp(&left.1)
                    .expect("objective should be finite")
                    .then_with(|| left.0.cmp(&right.0))
            });

            let selected_destination = destinations
                .iter()
                .find(|(destination_index, _)| {
                    remaining_limits
                        .iter()
                        .all(|(resource_index, remaining_limit)| {
                            let coefficient = resource_coefficients
                                .get(&(linear_index, *destination_index, *resource_index))
                                .copied()
                                .unwrap_or(0.0);
                            coefficient <= *remaining_limit + 1.0e-9
                        })
                })
                .map(|(destination_index, _)| *destination_index)
                .unwrap_or(destinations[0].0);

            for (resource_index, remaining_limit) in &mut remaining_limits {
                let coefficient = resource_coefficients
                    .get(&(linear_index, selected_destination, *resource_index))
                    .copied()
                    .unwrap_or(0.0);
                *remaining_limit -= coefficient;
            }

            assignments.push(MinelibScheduleAssignment {
                linear_index,
                destination_index: selected_destination,
                period_index,
                fraction: 1.0,
            });
        }
    }

    Ok(MinelibScheduleSolution {
        kind: problem.kind,
        unique_block_count: assignments
            .iter()
            .map(|assignment| assignment.linear_index)
            .collect::<BTreeSet<_>>()
            .len(),
        assignments,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DATASETS, LP_BZ_CUT_BUILDER_LABEL, LP_BZ_UNIT_GRANULARITY_LABEL,
        MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE,
        MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL,
        MARVIN_MR187_PAPERLIKE_CANDIDATE_ROLE,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        MCLAUGHLIN_LIMIT_LP_BZ_SIDECAR_VERSION, MCLAUGHLIN_LIMIT_PROMOTION_CHECKLIST_VERSION,
        NESTED_SHELL_PROBE_FACTOR_COUNT, aggregation_comparability_gap,
        build_benchmark_contract_audit, build_dataset_contract_roles,
        build_linear_index_to_row_index, build_lp_bz_baseline,
        build_marvin_lp_bz_sidecar_artifacts, build_marvin_paperlike_pipeline_checklist,
        build_marvin_preferred_nested_shell_family_contract,
        build_mclaughlin_limit_promotion_checklist,
        build_preferred_phase_plan_for_minelib_scheduling, build_primary_unit_family_traceability,
        build_promoted_pushback_bench_localized_cut_unit_family_traceability,
        parse_multi_mine_scheduler_cli_args, read_benchmark_blocks, read_minelib_cpit_solution,
        read_minelib_pcpsp_problem, read_minelib_pcpsp_solution, read_minelib_precedence_graph,
        read_minelib_upit_block_values,
        summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law,
        supports_lp_bz_baseline, temporal_solver_comparability_gap,
        validate_promoted_pushback_bench_localized_cut_access_law_contract,
    };
    use crate::benchmark_path_policy::BenchmarkPathPolicy;
    use crate::minelib_scheduling_support::MARVIN_SELECTED_BLOCK_SOURCE;
    use crate::minelib_scheduling_support::REFERENCE_SELECTED_BLOCK_SOURCE;
    use mine_sdk::{ColumnId, NestingAccessRules, PhaseDesign, PushbackPlan};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    const SAMPLE_LP_BZ_SELECTED_BLOCK_COUNT: usize = 321;

    fn repo_root_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn sample_limit_cut_readiness_phase_plan() -> PushbackPlan {
        PushbackPlan {
            phases: vec![
                PhaseDesign {
                    phase_id: "shell-01-bench-10".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(1),
                    revenue_factor: None,
                    bench: Some(10),
                    block_indices: vec![1, 2, 3],
                    block_count: 3,
                    total_tonnage: Some(30.0),
                    predecessor_phase_ids: Vec::new(),
                },
                PhaseDesign {
                    phase_id: "shell-01-bench-09".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(1),
                    revenue_factor: None,
                    bench: Some(9),
                    block_indices: vec![4],
                    block_count: 1,
                    total_tonnage: Some(10.0),
                    predecessor_phase_ids: vec!["shell-01-bench-10".to_owned()],
                },
                PhaseDesign {
                    phase_id: "shell-02-bench-09".to_owned(),
                    pushback_index: 1,
                    shell_index: Some(2),
                    revenue_factor: None,
                    bench: Some(9),
                    block_indices: vec![5, 6],
                    block_count: 2,
                    total_tonnage: Some(20.0),
                    predecessor_phase_ids: vec!["shell-01-bench-09".to_owned()],
                },
            ],
            phase_count: 3,
            total_block_count: 6,
            total_tonnage: Some(60.0),
            nesting_rules: NestingAccessRules::strict_sequential(),
            limitations: Vec::new(),
        }
    }

    fn sample_lp_bz_baseline_summary() -> super::LpBzBaselineSummary {
        let preferred_shell_family =
            build_marvin_preferred_nested_shell_family_contract(NESTED_SHELL_PROBE_FACTOR_COUNT)
                .expect("preferred shell family should build")
                .with_realized_shell_count(5);
        let ready_frontier_discounted_objective = 96.0;
        let focused_candidate_discounted_objective = 92.0;
        let local_optimizer_runtime_budget_contract =
            super::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract(
                "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8",
                12,
                2,
                "no-improving-local-move",
            );
        let phase_refinement_diagnostics = super::PushbackBenchLocalizedCutRefinementDiagnostics {
            base_phase_count: 10,
            refined_base_phase_count: 6,
            refined_single_component_phase_count: 4,
            total_cut_phase_count: 18,
            additional_phase_count: 8,
            max_cut_count_per_base_phase: 3,
            average_cut_count_per_base_phase: 1.8,
            realized_front_count_histogram: BTreeMap::from([(1, 4), (2, 3), (3, 3)]),
            readiness_reason_histogram: BTreeMap::from([
                ("paper-like-three-front-ready".to_owned(), 3),
                ("blocked-low-aspect-ratio".to_owned(), 4),
                ("refined-beyond-paper-like-three-front".to_owned(), 3),
            ]),
            exact_three_front_candidate_count: 5,
            exact_three_front_failure_count: 1,
            exact_three_front_failure_realized_front_histogram: BTreeMap::from([(2, 1)]),
            exact_three_front_failure_reason_histogram: BTreeMap::from([(
                "exact-three-front-infeasible-collapsed-target-partition".to_owned(),
                1,
            )]),
            refined_base_phase_examples: vec!["phase-a".to_owned()],
            refined_single_component_phase_examples: vec!["phase-b".to_owned()],
        };
        let mut baseline = super::LpBzBaselineSummary {
            phase_plan_builder_label: LP_BZ_CUT_BUILDER_LABEL.to_owned(),
            unit_granularity_label: LP_BZ_UNIT_GRANULARITY_LABEL.to_owned(),
            promoted_build_label: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL
                .to_owned(),
            paperlike_candidate_role: MARVIN_MR187_PAPERLIKE_CANDIDATE_ROLE.to_owned(),
            local_optimizer_scaffold_unit_family_label:
                MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL.to_owned(),
            local_optimizer_scaffold_role: MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE.to_owned(),
            unit_family_traceability:
                build_promoted_pushback_bench_localized_cut_unit_family_traceability(
                    MARVIN_SELECTED_BLOCK_SOURCE,
                    SAMPLE_LP_BZ_SELECTED_BLOCK_COUNT,
                    "nested-shell-bench",
                    Some(&preferred_shell_family),
                    phase_refinement_diagnostics.base_phase_count,
                    MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL,
                    LP_BZ_UNIT_GRANULARITY_LABEL,
                    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
                    "uniform-33-67-100",
                    phase_refinement_diagnostics.total_cut_phase_count,
                    12,
                ),
            cut_access_law: summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law(
                &phase_refinement_diagnostics,
            ),
            phase_refinement_diagnostics,
            summary: super::lp_bz_adapter::MarvinLpBzAdapterSummary {
                scope_label: "marvin-focused".to_owned(),
                lp_relaxation_assignment_count: 0,
                lp_relaxation_unique_block_count: 0,
                representative_period_block_count: 0,
                seeded_schedule_entry_count: 0,
                seeded_schedule_violation_count: 0,
                lp_bz_inputs: super::lp_bz_bound::LpBzInputArtifact {
                    problem_normalization: super::lp_bz_bound::LpBzProblemNormalization {
                        period_count: 20,
                        resource_constraint_count: 2,
                        destination_count: 2,
                        discount_rate: 0.1,
                    },
                    precedence_units: super::lp_bz_bound::LpBzPrecedenceUnits {
                        unit_count: 12,
                        edge_count: 11,
                        unit_granularity_label: LP_BZ_UNIT_GRANULARITY_LABEL.to_owned(),
                    },
                    lp_relaxation_source: super::lp_bz_bound::LpBzRelaxationSource {
                        source_label: "lp-pcpsp-native-proxy".to_owned(),
                        reference_artifact_path: "marvin.LPpcpsp".to_owned(),
                        objective_kind: "discounted_objective".to_owned(),
                    },
                },
                lp_bz_bound: super::lp_bz_bound::LpBzBoundArtifact {
                    bound_label: "lp-bz-native-resource-envelope".to_owned(),
                    discounted_objective_bound: 100.0,
                    period_count: 20,
                    resource_constraint_count: 2,
                    destination_count: 2,
                    unit_count: 12,
                    bound_strategy: "native-resource-envelope".to_owned(),
                    lp_proxy_discounted_objective: 90.0,
                    native_block_objective_upper_bound: 100.0,
                    native_resource_density_upper_bound: Some(95.0),
                    native_resource_knapsack_upper_bound: Some(98.0),
                    discount_inverse_upper_bound: 1.0,
                },
                lp_bz_lp_kernel: super::lp_bz_adapter::MarvinLpBzAdapterLpKernelSummary {
                    kernel_label: "lp-bz-lp-kernel-v8".to_owned(),
                    variable_count: 0,
                    non_zero_objective_coefficient_count: 0,
                    capacity_row_count: 0,
                    activation_row_count: 0,
                    precedence_row_count: 0,
                    access_unit_profile_count: 0,
                    limitations: Vec::new(),
                },
                lp_bz_lp_solve: super::lp_bz_adapter::MarvinLpBzAdapterLpSolveSummary {
                    solver_label: "minilp".to_owned(),
                    solve_status: super::lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal,
                    discounted_objective_bound: Some(96.0),
                    active_variable_count: 8,
                    min_positive_variable_value: Some(0.25),
                    max_variable_value: Some(1.0),
                    precedence_diagnostics:
                        super::lp_bz_lp_kernel::LpBzPrecedenceSolveDiagnostics {
                            strategy:
                                super::lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy::FullPerPeriod,
                            max_enforced_precedence_rows: 40,
                            total_precedence_rows: 40,
                            enforced_precedence_rows: 40,
                            skipped_precedence_rows: 0,
                            coverage_completeness:
                                super::lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Complete,
                            coverage_basis_points: Some(10_000),
                            enforced_period_indices: vec![0, 1, 2, 3],
                            skipped_period_indices: Vec::new(),
                        },
                    cut_diagnostics: super::lp_bz_lp_kernel::LpBzCutSolveDiagnostics {
                        strategy:
                            super::lp_bz_lp_kernel::LpBzCutTighteningStrategy::PrecedenceCumulativePrefixAndAccessClosureCapacityPrefix,
                        total_generated_row_count: 12,
                        total_applied_row_count: 12,
                        total_skipped_row_count: 0,
                        families: Vec::new(),
                    },
                    limitations: Vec::new(),
                },
                lp_bz_round_repair: super::lp_bz_adapter::MarvinLpBzAdapterRoundRepairSummary {
                    rounder_strategy_label: "round-repair-v6".to_owned(),
                    focused_round_repair: true,
                    target_score_decomposition:
                        super::lp_bz_rounder::LpBzUnitTargetScoreDecomposition {
                            rounded_discounted_target_score_proxy: 95.0,
                            repaired_discounted_target_score_proxy: 92.0,
                            local_search_discounted_target_score_proxy: 94.0,
                            repair_score_delta_vs_round_proxy: -3.0,
                            local_search_score_delta_vs_repair_proxy: 2.0,
                            local_search_score_delta_vs_round_proxy: -1.0,
                        },
                    competitive_probe: super::lp_bz_adapter::MarvinLpBzCompetitiveProbeSummary {
                        probe_strategy_label: "lp-bz-rounder-v6-full-round-repair-probe"
                            .to_owned(),
                        improvement_status:
                            "full-round-repair-probe-improves-focused-proxy".to_owned(),
                        competitive_budget_profile:
                            super::lp_bz_rounder::LpBzLocalOptimizerBudgetProfile {
                                mode_label: "full-round-repair".to_owned(),
                                target_unit_count: 4,
                                horizon_period_count: 4,
                                full_iteration_budget: 32,
                                requested_iteration_budget: 32,
                                effective_iteration_budget: 32,
                            },
                        competitive_local_optimizer_runtime_budget_contract:
                            super::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract(
                                "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8",
                                32,
                                3,
                                "no-improving-local-move",
                            ),
                        competitive_local_optimizer_strategy_label:
                            "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
                                .to_owned(),
                        competitive_local_optimizer_termination_reason:
                            "no-improving-local-move".to_owned(),
                        competitive_local_optimizer_executed_iteration_count: 3,
                        competitive_local_optimizer_improving_move_count: 2,
                        competitive_local_optimizer_residual_opportunity:
                            super::lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity {
                                improving_move_available: false,
                                move_kind_label: "none".to_owned(),
                                discounted_gain: 0.0,
                            },
                        competitive_local_search_discounted_target_score_proxy: 95.0,
                        local_search_score_delta_vs_focused_proxy: 1.0,
                        target_period_change_count_vs_focused: 1,
                    },
                    local_optimization_skipped: false,
                    local_optimizer_runtime_budget_contract:
                        local_optimizer_runtime_budget_contract.clone(),
                    local_optimizer_budget_profile:
                        super::lp_bz_rounder::LpBzLocalOptimizerBudgetProfile {
                            mode_label: "focused-refresh-budgeted".to_owned(),
                            target_unit_count: 4,
                            horizon_period_count: 4,
                            full_iteration_budget: 32,
                            requested_iteration_budget: 12,
                            effective_iteration_budget: 12,
                        },
                    local_optimizer_strategy_label:
                        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
                            .to_owned(),
                    local_optimizer_termination_reason: "no-improving-local-move".to_owned(),
                    local_optimizer_executed_iteration_count: 2,
                    local_optimizer_improving_move_count: 1,
                    local_optimizer_residual_opportunity:
                        super::lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity {
                            improving_move_available: false,
                            move_kind_label: "none".to_owned(),
                            discounted_gain: 0.0,
                        },
                    repaired_phase_target_count: 0,
                    repaired_unit_target_count: 0,
                    horizon_clamp_count: 0,
                    phase_target_count: 0,
                    unit_target_count: 0,
                    limitations: Vec::new(),
                },
                limitations: Vec::new(),
            },
            competitive_ready_frontier_probe:
                super::build_lp_bz_competitive_ready_frontier_probe_summary(
                    ready_frontier_discounted_objective,
                    focused_candidate_discounted_objective,
                    &super::lp_bz_adapter::MarvinLpBzAdapterSummary {
                        scope_label: "marvin-focused".to_owned(),
                        lp_relaxation_assignment_count: 0,
                        lp_relaxation_unique_block_count: 0,
                        representative_period_block_count: 0,
                        seeded_schedule_entry_count: 0,
                        seeded_schedule_violation_count: 0,
                        lp_bz_inputs: super::lp_bz_bound::LpBzInputArtifact {
                            problem_normalization: super::lp_bz_bound::LpBzProblemNormalization {
                                period_count: 20,
                                resource_constraint_count: 2,
                                destination_count: 2,
                                discount_rate: 0.1,
                            },
                            precedence_units: super::lp_bz_bound::LpBzPrecedenceUnits {
                                unit_count: 12,
                                edge_count: 11,
                                unit_granularity_label: LP_BZ_UNIT_GRANULARITY_LABEL.to_owned(),
                            },
                            lp_relaxation_source: super::lp_bz_bound::LpBzRelaxationSource {
                                source_label: "lp-pcpsp-native-proxy".to_owned(),
                                reference_artifact_path: "marvin.LPpcpsp".to_owned(),
                                objective_kind: "discounted_objective".to_owned(),
                            },
                        },
                        lp_bz_bound: super::lp_bz_bound::LpBzBoundArtifact {
                            bound_label: "lp-bz-native-resource-envelope".to_owned(),
                            discounted_objective_bound: 100.0,
                            period_count: 20,
                            resource_constraint_count: 2,
                            destination_count: 2,
                            unit_count: 12,
                            bound_strategy: "native-resource-envelope".to_owned(),
                            lp_proxy_discounted_objective: 90.0,
                            native_block_objective_upper_bound: 100.0,
                            native_resource_density_upper_bound: Some(95.0),
                            native_resource_knapsack_upper_bound: Some(98.0),
                            discount_inverse_upper_bound: 1.0,
                        },
                        lp_bz_lp_kernel: super::lp_bz_adapter::MarvinLpBzAdapterLpKernelSummary {
                            kernel_label: "lp-bz-lp-kernel-v8".to_owned(),
                            variable_count: 0,
                            non_zero_objective_coefficient_count: 0,
                            capacity_row_count: 0,
                            activation_row_count: 0,
                            precedence_row_count: 0,
                            access_unit_profile_count: 0,
                            limitations: Vec::new(),
                        },
                        lp_bz_lp_solve: super::lp_bz_adapter::MarvinLpBzAdapterLpSolveSummary {
                            solver_label: "minilp".to_owned(),
                            solve_status: super::lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal,
                            discounted_objective_bound: Some(96.0),
                            active_variable_count: 8,
                            min_positive_variable_value: Some(0.25),
                            max_variable_value: Some(1.0),
                            precedence_diagnostics:
                                super::lp_bz_lp_kernel::LpBzPrecedenceSolveDiagnostics {
                                    strategy:
                                        super::lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy::FullPerPeriod,
                                    max_enforced_precedence_rows: 40,
                                    total_precedence_rows: 40,
                                    enforced_precedence_rows: 40,
                                    skipped_precedence_rows: 0,
                                    coverage_completeness:
                                        super::lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Complete,
                                    coverage_basis_points: Some(10_000),
                                    enforced_period_indices: vec![0, 1, 2, 3],
                                    skipped_period_indices: Vec::new(),
                                },
                            cut_diagnostics: super::lp_bz_lp_kernel::LpBzCutSolveDiagnostics {
                                strategy:
                                    super::lp_bz_lp_kernel::LpBzCutTighteningStrategy::PrecedenceCumulativePrefixAndAccessClosureCapacityPrefix,
                                total_generated_row_count: 12,
                                total_applied_row_count: 12,
                                total_skipped_row_count: 0,
                                families: Vec::new(),
                            },
                            limitations: Vec::new(),
                        },
                        lp_bz_round_repair: super::lp_bz_adapter::MarvinLpBzAdapterRoundRepairSummary {
                            rounder_strategy_label: "round-repair-v6".to_owned(),
                            focused_round_repair: true,
                            target_score_decomposition:
                                super::lp_bz_rounder::LpBzUnitTargetScoreDecomposition {
                                    rounded_discounted_target_score_proxy: 95.0,
                                    repaired_discounted_target_score_proxy: 92.0,
                                    local_search_discounted_target_score_proxy: 94.0,
                                    repair_score_delta_vs_round_proxy: -3.0,
                                    local_search_score_delta_vs_repair_proxy: 2.0,
                                    local_search_score_delta_vs_round_proxy: -1.0,
                                },
                            competitive_probe: super::lp_bz_adapter::MarvinLpBzCompetitiveProbeSummary {
                                probe_strategy_label: "lp-bz-rounder-v6-full-round-repair-probe"
                                    .to_owned(),
                                improvement_status:
                                    "full-round-repair-probe-improves-focused-proxy".to_owned(),
                                competitive_budget_profile:
                                    super::lp_bz_rounder::LpBzLocalOptimizerBudgetProfile {
                                        mode_label: "full-round-repair".to_owned(),
                                        target_unit_count: 4,
                                        horizon_period_count: 4,
                                        full_iteration_budget: 32,
                                        requested_iteration_budget: 32,
                                        effective_iteration_budget: 32,
                                    },
                                competitive_local_optimizer_runtime_budget_contract:
                                    super::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract(
                                        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8",
                                        32,
                                        3,
                                        "no-improving-local-move",
                                    ),
                                competitive_local_optimizer_strategy_label:
                                    "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
                                        .to_owned(),
                                competitive_local_optimizer_termination_reason:
                                    "no-improving-local-move".to_owned(),
                                competitive_local_optimizer_executed_iteration_count: 3,
                                competitive_local_optimizer_improving_move_count: 2,
                                competitive_local_optimizer_residual_opportunity:
                                    super::lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity {
                                        improving_move_available: false,
                                        move_kind_label: "none".to_owned(),
                                        discounted_gain: 0.0,
                                    },
                                competitive_local_search_discounted_target_score_proxy: 100.0,
                                local_search_score_delta_vs_focused_proxy: 2.0,
                                target_period_change_count_vs_focused: 1,
                            },
                            local_optimization_skipped: false,
                            local_optimizer_runtime_budget_contract:
                                super::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract(
                                    "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8",
                                    12,
                                    2,
                                    "no-improving-local-move",
                                ),
                            local_optimizer_budget_profile:
                                super::lp_bz_rounder::LpBzLocalOptimizerBudgetProfile {
                                    mode_label: "focused-refresh-budgeted".to_owned(),
                                    target_unit_count: 4,
                                    horizon_period_count: 4,
                                    full_iteration_budget: 32,
                                    requested_iteration_budget: 12,
                                    effective_iteration_budget: 12,
                                },
                            local_optimizer_strategy_label:
                                "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
                                    .to_owned(),
                            local_optimizer_termination_reason: "no-improving-local-move".to_owned(),
                            local_optimizer_executed_iteration_count: 2,
                            local_optimizer_improving_move_count: 1,
                            local_optimizer_residual_opportunity:
                                super::lp_bz_rounder::LpBzLocalOptimizerResidualOpportunity {
                                    improving_move_available: false,
                                    move_kind_label: "none".to_owned(),
                                    discounted_gain: 0.0,
                                },
                            repaired_phase_target_count: 0,
                            repaired_unit_target_count: 0,
                            horizon_clamp_count: 0,
                            phase_target_count: 0,
                            unit_target_count: 0,
                            limitations: Vec::new(),
                        },
                        limitations: Vec::new(),
                    },
                ),
            lp_bz_promotion_readiness: super::build_lp_bz_promotion_readiness_summary(
                "exploratory-local",
                LP_BZ_UNIT_GRANULARITY_LABEL,
                MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
                false,
                false,
                &local_optimizer_runtime_budget_contract,
            ),
            candidate_pcpsp_summary: super::marvin_support::MinelibScheduleSolutionSummary {
                assignment_count: 0,
                unique_block_count: 0,
                fractional_assignment_count: 0,
                used_period_count: 0,
                used_destination_count: 0,
                total_fraction: 0.0,
                min_block_fraction_sum: 0.0,
                max_block_fraction_sum: 0.0,
                undiscounted_objective: focused_candidate_discounted_objective,
                discounted_objective: focused_candidate_discounted_objective,
                resource_summaries: Vec::new(),
            },
            candidate_vs_reference_metrics: mine_sdk::NumericMetricComparisonReport {
                shared_metrics: Vec::new(),
                reference_only_metrics: Vec::new(),
                candidate_only_metrics: Vec::new(),
            },
            candidate_vs_reference_period_alignment: super::PeriodAlignmentSummary {
                shared_block_count: 0,
                reference_only_block_count: 0,
                candidate_only_block_count: 0,
                exact_period_match_count: 0,
                earlier_than_reference_count: 0,
                later_than_reference_count: 0,
                mean_absolute_period_delta: 0.0,
                max_absolute_period_delta: 0.0,
                largest_absolute_period_delta_examples: Vec::new(),
            },
            candidate_vs_reference_destination_membership:
                super::CompactPeriodMembershipComparison {
                    shared_assignments: 0,
                    reference_only_assignment_count: 0,
                    candidate_only_assignment_count: 0,
                    jaccard_index: 1.0,
                    reference_only_assignment_examples: Vec::new(),
                    candidate_only_assignment_examples: Vec::new(),
                },
            temporal_routing_promotion_gate:
                super::build_temporal_routing_promotion_gate_summary(0.0, 0.0, 0, 0, 0.0, 0, 1.0),
        };
        baseline.competitive_ready_frontier_probe =
            super::build_lp_bz_competitive_ready_frontier_probe_summary(
                ready_frontier_discounted_objective,
                focused_candidate_discounted_objective,
                &baseline.summary,
            );
        baseline
    }

    fn sample_lp_bz_baseline_summary_with_termination(
        executed_iteration_count: usize,
        termination_reason: &str,
    ) -> super::LpBzBaselineSummary {
        let mut baseline = sample_lp_bz_baseline_summary();
        let max_iteration_count = baseline
            .summary
            .lp_bz_round_repair
            .local_optimizer_runtime_budget_contract
            .max_iteration_count;
        let runtime_budget_contract =
            super::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract(
                &baseline
                    .summary
                    .lp_bz_round_repair
                    .local_optimizer_strategy_label,
                max_iteration_count,
                executed_iteration_count,
                termination_reason,
            );
        baseline
            .summary
            .lp_bz_round_repair
            .local_optimization_skipped =
            super::lp_bz_rounder::local_optimizer_runtime_was_skipped(termination_reason);
        baseline
            .summary
            .lp_bz_round_repair
            .local_optimizer_runtime_budget_contract = runtime_budget_contract.clone();
        baseline
            .summary
            .lp_bz_round_repair
            .local_optimizer_budget_profile
            .effective_iteration_budget = max_iteration_count;
        baseline
            .summary
            .lp_bz_round_repair
            .local_optimizer_termination_reason = termination_reason.to_owned();
        baseline
            .summary
            .lp_bz_round_repair
            .local_optimizer_executed_iteration_count = executed_iteration_count;
        baseline.lp_bz_promotion_readiness = super::build_lp_bz_promotion_readiness_summary(
            "exploratory-local",
            LP_BZ_UNIT_GRANULARITY_LABEL,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
            false,
            false,
            &runtime_budget_contract,
        );
        baseline.competitive_ready_frontier_probe =
            super::build_lp_bz_competitive_ready_frontier_probe_summary(
                baseline
                    .competitive_ready_frontier_probe
                    .ready_frontier_discounted_objective,
                baseline.candidate_pcpsp_summary.discounted_objective,
                &baseline.summary,
            );
        baseline
    }

    #[test]
    fn promoted_nested_shell_gap_mentions_dynamic_strategy_and_shell_count() {
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred Marvin shell family should build")
            .with_realized_shell_count(5);
        let gap = aggregation_comparability_gap(
            "marvin",
            "nested-shell-bench",
            Some(&preferred_shell_family),
            None,
            false,
        );

        assert!(gap.contains("nested-shell-bench"));
        assert!(gap.contains("5 bounded shells"));
        assert!(gap.contains("7-factor strict-sequential family"));
    }

    #[test]
    fn reported_probe_gap_stays_on_reference_period_primary_pipeline() {
        let gap =
            aggregation_comparability_gap("marvin", "reference-period-bench", None, None, true);

        assert!(gap.contains("reference-period × bench"));
        assert!(gap.contains("nested-shell × bench probe is reported"));
    }

    #[test]
    fn promoted_open_upit_gap_mentions_proxy_nested_shell_route() {
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            None,
            None,
            12,
        );
        let gap = aggregation_comparability_gap(
            "mclaughlin-limit",
            "nested-shell-bench",
            None,
            Some(&traceability),
            false,
        );

        assert!(gap.contains("nested-shell × bench"));
        assert!(gap.contains("open `*.upit` block values"));
        assert!(gap.contains("pushback-equivalent mining units"));
        assert!(gap.contains("321 selected blocks"));
        assert!(gap.contains("10 shell×bench phases"));
        assert!(gap.contains("12 scheduling units"));
        assert!(gap.contains("bounded reproducible proxy"));
    }

    #[test]
    fn fallback_gap_without_probe_keeps_reference_period_message() {
        let gap =
            aggregation_comparability_gap("marvin", "reference-period-bench", None, None, false);

        assert!(gap.contains("reference-period × bench units are still derived"));
        assert!(!gap.contains("probe is reported"));
    }

    #[test]
    fn benchmark_contract_audit_surfaces_preferred_shell_family_contract() {
        let audit = build_benchmark_contract_audit();
        let scheduling_support = audit
            .modules
            .iter()
            .find(|module| {
                module.module_path == "examples/marvin-benchmark/src/minelib_scheduling_support.rs"
            })
            .expect("minelib scheduling support module should be audited");

        assert!(
            scheduling_support
                .report_surface
                .iter()
                .any(|field| field == "datasets[*].preferred_nested_shell_family_contract")
        );
        assert!(
            scheduling_support
                .report_surface
                .iter()
                .any(|field| field == "datasets[*].primary_unit_family_traceability")
        );
        assert!(
            scheduling_support
                .report_surface
                .iter()
                .any(|field| field == "datasets[*].marvin_paperlike_pipeline_checklist")
        );
        assert!(
            scheduling_support
                .report_surface
                .iter()
                .any(|field| field == "datasets[*].mclaughlin_limit_promotion_checklist")
        );
        assert!(
            scheduling_support
                .report_surface
                .iter()
                .any(|field| field == "datasets[*].temporal_routing_promotion_gate")
        );
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.variant_scope_label"
        }));
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites"
        }));
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_exit_criteria"
        }));
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_blocking_prerequisite_ids"
        }));
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_ready"
        }));
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_prerequisites"
        }));
        assert!(scheduling_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_exit_criteria"
        }));
    }

    #[test]
    fn marvin_contract_audit_surfaces_grouped_traceability_fields() {
        let audit = build_benchmark_contract_audit();
        let localized_cut_support = audit
            .modules
            .iter()
            .find(|module| {
                module.module_path
                    == "examples/marvin-benchmark/src/pushback_bench_localized_cut_support.rs"
            })
            .expect("localized cut support module should be audited");

        assert!(localized_cut_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_source"
        }));
        assert!(localized_cut_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_count"
        }));
        assert!(localized_cut_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.unit_family_traceability.preferred_phase_plan_proxy.aggregation_strategy"
        }));
        assert!(localized_cut_support.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.front_progression_label"
        }));
        assert!(localized_cut_support.limitations.iter().any(|entry| {
            entry.contains(LP_BZ_UNIT_GRANULARITY_LABEL)
                && entry.contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL)
                && entry.contains(MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL)
        }));
        let lp_bz_adapter = audit
            .modules
            .iter()
            .find(|module| module.module_path == "examples/marvin-benchmark/src/lp_bz_adapter.rs")
            .expect("lp/bz adapter module should be audited");
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field == "datasets[*].lp_bz_baseline.summary.lp_bz_lp_kernel.kernel_label"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field == "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.solve_status"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_residual_opportunity.discounted_gain"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.improvement_status"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_termination_reason"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.execution_state"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.budget_hit"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_executed_iteration_count"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.local_search_score_delta_vs_focused_proxy"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.closure_status"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.competitive_probe_proxy_gap_closure_share"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.empirical_dominant_blocker"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.empirical_driver_evidence"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.budget_coverage_experiment"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation_summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver_summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence_summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.remaining_blocker_count"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers_summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_state"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_summary"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.competitive_ready_frontier_probe.readiness_criteria"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_discounted_target_score_proxy"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.local_search_score_delta_vs_round_proxy"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field == "datasets[*].lp_bz_baseline.temporal_routing_promotion_gate"
        }));
    }

    #[test]
    fn marvin_diagnostics_schema_surfaces_grouped_traceability_fields() {
        let schema = super::build_benchmark_diagnostics_schema();
        let paperlike_group = schema
            .required_groups
            .iter()
            .find(|group| group.group_name == "paperlike-pipeline-checklist")
            .expect("paperlike diagnostics group should exist");

        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_source"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_count"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.unit_family_traceability.preferred_phase_plan_proxy.preferred_nested_shell_access_mode"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.promoted_unit_family_label"
        }));
        assert!(
            paperlike_group
                .fields
                .iter()
                .any(|field| { field == "lp_bz_baseline.summary.lp_bz_lp_kernel.kernel_label" })
        );
        assert!(
            paperlike_group
                .fields
                .iter()
                .any(|field| { field == "lp_bz_baseline.summary.lp_bz_lp_solve.solve_status" })
        );
        let comparability_group = schema
            .required_groups
            .iter()
            .find(|group| group.group_name == "comparability")
            .expect("comparability diagnostics group should exist");
        assert!(
            comparability_group
                .fields
                .iter()
                .any(|field| { field == "primary_unit_family_traceability" })
        );
        assert!(
            paperlike_group.fields.iter().any(|field| {
                field
                    == "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness"
            })
        );
        assert!(
            paperlike_group.fields.iter().any(|field| {
                field
                    == "lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points"
            })
        );
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.improvement_status"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_termination_reason"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.execution_state"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.budget_hit"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_runtime_budget_contract.summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.closure_status"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.competitive_ready_frontier_probe.residual_ready_frontier_gap_after_competitive_probe_proxy"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.empirical_dominant_blocker"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.empirical_driver_evidence"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.budget_coverage_experiment"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.competitive_ready_frontier_probe.residual_interpretation_summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver_summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.next_step_evidence_summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blocker_count"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers_summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.remaining_blockers"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.readiness_state"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.readiness_summary"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field == "lp_bz_baseline.competitive_ready_frontier_probe.readiness_criteria"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.target_score_decomposition.repaired_discounted_target_score_proxy"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
        }));
        assert!(
            paperlike_group
                .fields
                .iter()
                .any(|field| { field == "lp_bz_baseline.temporal_routing_promotion_gate" })
        );
        let limit_group = schema
            .required_groups
            .iter()
            .find(|group| group.group_name == "mclaughlin-limit-promotion-checklist")
            .expect("mclaughlin-limit promotion group should exist");
        assert!(
            limit_group
                .fields
                .iter()
                .any(|field| { field == "mclaughlin_limit_promotion_checklist" })
        );
        assert!(
            limit_group
                .fields
                .iter()
                .any(|field| { field == "mclaughlin_limit_lp_bz_sidecar" })
        );
        assert!(
            limit_group
                .fields
                .iter()
                .any(|field| { field == "primary_unit_family_traceability" })
        );
        assert!(
            limit_group
                .fields
                .iter()
                .any(|field| { field == "temporal_routing_promotion_gate" })
        );
        assert!(
            limit_group
                .fields
                .iter()
                .any(|field| { field == "benchmark_contract_roles" })
        );
    }

    #[test]
    fn mclaughlin_limit_promotion_checklist_surfaces_scaffold_and_stress_split() {
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            None,
            None,
            12,
        );
        let checklist = build_mclaughlin_limit_promotion_checklist(
            &traceability,
            "exploratory-local",
            &["gap-a".to_owned(), "gap-b".to_owned()],
            &super::build_temporal_routing_promotion_gate_summary(
                500.0, 700.0, 10, 14, 2.5, 42, 0.11,
            ),
        );

        assert_eq!(checklist.checklist_label, "mclaughlin-limit-promotion-path");
        assert_eq!(
            checklist.checklist_version,
            MCLAUGHLIN_LIMIT_PROMOTION_CHECKLIST_VERSION
        );
        assert_eq!(checklist.items.len(), 6);
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "pushback-equivalent-input-traceability"
                && item.summary.contains("selected_block_source = \"mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases\"")
                && item.summary.contains("321 selected blocks")
                && item.summary.contains("10 phase-plan proxy units")
                && item.summary.contains("12 scheduling units")
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "pushback-equivalent-bench-cut-readiness"
                && item.status == "audited"
                && item
                    .summary
                    .contains("pushback-equivalent-bench-cut-readiness")
                && item.summary.contains("multi-block refinement candidates")
                && item.evidence_fields.contains(
                    &"primary_unit_family_traceability.benchmark_side_evidence.cut_readiness"
                        .to_owned(),
                )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "benchmark-side-mining-cut-refinement"
                && item.status == "scaffold-only-not-implemented"
                && item
                    .summary
                    .contains("mclaughlin-limit-cut-sidecar-scaffold")
                && item
                    .summary
                    .contains("benchmark-side-mining-cut-refinement")
                && item.summary.contains("scaffold-only-not-implemented")
                && item.summary.contains("cut promotion ready = false")
                && item
                    .summary
                    .contains("benchmark-side-mining-cut-refinement-implementation")
                && item
                    .summary
                    .contains("benchmark-side-mining-cut-refinement-promotion-rule")
                && item
                    .summary
                    .contains("all-listed-prerequisites-must-clear")
                && item
                    .summary
                    .contains("publish-benchmark-cut-readiness-traceability=satisfied")
                && item
                    .summary
                    .contains("version-benchmark-cut-refinement-contract=blocked")
                && item
                    .summary
                    .contains("pushback-equivalent-bench-cut-readiness=audited")
                && item
                    .summary
                    .contains("benchmark-side-mining-cut-refinement-implementation=blocked")
                && item.evidence_fields.contains(
                    &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_promotion_rule"
                        .to_owned(),
                )
                && item.evidence_fields.contains(
                    &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_exit_criteria"
                        .to_owned(),
                )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "lp-bz-sidecar-input-family"
                && item.status == "scaffold-only"
                && item.summary.contains("lp-bz-sidecar-input-family")
                && item.summary.contains("sidecar promotion ready = false")
                && item
                    .summary
                    .contains("lp-bz-sidecar-input-family-promotion-rule")
                && item.summary.contains(
                    "temporal-routing-gate-becomes-binding-once-sidecar-exists"
                )
                && item.summary.contains("MR-206 gate")
                && item
                    .summary
                    .contains("advisory until an actual LP/BZ-side candidate exists")
                && item.summary.contains("preserve-limit-only-scope=satisfied")
                && item
                    .summary
                    .contains("version-mclaughlin-limit-lp-bz-sidecar=blocked")
                && item
                    .summary
                    .contains("lp-bz-sidecar-input-family-implementation=blocked")
                && item.evidence_fields.contains(
                    &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_promotion_rule"
                        .to_owned(),
                )
                && item.evidence_fields.contains(
                    &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_exit_criteria"
                        .to_owned(),
                )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "stress-only-full-variant-separation"
                && item.summary.contains("mclaughlin-full")
                && item.summary.contains("stress-only local variant")
                && item.evidence_fields.contains(
                    &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.variant_scope_summary"
                        .to_owned(),
                )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "comparability-classification"
                && item.status == "blocked"
                && item.summary.contains("exploratory-local")
                && item.summary.contains("2 explicit comparability gaps")
                && item.summary.contains("no-benchmark-cut-refinement")
                && item.summary.contains("no-lp-bz-sidecar")
        }));
    }

    #[test]
    fn mclaughlin_limit_partial_sidecar_updates_scaffold_checklist() {
        let sidecar = sample_mclaughlin_limit_lp_bz_sidecar_summary();
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            None,
            Some(&sidecar),
            12,
        );
        let checklist = build_mclaughlin_limit_promotion_checklist(
            &traceability,
            "exploratory-local",
            &["gap-a".to_owned(), "gap-b".to_owned()],
            &super::build_temporal_routing_promotion_gate_summary(
                500.0, 700.0, 10, 14, 2.5, 42, 0.11,
            ),
        );

        assert_eq!(
            checklist.checklist_version,
            MCLAUGHLIN_LIMIT_PROMOTION_CHECKLIST_VERSION
        );
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "lp-bz-sidecar-input-family"
                && item.status == "partial-bound-available"
                && item.summary.contains("partial-bound-available")
                && item.summary.contains("MR-206 gate")
                && item.summary.contains("now auditable")
                && item
                    .summary
                    .contains("lp-bz-sidecar-input-family-implementation=blocked")
                && item.summary.contains("partial diagnostic evidence")
                && item
                    .evidence_fields
                    .contains(&"mclaughlin_limit_lp_bz_sidecar".to_owned())
        }));
        let future_scaffold = traceability
            .benchmark_side_evidence
            .future_scaffold
            .as_ref()
            .expect("partial sidecar should still surface future scaffold");
        assert_eq!(
            future_scaffold.lp_bz_sidecar_contract_status,
            "partial-bound-available"
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids,
            vec![
                "benchmark-side-mining-cut-refinement".to_owned(),
                "lp-bz-sidecar-input-family-implementation".to_owned(),
            ]
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("contract step `partial-bound-available`")
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("MR-206 is now auditable")
        );
        assert!(
            future_scaffold
                .variant_scope_summary
                .contains("mclaughlin-full")
        );
        assert_eq!(
            traceability.benchmark_side_evidence.sidecar_evidence_label,
            "mclaughlin-limit-lp-bz-bound-sidecar"
        );
        assert!(
            traceability
                .benchmark_side_evidence
                .sidecar_evidence_summary
                .contains("diagnostic-only")
        );
    }

    #[test]
    fn implemented_limit_cut_contract_promotes_traceability_without_promoting_lp_bz_sidecar() {
        let cut_contract = sample_mclaughlin_limit_benchmark_cut_refinement_summary();
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            Some(&cut_contract),
            None,
            12,
        );

        assert_eq!(
            traceability.benchmark_side_evidence.cut_evidence_label,
            "mclaughlin-limit-benchmark-cut-refinement"
        );
        assert_eq!(
            traceability
                .benchmark_side_evidence
                .benchmark_cut_refinement
                .as_ref()
                .expect("implemented cut contract should be surfaced")
                .contract_status,
            "benchmark-side-implemented"
        );
        let future_scaffold = traceability
            .benchmark_side_evidence
            .future_scaffold
            .as_ref()
            .expect("implemented cut contract should keep scaffold state for LP/BZ promotion");
        assert_eq!(
            future_scaffold.benchmark_cut_contract_status,
            "benchmark-side-implemented"
        );
        assert!(future_scaffold.benchmark_cut_promotion_ready);
        assert_eq!(
            future_scaffold
                .benchmark_cut_blocking_prerequisite_ids
                .len(),
            0
        );
        assert_eq!(
            future_scaffold.outstanding_gap_labels,
            vec![
                "benchmark-cut-remains-benchmark-side".to_owned(),
                "no-lp-bz-sidecar".to_owned(),
            ]
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids,
            vec!["lp-bz-sidecar-input-family-implementation".to_owned()]
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "promote-benchmark-cut-refinement-first"
                        && criterion.status == "satisfied"
                })
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "version-mclaughlin-limit-lp-bz-sidecar"
                        && criterion.status == "blocked"
                })
        );
    }

    #[test]
    fn implemented_limit_cut_contract_with_partial_sidecar_keeps_lp_bz_promotion_blocked() {
        let cut_contract = sample_mclaughlin_limit_benchmark_cut_refinement_summary();
        let sidecar = sample_mclaughlin_limit_lp_bz_sidecar_summary();
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            Some(&cut_contract),
            Some(&sidecar),
            12,
        );

        let future_scaffold = traceability
            .benchmark_side_evidence
            .future_scaffold
            .as_ref()
            .expect("implemented cut contract plus partial sidecar should keep scaffold state");
        assert_eq!(
            future_scaffold.benchmark_cut_contract_status,
            "benchmark-side-implemented"
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_contract_status,
            "partial-bound-available"
        );
        assert!(future_scaffold.benchmark_cut_promotion_ready);
        assert!(!future_scaffold.lp_bz_sidecar_promotion_ready);
        assert_eq!(
            future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids,
            vec!["lp-bz-sidecar-input-family-implementation".to_owned()]
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "promote-benchmark-cut-refinement-first"
                        && criterion.status == "satisfied"
                })
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "version-mclaughlin-limit-lp-bz-sidecar"
                        && criterion.status == "blocked"
                        && criterion.current_state.contains("partial-bound-available")
                })
        );
    }

    #[test]
    fn marvin_pipeline_checklist_builder_surfaces_shell_cut_and_classification_contracts() {
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred Marvin shell family should build")
            .with_realized_shell_count(5);
        let checklist = build_marvin_paperlike_pipeline_checklist(
            &preferred_shell_family,
            &sample_lp_bz_baseline_summary(),
            "exploratory-local",
            &["gap-a".to_owned(), "gap-b".to_owned()],
        );

        assert_eq!(checklist.pipeline_label, "marvin-paperlike-pipeline");
        assert_eq!(checklist.checklist_version, "mr189-v2");
        assert_eq!(checklist.items.len(), 8);
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "preferred-shell-family-contract"
                && item.summary.contains("5 realized shells")
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "localized-cut-readiness-diagnostics"
                && item
                    .evidence_fields
                    .contains(&"lp_bz_baseline.phase_refinement_diagnostics".to_owned())
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "input-aggregation-traceability"
                && item
                    .summary
                    .contains(&format!(
                        "selected_block_source = \"{MARVIN_SELECTED_BLOCK_SOURCE}\""
                    ))
                && item.summary.contains(
                    "Quantitatively, 321 selected blocks currently lift into 10 shell×bench pushback phases, which then refine into 18 promoted mining-cut phases and 12 LP/BZ scheduling units."
                )
                && item.summary.contains("nested-shell-bench")
                && item
                    .summary
                    .contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL)
                && item
                    .summary
                    .contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL)
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_source"
                            .to_owned(),
                    )
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_count"
                            .to_owned(),
                    )
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.preferred_phase_plan_proxy.aggregation_strategy"
                            .to_owned(),
                    )
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.unit_family_traceability.preferred_phase_plan_proxy.preferred_phase_count"
                        .to_owned(),
                )
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.phase_refinement_diagnostics.total_cut_phase_count".to_owned(),
                )
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.summary.lp_bz_inputs.precedence_units.unit_count".to_owned(),
                )
                && item
                    .evidence_fields
                    .contains(&"preferred_nested_shell_family_contract".to_owned())
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.promoted_unit_family_label"
                            .to_owned(),
                    )
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.promoted_cut_phase_count"
                            .to_owned(),
                    )
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.scheduling_unit_count"
                            .to_owned(),
                    )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "promoted-lp-bz-runtime-solve-path"
                    && item.summary.contains("Kernel `lp-bz-lp-kernel-v8`")
                    && item.summary.contains("native LP solve status `optimal`")
                    && item.summary.contains("full_per_period precedence coverage `complete` (100.00%; enforced 40/40 rows, skipped 0)")
                    && item.summary.contains("runtime contract stays `completed-within-budget`")
                    && item.summary.contains("`budget_hit=false`")
                    && item
                        .summary
                        .contains("completed within the explicit iteration budget")
                    && item
                        .evidence_fields
                        .contains(&"lp_bz_baseline.summary.lp_bz_lp_kernel.kernel_label".to_owned())
                    && item
                        .evidence_fields
                        .contains(&"lp_bz_baseline.summary.lp_bz_lp_solve.solve_status".to_owned())
                    && item.evidence_fields.contains(
                        &"lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_completeness"
                            .to_owned(),
                    )
                    && item.evidence_fields.contains(
                        &"lp_bz_baseline.summary.lp_bz_lp_solve.precedence_diagnostics.coverage_basis_points"
                            .to_owned(),
                    )
                    && item.evidence_fields.contains(
                        &"lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
                            .to_owned(),
                    )
                    && item.evidence_fields.contains(
                        &"lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
                            .to_owned(),
                    )
                    && item.evidence_fields.contains(
                        &"lp_bz_baseline.lp_bz_promotion_readiness.summary".to_owned(),
                    )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "temporal-routing-promotion-gate"
                && item.summary.contains("MR-206 temporal/routing gate")
                && item.summary.contains("used_period_count")
                && item.summary.contains("mean_absolute_period_delta")
                && item.summary.contains("earlier_than_reference_count")
                && item.summary.contains("(period,destination) similarity")
                && item
                    .evidence_fields
                    .contains(&"lp_bz_baseline.temporal_routing_promotion_gate".to_owned())
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "promoted-paperlike-lp-bz-family"
                && item
                    .summary
                    .contains(&format!(
                        "selected_block_source = \"{MARVIN_SELECTED_BLOCK_SOURCE}\""
                    ))
                && item
                    .summary
                    .contains(&format!("{SAMPLE_LP_BZ_SELECTED_BLOCK_COUNT} selected blocks"))
                && item
                    .summary
                    .contains("nested-shell-bench")
                && item
                    .summary
                    .contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL)
                && item.summary.contains("shape-gated-local-front-phase")
                && item
                    .evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.localized_cut_builder_build_label"
                            .to_owned(),
                    )
                && item
                    .evidence_fields
                    .contains(&"lp_bz_baseline.lp_bz_promotion_readiness".to_owned())
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract"
                        .to_owned(),
                )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "localized-cut-bibliographic-gap-contract"
                && item.summary.contains("ramp-access-sequencing")
                && item
                    .summary
                    .contains("working-width-minimum-operating-width")
                && item.summary.contains("cut-design-lineage-bench-continuity")
                && item.summary.contains("complete-cut-design-law")
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.cut_access_law.bibliographic_gap_contract".to_owned(),
                )
                && item
                    .evidence_fields
                    .contains(&"lp_bz_baseline.cut_access_law.ramp_access_contract".to_owned())
                && item
                    .evidence_fields
                    .contains(&"lp_bz_baseline.cut_access_law.working_width_contract".to_owned())
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.cut_access_law.lineage_bench_continuity_contract".to_owned(),
                )
                && item.evidence_fields.contains(
                    &"lp_bz_baseline.cut_access_law.complete_cut_design_contract".to_owned(),
                )
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "comparability-classification"
                && item.summary.contains("exploratory-local")
                && item.summary.contains("2 explicit comparability gaps")
        }));
    }

    #[test]
    fn sidecar_sample_reports_real_local_optimizer_runtime() {
        let baseline = sample_lp_bz_baseline_summary();
        let round_repair = &baseline.summary.lp_bz_round_repair;

        assert_eq!(
            round_repair.local_optimizer_strategy_label,
            "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
        );
        assert!(round_repair.local_optimizer_executed_iteration_count > 0);
        assert!(!super::lp_bz_rounder::local_optimizer_runtime_was_skipped(
            &round_repair.local_optimizer_termination_reason
        ));
        assert_eq!(
            round_repair
                .local_optimizer_runtime_budget_contract
                .execution_state,
            "completed-within-budget"
        );
        assert_eq!(
            round_repair.local_optimizer_budget_profile.mode_label,
            "focused-refresh-budgeted"
        );
        assert!(
            !round_repair
                .local_optimizer_residual_opportunity
                .improving_move_available
        );
        assert_eq!(
            round_repair
                .local_optimizer_residual_opportunity
                .move_kind_label,
            "none"
        );
        assert_eq!(
            round_repair
                .local_optimizer_budget_profile
                .effective_iteration_budget,
            round_repair
                .local_optimizer_runtime_budget_contract
                .max_iteration_count
        );
    }

    #[test]
    fn sidecar_promotion_readiness_keeps_native_lp_complete() {
        let baseline = sample_lp_bz_baseline_summary();
        let readiness = &baseline.lp_bz_promotion_readiness;

        super::validate_lp_bz_promotion_readiness_summary(readiness)
            .expect("sidecar promotion readiness should validate");
        assert_eq!(readiness.promotion_state, "active-candidate");
        assert_eq!(readiness.comparison_classification, "exploratory-local");
        assert!(readiness.blocking_reasons.is_empty());
        assert_eq!(
            readiness
                .local_optimizer_runtime_budget_contract
                .execution_state,
            "completed-within-budget"
        );
        assert_eq!(
            readiness.local_optimizer_runtime_budget_contract,
            baseline
                .summary
                .lp_bz_round_repair
                .local_optimizer_runtime_budget_contract
        );
        assert!(
            readiness
                .summary
                .contains("active benchmark-side candidate route")
        );
        assert!(!readiness.summary.contains("native LP solve is skipped"));
        assert!(!readiness.summary.contains("local optimization is skipped"));
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_surfaces_gap_closure_slice() {
        let baseline = sample_lp_bz_baseline_summary();
        let probe = &baseline.competitive_ready_frontier_probe;

        super::validate_lp_bz_baseline_runtime_budget_contract(&baseline)
            .expect("competitive ready-frontier probe should validate");
        assert_eq!(
            probe.driver_targeting_status,
            "candidate-vs-ready-frontier-gap-active"
        );
        assert_eq!(
            probe.closure_status,
            "competitive-probe-proxy-partially-closes-ready-frontier-gap"
        );
        assert_eq!(probe.focused_candidate_vs_ready_frontier_objective_gap, 4.0);
        assert_eq!(probe.competitive_probe_proxy_gap_closure, 1.0);
        assert!((probe.competitive_probe_proxy_gap_closure_share - 0.25).abs() < 1.0e-9);
        assert_eq!(
            probe.residual_ready_frontier_gap_after_competitive_probe_proxy,
            3.0
        );
        assert_eq!(
            probe.empirical_dominant_blocker,
            "round-repair-local-search-mismatch"
        );
        assert!(
            probe
                .empirical_dominant_blocker_summary
                .contains("precedence coverage is cleared")
        );
        assert!(
            probe
                .empirical_dominant_blocker_summary
                .contains("budget depletion is not observed")
        );
        assert!(
            probe
                .empirical_driver_evidence_summary
                .contains("round-repair-local-search-mismatch=`blocking`")
        );
        assert!(probe.empirical_driver_evidence.iter().any(|driver| {
            driver.driver_id == "precedence-coverage"
                && driver.status == "cleared"
                && driver.summary.contains("100.00%")
        }));
        assert!(probe.empirical_driver_evidence.iter().any(|driver| {
            driver.driver_id == "budget-depletion"
                && driver.status == "cleared"
                && driver.summary.contains("2/12")
                && driver.summary.contains("3/32")
                && driver.summary.contains("`completed-within-budget`")
        }));
        assert!(probe.empirical_driver_evidence.iter().any(|driver| {
            driver.driver_id == "round-repair-local-search-mismatch"
                && driver.status == "blocking"
                && driver.summary.contains("25.00%")
                && driver
                    .summary
                    .contains("residual ready_frontier gap 3.000000")
        }));
        assert_eq!(
            probe.budget_coverage_experiment.experiment_status,
            "neither-budget-nor-coverage-dominates"
        );
        assert_eq!(
            probe.budget_coverage_experiment.recommended_next_action,
            "prioritize-candidate-improvement-evidence"
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .focused_budget_usage,
            "2/12 (16.67%)"
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .competitive_budget_usage,
            "3/32 (9.38%)"
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .proxy_objective_delta_vs_focused,
            1.0
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .competitive_probe_ready_frontier_gap,
            3.0
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("leaves residual ready_frontier gap 3.000000")
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("competitive 3/32 (9.38%) / `completed-within-budget`")
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("round-repair-local-search-mismatch")
        );
        assert_eq!(
            probe.residual_interpretation,
            "residual-gap-persists-without-probe-headroom"
        );
        assert!(
            probe
                .residual_interpretation_summary
                .contains("reports no further improving move")
        );
        assert_eq!(
            probe.dominant_residual_driver,
            "remaining-ready-frontier-gap"
        );
        assert!(
            probe
                .dominant_residual_driver_summary
                .contains("dominant measured blocker")
        );
        assert!(
            probe
                .dominant_residual_driver_summary
                .contains("no improving move")
        );
        assert_eq!(
            probe.next_step_evidence,
            "need-new-candidate-evidence-beyond-current-probe"
        );
        assert!(
            probe
                .next_step_evidence_summary
                .contains("stronger candidate path than the current probe")
        );
        assert_eq!(probe.parity_claim_status, "diagnostic-only");
        assert!(
            probe
                .parity_claim_summary
                .contains("material competitiveness")
        );
        assert_eq!(probe.remaining_blocker_count, 3);
        assert!(
            probe
                .remaining_blockers_summary
                .contains("3 active benchmark-side blocker(s)")
        );
        assert!(
            probe
                .remaining_blockers_summary
                .contains("dominant_residual_driver=`proxy-covered-measured-ready-frontier-gap`")
        );
        assert_eq!(probe.remaining_blockers.len(), 3);
        assert_eq!(
            probe.readiness_criteria_version,
            super::LP_BZ_COMPETITIVE_READINESS_CRITERIA_VERSION
        );
        assert_eq!(probe.readiness_state, "benchmark-side-not-ready");
        assert_eq!(probe.readiness_blocked_criteria_count, 3);
        assert!(
            probe
                .readiness_summary
                .contains("fails 3 competitive-readiness criterion/criteria")
        );
        assert_eq!(probe.readiness_criteria.len(), 4);
        assert!(probe.readiness_criteria.iter().any(|criterion| {
            criterion.criterion_id == "dominant-residual-driver-cleared"
                && criterion.status == "blocked"
                && criterion
                    .criterion_label
                    .contains("dominant_residual_driver must equal")
                && criterion
                    .summary
                    .contains("Expected dominant_residual_driver")
                && criterion.summary.contains("dominant measured blocker")
        }));
        assert!(probe.readiness_criteria.iter().any(|criterion| {
            criterion.criterion_id == "parity-claim-guardrail"
                && criterion.status == "guardrail-active"
                && criterion.summary.contains("diagnostic-only")
        }));
        assert!(probe.remaining_blockers.iter().any(|blocker| {
            blocker.blocker_id == "dominant-residual-driver"
                && blocker
                    .blocker_label
                    .contains("material competitiveness")
                && blocker.summary.contains("requires dominant_residual_driver")
                && blocker.summary.contains("dominant measured blocker")
                && blocker.evidence_fields.contains(
                    &"lp_bz_baseline.competitive_ready_frontier_probe.dominant_residual_driver_summary"
                        .to_owned(),
                )
        }));
        assert!(probe.remaining_blockers.iter().any(|blocker| {
            blocker.blocker_id == "next-step-evidence"
                && blocker.summary.contains("requires next_step_evidence")
                && blocker.summary.contains("stronger candidate path than the current probe")
                && blocker.evidence_fields.contains(
                    &"lp_bz_baseline.summary.lp_bz_round_repair.competitive_probe.competitive_local_optimizer_residual_opportunity.discounted_gain"
                        .to_owned(),
                )
        }));
        assert!(probe.remaining_blockers.iter().any(|blocker| {
            blocker.blocker_id == "schedule-level-ready-frontier-proof"
                && blocker.summary.contains("diagnostic-only")
                && blocker
                    .summary
                    .contains("current statuses are residual_interpretation=")
                && blocker.summary.contains("Residual read:")
                && blocker.evidence_fields.contains(
                    &"lp_bz_baseline.competitive_ready_frontier_probe.parity_claim_status"
                        .to_owned(),
                )
        }));
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_marks_schedule_proof_only_readiness() {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline.candidate_pcpsp_summary.discounted_objective = 98.0;
        baseline.candidate_pcpsp_summary.undiscounted_objective = 98.0;
        baseline
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_search_discounted_target_score_proxy = 100.0;
        baseline
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .local_search_score_delta_vs_focused_proxy = 2.0;
        baseline.competitive_ready_frontier_probe =
            super::build_lp_bz_competitive_ready_frontier_probe_summary(
                100.0,
                98.0,
                &baseline.summary,
            );
        let probe = &baseline.competitive_ready_frontier_probe;

        assert_eq!(
            probe.residual_interpretation,
            "proxy-covers-measured-ready-frontier-gap"
        );
        assert_eq!(
            probe.next_step_evidence,
            "need-schedule-level-ready-frontier-proof"
        );
        assert_eq!(
            probe.dominant_residual_driver,
            "proxy-covered-measured-ready-frontier-gap"
        );
        assert_eq!(
            probe.readiness_state,
            "benchmark-side-ready-for-schedule-proof"
        );
        assert_eq!(
            probe.empirical_dominant_blocker,
            "schedule-level-proof-only"
        );
        assert_eq!(
            probe.budget_coverage_experiment.experiment_status,
            "neither-budget-nor-coverage-dominates"
        );
        assert_eq!(
            probe.budget_coverage_experiment.recommended_next_action,
            "request-schedule-level-ready-frontier-proof"
        );
        assert_eq!(probe.readiness_blocked_criteria_count, 0);
        assert_eq!(probe.remaining_blocker_count, 1);
        assert!(
            probe
                .readiness_summary
                .contains("schedule-level ready_frontier proof is the only remaining step")
        );
        assert!(
            probe
                .readiness_summary
                .contains("residual_interpretation=`proxy-covers-measured-ready-frontier-gap`")
        );
        assert!(probe.remaining_blockers.iter().any(|blocker| {
            blocker.blocker_id == "schedule-level-ready-frontier-proof"
                && blocker
                    .summary
                    .contains("Benchmark-side competitive-readiness exit shape is satisfied")
        }));
        assert!(probe.readiness_criteria.iter().all(|criterion| {
            criterion.status != "blocked" || criterion.criterion_id == "parity-claim-guardrail"
        }));
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_marks_precedence_coverage_as_empirical_blocker() {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .summary
            .lp_bz_lp_solve
            .precedence_diagnostics
            .total_precedence_rows = 100;
        baseline
            .summary
            .lp_bz_lp_solve
            .precedence_diagnostics
            .enforced_precedence_rows = 93;
        baseline
            .summary
            .lp_bz_lp_solve
            .precedence_diagnostics
            .skipped_precedence_rows = 7;
        baseline
            .summary
            .lp_bz_lp_solve
            .precedence_diagnostics
            .coverage_basis_points = Some(9_300);
        baseline
            .summary
            .lp_bz_lp_solve
            .precedence_diagnostics
            .coverage_completeness =
            super::lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Partial;
        let probe = super::build_lp_bz_competitive_ready_frontier_probe_summary(
            baseline
                .competitive_ready_frontier_probe
                .ready_frontier_discounted_objective,
            baseline.candidate_pcpsp_summary.discounted_objective,
            &baseline.summary,
        );

        assert_eq!(probe.empirical_dominant_blocker, "precedence-coverage");
        assert!(
            probe
                .empirical_dominant_blocker_summary
                .contains("700 bps shortfall"),
            "precedence summary should report the measured shortfall"
        );
        assert!(probe.empirical_driver_evidence.iter().any(|driver| {
            driver.driver_id == "precedence-coverage"
                && driver.status == "blocking"
                && driver.summary.contains("93.00%")
        }));
        assert_eq!(
            probe.budget_coverage_experiment.experiment_status,
            "precedence-coverage-expansion-first"
        );
        assert_eq!(
            probe.budget_coverage_experiment.recommended_next_action,
            "expand-precedence-coverage-before-budget-rerun"
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .focused_ready_frontier_gap,
            4.0
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("Precedence coverage remains incomplete at 93.00%")
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("92.000000 -> 93.000000 (+1.000000)")
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_marks_budget_depletion_as_empirical_blocker() {
        let baseline = sample_lp_bz_baseline_summary_with_termination(12, "max-iterations-reached");
        let probe = &baseline.competitive_ready_frontier_probe;

        assert_eq!(probe.empirical_dominant_blocker, "budget-depletion");
        assert!(
            probe
                .empirical_dominant_blocker_summary
                .contains("focused local search is `budget-hit` after 12/12"),
            "budget summary should surface the depleted focused budget"
        );
        assert!(probe.empirical_driver_evidence.iter().any(|driver| {
            driver.driver_id == "budget-depletion"
                && driver.status == "blocking"
                && driver.summary.contains("12/12")
                && driver.summary.contains("`completed-within-budget`")
        }));
        assert_eq!(
            probe.budget_coverage_experiment.experiment_status,
            "budget-expansion-changes-proxy-candidate"
        );
        assert_eq!(
            probe.budget_coverage_experiment.recommended_next_action,
            "prioritize-budget-expansion-follow-up"
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .focused_budget_usage,
            "12/12 (100.00%)"
        );
        assert_eq!(
            probe
                .budget_coverage_experiment
                .comparison
                .competitive_budget_usage,
            "3/32 (9.38%)"
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("92.000000 -> 93.000000 (+1.000000)")
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("competitive probe used 3/32 (9.38%) with execution_state=`completed-within-budget`")
        );
        assert!(
            probe
                .budget_coverage_experiment
                .summary
                .contains("ready_frontier gap 4.000000 -> 3.000000")
        );
    }

    #[test]
    fn sidecar_temporal_routing_gate_stays_explicit() {
        let baseline = sample_lp_bz_baseline_summary();
        let gate = &baseline.temporal_routing_promotion_gate;

        super::validate_temporal_routing_promotion_gate_summary(gate)
            .expect("sidecar temporal/routing gate should validate");
        assert_eq!(gate.gate_version, "mr206-v1");
        assert_eq!(gate.promotion_decision, "blocked-by-npv");
        assert!(gate.temporal_routing_gate_passed);
        assert_eq!(gate.metrics.len(), 4);
        assert!(
            gate.summary
                .contains("promotion still requires an NPV improvement")
                || gate.summary.contains("requires an NPV improvement")
        );
    }

    #[test]
    fn sidecar_budget_hit_runtime_remains_explicit_without_skip_summary() {
        let baseline = sample_lp_bz_baseline_summary_with_termination(12, "max-iterations-reached");
        let round_repair = &baseline.summary.lp_bz_round_repair;
        let readiness = &baseline.lp_bz_promotion_readiness;

        super::validate_lp_bz_baseline_runtime_budget_contract(&baseline)
            .expect("budget-hit sidecar runtime contract should validate");
        assert_eq!(
            round_repair
                .local_optimizer_runtime_budget_contract
                .execution_state,
            "budget-hit"
        );
        assert!(!round_repair.local_optimization_skipped);
        assert_eq!(
            readiness
                .local_optimizer_runtime_budget_contract
                .execution_state,
            "budget-hit"
        );
        assert!(readiness.summary.contains(
            "hit the explicit iteration budget at 12/12 iterations (`max-iterations-reached`)"
        ));
        assert!(!readiness.summary.contains("local optimization is skipped"));
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_gap_closure_slice() {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .competitive_ready_frontier_probe
            .competitive_probe_proxy_gap_closure_share = 0.5;

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline)
            .expect_err("inconsistent ready-frontier competitive probe should be rejected");

        assert!(
            error
                .to_string()
                .contains("competitive ready-frontier probe"),
            "validation error should explain the competitive ready-frontier probe mismatch"
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_residual_interpretation_slice()
    {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .competitive_ready_frontier_probe
            .residual_interpretation_summary = "mismatched summary".to_owned();

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline).expect_err(
            "inconsistent competitive ready-frontier residual interpretation should be rejected",
        );

        assert!(
            error.to_string().contains("residual interpretation"),
            "validation error should explain the residual-interpretation mismatch"
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_next_step_evidence_slice() {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .competitive_ready_frontier_probe
            .next_step_evidence_summary = "mismatched summary".to_owned();

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline).expect_err(
            "inconsistent competitive ready-frontier next-step evidence should be rejected",
        );

        assert!(
            error.to_string().contains("next-step evidence"),
            "validation error should explain the next-step evidence mismatch"
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_remaining_blockers_slice() {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .competitive_ready_frontier_probe
            .remaining_blockers_summary = "mismatched summary".to_owned();

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline).expect_err(
            "inconsistent competitive ready-frontier remaining blockers should be rejected",
        );

        assert!(
            error.to_string().contains("remaining blockers summary"),
            "validation error should explain the remaining-blockers mismatch"
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_empirical_driver_summary_slice()
     {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .competitive_ready_frontier_probe
            .empirical_dominant_blocker_summary = "mismatched summary".to_owned();

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline).expect_err(
            "inconsistent competitive ready-frontier empirical driver summary should be rejected",
        );

        assert!(
            error
                .to_string()
                .contains("empirical dominant blocker summary"),
            "validation error should explain the empirical-driver mismatch"
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_budget_coverage_experiment_slice()
     {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline
            .competitive_ready_frontier_probe
            .budget_coverage_experiment
            .summary = "mismatched summary".to_owned();

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline).expect_err(
            "inconsistent competitive ready-frontier budget/coverage experiment should be rejected",
        );

        assert!(
            error
                .to_string()
                .contains("budget/coverage experiment summary"),
            "validation error should explain the budget/coverage experiment mismatch"
        );
    }

    #[test]
    fn sidecar_competitive_ready_frontier_probe_rejects_inconsistent_readiness_summary_slice() {
        let mut baseline = sample_lp_bz_baseline_summary();
        baseline.competitive_ready_frontier_probe.readiness_summary =
            "mismatched summary".to_owned();

        let error = super::validate_lp_bz_baseline_runtime_budget_contract(&baseline).expect_err(
            "inconsistent competitive ready-frontier readiness summary should be rejected",
        );

        assert!(
            error.to_string().contains("readiness summary"),
            "validation error should explain the readiness-summary mismatch"
        );
    }

    #[test]
    fn lp_bz_baseline_only_targets_marvin_with_lp_pcpsp_reference() {
        assert!(supports_lp_bz_baseline(&DATASETS[0]));
        assert!(!supports_lp_bz_baseline(&DATASETS[1]));
        assert!(!supports_lp_bz_baseline(&DATASETS[2]));
    }

    #[test]
    fn mclaughlin_limit_dataset_now_enables_nested_shell_primary_path() {
        assert!(DATASETS[1].nested_shell_probe_enabled);
        assert_eq!(
            DATASETS[1].selected_block_source,
            REFERENCE_SELECTED_BLOCK_SOURCE
        );
        assert_eq!(DATASETS[1].lp_pcpsp_solution_file, None);
    }

    #[test]
    fn mclaughlin_limit_selected_block_source_is_explicitly_shell_driven() {
        assert_eq!(
            crate::minelib_scheduling_support::MCLAUGHLIN_LIMIT_SELECTED_BLOCK_SOURCE,
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases"
        );
        assert!(
            crate::minelib_scheduling_support::MCLAUGHLIN_LIMIT_SELECTED_BLOCK_SOURCE
                .contains("shells")
        );
        assert!(
            crate::minelib_scheduling_support::MCLAUGHLIN_LIMIT_SELECTED_BLOCK_SOURCE
                .contains("pushback-equivalent")
        );
    }

    #[test]
    fn sample_lp_bz_baseline_shares_promoted_ramp_access_contract() {
        let baseline = sample_lp_bz_baseline_summary();
        let promoted_access_law =
            summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law(
                &baseline.phase_refinement_diagnostics,
            );

        assert_eq!(
            baseline.cut_access_law.ramp_access_contract,
            promoted_access_law.ramp_access_contract
        );
        assert_eq!(
            baseline.cut_access_law.lineage_bench_continuity_contract,
            promoted_access_law.lineage_bench_continuity_contract
        );
        assert_eq!(
            baseline.cut_access_law.complete_cut_design_contract,
            promoted_access_law.complete_cut_design_contract
        );
        validate_promoted_pushback_bench_localized_cut_access_law_contract(
            &baseline.cut_access_law,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        )
        .expect("sample LP/BZ baseline cut access law should validate");
    }

    #[test]
    fn temporal_solver_gap_mentions_lp_bz_sidecar_when_present() {
        let baseline = sample_lp_bz_baseline_summary();
        let gap = temporal_solver_comparability_gap(Some(&baseline));

        assert!(gap.contains("Marvin-scoped"));
        assert!(gap.contains(LP_BZ_UNIT_GRANULARITY_LABEL));
        assert!(gap.contains("cut_access_law"));
        assert!(gap.contains("inter-phase/inter-cut release"));
        assert!(gap.contains("structured bibliographic gap contract"));
        assert!(gap.contains("ramp-access-sequencing"));
        assert!(gap.contains("working-width-minimum-operating-width"));
        assert!(gap.contains("cut-design-lineage-bench-continuity"));
        assert!(gap.contains("complete-cut-design-law"));
        assert!(gap.contains("single paper-like candidate family"));
        assert!(gap.contains("shape-gated-local-front-phase"));
        assert!(gap.contains("explicit bounded local-optimizer runtime contract"));
        assert!(gap.contains("protocol-level mining-cut input contract"));
    }

    #[test]
    fn temporal_solver_gap_summary_is_classified_as_relaxation_model() {
        let baseline = sample_lp_bz_baseline_summary();
        let gap = super::temporal_solver_comparability_gap_summary(Some(&baseline), None, None);

        assert_eq!(
            gap.gap_source,
            super::ComparabilityGapSource::RelaxationModel
        );
        assert_eq!(gap.gap_id, "lp-bz-temporal-solver-route");
        assert!(
            gap.evidence_fields
                .contains(&"lp_bz_baseline.cut_access_law.bibliographic_gap_contract".to_owned())
        );
    }

    fn sample_mclaughlin_limit_lp_bz_sidecar_summary() -> super::MclaughlinLimitLpBzSidecarSummary {
        super::MclaughlinLimitLpBzSidecarSummary {
            sidecar_label: "mclaughlin-limit-benchmark-lp-bz-kernel".to_owned(),
            sidecar_version: MCLAUGHLIN_LIMIT_LP_BZ_SIDECAR_VERSION.to_owned(),
            sidecar_status: "benchmark-side-partial-relaxed-kernel-bound".to_owned(),
            scope_label: "mclaughlin-limit-only".to_owned(),
            objective_alignment_label: "pcpsp-objective-aligned-relaxed-kernel".to_owned(),
            unit_family_label: "nested-shell-bench".to_owned(),
            kernel_label: "lp-bz-lp-kernel-v8".to_owned(),
            solver_label: "minilp".to_owned(),
            solve_status: super::lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal,
            scheduling_unit_count: 12,
            variable_count: 48,
            active_variable_count: 16,
            discounted_objective_bound: Some(812.0),
            candidate_discounted_objective: 500.0,
            reference_discounted_objective: 700.0,
            bound_to_candidate_absolute_gap: Some(312.0),
            bound_to_reference_absolute_gap: Some(112.0),
            precedence_diagnostics: super::lp_bz_lp_kernel::LpBzPrecedenceSolveDiagnostics {
                strategy: super::lp_bz_lp_kernel::LpBzPrecedenceEnforcementStrategy::HybridCheckpoint,
                max_enforced_precedence_rows: 60_000,
                total_precedence_rows: 90_000,
                enforced_precedence_rows: 60_000,
                skipped_precedence_rows: 30_000,
                coverage_completeness:
                    super::lp_bz_lp_kernel::LpBzPrecedenceCoverageCompleteness::Partial,
                coverage_basis_points: Some(6_667),
                enforced_period_indices: vec![0, 1, 2, 3],
                skipped_period_indices: vec![4, 5],
            },
            cut_diagnostics: super::lp_bz_lp_kernel::LpBzCutSolveDiagnostics {
                strategy:
                    super::lp_bz_lp_kernel::LpBzCutTighteningStrategy::PrecedenceCumulativePrefixAndAccessClosureCapacityPrefix,
                total_generated_row_count: 24,
                total_applied_row_count: 24,
                total_skipped_row_count: 0,
                families: Vec::new(),
            },
            completeness_summary:
                "hybrid_checkpoint precedence coverage `partial` (66.67%; enforced 60000/90000 rows, skipped 30000)"
                    .to_owned(),
            disclosure_summary:
                "Partial relaxed-kernel benchmark-side bound for `mclaughlin-limit`; diagnostic-only and not a proof that the integer schedule is good."
                    .to_owned(),
            limitations: vec![
                "diagnostic-only relaxed benchmark-side kernel".to_owned(),
                "does not prove mining-cut comparability".to_owned(),
            ],
        }
    }

    fn sample_mclaughlin_limit_benchmark_cut_refinement_summary()
    -> super::MclaughlinLimitBenchmarkCutRefinementSummary {
        super::MclaughlinLimitBenchmarkCutRefinementSummary {
            contract_label: "benchmark-side-mining-cut-refinement".to_owned(),
            contract_version: super::MCLAUGHLIN_LIMIT_BENCHMARK_CUT_CONTRACT_VERSION.to_owned(),
            contract_status: "benchmark-side-implemented".to_owned(),
            scope_label: "mclaughlin-limit-only".to_owned(),
            source_unit_family_label: "nested-shell-bench".to_owned(),
            refined_unit_family_label: super::MCLAUGHLIN_LIMIT_BENCHMARK_CUT_UNIT_FAMILY_LABEL
                .to_owned(),
            localized_cut_builder_label: super::MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILDER_LABEL
                .to_owned(),
            build_label: super::MCLAUGHLIN_LIMIT_BENCHMARK_CUT_BUILD_LABEL.to_owned(),
            scheduling_unit_count: 18,
            build_config_summary: super::MclaughlinLimitBenchmarkCutBuildConfigSummary {
                max_front_count: 3,
                min_aspect_ratio: 2.0,
                min_dominant_span: 2,
                include_touching_neighbors: true,
                max_local_predecessor_count: Some(6),
                predecessor_cut_link_policy: "predecessor-last-cut".to_owned(),
                front_progression_label: "uniform-33-67-100".to_owned(),
            },
            phase_refinement_diagnostics: super::PushbackBenchLocalizedCutRefinementDiagnostics {
                base_phase_count: 10,
                refined_base_phase_count: 4,
                refined_single_component_phase_count: 2,
                total_cut_phase_count: 18,
                additional_phase_count: 8,
                max_cut_count_per_base_phase: 3,
                average_cut_count_per_base_phase: 1.8,
                realized_front_count_histogram: [(1usize, 2usize), (3usize, 4usize)]
                    .into_iter()
                    .collect(),
                readiness_reason_histogram: [("multi-block-shape-qualified".to_owned(), 4usize)]
                    .into_iter()
                    .collect(),
                exact_three_front_candidate_count: 4,
                exact_three_front_failure_count: 0,
                exact_three_front_failure_realized_front_histogram: BTreeMap::new(),
                exact_three_front_failure_reason_histogram: BTreeMap::new(),
                refined_base_phase_examples: vec!["pb-01::pbcut-c01".to_owned()],
                refined_single_component_phase_examples: vec!["pb-02::pbcut-c01".to_owned()],
            },
            disclosure_summary: "Synthetic benchmark-side cut contract for test coverage."
                .to_owned(),
            limitations: vec!["test-only contract summary".to_owned()],
        }
    }

    #[test]
    fn temporal_solver_gap_without_sidecar_uses_structured_limit_evidence() {
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            None,
            None,
            12,
        );
        let gap = super::temporal_solver_comparability_gap_summary(None, None, Some(&traceability));

        assert_eq!(gap.gap_id, "missing-lp-bz-sidecar");
        assert!(gap.summary.contains("no-benchmark-cut-refinement"));
        assert!(gap.summary.contains("no-lp-bz-sidecar"));
        assert!(
            gap.summary
                .contains("mclaughlin-limit-cut-sidecar-scaffold")
        );
        assert!(gap.evidence_fields.iter().any(|field| {
            field
                == "primary_unit_family_traceability.benchmark_side_evidence.sidecar_evidence_label"
        }));
        assert!(gap.evidence_fields.iter().any(|field| {
            field
                == "primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.scaffold_label"
        }));
    }

    #[test]
    fn temporal_solver_gap_with_partial_limit_sidecar_remains_relaxation_gap() {
        let sidecar = sample_mclaughlin_limit_lp_bz_sidecar_summary();
        let gap = super::temporal_solver_comparability_gap_summary(None, Some(&sidecar), None);

        assert_eq!(gap.gap_id, "partial-lp-bz-sidecar");
        assert_eq!(
            gap.gap_source,
            super::ComparabilityGapSource::RelaxationModel
        );
        assert!(gap.summary.contains("benchmark-side LP/BZ sidecar"));
        assert!(gap.summary.contains("diagnostic-only"));
        assert!(gap.summary.contains("schedule-proof semantics"));
        assert!(
            gap.evidence_fields
                .contains(&"mclaughlin_limit_lp_bz_sidecar".to_owned())
        );
    }

    #[test]
    fn aggregation_gap_summary_is_classified_as_aggregation_formulation() {
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred Marvin shell family should build")
            .with_realized_shell_count(5);
        let gap = super::aggregation_comparability_gap_summary(
            "marvin",
            "nested-shell-bench",
            Some(&preferred_shell_family),
            None,
            false,
        );

        assert_eq!(
            gap.gap_source,
            super::ComparabilityGapSource::AggregationFormulation
        );
        assert_eq!(gap.gap_id, "aggregation-proxy-family");
        assert!(
            gap.evidence_fields
                .contains(&"preferred_nested_shell_family_contract".to_owned())
        );
    }

    #[test]
    fn mclaughlin_limit_aggregation_gap_summary_uses_primary_traceability_fields() {
        let traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            None,
            None,
            12,
        );
        let gap = super::aggregation_comparability_gap_summary(
            "mclaughlin-limit",
            "nested-shell-bench",
            None,
            Some(&traceability),
            false,
        );

        assert_eq!(
            gap.gap_source,
            super::ComparabilityGapSource::AggregationFormulation
        );
        assert_eq!(gap.gap_id, "aggregation-open-upit-proxy-family");
        assert!(gap.summary.contains("321 selected blocks"));
        assert!(gap.summary.contains("no-benchmark-cut-refinement"));
        assert!(gap.summary.contains("no-lp-bz-sidecar"));
        assert!(
            gap.summary
                .contains("mclaughlin-limit-cut-sidecar-scaffold")
        );
        assert!(
            gap.evidence_fields.contains(
                &"primary_unit_family_traceability.preferred_phase_plan_proxy.preferred_phase_count"
                    .to_owned()
            )
        );
        assert!(gap.evidence_fields.contains(
            &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.readiness_dependency_label"
                .to_owned()
        ));
        assert!(gap.evidence_fields.contains(
            &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_contract_status"
                .to_owned()
        ));
        assert!(gap.evidence_fields.contains(
            &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_contract_status"
                .to_owned()
        ));
        assert!(gap.evidence_fields.contains(
            &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.variant_scope_label"
                .to_owned()
        ));
        assert!(gap.evidence_fields.contains(
            &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.benchmark_cut_prerequisites"
                .to_owned()
        ));
        assert!(gap.evidence_fields.contains(
            &"primary_unit_family_traceability.benchmark_side_evidence.future_scaffold.lp_bz_sidecar_prerequisites"
                .to_owned()
        ));
        assert!(
            gap.evidence_fields
                .contains(&"primary_unit_family_traceability.scheduling_unit_count".to_owned())
        );
    }

    #[test]
    fn marvin_input_aggregation_gap_summary_surfaces_layered_traceability() {
        let baseline = sample_lp_bz_baseline_summary();
        let gap = super::marvin_input_aggregation_traceability_gap_summary(&baseline);

        assert_eq!(gap.gap_source, super::ComparabilityGapSource::InputProtocol);
        assert_eq!(gap.gap_id, "marvin-input-aggregation-traceability");
        assert!(gap.summary.contains(&format!(
            "selected_block_source = \"{MARVIN_SELECTED_BLOCK_SOURCE}\""
        )));
        assert!(gap.summary.contains("nested-shell-bench"));
        assert!(
            gap.summary
                .contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL)
        );
        assert!(
            gap.summary
                .contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL)
        );
        assert_eq!(
            gap.summary,
            format!(
                "Input/aggregation traceability now stays explicit across the benchmark-side paper-like chain `shells -> pushbacks -> mining-cuts -> scheduling`: `selected_block_source = \"{MARVIN_SELECTED_BLOCK_SOURCE}\"` names the admissible shell-derived block contract; the bounded `nested-shell-bench` bridge keeps 7 revenue factors on `strict-sequential` access, realizes 5 shells, and materializes 10 shell×bench pushback phases before localized-cut refinement; and builder `pushback-bench-localized-mining-cuts` / build `front3-ar2.0-span2-n6` refines scaffold `shape-gated-local-front-phase` into promoted `pushback-bench-localized-cut-phase` units under `uniform-33-67-100` progression before scheduling. Quantitatively, 321 selected blocks currently lift into 10 shell×bench pushback phases, which then refine into 18 promoted mining-cut phases and 12 LP/BZ scheduling units. The route remains `exploratory-local` because this provenance is still a bounded benchmark-side reconstruction rather than a paper-reproduced shell generator plus calibrated mining-cut workflow."
            )
        );
        assert!(
            gap.evidence_fields
                .contains(
                    &"lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_source"
                        .to_owned(),
                )
        );
        assert!(
            gap.evidence_fields
                .contains(
                    &"lp_bz_baseline.unit_family_traceability.selected_block_provenance.selected_block_count"
                        .to_owned(),
                )
        );
        assert!(
            gap.evidence_fields
                .contains(
                    &"lp_bz_baseline.unit_family_traceability.preferred_phase_plan_proxy.aggregation_strategy"
                        .to_owned(),
                )
        );
        assert!(gap.evidence_fields.contains(
            &"lp_bz_baseline.unit_family_traceability.preferred_phase_plan_proxy.preferred_phase_count"
                .to_owned(),
        ));
        assert!(gap.evidence_fields.contains(
            &"lp_bz_baseline.phase_refinement_diagnostics.total_cut_phase_count".to_owned(),
        ));
        assert!(gap.evidence_fields.contains(
            &"lp_bz_baseline.summary.lp_bz_inputs.precedence_units.unit_count".to_owned(),
        ));
        assert!(
            gap.evidence_fields
                .contains(&"preferred_nested_shell_family_contract".to_owned())
        );
        assert!(
            gap.evidence_fields
                .contains(
                    &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.front_progression_label"
                        .to_owned(),
                )
        );
        assert!(
                gap.evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.promoted_cut_phase_count"
                            .to_owned(),
                    )
        );
        assert!(
                gap.evidence_fields
                    .contains(
                        &"lp_bz_baseline.unit_family_traceability.localized_cut_builder_provenance.scheduling_unit_count"
                            .to_owned(),
                    )
        );
    }

    #[test]
    fn dataset_contract_roles_label_paperlike_candidate_and_scaffold() {
        let roles = build_dataset_contract_roles(
            &DATASETS[0],
            "nested-shell-bench",
            true,
            true,
            false,
            false,
        );

        assert!(
            roles
                .iter()
                .any(|role| role == "marvin-lp-bz-active-candidate")
        );
        assert!(
            roles.iter().any(|role| role
                == &format!("paperlike-candidate-family:{LP_BZ_UNIT_GRANULARITY_LABEL}"))
        );
        assert!(roles.iter().any(|role| {
            role == &format!(
                "local-optimizer-scaffold:{MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL}"
            )
        }));
        assert!(roles.iter().any(|role| {
            role == &format!(
                "promoted-build-label:{MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL}"
            )
        }));
    }

    #[test]
    fn dataset_contract_roles_tag_mclaughlin_variants_correctly() {
        let limit_roles = build_dataset_contract_roles(
            &DATASETS[1],
            "nested-shell-bench",
            false,
            false,
            false,
            false,
        );
        let limit_sidecar_roles = build_dataset_contract_roles(
            &DATASETS[1],
            "nested-shell-bench",
            false,
            false,
            false,
            true,
        );
        let full_roles = build_dataset_contract_roles(
            &DATASETS[2],
            "reference-period-bench",
            false,
            false,
            false,
            false,
        );

        assert!(
            limit_roles
                .iter()
                .any(|role| role == "mclaughlin-limit-pushback-equivalent-routing")
        );
        assert!(
            limit_roles
                .iter()
                .any(|role| role == "mclaughlin-limit-cut-sidecar-scaffold")
        );
        assert!(
            limit_sidecar_roles
                .iter()
                .any(|role| role == "mclaughlin-limit-lp-bz-bound-sidecar")
        );
        assert!(
            !limit_roles
                .iter()
                .any(|role| role == "stress-only-local-variant")
        );
        assert!(
            full_roles
                .iter()
                .any(|role| role == "stress-only-local-variant")
        );
    }

    #[test]
    fn primary_unit_family_traceability_keeps_limit_and_stress_labels_explicit() {
        let limit_traceability = build_primary_unit_family_traceability(
            &DATASETS[1],
            "mclaughlin-limit-open-upit-shells-pushback-equivalent-bench-phases",
            321,
            "limit traceability summary",
            &[
                "open-upit-net-values".to_owned(),
                "precedence-constrained-shells".to_owned(),
                "pushback-equivalent-bench-phases".to_owned(),
                "scheduling".to_owned(),
            ],
            "nested-shell-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            Some(5),
            None,
            false,
            None,
            None,
            12,
        );
        let full_traceability = build_primary_unit_family_traceability(
            &DATASETS[2],
            crate::minelib_scheduling_support::REFERENCE_SELECTED_BLOCK_SOURCE,
            321,
            "full traceability summary",
            &["cpit-solution".to_owned()],
            "reference-period-bench",
            &sample_limit_cut_readiness_phase_plan(),
            10,
            None,
            None,
            false,
            None,
            None,
            12,
        );

        assert_eq!(
            limit_traceability.unit_family_role,
            "pushback-equivalent shell×bench routing"
        );
        assert_eq!(
            limit_traceability.literature_alignment_label,
            "literature-target-variant"
        );
        assert_eq!(
            limit_traceability
                .benchmark_side_evidence
                .cut_evidence_label,
            "no-benchmark-cut-refinement"
        );
        assert_eq!(
            limit_traceability
                .benchmark_side_evidence
                .sidecar_evidence_label,
            "no-lp-bz-sidecar"
        );
        let future_scaffold = limit_traceability
            .benchmark_side_evidence
            .future_scaffold
            .as_ref()
            .expect("limit traceability should publish a future scaffold");
        assert_eq!(
            future_scaffold.scaffold_label,
            "mclaughlin-limit-cut-sidecar-scaffold"
        );
        assert_eq!(
            future_scaffold.scaffold_role,
            "future-mining-cut-or-lp-bz-input-contract"
        );
        assert!(
            future_scaffold
                .target_contracts
                .iter()
                .any(|target| target == "benchmark-side-mining-cut-refinement")
        );
        assert!(
            future_scaffold
                .target_contracts
                .iter()
                .any(|target| target == "lp-bz-sidecar-input-family")
        );
        assert_eq!(
            future_scaffold.readiness_dependency_label,
            "pushback-equivalent-bench-cut-readiness"
        );
        assert_eq!(
            future_scaffold.benchmark_cut_contract_status,
            "scaffold-only-not-implemented"
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_contract_status,
            "scaffold-only-not-implemented"
        );
        assert!(!future_scaffold.benchmark_cut_promotion_ready);
        assert_eq!(
            future_scaffold.benchmark_cut_blocking_prerequisite_ids,
            vec!["benchmark-side-mining-cut-refinement-implementation".to_owned()]
        );
        assert_eq!(
            future_scaffold.benchmark_cut_promotion_rule.rule_id,
            "benchmark-side-mining-cut-refinement-promotion-rule"
        );
        assert_eq!(
            future_scaffold.benchmark_cut_promotion_rule.status,
            "blocked"
        );
        assert_eq!(
            future_scaffold.benchmark_cut_promotion_rule.target_contract,
            "benchmark-side-mining-cut-refinement"
        );
        assert_eq!(
            future_scaffold.benchmark_cut_promotion_rule.evaluation_mode,
            "all-listed-prerequisites-must-clear"
        );
        assert_eq!(
            future_scaffold
                .benchmark_cut_promotion_rule
                .blocking_prerequisite_ids,
            vec!["benchmark-side-mining-cut-refinement-implementation".to_owned()]
        );
        assert!(!future_scaffold.lp_bz_sidecar_promotion_ready);
        assert_eq!(
            future_scaffold.lp_bz_sidecar_blocking_prerequisite_ids,
            vec![
                "benchmark-side-mining-cut-refinement".to_owned(),
                "lp-bz-sidecar-input-family-implementation".to_owned(),
            ]
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_promotion_rule.rule_id,
            "lp-bz-sidecar-input-family-promotion-rule"
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_promotion_rule.status,
            "blocked"
        );
        assert_eq!(
            future_scaffold.lp_bz_sidecar_promotion_rule.target_contract,
            "lp-bz-sidecar-input-family"
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_promotion_rule
                .evaluation_mode
                .contains("temporal-routing-gate-becomes-binding-once-sidecar-exists")
        );
        assert_eq!(
            future_scaffold
                .lp_bz_sidecar_promotion_rule
                .blocking_prerequisite_ids,
            vec![
                "benchmark-side-mining-cut-refinement".to_owned(),
                "lp-bz-sidecar-input-family-implementation".to_owned(),
            ]
        );
        assert_eq!(
            future_scaffold.variant_scope_label,
            "mclaughlin-limit-only-scaffold"
        );
        assert!(
            future_scaffold
                .variant_scope_summary
                .contains("mclaughlin-full")
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("benchmark-side-mining-cut-refinement")
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("lp-bz-sidecar-input-family")
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("scaffold-only")
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("publish-benchmark-cut-readiness-traceability=satisfied")
        );
        assert!(
            future_scaffold
                .promotion_path_summary
                .contains("version-mclaughlin-limit-lp-bz-sidecar=blocked")
        );
        assert_eq!(future_scaffold.benchmark_cut_exit_criteria.len(), 3);
        assert!(
            future_scaffold
                .benchmark_cut_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "publish-benchmark-cut-readiness-traceability"
                        && criterion.status == "satisfied"
                })
        );
        assert!(
            future_scaffold
                .benchmark_cut_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "version-benchmark-cut-refinement-contract"
                        && criterion.status == "blocked"
                        && criterion
                            .expected_state
                            .contains("benchmark-side-implemented")
                })
        );
        assert_eq!(future_scaffold.lp_bz_sidecar_exit_criteria.len(), 4);
        assert!(
            future_scaffold
                .lp_bz_sidecar_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "preserve-limit-only-scope"
                        && criterion.status == "satisfied"
                })
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_exit_criteria
                .iter()
                .any(|criterion| {
                    criterion.criterion_id == "version-mclaughlin-limit-lp-bz-sidecar"
                        && criterion.status == "blocked"
                        && criterion
                            .summary
                            .contains("no `mclaughlin-limit` LP/BZ sidecar candidate")
                })
        );
        assert_eq!(future_scaffold.benchmark_cut_prerequisites.len(), 2);
        assert!(
            future_scaffold
                .benchmark_cut_prerequisites
                .iter()
                .any(|prerequisite| {
                    prerequisite.prerequisite_id == "pushback-equivalent-bench-cut-readiness"
                        && prerequisite.status == "audited"
                })
        );
        assert!(
            future_scaffold
                .benchmark_cut_prerequisites
                .iter()
                .any(|prerequisite| {
                    prerequisite.prerequisite_id
                        == "benchmark-side-mining-cut-refinement-implementation"
                        && prerequisite.status == "blocked"
                })
        );
        assert_eq!(future_scaffold.lp_bz_sidecar_prerequisites.len(), 4);
        assert!(
            future_scaffold
                .lp_bz_sidecar_prerequisites
                .iter()
                .any(|prerequisite| {
                    prerequisite.prerequisite_id == "lp-bz-sidecar-input-family-implementation"
                        && prerequisite.status == "blocked"
                })
        );
        assert!(
            future_scaffold
                .lp_bz_sidecar_prerequisites
                .iter()
                .any(|prerequisite| {
                    prerequisite.prerequisite_id == "stress-only-full-variant-separation"
                        && prerequisite.status == "audited"
                        && prerequisite.summary.contains("stress-only")
                })
        );
        assert_eq!(
            full_traceability.unit_family_role,
            "stress-only reference-period × bench fallback"
        );
        assert_eq!(
            full_traceability.literature_alignment_label,
            "stress-only-local-variant"
        );
        assert_eq!(
            full_traceability
                .benchmark_side_evidence
                .benchmark_scope_label,
            "stress-only-local-variant"
        );
        assert!(
            full_traceability
                .benchmark_side_evidence
                .future_scaffold
                .is_none()
        );
    }

    #[test]
    fn temporal_solver_gap_mentions_absence_when_lp_bz_sidecar_is_missing() {
        let gap = temporal_solver_comparability_gap(None);

        assert!(gap.contains("no LP/BZ sidecar is available"));
        assert!(!gap.contains("lp_bz_baseline"));
    }

    #[test]
    fn parse_cli_ignores_quiet_flag_without_rebinding_output_path() {
        let repo_root = repo_root_path();
        let path_policy = BenchmarkPathPolicy::from_repo_root(repo_root.clone());

        let cli = parse_multi_mine_scheduler_cli_args(&path_policy, ["--quiet"])
            .expect("multi-mine scheduler CLI should ignore quiet flag");

        assert_eq!(
            cli.output_path,
            repo_root
                .join("datasets")
                .join("benchmarks")
                .join("outputs")
                .join("multi-mine-scheduling-report.json")
        );
    }

    #[test]
    fn parse_cli_rebases_relative_output_path_on_repo_root() {
        let repo_root = repo_root_path();
        let path_policy = BenchmarkPathPolicy::from_repo_root(repo_root.clone());

        let cli = parse_multi_mine_scheduler_cli_args(
            &path_policy,
            [
                r"datasets\benchmarks\outputs\multi-mine-custom.json",
                "--quiet",
            ],
        )
        .expect("multi-mine scheduler CLI should rebase relative output path");

        assert_eq!(
            cli.output_path,
            repo_root
                .join("datasets")
                .join("benchmarks")
                .join("outputs")
                .join("multi-mine-custom.json")
        );
    }

    #[test]
    fn marvin_lp_bz_sidecar_builder_uses_pushback_bench_localized_cut_units() {
        let config = &DATASETS[0];
        let repo_root = repo_root_path()
            .canonicalize()
            .expect("repo root should resolve");
        let dataset_dir = repo_root
            .join("datasets")
            .join("benchmarks")
            .join(config.benchmark_family);
        let references_dir = dataset_dir.join("references");
        let model = read_benchmark_blocks(dataset_dir.join(config.blocks_file), config.dataset_id)
            .expect("marvin blocks should load");
        let linear_index_to_row_index =
            build_linear_index_to_row_index(&model).expect("linear index lookup should build");
        let selected_solution = read_minelib_cpit_solution(
            &references_dir.join(config.selected_block_solution_file),
            &model,
        )
        .expect("cpit reference should load");
        let selected_block_count = selected_solution
            .assignments
            .iter()
            .filter(|assignment| assignment.fraction > 1.0e-9)
            .count();
        let precedence_graph =
            read_minelib_precedence_graph(&references_dir.join(config.precedence_file), &model)
                .expect("precedence graph should load");
        let upit_block_values = read_minelib_upit_block_values(
            &references_dir.join(config.upit_objective_file),
            &model,
        )
        .expect("upit objective should load")
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let pcpsp_problem =
            read_minelib_pcpsp_problem(&references_dir.join(config.pcpsp_problem_file), &model)
                .expect("pcpsp problem should load");
        let pcpsp_solution =
            read_minelib_pcpsp_solution(&references_dir.join(config.pcpsp_solution_file), &model)
                .expect("pcpsp solution should load");
        let tonnage_column =
            ColumnId::new(config.tonnage_column).expect("tonnage column id should be valid");
        let preferred_phase_plan = build_preferred_phase_plan_for_minelib_scheduling(
            config.dataset_id,
            config.nested_shell_probe_enabled,
            &model,
            &linear_index_to_row_index,
            &selected_solution.assignments,
            Some(&precedence_graph),
            Some(&upit_block_values),
            &tonnage_column,
            NESTED_SHELL_PROBE_FACTOR_COUNT,
        )
        .expect("preferred Marvin phase plan should build");
        let resource_roles = config
            .resource_roles
            .iter()
            .copied()
            .collect::<BTreeMap<_, _>>();

        let cut_artifacts = build_marvin_lp_bz_sidecar_artifacts(
            config,
            &model,
            &preferred_phase_plan.phase_plan,
            &pcpsp_problem,
            &resource_roles,
            &tonnage_column,
        )
        .expect("lp/bz sidecar cut artifacts should build");

        assert_eq!(
            LP_BZ_CUT_BUILDER_LABEL,
            "pushback-bench-localized-mining-cuts"
        );
        assert_eq!(
            LP_BZ_UNIT_GRANULARITY_LABEL,
            "pushback-bench-localized-cut-phase"
        );
        assert!(
            cut_artifacts.benchmark.phase_plan.phase_count
                > preferred_phase_plan.phase_plan.phase_count,
            "localized cut builder should refine the Marvin primary phase plan for the LP/BZ sidecar"
        );
        assert!(
            cut_artifacts
                .benchmark
                .phase_plan
                .phases
                .iter()
                .any(|phase| phase.phase_id.contains("::pbcut-c")),
            "localized cut builder should emit pbcut phase ids for the LP/BZ sidecar"
        );
        let baseline = build_lp_bz_baseline(
            &repo_root,
            &references_dir,
            config,
            &model,
            &preferred_phase_plan.phase_plan,
            &pcpsp_problem,
            &pcpsp_solution,
            &resource_roles,
            &linear_index_to_row_index,
            &tonnage_column,
            95.0,
            selected_block_count,
            config.selected_block_source,
            &preferred_phase_plan.metadata.aggregation_strategy,
            preferred_phase_plan
                .metadata
                .marvin_nested_shell_family_contract
                .as_ref(),
        )
        .expect("lp/bz baseline should build")
        .expect("marvin should emit LP/BZ baseline");
        let promoted_access_law =
            summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law(
                &baseline.phase_refinement_diagnostics,
            );
        assert_eq!(
            baseline.promoted_build_label,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL
        );
        assert_eq!(
            baseline
                .lp_bz_promotion_readiness
                .local_optimizer_runtime_budget_contract,
            baseline
                .summary
                .lp_bz_round_repair
                .local_optimizer_runtime_budget_contract
        );
        assert_eq!(
            baseline
                .cut_access_law
                .intra_phase_progression
                .front_progression,
            promoted_access_law
                .intra_phase_progression
                .front_progression
        );
        assert_eq!(
            baseline
                .cut_access_law
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy,
            promoted_access_law
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy
        );
        assert_eq!(
            baseline
                .cut_access_law
                .local_predecessor_filter
                .predecessor_window_policy,
            promoted_access_law
                .local_predecessor_filter
                .predecessor_window_policy
        );
        assert_eq!(
            baseline.cut_access_law.ramp_access_contract,
            promoted_access_law.ramp_access_contract
        );
        assert_eq!(
            baseline.cut_access_law.lineage_bench_continuity_contract,
            promoted_access_law.lineage_bench_continuity_contract
        );
        assert_eq!(
            baseline.cut_access_law.complete_cut_design_contract,
            promoted_access_law.complete_cut_design_contract
        );
        assert_eq!(
            baseline.cut_access_law.missing_bibliographic_terms,
            promoted_access_law.missing_bibliographic_terms
        );
        assert_eq!(
            baseline.cut_access_law.bibliographic_gap_contract,
            promoted_access_law.bibliographic_gap_contract
        );
    }
}
