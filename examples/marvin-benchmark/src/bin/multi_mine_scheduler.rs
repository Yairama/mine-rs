//! Ejecuta una validacion multi-mine del scheduler sobre instancias MineLib abiertas.
//!
//! Uso:
//!   cargo run -p marvin-benchmark --bin multi_mine_scheduler [output_path]
//!
//! Si no se especifica `output_path`, el reporte se escribe en
//! `datasets/benchmarks/outputs/multi-mine-scheduling-report.json`.

#[path = "../benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
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

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use benchmark_blocks_support::read_benchmark_blocks;
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
    read_minelib_pcpsp_solution, read_minelib_precedence_graph,
    summarize_minelib_schedule_solution,
};
use mine_sdk::{
    ColumnId, DecomposedSchedulingConfig, Metadata, NumericMetricComparisonReport,
    compare_named_numeric_metrics, solve_decomposed_scheduling_problem,
};
use minelib_scheduling_support::{
    MARVIN_PREFERRED_NESTED_SHELL_FACTOR_COUNT, MarvinPreferredNestedShellFamilyContract,
    MinelibResourceRole, build_candidate_period_memberships, build_linear_index_float_lookup,
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
    PushbackBenchLocalizedCutRefinementDiagnostics,
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

const NESTED_SHELL_PROBE_FACTOR_COUNT: usize = MARVIN_PREFERRED_NESTED_SHELL_FACTOR_COUNT;
const LP_BZ_UNIT_GRANULARITY_LABEL: &str =
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_FAMILY_LABEL;
const LP_BZ_CUT_BUILDER_LABEL: &str = MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL;
const MARVIN_SIDECAR_TRACEABILITY_FIELD_SUFFIXES: &[&str] = &[
    "selected_block_provenance.selected_block_source",
    "selected_block_provenance.selected_block_count",
    "preferred_phase_plan_proxy.aggregation_strategy",
    "preferred_phase_plan_proxy.preferred_nested_shell_factor_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_realized_shell_count",
    "preferred_phase_plan_proxy.preferred_nested_shell_access_mode",
    "localized_cut_builder_provenance.localized_cut_builder_label",
    "localized_cut_builder_provenance.localized_cut_builder_build_label",
    "localized_cut_builder_provenance.scaffold_unit_family_label",
    "localized_cut_builder_provenance.promoted_unit_family_label",
    "localized_cut_builder_provenance.front_progression_label",
];
const MARVIN_SIDECAR_RUNTIME_CONTRACT_FIELD_SUFFIXES: &[&str] = &[
    "summary.lp_bz_lp_kernel.kernel_label",
    "summary.lp_bz_lp_solve.solve_status",
    "summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state",
    "summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit",
    "summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.summary",
    "lp_bz_promotion_readiness.summary",
    "lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract.execution_state",
    "lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract.budget_hit",
];

fn lp_bz_cut_scheduling_limitation_note() -> String {
    let promoted_family_status = format_promoted_lp_bz_family_status_summary(
        LP_BZ_UNIT_GRANULARITY_LABEL,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL,
    );
    format!(
        "Marvin LP/BZ sidecar keeps {promoted_family_status}; its audited `cut_access_law` and provenance/input-aggregation clause remain benchmark-side sidecar evidence (`selected_block_provenance`, `preferred_phase_plan_proxy`, `localized_cut_builder_provenance`) rather than shared/core scheduling logic, so the route still remains exploratory-local evidence rather than a closure-grade mining-cut workflow."
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

fn lp_bz_lp_solve_status_label(status: lp_bz_lp_kernel::LpBzLpSolveStatus) -> &'static str {
    match status {
        lp_bz_lp_kernel::LpBzLpSolveStatus::Optimal => "optimal",
        lp_bz_lp_kernel::LpBzLpSolveStatus::Infeasible => "infeasible",
        lp_bz_lp_kernel::LpBzLpSolveStatus::Unbounded => "unbounded",
        lp_bz_lp_kernel::LpBzLpSolveStatus::Skipped => "skipped",
    }
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
        selected_block_source: "cpit-solution",
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
        selected_block_source: "cpit-solution",
        cpit_problem_file: "mclaughlin_limit.cpit",
        selected_block_solution_file: "mclaughlin_limit_cpit_gmunoz120723.sol",
        pcpsp_problem_file: "mclaughlin_limit.pcpsp",
        pcpsp_solution_file: "mclaughlin_limit_pcpsp_gmunoz120723.sol",
        precedence_file: "mclaughlin_limit.prec",
        upit_objective_file: "mclaughlin_limit.upit",
        lp_cpit_solution_file: Some("mclaughlin_limit.LPcpit"),
        lp_pcpsp_solution_file: None,
        tonnage_column: "field_5",
        nested_shell_probe_enabled: false,
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
        selected_block_source: "cpit-solution",
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
    selected_block_solution_path: String,
    pcpsp_problem_path: String,
    pcpsp_solution_path: String,
    tonnage_column: String,
    aggregation_strategy: String,
    preferred_nested_shell_family_contract: Option<MarvinPreferredNestedShellFamilyContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    marvin_paperlike_pipeline_checklist: Option<MarvinPaperlikePipelineChecklist>,
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
    candidate_summary: CandidateSchedulingSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_period_alignment: PeriodAlignmentSummary,
    candidate_vs_reference_destination_membership: CompactPeriodMembershipComparison,
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
    lp_bz_promotion_readiness: LpBzPromotionReadinessSummary,
    candidate_pcpsp_summary: MinelibScheduleSolutionSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_period_alignment: PeriodAlignmentSummary,
    candidate_vs_reference_destination_membership: CompactPeriodMembershipComparison,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let output_path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        repo_root
            .join("datasets")
            .join("benchmarks")
            .join("outputs")
            .join("multi-mine-scheduling-report.json")
    });
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
            "The selected blocks fed to scheduling come from the staged open CPIT reference per dataset so the benchmark is closer to scheduling semantics than the earlier UPIT-seeded adapter.".to_owned(),
            "Reference-period × bench aggregation is derived from staged CPIT memberships, so the benchmark no longer depends on fixed four-bench bands but still does not reconstruct nested shells or mining cuts from first principles.".to_owned(),
            format!(
                "For Marvin, the report now promotes a bounded {NESTED_SHELL_PROBE_FACTOR_COUNT}-factor nested-shell × bench primary route built from revenue/cost-aware factor scenarios; equivalent factor-aware probes for other datasets still depend on better economic semantics than the open `*.upit` net values alone."
            ),
            "Resource semantics that MineLib leaves to dataset metadata are injected through dataset config (for example, Marvin uses mine+plant capacities while McLaughlin only stages plant capacity).".to_owned(),
            "When staged LP references exist, the report versions them explicitly; only LPpcpsp references are directly comparable to the PCPSP objective, while LPcpit remains a relaxation on the pit-limit problem.".to_owned(),
            "The report includes both mclaughlin-limit and mclaughlin-full; only the limit variant can be aligned directly to the most common MineLib scheduling tables in the literature.".to_owned(),
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
    let dataset_dir = repo_root
        .join("datasets")
        .join("benchmarks")
        .join(config.dataset_id);
    let references_dir = dataset_dir.join("references");
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
    let preferred_phase_plan = build_preferred_phase_plan_for_minelib_scheduling(
        config.dataset_id,
        config.nested_shell_probe_enabled,
        &model,
        &linear_index_to_row_index,
        &selected_solution.assignments,
        primary_precedence_graph.as_ref(),
        &tonnage_column,
        NESTED_SHELL_PROBE_FACTOR_COUNT,
    )?;
    let phase_plan = preferred_phase_plan.phase_plan;
    let phase_plan_metadata = preferred_phase_plan.metadata;
    let preferred_nested_shell_family_contract = phase_plan_metadata
        .marvin_nested_shell_family_contract
        .clone();
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
        selected_block_count,
        &phase_plan_metadata.aggregation_strategy,
        preferred_nested_shell_family_contract.as_ref(),
    )?;
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
        ComparabilityGapSummary {
            gap_id: "selected-block-source-staged-cpit".to_owned(),
            gap_source: ComparabilityGapSource::InputProtocol,
            summary: "selected blocks are seeded from a staged CPIT reference instead of a paper-reproduced shell/pushback generation pipeline".to_owned(),
            evidence_fields: vec![
                "selected_block_source".to_owned(),
                "selected_block_solution_path".to_owned(),
            ],
        },
        aggregation_comparability_gap_summary(
            &phase_plan_metadata.aggregation_strategy,
            preferred_nested_shell_family_contract.as_ref(),
            nested_shell_bench_probe.is_some(),
        ),
        temporal_solver_comparability_gap_summary(lp_bz_baseline.as_ref()),
    ];
    if !config.same_literature_variant {
        push_unique_comparability_gap_summary(
            &mut comparability_gap_contract,
            ComparabilityGapSummary {
                gap_id: "literature-instance-variant-mismatch".to_owned(),
                gap_source: ComparabilityGapSource::InstanceVariant,
                summary: format!(
                    "the executed instance variant `{}` does not match the literature target `{}`",
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
    let benchmark_contract_roles = build_dataset_contract_roles(
        config,
        &phase_plan_metadata.aggregation_strategy,
        nested_shell_bench_probe.is_some(),
        lp_bz_baseline.is_some(),
    );
    let diagnostic_groups_present = build_dataset_diagnostic_groups(
        nested_shell_bench_probe.is_some(),
        lp_bz_baseline.is_some(),
        marvin_paperlike_pipeline_checklist.is_some(),
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
        selected_block_source: config.selected_block_source.to_owned(),
        selected_block_solution_path: selected_block_solution_path.display().to_string(),
        pcpsp_problem_path: pcpsp_problem_path.display().to_string(),
        pcpsp_solution_path: pcpsp_solution_path.display().to_string(),
        tonnage_column: config.tonnage_column.to_owned(),
        aggregation_strategy: phase_plan_metadata.aggregation_strategy,
        preferred_nested_shell_family_contract,
        marvin_paperlike_pipeline_checklist,
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
        candidate_vs_reference_metrics: compare_named_numeric_metrics(
            &solution_metric_map(&reference_summary),
            &solution_metric_map(&candidate_pcpsp_summary),
            &BTreeMap::new(),
        ),
        candidate_vs_reference_period_alignment: compare_period_alignment(
            &pcpsp_solution,
            &candidate_solution,
        ),
        candidate_vs_reference_destination_membership: compare_period_memberships(
            &build_period_destination_memberships(&pcpsp_solution),
            &build_period_destination_memberships(&candidate_solution),
        ),
        reference_summary,
        candidate_summary: CandidateSchedulingSummary {
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
            candidate_pcpsp_summary,
        },
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
    aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
    has_nested_shell_bench_probe: bool,
) -> String {
    aggregation_comparability_gap_summary(
        aggregation_strategy,
        preferred_nested_shell_family_contract,
        has_nested_shell_bench_probe,
    )
    .summary
}

fn aggregation_comparability_gap_summary(
    aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
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
    temporal_solver_comparability_gap_summary(lp_bz_baseline).summary
}

fn temporal_solver_comparability_gap_summary(
    lp_bz_baseline: Option<&LpBzBaselineSummary>,
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
                "the main candidate now runs the Marvin-scoped focused LP/BZ route rebuilt on benchmark-side {LP_BZ_UNIT_GRANULARITY_LABEL} units as the single paper-like candidate family, and its audited `cut_access_law` now separates inter-phase/inter-cut release, local predecessor filtering, intra-phase progression, a benchmark-side partial ramp-access proxy, an explicit working-width proxy, a benchmark-side partial lineage / bench-continuity proxy, a benchmark-side partial complete-cut-design proxy and a structured bibliographic gap contract [{bibliographic_gap_ids}]. {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL} remains just a {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE}, while the active candidate still reports an explicit bounded local-optimizer runtime contract that distinguishes completed execution from budget hits or skips. ramp access, working width, lineage / bench continuity and complete cut design all remain benchmark-side partial proxies, so this remains exploratory-local evidence rather than a closure-grade literature workflow"
            )
        } else {
            format!(
                "the main candidate still uses ready_frontier; `lp_bz_baseline` only adds a Marvin-scoped focused LP/BZ sidecar rebuilt on benchmark-side {LP_BZ_UNIT_GRANULARITY_LABEL} units as the single paper-like candidate family, and its audited `cut_access_law` now separates inter-phase/inter-cut release, local predecessor filtering, intra-phase progression, a benchmark-side partial ramp-access proxy, an explicit working-width proxy, a benchmark-side partial lineage / bench-continuity proxy, a benchmark-side partial complete-cut-design proxy and a structured bibliographic gap contract [{bibliographic_gap_ids}]. {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL} remains just a {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE}, while the promoted sidecar now reports an explicit bounded local-optimizer runtime contract that distinguishes completed execution from budget hits or skips. ramp access, working width, lineage / bench continuity and complete cut design all remain benchmark-side partial proxies, so this remains exploratory-local evidence rather than a closure-grade literature workflow"
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
    } else {
        ComparabilityGapSummary {
            gap_id: "missing-lp-bz-sidecar".to_owned(),
            gap_source: ComparabilityGapSource::RelaxationModel,
            summary: "the temporal solver is still ready_frontier and no LP/BZ sidecar is available on this dataset, so the benchmark still lacks an LP/BZ-guided baseline with rounding or another literature-grade workflow".to_owned(),
            evidence_fields: vec![
                "candidate_summary".to_owned(),
                "reference_period_routed_baseline".to_owned(),
            ],
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
                report_surface: vec![
                    "datasets[*].aggregation_strategy".to_owned(),
                    "datasets[*].preferred_nested_shell_family_contract".to_owned(),
                    "datasets[*].marvin_paperlike_pipeline_checklist".to_owned(),
                    "datasets[*].benchmark_contract_roles".to_owned(),
                    "datasets[*].comparability_gaps".to_owned(),
                ],
                limitations: vec![
                    "still relies on staged CPIT memberships or bounded nested-shell proxies rather than first-principles paper-grade pushback generation".to_owned(),
                ],
            },
            BenchmarkContractModuleSummary {
                module_path: "examples/marvin-benchmark/src/pushback_bench_localized_cut_support.rs"
                    .to_owned(),
                contract_role:
                    "shared localized-cut builder, access-law summary and refinement diagnostics"
                        .to_owned(),
                scope_label: "marvin benchmark-shared".to_owned(),
                maturity_label: "reusable benchmark contract".to_owned(),
                report_surface: {
                    let mut report_surface = marvin_sidecar_traceability_field_paths(
                        "datasets[*].lp_bz_baseline.unit_family_traceability",
                    );
                    report_surface.extend([
                        "datasets[*].lp_bz_baseline.cut_access_law".to_owned(),
                        "datasets[*].lp_bz_baseline.phase_refinement_diagnostics".to_owned(),
                        "datasets[*].lp_bz_baseline.lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract".to_owned(),
                        "datasets[*].marvin_paperlike_pipeline_checklist".to_owned(),
                    ]);
                    report_surface
                },
                limitations: vec![
                    lp_bz_cut_scheduling_limitation_note(),
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
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit".to_owned(),
                    "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract".to_owned(),
                    "datasets[*].lp_bz_baseline.candidate_vs_reference_metrics".to_owned(),
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
                    "datasets[*].marvin_paperlike_pipeline_checklist".to_owned(),
                ],
                limitations: vec![
                    "the report can formalize evidence and gaps, including the benchmark-side sidecar-only provenance/input-aggregation clause, but it does not upgrade exploratory-local methods into literature-grade pipelines by itself".to_owned(),
                ],
            },
        ],
        promotion_rules: vec![
            "Only reusable benchmark contracts should flow into shared report surfaces; exploratory sidecars must stay explicitly labeled.".to_owned(),
            "Benchmark-side modules may become primary baselines only when their units, access law and financial assumptions are paper-comparable.".to_owned(),
            format!(
                "For MR-187.A/B, `{LP_BZ_UNIT_GRANULARITY_LABEL}` is the only benchmark-side paper-like candidate family, its audited `cut_access_law` stays exploratory-local until ramp access, working width and lineage / bench continuity mature beyond benchmark-side partial coverage and complete cut design exists, and `{MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL}` must remain labeled as a {MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE}."
            ),
            "Marvin-specific support must remain outside core crates even when promoted inside benchmark reporting.".to_owned(),
        ],
    }
}

fn build_benchmark_diagnostics_schema() -> BenchmarkDiagnosticsSchema {
    BenchmarkDiagnosticsSchema {
        schema_version: "v1".to_owned(),
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
                    "instance_variant".to_owned(),
                    "literature_reference_instance".to_owned(),
                    "aggregation_strategy".to_owned(),
                    "preferred_nested_shell_family_contract".to_owned(),
                    "selected_block_source".to_owned(),
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
                        "preferred_nested_shell_family_contract".to_owned(),
                        "lp_bz_baseline.cut_access_law".to_owned(),
                        "lp_bz_baseline.phase_refinement_diagnostics".to_owned(),
                        "lp_bz_baseline.lp_bz_promotion_readiness".to_owned(),
                        "lp_bz_baseline.lp_bz_promotion_readiness.local_optimizer_runtime_budget_contract".to_owned(),
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
                group_name: "phase-plan-and-access-law".to_owned(),
                fields: {
                    let mut fields =
                        marvin_sidecar_traceability_field_paths("lp_bz_baseline.unit_family_traceability");
                    fields.extend([
                        "benchmark_contract_roles".to_owned(),
                        "diagnostic_groups_present".to_owned(),
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
    if config.dataset_id == "marvin" {
        roles.push("marvin-focused-research-harness".to_owned());
    }
    roles
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
        "Kernel `{}` reports native LP solve status `{}` for the promoted LP/BZ route, and the explicit local-optimizer runtime contract stays `{}` with `budget_hit={}`. {}",
        lp_kernel.kernel_label,
        lp_bz_lp_solve_status_label(lp_solve.solve_status),
        runtime_budget_contract.execution_state,
        runtime_budget_contract.budget_hit,
        runtime_budget_contract.summary,
    );
    let classification_summary = format!(
        "The Marvin dataset stays `{comparison_classification}` with {} explicit comparability gaps, so the paper-like path remains audited benchmark evidence rather than a silent literature-grade promotion.",
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

fn build_dataset_diagnostic_groups(
    has_nested_shell_bench_probe: bool,
    has_lp_bz_baseline: bool,
    has_marvin_paperlike_pipeline_checklist: bool,
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
fn build_marvin_lp_bz_sidecar_artifacts(
    config: &DatasetConfig,
    model: &mine_sdk::BlockModel,
    base_phase_plan: &mine_sdk::PushbackPlan,
    pcpsp_problem: &MinelibScheduleProblem,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    tonnage_column: &ColumnId,
) -> Result<PushbackBenchLocalizedCutBuildArtifacts<mine_sdk::SchedulingProblem>, mine_sdk::MineError>
{
    let tonnage_by_linear_index = build_linear_index_float_lookup(model, tonnage_column)?;
    build_pushback_bench_localized_cut_benchmark_artifacts(
        model,
        base_phase_plan,
        &tonnage_by_linear_index,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        |phase_plan| {
            build_scheduling_problem_from_minelib_problem(
                phase_plan,
                pcpsp_problem,
                config.dataset_id,
                resource_roles,
                &lp_bz_cut_scheduling_limitation_note(),
            )
        },
    )
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
    selected_block_count: usize,
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
    let promoted_contract_surfaces =
        build_marvin_mr187_promoted_pushback_bench_localized_cut_contract_surfaces(
            config.selected_block_source,
            selected_block_count,
            preferred_phase_plan_aggregation_strategy,
            preferred_nested_shell_family_contract,
            LP_BZ_UNIT_GRANULARITY_LABEL,
            &cut_artifacts.phase_refinement_diagnostics,
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
        candidate_vs_reference_destination_membership: compare_period_memberships(
            &build_period_destination_memberships(pcpsp_solution),
            &build_period_destination_memberships(&candidate_solution),
        ),
    };
    validate_promoted_pushback_bench_localized_cut_access_law_contract(
        &baseline.cut_access_law,
        &baseline.promoted_build_label,
    )?;
    validate_promoted_pushback_bench_localized_cut_unit_family_traceability(
        &baseline.unit_family_traceability,
        config.selected_block_source,
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
    validate_lp_bz_baseline_runtime_budget_contract(&baseline)?;
    Ok(Some(baseline))
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
        NESTED_SHELL_PROBE_FACTOR_COUNT, aggregation_comparability_gap,
        build_benchmark_contract_audit, build_dataset_contract_roles,
        build_linear_index_to_row_index, build_lp_bz_baseline,
        build_marvin_lp_bz_sidecar_artifacts, build_marvin_paperlike_pipeline_checklist,
        build_marvin_preferred_nested_shell_family_contract,
        build_preferred_phase_plan_for_minelib_scheduling,
        build_promoted_pushback_bench_localized_cut_unit_family_traceability,
        read_benchmark_blocks, read_minelib_cpit_solution, read_minelib_pcpsp_problem,
        read_minelib_pcpsp_solution, read_minelib_precedence_graph,
        summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law,
        supports_lp_bz_baseline, temporal_solver_comparability_gap,
        validate_promoted_pushback_bench_localized_cut_access_law_contract,
    };
    use mine_sdk::ColumnId;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    const SAMPLE_LP_BZ_SELECTED_BLOCK_COUNT: usize = 321;

    fn sample_lp_bz_baseline_summary() -> super::LpBzBaselineSummary {
        let preferred_shell_family =
            build_marvin_preferred_nested_shell_family_contract(NESTED_SHELL_PROBE_FACTOR_COUNT)
                .expect("preferred shell family should build")
                .with_realized_shell_count(5);
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
        super::LpBzBaselineSummary {
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
                    "cpit-solution",
                    SAMPLE_LP_BZ_SELECTED_BLOCK_COUNT,
                    "nested-shell-bench",
                    Some(&preferred_shell_family),
                    MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL,
                    LP_BZ_UNIT_GRANULARITY_LABEL,
                    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
                    "uniform-33-67-100",
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
                    local_optimization_skipped: false,
                    local_optimizer_runtime_budget_contract:
                        local_optimizer_runtime_budget_contract.clone(),
                    local_optimizer_strategy_label:
                        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
                            .to_owned(),
                    local_optimizer_termination_reason: "no-improving-local-move".to_owned(),
                    local_optimizer_executed_iteration_count: 2,
                    local_optimizer_improving_move_count: 1,
                    repaired_phase_target_count: 0,
                    repaired_unit_target_count: 0,
                    horizon_clamp_count: 0,
                    phase_target_count: 0,
                    unit_target_count: 0,
                    limitations: Vec::new(),
                },
                limitations: Vec::new(),
            },
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
                undiscounted_objective: 0.0,
                discounted_objective: 0.0,
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
        }
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
        baseline
    }

    #[test]
    fn promoted_nested_shell_gap_mentions_dynamic_strategy_and_shell_count() {
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred Marvin shell family should build")
            .with_realized_shell_count(5);
        let gap = aggregation_comparability_gap(
            "nested-shell-bench",
            Some(&preferred_shell_family),
            false,
        );

        assert!(gap.contains("nested-shell-bench"));
        assert!(gap.contains("5 bounded shells"));
        assert!(gap.contains("7-factor strict-sequential family"));
    }

    #[test]
    fn reported_probe_gap_stays_on_reference_period_primary_pipeline() {
        let gap = aggregation_comparability_gap("reference-period-bench", None, true);

        assert!(gap.contains("reference-period × bench"));
        assert!(gap.contains("nested-shell × bench probe is reported"));
    }

    #[test]
    fn fallback_gap_without_probe_keeps_reference_period_message() {
        let gap = aggregation_comparability_gap("reference-period-bench", None, false);

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
                .any(|field| field == "datasets[*].marvin_paperlike_pipeline_checklist")
        );
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
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
        }));
        assert!(lp_bz_adapter.report_surface.iter().any(|field| {
            field
                == "datasets[*].lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
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
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.execution_state"
        }));
        assert!(paperlike_group.fields.iter().any(|field| {
            field
                == "lp_bz_baseline.summary.lp_bz_round_repair.local_optimizer_runtime_budget_contract.budget_hit"
        }));
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
        assert_eq!(checklist.items.len(), 7);
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
                    .contains("selected_block_source = \"cpit-solution\"")
                && item.summary.contains(
                    "Quantitatively, 321 selected blocks currently compress into 18 promoted phases and 12 LP/BZ scheduling units."
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
        }));
        assert!(checklist.items.iter().any(|item| {
            item.contract_id == "promoted-lp-bz-runtime-solve-path"
                    && item.summary.contains("Kernel `lp-bz-lp-kernel-v8`")
                    && item.summary.contains("native LP solve status `optimal`")
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
            item.contract_id == "promoted-paperlike-lp-bz-family"
                && item
                    .summary
                    .contains("selected_block_source = \"cpit-solution\"")
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
    fn lp_bz_baseline_only_targets_marvin_with_lp_pcpsp_reference() {
        assert!(supports_lp_bz_baseline(&DATASETS[0]));
        assert!(!supports_lp_bz_baseline(&DATASETS[1]));
        assert!(!supports_lp_bz_baseline(&DATASETS[2]));
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
    }

    #[test]
    fn temporal_solver_gap_summary_is_classified_as_relaxation_model() {
        let baseline = sample_lp_bz_baseline_summary();
        let gap = super::temporal_solver_comparability_gap_summary(Some(&baseline));

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

    #[test]
    fn aggregation_gap_summary_is_classified_as_aggregation_formulation() {
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred Marvin shell family should build")
            .with_realized_shell_count(5);
        let gap = super::aggregation_comparability_gap_summary(
            "nested-shell-bench",
            Some(&preferred_shell_family),
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
    fn marvin_input_aggregation_gap_summary_surfaces_layered_traceability() {
        let baseline = sample_lp_bz_baseline_summary();
        let gap = super::marvin_input_aggregation_traceability_gap_summary(&baseline);

        assert_eq!(gap.gap_source, super::ComparabilityGapSource::InputProtocol);
        assert_eq!(gap.gap_id, "marvin-input-aggregation-traceability");
        assert!(
            gap.summary
                .contains("selected_block_source = \"cpit-solution\"")
        );
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
            "Input/aggregation traceability now stays explicit across three benchmark-side layers: `selected_block_source = \"cpit-solution\"` seeds the admissible block set; the bounded `nested-shell-bench` bridge keeps 7 revenue factors on `strict-sequential` access and realizes 5 shells before localized-cut refinement; and builder `pushback-bench-localized-mining-cuts` / build `front3-ar2.0-span2-n6` refines scaffold `shape-gated-local-front-phase` into promoted `pushback-bench-localized-cut-phase` units under `uniform-33-67-100` progression. Quantitatively, 321 selected blocks currently compress into 18 promoted phases and 12 LP/BZ scheduling units. The route remains `exploratory-local` because the block provenance still starts from a staged benchmark-side selection and the intermediate shell family is still a reproducible proxy rather than a paper-reproduced pushback/mining-cut pipeline."
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
    }

    #[test]
    fn dataset_contract_roles_label_paperlike_candidate_and_scaffold() {
        let roles = build_dataset_contract_roles(&DATASETS[0], "nested-shell-bench", true, true);

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
    fn temporal_solver_gap_mentions_absence_when_lp_bz_sidecar_is_missing() {
        let gap = temporal_solver_comparability_gap(None);

        assert!(gap.contains("no LP/BZ sidecar is available"));
        assert!(!gap.contains("lp_bz_baseline"));
    }

    #[test]
    fn marvin_lp_bz_sidecar_builder_uses_pushback_bench_localized_cut_units() {
        let config = &DATASETS[0];
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
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
            selected_block_count,
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
