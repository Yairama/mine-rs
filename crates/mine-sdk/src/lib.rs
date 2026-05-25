//! Fachada publica Rust para las capacidades del workspace `mine-rs`.

/// Reexports del dominio de block model.
pub use mine_blockmodel as blockmodel;
/// Reexports del dominio core compartido.
pub use mine_core as core;
/// Reexports del dominio económico.
pub use mine_economics as economics;
/// Reexports del dominio de indexing.
pub use mine_indexing as indexing;
/// Reexports del dominio de IO.
pub use mine_io as io;
/// Reexports del dominio de planificación.
pub use mine_planning as planning;
/// Reexports del dominio de reblocking.
pub use mine_reblock as reblock;
/// Reexports del dominio de validación.
pub use mine_validation as validation;

pub use blockmodel::{
    BasicStatistics, BlockDiscretization, BlockLayout, BlockModel, BlockSelection,
    BlockSupportRegularization, CellDeclusteringOptions, CellDeclusteringResult, CellOriginOffset,
    ColumnData, ColumnNullCount, ColumnSummary, CompositeContribution, CompositeDomainAuditIssue,
    CompositeDomainAuditIssueCode, CompositeDomainAuditReport, CompositeInterval,
    CompositeResidualPolicy, CompositingOptions, DeclusteredSampleWeight,
    DeterministicEstimatorKind, DomainFilterReport, DomainMask, DomainWeightedStatistics,
    EstimateContribution, EstimationPass, EstimationPassEvaluation, EstimationPassSelection,
    ExperimentalVariogram, ExperimentalVariogramLag, ExperimentalVariogramLagRow,
    GradeTonnagePoint, GroupedStatistics, IntervalSample, InverseDistanceWeightingOptions,
    KrigingEstimate, KrigingEstimatorKind, ModelSummary, NeighborhoodSample, NeighborhoodSelection,
    PointEstimate, SampleCountLimits, SearchAnisotropy, SearchNeighborhood, SimpleKrigingOptions,
    SpatialExtent, SpatialSample, VariogramDirection, VariogramFitOptions, VariogramFitSummary,
    VariogramLagConfig, VariogramModel, VariogramModelKind, WeightedGradeStatistic,
    WeightedHistogram, WeightedHistogramBin, WeightedSampleStatistics, WeightedStatisticsReport,
    WeightedVariableSummary, audit_composite_domains, build_experimental_variogram,
    build_weighted_histogram, build_weighted_statistics_report, composite_intervals,
    compute_block_to_block_covariance, compute_cell_declustering_weights,
    compute_point_to_block_covariance, estimate_inverse_distance_weighting,
    estimate_nearest_neighbor, estimate_ordinary_kriging, estimate_simple_kriging,
    experimental_variogram_from_lag_rows, filter_interval_samples_by_domain_mask,
    fit_variogram_model, regularize_block_support, select_samples_by_estimation_passes,
    select_samples_in_neighborhood,
};
pub use core::{
    ArtifactId, BlockDimensions, BlockId, ColumnId, ColumnLogicalType, ColumnMiningRole,
    ColumnSchema, ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, LayerDescriptor,
    MeasurementUnit, Metadata, MetadataValue, MineError, ModelId, RequiredColumn, ScenarioId,
    core_layer,
};
pub use economics::{
    BlockDestinationValue, BlockEconomicSummary, BlockEconomics, BlockEconomicsReport, BlockGrades,
    DestinationAssumptions, DestinationAssumptionSet, DestinationCapacity, DestinationId,
    DestinationKind, DestinationPayability, DestinationRecovery, EconomicAssumptions,
    EconomicBlockModel, EconomicBlockModelConfig, EconomicUnits, EvParameters, EvResult,
    MultiDestinationBlockValuation, NsrMetalInput, NsrResult, PeriodCashflowInput,
    ScenarioCashflowReport, ScenarioPeriodCashflow, compute_ev, compute_nsr,
    evaluate_block_economics, evaluate_scenario_cashflow, value_block_by_destinations,
};
pub use indexing::{
    GridIndex, NeighborConnectivity, ijk_to_linear, ijk_to_xyz, linear_to_ijk, neighboring_blocks,
    xyz_to_ijk,
};
pub use io::{
    CsvIndexColumns, CsvReadOptions, CsvWriteOptions, InferredModelSchema, SchemaInferenceHints,
    SchemaInferenceWarning, SchemaInferenceWarningCode, VtuWriteOptions, VulcanBooleanFormat,
    VulcanCoordinateColumns, VulcanCsvWriteOptions, block_model_from_record_batch,
    block_model_to_record_batch, experimental_variogram_from_record_batch,
    experimental_variogram_to_record_batch, infer_csv_schema, infer_parquet_schema,
    read_block_model_csv, read_block_model_parquet, read_experimental_variogram_json,
    read_experimental_variogram_parquet, read_marvin_blocks, write_block_model_csv,
    write_block_model_parquet, write_block_model_vtu, write_block_model_vulcan_csv,
    write_experimental_variogram_json, write_experimental_variogram_parquet,
};
pub use planning::{
    BenchAssignment, BenchParameters, BlockMembershipComparisonReport, BlockPrecedenceTemplate,
    MiningScenario, NumericMetricComparison, NumericMetricComparisonReport, NumericMetricTolerance,
    PhaseAssignment, PhaseTaggingReport, PrecedenceEdge, PrecedenceGraph,
    PrecedenceGraphComparisonReport, PrecedenceNode, PrecedenceOffset, PushbackGenerationRules,
    PushbackPrototype, PushbackPrototypeReport, ScenarioConstraints, ScenarioPeriod, ScenarioRules,
    Schedule, ScheduleConstraints, ScheduleEntry, SchedulePeriodSummary, ScheduleViolation,
    ScheduleViolationCode, UpitPrototypeReport, assign_benches, assign_phases_from_column,
    build_block_precedence_graph, build_pushback_prototype, build_schedule, build_upit_prototype,
    compare_block_memberships, compare_named_numeric_metrics, compare_precedence_graphs,
    compare_upit_reports, read_marvin_precedence_graph, read_marvin_upit_block_values,
    read_marvin_upit_solution, read_precedence_graph_json, validate_vertical_advance,
    write_precedence_graph_json,
};
pub use reblock::{
    AdaptiveReblockPrototype, AdaptiveResolutionStrategy, AdaptiveZonePrototype, AdaptiveZoneRule,
    AggregationOperation, AggregationRule, AggregationRules, CustomAggregationSpec,
    DistributionOperation, DistributionRule, DistributionRules, ReconciliationBlockCount,
    ReconciliationMetric, ReconciliationReport, ReconciliationTolerances, WeightedAggregation,
    aggregate_weighted_column, aggregate_weighted_values, build_adaptive_reblock_prototype,
    reconcile_models, subblock, superblock,
};
pub use validation::{
    BlockModelValidationExt, ValidationIssue, ValidationIssueCode, ValidationOptions,
    ValidationReport, ValidationSeverity, validate_block_model, validate_block_model_extents,
    validate_block_model_schema, validate_block_model_with_options,
    validate_duplicate_block_coordinates, validate_duplicate_block_indices,
};

/// Enumera las capas públicas mínimas disponibles para consumidores Rust.
#[must_use]
pub fn public_layers() -> [LayerDescriptor; 2] {
    [
        core_layer(),
        LayerDescriptor {
            name: "mine-sdk",
            responsibility: "API publica Rust que reexporta crates internos.",
        },
    ]
}
