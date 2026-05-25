//! Ejemplo ejecutable para comparar referencias Marvin locales contra salidas actuales de `mine-rs`.
//!
//! Uso:
//!   cargo run -p marvin-benchmark [dataset_dir] [output_path]
//!
//! Si no se especifican argumentos, el dataset se toma desde `datasets/benchmarks/marvin/`
//! y el reporte se escribe en `datasets/benchmarks/marvin/outputs/comparison-report.json`.

mod benchmark_blocks_support;
mod marvin_support;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleSolution,
    MarvinScheduleSolutionSummary, read_marvin_cpit_problem, read_marvin_cpit_solution,
    read_marvin_lp_cpit_solution, read_marvin_lp_pcpsp_solution, read_marvin_pcpsp_problem,
    read_marvin_pcpsp_solution, read_marvin_precedence_graph, read_marvin_upit_block_values,
    read_marvin_upit_solution, summarize_marvin_schedule_solution,
};
use mine_sdk::{
    BenchParameters, BlockModel, BlockPrecedenceTemplate, ColumnData, ColumnId,
    DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
    DestinationKind, DestinationPayability, DestinationRecovery, EconomicBlockModel,
    EconomicBlockModelConfig, LongTermScheduleEconomicsReport, MeasurementUnit, Metadata,
    MetadataValue, ModelId, NumericMetricComparisonReport, NumericMetricTolerance, PhaseDesign,
    PrecedenceNode, PrecedenceOffset, PushbackPlan, ScenarioId, ScheduleDestinationId,
    SchedulingObjectiveTerm, SchedulingPeriod, SchedulingProblem, SchedulingResourceBound,
    SchedulingResourceId, SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId,
    assign_benches, build_block_precedence_graph, build_max_closure_graph,
    build_ready_frontier_long_term_schedule, build_upit_prototype, compare_block_memberships,
    compare_named_numeric_metrics, evaluate_long_term_schedule_economics, solve_upl_exact,
};
use serde::Serialize;

const OFFICIAL_CPIT_OBJECTIVE: f64 = 820_726_048.0;
const OFFICIAL_LP_CPIT_OBJECTIVE: f64 = 863_916_131.0;
const OFFICIAL_PCPSP_OBJECTIVE: f64 = 885_968_070.0;
const OFFICIAL_LP_PCPSP_OBJECTIVE: f64 = 911_704_665.0;
const PHASE_BENCH_SPAN: i64 = 4;

#[derive(Debug, Serialize)]
struct MarvinBenchmarkOutput {
    dataset_dir: String,
    reference_prec_path: String,
    reference_upit_solution_path: String,
    reference_upit_objective_path: String,
    reference_cpit_problem_path: String,
    reference_cpit_solution_path: String,
    reference_pcpsp_problem_path: String,
    reference_pcpsp_solution_path: String,
    reference_lp_cpit_solution_path: String,
    reference_lp_pcpsp_solution_path: String,
    value_column: String,
    tonnage_column: String,
    candidate_predecessor_offsets: Vec<(isize, isize, isize)>,
    reference_precedence: PrecedenceArtifactSummary,
    candidate_precedence: PrecedenceArtifactSummary,
    precedence_comparison: CompactPrecedenceComparison,
    reference_upit: MembershipArtifactSummary,
    candidate_upit: MembershipArtifactSummary,
    exact_upit: MembershipArtifactSummary,
    upit_membership_comparison: CompactMembershipComparison,
    exact_upit_membership_comparison: CompactMembershipComparison,
    upit_metric_comparison: NumericMetricComparisonReport,
    exact_upit_metric_comparison: NumericMetricComparisonReport,
    cpit_reference: ScheduleReferenceArtifactSummary,
    pcpsp_reference: ScheduleReferenceArtifactSummary,
    lp_cpit_reference: ScheduleReferenceArtifactSummary,
    lp_pcpsp_reference: ScheduleReferenceArtifactSummary,
    mine_rs_end_to_end: MineRsEndToEndSummary,
    mine_rs_vs_cpit_metric_comparison: NumericMetricComparisonReport,
    mine_rs_vs_cpit_membership_comparison: CompactPeriodMembershipComparison,
    mine_rs_vs_cpit_period_metric_comparison: NumericMetricComparisonReport,
    mine_rs_vs_pcpsp_metric_comparison: NumericMetricComparisonReport,
    mine_rs_vs_pcpsp_membership_comparison: CompactPeriodMembershipComparison,
    mine_rs_vs_pcpsp_period_metric_comparison: NumericMetricComparisonReport,
    assumptions: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PrecedenceArtifactSummary {
    node_count: usize,
    edge_count: usize,
}

#[derive(Debug, Serialize)]
struct MembershipArtifactSummary {
    block_count: usize,
    /// Suma de proc_profit × tonelaje para los bloques seleccionados.
    total_proc_profit_x_tonnage: f64,
    /// Objetivo económico UPIT: sum((max(proc_profit, 0) - mine_cost) × tonnage).
    total_economic_objective: f64,
    total_tonnage: f64,
}

#[derive(Debug, Serialize)]
struct CompactPrecedenceComparison {
    shared_nodes: usize,
    shared_edges: usize,
    reference_only_edge_count: usize,
    candidate_only_edge_count: usize,
    node_jaccard_index: f64,
    edge_jaccard_index: f64,
    reference_only_edge_examples: Vec<(usize, usize)>,
    candidate_only_edge_examples: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize)]
struct CompactMembershipComparison {
    shared_blocks: usize,
    reference_only_block_count: usize,
    candidate_only_block_count: usize,
    jaccard_index: f64,
    reference_only_block_examples: Vec<usize>,
    candidate_only_block_examples: Vec<usize>,
}

#[derive(Debug, Serialize)]
struct ScheduleReferenceArtifactSummary {
    period_count: usize,
    destination_count: usize,
    resource_constraint_count: usize,
    discount_rate: f64,
    official_objective: f64,
    objective_gap_vs_official: f64,
    solution_summary: MarvinScheduleSolutionSummary,
}

#[derive(Debug, Serialize)]
struct MineRsEndToEndSummary {
    phase_count: usize,
    total_block_count: usize,
    schedule_period_count: usize,
    schedule_entry_count: usize,
    schedule_violation_count: usize,
    total_tonnage: f64,
    total_cashflow: f64,
    npv: f64,
    periods: Vec<MineRsPeriodSummary>,
}

#[derive(Debug, Serialize)]
struct MineRsPeriodSummary {
    period_label: String,
    tonnage: f64,
    cashflow: f64,
    discounted_cashflow: f64,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let dataset_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("datasets").join("benchmarks").join("marvin"));
    let output_path = env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| dataset_dir.join("outputs").join("comparison-report.json"));
    let blocks_path = dataset_dir.join("marvin.blocks");
    let references_dir = dataset_dir.join("references");
    let prec_path = references_dir.join("marvin.prec");
    let upit_solution_path = references_dir.join("marvin_upit.sol");
    let upit_objective_path = references_dir.join("marvin.upit");
    let cpit_problem_path = references_dir.join("marvin.cpit");
    let cpit_solution_path = references_dir.join("marvin_cpit_gmunoz120723.sol");
    let pcpsp_problem_path = references_dir.join("marvin.pcpsp");
    let pcpsp_solution_path = references_dir.join("marvin_pcpsp_gmunoz120723.sol");
    let lp_cpit_solution_path = references_dir.join("marvin.LPcpit");
    let lp_pcpsp_solution_path = references_dir.join("marvin.LPpcpsp");

    let model = read_benchmark_blocks(&blocks_path, "marvin")?;
    let reference_prec = read_marvin_precedence_graph(&prec_path, &model)?;
    let reference_upit_membership = read_marvin_upit_solution(&upit_solution_path, &model)?;
    let exact_upit_weights = read_marvin_upit_block_values(&upit_objective_path, &model)?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let cpit_problem = read_marvin_cpit_problem(&cpit_problem_path, &model)?;
    let cpit_solution = read_marvin_cpit_solution(&cpit_solution_path, &model)?;
    let pcpsp_problem = read_marvin_pcpsp_problem(&pcpsp_problem_path, &model)?;
    let pcpsp_solution = read_marvin_pcpsp_solution(&pcpsp_solution_path, &model)?;
    let lp_cpit_solution = read_marvin_lp_cpit_solution(&lp_cpit_solution_path, &model)?;
    let lp_pcpsp_solution = read_marvin_lp_pcpsp_solution(&lp_pcpsp_solution_path, &model)?;
    let cpit_summary = summarize_marvin_schedule_solution(&cpit_problem, &cpit_solution)?;
    let pcpsp_summary = summarize_marvin_schedule_solution(&pcpsp_problem, &pcpsp_solution)?;
    let lp_cpit_summary = summarize_marvin_schedule_solution(&cpit_problem, &lp_cpit_solution)?;
    let lp_pcpsp_summary = summarize_marvin_schedule_solution(&pcpsp_problem, &lp_pcpsp_solution)?;

    let template = marvin_slope_template()?;
    let candidate_prec = build_block_precedence_graph(&model, &template)?;
    let precedence_comparison = compact_precedence_comparison(mine_sdk::compare_precedence_graphs(
        &reference_prec,
        &candidate_prec,
    ));

    let value_column = ColumnId::new("field_7")?;
    let tonnage_column = ColumnId::new("field_4")?;
    let exact_upit_result = solve_upl_exact(&build_max_closure_graph(
        &exact_upit_weights,
        &reference_prec,
    )?)?;
    let candidate_upit = build_upit_prototype(
        &model,
        &candidate_prec,
        &value_column,
        Some(&tonnage_column),
    )?;
    let upit_membership_comparison = compact_membership_comparison(compare_block_memberships(
        &reference_upit_membership,
        &candidate_upit.selected_linear_indices,
    ));
    let exact_upit_membership_comparison =
        compact_membership_comparison(compare_block_memberships(
            &reference_upit_membership,
            &exact_upit_result.selected_blocks,
        ));

    let reference_upit_metrics = membership_metrics(
        &model,
        &reference_upit_membership,
        &value_column,
        &tonnage_column,
    )?;
    let candidate_upit_metrics = {
        let mut m = membership_metrics(
            &model,
            &candidate_upit.selected_linear_indices,
            &value_column,
            &tonnage_column,
        )?;
        m.insert("block_count".to_owned(), candidate_upit.block_count as f64);
        m
    };
    let exact_upit_metrics = {
        let mut m = membership_metrics(
            &model,
            &exact_upit_result.selected_blocks,
            &value_column,
            &tonnage_column,
        )?;
        m.insert(
            "block_count".to_owned(),
            exact_upit_result.selected_block_count as f64,
        );
        m
    };
    let upit_metric_comparison = compare_named_numeric_metrics(
        &reference_upit_metrics,
        &candidate_upit_metrics,
        &BTreeMap::from([
            (
                "block_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_proc_profit_x_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_economic_objective".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
        ]),
    );
    let exact_upit_metric_comparison = compare_named_numeric_metrics(
        &reference_upit_metrics,
        &exact_upit_metrics,
        &BTreeMap::from([
            (
                "block_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_proc_profit_x_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_economic_objective".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
        ]),
    );
    let mine_rs_end_to_end = build_mine_rs_end_to_end_artifacts(
        &model,
        &candidate_upit.selected_linear_indices,
        &pcpsp_problem,
    )?;
    let mine_rs_vs_cpit_metric_comparison = compare_named_numeric_metrics(
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                cpit_summary.discounted_objective,
            ),
            (
                "used_period_count".to_owned(),
                cpit_summary.used_period_count as f64,
            ),
            (
                "unique_block_count".to_owned(),
                cpit_summary.unique_block_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                cpit_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 0)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
        ]),
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                mine_rs_end_to_end.report.npv,
            ),
            (
                "used_period_count".to_owned(),
                mine_rs_end_to_end
                    .summary
                    .periods
                    .iter()
                    .filter(|period| period.tonnage > 0.0)
                    .count() as f64,
            ),
            (
                "unique_block_count".to_owned(),
                mine_rs_end_to_end.summary.total_block_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                mine_rs_end_to_end
                    .summary
                    .periods
                    .iter()
                    .map(|period| period.tonnage)
                    .fold(0.0_f64, f64::max),
            ),
        ]),
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "used_period_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "unique_block_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "max_mine_period_usage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
        ]),
    );
    let mine_rs_vs_cpit_membership_comparison = compare_period_memberships(
        &build_reference_period_memberships(&cpit_solution),
        &mine_rs_end_to_end.period_memberships,
    );
    let mine_rs_vs_cpit_period_metric_comparison = compare_named_numeric_metrics(
        &build_reference_period_metric_map(&model, &cpit_problem, &cpit_solution, &tonnage_column)?,
        &build_candidate_period_metric_map(
            &mine_rs_end_to_end.report,
            &mine_rs_end_to_end.period_memberships,
        ),
        &BTreeMap::new(),
    );
    let candidate_pcpsp_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &mine_rs_end_to_end.period_memberships)?;
    let candidate_pcpsp_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &candidate_pcpsp_solution)?;
    let mine_rs_vs_pcpsp_metric_comparison = compare_named_numeric_metrics(
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                pcpsp_summary.discounted_objective,
            ),
            (
                "used_period_count".to_owned(),
                pcpsp_summary.used_period_count as f64,
            ),
            (
                "unique_block_count".to_owned(),
                pcpsp_summary.unique_block_count as f64,
            ),
            (
                "used_destination_count".to_owned(),
                pcpsp_summary.used_destination_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 0)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
            (
                "max_process_period_usage".to_owned(),
                pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 1)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
        ]),
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                candidate_pcpsp_summary.discounted_objective,
            ),
            (
                "used_period_count".to_owned(),
                candidate_pcpsp_summary.used_period_count as f64,
            ),
            (
                "unique_block_count".to_owned(),
                candidate_pcpsp_summary.unique_block_count as f64,
            ),
            (
                "used_destination_count".to_owned(),
                candidate_pcpsp_summary.used_destination_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                candidate_pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 0)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
            (
                "max_process_period_usage".to_owned(),
                candidate_pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 1)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
        ]),
        &BTreeMap::new(),
    );
    let mine_rs_vs_pcpsp_membership_comparison = compare_period_memberships(
        &build_reference_period_destination_memberships(&pcpsp_solution),
        &build_reference_period_destination_memberships(&candidate_pcpsp_solution),
    );
    let mine_rs_vs_pcpsp_period_metric_comparison = compare_named_numeric_metrics(
        &build_reference_period_metric_map(
            &model,
            &pcpsp_problem,
            &pcpsp_solution,
            &tonnage_column,
        )?,
        &build_reference_period_metric_map(
            &model,
            &pcpsp_problem,
            &candidate_pcpsp_solution,
            &tonnage_column,
        )?,
        &BTreeMap::new(),
    );

    let output = MarvinBenchmarkOutput {
        dataset_dir: relative_or_display(&dataset_dir, &repo_root),
        reference_prec_path: relative_or_display(&prec_path, &repo_root),
        reference_upit_solution_path: relative_or_display(&upit_solution_path, &repo_root),
        reference_upit_objective_path: relative_or_display(&upit_objective_path, &repo_root),
        reference_cpit_problem_path: relative_or_display(&cpit_problem_path, &repo_root),
        reference_cpit_solution_path: relative_or_display(&cpit_solution_path, &repo_root),
        reference_pcpsp_problem_path: relative_or_display(&pcpsp_problem_path, &repo_root),
        reference_pcpsp_solution_path: relative_or_display(&pcpsp_solution_path, &repo_root),
        reference_lp_cpit_solution_path: relative_or_display(&lp_cpit_solution_path, &repo_root),
        reference_lp_pcpsp_solution_path: relative_or_display(
            &lp_pcpsp_solution_path,
            &repo_root,
        ),
        value_column: value_column.to_string(),
        tonnage_column: tonnage_column.to_string(),
        candidate_predecessor_offsets: template
            .predecessor_offsets()
            .iter()
            .map(|offset| (offset.di(), offset.dj(), offset.dk()))
            .collect(),
        reference_precedence: PrecedenceArtifactSummary {
            node_count: reference_prec.nodes().len(),
            edge_count: reference_prec.edges().len(),
        },
        candidate_precedence: PrecedenceArtifactSummary {
            node_count: candidate_prec.nodes().len(),
            edge_count: candidate_prec.edges().len(),
        },
        precedence_comparison,
        reference_upit: MembershipArtifactSummary {
            block_count: reference_upit_metrics["block_count"] as usize,
            total_proc_profit_x_tonnage: reference_upit_metrics["total_proc_profit_x_tonnage"],
            total_economic_objective: reference_upit_metrics["total_economic_objective"],
            total_tonnage: reference_upit_metrics["total_tonnage"],
        },
        candidate_upit: MembershipArtifactSummary {
            block_count: candidate_upit_metrics["block_count"] as usize,
            total_proc_profit_x_tonnage: candidate_upit_metrics["total_proc_profit_x_tonnage"],
            total_economic_objective: candidate_upit_metrics["total_economic_objective"],
            total_tonnage: candidate_upit_metrics["total_tonnage"],
        },
        exact_upit: MembershipArtifactSummary {
            block_count: exact_upit_metrics["block_count"] as usize,
            total_proc_profit_x_tonnage: exact_upit_metrics["total_proc_profit_x_tonnage"],
            total_economic_objective: exact_upit_metrics["total_economic_objective"],
            total_tonnage: exact_upit_metrics["total_tonnage"],
        },
        upit_membership_comparison,
        exact_upit_membership_comparison,
        upit_metric_comparison,
        exact_upit_metric_comparison,
        cpit_reference: ScheduleReferenceArtifactSummary {
            period_count: cpit_problem.period_count,
            destination_count: cpit_problem.destination_count,
            resource_constraint_count: cpit_problem.resource_constraint_count,
            discount_rate: cpit_problem.discount_rate,
            official_objective: OFFICIAL_CPIT_OBJECTIVE,
            objective_gap_vs_official: (cpit_summary.discounted_objective - OFFICIAL_CPIT_OBJECTIVE)
                .abs(),
            solution_summary: cpit_summary,
        },
        pcpsp_reference: ScheduleReferenceArtifactSummary {
            period_count: pcpsp_problem.period_count,
            destination_count: pcpsp_problem.destination_count,
            resource_constraint_count: pcpsp_problem.resource_constraint_count,
            discount_rate: pcpsp_problem.discount_rate,
            official_objective: OFFICIAL_PCPSP_OBJECTIVE,
            objective_gap_vs_official: (pcpsp_summary.discounted_objective
                - OFFICIAL_PCPSP_OBJECTIVE)
                .abs(),
            solution_summary: pcpsp_summary,
        },
        lp_cpit_reference: ScheduleReferenceArtifactSummary {
            period_count: cpit_problem.period_count,
            destination_count: cpit_problem.destination_count,
            resource_constraint_count: cpit_problem.resource_constraint_count,
            discount_rate: cpit_problem.discount_rate,
            official_objective: OFFICIAL_LP_CPIT_OBJECTIVE,
            objective_gap_vs_official: (lp_cpit_summary.discounted_objective
                - OFFICIAL_LP_CPIT_OBJECTIVE)
                .abs(),
            solution_summary: lp_cpit_summary,
        },
        lp_pcpsp_reference: ScheduleReferenceArtifactSummary {
            period_count: pcpsp_problem.period_count,
            destination_count: pcpsp_problem.destination_count,
            resource_constraint_count: pcpsp_problem.resource_constraint_count,
            discount_rate: pcpsp_problem.discount_rate,
            official_objective: OFFICIAL_LP_PCPSP_OBJECTIVE,
            objective_gap_vs_official: (lp_pcpsp_summary.discounted_objective
                - OFFICIAL_LP_PCPSP_OBJECTIVE)
                .abs(),
            solution_summary: lp_pcpsp_summary,
        },
        mine_rs_end_to_end: mine_rs_end_to_end.summary,
        mine_rs_vs_cpit_metric_comparison,
        mine_rs_vs_cpit_membership_comparison,
        mine_rs_vs_cpit_period_metric_comparison,
        mine_rs_vs_pcpsp_metric_comparison,
        mine_rs_vs_pcpsp_membership_comparison,
        mine_rs_vs_pcpsp_period_metric_comparison,
        assumptions: vec![
            "marving-info.txt was used to confirm that field_4 is tonnage and field_7 is proc_profit ($/ton), and that mine_cost = 0.9 $/ton.".to_owned(),
            "The candidate precedence template uses the 17-offset Marvin slope pattern (45°/8-niveles): 5 cross at dk=1, 4 diagonal corners at dk=3, 8 near-circle at dk=5.".to_owned(),
            "total_economic_objective = sum((max(proc_profit, 0) - 0.9) × tonnage). Official UPIT target: 1,415,655,436.".to_owned(),
            "The exact UPL comparison uses `marvin.prec` + `marvin.upit` block objective values and solves max-closure directly with the exact backend.".to_owned(),
            "CPIT/PCPSP objective audits apply the MineLib-style discounted objective sum(value × fraction / (1 + discount_rate)^period) over the normalized reference problems and solutions.".to_owned(),
            "The internal mine-rs end-to-end candidate rebuilds Marvin economics from field_4/field_5/field_6 and uses synthetic bench-band phases over the heuristic UPIT membership.".to_owned(),
        ],
        limitations: vec![
            "The internal end-to-end candidate now builds a destination-aware ready-frontier schedule over a normalized Marvin `SchedulingProblem`, but the economic evaluator still aggregates phase cashflow using each block's best destination rather than the routed destination in the candidate schedule.".to_owned(),
            "The heuristic UPIT path is still reported because it remains useful as a cheap baseline; exact parity now comes from the dedicated exact UPL comparison built on `marvin.upit` + `marvin.prec`.".to_owned(),
            "The prec template was reverse-engineered from the reference file; formal proof of completeness against the MineLib algorithm is pending (pending MR-154).".to_owned(),
        ],
    };

    let json = serde_json::to_string_pretty(&output)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, &json)?;
    eprintln!("comparison report written to {}", output_path.display());

    println!("{json}");

    Ok(())
}

fn compact_precedence_comparison(
    report: mine_sdk::PrecedenceGraphComparisonReport,
) -> CompactPrecedenceComparison {
    CompactPrecedenceComparison {
        shared_nodes: report.shared_nodes,
        shared_edges: report.shared_edges,
        reference_only_edge_count: report.reference_only_edges.len(),
        candidate_only_edge_count: report.candidate_only_edges.len(),
        node_jaccard_index: report.node_jaccard_index,
        edge_jaccard_index: report.edge_jaccard_index,
        reference_only_edge_examples: report
            .reference_only_edges
            .into_iter()
            .filter_map(block_edge_tuple)
            .take(10)
            .collect(),
        candidate_only_edge_examples: report
            .candidate_only_edges
            .into_iter()
            .filter_map(block_edge_tuple)
            .take(10)
            .collect(),
    }
}

fn compact_membership_comparison(
    report: mine_sdk::BlockMembershipComparisonReport,
) -> CompactMembershipComparison {
    CompactMembershipComparison {
        shared_blocks: report.shared_blocks,
        reference_only_block_count: report.reference_only_blocks.len(),
        candidate_only_block_count: report.candidate_only_blocks.len(),
        jaccard_index: report.jaccard_index,
        reference_only_block_examples: report.reference_only_blocks.into_iter().take(10).collect(),
        candidate_only_block_examples: report.candidate_only_blocks.into_iter().take(10).collect(),
    }
}

fn block_edge_tuple(edge: mine_sdk::PrecedenceEdge) -> Option<(usize, usize)> {
    match (edge.predecessor(), edge.successor()) {
        (PrecedenceNode::Block(predecessor), PrecedenceNode::Block(successor)) => {
            Some((*predecessor, *successor))
        }
        _ => None,
    }
}

fn relative_or_display(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root).map_or_else(
        |_| path.display().to_string(),
        |relative| relative.display().to_string(),
    )
}

fn marvin_slope_template() -> Result<BlockPrecedenceTemplate, mine_sdk::MineError> {
    BlockPrecedenceTemplate::new(vec![
        // dk=1: patrón cardinal (5 bloques)
        PrecedenceOffset::new(0, 0, 1)?,
        PrecedenceOffset::new(-1, 0, 1)?,
        PrecedenceOffset::new(1, 0, 1)?,
        PrecedenceOffset::new(0, -1, 1)?,
        PrecedenceOffset::new(0, 1, 1)?,
        // dk=3: esquinas diagonales (4 bloques)
        PrecedenceOffset::new(-2, -2, 3)?,
        PrecedenceOffset::new(-2, 2, 3)?,
        PrecedenceOffset::new(2, -2, 3)?,
        PrecedenceOffset::new(2, 2, 3)?,
        // dk=5: arco semicircular (8 bloques)
        PrecedenceOffset::new(-4, -3, 5)?,
        PrecedenceOffset::new(-4, 3, 5)?,
        PrecedenceOffset::new(-3, -4, 5)?,
        PrecedenceOffset::new(-3, 4, 5)?,
        PrecedenceOffset::new(3, -4, 5)?,
        PrecedenceOffset::new(3, 4, 5)?,
        PrecedenceOffset::new(4, -3, 5)?,
        PrecedenceOffset::new(4, 3, 5)?,
    ])
}

fn membership_metrics(
    model: &BlockModel,
    selected_linear_indices: &[usize],
    value_column: &ColumnId,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, f64>, mine_sdk::MineError> {
    const MINE_COST_PER_TON: f64 = 0.9;
    let proc_profit_values = float_column(model, value_column)?;
    let tonnage_values = float_column(model, tonnage_column)?;
    let mut total_proc_profit_x_tonnage = 0.0;
    let mut total_economic_objective = 0.0;
    let mut total_tonnage = 0.0;

    for linear_index in selected_linear_indices {
        let row_index = row_index_for_linear_index(model, *linear_index)?;
        let proc_profit = proc_profit_values[row_index];
        let tonnage = tonnage_values[row_index];
        total_proc_profit_x_tonnage += proc_profit * tonnage;
        total_economic_objective += (proc_profit.max(0.0) - MINE_COST_PER_TON) * tonnage;
        total_tonnage += tonnage;
    }

    Ok(BTreeMap::from([
        (
            "block_count".to_owned(),
            selected_linear_indices.len() as f64,
        ),
        (
            "total_proc_profit_x_tonnage".to_owned(),
            total_proc_profit_x_tonnage,
        ),
        (
            "total_economic_objective".to_owned(),
            total_economic_objective,
        ),
        ("total_tonnage".to_owned(), total_tonnage),
    ]))
}

fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
) -> Result<&'a [f64], mine_sdk::MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(mine_sdk::MineError::schema(format!(
            "column `{column_id}` does not exist in block model storage"
        )));
    };
    let ColumnData::Floats(values) = column_data else {
        return Err(mine_sdk::MineError::schema(format!(
            "column `{column_id}` must be a float column"
        )));
    };
    Ok(values)
}

fn row_index_for_linear_index(
    model: &BlockModel,
    linear_index: usize,
) -> Result<usize, mine_sdk::MineError> {
    for row_index in 0..model.block_count() {
        if model.linear_index_at(row_index)? == linear_index {
            return Ok(row_index);
        }
    }

    Err(mine_sdk::MineError::validation(format!(
        "linear index `{linear_index}` is not materialized in the block model"
    )))
}

struct MineRsEndToEndArtifacts {
    summary: MineRsEndToEndSummary,
    report: LongTermScheduleEconomicsReport,
    period_memberships: BTreeMap<String, BTreeSet<usize>>,
}

fn build_mine_rs_end_to_end_artifacts(
    model: &BlockModel,
    selected_linear_indices: &[usize],
    marvin_problem: &MarvinScheduleProblem,
) -> Result<MineRsEndToEndArtifacts, mine_sdk::MineError> {
    let tonnage_column = ColumnId::new("field_4")?;
    let bench_assignments = assign_benches(model, &BenchParameters::new(1.0, 0.0, 1e-9)?)?;
    let phase_plan = build_phase_plan_from_selected_blocks(
        model,
        &bench_assignments,
        selected_linear_indices,
        &tonnage_column,
    )?;
    let scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(&phase_plan, marvin_problem)?;
    let schedule =
        build_ready_frontier_long_term_schedule(&scheduling_problem, None, Metadata::new())?;
    let economic_model = build_marvin_economic_block_model(model)?;
    let report = evaluate_long_term_schedule_economics(
        &schedule,
        &phase_plan,
        &economic_model,
        marvin_problem.discount_rate,
    )?;
    let period_memberships =
        build_candidate_period_memberships(model, &phase_plan, &schedule, &tonnage_column)?;

    Ok(MineRsEndToEndArtifacts {
        summary: compact_end_to_end_summary(&phase_plan, &schedule, &report),
        report,
        period_memberships,
    })
}

fn build_phase_plan_from_selected_blocks(
    model: &BlockModel,
    bench_assignments: &[mine_sdk::BenchAssignment],
    selected_linear_indices: &[usize],
    tonnage_column: &ColumnId,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let tonnage_values = float_column(model, tonnage_column)?;
    let selected = selected_linear_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let bench_by_linear_index = bench_assignments
        .iter()
        .map(|assignment| (assignment.linear_index, assignment.bench))
        .collect::<BTreeMap<_, _>>();
    let max_bench = selected
        .iter()
        .filter_map(|linear_index| bench_by_linear_index.get(linear_index).copied())
        .max()
        .ok_or_else(|| mine_sdk::MineError::Planning {
            message: "marvin end-to-end benchmark requires at least one selected block".to_owned(),
        })?;
    let mut phase_blocks = BTreeMap::<i64, Vec<usize>>::new();
    let mut phase_tonnage = BTreeMap::<i64, f64>::new();
    let mut phase_max_bench = BTreeMap::<i64, i64>::new();

    for linear_index in selected {
        let bench = *bench_by_linear_index.get(&linear_index).ok_or_else(|| {
            mine_sdk::MineError::Planning {
                message: format!("selected block `{linear_index}` is missing a bench assignment"),
            }
        })?;
        let phase_index = ((max_bench - bench) / PHASE_BENCH_SPAN) + 1;
        let row_index = row_index_for_linear_index(model, linear_index)?;
        phase_blocks
            .entry(phase_index)
            .or_default()
            .push(linear_index);
        *phase_tonnage.entry(phase_index).or_insert(0.0) += tonnage_values[row_index];
        phase_max_bench
            .entry(phase_index)
            .and_modify(|current| *current = (*current).max(bench))
            .or_insert(bench);
    }

    let mut previous_phase_id = None::<String>;
    let phases = phase_blocks
        .into_iter()
        .map(|(phase_index, mut block_indices)| {
            block_indices.sort_unstable();
            let phase_id = format!("phase-{phase_index:02}");
            let predecessor_phase_ids = previous_phase_id.iter().cloned().collect::<Vec<_>>();
            previous_phase_id = Some(phase_id.clone());
            Ok(PhaseDesign {
                phase_id,
                pushback_index: 0,
                shell_index: None,
                revenue_factor: None,
                bench: phase_max_bench.get(&phase_index).copied(),
                block_count: block_indices.len(),
                total_tonnage: phase_tonnage.get(&phase_index).copied(),
                block_indices,
                predecessor_phase_ids,
            })
        })
        .collect::<Result<Vec<_>, mine_sdk::MineError>>()?;

    Ok(PushbackPlan {
        total_block_count: selected_linear_indices.len(),
        total_tonnage: Some(phase_tonnage.values().sum()),
        phase_count: phases.len(),
        phases,
        nesting_rules: mine_sdk::NestingAccessRules::strict_sequential(),
        limitations: vec![
            "Synthetic phase bands of four benches are used for the Marvin end-to-end benchmark."
                .to_owned(),
        ],
    })
}

fn build_phase_scheduling_problem_from_marvin_problem(
    phase_plan: &PushbackPlan,
    marvin_problem: &MarvinScheduleProblem,
) -> Result<SchedulingProblem, mine_sdk::MineError> {
    let phase_by_linear_index = phase_plan
        .phases
        .iter()
        .flat_map(|phase| {
            phase
                .block_indices
                .iter()
                .copied()
                .map(move |linear_index| (linear_index, phase.phase_id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut objective_by_phase_destination = BTreeMap::<(String, usize), f64>::new();
    let mut requirements_by_phase_resource_destination =
        BTreeMap::<(String, usize, usize), f64>::new();

    for term in &marvin_problem.objective_terms {
        let Some(phase_id) = phase_by_linear_index.get(&term.linear_index) else {
            continue;
        };
        *objective_by_phase_destination
            .entry((phase_id.clone(), term.destination_index))
            .or_insert(0.0) += term.objective_value;
    }

    for coefficient in &marvin_problem.resource_coefficients {
        if coefficient.coefficient < -1.0e-9 {
            return Err(mine_sdk::MineError::validation(format!(
                "Marvin resource coefficient for block `{}` resource `{}` destination `{}` must be non-negative to build an aggregated SchedulingProblem",
                coefficient.linear_index, coefficient.resource_index, coefficient.destination_index
            )));
        }
        if coefficient.coefficient <= 1.0e-9 {
            continue;
        }
        let Some(phase_id) = phase_by_linear_index.get(&coefficient.linear_index) else {
            continue;
        };
        *requirements_by_phase_resource_destination
            .entry((
                phase_id.clone(),
                coefficient.resource_index,
                coefficient.destination_index,
            ))
            .or_insert(0.0) += coefficient.coefficient;
    }

    let periods = build_periods_from_marvin_problem(marvin_problem)?;
    let destination_ids = (0..marvin_problem.destination_count)
        .map(marvin_destination_id)
        .collect::<Result<Vec<_>, _>>()?;
    let mut max_limit_by_resource = BTreeMap::<usize, f64>::new();
    for limit in &marvin_problem.resource_constraint_limits {
        if !matches!(limit.relation, 'L' | 'E') || limit.limit <= 1.0e-9 {
            continue;
        }
        max_limit_by_resource
            .entry(limit.resource_index)
            .and_modify(|current| *current = (*current).min(limit.limit))
            .or_insert(limit.limit);
    }

    let mut units = Vec::new();
    let mut objective_terms = Vec::new();
    let mut resource_requirements = Vec::new();
    let mut last_chunk_id_by_phase = BTreeMap::<String, SchedulingUnitId>::new();

    for phase in &phase_plan.phases {
        let total_tonnage = phase
            .total_tonnage
            .ok_or_else(|| mine_sdk::MineError::Planning {
                message: format!(
                    "phase `{}` requires total_tonnage to build a Marvin scheduling problem",
                    phase.phase_id
                ),
            })?;
        let candidate_destination_indices = (0..marvin_problem.destination_count)
            .filter(|destination_index| {
                objective_by_phase_destination
                    .contains_key(&(phase.phase_id.clone(), *destination_index))
                    || requirements_by_phase_resource_destination.keys().any(
                        |(phase_id, _, requirement_destination_index)| {
                            phase_id == &phase.phase_id
                                && requirement_destination_index == destination_index
                        },
                    )
            })
            .collect::<Vec<_>>();
        let candidate_destinations = candidate_destination_indices
            .iter()
            .copied()
            .map(marvin_destination_id)
            .collect::<Result<Vec<_>, _>>()?;
        let mut chunk_count = 1usize;

        if let Some(max_limit) = max_limit_by_resource.get(&0) {
            chunk_count = chunk_count.max((total_tonnage / max_limit).ceil() as usize);
        }
        for ((phase_id, resource_index, _), amount) in &requirements_by_phase_resource_destination {
            if phase_id != &phase.phase_id {
                continue;
            }
            if let Some(max_limit) = max_limit_by_resource.get(resource_index) {
                chunk_count = chunk_count.max((amount / max_limit).ceil() as usize);
            }
        }
        chunk_count = chunk_count.max(1);

        let tonnage_splits = split_f64(total_tonnage, chunk_count);
        let block_splits = split_usize(phase.block_count, chunk_count);
        let mut previous_chunk_id = None::<SchedulingUnitId>;

        for chunk_index in 0..chunk_count {
            let unit_name = if chunk_count == 1 {
                phase.phase_id.clone()
            } else {
                format!("{}::part-{:02}", phase.phase_id, chunk_index + 1)
            };
            let unit_id = SchedulingUnitId::new(unit_name)?;
            let predecessor_unit_ids = if let Some(previous_chunk_id) = &previous_chunk_id {
                vec![previous_chunk_id.clone()]
            } else {
                phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|phase_id| {
                        last_chunk_id_by_phase.get(phase_id).cloned().ok_or_else(|| {
                            mine_sdk::MineError::Planning {
                                message: format!(
                                    "phase `{}` references predecessor `{phase_id}` before it was chunked",
                                    phase.phase_id
                                ),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            let unit_metadata = Metadata::from_entries(vec![(
                "phase_id".to_owned(),
                MetadataValue::Text(phase.phase_id.clone()),
            )])?;

            units.push(SchedulingUnit::new(
                unit_id.clone(),
                tonnage_splits[chunk_index],
                block_splits[chunk_index],
                predecessor_unit_ids,
                candidate_destinations.clone(),
                Vec::new(),
                Vec::new(),
                phase.bench,
                phase.shell_index,
                unit_metadata,
            )?);

            for destination_index in &candidate_destination_indices {
                let phase_objective = objective_by_phase_destination
                    .get(&(phase.phase_id.clone(), *destination_index))
                    .copied()
                    .unwrap_or(0.0);
                if phase_objective.abs() > 1.0e-9 {
                    objective_terms.push(SchedulingObjectiveTerm::new(
                        unit_id.clone(),
                        Some(marvin_destination_id(*destination_index)?),
                        phase_objective / chunk_count as f64,
                    )?);
                }
            }

            for ((phase_id, resource_index, destination_index), amount) in
                &requirements_by_phase_resource_destination
            {
                if phase_id != &phase.phase_id || *amount <= 1.0e-9 {
                    continue;
                }
                resource_requirements.push(SchedulingResourceRequirement::new(
                    unit_id.clone(),
                    marvin_resource_id(*resource_index)?,
                    Some(marvin_destination_id(*destination_index)?),
                    amount / chunk_count as f64,
                )?);
            }

            previous_chunk_id = Some(unit_id.clone());
            last_chunk_id_by_phase.insert(phase.phase_id.clone(), unit_id);
        }
    }

    SchedulingProblem::new(
        ScenarioId::new("marvin-candidate")?,
        ModelId::new("marvin")?,
        periods,
        units,
        objective_terms,
        resource_requirements,
        destination_ids,
        Vec::new(),
        marvin_problem.discount_rate,
        Metadata::new(),
        vec![
            "Synthetic four-bench phases aggregate Marvin blocks before routing, so the scheduling objective is phase-level rather than block-level.".to_owned(),
        ],
    )
}

fn build_periods_from_marvin_problem(
    marvin_problem: &MarvinScheduleProblem,
) -> Result<Vec<SchedulingPeriod>, mine_sdk::MineError> {
    let mut bounds_by_period =
        vec![BTreeMap::<usize, (Option<f64>, Option<f64>)>::new(); marvin_problem.period_count];

    for limit in &marvin_problem.resource_constraint_limits {
        let period_bounds = bounds_by_period
            .get_mut(limit.period_index)
            .ok_or_else(|| {
                mine_sdk::MineError::validation(format!(
                    "Marvin resource limit references period `{}` outside declared range 0..{}",
                    limit.period_index, marvin_problem.period_count
                ))
            })?;
        let bound = period_bounds
            .entry(limit.resource_index)
            .or_insert((None, None));
        match limit.relation {
            'L' => {
                bound.1 = Some(
                    bound
                        .1
                        .map_or(limit.limit, |current| current.min(limit.limit)),
                );
            }
            'G' => {
                bound.0 = Some(
                    bound
                        .0
                        .map_or(limit.limit, |current| current.max(limit.limit)),
                );
            }
            'E' => {
                bound.0 = Some(
                    bound
                        .0
                        .map_or(limit.limit, |current| current.max(limit.limit)),
                );
                bound.1 = Some(
                    bound
                        .1
                        .map_or(limit.limit, |current| current.min(limit.limit)),
                );
            }
            relation => {
                return Err(mine_sdk::MineError::validation(format!(
                    "Marvin resource limit uses unsupported relation `{relation}`"
                )));
            }
        }
    }

    bounds_by_period
        .into_iter()
        .enumerate()
        .map(|(period_index, resource_bounds)| {
            SchedulingPeriod::new(
                format!("P{:02}", period_index + 1),
                resource_bounds
                    .into_iter()
                    .map(|(resource_index, (min_total, max_total))| {
                        SchedulingResourceBound::new(
                            marvin_resource_id(resource_index)?,
                            min_total,
                            max_total,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                Vec::new(),
                Vec::new(),
            )
        })
        .collect()
}

fn marvin_resource_id(resource_index: usize) -> Result<SchedulingResourceId, mine_sdk::MineError> {
    match resource_index {
        0 => SchedulingResourceId::new("mine_tonnage"),
        1 => SchedulingResourceId::new("plant_tonnage"),
        _ => SchedulingResourceId::new(format!("resource-{resource_index:02}")),
    }
}

fn marvin_destination_id(
    destination_index: usize,
) -> Result<ScheduleDestinationId, mine_sdk::MineError> {
    ScheduleDestinationId::new(format!("dest-{destination_index:02}"))
}

fn split_f64(total: f64, parts: usize) -> Vec<f64> {
    if parts <= 1 {
        return vec![total];
    }

    let base = total / parts as f64;
    let mut result = Vec::with_capacity(parts);
    let mut remaining = total;
    for part_index in 0..parts {
        if part_index + 1 == parts {
            result.push(remaining);
            continue;
        }
        result.push(base);
        remaining -= base;
    }
    result
}

fn split_usize(total: usize, parts: usize) -> Vec<usize> {
    if parts <= 1 {
        return vec![total];
    }

    let mut result = Vec::with_capacity(parts);
    let mut assigned = 0usize;
    for part_index in 0..parts {
        if part_index + 1 == parts {
            result.push(total.saturating_sub(assigned));
            continue;
        }
        let next_assigned =
            (((part_index + 1) as f64 / parts as f64) * total as f64).round() as usize;
        let current = next_assigned.saturating_sub(assigned);
        result.push(current);
        assigned += current;
    }
    result
}

fn build_marvin_economic_block_model(
    model: &BlockModel,
) -> Result<EconomicBlockModel, mine_sdk::MineError> {
    let tonnage_column = ColumnId::new("field_4")?;
    let au_column = ColumnId::new("field_5")?;
    let cu_column = ColumnId::new("field_6")?;
    let tonne_unit = MeasurementUnit::new("t")?;
    let process_destination = DestinationAssumptions::new(
        DestinationId::new("process")?,
        DestinationKind::Mill,
        0.9,
        4.0,
        vec![
            DestinationRecovery::new(au_column.clone(), 0.6)?,
            DestinationRecovery::new(cu_column.clone(), 0.88)?,
        ],
        vec![
            DestinationPayability::new(au_column.clone(), (12.0 - 0.2) / 12.0)?,
            DestinationPayability::new(cu_column.clone(), (20.0 - 7.2) / 20.0)?,
        ],
        DestinationCapacity::new(None, tonne_unit.clone())?,
        BTreeMap::from([
            (au_column.as_str().to_owned(), 12.0),
            (cu_column.as_str().to_owned(), 20.0),
        ]),
    )?;
    let waste_destination = DestinationAssumptions::new(
        DestinationId::new("waste")?,
        DestinationKind::Waste,
        0.9,
        0.0,
        Vec::new(),
        Vec::new(),
        DestinationCapacity::new(None, tonne_unit)?,
        BTreeMap::new(),
    )?;

    EconomicBlockModel::build(
        model.clone(),
        EconomicBlockModelConfig {
            tonnage_column,
            grade_columns: vec![au_column, cu_column],
            destinations: DestinationAssumptionSet::new(vec![
                process_destination,
                waste_destination,
            ])?,
        },
    )
}

fn build_reference_period_memberships(
    solution: &marvin_support::MarvinScheduleSolution,
) -> BTreeMap<String, BTreeSet<usize>> {
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();
    for assignment in &solution.assignments {
        if assignment.fraction <= 0.0 {
            continue;
        }
        memberships
            .entry(format!("P{:02}", assignment.period_index + 1))
            .or_default()
            .insert(assignment.linear_index);
    }
    memberships
}

fn build_reference_period_destination_memberships(
    solution: &marvin_support::MarvinScheduleSolution,
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

fn build_reference_period_metric_map(
    model: &BlockModel,
    problem: &MarvinScheduleProblem,
    solution: &marvin_support::MarvinScheduleSolution,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, f64>, mine_sdk::MineError> {
    let objective_lookup = problem
        .objective_terms
        .iter()
        .map(|term| {
            (
                (term.linear_index, term.destination_index),
                term.objective_value,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let tonnage_values = float_column(model, tonnage_column)?;
    let mut tonnage_by_period = BTreeMap::<String, f64>::new();
    let mut discounted_objective_by_period = BTreeMap::<String, f64>::new();
    let mut block_membership_by_period = BTreeMap::<String, BTreeSet<usize>>::new();

    for assignment in &solution.assignments {
        let period_label = format!("P{:02}", assignment.period_index + 1);
        let row_index = row_index_for_linear_index(model, assignment.linear_index)?;
        let tonnage = tonnage_values[row_index] * assignment.fraction;
        let objective_value = objective_lookup
            .get(&(assignment.linear_index, assignment.destination_index))
            .copied()
            .ok_or_else(|| {
                mine_sdk::MineError::validation(format!(
                    "missing Marvin objective term for block {} and destination {}",
                    assignment.linear_index, assignment.destination_index
                ))
            })?;

        *tonnage_by_period.entry(period_label.clone()).or_insert(0.0) += tonnage;
        *discounted_objective_by_period
            .entry(period_label.clone())
            .or_insert(0.0) += objective_value * assignment.fraction
            / (1.0 + problem.discount_rate).powi(assignment.period_index as i32);
        block_membership_by_period
            .entry(period_label)
            .or_default()
            .insert(assignment.linear_index);
    }

    let mut metrics = BTreeMap::new();
    for period_index in 0..problem.period_count {
        let period_label = format!("P{:02}", period_index + 1);
        metrics.insert(
            format!("{period_label}.tonnage"),
            tonnage_by_period.get(&period_label).copied().unwrap_or(0.0),
        );
        metrics.insert(
            format!("{period_label}.discounted_objective"),
            discounted_objective_by_period
                .get(&period_label)
                .copied()
                .unwrap_or(0.0),
        );
        metrics.insert(
            format!("{period_label}.block_count"),
            block_membership_by_period
                .get(&period_label)
                .map_or(0usize, BTreeSet::len) as f64,
        );
    }
    Ok(metrics)
}

fn build_candidate_pcpsp_solution(
    problem: &MarvinScheduleProblem,
    period_memberships: &BTreeMap<String, BTreeSet<usize>>,
) -> Result<MarvinScheduleSolution, mine_sdk::MineError> {
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
                            "missing Marvin objective terms for candidate block `{linear_index}`"
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

            assignments.push(MarvinScheduleAssignment {
                linear_index,
                destination_index: selected_destination,
                period_index,
                fraction: 1.0,
            });
        }
    }

    Ok(MarvinScheduleSolution {
        kind: problem.kind,
        unique_block_count: assignments
            .iter()
            .map(|assignment| assignment.linear_index)
            .collect::<BTreeSet<_>>()
            .len(),
        assignments,
    })
}

fn build_candidate_period_metric_map(
    report: &LongTermScheduleEconomicsReport,
    period_memberships: &BTreeMap<String, BTreeSet<usize>>,
) -> BTreeMap<String, f64> {
    let mut metrics = BTreeMap::new();
    for period in &report.periods {
        metrics.insert(format!("{}.tonnage", period.period_label), period.tonnage);
        metrics.insert(
            format!("{}.discounted_objective", period.period_label),
            period.discounted_cashflow,
        );
        metrics.insert(
            format!("{}.block_count", period.period_label),
            period_memberships
                .get(&period.period_label)
                .map_or(0usize, BTreeSet::len) as f64,
        );
    }
    metrics
}

fn build_candidate_period_memberships(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    schedule: &mine_sdk::LongTermSchedule,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, BTreeSet<usize>>, mine_sdk::MineError> {
    let tonnage_values = float_column(model, tonnage_column)?;
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();

    for phase in &phase_plan.phases {
        let mut entries = schedule
            .entries()
            .iter()
            .filter(|entry| entry.phase_id() == Some(phase.phase_id.as_str()))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            continue;
        }
        entries.sort_by_key(|entry| entry.period_label().to_owned());

        let mut entry_index = 0usize;
        let mut remaining_tonnage = entries[entry_index].tonnage();
        let mut blocks = phase.block_indices.clone();
        blocks.sort_unstable();

        for linear_index in blocks {
            while remaining_tonnage <= 1e-9 && entry_index + 1 < entries.len() {
                entry_index += 1;
                remaining_tonnage = entries[entry_index].tonnage();
            }

            let row_index = row_index_for_linear_index(model, linear_index)?;
            let block_tonnage = tonnage_values[row_index];
            memberships
                .entry(entries[entry_index].period_label().to_owned())
                .or_default()
                .insert(linear_index);
            remaining_tonnage -= block_tonnage;
        }
    }

    Ok(memberships)
}

fn compact_end_to_end_summary(
    phase_plan: &PushbackPlan,
    schedule: &mine_sdk::LongTermSchedule,
    report: &LongTermScheduleEconomicsReport,
) -> MineRsEndToEndSummary {
    MineRsEndToEndSummary {
        phase_count: phase_plan.phase_count,
        total_block_count: phase_plan.total_block_count,
        schedule_period_count: report.periods.len(),
        schedule_entry_count: schedule.entries().len(),
        schedule_violation_count: schedule.violations().len(),
        total_tonnage: report.total_tonnage,
        total_cashflow: report.total_cashflow,
        npv: report.npv,
        periods: report
            .periods
            .iter()
            .map(|period| MineRsPeriodSummary {
                period_label: period.period_label.clone(),
                tonnage: period.tonnage,
                cashflow: period.cashflow,
                discounted_cashflow: period.discounted_cashflow,
            })
            .collect(),
    }
}
