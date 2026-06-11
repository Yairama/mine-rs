//! Candidato PCPSP propio vía heurística TopoSort multi-destino (MR-212, hito 1).
//!
//! Construye un schedule PCPSP factible usando `solve_pcpsp_with_toposort` del
//! core (`mine-planning`), con scores de orden derivados del tiempo esperado de
//! extracción de la relajación LP staged por MineLib y decisión de destino
//! durante la construcción (valor descontado máximo entre destinos factibles).
//!
//! Método de referencia: Chicoisne et al. (2012), doi 10.1287/opre.1120.1072
//! ([R35]). Notas de protocolo: (1) el ordering reusa la relajación LP staged
//! (`marvin.LPpcpsp`; en McLaughlin se usa `*.LPcpit` como proxy documentado
//! porque el artefacto LPpcpsp no está staged, ver MR-214); (2) el bound LP
//! propio queda para MR-213.
//!
//! Uso:
//!   cargo run --release -p marvin-benchmark --bin pcpsp_toposort -- [--include-full] [--quiet] [output_path]
//!
//! Por defecto corre `marvin` y `mclaughlin-limit`. Si no se especifica
//! `output_path`, escribe `datasets/benchmarks/outputs/pcpsp-toposort-report.json`.
//! Las rutas relativas se rebasan contra la raíz del repo (política MR-202).

#[path = "../benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
#[path = "../benchmark_cli_support.rs"]
mod benchmark_cli_support;
#[path = "../benchmark_path_policy.rs"]
mod benchmark_path_policy;
#[path = "../benchmark_runtime_telemetry.rs"]
mod benchmark_runtime_telemetry;
#[path = "../cpit_toposort_support.rs"]
mod cpit_toposort_support;
#[path = "../marvin_support.rs"]
mod marvin_support;
#[path = "../pcpsp_toposort_support.rs"]
mod pcpsp_toposort_support;

use std::fs;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use benchmark_cli_support::parse_benchmark_cli_args;
use benchmark_path_policy::BenchmarkPathPolicy;
use benchmark_runtime_telemetry::{RuntimeTelemetry, StageTimer};
use cpit_toposort_support::build_expected_period_scores;
use marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleProblemKind,
    MarvinScheduleSolution, read_minelib_lp_cpit_solution, read_minelib_lp_pcpsp_solution,
    read_minelib_pcpsp_problem, read_minelib_pcpsp_solution, read_minelib_precedence_graph,
    summarize_minelib_schedule_solution,
};
use mine_sdk::{
    MineError, PcpspToposortOptions, PcpspToposortSchedule, PrecedenceGraph,
    solve_pcpsp_with_toposort,
};
use pcpsp_toposort_support::{
    TemporalAlignmentSummary, build_pcpsp_toposort_problem_from_minelib,
    summarize_temporal_alignment, verify_pcpsp_schedule_precedence,
};
use serde::Serialize;

const REPORT_TICKETS: &[&str] = &["MR-212", "MR-215"];
const DEFAULT_OUTPUT_RELATIVE_PATH: &str = "datasets/benchmarks/outputs/pcpsp-toposort-report.json";
const AUDIT_OBJECTIVE_RELATIVE_TOLERANCE: f64 = 1.0e-9;

const LITERATURE_CONTEXT: &[&str] = &[
    "TopoSort reference method: Chicoisne et al. (2012), Operations Research 60(3):517-528, doi 10.1287/opre.1120.1072 ([R35]); destination choice is taken greedily during construction by maximum discounted value among feasible (destination, period) pairs.",
    "Official PCPSP best-known objectives and LP relaxation values come from the staged MineLib info files ([R29] Espinoza et al., doi 10.1007/s10479-012-1258-3); later literature ([R33], [R37]) improved several best-knowns, so beating staged incumbents does not claim a world best-known.",
    "Protocol notes: ordering scores reuse the staged MineLib LP relaxation (`marvin.LPpcpsp`; for both McLaughlin variants the `*.LPcpit` relaxation is used as a documented proxy because no LPpcpsp artifact is staged, see MR-214); a self-computed LP/BZ bound is the goal of MR-213.",
    "MR-212 quantitative milestones tracked by this report: (1) beat the `cpit-period-routed` baseline (marvin 820,726,047.95); (2) reach <=10% gap vs the official PCPSP objective; the full campaign (BZ bound + K-step rounding + repair + budgeted local search, [R37]) remains open.",
];

#[derive(Debug, Clone, Copy)]
enum OrderingSource {
    LpPcpsp,
    LpCpit,
}

struct DatasetConfig {
    dataset_id: &'static str,
    blocks_file: &'static str,
    precedence_file: &'static str,
    pcpsp_problem_file: &'static str,
    pcpsp_solution_file: &'static str,
    ordering_solution_file: &'static str,
    ordering_source: OrderingSource,
    ordering_note: &'static str,
    official_pcpsp_objective: f64,
    official_lp_pcpsp_objective: f64,
    official_source: &'static str,
    heavy: bool,
}

const DATASETS: &[DatasetConfig] = &[
    DatasetConfig {
        dataset_id: "marvin",
        blocks_file: "marvin.blocks",
        precedence_file: "marvin.prec",
        pcpsp_problem_file: "marvin.pcpsp",
        pcpsp_solution_file: "marvin_pcpsp_gmunoz120723.sol",
        ordering_solution_file: "marvin.LPpcpsp",
        ordering_source: OrderingSource::LpPcpsp,
        ordering_note: "expected extraction period from the staged PCPSP LP relaxation",
        official_pcpsp_objective: 885_968_070.0,
        official_lp_pcpsp_objective: 911_704_665.0,
        official_source: "datasets/benchmarks/marvin/marving-info.txt",
        heavy: false,
    },
    DatasetConfig {
        dataset_id: "mclaughlin-limit",
        blocks_file: "mclaughlin_limit.blocks",
        precedence_file: "mclaughlin_limit.prec",
        pcpsp_problem_file: "mclaughlin_limit.pcpsp",
        pcpsp_solution_file: "mclaughlin_limit_pcpsp_gmunoz120723.sol",
        ordering_solution_file: "mclaughlin_limit.LPcpit",
        ordering_source: OrderingSource::LpCpit,
        ordering_note: "expected extraction period from the staged CPIT LP relaxation, used as a \
                        documented proxy because no LPpcpsp artifact is staged (MR-214)",
        official_pcpsp_objective: 1_321_662_551.0,
        official_lp_pcpsp_objective: 1_324_829_727.0,
        official_source: "datasets/benchmarks/mclaughlin-limit/mclaughlin-limit-info.txt",
        heavy: false,
    },
    DatasetConfig {
        dataset_id: "mclaughlin",
        blocks_file: "mclaughlin.blocks",
        precedence_file: "mclaughlin.prec",
        pcpsp_problem_file: "mclaughlin.pcpsp",
        pcpsp_solution_file: "mclaughlin_pcpsp_gmunoz120723.sol",
        ordering_solution_file: "mclaughlin.LPcpit",
        ordering_source: OrderingSource::LpCpit,
        ordering_note: "expected extraction period from the staged CPIT LP relaxation, used as a \
                        documented proxy because no LPpcpsp artifact is staged (MR-214)",
        official_pcpsp_objective: 1_510_126_435.0,
        official_lp_pcpsp_objective: 1_512_971_680.0,
        official_source: "datasets/benchmarks/mclaughlin/mclaughlin-info.txt",
        heavy: true,
    },
];

#[derive(Debug, Serialize)]
struct CandidateRecord {
    variant: String,
    scheduled_block_count: usize,
    dropped_for_capacity_count: usize,
    dropped_for_predecessor_count: usize,
    delayed_negative_block_count: usize,
    used_period_count: usize,
    used_destination_count: usize,
    discounted_objective: f64,
    undiscounted_objective: f64,
    gap_vs_official_absolute: f64,
    gap_vs_official_relative: f64,
    gap_vs_lp_relaxation_relative: f64,
    audited_discounted_objective: f64,
    audit_objective_consistent: bool,
    audited_max_resource_excess: f64,
    precedence_edges_verified: usize,
    precedence_feasibility_verified: bool,
}

#[derive(Debug, Serialize)]
struct DatasetPcpspToposortRecord {
    dataset_id: String,
    method: String,
    ordering_score_source: String,
    period_count: usize,
    destination_count: usize,
    resource_count: usize,
    lp_support_block_count: usize,
    unenforced_resource_relations: Vec<String>,
    official_pcpsp_objective: f64,
    official_lp_pcpsp_objective: f64,
    official_source: String,
    candidates: Vec<CandidateRecord>,
    /// Alineación temporal/ruteo del candidato `toposort-delayed-waste`
    /// contra la solución de referencia PCPSP (insumo del gate MR-206).
    delayed_candidate_temporal_alignment: TemporalAlignmentSummary,
    runtime_telemetry: RuntimeTelemetry,
}

#[derive(Debug, Serialize)]
struct SkippedDatasetRecord {
    dataset_id: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct PcpspToposortReport {
    generated_by: String,
    tickets: Vec<String>,
    literature_context: Vec<String>,
    datasets: Vec<DatasetPcpspToposortRecord>,
    skipped_datasets: Vec<SkippedDatasetRecord>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = parse_benchmark_cli_args(&args).map_err(std::io::Error::other)?;
    let path_policy = BenchmarkPathPolicy::discover()?;

    let mut dataset_records = Vec::new();
    let mut skipped_datasets = Vec::new();

    for config in DATASETS {
        if config.heavy && !options.include_full {
            skipped_datasets.push(SkippedDatasetRecord {
                dataset_id: config.dataset_id.to_owned(),
                reason: "heavy full-scale instance; rerun with --include-full to measure it"
                    .to_owned(),
            });
            continue;
        }

        if !options.quiet {
            println!(
                "[pcpsp-toposort] running dataset `{}`...",
                config.dataset_id
            );
        }
        let record = run_dataset(&path_policy, config)?;
        if !options.quiet {
            for candidate in &record.candidates {
                println!(
                    "[pcpsp-toposort] `{}` [{}]: discounted={:.3} (official {:.0}, gap {:.3}%) periods={} dests={} scheduled={}",
                    record.dataset_id,
                    candidate.variant,
                    candidate.discounted_objective,
                    record.official_pcpsp_objective,
                    candidate.gap_vs_official_relative * 100.0,
                    candidate.used_period_count,
                    candidate.used_destination_count,
                    candidate.scheduled_block_count,
                );
            }
        }
        dataset_records.push(record);
    }

    let report = PcpspToposortReport {
        generated_by: "cargo run --release -p marvin-benchmark --bin pcpsp_toposort".to_owned(),
        tickets: REPORT_TICKETS.iter().map(ToString::to_string).collect(),
        literature_context: LITERATURE_CONTEXT.iter().map(ToString::to_string).collect(),
        datasets: dataset_records,
        skipped_datasets,
    };

    let output_path = options.output_path.map_or_else(
        || path_policy.resolve_cli_path(&PathBuf::from(DEFAULT_OUTPUT_RELATIVE_PATH)),
        |path| path_policy.resolve_cli_path(&path),
    );
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
    if !options.quiet {
        println!(
            "[pcpsp-toposort] report written to {}",
            output_path.display()
        );
    }
    Ok(())
}

fn run_dataset(
    path_policy: &BenchmarkPathPolicy,
    config: &DatasetConfig,
) -> Result<DatasetPcpspToposortRecord, Box<dyn std::error::Error>> {
    let dataset_dir = path_policy.dataset_dir(config.dataset_id);
    let references_dir = path_policy.references_dir(&dataset_dir);

    let mut timer = StageTimer::start();

    let model = read_benchmark_blocks(dataset_dir.join(config.blocks_file), config.dataset_id)?;
    timer.record_stage("read-blocks");

    let pcpsp_problem =
        read_minelib_pcpsp_problem(references_dir.join(config.pcpsp_problem_file), &model)?;
    timer.record_stage("read-pcpsp-problem");

    let precedence_graph =
        read_minelib_precedence_graph(references_dir.join(config.precedence_file), &model)?;
    timer.record_stage("read-precedence");

    let ordering_path = references_dir.join(config.ordering_solution_file);
    let lp_solution = match config.ordering_source {
        OrderingSource::LpPcpsp => read_minelib_lp_pcpsp_solution(&ordering_path, &model)?,
        OrderingSource::LpCpit => read_minelib_lp_cpit_solution(&ordering_path, &model)?,
    };
    timer.record_stage("read-lp-relaxation");

    let ordering_scores = build_expected_period_scores(&lp_solution.assignments);
    timer.record_stage("build-ordering-scores");

    let (toposort_problem, unenforced_relations) =
        build_pcpsp_toposort_problem_from_minelib(&pcpsp_problem)?;
    timer.record_stage("build-toposort-problem");

    let baseline_schedule = solve_pcpsp_with_toposort(
        &toposort_problem,
        &precedence_graph,
        &ordering_scores,
        &PcpspToposortOptions {
            delay_negative_blocks: false,
        },
    )?;
    timer.record_stage("solve-toposort-baseline");

    let delayed_schedule = solve_pcpsp_with_toposort(
        &toposort_problem,
        &precedence_graph,
        &ordering_scores,
        &PcpspToposortOptions {
            delay_negative_blocks: true,
        },
    )?;
    timer.record_stage("solve-toposort-delayed-waste");

    let baseline_candidate = audit_candidate(
        config,
        &pcpsp_problem,
        &precedence_graph,
        &baseline_schedule,
        "toposort-baseline",
    )?;
    let delayed_candidate = audit_candidate(
        config,
        &pcpsp_problem,
        &precedence_graph,
        &delayed_schedule,
        "toposort-delayed-waste",
    )?;
    timer.record_stage("audit-candidates");

    let reference_solution =
        read_minelib_pcpsp_solution(references_dir.join(config.pcpsp_solution_file), &model)?;
    let delayed_candidate_temporal_alignment =
        summarize_temporal_alignment(&delayed_schedule, &reference_solution.assignments);
    timer.record_stage("temporal-alignment-vs-reference");

    Ok(DatasetPcpspToposortRecord {
        dataset_id: config.dataset_id.to_owned(),
        method:
            "core mine-planning::solve_pcpsp_with_toposort (Chicoisne et al. 2012, [R35]; greedy \
             destination by max discounted value among feasible pairs)"
                .to_owned(),
        ordering_score_source: format!(
            "{} (`{}`)",
            config.ordering_note, config.ordering_solution_file
        ),
        period_count: pcpsp_problem.period_count,
        destination_count: pcpsp_problem.destination_count,
        resource_count: pcpsp_problem.resource_constraint_count,
        lp_support_block_count: ordering_scores.len(),
        unenforced_resource_relations: unenforced_relations,
        official_pcpsp_objective: config.official_pcpsp_objective,
        official_lp_pcpsp_objective: config.official_lp_pcpsp_objective,
        official_source: config.official_source.to_owned(),
        candidates: vec![baseline_candidate, delayed_candidate],
        delayed_candidate_temporal_alignment,
        runtime_telemetry: timer.finish(),
    })
}

/// Audita el schedule con el mismo auditor usado para soluciones MineLib.
fn audit_candidate(
    config: &DatasetConfig,
    pcpsp_problem: &MarvinScheduleProblem,
    precedence_graph: &PrecedenceGraph,
    schedule: &PcpspToposortSchedule,
    variant: &str,
) -> Result<CandidateRecord, MineError> {
    let precedence_edges_verified = verify_pcpsp_schedule_precedence(schedule, precedence_graph)?;
    let solution = MarvinScheduleSolution {
        kind: MarvinScheduleProblemKind::Pcpsp,
        assignments: schedule
            .assignments
            .iter()
            .map(|assignment| MarvinScheduleAssignment {
                linear_index: assignment.linear_index,
                destination_index: assignment.destination_index,
                period_index: assignment.period_index,
                fraction: 1.0,
            })
            .collect(),
        unique_block_count: schedule.scheduled_block_count,
    };
    let summary = summarize_minelib_schedule_solution(pcpsp_problem, &solution)?;

    let audit_consistent = (summary.discounted_objective - schedule.discounted_objective).abs()
        <= AUDIT_OBJECTIVE_RELATIVE_TOLERANCE * schedule.discounted_objective.abs().max(1.0);
    let max_resource_excess = summary
        .resource_summaries
        .iter()
        .map(|resource| resource.max_period_excess)
        .fold(0.0_f64, f64::max);

    let gap_absolute = config.official_pcpsp_objective - schedule.discounted_objective;
    Ok(CandidateRecord {
        variant: variant.to_owned(),
        scheduled_block_count: schedule.scheduled_block_count,
        dropped_for_capacity_count: schedule.dropped_for_capacity_count,
        dropped_for_predecessor_count: schedule.dropped_for_predecessor_count,
        delayed_negative_block_count: schedule.delayed_negative_block_count,
        used_period_count: schedule.used_period_count,
        used_destination_count: schedule.used_destination_count,
        discounted_objective: schedule.discounted_objective,
        undiscounted_objective: schedule.undiscounted_objective,
        gap_vs_official_absolute: gap_absolute,
        gap_vs_official_relative: gap_absolute / config.official_pcpsp_objective,
        gap_vs_lp_relaxation_relative: (config.official_lp_pcpsp_objective
            - schedule.discounted_objective)
            / config.official_lp_pcpsp_objective,
        audited_discounted_objective: summary.discounted_objective,
        audit_objective_consistent: audit_consistent,
        audited_max_resource_excess: max_resource_excess,
        precedence_edges_verified,
        precedence_feasibility_verified: true,
    })
}
