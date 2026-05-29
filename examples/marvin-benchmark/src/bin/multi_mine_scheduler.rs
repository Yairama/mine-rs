//! Ejecuta una validacion multi-mine del scheduler sobre instancias MineLib abiertas.
//!
//! Uso:
//!   cargo run -p marvin-benchmark --bin multi_mine_scheduler [output_path]
//!
//! Si no se especifica `output_path`, el reporte se escribe en
//! `datasets/benchmarks/outputs/multi-mine-scheduling-report.json`.

#[path = "../benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
#[path = "../lp_bz_adapter.rs"]
mod lp_bz_adapter;
#[path = "../lp_bz_bound.rs"]
mod lp_bz_bound;
#[path = "../lp_bz_lp_kernel.rs"]
mod lp_bz_lp_kernel;
#[path = "../lp_bz_rounder.rs"]
mod lp_bz_rounder;
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
use lp_bz_adapter::{MarvinLpBzAdapterSummary, run_marvin_focused_lp_bz_adapter};
use marvin_support::{
    MinelibScheduleAssignment, MinelibScheduleProblem, MinelibScheduleSolution,
    MinelibScheduleSolutionSummary, read_minelib_cpit_problem, read_minelib_cpit_solution,
    read_minelib_lp_cpit_solution, read_minelib_lp_pcpsp_solution, read_minelib_pcpsp_problem,
    read_minelib_pcpsp_solution, read_minelib_precedence_graph,
    summarize_minelib_schedule_solution,
};
use mine_sdk::{
    ColumnId, DecomposedSchedulingConfig, Metadata, NestingAccessRules,
    NumericMetricComparisonReport, compare_named_numeric_metrics,
    solve_decomposed_scheduling_problem, uniform_revenue_factors,
};
use minelib_scheduling_support::{
    MinelibResourceRole, build_candidate_period_memberships, build_linear_index_float_lookup,
    build_linear_index_to_row_index, build_marvin_phase_plan_from_revenue_factor_shells,
    build_preferred_phase_plan_for_minelib_scheduling,
    build_scheduling_problem_from_minelib_problem,
};
use pushback_bench_localized_cut_support::{
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
    PushbackBenchLocalizedCutAccessPolicySummary, PushbackBenchLocalizedCutBuildArtifacts,
    PushbackBenchLocalizedCutRefinementDiagnostics,
    build_pushback_bench_localized_cut_benchmark_artifacts,
    summarize_pushback_bench_localized_cut_build_config,
};
use serde::Serialize;

const NESTED_SHELL_PROBE_FACTOR_COUNT: usize = 7;
const LP_BZ_UNIT_GRANULARITY_LABEL: &str = "pushback-bench-localized-cut-phase";
const LP_BZ_CUT_BUILDER_LABEL: &str = "pushback-bench-localized-mining-cuts";
const LP_BZ_CUT_SCHEDULING_LIMITATION_NOTE: &str = "Marvin LP/BZ sidecar rebuilds the benchmark-side scheduling problem on pushback-bench-localized-cut units; this remains exploratory-local evidence rather than a closure-grade mining-cut workflow.";

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
    comparability_gaps: Vec<String>,
    dataset_dir: String,
    blocks_path: String,
    selected_block_source: String,
    selected_block_solution_path: String,
    pcpsp_problem_path: String,
    pcpsp_solution_path: String,
    tonnage_column: String,
    aggregation_strategy: String,
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
    cut_access_law: PushbackBenchLocalizedCutAccessPolicySummary,
    phase_refinement_diagnostics: PushbackBenchLocalizedCutRefinementDiagnostics,
    summary: MarvinLpBzAdapterSummary,
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
    let mut comparability_gaps = vec![
        "selected blocks are seeded from a staged CPIT reference instead of a paper-reproduced shell/pushback generation pipeline".to_owned(),
        aggregation_comparability_gap(
            &phase_plan_metadata.aggregation_strategy,
            phase_plan_metadata.nested_shell_primary,
            phase_plan_metadata.unique_shell_count,
            nested_shell_bench_probe.is_some(),
        ),
        temporal_solver_comparability_gap(lp_bz_baseline.is_some()),
    ];
    if !config.same_literature_variant {
        comparability_gaps.push(format!(
            "the executed instance variant `{}` does not match the literature target `{}`",
            config.instance_variant, config.literature_reference_instance
        ));
    }
    for limitation in &phase_plan_metadata.limitations {
        if !comparability_gaps.contains(limitation) {
            comparability_gaps.push(limitation.clone());
        }
    }
    let comparison_classification = if comparability_gaps.is_empty() {
        "paper-comparable"
    } else {
        "exploratory-local"
    };

    Ok(DatasetSchedulingReport {
        dataset_id: config.dataset_id.to_owned(),
        instance_id: config.instance_id.to_owned(),
        instance_variant: config.instance_variant.to_owned(),
        literature_reference_instance: config.literature_reference_instance.to_owned(),
        same_literature_variant: config.same_literature_variant,
        comparison_classification: comparison_classification.to_owned(),
        comparability_gaps,
        dataset_dir: dataset_dir.display().to_string(),
        blocks_path: blocks_path.display().to_string(),
        selected_block_source: config.selected_block_source.to_owned(),
        selected_block_solution_path: selected_block_solution_path.display().to_string(),
        pcpsp_problem_path: pcpsp_problem_path.display().to_string(),
        pcpsp_solution_path: pcpsp_solution_path.display().to_string(),
        tonnage_column: config.tonnage_column.to_owned(),
        aggregation_strategy: phase_plan_metadata.aggregation_strategy,
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
            selected_block_count: selected_linear_indices.len(),
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

fn aggregation_comparability_gap(
    aggregation_strategy: &str,
    nested_shell_primary: bool,
    unique_shell_count: Option<usize>,
    has_nested_shell_bench_probe: bool,
) -> String {
    if nested_shell_primary {
        match unique_shell_count {
            Some(unique_shell_count) => format!(
                "the main candidate now uses {aggregation_strategy} units backed by {unique_shell_count} bounded shells, but the shell family is still a reproducible revenue/cost-aware proxy rather than a paper-reproduced pushback pipeline"
            ),
            None => format!(
                "the main candidate now uses {aggregation_strategy} units, but the shell family is still a bounded reproducible proxy built from revenue/cost-aware factor scenarios rather than a paper-reproduced pushback pipeline"
            ),
        }
    } else if has_nested_shell_bench_probe {
        "the main candidate still uses reference-period × bench units; a separate bounded nested-shell × bench probe is reported, but it is not yet the primary paper-comparable pipeline".to_owned()
    } else {
        "reference-period × bench units are still derived from staged CPIT memberships rather than from nested-shell pushbacks or literature-grade mining cuts".to_owned()
    }
}

fn temporal_solver_comparability_gap(has_lp_bz_baseline: bool) -> String {
    if has_lp_bz_baseline {
        "the main candidate still uses ready_frontier; `lp_bz_baseline` only adds a Marvin-scoped focused LP/BZ sidecar rebuilt on benchmark-side pushback-bench-localized-cut units and seeded from LPpcpsp, and that adapter intentionally skips local optimization, so this remains exploratory-local evidence rather than a closure-grade literature workflow".to_owned()
    } else {
        "the temporal solver is still ready_frontier and no LP/BZ sidecar is available on this dataset, so the benchmark still lacks an LP/BZ-guided baseline with rounding or another literature-grade workflow".to_owned()
    }
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
    let phase_plan = build_marvin_phase_plan_from_revenue_factor_shells(
        model,
        precedence_graph,
        &uniform_revenue_factors(NESTED_SHELL_PROBE_FACTOR_COUNT)?,
        NestingAccessRules::strict_sequential(),
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
                LP_BZ_CUT_SCHEDULING_LIMITATION_NOTE,
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

    Ok(Some(LpBzBaselineSummary {
        phase_plan_builder_label: LP_BZ_CUT_BUILDER_LABEL.to_owned(),
        unit_granularity_label: LP_BZ_UNIT_GRANULARITY_LABEL.to_owned(),
        cut_access_law: summarize_pushback_bench_localized_cut_build_config(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        ),
        phase_refinement_diagnostics: cut_artifacts.phase_refinement_diagnostics,
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
    }))
}

fn supports_lp_bz_baseline(config: &DatasetConfig) -> bool {
    config.dataset_id == "marvin" && config.benchmark_family == "marvin"
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
    let revenue_factors = uniform_revenue_factors(NESTED_SHELL_PROBE_FACTOR_COUNT)?;
    let shell_artifacts = if config.dataset_id == "marvin" {
        build_marvin_phase_plan_from_revenue_factor_shells(
            model,
            &precedence_graph,
            &revenue_factors,
            NestingAccessRules::strict_sequential(),
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
        aggregation_strategy: "nested-shell-bench".to_owned(),
        revenue_factor_count: revenue_factors.len(),
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
        NESTED_SHELL_PROBE_FACTOR_COUNT, aggregation_comparability_gap,
        build_linear_index_to_row_index, build_marvin_lp_bz_sidecar_artifacts,
        build_preferred_phase_plan_for_minelib_scheduling, read_benchmark_blocks,
        read_minelib_cpit_solution, read_minelib_pcpsp_problem, read_minelib_precedence_graph,
        supports_lp_bz_baseline, temporal_solver_comparability_gap,
    };
    use mine_sdk::ColumnId;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn promoted_nested_shell_gap_mentions_dynamic_strategy_and_shell_count() {
        let gap = aggregation_comparability_gap("nested-shell-bench", true, Some(5), false);

        assert!(gap.contains("nested-shell-bench"));
        assert!(gap.contains("5 bounded shells"));
    }

    #[test]
    fn reported_probe_gap_stays_on_reference_period_primary_pipeline() {
        let gap = aggregation_comparability_gap("reference-period-bench", false, None, true);

        assert!(gap.contains("reference-period × bench"));
        assert!(gap.contains("nested-shell × bench probe is reported"));
    }

    #[test]
    fn fallback_gap_without_probe_keeps_reference_period_message() {
        let gap = aggregation_comparability_gap("reference-period-bench", false, None, false);

        assert!(gap.contains("reference-period × bench units are still derived"));
        assert!(!gap.contains("probe is reported"));
    }

    #[test]
    fn lp_bz_baseline_only_targets_marvin_with_lp_pcpsp_reference() {
        assert!(supports_lp_bz_baseline(&DATASETS[0]));
        assert!(!supports_lp_bz_baseline(&DATASETS[1]));
        assert!(!supports_lp_bz_baseline(&DATASETS[2]));
    }

    #[test]
    fn temporal_solver_gap_mentions_lp_bz_sidecar_when_present() {
        let gap = temporal_solver_comparability_gap(true);

        assert!(gap.contains("lp_bz_baseline"));
        assert!(gap.contains("Marvin-scoped"));
        assert!(gap.contains("pushback-bench-localized-cut units"));
        assert!(gap.contains("skips local optimization"));
    }

    #[test]
    fn temporal_solver_gap_mentions_absence_when_lp_bz_sidecar_is_missing() {
        let gap = temporal_solver_comparability_gap(false);

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
        let precedence_graph =
            read_minelib_precedence_graph(&references_dir.join(config.precedence_file), &model)
                .expect("precedence graph should load");
        let pcpsp_problem =
            read_minelib_pcpsp_problem(&references_dir.join(config.pcpsp_problem_file), &model)
                .expect("pcpsp problem should load");
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
    }
}
