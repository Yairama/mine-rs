//! Tests de integración de la heurística CPIT TopoSort (MR-211).
//!
//! Método de referencia: Chicoisne et al. (2012), Operations Research
//! 60(3):517-528, doi 10.1287/opre.1120.1072 ([R35]).

use std::collections::BTreeMap;

use mine_planning::{
    CpitToposortOptions, CpitToposortProblem, PrecedenceEdge, PrecedenceGraph, PrecedenceNode,
    solve_cpit_with_toposort,
};

fn block_edge(pred: usize, succ: usize) -> PrecedenceEdge {
    PrecedenceEdge::new(PrecedenceNode::Block(pred), PrecedenceNode::Block(succ))
}

fn single_resource_problem(
    period_count: usize,
    per_period_limit: f64,
    values: &[(usize, f64)],
    usage: &[(usize, f64)],
) -> CpitToposortProblem {
    CpitToposortProblem {
        period_count,
        discount_rate: 0.1,
        resource_count: 1,
        block_values: values.iter().copied().collect(),
        block_resource_usage: usage
            .iter()
            .map(|(linear, amount)| (*linear, vec![*amount]))
            .collect(),
        period_resource_upper_limits: vec![vec![Some(per_period_limit)]; period_count],
    }
}

#[test]
fn capacity_splits_blocks_across_periods_in_score_order() {
    // 3 bloques de 1.0t cada uno, capacidad 2.0t por periodo.
    let problem = single_resource_problem(
        2,
        2.0,
        &[(0, 10.0), (1, 8.0), (2, 6.0)],
        &[(0, 1.0), (1, 1.0), (2, 1.0)],
    );
    let graph = PrecedenceGraph::new(vec![]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0), (2usize, 3.0)]);

    let schedule =
        solve_cpit_with_toposort(&problem, &graph, &scores, &CpitToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(schedule.scheduled_block_count, 3);
    // Bloques 0 y 1 (mejor score) entran al periodo 0; bloque 2 al periodo 1.
    let by_block: BTreeMap<usize, usize> = schedule
        .assignments
        .iter()
        .map(|a| (a.linear_index, a.period_index))
        .collect();
    assert_eq!(by_block[&0], 0);
    assert_eq!(by_block[&1], 0);
    assert_eq!(by_block[&2], 1);
    assert_eq!(schedule.used_period_count, 2);
}

#[test]
fn successor_never_precedes_predecessor_period() {
    // Bloque 1 requiere bloque 0; capacidad fuerza 0 → periodo 0, 1 → periodo 1.
    let problem = single_resource_problem(3, 1.0, &[(0, -2.0), (1, 10.0)], &[(0, 1.0), (1, 1.0)]);
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    // El score intenta poner el bloque 1 primero, pero la topología manda.
    let scores = BTreeMap::from([(0usize, 5.0), (1usize, 1.0)]);

    let schedule = solve_cpit_with_toposort(
        &problem,
        &graph,
        &scores,
        &CpitToposortOptions {
            delay_negative_blocks: false,
        },
    )
    .expect("solver should succeed");

    let by_block: BTreeMap<usize, usize> = schedule
        .assignments
        .iter()
        .map(|a| (a.linear_index, a.period_index))
        .collect();
    assert!(by_block[&1] >= by_block[&0]);
}

#[test]
fn missing_predecessor_drops_block_and_descendants() {
    // Soporte: {1, 2}; el bloque 0 (predecesor de 1) queda fuera del soporte
    // y la cadena 0→1→2 colapsa completa.
    let problem = single_resource_problem(
        2,
        10.0,
        &[(0, 1.0), (1, 5.0), (2, 5.0)],
        &[(0, 1.0), (1, 1.0), (2, 1.0)],
    );
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1), block_edge(1, 2)])
        .expect("graph should be valid");
    let scores = BTreeMap::from([(1usize, 1.0), (2usize, 2.0)]);

    let schedule =
        solve_cpit_with_toposort(&problem, &graph, &scores, &CpitToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(schedule.scheduled_block_count, 0);
    assert_eq!(schedule.dropped_for_predecessor_count, 2);
}

#[test]
fn capacity_drop_cascades_to_successors() {
    // El bloque 0 (3.0t) nunca cabe (límite 2.0t) → el sucesor 1 también cae.
    let problem = single_resource_problem(2, 2.0, &[(0, 1.0), (1, 50.0)], &[(0, 3.0), (1, 1.0)]);
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0)]);

    let schedule =
        solve_cpit_with_toposort(&problem, &graph, &scores, &CpitToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(schedule.scheduled_block_count, 0);
    assert_eq!(schedule.dropped_for_capacity_count, 1);
    assert_eq!(schedule.dropped_for_predecessor_count, 1);
}

#[test]
fn delay_pass_moves_negative_blocks_later_and_improves_npv() {
    // Bloque 0 negativo con sucesor: solo puede llegar hasta el periodo del
    // sucesor. Bloque 2 negativo sin sucesores: puede ir al último periodo.
    let problem = single_resource_problem(
        3,
        10.0,
        &[(0, -5.0), (1, 20.0), (2, -4.0)],
        &[(0, 1.0), (1, 1.0), (2, 1.0)],
    );
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0), (2usize, 3.0)]);

    let baseline = solve_cpit_with_toposort(
        &problem,
        &graph,
        &scores,
        &CpitToposortOptions {
            delay_negative_blocks: false,
        },
    )
    .expect("solver should succeed");
    let improved = solve_cpit_with_toposort(
        &problem,
        &graph,
        &scores,
        &CpitToposortOptions {
            delay_negative_blocks: true,
        },
    )
    .expect("solver should succeed");

    // El bloque 2 (negativo, sin sucesores) se mueve al último periodo.
    let by_block: BTreeMap<usize, usize> = improved
        .assignments
        .iter()
        .map(|a| (a.linear_index, a.period_index))
        .collect();
    assert_eq!(by_block[&2], 2);
    // El bloque 0 no puede pasar del periodo del bloque 1.
    assert!(by_block[&0] <= by_block[&1]);
    assert!(improved.discounted_objective > baseline.discounted_objective);
    assert!(improved.delayed_negative_block_count >= 1);
}

#[test]
fn rejects_score_for_unknown_block() {
    let problem = single_resource_problem(1, 10.0, &[(0, 1.0)], &[(0, 1.0)]);
    let graph = PrecedenceGraph::new(vec![]).expect("graph should be valid");
    let scores = BTreeMap::from([(99usize, 1.0)]);

    let error =
        solve_cpit_with_toposort(&problem, &graph, &scores, &CpitToposortOptions::default())
            .expect_err("unknown block score should fail");
    assert!(error.to_string().contains("99"));
}

#[test]
fn solver_is_deterministic() {
    let problem = single_resource_problem(
        3,
        2.0,
        &[(0, 3.0), (1, -1.0), (2, 7.0), (3, 2.0)],
        &[(0, 1.0), (1, 1.0), (2, 1.0), (3, 1.0)],
    );
    let graph = PrecedenceGraph::new(vec![block_edge(1, 2), block_edge(0, 3)])
        .expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 2.0), (1usize, 1.0), (2usize, 1.5), (3usize, 2.5)]);

    let first =
        solve_cpit_with_toposort(&problem, &graph, &scores, &CpitToposortOptions::default())
            .expect("solver should succeed");
    let second =
        solve_cpit_with_toposort(&problem, &graph, &scores, &CpitToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(first, second);
}
