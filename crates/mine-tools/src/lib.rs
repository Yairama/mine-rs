//! Tools deterministas y contratos estructurados para automatizacion.

mod analytics;
mod catalog;
mod contract;
mod inspect;
mod query;
mod scenarios;
mod validation;

pub use analytics::{
    AggregateBlocksInput, AggregateBlocksOutput, GradeTonnageInput, GradeTonnageOutput,
    GradeTonnageSummary, aggregate_blocks, grade_tonnage,
};
pub use catalog::{ToolCatalog, initial_tool_catalog};
pub use contract::{
    ArtifactReference, ToolDescriptor, ToolError, ToolExecutionMetadata, ToolResponse,
};
pub use inspect::{InspectModelInput, InspectModelOutput, inspect_model};
pub use query::{
    QueryBlocksInput, QueryBlocksOutput, QueryFilter, QueryRow, QueryValue, query_blocks,
};
pub use scenarios::{
    CompareScenariosInput, CompareScenariosOutput, CreateScenarioInput, CreateScenarioOutput,
    CreateScenarioPeriodInput, EvaluateScenarioInput, EvaluateScenarioOutput,
    ScenarioPeriodComparison, compare_scenarios, create_scenario, evaluate_scenario,
};
pub use validation::{ValidateModelInput, ValidateModelOutput, validate_model};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_sdk::{
        BlockDimensions, BlockModel, ColumnData, ColumnId, ColumnLogicalType, ColumnMiningRole,
        ColumnSchema, ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit,
        Metadata, ModelId, ScenarioConstraints, ScenarioId, ScenarioPeriod, ScenarioRules,
    };

    use super::*;

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

    #[test]
    fn expose_sdk_layers() {
        let catalog = initial_tool_catalog();

        assert_eq!(catalog.tool_layer.name, "mine-tools");
        assert_eq!(catalog.exposed_layers.len(), 2);
        assert_eq!(catalog.exposed_layers[1].name, "mine-sdk");
        assert_eq!(
            catalog
                .available_tools
                .iter()
                .map(|tool| tool.name)
                .collect::<Vec<_>>(),
            vec![
                "inspect_model",
                "validate_model",
                "query_blocks",
                "aggregate_blocks",
                "grade_tonnage",
                "create_scenario",
                "evaluate_scenario",
                "compare_scenarios",
            ]
        );
    }

    #[test]
    fn aggregate_blocks_reports_schema_errors() {
        let response = aggregate_blocks(
            &sample_model(),
            &AggregateBlocksInput {
                group_by: ColumnId::new("cu").expect("column id should be valid"),
                tonnage_column: ColumnId::new("tonnes").expect("column id should be valid"),
            },
        );

        assert!(!response.success);
        assert_eq!(response.errors[0].code, "schema_error");
    }

    #[test]
    fn create_scenario_reports_missing_periods() {
        let response = create_scenario(&CreateScenarioInput {
            scenario_id: ScenarioId::new("scenario-01").expect("scenario id should be valid"),
            model_id: ModelId::new("model-01").expect("model id should be valid"),
            periods: Vec::new(),
            phase_column: None,
            bench_parameters: None,
            max_vertical_advance: None,
            max_active_phases: None,
            assumptions: Metadata::new(),
        });

        assert!(!response.success);
        assert_eq!(response.errors[0].code, "invalid_parameter");
        assert!(response.errors[0].message.contains("at least one period"));
    }

    #[test]
    fn sample_scenario_builder_stays_serializable() {
        let scenario = mine_sdk::MiningScenario::new(
            ScenarioId::new("scenario-01").expect("scenario id should be valid"),
            ModelId::new("model-01").expect("model id should be valid"),
            vec![
                ScenarioPeriod::new("P1", Some(1000.0), None).expect("period should be valid"),
                ScenarioPeriod::new("P2", Some(1200.0), None).expect("period should be valid"),
            ],
            ScenarioRules::default(),
            ScenarioConstraints::default(),
            Metadata::new(),
        )
        .expect("scenario should be valid");

        let json = serde_json::to_string(&scenario).expect("scenario should serialize");
        assert!(json.contains("scenario-01"));
    }
}
