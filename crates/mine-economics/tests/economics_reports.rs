//! Tests de integración para los flujos públicos de `mine-economics`.

use std::collections::BTreeMap;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MineError, ModelId,
    ScenarioId,
};
use mine_economics::{
    DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
    DestinationKind, DestinationPayability, DestinationRecovery, EconomicAssumptions,
    EconomicBlockModel, EconomicBlockModelConfig, EconomicUnits, LongTermScheduleSensitivityCase,
    PeriodCashflowInput, evaluate_block_economics, evaluate_long_term_schedule_economics,
    evaluate_long_term_schedule_sensitivity_pack, evaluate_scenario_cashflow,
    summarize_long_term_schedule_risk,
};
use mine_planning::{
    LongTermSchedulePeriodCapacity, MiningScenario, NestingAccessRules, PushbackPlan,
    ScenarioConstraints, ScenarioPeriod, ScenarioRules, build_aggregated_long_term_schedule,
};

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

#[test]
fn evaluate_long_term_schedule_period_kpis() {
    let economic_model = sample_economic_block_model();
    let phase_plan = sample_phase_plan();
    let schedule = build_aggregated_long_term_schedule(
        ScenarioId::new("schedule-scenario").expect("scenario should be valid"),
        ModelId::new("schedule-model").expect("model should be valid"),
        &phase_plan,
        vec![
            LongTermSchedulePeriodCapacity::new("P1", Some(10.0), None, vec![], vec![])
                .expect("capacity should be valid"),
            LongTermSchedulePeriodCapacity::new("P2", Some(20.0), None, vec![], vec![])
                .expect("capacity should be valid"),
        ],
        Some(10),
        Metadata::new(),
    )
    .expect("schedule should build");

    let report =
        evaluate_long_term_schedule_economics(&schedule, &phase_plan, &economic_model, 0.1)
            .expect("schedule economics should evaluate");

    assert_eq!(report.periods.len(), 2);
    assert_eq!(report.periods[0].period_label, "P1");
    assert_eq!(report.periods[0].phase_ids, vec!["phase-a".to_owned()]);
    assert_close(report.periods[0].tonnage, 10.0);
    assert_close(report.periods[0].revenue, 720.0);
    assert_close(report.periods[0].cost, 100.0);
    assert_close(report.periods[0].cashflow, 620.0);
    assert_close(report.periods[0].payable_metal["cu"], 7.2);
    assert_close(report.periods[0].destination_tonnage["mill"], 10.0);

    assert_eq!(report.periods[1].period_label, "P2");
    assert_eq!(report.periods[1].phase_ids, vec!["phase-b".to_owned()]);
    assert_close(report.periods[1].tonnage, 20.0);
    assert_close(report.periods[1].revenue, 720.0);
    assert_close(report.periods[1].cost, 200.0);
    assert_close(report.periods[1].cashflow, 520.0);
    assert_close(report.periods[1].discounted_cashflow, 520.0 / 1.1);
    assert_close(report.periods[1].payable_metal["cu"], 7.2);
    assert_close(report.periods[1].destination_tonnage["mill"], 20.0);

    assert_close(report.total_revenue, 1_440.0);
    assert_close(report.total_cost, 300.0);
    assert_close(report.total_cashflow, 1_140.0);
    assert_close(report.npv, 620.0 + (520.0 / 1.1));
    assert_close(report.total_tonnage, 30.0);
    assert_eq!(report.total_block_count, 2);
    assert_close(report.payable_metal["cu"], 14.4);
    assert_close(report.destination_tonnage["mill"], 30.0);
}

#[test]
fn reject_schedule_phase_missing_from_phase_plan_for_economics() {
    let economic_model = sample_economic_block_model();
    let phase_plan = sample_phase_plan();
    let schedule = mine_planning::LongTermSchedule::new(
        ScenarioId::new("schedule-scenario").expect("scenario should be valid"),
        ModelId::new("schedule-model").expect("model should be valid"),
        vec![
            mine_planning::LongTermScheduleEntry::new(
                "P1",
                Some("phase-missing".to_owned()),
                Some(0),
                Some(100),
                30.0,
                2,
                None,
                None,
                Vec::new(),
            )
            .expect("replacement entry should be valid"),
        ],
        vec![
            LongTermSchedulePeriodCapacity::new("P1", Some(30.0), None, vec![], vec![])
                .expect("capacity should be valid"),
        ],
        Vec::new(),
        Vec::new(),
        Metadata::new(),
    )
    .expect("replacement schedule should be valid");

    let error = evaluate_long_term_schedule_economics(&schedule, &phase_plan, &economic_model, 0.1)
        .expect_err("missing phase should fail");

    assert_eq!(
        error,
        MineError::Economics {
            message: "schedule phase `phase-missing` is missing from the pushback plan used for economic evaluation"
                .to_owned(),
        }
    );
}

#[test]
fn evaluate_schedule_sensitivity_pack_reports_deltas() {
    let economic_model = sample_economic_block_model();
    let phase_plan = single_phase_plan();
    let report = evaluate_long_term_schedule_sensitivity_pack(
        ScenarioId::new("schedule-scenario").expect("scenario should be valid"),
        ModelId::new("schedule-model").expect("model should be valid"),
        &phase_plan,
        vec![
            LongTermSchedulePeriodCapacity::new("P1", Some(20.0), None, vec![], vec![])
                .expect("capacity should be valid"),
            LongTermSchedulePeriodCapacity::new("P2", Some(20.0), None, vec![], vec![])
                .expect("capacity should be valid"),
            LongTermSchedulePeriodCapacity::new("P3", Some(20.0), None, vec![], vec![])
                .expect("capacity should be valid"),
        ],
        Some(10),
        &economic_model,
        0.1,
        &[
            LongTermScheduleSensitivityCase::new(
                "higher-price",
                Some(1.1),
                None,
                None,
                None,
                None,
                None,
            )
            .expect("case should be valid"),
            LongTermScheduleSensitivityCase::new(
                "tighter-capacity",
                None,
                None,
                None,
                None,
                Some(0.5),
                None,
            )
            .expect("case should be valid"),
        ],
    )
    .expect("sensitivity pack should evaluate");

    assert_eq!(report.base_case_id, "base");
    assert_eq!(report.comparisons.len(), 2);
    assert_eq!(report.base_report.periods.len(), 3);
    assert_close(report.base_report.periods[0].tonnage, 20.0);
    assert_close(report.base_report.periods[1].tonnage, 10.0);
    assert_close(report.base_report.periods[2].tonnage, 0.0);

    let higher_price = report
        .comparisons
        .iter()
        .find(|comparison| comparison.case_id == "higher-price")
        .expect("higher-price case should exist");
    assert!(higher_price.npv_delta > 0.0);
    assert_close(higher_price.total_tonnage_delta, 0.0);
    assert_eq!(
        higher_price.preferred_case_id.as_deref(),
        Some("higher-price")
    );

    let tighter_capacity = report
        .comparisons
        .iter()
        .find(|comparison| comparison.case_id == "tighter-capacity")
        .expect("tighter-capacity case should exist");
    assert!(tighter_capacity.npv_delta < 0.0);
    assert_close(tighter_capacity.total_tonnage_delta, 0.0);
    assert_eq!(tighter_capacity.preferred_case_id.as_deref(), Some("base"));
    assert_eq!(tighter_capacity.report.periods.len(), 3);
    assert_close(tighter_capacity.report.periods[0].tonnage, 10.0);
    assert_close(tighter_capacity.report.periods[1].tonnage, 10.0);
    assert_close(tighter_capacity.report.periods[2].tonnage, 10.0);

    assert_eq!(tighter_capacity.period_comparisons[0].period_label, "P1");
    assert_close(
        tighter_capacity.period_comparisons[0]
            .tonnage_delta
            .expect("delta should exist"),
        -10.0,
    );
    assert_eq!(tighter_capacity.period_comparisons[2].period_label, "P3");
    assert_close(
        tighter_capacity.period_comparisons[2]
            .tonnage_delta
            .expect("delta should exist"),
        10.0,
    );
}

#[test]
fn reject_invalid_sensitivity_factor() {
    let error =
        LongTermScheduleSensitivityCase::new("invalid", Some(0.0), None, None, None, None, None)
            .expect_err("zero factor should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "price_factor",
            "must be finite and greater than zero when provided"
        )
    );
}

#[test]
fn summarize_risk_metrics_over_schedule_reports() {
    let report = summarize_long_term_schedule_risk(&[
        synthetic_economics_report("base", 100.0, 120.0, 10.0),
        synthetic_economics_report("downside", -50.0, -20.0, 8.0),
        synthetic_economics_report("upside", 250.0, 300.0, 12.0),
        synthetic_economics_report("stress", 75.0, 80.0, 9.0),
        synthetic_economics_report("mid", 150.0, 200.0, 11.0),
    ])
    .expect("risk report should evaluate");

    assert_eq!(report.scenario_ids.len(), 5);
    assert_eq!(report.quantile_method, "nearest-rank");
    assert_eq!(report.npv.sample_count, 5);
    assert_close(report.npv.min, -50.0);
    assert_close(report.npv.max, 250.0);
    assert_close(report.npv.mean, 105.0);
    assert_close(report.npv.p10, -50.0);
    assert_close(report.npv.p50, 100.0);
    assert_close(report.npv.p90, 250.0);
    assert_close(report.npv.downside_probability, 0.2);
    assert_close(report.npv.cvar10, -50.0);

    assert_close(report.total_cashflow.p10, -20.0);
    assert_close(report.total_cashflow.p50, 120.0);
    assert_close(report.total_tonnage.p90, 12.0);
}

#[test]
fn reject_empty_risk_summary_inputs() {
    let error = summarize_long_term_schedule_risk(&[]).expect_err("empty risk inputs should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "reports",
            "risk summary requires at least one economic report"
        )
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

fn sample_economic_block_model() -> EconomicBlockModel {
    EconomicBlockModel::build(
        sample_model(),
        EconomicBlockModelConfig {
            tonnage_column: ColumnId::new("tonnes").expect("column should be valid"),
            grade_columns: vec![ColumnId::new("cu").expect("column should be valid")],
            destinations: DestinationAssumptionSet::new(vec![sample_mill_destination()])
                .expect("destination set should be valid"),
        },
    )
    .expect("economic block model should build")
}

fn sample_phase_plan() -> PushbackPlan {
    PushbackPlan {
        phases: vec![
            mine_planning::PhaseDesign {
                phase_id: "phase-a".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(1.0),
                bench: Some(100),
                block_indices: vec![0],
                block_count: 1,
                total_tonnage: Some(10.0),
                predecessor_phase_ids: vec![],
            },
            mine_planning::PhaseDesign {
                phase_id: "phase-b".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(1.0),
                bench: Some(95),
                block_indices: vec![1],
                block_count: 1,
                total_tonnage: Some(20.0),
                predecessor_phase_ids: vec!["phase-a".to_owned()],
            },
        ],
        phase_count: 2,
        total_block_count: 2,
        total_tonnage: Some(30.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    }
}

fn single_phase_plan() -> PushbackPlan {
    PushbackPlan {
        phases: vec![mine_planning::PhaseDesign {
            phase_id: "phase-a".to_owned(),
            pushback_index: 0,
            shell_index: Some(0),
            revenue_factor: Some(1.0),
            bench: Some(100),
            block_indices: vec![0, 1],
            block_count: 2,
            total_tonnage: Some(30.0),
            predecessor_phase_ids: vec![],
        }],
        phase_count: 1,
        total_block_count: 2,
        total_tonnage: Some(30.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    }
}

fn sample_mill_destination() -> DestinationAssumptions {
    DestinationAssumptions::new(
        DestinationId::new("mill").expect("destination should be valid"),
        DestinationKind::Mill,
        2.0,
        8.0,
        vec![
            DestinationRecovery::new(ColumnId::new("cu").expect("column should be valid"), 0.9)
                .expect("recovery should be valid"),
        ],
        vec![
            DestinationPayability::new(ColumnId::new("cu").expect("column should be valid"), 0.8)
                .expect("payability should be valid"),
        ],
        DestinationCapacity::new(
            None,
            MeasurementUnit::new("t").expect("unit should be valid"),
        )
        .expect("capacity should be valid"),
        BTreeMap::from([("cu".to_owned(), 100.0)]),
    )
    .expect("destination should be valid")
}

fn synthetic_economics_report(
    scenario_id: &str,
    npv: f64,
    total_cashflow: f64,
    total_tonnage: f64,
) -> mine_economics::LongTermScheduleEconomicsReport {
    mine_economics::LongTermScheduleEconomicsReport {
        scenario_id: scenario_id.to_owned(),
        model_id: "model".to_owned(),
        periods: vec![],
        total_revenue: total_cashflow.max(0.0),
        total_cost: (-total_cashflow).max(0.0),
        total_cashflow,
        npv,
        discount_rate_per_period: 0.1,
        total_tonnage,
        total_block_count: 0,
        destination_tonnage: BTreeMap::new(),
        payable_metal: BTreeMap::new(),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}
