//! Tests de integración para workflows públicos de `mine-tools`.

use std::collections::BTreeMap;

use mine_sdk::{
    BlockDimensions, BlockModel, ColumnData, ColumnId, ColumnLogicalType, ColumnMiningRole,
    ColumnSchema, ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit,
    Metadata, ModelId, PeriodCashflowInput, RequiredColumn, ScenarioConstraints, ScenarioId,
    ScenarioPeriod, ScenarioRules,
};
use mine_tools::{
    CompareScenariosInput, CreateScenarioInput, CreateScenarioPeriodInput, EvaluateScenarioInput,
    GradeTonnageInput, InspectModelInput, QueryBlocksInput, QueryFilter, QueryValue,
    ValidateModelInput, compare_scenarios, create_scenario, evaluate_scenario, grade_tonnage,
    inspect_model, query_blocks, validate_model,
};

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
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
        ColumnSchema::new(
            ColumnId::new("domain").expect("column id should be valid"),
            ColumnLogicalType::Text,
            None,
            false,
            ColumnMiningRole::Domain,
        ),
    ])
    .expect("schema should be valid");

    BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnData::Floats(vec![0.8, 1.1]),
            ),
            (
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(vec![12.0, 15.0]),
            ),
            (
                ColumnId::new("domain").expect("column id should be valid"),
                ColumnData::Texts(vec!["waste".to_owned(), "ore".to_owned()]),
            ),
        ]),
    )
    .expect("block model should be valid")
}

fn invalid_tonnage_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");

    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
    ])
    .expect("schema should be valid");

    BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnData::Floats(vec![0.8, 1.1]),
            ),
            (
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(vec![-1.0, 15.0]),
            ),
        ]),
    )
    .expect("block model should be valid")
}

fn sparse_sample_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(3, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");

    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
    ])
    .expect("schema should be valid");

    BlockModel::new_sparse(
        grid,
        schema,
        Metadata::new(),
        vec![0, 2],
        BTreeMap::from([
            (
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnData::Floats(vec![0.8, 1.1]),
            ),
            (
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(vec![12.0, 15.0]),
            ),
        ]),
    )
    .expect("sparse block model should be valid")
}

fn sample_scenario() -> mine_sdk::MiningScenario {
    sample_scenario_with_id("scenario-01")
}

fn sample_scenario_with_id(scenario_id: &str) -> mine_sdk::MiningScenario {
    mine_sdk::MiningScenario::new(
        ScenarioId::new(scenario_id).expect("scenario id should be valid"),
        ModelId::new("model-01").expect("model id should be valid"),
        vec![
            ScenarioPeriod::new("P1", Some(1000.0), None).expect("period should be valid"),
            ScenarioPeriod::new("P2", Some(1200.0), None).expect("period should be valid"),
        ],
        ScenarioRules::default(),
        ScenarioConstraints::default(),
        Metadata::new(),
    )
    .expect("scenario should be valid")
}

#[test]
fn inspect_model_returns_serializable_summary() {
    let response = inspect_model(&sample_model(), &InspectModelInput);
    let json = serde_json::to_string(&response).expect("response should serialize");

    assert!(response.success);
    assert!(response.errors.is_empty());
    assert!(
        response
            .output
            .as_ref()
            .expect("output should exist")
            .warnings
            .is_empty()
    );
    assert!(json.contains("\"tool_name\":\"inspect_model\""));
    assert!(json.contains("\"block_count\":2"));
}

#[test]
fn validate_model_wraps_validation_report() {
    let response = validate_model(
        &sample_model(),
        &ValidateModelInput {
            required_columns: vec![RequiredColumn::new(
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnLogicalType::Float,
            )],
            ..ValidateModelInput::default()
        },
    );

    assert!(response.success);
    assert_eq!(
        response
            .output
            .expect("output should exist")
            .report
            .error_count(),
        0
    );
}

#[test]
fn validate_model_can_disable_value_checks() {
    let response = validate_model(
        &invalid_tonnage_model(),
        &ValidateModelInput {
            validate_values: false,
            ..ValidateModelInput::default()
        },
    );

    assert!(response.success);
    assert!(
        response
            .output
            .expect("output should exist")
            .report
            .issues
            .is_empty()
    );
}

#[test]
fn validate_model_can_allow_sparse_layouts() {
    let response = validate_model(
        &sparse_sample_model(),
        &ValidateModelInput {
            allow_sparse: true,
            ..ValidateModelInput::default()
        },
    );

    assert!(response.success);
    assert!(
        response
            .output
            .expect("output should exist")
            .report
            .issues
            .is_empty()
    );
}

#[test]
fn query_blocks_applies_filters_and_reports_pagination() {
    let response = query_blocks(
        &sample_model(),
        &QueryBlocksInput {
            filters: vec![
                QueryFilter::TextMatch {
                    column: ColumnId::new("domain").expect("column id should be valid"),
                    value: "ore".to_owned(),
                },
                QueryFilter::FloatMinimum {
                    column: ColumnId::new("cu").expect("column id should be valid"),
                    minimum: 1.0,
                },
            ],
            selected_columns: vec![
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnId::new("domain").expect("column id should be valid"),
            ],
            offset: 0,
            limit: 1,
        },
    );

    assert!(response.success);
    let output = response.output.expect("output should exist");
    assert_eq!(output.total_matches, 1);
    assert_eq!(output.returned_count, 1);
    assert!(!output.truncated);
    assert_eq!(output.rows[0].linear_index, 1);
    assert_eq!(
        output.rows[0]
            .values
            .get(&ColumnId::new("domain").expect("column id should be valid")),
        Some(&QueryValue::Text("ore".to_owned()))
    );
}

#[test]
fn query_blocks_reports_next_offset_when_truncated() {
    let response = query_blocks(
        &sample_model(),
        &QueryBlocksInput {
            limit: 1,
            ..QueryBlocksInput::default()
        },
    );

    assert!(response.success);
    let output = response.output.expect("output should exist");
    assert_eq!(output.total_matches, 2);
    assert_eq!(output.returned_count, 1);
    assert!(output.truncated);
    assert_eq!(output.next_offset, Some(1));
}

#[test]
fn query_blocks_reports_materialized_linear_index_for_sparse_models() {
    let response = query_blocks(
        &sparse_sample_model(),
        &QueryBlocksInput {
            filters: vec![QueryFilter::FloatMinimum {
                column: ColumnId::new("cu").expect("column id should be valid"),
                minimum: 1.0,
            }],
            selected_columns: vec![ColumnId::new("cu").expect("column id should be valid")],
            offset: 0,
            limit: 10,
        },
    );

    assert!(response.success);
    let output = response.output.expect("output should exist");
    assert_eq!(output.total_matches, 1);
    assert_eq!(output.rows[0].linear_index, 2);
}

#[test]
fn grade_tonnage_returns_curve_with_summary_and_assumptions() {
    let response = grade_tonnage(
        &sample_model(),
        &GradeTonnageInput {
            grade_column: ColumnId::new("cu").expect("column id should be valid"),
            tonnage_column: ColumnId::new("tonnes").expect("column id should be valid"),
            cutoffs: vec![0.7, 1.0],
        },
    );

    assert!(response.success);
    let output = response.output.expect("output should exist");
    assert_eq!(output.grade_column.as_str(), "cu");
    assert_eq!(output.tonnage_column.as_str(), "tonnes");
    assert_eq!(output.summary.total_block_count, 2);
    assert_eq!(output.summary.total_tonnage, 27.0);
    assert_eq!(output.points.len(), 2);
}

#[test]
fn create_scenario_returns_serializable_scenario() {
    let response = create_scenario(&CreateScenarioInput {
        scenario_id: ScenarioId::new("scenario-01").expect("scenario id should be valid"),
        model_id: ModelId::new("model-01").expect("model id should be valid"),
        periods: vec![
            CreateScenarioPeriodInput {
                label: "P1".to_owned(),
                target_tonnage: Some(1000.0),
                target_blocks: None,
            },
            CreateScenarioPeriodInput {
                label: "P2".to_owned(),
                target_tonnage: Some(1200.0),
                target_blocks: Some(10),
            },
        ],
        phase_column: Some(ColumnId::new("phase").expect("column id should be valid")),
        bench_parameters: Some(
            mine_sdk::BenchParameters::new(20.0, 100.0, 1e-9)
                .expect("bench parameters should be valid"),
        ),
        max_vertical_advance: Some(30.0),
        max_active_phases: Some(2),
        assumptions: Metadata::new(),
    });

    assert!(response.success);
    let scenario = response.output.expect("output should exist").scenario;
    assert_eq!(scenario.periods().len(), 2);
    assert_eq!(scenario.constraints().max_vertical_advance(), Some(30.0));
}

#[test]
fn evaluate_scenario_returns_cashflow_report() {
    let response = evaluate_scenario(&EvaluateScenarioInput {
        scenario: sample_scenario(),
        period_inputs: vec![
            PeriodCashflowInput::new("P1", 100.0, 40.0).expect("input should be valid"),
            PeriodCashflowInput::new("P2", 120.0, 50.0).expect("input should be valid"),
        ],
        discount_rate_per_period: 0.1,
    });

    assert!(response.success);
    let report = response.output.expect("output should exist").report;
    assert_eq!(report.scenario_id, "scenario-01");
    assert_eq!(report.periods.len(), 2);
    assert!((report.npv - (60.0 + 70.0 / 1.1)).abs() < 1e-9);
}

#[test]
fn compare_scenarios_summarizes_npv_and_period_deltas() {
    let base = mine_sdk::evaluate_scenario_cashflow(
        &sample_scenario(),
        &[
            PeriodCashflowInput::new("P1", 100.0, 40.0).expect("input should be valid"),
            PeriodCashflowInput::new("P2", 120.0, 50.0).expect("input should be valid"),
        ],
        0.1,
    )
    .expect("base report should build");
    let candidate = mine_sdk::evaluate_scenario_cashflow(
        &sample_scenario_with_id("scenario-02"),
        &[
            PeriodCashflowInput::new("P1", 110.0, 40.0).expect("input should be valid"),
            PeriodCashflowInput::new("P2", 140.0, 55.0).expect("input should be valid"),
        ],
        0.1,
    )
    .expect("candidate report should build");
    let response = compare_scenarios(&CompareScenariosInput { base, candidate });

    assert!(response.success);
    let output = response.output.expect("output should exist");
    assert_eq!(output.base_scenario_id, "scenario-01");
    assert_eq!(output.candidate_scenario_id, "scenario-02");
    assert!(output.npv_delta > 0.0);
    assert_eq!(output.preferred_scenario_id.as_deref(), Some("scenario-02"));
    assert_eq!(output.period_comparisons.len(), 2);
    assert_eq!(output.period_comparisons[0].period_label, "P1");
    assert_eq!(output.period_comparisons[0].cashflow_delta, Some(10.0));
}
