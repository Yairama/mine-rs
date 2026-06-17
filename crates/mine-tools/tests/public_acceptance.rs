//! Acceptance test enfocada en el contrato Rust alpha recomendado.

use std::collections::BTreeMap;

use mine_sdk::{
    blockmodel::{BlockModel, ColumnData},
    core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        MetadataValue, RequiredColumn,
    },
};
use mine_tools::{
    GradeTonnageInput, InspectModelInput, QueryBlocksInput, QueryFilter, ValidateModelInput,
    grade_tonnage, inspect_model, query_blocks, validate_model,
};

fn tiny_public_model() -> BlockModel {
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
        Metadata::from_entries(vec![(
            "source".to_owned(),
            MetadataValue::Text("synthetic-acceptance".to_owned()),
        )])
        .expect("metadata should be valid"),
        BTreeMap::from([
            (
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnData::Floats(vec![0.2, 0.8, 1.2]),
            ),
            (
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(vec![12.0, 15.0, 18.0]),
            ),
            (
                ColumnId::new("domain").expect("column id should be valid"),
                ColumnData::Texts(vec!["waste".to_owned(), "ore".to_owned(), "ore".to_owned()]),
            ),
        ]),
    )
    .expect("block model should be valid")
}

#[test]
fn recommended_rust_public_flow_stays_coherent() {
    let model = tiny_public_model();

    let inspect_response = inspect_model(&model, &InspectModelInput);
    assert!(inspect_response.success);
    let inspect_output = inspect_response
        .output
        .expect("inspect output should exist");
    assert_eq!(inspect_output.summary.block_count, 3);
    assert_eq!(
        inspect_output.summary.metadata_keys,
        vec!["source".to_owned()]
    );

    let validate_response = validate_model(
        &model,
        &ValidateModelInput {
            required_columns: vec![
                RequiredColumn::new(
                    ColumnId::new("cu").expect("column id should be valid"),
                    ColumnLogicalType::Float,
                ),
                RequiredColumn::new(
                    ColumnId::new("tonnes").expect("column id should be valid"),
                    ColumnLogicalType::Float,
                ),
            ],
            ..ValidateModelInput::default()
        },
    );
    assert!(validate_response.success);
    assert_eq!(
        validate_response
            .output
            .expect("validation output should exist")
            .report
            .error_count(),
        0
    );

    let query_response = query_blocks(
        &model,
        &QueryBlocksInput {
            filters: vec![
                QueryFilter::TextMatch {
                    column: ColumnId::new("domain").expect("column id should be valid"),
                    value: "ore".to_owned(),
                },
                QueryFilter::FloatMinimum {
                    column: ColumnId::new("cu").expect("column id should be valid"),
                    minimum: 0.5,
                },
            ],
            selected_columns: vec![
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnId::new("tonnes").expect("column id should be valid"),
            ],
            offset: 0,
            limit: 10,
        },
    );
    assert!(query_response.success);
    let query_output = query_response.output.expect("query output should exist");
    assert_eq!(query_output.total_matches, 2);
    assert_eq!(query_output.returned_count, 2);
    assert_eq!(
        query_output
            .rows
            .iter()
            .map(|row| row.linear_index)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    let grade_tonnage_response = grade_tonnage(
        &model,
        &GradeTonnageInput {
            grade_column: ColumnId::new("cu").expect("column id should be valid"),
            tonnage_column: ColumnId::new("tonnes").expect("column id should be valid"),
            cutoffs: vec![0.0, 0.5, 1.0],
        },
    );
    assert!(grade_tonnage_response.success);
    let grade_tonnage_output = grade_tonnage_response
        .output
        .as_ref()
        .expect("grade-tonnage output should exist");
    assert_eq!(grade_tonnage_output.summary.total_block_count, 3);
    assert_eq!(grade_tonnage_output.summary.total_tonnage, 45.0);
    assert_eq!(grade_tonnage_output.points.len(), 3);
    assert_eq!(grade_tonnage_output.points[1].cutoff, 0.5);
    assert_eq!(grade_tonnage_output.points[1].block_count, 2);
    assert_eq!(grade_tonnage_output.points[1].tonnage, 33.0);
    assert_eq!(grade_tonnage_output.points[2].cutoff, 1.0);
    assert_eq!(grade_tonnage_output.points[2].block_count, 1);

    let response_json =
        serde_json::to_string(&grade_tonnage_response).expect("response should serialize");
    assert!(response_json.contains("\"tool_name\":\"grade_tonnage\""));
    assert!(response_json.contains("\"total_tonnage\":45.0"));
}
