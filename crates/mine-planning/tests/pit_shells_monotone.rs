//! Tests de integración del sweep anidado por restricción monótona (MR-210).
//!
//! La ruta optimizada debe producir EXACTAMENTE las mismas membresías que el
//! sweep naive factor por factor cuando los escenarios son monótonos por
//! bloque (Topkis 1978, doi 10.1287/opre.26.2.305; Gallo-Grigoriadis-Tarjan
//! 1989, doi 10.1137/0218003).

use std::collections::BTreeMap;

use mine_planning::{
    PrecedenceEdge, PrecedenceGraph, PrecedenceNode,
    generate_nested_shells_from_monotone_weight_scenarios,
    generate_nested_shells_from_weight_scenarios,
};

fn block_edge(pred: usize, succ: usize) -> PrecedenceEdge {
    PrecedenceEdge::new(PrecedenceNode::Block(pred), PrecedenceNode::Block(succ))
}

/// Escenarios revenue/cost-aware: `w_b(λ) = λ·max(base, 0) + min(base, 0)`.
/// Solo el componente de ingreso escala con el factor → monótono por bloque.
fn revenue_scaled_scenarios(
    base_weights: &BTreeMap<usize, f64>,
    factors: &[f64],
) -> Vec<(f64, BTreeMap<usize, f64>)> {
    factors
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
        .collect()
}

/// Yacimiento sintético con tres cadenas estéril → mineral de breakeven
/// distinto: el shell crece por etapas con el factor (λ ≈ 0.1, 0.5 y 0.9).
fn staged_breakeven_fixture() -> (BTreeMap<usize, f64>, PrecedenceGraph) {
    let weights = BTreeMap::from([
        // Cadena A: estéril barato (−1) sobre mineral 10 → rentable desde λ ≈ 0.1.
        (0usize, -1.0),
        (1usize, 10.0),
        // Cadena B: estéril medio (−5) sobre mineral 10 → rentable desde λ ≈ 0.5.
        (2usize, -5.0),
        (3usize, 10.0),
        // Cadena C: estéril caro (−9) sobre mineral 10 → rentable desde λ ≈ 0.9.
        (4usize, -9.0),
        (5usize, 10.0),
    ]);
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1), block_edge(2, 3), block_edge(4, 5)])
        .expect("staged graph should be valid");
    (weights, graph)
}

#[test]
fn monotone_sweep_matches_naive_sweep_exactly() {
    let (weights, graph) = staged_breakeven_fixture();
    let factors = [0.05, 0.15, 0.3, 0.5, 0.7, 0.85, 1.0];
    let scenarios = revenue_scaled_scenarios(&weights, &factors);

    let naive = generate_nested_shells_from_weight_scenarios(&scenarios, &graph)
        .expect("naive sweep should succeed");
    let nested = generate_nested_shells_from_monotone_weight_scenarios(&scenarios, &graph)
        .expect("monotone sweep should succeed");

    assert_eq!(naive, nested);
    // El fixture debe ejercitar shells realmente distintos para que la
    // igualdad no sea trivial.
    assert!(
        nested.unique_shell_count >= 3,
        "fixture should produce at least 3 distinct shells, got {}",
        nested.unique_shell_count
    );
}

#[test]
fn monotone_sweep_handles_empty_smaller_shells() {
    // Con factores muy pequeños nada es rentable: el shell menor queda vacío y
    // los factores aún menores se sintetizan sin resolver.
    let weights = BTreeMap::from([(0usize, 5.0), (1usize, -20.0), (2usize, 30.0)]);
    let graph = PrecedenceGraph::new(vec![block_edge(0, 2), block_edge(1, 2)])
        .expect("graph should be valid");
    // Nota: bloque 0 aislado positivo siempre entra salvo factor que lo anule;
    // usamos pesos donde TODO el valor viene de bloques bloqueados por estéril
    // caro para forzar shells vacíos en factores chicos.
    let blocked_weights = BTreeMap::from([(0usize, -10.0), (1usize, -20.0), (2usize, 30.0)]);
    let factors = [0.1, 0.2, 0.9, 1.0];

    for base in [&weights, &blocked_weights] {
        let scenarios = revenue_scaled_scenarios(base, &factors);
        let naive = generate_nested_shells_from_weight_scenarios(&scenarios, &graph)
            .expect("naive sweep should succeed");
        let nested = generate_nested_shells_from_monotone_weight_scenarios(&scenarios, &graph)
            .expect("monotone sweep should succeed");
        assert_eq!(naive, nested);
    }
}

#[test]
fn monotone_sweep_preserves_sparse_linear_indices() {
    let weights = BTreeMap::from([(7usize, 12.0), (42usize, -3.0), (99usize, 4.0)]);
    let graph = PrecedenceGraph::from_nodes_and_edges(
        vec![
            PrecedenceNode::Block(7),
            PrecedenceNode::Block(42),
            PrecedenceNode::Block(99),
        ],
        vec![block_edge(42, 7)],
    )
    .expect("sparse graph should be valid");
    let factors = [0.1, 0.5, 1.0];
    let scenarios = revenue_scaled_scenarios(&weights, &factors);

    let naive = generate_nested_shells_from_weight_scenarios(&scenarios, &graph)
        .expect("naive sweep should succeed");
    let nested = generate_nested_shells_from_monotone_weight_scenarios(&scenarios, &graph)
        .expect("monotone sweep should succeed");

    assert_eq!(naive, nested);
    for shell in &nested.shells {
        for block in &shell.selected_blocks {
            assert!([7usize, 42, 99].contains(block));
        }
    }
}

#[test]
fn monotone_sweep_rejects_non_monotone_scenarios() {
    // El bloque 0 empeora al subir el factor → anidamiento no garantizado.
    let graph = PrecedenceGraph::from_nodes_and_edges(vec![PrecedenceNode::Block(0)], vec![])
        .expect("single-node graph should be valid");
    let scenarios = vec![
        (0.5, BTreeMap::from([(0usize, 5.0)])),
        (1.0, BTreeMap::from([(0usize, 3.0)])),
    ];

    let error = generate_nested_shells_from_monotone_weight_scenarios(&scenarios, &graph)
        .expect_err("non-monotone scenarios should be rejected");
    let message = error.to_string();
    assert!(message.contains("monotonicity"));
    assert!(message.contains("generate_nested_shells_from_weight_scenarios"));
}

#[test]
fn monotone_sweep_is_deterministic() {
    let (weights, graph) = staged_breakeven_fixture();
    let factors = [0.2, 0.6, 1.0];
    let scenarios = revenue_scaled_scenarios(&weights, &factors);

    let first = generate_nested_shells_from_monotone_weight_scenarios(&scenarios, &graph)
        .expect("monotone sweep should succeed");
    let second = generate_nested_shells_from_monotone_weight_scenarios(&scenarios, &graph)
        .expect("monotone sweep should succeed");

    assert_eq!(first, second);
}
