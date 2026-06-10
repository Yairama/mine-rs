//! Candidato CPIT propio vía heurística TopoSort guiada por LP (MR-211).
//!
//! Construye un schedule CPIT factible usando `solve_cpit_with_toposort` del
//! core (`mine-planning`), con scores de orden derivados del tiempo esperado de
//! extracción de la relajación LP abierta (`*.LPcpit`) staged por MineLib.
//!
//! Método de referencia: Chicoisne, Espinoza, Goycoolea, Moreno, Rubio (2012),
//! Operations Research 60(3):517-528, doi 10.1287/opre.1120.1072 ([R35]), que
//! reporta gaps <= ~5% contra el bound LP en instancias MineLib. Nota de
//! protocolo: este harness todavía consume la relajación LP publicada por
//! MineLib como insumo de ordering (la misma información que [R35] computa
//! internamente); el bound LP propio queda para MR-213.
//!
//! Uso:
//!   cargo run --release -p marvin-benchmark --bin cpit_toposort -- [--include-full] [--quiet] [output_path]
//!
//! Por defecto corre `marvin` y `mclaughlin-limit`. Si no se especifica
//! `output_path`, escribe `datasets/benchmarks/outputs/cpit-toposort-report.json`.
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

use std::fs;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use benchmark_cli_support::parse_benchmark_cli_args;
use benchmark_path_policy::BenchmarkPathPolicy;
use benchmark_runtime_telemetry::{RuntimeTelemetry, StageTimer};
use cpit_toposort_support::{
    build_expected_period_scores, build_toposort_problem_from_minelib_cpit,
    verify_schedule_precedence,
};
use marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleProblemKind,
    MarvinScheduleSolution, read_minelib_cpit_problem, read_minelib_lp_cpit_solution,
    read_minelib_precedence_graph, summarize_minelib_schedule_solution,
};
use mine_sdk::{
    CpitToposortOptions, CpitToposortSchedule, MineError, PrecedenceGraph, solve_cpit_with_toposort,
};
use serde::Serialize;

const REPORT_TICKETS: &[&str] = &["MR-211", "MR-215"];
const DEFAULT_OUTPUT_RELATIVE_PATH: &str = "datasets/benchmarks/outputs/cpit-toposort-report.json";
const AUDIT_OBJECTIVE_RELATIVE_TOLERANCE: f64 = 1.0e-9;

const LITERATURE_CONTEXT: &[&str] = &[
    "TopoSort reference method: Chicoisne et al. (2012), Operations Research 60(3):517-528, doi 10.1287/opre.1120.1072 ([R35]); reported integer solutions within ~5% of the LP bound on MineLib-scale instances.",
    "Official CPIT best-known objectives and LP relaxation values come from the staged MineLib info files ([R29] Espinoza et al., doi 10.1007/s10479-012-1258-3); later literature ([R33] Jelvez et al., [R37] Rivera Letelier et al.) has improved several best-knowns, so beating the staged 2012 incumbent does not claim a world best-known.",
    "Protocol note: the ordering scores reuse the published MineLib LP relaxation (`*.LPcpit`) as input; a self-computed LP/BZ bound is the goal of MR-213, after which this candidate becomes fully self-contained.",
];

struct DatasetConfig {
    dataset_id: &'static str,
    blocks_file: &'static str,
    precedence_file: &'static str,
    cpit_problem_file: &'static str,
    lp_cpit_solution_file: &'static str,
    official_cpit_objective: f64,
    official_lp_cpit_objective: f64,
    official_source: &'static str,
    heavy: bool,
}

const DATASETS: &[DatasetConfig] = &[
    DatasetConfig {
        dataset_id: "marvin",
        blocks_file: "marvin.blocks",
        precedence_file: "marvin.prec",
        cpit_problem_file: "marvin.cpit",
        lp_cpit_solution_file: "marvin.LPcpit",
        official_cpit_objective: 820_726_048.0,
        official_lp_cpit_objective: 863_916_131.0,
        official_source: "datasets/benchmarks/marvin/marving-info.txt",
        heavy: false,
    },
    DatasetConfig {
        dataset_id: "mclaughlin-limit",
        blocks_file: "mclaughlin_limit.blocks",
        precedence_file: "mclaughlin_limit.prec",
        cpit_problem_file: "mclaughlin_limit.cpit",
        lp_cpit_solution_file: "mclaughlin_limit.LPcpit",
        official_cpit_objective: 1_073_327_197.0,
        official_lp_cpit_objective: 1_078_979_501.0,
        official_source: "datasets/benchmarks/mclaughlin-limit/mclaughlin-limit-info.txt",
        heavy: false,
    },
    DatasetConfig {
        dataset_id: "mclaughlin",
        blocks_file: "mclaughlin.blocks",
        precedence_file: "mclaughlin.prec",
        cpit_problem_file: "mclaughlin.cpit",
        lp_cpit_solution_file: "mclaughlin.LPcpit",
        official_cpit_objective: 1_073_327_197.0,
        official_lp_cpit_objective: 1_079_024_268.0,
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
struct DatasetCpitToposortRecord {
    dataset_id: String,
    method: String,
    ordering_score_source: String,
    period_count: usize,
    resource_count: usize,
    lp_support_block_count: usize,
    unenforced_resource_relations: Vec<String>,
    official_cpit_objective: f64,
    official_lp_cpit_objective: f64,
    official_source: String,
    candidates: Vec<CandidateRecord>,
    runtime_telemetry: RuntimeTelemetry,
}

#[derive(Debug, Serialize)]
struct SkippedDatasetRecord {
    dataset_id: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct CpitToposortReport {
    generated_by: String,
    tickets: Vec<String>,
    literature_context: Vec<String>,
    datasets: Vec<DatasetCpitToposortRecord>,
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
            println!("[cpit-toposort] running dataset `{}`...", config.dataset_id);
        }
        let record = run_dataset(&path_policy, config)?;
        if !options.quiet {
            for candidate in &record.candidates {
                println!(
                    "[cpit-toposort] `{}` [{}]: discounted={:.3} (official {:.0}, gap {:.3}%) periods={} scheduled={}",
                    record.dataset_id,
                    candidate.variant,
                    candidate.discounted_objective,
                    record.official_cpit_objective,
                    candidate.gap_vs_official_relative * 100.0,
                    candidate.used_period_count,
                    candidate.scheduled_block_count,
                );
            }
        }
        dataset_records.push(record);
    }

    let report = CpitToposortReport {
        generated_by: "cargo run --release -p marvin-benchmark --bin cpit_toposort".to_owned(),
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
            "[cpit-toposort] report written to {}",
            output_path.display()
        );
    }
    Ok(())
}

fn run_dataset(
    path_policy: &BenchmarkPathPolicy,
    config: &DatasetConfig,
) -> Result<DatasetCpitToposortRecord, Box<dyn std::error::Error>> {
    let dataset_dir = path_policy.dataset_dir(config.dataset_id);
    let references_dir = path_policy.references_dir(&dataset_dir);

    let mut timer = StageTimer::start();

    let model = read_benchmark_blocks(dataset_dir.join(config.blocks_file), config.dataset_id)?;
    timer.record_stage("read-blocks");

    let cpit_problem =
        read_minelib_cpit_problem(references_dir.join(config.cpit_problem_file), &model)?;
    timer.record_stage("read-cpit-problem");

    let precedence_graph =
        read_minelib_precedence_graph(references_dir.join(config.precedence_file), &model)?;
    timer.record_stage("read-precedence");

    let lp_solution =
        read_minelib_lp_cpit_solution(references_dir.join(config.lp_cpit_solution_file), &model)?;
    timer.record_stage("read-lp-cpit-relaxation");

    let ordering_scores = build_expected_period_scores(&lp_solution.assignments);
    timer.record_stage("build-ordering-scores");

    let (toposort_problem, unenforced_relations) =
        build_toposort_problem_from_minelib_cpit(&cpit_problem)?;
    timer.record_stage("build-toposort-problem");

    let baseline_schedule = solve_cpit_with_toposort(
        &toposort_problem,
        &precedence_graph,
        &ordering_scores,
        &CpitToposortOptions {
            delay_negative_blocks: false,
        },
    )?;
    timer.record_stage("solve-toposort-baseline");

    let delayed_schedule = solve_cpit_with_toposort(
        &toposort_problem,
        &precedence_graph,
        &ordering_scores,
        &CpitToposortOptions {
            delay_negative_blocks: true,
        },
    )?;
    timer.record_stage("solve-toposort-delayed-waste");

    let baseline_candidate = audit_candidate(
        config,
        &cpit_problem,
        &precedence_graph,
        &baseline_schedule,
        "toposort-baseline",
    )?;
    let delayed_candidate = audit_candidate(
        config,
        &cpit_problem,
        &precedence_graph,
        &delayed_schedule,
        "toposort-delayed-waste",
    )?;
    timer.record_stage("audit-candidates");

    Ok(DatasetCpitToposortRecord {
        dataset_id: config.dataset_id.to_owned(),
        method: "core mine-planning::solve_cpit_with_toposort (Chicoisne et al. 2012, [R35])"
            .to_owned(),
        ordering_score_source: format!(
            "expected extraction period from staged MineLib LP relaxation `{}`",
            config.lp_cpit_solution_file
        ),
        period_count: cpit_problem.period_count,
        resource_count: cpit_problem.resource_constraint_count,
        lp_support_block_count: ordering_scores.len(),
        unenforced_resource_relations: unenforced_relations,
        official_cpit_objective: config.official_cpit_objective,
        official_lp_cpit_objective: config.official_lp_cpit_objective,
        official_source: config.official_source.to_owned(),
        candidates: vec![baseline_candidate, delayed_candidate],
        runtime_telemetry: timer.finish(),
    })
}

/// Audita el schedule con el mismo auditor usado para soluciones MineLib.
fn audit_candidate(
    config: &DatasetConfig,
    cpit_problem: &MarvinScheduleProblem,
    precedence_graph: &PrecedenceGraph,
    schedule: &CpitToposortSchedule,
    variant: &str,
) -> Result<CandidateRecord, MineError> {
    let precedence_edges_verified = verify_schedule_precedence(schedule, precedence_graph)?;
    let solution = MarvinScheduleSolution {
        kind: MarvinScheduleProblemKind::Cpit,
        assignments: schedule
            .assignments
            .iter()
            .map(|assignment| MarvinScheduleAssignment {
                linear_index: assignment.linear_index,
                destination_index: 0,
                period_index: assignment.period_index,
                fraction: 1.0,
            })
            .collect(),
        unique_block_count: schedule.scheduled_block_count,
    };
    let summary = summarize_minelib_schedule_solution(cpit_problem, &solution)?;

    let audit_consistent = (summary.discounted_objective - schedule.discounted_objective).abs()
        <= AUDIT_OBJECTIVE_RELATIVE_TOLERANCE * schedule.discounted_objective.abs().max(1.0);
    let max_resource_excess = summary
        .resource_summaries
        .iter()
        .map(|resource| resource.max_period_excess)
        .fold(0.0_f64, f64::max);

    let gap_absolute = config.official_cpit_objective - schedule.discounted_objective;
    Ok(CandidateRecord {
        variant: variant.to_owned(),
        scheduled_block_count: schedule.scheduled_block_count,
        dropped_for_capacity_count: schedule.dropped_for_capacity_count,
        dropped_for_predecessor_count: schedule.dropped_for_predecessor_count,
        delayed_negative_block_count: schedule.delayed_negative_block_count,
        used_period_count: schedule.used_period_count,
        discounted_objective: schedule.discounted_objective,
        undiscounted_objective: schedule.undiscounted_objective,
        gap_vs_official_absolute: gap_absolute,
        gap_vs_official_relative: gap_absolute / config.official_cpit_objective,
        gap_vs_lp_relaxation_relative: (config.official_lp_cpit_objective
            - schedule.discounted_objective)
            / config.official_lp_cpit_objective,
        audited_discounted_objective: summary.discounted_objective,
        audit_objective_consistent: audit_consistent,
        audited_max_resource_excess: max_resource_excess,
        precedence_edges_verified,
        precedence_feasibility_verified: true,
    })
}
