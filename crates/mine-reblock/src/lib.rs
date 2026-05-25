//! Contratos declarativos para reblocking determinista.

mod adaptive;
mod aggregation;
mod distribution;
mod internal;
mod reconciliation;

pub use adaptive::{
    AdaptiveReblockPrototype, AdaptiveResolutionStrategy, AdaptiveZonePrototype, AdaptiveZoneRule,
    build_adaptive_reblock_prototype,
};
pub use aggregation::{
    AggregationOperation, AggregationRule, AggregationRules, CustomAggregationSpec,
    WeightedAggregation, aggregate_weighted_column, aggregate_weighted_values, superblock,
};
pub use distribution::{DistributionOperation, DistributionRule, DistributionRules, subblock};
pub use reconciliation::{
    ReconciliationBlockCount, ReconciliationMetric, ReconciliationReport, ReconciliationTolerances,
    reconcile_models,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_blockmodel::{BlockModel, ColumnData};
    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        MineError,
    };

    use super::*;

    fn sample_schema() -> ColumnSchemaSet {
        ColumnSchemaSet::from_columns(vec![
            ColumnSchema::new(
                ColumnId::new("tonnes").expect("column should be valid"),
                ColumnLogicalType::Float,
                Some(MeasurementUnit::new("t").expect("unit should be valid")),
                false,
                ColumnMiningRole::Tonnage,
            ),
            ColumnSchema::new(
                ColumnId::new("cu").expect("column should be valid"),
                ColumnLogicalType::Float,
                Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
                false,
                ColumnMiningRole::Grade,
            ),
            ColumnSchema::new(
                ColumnId::new("domain").expect("column should be valid"),
                ColumnLogicalType::Text,
                None,
                false,
                ColumnMiningRole::Domain,
            ),
            ColumnSchema::new(
                ColumnId::new("selected").expect("column should be valid"),
                ColumnLogicalType::Boolean,
                None,
                false,
                ColumnMiningRole::Other,
            ),
        ])
        .expect("schema should be valid")
    }

    fn sample_model(weight_values: Vec<f64>) -> BlockModel {
        let schema = sample_schema();
        let grid = GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
            BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
            GridShape::new(3, 1, 1).expect("shape should be valid"),
            None,
        )
        .expect("grid should be valid");

        BlockModel::new(
            grid,
            schema,
            Metadata::new(),
            BTreeMap::from([
                (
                    ColumnId::new("tonnes").expect("column should be valid"),
                    ColumnData::Floats(weight_values),
                ),
                (
                    ColumnId::new("cu").expect("column should be valid"),
                    ColumnData::Floats(vec![0.5, 1.0, 1.5]),
                ),
                (
                    ColumnId::new("domain").expect("column should be valid"),
                    ColumnData::Texts(vec!["waste".to_owned(), "ore".to_owned(), "ore".to_owned()]),
                ),
                (
                    ColumnId::new("selected").expect("column should be valid"),
                    ColumnData::Booleans(vec![false, true, true]),
                ),
            ]),
        )
        .expect("model should be valid")
    }

    #[test]
    fn validate_safe_sum_and_weighted_average_rules() {
        let rules = AggregationRules::new(vec![
            AggregationRule::sum(
                ColumnId::new("tonnes_total").expect("column should be valid"),
                ColumnId::new("tonnes").expect("column should be valid"),
            ),
            AggregationRule::weighted_average(
                ColumnId::new("cu_avg").expect("column should be valid"),
                ColumnId::new("cu").expect("column should be valid"),
                ColumnId::new("tonnes").expect("column should be valid"),
            ),
            AggregationRule::majority(
                ColumnId::new("domain_mode").expect("column should be valid"),
                ColumnId::new("domain").expect("column should be valid"),
            ),
        ])
        .expect("rules should be valid");

        assert!(rules.validate_against_schema(&sample_schema()).is_ok());
    }

    #[test]
    fn reject_duplicate_output_columns() {
        let error = AggregationRules::new(vec![
            AggregationRule::sum(
                ColumnId::new("tonnes_total").expect("column should be valid"),
                ColumnId::new("tonnes").expect("column should be valid"),
            ),
            AggregationRule::maximum(
                ColumnId::new("tonnes_total").expect("column should be valid"),
                ColumnId::new("tonnes").expect("column should be valid"),
            ),
        ])
        .expect_err("duplicate outputs should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "rules",
                "aggregation output column `tonnes_total` is duplicated"
            )
        );
    }

    #[test]
    fn reject_missing_required_column() {
        let rules = AggregationRules::new(vec![AggregationRule::sum(
            ColumnId::new("metal").expect("column should be valid"),
            ColumnId::new("contained_metal").expect("column should be valid"),
        )])
        .expect("rules should be valid");

        let error = rules
            .validate_against_schema(&sample_schema())
            .expect_err("missing source column should fail");

        assert_eq!(
            error,
            MineError::schema(
                "aggregation rule for output `metal` requires column `contained_metal` but it is missing from the schema"
            )
        );
    }

    #[test]
    fn reject_unsafe_sum_on_text_column() {
        let rules = AggregationRules::new(vec![AggregationRule::sum(
            ColumnId::new("domain_total").expect("column should be valid"),
            ColumnId::new("domain").expect("column should be valid"),
        )])
        .expect("rules should be valid");

        let error = rules
            .validate_against_schema(&sample_schema())
            .expect_err("text sum should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "rules",
                "aggregation rule for output `domain_total` requires numeric column `domain`, but found `Text`"
            )
        );
    }

    #[test]
    fn reject_majority_on_float_column() {
        let rules = AggregationRules::new(vec![AggregationRule::majority(
            ColumnId::new("cu_mode").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
        )])
        .expect("rules should be valid");

        let error = rules
            .validate_against_schema(&sample_schema())
            .expect_err("float majority should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "rules",
                "majority aggregation for output `cu_mode` requires boolean, integer or text input, but `cu` is `Float`"
            )
        );
    }

    #[test]
    fn validate_custom_numeric_rule() {
        let rules = AggregationRules::new(vec![AggregationRule::custom_numeric(
            ColumnId::new("grade_index").expect("column should be valid"),
            CustomAggregationSpec::new(
                "grade_index",
                vec![
                    ColumnId::new("cu").expect("column should be valid"),
                    ColumnId::new("tonnes").expect("column should be valid"),
                ],
                ColumnLogicalType::Float,
            )
            .expect("custom rule should be valid"),
        )])
        .expect("rules should be valid");

        assert!(rules.validate_against_schema(&sample_schema()).is_ok());
    }

    #[test]
    fn aggregate_weighted_values_skips_nulls_and_zero_weights() {
        let aggregation = aggregate_weighted_values(
            &[Some(0.5), None, Some(1.5)],
            &[Some(0.0), Some(2.0), Some(1.0)],
        )
        .expect("weighted aggregation should succeed");

        assert_eq!(aggregation.input_count, 3);
        assert_eq!(aggregation.skipped_null_count, 1);
        assert_eq!(aggregation.contributing_count, 2);
        assert_eq!(aggregation.total_weight, 1.0);
        assert_eq!(aggregation.weighted_sum, 1.5);
        assert_eq!(aggregation.weighted_average, Some(1.5));
    }

    #[test]
    fn aggregate_weighted_column_returns_none_for_zero_total_weight() {
        let aggregation = aggregate_weighted_column(
            &sample_model(vec![0.0, 0.0, 0.0]),
            &ColumnId::new("cu").expect("column should be valid"),
            &ColumnId::new("tonnes").expect("column should be valid"),
            None,
        )
        .expect("weighted aggregation should succeed");

        assert_eq!(aggregation.total_weight, 0.0);
        assert_eq!(aggregation.weighted_average, None);
    }

    #[test]
    fn aggregate_weighted_column_reports_missing_columns() {
        let error = aggregate_weighted_column(
            &sample_model(vec![10.0, 20.0, 30.0]),
            &ColumnId::new("density").expect("column should be valid"),
            &ColumnId::new("tonnes").expect("column should be valid"),
            Some(&[0, 2]),
        )
        .expect_err("missing value column should fail");

        assert_eq!(
            error,
            MineError::schema(
                "weighted aggregation value column `density` does not exist in block model storage"
            )
        );
    }

    #[test]
    fn validate_safe_distribution_rules() {
        let rules = DistributionRules::new(vec![
            DistributionRule::split_equally(
                ColumnId::new("tonnes").expect("column should be valid"),
                ColumnId::new("tonnes").expect("column should be valid"),
            ),
            DistributionRule::replicate(
                ColumnId::new("cu").expect("column should be valid"),
                ColumnId::new("cu").expect("column should be valid"),
            ),
            DistributionRule::replicate(
                ColumnId::new("domain").expect("column should be valid"),
                ColumnId::new("domain").expect("column should be valid"),
            ),
        ])
        .expect("rules should be valid");

        assert!(rules.validate_against_schema(&sample_schema()).is_ok());
    }
}
