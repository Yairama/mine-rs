//! Tests de integración para los flujos públicos de `mine-economics`.

use std::collections::BTreeMap;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MineError, ModelId,
    ScenarioId,
};
use mine_economics::{
    EconomicAssumptions, EconomicUnits, PeriodCashflowInput, evaluate_block_economics,
    evaluate_scenario_cashflow,
};
use mine_planning::{MiningScenario, ScenarioConstraints, ScenarioPeriod, ScenarioRules};

#[test]
fn reject_invalid_economic_assumptions() {
    let error = EconomicAssumptions::new(
        0.0,
        5.0,
        2.0,
        1.0,
        0.9,
        EconomicUnits::new(
            MeasurementUnit::new("%Cu").expect("unit should be valid"),
            MeasurementUnit::new("t").expect("unit should be valid"),
            MeasurementUnit::new("t").expect("unit should be valid"),
        ),
    )
    .expect_err("zero price should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "price_per_recovered_metal_unit",
            "must be finite and greater than zero"
        )
    );
}

#[test]
fn evaluate_revenue_and_margin_per_block() {
    let model = sample_model();
    let assumptions = EconomicAssumptions::new(
        1_000.0,
        50.0,
        2.0,
        1.0,
        0.9,
        EconomicUnits::new(
            MeasurementUnit::new("%Cu").expect("unit should be valid"),
            MeasurementUnit::new("t").expect("unit should be valid"),
            MeasurementUnit::new("t").expect("unit should be valid"),
        ),
    )
    .expect("assumptions should be valid");
    let report = evaluate_block_economics(
        &model,
        &ColumnId::new("cu").expect("column should be valid"),
        &ColumnId::new("tonnes").expect("column should be valid"),
        &assumptions,
    )
    .expect("economics should evaluate");

    assert_eq!(report.blocks.len(), 2);
    assert_close(report.blocks[0].contained_metal, 0.1);
    assert_close(report.blocks[0].recovered_metal, 0.09);
    assert_close(report.blocks[0].revenue, 90.0);
    assert_close(report.blocks[0].total_cost, 34.5);
    assert_close(report.blocks[0].margin, 55.5);
    assert_close(report.blocks[1].revenue, 90.0);
    assert_close(report.blocks[1].total_cost, 64.5);
    assert_close(report.blocks[1].margin, 25.5);
    assert_close(report.total_revenue, 180.0);
    assert_close(report.total_cost, 99.0);
    assert_close(report.total_margin, 81.0);
}

#[test]
fn reject_unit_mismatch_between_model_and_assumptions() {
    let model = sample_model();
    let assumptions = EconomicAssumptions::new(
        1_000.0,
        50.0,
        2.0,
        1.0,
        0.9,
        EconomicUnits::new(
            MeasurementUnit::new("ppm").expect("unit should be valid"),
            MeasurementUnit::new("t").expect("unit should be valid"),
            MeasurementUnit::new("t").expect("unit should be valid"),
        ),
    )
    .expect("assumptions should be valid");

    let error = evaluate_block_economics(
        &model,
        &ColumnId::new("cu").expect("column should be valid"),
        &ColumnId::new("tonnes").expect("column should be valid"),
        &assumptions,
    )
    .expect_err("unit mismatch should fail");

    assert_eq!(
        error,
        MineError::Economics {
            message: "grade column `cu` uses unit `%Cu` but economics expect `ppm`".to_owned(),
        }
    );
}

#[test]
fn evaluate_cashflow_and_npv_for_scenario_periods() {
    let scenario = sample_scenario();
    let report = evaluate_scenario_cashflow(
        &scenario,
        &[
            PeriodCashflowInput::new("P1", 100.0, 40.0).expect("input should be valid"),
            PeriodCashflowInput::new("P2", 120.0, 50.0).expect("input should be valid"),
        ],
        0.1,
    )
    .expect("cashflow should evaluate");

    assert_eq!(report.periods.len(), 2);
    assert_close(report.periods[0].cashflow, 60.0);
    assert_close(report.periods[0].discount_factor, 1.0);
    assert_close(report.periods[0].discounted_cashflow, 60.0);
    assert_close(report.periods[1].cashflow, 70.0);
    assert_close(report.periods[1].discount_factor, 1.0 / 1.1);
    assert_close(report.periods[1].discounted_cashflow, 70.0 / 1.1);
    assert_close(report.total_cashflow, 130.0);
    assert_close(report.npv, 60.0 + (70.0 / 1.1));
}

#[test]
fn reject_missing_scenario_period_in_cashflow_inputs() {
    let scenario = sample_scenario();
    let error = evaluate_scenario_cashflow(
        &scenario,
        &[PeriodCashflowInput::new("P1", 100.0, 40.0).expect("input should be valid")],
        0.1,
    )
    .expect_err("missing period should fail");

    assert_eq!(
        error,
        MineError::Economics {
            message: "scenario period `P2` is missing from cashflow inputs".to_owned(),
        }
    );
}

fn sample_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("cu").expect("column should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
    ])
    .expect("schema should be valid");
    let mut columns = BTreeMap::new();
    columns.insert(
        ColumnId::new("cu").expect("column should be valid"),
        ColumnData::Floats(vec![1.0, 0.5]),
    );
    columns.insert(
        ColumnId::new("tonnes").expect("column should be valid"),
        ColumnData::Floats(vec![10.0, 20.0]),
    );

    BlockModel::new(grid, schema, Metadata::new(), columns).expect("model should be valid")
}

fn sample_scenario() -> MiningScenario {
    MiningScenario::new(
        ScenarioId::new("scenario-01").expect("scenario should be valid"),
        ModelId::new("model-01").expect("model should be valid"),
        vec![
            ScenarioPeriod::new("P1", Some(1_000.0), None).expect("period should be valid"),
            ScenarioPeriod::new("P2", Some(1_200.0), None).expect("period should be valid"),
        ],
        ScenarioRules::default(),
        ScenarioConstraints::default(),
        Metadata::new(),
    )
    .expect("scenario should be valid")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}
