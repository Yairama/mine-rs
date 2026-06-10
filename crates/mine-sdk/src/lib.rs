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
    ClassificationLevelAssessment, ClassificationMetricConfig, ClassificationMetricsReport,
    ClassificationThreshold, ColumnData, ColumnNullCount, ColumnSummary, CompositeContribution,
    CompositeDomainAuditIssue, CompositeDomainAuditIssueCode, CompositeDomainAuditReport,
    CompositeInterval, CompositeResidualPolicy, CompositeVsBlockReport, CompositingOptions,
    ConditionalRealization, ConditionalRealizationLineage, ConditionalRealizationSet,
    ContinuityMetrics, CrossValidationEntry, CrossValidationEstimator, CrossValidationMetrics,
    CrossValidationReport, DeclusteredSampleWeight, DeterministicEstimatorKind, DomainFilterReport,
    DomainMask, DomainWeightedStatistics, EstimateContribution, EstimationPass,
    EstimationPassEvaluation, EstimationPassSelection, ExperimentalVariogram,
    ExperimentalVariogramLag, ExperimentalVariogramLagRow, GradeTonnagePoint, GroupedStatistics,
    InformednessMetrics, IntervalSample, InverseDistanceWeightingOptions, KrigingEstimate,
    KrigingEstimatorKind, ModelSummary, NeighborhoodSample, NeighborhoodSelection, PassUsageMetric,
    PointEstimate, RealizationStorageFormat, RealizationSupport, SampleCountLimits,
    SampleSpacingMetrics, SearchAnisotropy, SearchNeighborhood,
    SequentialGaussianSimulationOptions, SequentialIndicatorSimulationOptions,
    SequentialSimulationEnsemble, SequentialSimulationRealization, SequentialSimulationSummary,
    SimpleKrigingOptions, SimulatedNodeValue, SimulationTarget, SpatialExtent, SpatialSample,
    SwathAxis, SwathBin, SwathDataPoint, SwathPlotReport, VariableStatistics, VariogramDirection,
    VariogramFitOptions, VariogramFitSummary, VariogramLagConfig, VariogramModel,
    VariogramModelKind, WeightedGradeStatistic, WeightedHistogram, WeightedHistogramBin,
    WeightedSampleStatistics, WeightedStatisticsReport, WeightedVariableSummary,
    audit_composite_domains, build_experimental_variogram, build_swath_plot,
    build_weighted_histogram, build_weighted_statistics_report, compare_composites_vs_blocks,
    composite_intervals, compute_block_to_block_covariance, compute_cell_declustering_weights,
    compute_point_to_block_covariance, cross_validate_leave_one_out,
    estimate_inverse_distance_weighting, estimate_nearest_neighbor, estimate_ordinary_kriging,
    estimate_simple_kriging, evaluate_classification_metrics, experimental_variogram_from_lag_rows,
    filter_interval_samples_by_domain_mask, fit_variogram_model,
    generate_sequential_gaussian_ensemble, generate_sequential_indicator_ensemble,
    regularize_block_support, select_samples_by_estimation_passes, select_samples_in_neighborhood,
};
pub use core::{
    ArtifactId, BlockDimensions, BlockId, ColumnId, ColumnLogicalType, ColumnMiningRole,
    ColumnSchema, ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, LayerDescriptor,
    MeasurementUnit, Metadata, MetadataValue, MineError, ModelId, RequiredColumn, ScenarioId,
    core_layer,
};
pub use economics::{
    BlockDestinationValue, BlockEconomicSummary, BlockEconomics, BlockEconomicsReport, BlockGrades,
    DestinationAssumptionSet, DestinationAssumptions, DestinationBlendReport, DestinationCapacity,
    DestinationId, DestinationKind, DestinationPayability, DestinationPurePhaseRefinement,
    DestinationPurePushbackPlan, DestinationRecovery, DirectDestinationFeed, EconomicAssumptions,
    EconomicBlockModel, EconomicBlockModelConfig, EconomicUnits, EvParameters, EvResult,
    LongTermScheduleEconomicsReport, LongTermSchedulePeriodEconomics,
    LongTermScheduleSensitivityCase, MaterialParcel, MultiDestinationBlockValuation, NsrMetalInput,
    NsrResult, PeriodCashflowInput, RiskMetricSummary, ScenarioCashflowReport, ScenarioComparison,
    ScenarioComparisonReport, ScenarioPeriodCashflow, ScenarioPeriodComparison, ScenarioRiskReport,
    StagedStockpileReclaimDownstreamProfile, StagedStockpileReclaimPolicy,
    StagedStockpileReclaimRule, StockpileBalanceReport, StockpileDefinition, StockpileDegradation,
    StockpileDeposit, StockpileId, StockpileInventorySnapshot, StockpilePeriodInput,
    StockpilePeriodReport, StockpilePlanInput, StockpilePlanReport, StockpileReclaim,
    StockpileSchedulingStage, StockpileTargetParcel, StockpileTransactionOrder,
    build_scheduling_problem_from_economic_block_model,
    build_scheduling_problem_from_economic_block_model_with_reclaim_policy, compute_ev,
    compute_nsr, evaluate_block_economics, evaluate_long_term_schedule_economics,
    evaluate_long_term_schedule_economics_with_reclaim_policy,
    evaluate_long_term_schedule_sensitivity_pack, evaluate_scenario_cashflow,
    evaluate_stockpile_plan, refine_pushback_plan_to_destination_pure,
    stage_pushback_plan_for_stockpile_readiness, summarize_long_term_schedule_risk,
    value_block_by_destinations,
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
    read_experimental_variogram_parquet, write_block_model_csv, write_block_model_parquet,
    write_block_model_vtu, write_block_model_vulcan_csv, write_experimental_variogram_json,
    write_experimental_variogram_parquet,
};
pub use planning::{
    BenchAssignment, BenchParameters, BlockMembershipComparisonReport, BlockPrecedenceTemplate,
    CpitToposortAssignment, CpitToposortOptions, CpitToposortProblem, CpitToposortSchedule,
    DecomposedSchedulingArtifacts, DecomposedSchedulingConfig, DecomposedTemporalSolver,
    LongTermSchedule, LongTermScheduleEntry, LongTermScheduleMaterialFlowReport,
    LongTermSchedulePeriodCapacity, LongTermSchedulePeriodFlow, LongTermScheduleStockpile,
    LongTermScheduleStockpileBalance, LongTermScheduleViolation, LongTermScheduleViolationCode,
    LongTermStockpileDepositPolicy, LongTermStockpilePolicy, LongTermStockpileReclaimPolicy,
    MaxClosureArc, MaxClosureArcKind, MaxClosureGraph, MaxClosureNodeId, MiningScenario,
    NestingAccessRules, NumericMetricComparison, NumericMetricComparisonReport,
    NumericMetricTolerance, PcpspToposortAssignment, PcpspToposortOptions, PcpspToposortProblem,
    PcpspToposortSchedule, PhaseAssignment, PhaseDesign, PhaseTaggingReport, PitShell,
    PitShellMetrics, PitShellSet, PrecedenceEdge, PrecedenceGraph, PrecedenceGraphComparisonReport,
    PrecedenceNode, PrecedenceOffset, PushbackGenerationRules, PushbackPlan, PushbackPrototype,
    PushbackPrototypeReport, ScenarioConstraints, ScenarioPeriod, ScenarioRules, Schedule,
    ScheduleConstraints, ScheduleDestinationCapacity, ScheduleDestinationId, ScheduleEntry,
    SchedulePeriodSummary, ScheduleStockpileCapacity, ScheduleStockpileId, ScheduleViolation,
    ScheduleViolationCode, SchedulingObjectiveTerm, SchedulingPeriod, SchedulingProblem,
    SchedulingResourceBound, SchedulingResourceId, SchedulingResourceRequirement, SchedulingUnit,
    SchedulingUnitId, SlopeAngleRule, SmallSchedulingAssignment, SmallSchedulingPeriodSummary,
    SmallSchedulingResourceUsage, SmallSchedulingSolution, UpitPrototypeReport, UplSolverResult,
    VariableSlopeTemplate, apply_long_term_stockpile_policy, assign_benches,
    assign_phases_from_column, build_aggregated_long_term_schedule, build_block_precedence_graph,
    build_max_closure_graph, build_pushback_prototype, build_ready_frontier_long_term_schedule,
    build_ready_frontier_schedule, build_schedule, build_target_period_seeded_long_term_schedule,
    build_target_period_seeded_schedule, build_target_period_windowed_long_term_schedule,
    build_target_period_windowed_schedule, build_upit_prototype, compare_block_memberships,
    compare_named_numeric_metrics, compare_precedence_graphs, compare_upit_reports,
    compute_pit_shell_metrics, derive_phase_design_from_nested_shells,
    derive_phase_design_from_nested_shells_from_map, derive_precedence_template_from_slope,
    derive_pushbacks_from_nested_shells, derive_pushbacks_from_nested_shells_from_map,
    evaluate_long_term_schedule_material_flows, generate_nested_shells,
    generate_nested_shells_from_model, generate_nested_shells_from_monotone_weight_scenarios,
    generate_nested_shells_from_weight_map, generate_nested_shells_from_weight_scenarios,
    read_pit_shell_set_json, read_precedence_graph_json, solve_cpit_with_toposort,
    solve_decomposed_scheduling_problem, solve_pcpsp_with_toposort, solve_small_scheduling_problem,
    solve_upl_exact, uniform_revenue_factors, validate_vertical_advance, verify_closure,
    write_pit_shell_set_json, write_precedence_graph_json,
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
