//! Tests de integración para normalización y comparación del benchmark Marvin.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mine_core::ColumnId;
use mine_io::read_marvin_blocks;
use mine_planning::{
    BlockPrecedenceTemplate, NumericMetricTolerance, PrecedenceOffset,
    build_block_precedence_graph, build_upit_prototype, compare_block_memberships,
    compare_named_numeric_metrics, compare_precedence_graphs, read_marvin_precedence_graph,
    read_marvin_upit_block_values, read_marvin_upit_solution,
};

fn marvin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("datasets")
        .join("benchmarks")
        .join("marvin")
}

/// Plantilla de talud Marvin 45°/8-niveles (17 offsets). Reverse-engineered en MR-167.
fn marvin_slope_template() -> BlockPrecedenceTemplate {
    BlockPrecedenceTemplate::new(vec![
        // dk=1: cruce cardinal (5 bloques)
        PrecedenceOffset::new(0, 0, 1).expect("offset should be valid"),
        PrecedenceOffset::new(-1, 0, 1).expect("offset should be valid"),
        PrecedenceOffset::new(1, 0, 1).expect("offset should be valid"),
        PrecedenceOffset::new(0, -1, 1).expect("offset should be valid"),
        PrecedenceOffset::new(0, 1, 1).expect("offset should be valid"),
        // dk=3: esquinas diagonales (4 bloques)
        PrecedenceOffset::new(-2, -2, 3).expect("offset should be valid"),
        PrecedenceOffset::new(-2, 2, 3).expect("offset should be valid"),
        PrecedenceOffset::new(2, -2, 3).expect("offset should be valid"),
        PrecedenceOffset::new(2, 2, 3).expect("offset should be valid"),
        // dk=5: arco semicircular (8 bloques)
        PrecedenceOffset::new(-4, -3, 5).expect("offset should be valid"),
        PrecedenceOffset::new(-4, 3, 5).expect("offset should be valid"),
        PrecedenceOffset::new(-3, -4, 5).expect("offset should be valid"),
        PrecedenceOffset::new(-3, 4, 5).expect("offset should be valid"),
        PrecedenceOffset::new(3, -4, 5).expect("offset should be valid"),
        PrecedenceOffset::new(3, 4, 5).expect("offset should be valid"),
        PrecedenceOffset::new(4, -3, 5).expect("offset should be valid"),
        PrecedenceOffset::new(4, 3, 5).expect("offset should be valid"),
    ])
    .expect("template should be valid")
}

#[test]
fn normalize_staged_marvin_prec_and_upit_solution() {
    let dir = marvin_dir();
    let model = read_marvin_blocks(dir.join("marvin.blocks")).expect("marvin.blocks should load");

    let precedence_graph = read_marvin_precedence_graph(dir.join("marvin.prec"), &model)
        .expect("marvin.prec should normalize");
    let upit_solution = read_marvin_upit_solution(dir.join("marvin_upit.sol"), &model)
        .expect("marvin_upit.sol should normalize");

    assert_eq!(precedence_graph.nodes().len(), model.block_count());
    assert_eq!(upit_solution.len(), 8516);
    assert_eq!(
        upit_solution.len(),
        upit_solution
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    assert!(precedence_graph.edges().len() > model.block_count());
}

#[test]
fn marvin_slope_template_reaches_full_prec_parity() {
    let dir = marvin_dir();
    let model = read_marvin_blocks(dir.join("marvin.blocks")).expect("marvin.blocks should load");
    let reference_prec = read_marvin_precedence_graph(dir.join("marvin.prec"), &model)
        .expect("marvin.prec should normalize");
    let reference_upit = read_marvin_upit_solution(dir.join("marvin_upit.sol"), &model)
        .expect("marvin_upit.sol should normalize");

    let candidate_prec = build_block_precedence_graph(&model, &marvin_slope_template())
        .expect("prec should build");
    let precedence_comparison = compare_precedence_graphs(&reference_prec, &candidate_prec);

    // La plantilla 17-offset debe reproducir exactamente el prec de referencia.
    assert_eq!(
        precedence_comparison.edge_jaccard_index, 1.0,
        "17-offset template should achieve edge_jaccard = 1.0 against marvin.prec"
    );
    assert_eq!(
        precedence_comparison.reference_only_edges.len(),
        0,
        "no reference edges should be missing from candidate"
    );
    assert_eq!(
        precedence_comparison.candidate_only_edges.len(),
        0,
        "no candidate edges should be outside reference"
    );

    let value_column = ColumnId::new("field_7").expect("column id should be valid");
    let tonnage_column = ColumnId::new("field_4").expect("column id should be valid");
    let candidate_upit = build_upit_prototype(
        &model,
        &candidate_prec,
        &value_column,
        Some(&tonnage_column),
    )
    .expect("upit should build");
    let upit_comparison =
        compare_block_memberships(&reference_upit, &candidate_upit.selected_linear_indices);

    let reference_metrics =
        membership_metrics(&model, &reference_upit, &value_column, &tonnage_column)
            .expect("reference metrics should compute");
    let candidate_metrics =
        membership_metrics(&model, &candidate_upit.selected_linear_indices, &value_column, &tonnage_column)
            .expect("candidate metrics should compute");

    let metric_comparison = compare_named_numeric_metrics(
        &reference_metrics,
        &candidate_metrics,
        &BTreeMap::from([
            (
                "block_count".to_owned(),
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
        ]),
    );

    // El UPIT heurístico sigue siendo imperfecto (sin solver exacto) pero debe contener
    // todos los bloques de referencia (reference_only = 0).
    assert!(
        upit_comparison.jaccard_index < 1.0,
        "upit jaccard < 1.0: heuristic over-selects without exact max-closure solver"
    );
    assert_eq!(
        upit_comparison.reference_only_blocks.len(),
        0,
        "all reference UPIT blocks should be included in candidate"
    );

    // La fórmula económica debe reproducir el objetivo oficial Marvin para los bloques de referencia.
    let reference_objective = reference_metrics["total_economic_objective"];
    assert!(
        (reference_objective - 1_415_655_436.0_f64).abs() < 1.0,
        "reference economic objective should match official UPIT target 1,415,655,436 (got {reference_objective})"
    );

    assert!(
        metric_comparison
            .shared_metrics
            .iter()
            .any(|metric| !metric.within_tolerance),
        "at least one metric should differ between reference and candidate"
    );
}

fn membership_metrics(
    model: &mine_blockmodel::BlockModel,
    selected_linear_indices: &[usize],
    value_column: &ColumnId,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, f64>, mine_core::MineError> {
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
    model: &'a mine_blockmodel::BlockModel,
    column_id: &ColumnId,
) -> Result<&'a [f64], mine_core::MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(mine_core::MineError::schema(format!(
            "column `{column_id}` does not exist in block model storage"
        )));
    };
    let mine_blockmodel::ColumnData::Floats(values) = column_data else {
        return Err(mine_core::MineError::schema(format!(
            "column `{column_id}` must be a float column"
        )));
    };
    Ok(values)
}

fn row_index_for_linear_index(
    model: &mine_blockmodel::BlockModel,
    linear_index: usize,
) -> Result<usize, mine_core::MineError> {
    for row_index in 0..model.block_count() {
        if model.linear_index_at(row_index)? == linear_index {
            return Ok(row_index);
        }
    }

    Err(mine_core::MineError::validation(format!(
        "linear index `{linear_index}` is not materialized in the block model"
    )))
}

#[test]
fn read_marvin_upit_block_values_sums_to_official_upit_objective_for_selected_blocks() {
    let dir = marvin_dir();
    let model = read_marvin_blocks(dir.join("marvin.blocks")).expect("marvin.blocks should load");
    let upit_solution = read_marvin_upit_solution(dir.join("marvin_upit.sol"), &model)
        .expect("marvin_upit.sol should normalize");
    let block_values = read_marvin_upit_block_values(dir.join("marvin.upit"), &model)
        .expect("marvin.upit should normalize");

    assert_eq!(
        block_values.len(),
        model.block_count(),
        "marvin.upit must contain one entry per block"
    );

    let selected_set: std::collections::BTreeSet<usize> =
        upit_solution.iter().copied().collect();
    let objective_sum: f64 = block_values
        .iter()
        .filter(|(linear_index, _)| selected_set.contains(linear_index))
        .map(|(_, value)| value)
        .sum();

    assert!(
        (objective_sum - 1_415_655_436.0_f64).abs() < 1.0,
        "sum of block objective values over upit solution must match official target 1,415,655,436 (got {objective_sum:.2})"
    );
}

