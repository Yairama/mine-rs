#![allow(missing_docs)]

//! Microbenchmarks criterion de operaciones críticas del SDK (MR-216).
//!
//! Cubre las operaciones que dominan los pipelines de planeamiento del repo:
//!
//! 1. indexación `xyz ↔ ijk ↔ lineal`;
//! 2. construcción de precedencias con la plantilla Marvin de 17 offsets
//!    (45°/8 niveles, MR-167);
//! 3. solver exacto de UPL (Dinic, MR-173);
//! 4. generación de shells anidados por revenue factors (MR-157);
//! 5. slice del scheduler `ready frontier` (MR-177);
//! 6. heurística CPIT TopoSort (MR-211, Chicoisne et al. 2012,
//!    doi 10.1287/opre.1120.1072).
//!
//! Los fixtures son sintéticos, deterministas y pequeños a propósito: la
//! escalabilidad sobre instancias MineLib reales se mide con los harnesses
//! `upit_runtime` / `cpit_toposort` (MR-209/MR-215).

use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mine_sdk::{
    BlockDimensions, BlockModel, BlockPrecedenceTemplate, ColumnData, ColumnId, ColumnLogicalType,
    ColumnMiningRole, ColumnSchema, ColumnSchemaSet, Coordinate3D, CpitToposortOptions,
    CpitToposortProblem, GridDefinition, GridIndex, GridShape, Metadata, ModelId, PrecedenceOffset,
    ScenarioId, SchedulingObjectiveTerm, SchedulingPeriod, SchedulingProblem,
    SchedulingResourceBound, SchedulingResourceId, SchedulingResourceRequirement, SchedulingUnit,
    SchedulingUnitId, build_block_precedence_graph, build_max_closure_graph,
    build_ready_frontier_schedule, generate_nested_shells_from_monotone_weight_scenarios,
    generate_nested_shells_from_weight_map, generate_nested_shells_from_weight_scenarios,
    ijk_to_linear, solve_cpit_with_toposort, solve_upl_exact, xyz_to_ijk,
};

const GRID_NX: usize = 20;
const GRID_NY: usize = 20;
const GRID_NZ: usize = 8;
const COORDINATE_TOLERANCE: f64 = 1.0e-9;

fn bench_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(GRID_NX, GRID_NY, GRID_NZ).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid")
}

/// Valor sintético determinista con mezcla de mineral (positivo) y estéril.
fn synthetic_block_value(i: usize, j: usize, k: usize) -> f64 {
    let hash = (i * 31 + j * 17 + k * 7) % 13;
    hash as f64 - 4.0
}

fn bench_block_model() -> BlockModel {
    let grid = bench_grid();
    let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
        ColumnId::new("value").expect("column id should be valid"),
        ColumnLogicalType::Float,
        None,
        false,
        ColumnMiningRole::Other,
    )])
    .expect("schema should be valid");

    let mut values = Vec::with_capacity(GRID_NX * GRID_NY * GRID_NZ);
    for k in 0..GRID_NZ {
        for j in 0..GRID_NY {
            for i in 0..GRID_NX {
                values.push(synthetic_block_value(i, j, k));
            }
        }
    }

    BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([(
            ColumnId::new("value").expect("column id should be valid"),
            ColumnData::Floats(values),
        )]),
    )
    .expect("block model should be valid")
}

/// Plantilla Marvin exacta de 17 offsets (45°/8 niveles, MR-167).
fn marvin_17_offset_template() -> BlockPrecedenceTemplate {
    BlockPrecedenceTemplate::new(vec![
        PrecedenceOffset::new(0, 0, 1).expect("offset should be valid"),
        PrecedenceOffset::new(-1, 0, 1).expect("offset should be valid"),
        PrecedenceOffset::new(1, 0, 1).expect("offset should be valid"),
        PrecedenceOffset::new(0, -1, 1).expect("offset should be valid"),
        PrecedenceOffset::new(0, 1, 1).expect("offset should be valid"),
        PrecedenceOffset::new(-2, -2, 3).expect("offset should be valid"),
        PrecedenceOffset::new(-2, 2, 3).expect("offset should be valid"),
        PrecedenceOffset::new(2, -2, 3).expect("offset should be valid"),
        PrecedenceOffset::new(2, 2, 3).expect("offset should be valid"),
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

fn synthetic_weights(grid: &GridDefinition) -> BTreeMap<usize, f64> {
    let mut weights = BTreeMap::new();
    for k in 0..GRID_NZ {
        for j in 0..GRID_NY {
            for i in 0..GRID_NX {
                let linear =
                    ijk_to_linear(grid, GridIndex::new(i, j, k)).expect("index should be in range");
                weights.insert(linear, synthetic_block_value(i, j, k));
            }
        }
    }
    weights
}

fn bench_indexing(c: &mut Criterion) {
    let grid = bench_grid();
    c.bench_function("indexing/xyz_to_ijk_to_linear_3200_blocks", |b| {
        b.iter(|| {
            let mut accumulator = 0usize;
            for k in 0..GRID_NZ {
                for j in 0..GRID_NY {
                    for i in 0..GRID_NX {
                        let coordinate = Coordinate3D::new(
                            5.0 + 10.0 * i as f64,
                            5.0 + 10.0 * j as f64,
                            5.0 + 10.0 * k as f64,
                        )
                        .expect("coordinate should be valid");
                        let index = xyz_to_ijk(&grid, coordinate, COORDINATE_TOLERANCE)
                            .expect("coordinate should map to grid");
                        accumulator +=
                            ijk_to_linear(&grid, index).expect("index should be in range");
                    }
                }
            }
            black_box(accumulator)
        });
    });
}

fn bench_precedence_build(c: &mut Criterion) {
    let model = bench_block_model();
    let template = marvin_17_offset_template();
    c.bench_function("precedence/marvin_17_offset_template_3200_blocks", |b| {
        b.iter(|| {
            let graph = build_block_precedence_graph(black_box(&model), black_box(&template))
                .expect("precedence graph should build");
            black_box(graph.edges().len())
        });
    });
}

fn bench_upl_solver(c: &mut Criterion) {
    let model = bench_block_model();
    let template = marvin_17_offset_template();
    let graph =
        build_block_precedence_graph(&model, &template).expect("precedence graph should build");
    let weights = synthetic_weights(&bench_grid());
    let closure_graph =
        build_max_closure_graph(&weights, &graph).expect("closure graph should build");

    c.bench_function("upl/solve_upl_exact_dinic_3200_blocks", |b| {
        b.iter(|| {
            let result = solve_upl_exact(black_box(&closure_graph)).expect("solver should succeed");
            black_box(result.selected_block_count)
        });
    });
}

fn bench_nested_shells(c: &mut Criterion) {
    let model = bench_block_model();
    let template = marvin_17_offset_template();
    let graph =
        build_block_precedence_graph(&model, &template).expect("precedence graph should build");
    let weights = synthetic_weights(&bench_grid());
    let factors = vec![0.2, 0.4, 0.6, 0.8, 1.0];

    c.bench_function("shells/nested_shells_5_factors_3200_blocks", |b| {
        b.iter(|| {
            let shells = generate_nested_shells_from_weight_map(
                black_box(&weights),
                black_box(&graph),
                black_box(&factors),
            )
            .expect("shells should generate");
            black_box(shells.shells.len())
        });
    });

    // Escenarios revenue-scaled monótonos (solo el componente positivo escala
    // con el factor) para comparar el sweep naive contra la ruta anidada por
    // restricción monótona (MR-210).
    let monotone_scenarios: Vec<(f64, std::collections::BTreeMap<usize, f64>)> = factors
        .iter()
        .map(|factor| {
            (
                *factor,
                weights
                    .iter()
                    .map(|(linear, weight)| (*linear, factor * weight.max(0.0) + weight.min(0.0)))
                    .collect(),
            )
        })
        .collect();

    c.bench_function("shells/naive_sweep_monotone_scenarios_5_factors", |b| {
        b.iter(|| {
            let shells = generate_nested_shells_from_weight_scenarios(
                black_box(&monotone_scenarios),
                black_box(&graph),
            )
            .expect("naive sweep should generate");
            black_box(shells.shells.len())
        });
    });

    c.bench_function("shells/monotone_restricted_sweep_5_factors", |b| {
        b.iter(|| {
            let shells = generate_nested_shells_from_monotone_weight_scenarios(
                black_box(&monotone_scenarios),
                black_box(&graph),
            )
            .expect("monotone sweep should generate");
            black_box(shells.shells.len())
        });
    });
}

fn bench_scheduling_problem() -> SchedulingProblem {
    let resource_id = SchedulingResourceId::new("mine_tonnage").expect("id should be valid");
    let unit_count = 60usize;
    let period_count = 6usize;

    let periods = (0..period_count)
        .map(|period| {
            SchedulingPeriod::new(
                format!("P{period}"),
                vec![
                    SchedulingResourceBound::new(resource_id.clone(), None, Some(120.0))
                        .expect("bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid")
        })
        .collect::<Vec<_>>();

    let mut units = Vec::with_capacity(unit_count);
    let mut objective_terms = Vec::with_capacity(unit_count);
    let mut resource_requirements = Vec::with_capacity(unit_count);
    for index in 0..unit_count {
        let unit_id = SchedulingUnitId::new(format!("unit-{index:03}")).expect("id valid");
        let predecessors = if index >= 4 {
            vec![SchedulingUnitId::new(format!("unit-{:03}", index - 4)).expect("id valid")]
        } else {
            vec![]
        };
        units.push(
            SchedulingUnit::new(
                unit_id.clone(),
                10.0,
                1,
                predecessors,
                vec![],
                vec![],
                vec![],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        );
        objective_terms.push(
            SchedulingObjectiveTerm::new(unit_id.clone(), None, ((index % 9) as f64) - 2.0)
                .expect("term should be valid"),
        );
        resource_requirements.push(
            SchedulingResourceRequirement::new(unit_id, resource_id.clone(), None, 10.0)
                .expect("requirement should be valid"),
        );
    }

    SchedulingProblem::new(
        ScenarioId::new("bench-scenario").expect("scenario id valid"),
        ModelId::new("bench-model").expect("model id valid"),
        periods,
        units,
        objective_terms,
        resource_requirements,
        vec![],
        vec![],
        0.1,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid")
}

fn bench_ready_frontier(c: &mut Criterion) {
    let problem = bench_scheduling_problem();
    c.bench_function("scheduling/ready_frontier_60_units_6_periods", |b| {
        b.iter(|| {
            let solution = build_ready_frontier_schedule(black_box(&problem))
                .expect("scheduler should succeed");
            black_box(solution.assignments().len())
        });
    });
}

fn bench_cpit_toposort(c: &mut Criterion) {
    let model = bench_block_model();
    let template = marvin_17_offset_template();
    let graph =
        build_block_precedence_graph(&model, &template).expect("precedence graph should build");
    let grid = bench_grid();
    let weights = synthetic_weights(&grid);
    let block_resource_usage: BTreeMap<usize, Vec<f64>> =
        weights.keys().map(|linear| (*linear, vec![1.0])).collect();
    // Score sintético: profundidad primero (k descendente en la grilla → k
    // mayor se extrae antes por la convención de offsets dk > 0).
    let ordering_scores: BTreeMap<usize, f64> = weights
        .keys()
        .map(|linear| {
            let k = linear / (GRID_NX * GRID_NY);
            (*linear, -(k as f64))
        })
        .collect();
    let problem = CpitToposortProblem {
        period_count: 8,
        discount_rate: 0.1,
        resource_count: 1,
        block_values: weights,
        block_resource_usage,
        period_resource_upper_limits: vec![vec![Some(450.0)]; 8],
    };

    c.bench_function("cpit/toposort_3200_blocks_8_periods", |b| {
        b.iter(|| {
            let schedule = solve_cpit_with_toposort(
                black_box(&problem),
                black_box(&graph),
                black_box(&ordering_scores),
                &CpitToposortOptions::default(),
            )
            .expect("toposort should succeed");
            black_box(schedule.scheduled_block_count)
        });
    });
}

fn all_benches(c: &mut Criterion) {
    bench_indexing(c);
    bench_precedence_build(c);
    bench_upl_solver(c);
    bench_nested_shells(c);
    bench_ready_frontier(c);
    bench_cpit_toposort(c);
}

criterion_group!(benches, all_benches);
criterion_main!(benches);
