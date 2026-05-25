//! Motor mínimo de block models regulares en memoria.

mod analytics;
mod block_support;
mod classification;
mod compositing;
mod data;
mod declustering;
mod estimators;
mod filters;
mod kriging;
mod layout;
mod model;
mod neighborhoods;
mod realizations;
mod selection;
mod simulation;
mod summary;
mod validation;
mod variography;

pub use analytics::{
    BasicStatistics, ColumnNullCount, GradeTonnagePoint, GroupedStatistics, WeightedGradeStatistic,
};
pub use block_support::{
    BlockDiscretization, BlockSupportRegularization, compute_block_to_block_covariance,
    compute_point_to_block_covariance, regularize_block_support,
};
pub use classification::{
    ClassificationLevelAssessment, ClassificationMetricConfig, ClassificationMetricsReport,
    ClassificationThreshold, ContinuityMetrics, InformednessMetrics, PassUsageMetric,
    SampleSpacingMetrics, evaluate_classification_metrics,
};
pub use compositing::{
    CompositeContribution, CompositeDomainAuditIssue, CompositeDomainAuditIssueCode,
    CompositeDomainAuditReport, CompositeInterval, CompositeResidualPolicy, CompositingOptions,
    DomainFilterReport, DomainMask, IntervalSample, audit_composite_domains, composite_intervals,
    filter_interval_samples_by_domain_mask,
};
pub use data::ColumnData;
pub use declustering::{
    CellDeclusteringOptions, CellDeclusteringResult, CellOriginOffset, DeclusteredSampleWeight,
    DomainWeightedStatistics, SpatialSample, WeightedHistogram, WeightedHistogramBin,
    WeightedSampleStatistics, WeightedStatisticsReport, WeightedVariableSummary,
    build_weighted_histogram, build_weighted_statistics_report, compute_cell_declustering_weights,
};
pub use estimators::{
    DeterministicEstimatorKind, EstimateContribution, InverseDistanceWeightingOptions,
    PointEstimate, estimate_inverse_distance_weighting, estimate_nearest_neighbor,
};
pub use kriging::{
    KrigingEstimate, KrigingEstimatorKind, SimpleKrigingOptions, estimate_ordinary_kriging,
    estimate_simple_kriging,
};
pub use layout::BlockLayout;
pub use model::BlockModel;
pub use neighborhoods::{
    EstimationPass, EstimationPassEvaluation, EstimationPassSelection, NeighborhoodSample,
    NeighborhoodSelection, SampleCountLimits, SearchAnisotropy, SearchNeighborhood,
    select_samples_by_estimation_passes, select_samples_in_neighborhood,
};
pub use realizations::{
    ConditionalRealization, ConditionalRealizationLineage, ConditionalRealizationSet,
    RealizationStorageFormat, RealizationSupport,
};
pub use selection::BlockSelection;
pub use simulation::{
    SequentialGaussianSimulationOptions, SequentialIndicatorSimulationOptions,
    SequentialSimulationEnsemble, SequentialSimulationRealization, SequentialSimulationSummary,
    SimulatedNodeValue, SimulationTarget, generate_sequential_gaussian_ensemble,
    generate_sequential_indicator_ensemble,
};
pub use summary::{ColumnSummary, ModelSummary, SpatialExtent};
pub use validation::{
    CompositeVsBlockReport, CrossValidationEntry, CrossValidationEstimator, CrossValidationMetrics,
    CrossValidationReport, SwathAxis, SwathBin, SwathDataPoint, SwathPlotReport,
    VariableStatistics, build_swath_plot, compare_composites_vs_blocks,
    cross_validate_leave_one_out,
};
pub use variography::{
    ExperimentalVariogram, ExperimentalVariogramLag, ExperimentalVariogramLagRow,
    VariogramDirection, VariogramFitOptions, VariogramFitSummary, VariogramLagConfig,
    VariogramModel, VariogramModelKind, build_experimental_variogram,
    experimental_variogram_from_lag_rows, fit_variogram_model,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        MineError,
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

    fn sample_schema() -> ColumnSchemaSet {
        ColumnSchemaSet::from_columns(vec![
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
        .expect("schema should be valid")
    }

    fn sample_columns() -> BTreeMap<ColumnId, ColumnData> {
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
        ])
    }

    fn sparse_grid() -> GridDefinition {
        GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
            BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
            GridShape::new(3, 1, 1).expect("shape should be valid"),
            None,
        )
        .expect("grid should be valid")
    }

    fn sparse_columns() -> BTreeMap<ColumnId, ColumnData> {
        sample_columns()
    }

    #[test]
    fn build_minimal_block_model() {
        let metadata = Metadata::from_entries(vec![(
            "source".to_owned(),
            mine_core::MetadataValue::Text("synthetic".to_owned()),
        )])
        .expect("metadata should be valid");

        let model = BlockModel::new(
            sample_grid(),
            sample_schema(),
            metadata.clone(),
            sample_columns(),
        )
        .expect("block model should be valid");

        assert_eq!(model.block_count(), 2);
        assert_eq!(model.grid_cell_count(), 2);
        assert!(!model.is_sparse());
        assert_eq!(model.metadata(), &metadata);
        assert!(
            model
                .column(&ColumnId::new("cu").expect("column id should be valid"))
                .is_some()
        );
    }

    #[test]
    fn reject_missing_schema_column() {
        let error = BlockModel::new(
            sample_grid(),
            sample_schema(),
            Metadata::new(),
            BTreeMap::from([
                (
                    ColumnId::new("cu").expect("column id should be valid"),
                    ColumnData::Floats(vec![0.8, 1.1]),
                ),
                (
                    ColumnId::new("domain").expect("column id should be valid"),
                    ColumnData::Texts(vec!["waste".to_owned(), "ore".to_owned()]),
                ),
            ]),
        )
        .expect_err("missing schema column should fail");

        assert_eq!(
            error,
            MineError::schema(
                "column `tonnes` is declared in schema but missing from block model storage"
            )
        );
    }

    #[test]
    fn reject_type_mismatch() {
        let error = BlockModel::new(
            sample_grid(),
            sample_schema(),
            Metadata::new(),
            BTreeMap::from([
                (
                    ColumnId::new("cu").expect("column id should be valid"),
                    ColumnData::Booleans(vec![true, false]),
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
        .expect_err("type mismatch should fail");

        assert_eq!(
            error,
            MineError::schema("column `cu` has logical type `Boolean` but schema expects `Float`")
        );
    }

    #[test]
    fn reject_wrong_row_count() {
        let error = BlockModel::new(
            sample_grid(),
            sample_schema(),
            Metadata::new(),
            BTreeMap::from([
                (
                    ColumnId::new("cu").expect("column id should be valid"),
                    ColumnData::Floats(vec![0.8]),
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
        .expect_err("row count mismatch should fail");

        assert_eq!(
            error,
            MineError::validation("column `cu` has 1 rows but grid expects 2")
        );
    }

    #[test]
    fn build_sparse_block_model() {
        let model = BlockModel::new_sparse(
            sparse_grid(),
            sample_schema(),
            Metadata::new(),
            vec![0, 2],
            sparse_columns(),
        )
        .expect("sparse block model should be valid");

        assert_eq!(model.block_count(), 2);
        assert_eq!(model.grid_cell_count(), 3);
        assert!(model.is_sparse());
        assert_eq!(model.linear_index_at(0).expect("row should exist"), 0);
        assert_eq!(model.linear_index_at(1).expect("row should exist"), 2);
        assert_eq!(model.missing_linear_indices(), vec![1]);
    }

    #[test]
    fn reject_unsorted_sparse_indices() {
        let error = BlockModel::new_sparse(
            sparse_grid(),
            sample_schema(),
            Metadata::new(),
            vec![2, 0],
            sparse_columns(),
        )
        .expect_err("unsorted sparse indices should fail");

        assert_eq!(
            error,
            MineError::validation("sparse materialized linear indices must be strictly increasing")
        );
    }
}
