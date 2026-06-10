//! Tests de integración para normalización y comparación del benchmark Marvin.

#[path = "../../../examples/marvin-benchmark/src/benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
#[path = "../../../examples/marvin-benchmark/src/marvin_support.rs"]
mod marvin_support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use marvin_support::{
    read_marvin_cpit_problem, read_marvin_cpit_solution, read_marvin_lp_cpit_solution,
    read_marvin_lp_pcpsp_solution, read_marvin_pcpsp_problem, read_marvin_pcpsp_solution,
    read_marvin_precedence_graph, read_marvin_upit_block_values, read_marvin_upit_solution,
    summarize_marvin_schedule_solution,
};
use mine_core::ColumnId;
use mine_planning::{
    BlockPrecedenceTemplate, NumericMetricTolerance, PrecedenceOffset,
    build_block_precedence_graph, build_max_closure_graph, build_upit_prototype,
    compare_block_memberships, compare_named_numeric_metrics, compare_precedence_graphs,
    solve_upl_exact,
};

fn marvin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("datasets")
        .join("benchmarks")
        .join("marvin")
}

fn marvin_references_dir() -> PathBuf {
    marvin_dir().join("references")
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
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");

    let precedence_graph = read_marvin_precedence_graph(references_dir.join("marvin.prec"), &model)
        .expect("marvin.prec should normalize");
    let upit_solution = read_marvin_upit_solution(references_dir.join("marvin_upit.sol"), &model)
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
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let reference_prec = read_marvin_precedence_graph(references_dir.join("marvin.prec"), &model)
        .expect("marvin.prec should normalize");
    let reference_upit = read_marvin_upit_solution(references_dir.join("marvin_upit.sol"), &model)
        .expect("marvin_upit.sol should normalize");

    let candidate_prec =
        build_block_precedence_graph(&model, &marvin_slope_template()).expect("prec should build");
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
    let candidate_metrics = membership_metrics(
        &model,
        &candidate_upit.selected_linear_indices,
        &value_column,
        &tonnage_column,
    )
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
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let upit_solution = read_marvin_upit_solution(references_dir.join("marvin_upit.sol"), &model)
        .expect("marvin_upit.sol should normalize");
    let block_values = read_marvin_upit_block_values(references_dir.join("marvin.upit"), &model)
        .expect("marvin.upit should normalize");

    assert_eq!(
        block_values.len(),
        model.block_count(),
        "marvin.upit must contain one entry per block"
    );

    let selected_set: std::collections::BTreeSet<usize> = upit_solution.iter().copied().collect();
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

#[test]
fn read_marvin_cpit_reference_matches_official_discounted_objective() {
    let dir = marvin_dir();
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let problem = read_marvin_cpit_problem(references_dir.join("marvin.cpit"), &model)
        .expect("cpit should load");
    let solution =
        read_marvin_cpit_solution(references_dir.join("marvin_cpit_gmunoz120723.sol"), &model)
            .expect("cpit solution should load");

    let summary = summarize_marvin_schedule_solution(&problem, &solution)
        .expect("cpit summary should compute");

    assert_eq!(summary.unique_block_count, 8516);
    assert_eq!(summary.used_period_count, 16);
    assert_eq!(summary.used_destination_count, 1);
    assert!(
        (summary.discounted_objective - 820_726_048.0_f64).abs() < 1.0,
        "discounted CPIT objective must match official target 820,726,048 (got {})",
        summary.discounted_objective
    );
    assert!(
        summary
            .resource_summaries
            .iter()
            .all(|resource| resource.max_period_excess <= 1e-6),
        "CPIT reference solution should respect resource limits"
    );
}

#[test]
fn exact_upl_solver_matches_marvin_upit_membership() {
    let dir = marvin_dir();
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let reference_prec = read_marvin_precedence_graph(references_dir.join("marvin.prec"), &model)
        .expect("marvin.prec should normalize");
    let reference_upit = read_marvin_upit_solution(references_dir.join("marvin_upit.sol"), &model)
        .expect("marvin_upit.sol should normalize");
    let block_weights = read_marvin_upit_block_values(references_dir.join("marvin.upit"), &model)
        .expect("marvin.upit should normalize")
        .into_iter()
        .collect::<BTreeMap<_, _>>();

    let closure_graph = build_max_closure_graph(&block_weights, &reference_prec)
        .expect("closure graph should build");
    let result = solve_upl_exact(&closure_graph).expect("exact UPL solver should succeed");
    let comparison = compare_block_memberships(&reference_upit, &result.selected_blocks);

    assert_eq!(
        comparison.jaccard_index, 1.0,
        "exact max-closure should reproduce Marvin UPIT membership exactly"
    );
    assert_eq!(comparison.reference_only_blocks.len(), 0);
    assert_eq!(comparison.candidate_only_blocks.len(), 0);
    assert!(
        (result.pit_value - 1_415_655_436.0_f64).abs() < 1.0,
        "exact UPL pit value should match official Marvin UPIT objective (got {})",
        result.pit_value
    );
}

#[test]
fn read_marvin_pcpsp_reference_matches_official_discounted_objective() {
    let dir = marvin_dir();
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let problem = read_marvin_pcpsp_problem(references_dir.join("marvin.pcpsp"), &model)
        .expect("pcpsp should load");
    let solution =
        read_marvin_pcpsp_solution(references_dir.join("marvin_pcpsp_gmunoz120723.sol"), &model)
            .expect("pcpsp solution should load");

    let summary = summarize_marvin_schedule_solution(&problem, &solution)
        .expect("pcpsp summary should compute");

    assert_eq!(summary.unique_block_count, 8516);
    assert_eq!(summary.used_period_count, 14);
    assert_eq!(summary.used_destination_count, 2);
    assert!(
        (summary.discounted_objective - 885_968_070.0_f64).abs() < 10.0,
        "discounted PCPSP objective must match official target 885,968,070 (got {})",
        summary.discounted_objective
    );
    assert!(
        summary
            .resource_summaries
            .iter()
            .all(|resource| resource.max_period_excess <= 0.1),
        "PCPSP reference solution should exceed limits by at most rounding noise"
    );
}

#[test]
fn read_marvin_lp_cpit_reference_preserves_fractional_assignments() {
    let dir = marvin_dir();
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let problem = read_marvin_cpit_problem(references_dir.join("marvin.cpit"), &model)
        .expect("cpit should load");
    let solution = read_marvin_lp_cpit_solution(references_dir.join("marvin.LPcpit"), &model)
        .expect("lp cpit should load");

    let summary = summarize_marvin_schedule_solution(&problem, &solution)
        .expect("lp cpit summary should compute");

    assert!(summary.fractional_assignment_count > 0);
    assert!(
        (summary.discounted_objective - 863_915_586.9532448_f64).abs() < 1e-6,
        "discounted LP-CPIT objective must match the normalized local file sum (got {})",
        summary.discounted_objective
    );
}

#[test]
fn read_marvin_lp_pcpsp_reference_preserves_fractional_assignments() {
    let dir = marvin_dir();
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let problem = read_marvin_pcpsp_problem(references_dir.join("marvin.pcpsp"), &model)
        .expect("pcpsp should load");
    let solution = read_marvin_lp_pcpsp_solution(references_dir.join("marvin.LPpcpsp"), &model)
        .expect("lp pcpsp should load");

    let summary = summarize_marvin_schedule_solution(&problem, &solution)
        .expect("lp pcpsp summary should compute");

    assert!(summary.fractional_assignment_count > 0);
    assert!(
        (summary.discounted_objective - 911_699_907.9443411_f64).abs() < 1e-6,
        "discounted LP-PCPSP objective must match the normalized local file sum (got {})",
        summary.discounted_objective
    );
}

/// MR-210: el sweep anidado por restricción monótona debe reproducir
/// exactamente las membresías del sweep naive sobre Marvin y reducir el
/// runtime. Usa escenarios revenue-scaled monótonos construidos desde los
/// valores objetivo abiertos de `marvin.upit` (solo el componente positivo
/// escala con el factor).
#[test]
fn marvin_monotone_nested_shells_match_naive_sweep_and_run_faster() {
    let dir = marvin_dir();
    let references_dir = marvin_references_dir();
    let model = read_benchmark_blocks(dir.join("marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let precedence_graph = read_marvin_precedence_graph(references_dir.join("marvin.prec"), &model)
        .expect("marvin.prec should normalize");
    let upit_values = read_marvin_upit_block_values(references_dir.join("marvin.upit"), &model)
        .expect("marvin.upit should normalize");

    let base_weights: BTreeMap<usize, f64> = upit_values.into_iter().collect();
    let factors = mine_planning::uniform_revenue_factors(7).expect("factors should build");
    let scenarios: Vec<(f64, BTreeMap<usize, f64>)> = factors
        .iter()
        .map(|factor| {
            (
                *factor,
                base_weights
                    .iter()
                    .map(|(linear, weight)| (*linear, factor * weight.max(0.0) + weight.min(0.0)))
                    .collect(),
            )
        })
        .collect();

    let naive_started = std::time::Instant::now();
    let naive =
        mine_planning::generate_nested_shells_from_weight_scenarios(&scenarios, &precedence_graph)
            .expect("naive sweep should succeed");
    let naive_elapsed = naive_started.elapsed();

    let nested_started = std::time::Instant::now();
    let nested = mine_planning::generate_nested_shells_from_monotone_weight_scenarios(
        &scenarios,
        &precedence_graph,
    )
    .expect("monotone sweep should succeed");
    let nested_elapsed = nested_started.elapsed();

    assert_eq!(
        naive, nested,
        "memberships must match the naive sweep exactly"
    );
    assert!(
        nested.unique_shell_count >= 3,
        "marvin revenue-scaled sweep should produce several distinct shells, got {}",
        nested.unique_shell_count
    );
    // Evidencia de runtime (los valores absolutos dependen de la máquina; la
    // mejora relativa debe sostenerse en cualquier hardware).
    println!(
        "[mr210] marvin 7-factor sweep: naive={:?} nested={:?} speedup={:.2}x shells={}",
        naive_elapsed,
        nested_elapsed,
        naive_elapsed.as_secs_f64() / nested_elapsed.as_secs_f64().max(1e-9),
        nested.unique_shell_count
    );
    assert!(
        nested_elapsed < naive_elapsed,
        "monotone sweep should not be slower than the naive sweep (naive {naive_elapsed:?}, nested {nested_elapsed:?})"
    );
}
