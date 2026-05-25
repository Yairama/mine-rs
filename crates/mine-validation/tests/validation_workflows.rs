//! Tests de integración para workflows públicos de `mine-validation`.

use std::collections::BTreeMap;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, RequiredColumn,
};
use mine_validation::{
    BlockModelValidationExt, ValidationIssueCode, ValidationOptions, validate_block_model,
    validate_block_model_with_options,
};

fn sample_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid")
}

fn sample_model(with_grade_unit: bool) -> BlockModel {
    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            with_grade_unit.then(|| MeasurementUnit::new("%Cu").expect("unit should be valid")),
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
        sample_grid(),
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
        ]),
    )
    .expect("block model should be valid")
}

fn sample_model_with_tonnage_values(tonnage_values: Vec<f64>) -> BlockModel {
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
        sample_grid(),
        schema,
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnData::Floats(vec![0.8, 1.1]),
            ),
            (
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(tonnage_values),
            ),
        ]),
    )
    .expect("block model should be valid")
}

fn sparse_model() -> BlockModel {
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

    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(3, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");

    BlockModel::new_sparse(
        grid,
        schema,
        Metadata::new(),
        vec![0, 1],
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

#[test]
fn combine_schema_and_grid_validation() {
    let report = validate_block_model(
        &sample_model(false),
        &[RequiredColumn::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
        )],
    );

    assert_eq!(report.error_count(), 0);
    assert_eq!(report.warning_count(), 1);
    assert_eq!(
        report.issues[0].code,
        ValidationIssueCode::MissingMeasurementUnit
    );
}

#[test]
fn combine_value_validation_with_schema_and_grid_validation() {
    let report = validate_block_model(&sample_model_with_tonnage_values(vec![-1.0, 15.0]), &[]);

    assert!(report.has_errors());
    assert_eq!(report.error_count(), 1);
    assert_eq!(
        report.issues[0].code,
        ValidationIssueCode::InvalidTonnageValue
    );
}

#[test]
fn combine_missing_blocks_and_extent_validation_for_sparse_model() {
    let report = validate_block_model_with_options(&sparse_model(), &ValidationOptions::new());

    assert!(report.has_errors());
    assert_eq!(report.error_count(), 1);
    assert_eq!(report.warning_count(), 1);
    assert_eq!(
        report.issues[0].code,
        ValidationIssueCode::MissingBlocksDetected
    );
    assert_eq!(report.issues[1].code, ValidationIssueCode::IncompleteExtent);
}

#[test]
fn allow_sparse_models_when_configured() {
    let report = validate_block_model_with_options(
        &sparse_model(),
        &ValidationOptions::new().with_sparse_allowed(true),
    );

    assert!(!report.has_errors());
    assert!(report.issues.is_empty());
}

#[test]
fn allow_disabling_value_validation() {
    let options = ValidationOptions::new().with_value_validation(false);
    let report = validate_block_model_with_options(
        &sample_model_with_tonnage_values(vec![-1.0, 15.0]),
        &options,
    );

    assert!(!report.has_errors());
    assert!(report.issues.is_empty());
}

#[test]
fn expose_model_validate_extension_methods() {
    let report = sample_model(true).validate();
    let options = ValidationOptions::new()
        .with_required_columns(vec![RequiredColumn::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
        )])
        .with_schema_validation(false);
    let configured_report =
        sample_model_with_tonnage_values(vec![-1.0, 15.0]).validate_with_options(&options);

    assert!(report.issues.is_empty());
    assert_eq!(configured_report.error_count(), 1);
    assert_eq!(
        configured_report.issues[0].code,
        ValidationIssueCode::InvalidTonnageValue
    );
}
