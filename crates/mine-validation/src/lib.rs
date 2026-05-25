//! Reportes y validadores estructurados para modelos mineros.

mod options;
mod report;
mod schema;
mod spatial;
mod suite;
mod values;

pub use options::{BlockModelValidationExt, ValidationOptions};
pub use report::{ValidationIssue, ValidationIssueCode, ValidationReport, ValidationSeverity};
pub use schema::validate_block_model_schema;
pub use spatial::{
    validate_block_model_extents, validate_block_model_missing_blocks,
    validate_block_model_regular_grid, validate_duplicate_block_coordinates,
    validate_duplicate_block_indices,
};
pub use suite::{validate_block_model, validate_block_model_with_options};
pub use values::validate_block_model_values;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_blockmodel::{BlockModel, ColumnData};
    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        RequiredColumn,
    };

    use super::*;

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

    fn rotated_model() -> BlockModel {
        let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        )])
        .expect("schema should be valid");

        let grid = GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
            BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
            GridShape::new(2, 1, 1).expect("shape should be valid"),
            Some(15.0),
        )
        .expect("grid should be valid");

        BlockModel::new(
            grid,
            schema,
            Metadata::new(),
            BTreeMap::from([(
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnData::Floats(vec![0.8, 1.1]),
            )]),
        )
        .expect("block model should be valid")
    }

    fn sample_model_with_grade_values(grade_values: Vec<f64>) -> BlockModel {
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
                    ColumnData::Floats(grade_values),
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

    fn density_model(density_values: Vec<f64>) -> BlockModel {
        let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
            ColumnId::new("density").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t/m3").expect("unit should be valid")),
            false,
            ColumnMiningRole::Density,
        )])
        .expect("schema should be valid");

        BlockModel::new(
            sample_grid(),
            schema,
            Metadata::new(),
            BTreeMap::from([(
                ColumnId::new("density").expect("column id should be valid"),
                ColumnData::Floats(density_values),
            )]),
        )
        .expect("block model should be valid")
    }

    fn recovery_model(recovery_values: Vec<f64>) -> BlockModel {
        let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
            ColumnId::new("recovery").expect("column id should be valid"),
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Recovery,
        )])
        .expect("schema should be valid");

        BlockModel::new(
            sample_grid(),
            schema,
            Metadata::new(),
            BTreeMap::from([(
                ColumnId::new("recovery").expect("column id should be valid"),
                ColumnData::Floats(recovery_values),
            )]),
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
    fn serialize_validation_report() {
        let mut report = ValidationReport::new();
        report.push(
            ValidationIssue::new(
                ValidationSeverity::Error,
                ValidationIssueCode::MissingRequiredColumn,
                "missing column",
            )
            .with_location("cu")
            .with_affected_count(2),
        );

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains("MissingRequiredColumn"));
        assert!(json.contains("missing column"));
    }

    #[test]
    fn combine_issues_in_report() {
        let mut report = ValidationReport::new();
        report.extend([
            ValidationIssue::new(
                ValidationSeverity::Error,
                ValidationIssueCode::MissingRequiredColumn,
                "missing",
            ),
            ValidationIssue::new(
                ValidationSeverity::Warning,
                ValidationIssueCode::MissingMeasurementUnit,
                "unit",
            ),
        ]);

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
    }

    #[test]
    fn report_missing_required_column() {
        let report = validate_block_model_schema(
            &sample_model(true),
            &[RequiredColumn::new(
                ColumnId::new("density").expect("column id should be valid"),
                ColumnLogicalType::Float,
            )],
        );

        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::MissingRequiredColumn
        );
    }

    #[test]
    fn report_wrong_logical_type() {
        let report = validate_block_model_schema(
            &sample_model(true),
            &[RequiredColumn::new(
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnLogicalType::Boolean,
            )],
        );

        assert_eq!(report.error_count(), 1);
        assert_eq!(report.issues[0].code, ValidationIssueCode::WrongLogicalType);
    }

    #[test]
    fn warn_about_missing_measurement_unit() {
        let report = validate_block_model_schema(&sample_model(false), &[]);

        assert_eq!(report.warning_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::MissingMeasurementUnit
        );
    }

    #[test]
    fn validate_regular_grid_model_without_issues() {
        let report = validate_block_model_regular_grid(&sample_model(true), 1e-9);

        assert!(!report.has_errors());
        assert!(report.issues.is_empty());
    }

    #[test]
    fn validate_rotated_regular_grid_model_without_issues() {
        let report = validate_block_model_regular_grid(&rotated_model(), 1e-9);

        assert!(!report.has_errors());
        assert!(report.issues.is_empty());
    }

    #[test]
    fn detect_duplicate_block_indices_from_explicit_rows() {
        let report = validate_duplicate_block_indices(&[
            mine_indexing::GridIndex::new(0, 0, 0),
            mine_indexing::GridIndex::new(1, 0, 0),
            mine_indexing::GridIndex::new(0, 0, 0),
        ]);

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::DuplicateBlockDetected
        );
        assert_eq!(report.issues[0].affected_count, Some(2));
    }

    #[test]
    fn detect_duplicate_block_coordinates_after_normalization() {
        let report = validate_duplicate_block_coordinates(
            &sample_grid(),
            &[
                Coordinate3D::new(5.0, 5.0, 5.0).expect("coordinate should be valid"),
                Coordinate3D::new(6.0, 5.5, 5.0).expect("coordinate should be valid"),
                Coordinate3D::new(15.0, 5.0, 5.0).expect("coordinate should be valid"),
            ],
            1e-9,
        )
        .expect("coordinate validation should succeed");

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::DuplicateBlockDetected
        );
        assert_eq!(report.issues[0].location.as_deref(), Some("coordinates"));
    }

    #[test]
    fn validate_dense_model_extents_without_issues() {
        let report = validate_block_model_extents(&sample_model(true), 1e-9, false);

        assert!(!report.has_errors());
        assert!(report.issues.is_empty());
    }

    #[test]
    fn validate_rotated_model_extents_without_issues() {
        let report = validate_block_model_extents(&rotated_model(), 1e-9, false);

        assert!(!report.has_errors());
        assert!(report.issues.is_empty());
    }

    #[test]
    fn report_non_finite_grade_values() {
        let report =
            validate_block_model_values(&sample_model_with_grade_values(vec![f64::NAN, 1.1]));

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::NonFiniteGradeValue
        );
    }

    #[test]
    fn report_invalid_tonnage_values() {
        let report =
            validate_block_model_values(&sample_model_with_tonnage_values(vec![-1.0, 15.0]));

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::InvalidTonnageValue
        );
    }

    #[test]
    fn report_invalid_density_values() {
        let report = validate_block_model_values(&density_model(vec![0.0, 2.5]));

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::InvalidDensityValue
        );
    }

    #[test]
    fn report_invalid_recovery_values() {
        let report = validate_block_model_values(&recovery_model(vec![1.1, 0.8]));

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::InvalidRecoveryValue
        );
    }

    #[test]
    fn report_missing_blocks_when_sparse_is_not_allowed() {
        let report = validate_block_model_missing_blocks(&sparse_model(), false);

        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.issues[0].code,
            ValidationIssueCode::MissingBlocksDetected
        );
        assert_eq!(report.issues[0].affected_count, Some(1));
    }

    #[test]
    fn warn_about_incomplete_extent_when_sparse_is_not_allowed() {
        let report = validate_block_model_extents(&sparse_model(), 1e-9, false);

        assert!(!report.has_errors());
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.issues[0].code, ValidationIssueCode::IncompleteExtent);
    }
}
