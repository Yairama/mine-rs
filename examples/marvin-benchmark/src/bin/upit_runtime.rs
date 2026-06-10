//! Benchmark de runtime y escalabilidad del solver exacto de UPIT (MR-209).
//!
//! Mide el tiempo de pared por etapa (carga, precedencias, construcción del
//! grafo de cierre máximo y solve Dinic) sobre las instancias MineLib staged y
//! compara el valor del pit contra el objetivo oficial publicado por MineLib
//! ([R29] Espinoza et al., doi 10.1007/s10479-012-1258-3). La literatura de
//! referencia resuelve UPIT de millones de bloques en segundos con pseudoflow
//! (Hochbaum 2008, Operations Research 56(4):992-1009).
//!
//! Uso:
//!   cargo run --release -p marvin-benchmark --bin upit_runtime -- [--include-full] [--quiet] [output_path]
//!
//! Por defecto corre `marvin` y `mclaughlin-limit`; la instancia `mclaughlin`
//! full (2.14M bloques) solo corre con `--include-full` por su costo. Si no se
//! especifica `output_path`, el reporte se escribe en
//! `datasets/benchmarks/outputs/upit-runtime-report.json`. Las rutas relativas
//! se rebasan contra la raíz del repo (política MR-202).

#[path = "../benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
#[path = "../benchmark_cli_support.rs"]
mod benchmark_cli_support;
#[path = "../benchmark_path_policy.rs"]
mod benchmark_path_policy;
#[path = "../benchmark_runtime_telemetry.rs"]
mod benchmark_runtime_telemetry;
#[path = "../marvin_support.rs"]
mod marvin_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use benchmark_cli_support::parse_benchmark_cli_args;
use benchmark_path_policy::BenchmarkPathPolicy;
use benchmark_runtime_telemetry::{RuntimeTelemetry, StageTimer};
use marvin_support::{read_minelib_precedence_graph, read_minelib_upit_block_values};
use mine_sdk::{build_max_closure_graph, solve_upl_exact, verify_closure};
use serde::Serialize;

const REPORT_TICKETS: &[&str] = &["MR-209", "MR-215"];
const DEFAULT_OUTPUT_RELATIVE_PATH: &str = "datasets/benchmarks/outputs/upit-runtime-report.json";
const OBJECTIVE_MATCH_ABSOLUTE_TOLERANCE: f64 = 1.0;

/// Contexto bibliográfico fijo del benchmark.
const LITERATURE_CONTEXT: &[&str] = &[
    "MineLib official UPIT objectives come from Hochbaum's pseudoflow solver; see [R29] Espinoza et al. (2013), Annals of Operations Research 206:93-114, doi 10.1007/s10479-012-1258-3.",
    "Published pseudoflow implementations solve multi-million-block UPIT instances in seconds; see Hochbaum (2008), Operations Research 56(4):992-1009 (cited by the staged *-info.txt files).",
    "mine-rs currently uses a Dinic max-flow backend (MR-173); this report versions its wall-clock behaviour to decide whether a pseudoflow/push-relabel backend is required (MR-209).",
];

struct DatasetConfig {
    dataset_id: &'static str,
    blocks_file: &'static str,
    precedence_file: &'static str,
    upit_objective_file: &'static str,
    official_upit_objective: f64,
    official_source: &'static str,
    heavy: bool,
}

const DATASETS: &[DatasetConfig] = &[
    DatasetConfig {
        dataset_id: "marvin",
        blocks_file: "marvin.blocks",
        precedence_file: "marvin.prec",
        upit_objective_file: "marvin.upit",
        official_upit_objective: 1_415_655_436.0,
        official_source: "datasets/benchmarks/marvin/marving-info.txt",
        heavy: false,
    },
    DatasetConfig {
        dataset_id: "mclaughlin-limit",
        blocks_file: "mclaughlin_limit.blocks",
        precedence_file: "mclaughlin_limit.prec",
        upit_objective_file: "mclaughlin_limit.upit",
        official_upit_objective: 1_495_726_474.0,
        official_source: "datasets/benchmarks/mclaughlin-limit/mclaughlin-limit-info.txt",
        heavy: false,
    },
    DatasetConfig {
        dataset_id: "mclaughlin",
        blocks_file: "mclaughlin.blocks",
        precedence_file: "mclaughlin.prec",
        upit_objective_file: "mclaughlin.upit",
        official_upit_objective: 1_495_886_962.0,
        official_source: "datasets/benchmarks/mclaughlin/mclaughlin-info.txt",
        heavy: true,
    },
];

#[derive(Debug, Serialize)]
struct DatasetUpitRuntimeRecord {
    dataset_id: String,
    solver_backend: String,
    block_count: usize,
    weighted_block_count: usize,
    precedence_edge_count: usize,
    selected_block_count: usize,
    pit_value: f64,
    official_upit_objective: f64,
    official_source: String,
    objective_absolute_difference: f64,
    objective_relative_difference: f64,
    matches_official_objective: bool,
    closure_verified: bool,
    runtime_telemetry: RuntimeTelemetry,
}

#[derive(Debug, Serialize)]
struct SkippedDatasetRecord {
    dataset_id: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct UpitRuntimeReport {
    generated_by: String,
    tickets: Vec<String>,
    literature_context: Vec<String>,
    datasets: Vec<DatasetUpitRuntimeRecord>,
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
            println!("[upit-runtime] running dataset `{}`...", config.dataset_id);
        }
        let record = run_dataset(&path_policy, config)?;
        if !options.quiet {
            println!(
                "[upit-runtime] `{}`: pit_value={:.3} (official {:.0}, diff {:.3}) selected={} total_ms={:.1}",
                record.dataset_id,
                record.pit_value,
                record.official_upit_objective,
                record.objective_absolute_difference,
                record.selected_block_count,
                record.runtime_telemetry.total_wall_clock_ms,
            );
        }
        dataset_records.push(record);
    }

    let report = UpitRuntimeReport {
        generated_by: "cargo run --release -p marvin-benchmark --bin upit_runtime".to_owned(),
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
        println!("[upit-runtime] report written to {}", output_path.display());
    }
    Ok(())
}

fn run_dataset(
    path_policy: &BenchmarkPathPolicy,
    config: &DatasetConfig,
) -> Result<DatasetUpitRuntimeRecord, Box<dyn std::error::Error>> {
    let dataset_dir = path_policy.dataset_dir(config.dataset_id);
    let references_dir = path_policy.references_dir(&dataset_dir);

    let mut timer = StageTimer::start();

    let model = read_benchmark_blocks(dataset_dir.join(config.blocks_file), config.dataset_id)?;
    timer.record_stage("read-blocks");

    let upit_block_values =
        read_minelib_upit_block_values(references_dir.join(config.upit_objective_file), &model)?;
    timer.record_stage("read-upit-objective");

    let precedence_graph =
        read_minelib_precedence_graph(references_dir.join(config.precedence_file), &model)?;
    timer.record_stage("read-precedence");

    let block_weights: BTreeMap<usize, f64> = upit_block_values.iter().copied().collect();
    let closure_graph = build_max_closure_graph(&block_weights, &precedence_graph)?;
    timer.record_stage("build-max-closure-graph");

    let result = solve_upl_exact(&closure_graph)?;
    timer.record_stage("solve-upl-exact-dinic");

    verify_closure(&result.selected_blocks, &precedence_graph)?;
    timer.record_stage("verify-closure");

    let telemetry = timer.finish();
    let absolute_difference = (result.pit_value - config.official_upit_objective).abs();
    let relative_difference = absolute_difference / config.official_upit_objective;

    Ok(DatasetUpitRuntimeRecord {
        dataset_id: config.dataset_id.to_owned(),
        solver_backend: "dinic-max-flow (mine-planning::solve_upl_exact, MR-173)".to_owned(),
        block_count: model.block_count(),
        weighted_block_count: block_weights.len(),
        precedence_edge_count: precedence_graph.edges().len(),
        selected_block_count: result.selected_block_count,
        pit_value: result.pit_value,
        official_upit_objective: config.official_upit_objective,
        official_source: config.official_source.to_owned(),
        objective_absolute_difference: absolute_difference,
        objective_relative_difference: relative_difference,
        matches_official_objective: absolute_difference <= OBJECTIVE_MATCH_ABSOLUTE_TOLERANCE,
        closure_verified: true,
        runtime_telemetry: telemetry,
    })
}
