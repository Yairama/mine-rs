//! Ejemplo ejecutable para comparar referencias Marvin locales contra salidas actuales de `mine-rs`.
//!
//! Uso:
//!   cargo run -p marvin-benchmark [dataset_dir] [output_path]
//!
//! Si no se especifican argumentos, el dataset se toma desde `datasets/benchmarks/marvin/`
//! y el reporte se escribe en `datasets/benchmarks/marvin/outputs/comparison-report.json`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use mine_sdk::{
    BlockModel, BlockPrecedenceTemplate, ColumnData, ColumnId, NumericMetricComparisonReport,
    NumericMetricTolerance, PrecedenceNode, PrecedenceOffset, build_block_precedence_graph,
    build_upit_prototype, compare_block_memberships, compare_named_numeric_metrics,
    read_marvin_blocks, read_marvin_precedence_graph, read_marvin_upit_solution,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct MarvinBenchmarkOutput {
    dataset_dir: String,
    reference_prec_path: String,
    reference_upit_solution_path: String,
    value_column: String,
    tonnage_column: String,
    candidate_predecessor_offsets: Vec<(isize, isize, isize)>,
    reference_precedence: PrecedenceArtifactSummary,
    candidate_precedence: PrecedenceArtifactSummary,
    precedence_comparison: CompactPrecedenceComparison,
    reference_upit: MembershipArtifactSummary,
    candidate_upit: MembershipArtifactSummary,
    upit_membership_comparison: CompactMembershipComparison,
    upit_metric_comparison: NumericMetricComparisonReport,
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
    let prec_path = dataset_dir.join("marvin.prec");
    let upit_solution_path = dataset_dir.join("marvin_upit.sol");

    let model = read_marvin_blocks(&blocks_path)?;
    let reference_prec = read_marvin_precedence_graph(&prec_path, &model)?;
    let reference_upit_membership = read_marvin_upit_solution(&upit_solution_path, &model)?;

    let template = marvin_slope_template()?;
    let candidate_prec = build_block_precedence_graph(&model, &template)?;
    let precedence_comparison = compact_precedence_comparison(mine_sdk::compare_precedence_graphs(
        &reference_prec,
        &candidate_prec,
    ));

    let value_column = ColumnId::new("field_7")?;
    let tonnage_column = ColumnId::new("field_4")?;
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

    let output = MarvinBenchmarkOutput {
        dataset_dir: relative_or_display(&dataset_dir, &repo_root),
        reference_prec_path: relative_or_display(&prec_path, &repo_root),
        reference_upit_solution_path: relative_or_display(&upit_solution_path, &repo_root),
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
        upit_membership_comparison,
        upit_metric_comparison,
        assumptions: vec![
            "marving-info.txt was used to confirm that field_4 is tonnage and field_7 is proc_profit ($/ton), and that mine_cost = 0.9 $/ton.".to_owned(),
            "The candidate precedence template uses the 17-offset Marvin slope pattern (45°/8-niveles): 5 cross at dk=1, 4 diagonal corners at dk=3, 8 near-circle at dk=5.".to_owned(),
            "total_economic_objective = sum((max(proc_profit, 0) - 0.9) × tonnage). Official UPIT target: 1,415,655,436.".to_owned(),
        ],
        limitations: vec![
            "This benchmark does not yet normalize or compare CPIT, PCPSP or LP relaxation artifacts (pending MR-170).".to_owned(),
            "UPIT comparison is against the existing positive-block-closure heuristic, not an exact maximum-closure solver (pending MR-155/MR-156).".to_owned(),
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
