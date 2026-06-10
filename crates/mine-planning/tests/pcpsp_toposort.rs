//! Tests de integración de la heurística PCPSP TopoSort multi-destino (MR-212).
//!
//! Referencias: Chicoisne et al. (2012), doi 10.1287/opre.1120.1072 ([R35]);
//! Espinoza et al. (2013), doi 10.1007/s10479-012-1258-3 ([R29]).

use std::collections::BTreeMap;

use mine_planning::{
    PcpspToposortOptions, PcpspToposortProblem, PrecedenceEdge, PrecedenceGraph, PrecedenceNode,
    solve_pcpsp_with_toposort,
};

fn block_edge(pred: usize, succ: usize) -> PrecedenceEdge {
    PrecedenceEdge::new(PrecedenceNode::Block(pred), PrecedenceNode::Block(succ))
}

/// Problema con 2 destinos (0 = mill, 1 = waste) y 2 recursos
/// (0 = mina para ambos destinos, 1 = planta solo para mill).
fn two_destination_problem(
    period_count: usize,
    mine_limit: f64,
    plant_limit: f64,
    blocks: &[(usize, f64, f64)], // (linear, valor mill, valor waste)
) -> PcpspToposortProblem {
    PcpspToposortProblem {
        period_count,
        discount_rate: 0.1,
        destination_count: 2,
        resource_count: 2,
        block_values: blocks
            .iter()
            .map(|(linear, mill, waste)| (*linear, vec![*mill, *waste]))
            .collect(),
        block_resource_usage: blocks
            .iter()
            .map(|(linear, _, _)| (*linear, vec![vec![1.0, 1.0], vec![1.0, 0.0]]))
            .collect(),
        period_resource_upper_limits: vec![vec![Some(mine_limit), Some(plant_limit)]; period_count],
    }
}

#[test]
fn chooses_highest_value_destination_when_capacity_allows() {
    let problem = two_destination_problem(2, 10.0, 10.0, &[(0, 20.0, -1.0)]);
    let graph = PrecedenceGraph::new(vec![]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0)]);

    let schedule =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(schedule.scheduled_block_count, 1);
    assert_eq!(schedule.assignments[0].destination_index, 0);
    assert_eq!(schedule.assignments[0].period_index, 0);
}

#[test]
fn ore_waits_for_plant_capacity_instead_of_wasting_value() {
    // Planta de 1.0 por periodo: dos bloques de mineral valioso; el segundo
    // debe ir a mill en el periodo 1, no a waste en el periodo 0.
    let problem = two_destination_problem(3, 10.0, 1.0, &[(0, 20.0, -1.0), (1, 18.0, -1.0)]);
    let graph = PrecedenceGraph::new(vec![]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0)]);

    let schedule =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");

    let by_block: BTreeMap<usize, (usize, usize)> = schedule
        .assignments
        .iter()
        .map(|a| (a.linear_index, (a.destination_index, a.period_index)))
        .collect();
    assert_eq!(by_block[&0], (0, 0)); // mill en periodo 0
    assert_eq!(by_block[&1], (0, 1)); // mill en periodo 1 (espera capacidad)
    assert_eq!(schedule.used_destination_count, 1);
}

#[test]
fn negative_blocks_are_delayed_when_possible() {
    // Bloque waste (negativo en ambos destinos) sin sucesores: debe terminar
    // en el último periodo tras el post-pass.
    let problem = two_destination_problem(3, 10.0, 10.0, &[(0, -5.0, -2.0), (1, 30.0, -1.0)]);
    let graph = PrecedenceGraph::new(vec![]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0)]);

    let schedule =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");

    let by_block: BTreeMap<usize, (usize, usize)> = schedule
        .assignments
        .iter()
        .map(|a| (a.linear_index, (a.destination_index, a.period_index)))
        .collect();
    // El bloque 0 elige waste (-2.0 > -5.0) y se retrasa al último periodo.
    assert_eq!(by_block[&0].0, 1);
    assert_eq!(by_block[&0].1, 2);
    assert!(schedule.delayed_negative_block_count >= 1);
}

#[test]
fn delayed_waste_respects_successor_periods() {
    // El bloque 0 (waste, predecesor de 1) no puede retrasarse más allá del
    // periodo de su sucesor.
    let problem = two_destination_problem(4, 1.0, 1.0, &[(0, -5.0, -2.0), (1, 30.0, -1.0)]);
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0)]);

    let schedule =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");

    let by_block: BTreeMap<usize, (usize, usize)> = schedule
        .assignments
        .iter()
        .map(|a| (a.linear_index, (a.destination_index, a.period_index)))
        .collect();
    assert!(by_block[&0].1 <= by_block[&1].1);
}

#[test]
fn capacity_exhaustion_drops_block_and_descendants() {
    // Mina de 1.0 por periodo y 1 solo periodo: el segundo bloque no cabe en
    // ningún destino y su sucesor cae en cascada.
    let problem = two_destination_problem(
        1,
        1.0,
        10.0,
        &[(0, 20.0, -1.0), (1, 15.0, -1.0), (2, 10.0, -1.0)],
    );
    let graph = PrecedenceGraph::new(vec![block_edge(1, 2)]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 2.0), (2usize, 3.0)]);

    let schedule =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(schedule.scheduled_block_count, 1);
    assert_eq!(schedule.dropped_for_capacity_count, 1);
    assert_eq!(schedule.dropped_for_predecessor_count, 1);
}

#[test]
fn rejects_mismatched_destination_dimensions() {
    let problem = PcpspToposortProblem {
        period_count: 1,
        discount_rate: 0.1,
        destination_count: 2,
        resource_count: 1,
        block_values: BTreeMap::from([(0usize, vec![1.0])]), // solo 1 destino declarado
        block_resource_usage: BTreeMap::new(),
        period_resource_upper_limits: vec![vec![None]],
    };
    let graph = PrecedenceGraph::new(vec![]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0)]);

    assert!(
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .is_err()
    );
}

#[test]
fn solver_is_deterministic() {
    let problem = two_destination_problem(
        3,
        2.0,
        1.0,
        &[
            (0, 5.0, -1.0),
            (1, -2.0, -0.5),
            (2, 9.0, -1.0),
            (3, 4.0, -1.0),
        ],
    );
    let graph = PrecedenceGraph::new(vec![block_edge(1, 2), block_edge(0, 3)])
        .expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 2.0), (1usize, 1.0), (2usize, 1.5), (3usize, 2.5)]);

    let first =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");
    let second =
        solve_pcpsp_with_toposort(&problem, &graph, &scores, &PcpspToposortOptions::default())
            .expect("solver should succeed");

    assert_eq!(first, second);
}
