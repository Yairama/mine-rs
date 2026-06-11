//! Tests de integración del bound Lagrangiano CPIT/PCPSP (MR-213).
//!
//! Validez: para cualquier multiplicador `π >= 0`, `L(π)` debe acotar por
//! arriba el óptimo entero (Geoffrion 1974, doi 10.1007/BFb0120690).

use std::collections::BTreeMap;

use mine_planning::{
    LagrangianBoundOptions, PcpspToposortProblem, PrecedenceEdge, PrecedenceGraph, PrecedenceNode,
    compute_pcpsp_lagrangian_bound,
};

fn block_edge(pred: usize, succ: usize) -> PrecedenceEdge {
    PrecedenceEdge::new(PrecedenceNode::Block(pred), PrecedenceNode::Block(succ))
}

/// Problema CPIT pequeño (1 destino, 1 recurso) cuyo óptimo entero puede
/// enumerarse a mano:
///
/// - Bloques: 0 (+10, 1.0t), 1 (+8, 1.0t), 2 (−2, 1.0t), 3 (+6, 1.0t).
/// - Precedencia: 2 → 3 (el mineral 3 requiere el estéril 2).
/// - 2 periodos, capacidad mina 2.0t por periodo, descuento 0.1.
///
/// Óptimo entero: P0 = {0, 1} (18); P1 = {2, 3} ((−2+6)/1.1 = 3.636...) →
/// objetivo = 21.6363...
fn enumerable_problem() -> (PcpspToposortProblem, PrecedenceGraph, f64) {
    let problem = PcpspToposortProblem {
        period_count: 2,
        discount_rate: 0.1,
        destination_count: 1,
        resource_count: 1,
        block_values: BTreeMap::from([
            (0usize, vec![10.0]),
            (1usize, vec![8.0]),
            (2usize, vec![-2.0]),
            (3usize, vec![6.0]),
        ]),
        block_resource_usage: BTreeMap::from([
            (0usize, vec![vec![1.0]]),
            (1usize, vec![vec![1.0]]),
            (2usize, vec![vec![1.0]]),
            (3usize, vec![vec![1.0]]),
        ]),
        period_resource_upper_limits: vec![vec![Some(2.0)]; 2],
    };
    let graph = PrecedenceGraph::new(vec![block_edge(2, 3)]).expect("graph should be valid");
    let integer_optimum = 18.0 + (-2.0 + 6.0) / 1.1;
    (problem, graph, integer_optimum)
}

#[test]
fn bound_is_valid_upper_bound_for_integer_optimum() {
    let (problem, graph, integer_optimum) = enumerable_problem();

    let result =
        compute_pcpsp_lagrangian_bound(&problem, &graph, &LagrangianBoundOptions::default())
            .expect("bound should compute");

    assert!(
        result.best_bound >= integer_optimum - 1.0e-9,
        "Lagrangian bound {} must dominate the integer optimum {}",
        result.best_bound,
        integer_optimum
    );
    // Cada iteración individual también debe ser un bound válido.
    for record in &result.iteration_records {
        assert!(
            record.bound >= integer_optimum - 1.0e-9,
            "iteration {} bound {} fell below the integer optimum {}",
            record.iteration,
            record.bound,
            integer_optimum
        );
    }
}

#[test]
fn subgradient_improves_over_unconstrained_iteration_zero() {
    let (problem, graph, _) = enumerable_problem();

    let result = compute_pcpsp_lagrangian_bound(
        &problem,
        &graph,
        &LagrangianBoundOptions {
            max_iterations: 40,
            lower_bound_hint: Some(18.0 + (-2.0 + 6.0) / 1.1),
            ..LagrangianBoundOptions::default()
        },
    )
    .expect("bound should compute");

    // π = 0 en la iteración 0: bound sin capacidades (todos los positivos en
    // el periodo 0) = 10 + 8 + (−2 + 6) = 22.0.
    let initial_bound = result.iteration_records[0].bound;
    assert!((initial_bound - 22.0).abs() < 1.0e-9);
    assert!(
        result.best_bound < initial_bound,
        "subgradient should tighten the bound below the unconstrained value \
         (best {}, initial {})",
        result.best_bound,
        initial_bound
    );
}

#[test]
fn multi_destination_inner_choice_prefers_adjusted_value() {
    // 2 destinos: mill (valor 10, usa planta) y waste (valor −1, no usa
    // planta). Sin capacidad de planta el interior elige mill; el bound con
    // π = 0 debe reflejar el valor mill descontado al periodo 0.
    let problem = PcpspToposortProblem {
        period_count: 2,
        discount_rate: 0.1,
        destination_count: 2,
        resource_count: 1,
        block_values: BTreeMap::from([(0usize, vec![10.0, -1.0])]),
        block_resource_usage: BTreeMap::from([(0usize, vec![vec![1.0], vec![0.0]])]),
        period_resource_upper_limits: vec![vec![Some(5.0)]; 2],
    };
    let graph = PrecedenceGraph::from_nodes_and_edges(vec![PrecedenceNode::Block(0)], vec![])
        .expect("single-node graph should be valid");

    let result = compute_pcpsp_lagrangian_bound(
        &problem,
        &graph,
        &LagrangianBoundOptions {
            max_iterations: 1,
            ..LagrangianBoundOptions::default()
        },
    )
    .expect("bound should compute");

    assert!((result.best_bound - 10.0).abs() < 1.0e-9);
    assert_eq!(result.best_inner_assignments.len(), 1);
    assert_eq!(result.best_inner_assignments[0].destination_index, 0);
    assert_eq!(result.best_inner_assignments[0].period_index, 0);
}

#[test]
fn precedence_coverage_counts_all_expanded_arcs() {
    let (problem, graph, _) = enumerable_problem();
    let result = compute_pcpsp_lagrangian_bound(
        &problem,
        &graph,
        &LagrangianBoundOptions {
            max_iterations: 1,
            ..LagrangianBoundOptions::default()
        },
    )
    .expect("bound should compute");

    // 1 arista de precedencia × 2 periodos.
    assert_eq!(result.expanded_precedence_arc_count, 2);
    assert_eq!(result.expanded_node_count, 8); // 4 bloques × 2 periodos
    assert_eq!(result.multiplier_count, 2); // 1 recurso acotado × 2 periodos
}

#[test]
fn bound_is_deterministic() {
    let (problem, graph, _) = enumerable_problem();
    let options = LagrangianBoundOptions {
        max_iterations: 15,
        lower_bound_hint: Some(20.0),
        ..LagrangianBoundOptions::default()
    };

    let first =
        compute_pcpsp_lagrangian_bound(&problem, &graph, &options).expect("bound should compute");
    let second =
        compute_pcpsp_lagrangian_bound(&problem, &graph, &options).expect("bound should compute");

    assert_eq!(first, second);
}

#[test]
fn rejects_zero_iterations() {
    let (problem, graph, _) = enumerable_problem();
    let error = compute_pcpsp_lagrangian_bound(
        &problem,
        &graph,
        &LagrangianBoundOptions {
            max_iterations: 0,
            ..LagrangianBoundOptions::default()
        },
    )
    .expect_err("zero iterations should fail");
    assert!(error.to_string().contains("max_iterations"));
}
