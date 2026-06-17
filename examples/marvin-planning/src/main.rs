//! Ejemplo ejecutable para aplicar planning experimental sobre `marvin.blocks`.
//!
//! Uso:
//!   cargo run -p marvin-planning [dataset_path] [output_path]
//!
//! Si no se especifican argumentos, el dataset se toma desde `datasets/benchmarks/marvin/marvin.blocks`
//! y el reporte se escribe en `datasets/benchmarks/marvin/outputs/planning-report.json`.

#[path = "../../marvin-benchmark/src/benchmark_blocks_support.rs"]
mod benchmark_blocks_support;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use mine_sdk::{
    blockmodel::{BlockModel, ColumnData},
    core::{ColumnId, MineError},
    experimental::{
        PushbackGenerationRules, PushbackPrototype, build_pushback_prototype, build_upit_prototype,
    },
    planning::{
        BenchAssignment, BenchParameters, BlockPrecedenceTemplate, PrecedenceOffset,
        ScheduleConstraints, ScheduleEntry, SchedulePeriodSummary, assign_benches,
        build_block_precedence_graph, build_schedule,
    },
};
use serde::Serialize;

const PHASE_BENCH_SPAN: i64 = 4;

#[derive(Debug, Serialize)]
struct MarvinPlanningOutput {
    dataset_path: String,
    value_column: String,
    tonnage_column: String,
    precedence_node_count: usize,
    precedence_edge_count: usize,
    upit_block_count: usize,
    upit_total_value: f64,
    upit_total_tonnage: Option<f64>,
    schedule_period_count: usize,
    schedule_entry_count: usize,
    schedule_violation_count: usize,
    schedule_period_summaries: Vec<SchedulePeriodSummary>,
    pushback_count: usize,
    pushbacks: Vec<PushbackPrototype>,
    assumptions: Vec<String>,
    limitations: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let marvin_dir = repo_root.join("datasets").join("benchmarks").join("marvin");
    let dataset_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| marvin_dir.join("marvin.blocks"));
    let output_path = env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| marvin_dir.join("outputs").join("planning-report.json"));
    let model = read_benchmark_blocks(&dataset_path, "marvin")?;
    let value_column = ColumnId::new("field_7")?;
    let tonnage_column = ColumnId::new("field_4")?;
    let template = BlockPrecedenceTemplate::new(vec![
        PrecedenceOffset::new(0, 0, 1)?,
        PrecedenceOffset::new(-1, 0, 1)?,
        PrecedenceOffset::new(1, 0, 1)?,
        PrecedenceOffset::new(0, -1, 1)?,
        PrecedenceOffset::new(0, 1, 1)?,
    ])?;
    let precedence_graph = build_block_precedence_graph(&model, &template)?;
    let upit = build_upit_prototype(
        &model,
        &precedence_graph,
        &value_column,
        Some(&tonnage_column),
    )?;
    let bench_assignments = assign_benches(&model, &BenchParameters::new(1.0, 0.0, 1e-9)?)?;
    let schedule_entries = build_schedule_entries(
        &model,
        &bench_assignments,
        &upit.selected_linear_indices,
        &tonnage_column,
    )?;
    let schedule = build_schedule(schedule_entries, ScheduleConstraints::default())?;
    let pushback_report =
        build_pushback_prototype(&schedule, &PushbackGenerationRules::new(true, None)?)?;
    let output = MarvinPlanningOutput {
        dataset_path: dataset_path.display().to_string(),
        value_column: value_column.to_string(),
        tonnage_column: tonnage_column.to_string(),
        precedence_node_count: precedence_graph.nodes().len(),
        precedence_edge_count: precedence_graph.edges().len(),
        upit_block_count: upit.block_count,
        upit_total_value: upit.total_value,
        upit_total_tonnage: upit.total_tonnage,
        schedule_period_count: schedule.period_summaries().len(),
        schedule_entry_count: schedule.entries().len(),
        schedule_violation_count: schedule.violations().len(),
        schedule_period_summaries: schedule.period_summaries().to_vec(),
        pushback_count: pushback_report.pushbacks.len(),
        pushbacks: pushback_report.pushbacks.clone(),
        assumptions: vec![
            "marvin.blocks is loaded with the current unit-grid i/j/k staging used by benchmark_blocks_support::read_benchmark_blocks(..., \"marvin\").".to_owned(),
            "field_7 is treated as value and field_4 as tonnage only for this experimental workflow.".to_owned(),
            format!(
                "Phase labels are synthetic bench bands of {PHASE_BENCH_SPAN} benches each, not official Marvin phases."
            ),
        ],
        limitations: vec![
            "This workflow does not run an exact optimal pit solver; it uses the existing positive-block-closure heuristic.".to_owned(),
            "Pushbacks are grouped from the derived schedule and synthetic phase labels, so they are planning prototypes rather than calibrated mine designs.".to_owned(),
            "Results remain non-comparable against official Marvin references until external prec/upit artifacts and field semantics are verified.".to_owned(),
        ],
    };

    let json = serde_json::to_string_pretty(&output)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, &json)?;
    eprintln!("planning report written to {}", output_path.display());
    println!("{json}");

    Ok(())
}
fn build_schedule_entries(
    model: &BlockModel,
    bench_assignments: &[BenchAssignment],
    selected_linear_indices: &[usize],
    tonnage_column: &ColumnId,
) -> Result<Vec<ScheduleEntry>, MineError> {
    let tonnage_values = float_column(model, tonnage_column, "tonnage")?;
    let selected = selected_linear_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let bench_by_linear_index = bench_assignments
        .iter()
        .map(|assignment| (assignment.linear_index, assignment.bench))
        .collect::<BTreeMap<_, _>>();
    let mut by_bench = BTreeMap::<i64, (f64, usize)>::new();

    for (row_index, tonnage) in tonnage_values.iter().enumerate() {
        let linear_index = model.linear_index_at(row_index)?;
        if !selected.contains(&linear_index) {
            continue;
        }

        let bench = *bench_by_linear_index
            .get(&linear_index)
            .expect("every materialized block should have a bench assignment");
        let entry = by_bench.entry(bench).or_insert((0.0, 0));
        entry.0 += *tonnage;
        entry.1 += 1;
    }

    if by_bench.is_empty() {
        return Err(MineError::Planning {
            message: "marvin planning example requires at least one selected block".to_owned(),
        });
    }

    let max_bench = *by_bench
        .keys()
        .next_back()
        .expect("by_bench should not be empty");

    by_bench
        .into_iter()
        .rev()
        .enumerate()
        .map(|(period_index, (bench, (tonnage, block_count)))| {
            let phase_index = ((max_bench - bench) / PHASE_BENCH_SPAN) + 1;
            ScheduleEntry::new(
                format!("P{:02}", period_index + 1),
                bench,
                tonnage,
                block_count,
                Some(format!("phase-{phase_index:02}")),
            )
        })
        .collect()
}

fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
    purpose: &str,
) -> Result<&'a [f64], MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` does not exist in block model storage"
        )));
    };

    let ColumnData::Floats(values) = column_data else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` must be a float column"
        )));
    };

    Ok(values)
}
