//! Bound superior Lagrangiano self-contained para CPIT/PCPSP (MR-213) más
//! candidato TopoSort ordenado por la propia relajación (MR-212).
//!
//! A diferencia del sidecar LP/BZ benchmark-side previo (solve `minilp` con un
//! checkpoint parcial de filas de precedencia), este bound usa la relajación
//! Lagrangiana de capacidades del core (`compute_pcpsp_lagrangian_bound`),
//! cuyo subproblema interno es un max-closure exacto tiempo-expandido con el
//! **100% de las precedencias en cada iteración**. Para cualquier multiplicador
//! `π >= 0` el valor `L(π)` es un bound superior válido del óptimo entero y de
//! la relajación LP; el dual converge al valor LP (Geoffrion 1974, doi
//! 10.1007/BFb0120690; Dagdelen & Johnson 1986, APCOM 19).
//!
//! Además deriva scores de orden desde la mejor solución interna de la
//! relajación y construye un candidato TopoSort **self-contained** (sin
//! consumir las relajaciones LP staged de MineLib), cerrando ese gap de
//! protocolo de MR-211/MR-212.
//!
//! Uso:
//!   cargo run --release -p marvin-benchmark --bin pcpsp_bound -- [--include-full] [--quiet] [output_path]
//!
//! Por defecto corre las formulaciones CPIT y PCPSP de `marvin`;
//! `--include-full` agrega `mclaughlin-limit` (pesado: el grafo expandido
//! supera 45M de arcos por iteración). Si no se especifica `output_path`,
//! escribe `datasets/benchmarks/outputs/pcpsp-bound-report.json`. Las rutas
//! relativas se rebasan contra la raíz del repo (política MR-202).

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

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use benchmark_cli_support::parse_benchmark_cli_args;
use benchmark_path_policy::BenchmarkPathPolicy;
use benchmark_runtime_telemetry::{RuntimeTelemetry, StageTimer};
use cpit_toposort_support::build_toposort_problem_from_minelib_cpit;
use marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleProblemKind,
    MarvinScheduleSolution, read_minelib_cpit_problem, read_minelib_pcpsp_problem,
    read_minelib_precedence_graph, summarize_minelib_schedule_solution,
};
use mine_sdk::{
    LagrangianBoundOptions, MineError, PcpspToposortOptions, PcpspToposortSchedule,
    PrecedenceGraph, compute_pcpsp_lagrangian_bound, solve_pcpsp_with_toposort,
};
use pcpsp_toposort_support::{
    build_pcpsp_toposort_problem_from_minelib, pcpsp_problem_from_cpit_toposort,
    verify_pcpsp_schedule_precedence,
};
use serde::Serialize;

const REPORT_TICKETS: &[&str] = &["MR-212", "MR-213", "MR-215"];
const DEFAULT_OUTPUT_RELATIVE_PATH: &str = "datasets/benchmarks/outputs/pcpsp-bound-report.json";
const AUDIT_OBJECTIVE_RELATIVE_TOLERANCE: f64 = 1.0e-9;

const LITERATURE_CONTEXT: &[&str] = &[
    "Bound method: Lagrangian relaxation of capacity constraints with an exact time-expanded max-closure inner subproblem (100% precedence coverage every iteration); any multiplier vector yields a valid upper bound for the integer optimum and the LP relaxation, and the Lagrangian dual equals the LP bound by the integrality property (Geoffrion 1974, doi 10.1007/BFb0120690; mining application: Dagdelen & Johnson 1986, Proc. 19th APCOM, conference/practice literature).",
    "Relation to BZ: the Bienstock-Zuckerberg algorithm ([R34] Munoz et al., doi 10.1007/s10589-017-9946-1) solves the same LP relaxation by specialized decomposition; this Lagrangian route reaches the same bound value in the limit. Finite subgradient iterations report the best (lowest) valid bound found, which may remain above the official LP value.",
    "Self-contained candidate: ordering scores derive from the best inner relaxation solution (no staged MineLib LP artifacts consumed), removing the protocol caveat of MR-211/MR-212 toposort candidates.",
    "Official LP/incumbent values come from the staged MineLib info files ([R29] Espinoza et al., doi 10.1007/s10479-012-1258-3).",
];

struct FormulationConfig {
    run_id: &'static str,
    dataset_id: &'static str,
    blocks_file: &'static str,
    precedence_file: &'static str,
    problem_file: &'static str,
    kind: MarvinScheduleProblemKind,
    official_lp_objective: f64,
    official_incumbent_objective: f64,
    /// Candidato propio versionado usado como hint de cota inferior para la
    /// regla de paso del subgradiente (no afecta la validez del bound).
    lower_bound_hint: f64,
    iterations: usize,
    official_source: &'static str,
    heavy: bool,
}

const FORMULATIONS: &[FormulationConfig] = &[
    FormulationConfig {
        run_id: "marvin-cpit",
        dataset_id: "marvin",
        blocks_file: "marvin.blocks",
        precedence_file: "marvin.prec",
        problem_file: "marvin.cpit",
        kind: MarvinScheduleProblemKind::Cpit,
        official_lp_objective: 863_916_131.0,
        official_incumbent_objective: 820_726_048.0,
        lower_bound_hint: 831_910_167.0, // candidato MR-211 (cpit-toposort-report.json)
        iterations: 120,
        official_source: "datasets/benchmarks/marvin/marving-info.txt",
        heavy: false,
    },
    FormulationConfig {
        run_id: "marvin-pcpsp",
        dataset_id: "marvin",
        blocks_file: "marvin.blocks",
        precedence_file: "marvin.prec",
        problem_file: "marvin.pcpsp",
        kind: MarvinScheduleProblemKind::Pcpsp,
        official_lp_objective: 911_704_665.0,
        official_incumbent_objective: 885_968_070.0,
        lower_bound_hint: 829_532_040.0, // candidato MR-212 (pcpsp-toposort-report.json)
        iterations: 120,
        official_source: "datasets/benchmarks/marvin/marving-info.txt",
        heavy: false,
    },
    FormulationConfig {
        run_id: "mclaughlin-limit-cpit",
        dataset_id: "mclaughlin-limit",
        blocks_file: "mclaughlin_limit.blocks",
        precedence_file: "mclaughlin_limit.prec",
        problem_file: "mclaughlin_limit.cpit",
        kind: MarvinScheduleProblemKind::Cpit,
        official_lp_objective: 1_078_979_501.0,
        official_incumbent_objective: 1_073_327_197.0,
        lower_bound_hint: 1_076_575_736.0, // candidato MR-211
        iterations: 12,
        official_source: "datasets/benchmarks/mclaughlin-limit/mclaughlin-limit-info.txt",
        heavy: true,
    },
    FormulationConfig {
        run_id: "mclaughlin-limit-pcpsp",
        dataset_id: "mclaughlin-limit",
        blocks_file: "mclaughlin_limit.blocks",
        precedence_file: "mclaughlin_limit.prec",
        problem_file: "mclaughlin_limit.pcpsp",
        kind: MarvinScheduleProblemKind::Pcpsp,
        official_lp_objective: 1_324_829_727.0,
        official_incumbent_objective: 1_321_662_551.0,
        lower_bound_hint: 1_072_520_168.0, // candidato MR-212
        iterations: 12,
        official_source: "datasets/benchmarks/mclaughlin-limit/mclaughlin-limit-info.txt",
        heavy: true,
    },
];

#[derive(Debug, Serialize)]
struct SelfContainedCandidateRecord {
    ordering_score_source: String,
    scheduled_block_count: usize,
    used_period_count: usize,
    used_destination_count: usize,
    discounted_objective: f64,
    gap_vs_official_incumbent_relative: f64,
    audited_discounted_objective: f64,
    audit_objective_consistent: bool,
    audited_max_resource_excess: f64,
    precedence_edges_verified: usize,
    precedence_feasibility_verified: bool,
}

#[derive(Debug, Serialize)]
struct FormulationBoundRecord {
    run_id: String,
    dataset_id: String,
    formulation: String,
    method: String,
    period_count: usize,
    destination_count: usize,
    block_count: usize,
    expanded_node_count: usize,
    expanded_precedence_arc_count: usize,
    precedence_coverage_completeness: String,
    multiplier_count: usize,
    iterations_executed: usize,
    best_bound: f64,
    best_iteration: usize,
    unconstrained_iteration_zero_bound: f64,
    official_lp_objective: f64,
    official_source: String,
    bound_vs_official_lp_relative: f64,
    bound_validity_note: String,
    iteration_bounds: Vec<f64>,
    self_contained_candidate: SelfContainedCandidateRecord,
    runtime_telemetry: RuntimeTelemetry,
}

#[derive(Debug, Serialize)]
struct SkippedFormulationRecord {
    run_id: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct PcpspBoundReport {
    generated_by: String,
    tickets: Vec<String>,
    literature_context: Vec<String>,
    formulations: Vec<FormulationBoundRecord>,
    skipped_formulations: Vec<SkippedFormulationRecord>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = parse_benchmark_cli_args(&args).map_err(std::io::Error::other)?;
    let path_policy = BenchmarkPathPolicy::discover()?;

    let mut formulation_records = Vec::new();
    let mut skipped_formulations = Vec::new();

    for config in FORMULATIONS {
        if config.heavy && !options.include_full {
            skipped_formulations.push(SkippedFormulationRecord {
                run_id: config.run_id.to_owned(),
                reason: "heavy expanded graph (>45M arcs per iteration); rerun with \
                         --include-full to measure it"
                    .to_owned(),
            });
            continue;
        }

        if !options.quiet {
            println!("[pcpsp-bound] running `{}`...", config.run_id);
        }
        let record = run_formulation(&path_policy, config)?;
        if !options.quiet {
            println!(
                "[pcpsp-bound] `{}`: best_bound={:.3} (official LP {:.0}, +{:.3}%) iter0={:.3} candidate={:.3} (gap {:.3}%)",
                record.run_id,
                record.best_bound,
                record.official_lp_objective,
                record.bound_vs_official_lp_relative * 100.0,
                record.unconstrained_iteration_zero_bound,
                record.self_contained_candidate.discounted_objective,
                record
                    .self_contained_candidate
                    .gap_vs_official_incumbent_relative
                    * 100.0,
            );
        }
        formulation_records.push(record);
    }

    let report = PcpspBoundReport {
        generated_by: "cargo run --release -p marvin-benchmark --bin pcpsp_bound".to_owned(),
        tickets: REPORT_TICKETS.iter().map(ToString::to_string).collect(),
        literature_context: LITERATURE_CONTEXT.iter().map(ToString::to_string).collect(),
        formulations: formulation_records,
        skipped_formulations,
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
        println!("[pcpsp-bound] report written to {}", output_path.display());
    }
    Ok(())
}

fn run_formulation(
    path_policy: &BenchmarkPathPolicy,
    config: &FormulationConfig,
) -> Result<FormulationBoundRecord, Box<dyn std::error::Error>> {
    let dataset_dir = path_policy.dataset_dir(config.dataset_id);
    let references_dir = path_policy.references_dir(&dataset_dir);

    let mut timer = StageTimer::start();

    let model = read_benchmark_blocks(dataset_dir.join(config.blocks_file), config.dataset_id)?;
    timer.record_stage("read-blocks");

    let problem_path = references_dir.join(config.problem_file);
    let (minelib_problem, toposort_problem) = match config.kind {
        MarvinScheduleProblemKind::Cpit => {
            let minelib_problem = read_minelib_cpit_problem(&problem_path, &model)?;
            let (cpit_problem, _) = build_toposort_problem_from_minelib_cpit(&minelib_problem)?;
            let pcpsp_problem = pcpsp_problem_from_cpit_toposort(&cpit_problem);
            (minelib_problem, pcpsp_problem)
        }
        MarvinScheduleProblemKind::Pcpsp => {
            let minelib_problem = read_minelib_pcpsp_problem(&problem_path, &model)?;
            let (pcpsp_problem, _) = build_pcpsp_toposort_problem_from_minelib(&minelib_problem)?;
            (minelib_problem, pcpsp_problem)
        }
    };
    timer.record_stage("read-and-adapt-problem");

    let precedence_graph =
        read_minelib_precedence_graph(references_dir.join(config.precedence_file), &model)?;
    timer.record_stage("read-precedence");

    let bound_result = compute_pcpsp_lagrangian_bound(
        &toposort_problem,
        &precedence_graph,
        &LagrangianBoundOptions {
            max_iterations: config.iterations,
            lower_bound_hint: Some(config.lower_bound_hint),
            ..LagrangianBoundOptions::default()
        },
    )?;
    timer.record_stage("lagrangian-subgradient");

    // Candidato self-contained: scores de orden desde la mejor solución
    // interna de la relajación (sin artefactos LP staged).
    let ordering_scores: BTreeMap<usize, f64> = bound_result
        .best_inner_assignments
        .iter()
        .map(|assignment| (assignment.linear_index, assignment.period_index as f64))
        .collect();
    let candidate_schedule = solve_pcpsp_with_toposort(
        &toposort_problem,
        &precedence_graph,
        &ordering_scores,
        &PcpspToposortOptions::default(),
    )?;
    timer.record_stage("self-contained-toposort-candidate");

    let candidate_record = audit_self_contained_candidate(
        config,
        &minelib_problem,
        &precedence_graph,
        &candidate_schedule,
    )?;
    timer.record_stage("audit-candidate");

    let iteration_bounds: Vec<f64> = bound_result
        .iteration_records
        .iter()
        .map(|record| record.bound)
        .collect();
    let formulation_label = match config.kind {
        MarvinScheduleProblemKind::Cpit => "CPIT",
        MarvinScheduleProblemKind::Pcpsp => "PCPSP",
    };

    Ok(FormulationBoundRecord {
        run_id: config.run_id.to_owned(),
        dataset_id: config.dataset_id.to_owned(),
        formulation: formulation_label.to_owned(),
        method: "core mine-planning::compute_pcpsp_lagrangian_bound (Lagrangian relaxation of \
                 capacities; exact time-expanded max-closure inner subproblem via Dinic)"
            .to_owned(),
        period_count: toposort_problem.period_count,
        destination_count: toposort_problem.destination_count,
        block_count: toposort_problem.block_values.len(),
        expanded_node_count: bound_result.expanded_node_count,
        expanded_precedence_arc_count: bound_result.expanded_precedence_arc_count,
        precedence_coverage_completeness:
            "complete: every block-level precedence edge is expanded to all periods in every \
             subgradient iteration (no partial checkpoints)"
                .to_owned(),
        multiplier_count: bound_result.multiplier_count,
        iterations_executed: bound_result.iteration_records.len(),
        best_bound: bound_result.best_bound,
        best_iteration: bound_result.best_iteration,
        unconstrained_iteration_zero_bound: iteration_bounds.first().copied().unwrap_or(f64::NAN),
        official_lp_objective: config.official_lp_objective,
        official_source: config.official_source.to_owned(),
        bound_vs_official_lp_relative: (bound_result.best_bound - config.official_lp_objective)
            / config.official_lp_objective,
        bound_validity_note: "valid upper bound for the integer optimum and the LP relaxation \
                              at every iteration; finite subgradient iterations may leave it \
                              above the official LP value (remaining dual gap, not a coverage \
                              shortcut)"
            .to_owned(),
        iteration_bounds,
        self_contained_candidate: candidate_record,
        runtime_telemetry: timer.finish(),
    })
}

fn audit_self_contained_candidate(
    config: &FormulationConfig,
    minelib_problem: &MarvinScheduleProblem,
    precedence_graph: &PrecedenceGraph,
    schedule: &PcpspToposortSchedule,
) -> Result<SelfContainedCandidateRecord, MineError> {
    let precedence_edges_verified = verify_pcpsp_schedule_precedence(schedule, precedence_graph)?;
    let solution = MarvinScheduleSolution {
        kind: config.kind,
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
    let summary = summarize_minelib_schedule_solution(minelib_problem, &solution)?;

    let audit_consistent = (summary.discounted_objective - schedule.discounted_objective).abs()
        <= AUDIT_OBJECTIVE_RELATIVE_TOLERANCE * schedule.discounted_objective.abs().max(1.0);
    let max_resource_excess = summary
        .resource_summaries
        .iter()
        .map(|resource| resource.max_period_excess)
        .fold(0.0_f64, f64::max);

    Ok(SelfContainedCandidateRecord {
        ordering_score_source: "expected extraction period from the best inner solution of the \
                                own Lagrangian relaxation (self-contained: no staged MineLib LP \
                                artifacts consumed)"
            .to_owned(),
        scheduled_block_count: schedule.scheduled_block_count,
        used_period_count: schedule.used_period_count,
        used_destination_count: schedule.used_destination_count,
        discounted_objective: schedule.discounted_objective,
        gap_vs_official_incumbent_relative: (config.official_incumbent_objective
            - schedule.discounted_objective)
            / config.official_incumbent_objective,
        audited_discounted_objective: summary.discounted_objective,
        audit_objective_consistent: audit_consistent,
        audited_max_resource_excess: max_resource_excess,
        precedence_edges_verified,
        precedence_feasibility_verified: true,
    })
}
