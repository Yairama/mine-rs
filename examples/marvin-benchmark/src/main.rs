//! Ejemplo ejecutable para comparar referencias Marvin locales contra salidas actuales de `mine-rs`.
//!
//! Uso:
//!   cargo run -p marvin-benchmark -- [--mode focused-mr187] [dataset_dir] [output_path]
//!
//! Si no se especifican argumentos, el dataset se toma desde `datasets/benchmarks/marvin/`
//! y el reporte se escribe en `datasets/benchmarks/marvin/outputs/comparison-report.json`.
//! Definir `MARVIN_BENCHMARK_PRINT_REPORT=1` replica el JSON a stdout solo cuando se necesite.

mod benchmark_blocks_support;
mod lp_bz_adapter;
mod lp_bz_bound;
mod lp_bz_lp_kernel;
mod lp_bz_rounder;
mod lp_bz_runtime_budget;
mod marvin_support;
mod minelib_scheduling_support;
mod pushback_bench_localized_cut_support;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use lp_bz_bound::{LpBzBoundArtifact, LpBzInputArtifact, compute_lp_bz_bound_artifacts};
use lp_bz_lp_kernel::{
    LpBzCutSolveDiagnostics, LpBzCutTighteningStrategy, LpBzLpKernelArtifact,
    LpBzLpKernelConstraintKind, LpBzLpKernelConstraintSense, LpBzLpSolveArtifact,
    LpBzLpSolveStatus, LpBzPrecedenceEnforcementStrategy, LpBzPrecedenceSolveDiagnostics,
    build_lp_bz_lp_kernel_artifact, solve_lp_bz_lp_kernel_artifact,
};
use lp_bz_rounder::{
    build_target_period_seeded_schedule_from_lp_round_repair_v6,
    build_target_period_seeded_schedule_from_lp_round_repair_v6_focused,
};
use marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleSolution,
    MarvinScheduleSolutionSummary, read_marvin_cpit_problem, read_marvin_cpit_solution,
    read_marvin_lp_cpit_solution, read_marvin_lp_pcpsp_solution, read_marvin_pcpsp_problem,
    read_marvin_pcpsp_solution, read_marvin_precedence_graph, read_marvin_upit_block_values,
    read_marvin_upit_solution, summarize_marvin_schedule_solution,
};
use mine_sdk::{
    BlockModel, BlockPrecedenceTemplate, ColumnData, ColumnId, DecomposedSchedulingConfig,
    DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
    DestinationKind, DestinationPayability, DestinationRecovery, EconomicBlockModel,
    EconomicBlockModelConfig, LongTermScheduleEconomicsReport, MeasurementUnit, Metadata,
    MetadataValue, MineError, ModelId, NestingAccessRules, NumericMetricComparisonReport,
    NumericMetricTolerance, PhaseDesign, PrecedenceNode, PrecedenceOffset, PushbackPlan,
    ScenarioId, ScheduleDestinationId, SchedulingObjectiveTerm, SchedulingPeriod,
    SchedulingProblem, SchedulingResourceBound, SchedulingResourceId,
    SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId, build_block_precedence_graph,
    build_max_closure_graph, build_target_period_seeded_long_term_schedule,
    build_target_period_windowed_long_term_schedule, build_upit_prototype,
    compare_block_memberships, compare_named_numeric_metrics,
    evaluate_long_term_schedule_economics, linear_to_ijk, solve_decomposed_scheduling_problem,
    solve_upl_exact, uniform_revenue_factors,
};
use minelib_scheduling_support::{
    build_linear_index_float_lookup, build_marvin_phase_plan_from_revenue_factor_shells,
    build_marvin_preferred_nested_shell_family_contract_for_phase_plan,
};
use pushback_bench_localized_cut_support::{
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
    PushbackBenchLocalizedCutBuildArtifacts, PushbackBenchLocalizedCutBuildConfig,
    PushbackBenchLocalizedCutFrontProgression, PushbackBenchLocalizedCutPredecessorLinkPolicy,
    PushbackBenchLocalizedCutRefinementDiagnostics,
    PushbackBenchLocalizedCutUnitFamilyTraceability,
    build_promoted_pushback_bench_localized_cut_unit_family_traceability,
    build_pushback_bench_localized_cut_benchmark_artifacts,
    format_promoted_lp_bz_family_status_summary,
    format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary,
};
use serde::Serialize;

const OFFICIAL_CPIT_OBJECTIVE: f64 = 820_726_048.0;
const OFFICIAL_LP_CPIT_OBJECTIVE: f64 = 863_916_131.0;
const OFFICIAL_PCPSP_OBJECTIVE: f64 = 885_968_070.0;
const OFFICIAL_LP_PCPSP_OBJECTIVE: f64 = 911_704_665.0;
const LP_WINDOW_CANDIDATE_SIZE: usize = 18;
const LP_CUT_PERIOD_BAND_WIDTH: usize = 3;
const LP_CUT_BAND_SWEEP_WIDTHS: [usize; 4] = [1, 2, 3, 4];
const GEOMETRIC_COMPONENT_STRIPE_COUNT: usize = 2;
const DIRECTIONAL_FRONT_BAND_COUNT: usize = 2;
const ADAPTIVE_COMPONENT_FRONT_COUNT: usize = 2;
const ADAPTIVE_COMPONENT_FRONT_MIN_SHARE: f64 = 0.5;
const ADAPTIVE_COMPONENT_FRONT_SHARE_SWEEP: [f64; 5] = [0.35, 0.5, 0.65, 0.8, 0.95];
const SHAPE_GATED_FRONT_COUNT: usize = 3;
const SHAPE_GATED_FRONT_MIN_ASPECT_RATIO: f64 = 2.0;
const SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN: usize = 2;
const SHAPE_GATED_FRONT_ASPECT_RATIO_SWEEP: [f64; 6] = [1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
const SHAPE_GATED_FRONT_DOMINANT_SPAN_SWEEP: [usize; 4] = [1, 2, 3, 4];
const SHAPE_GATED_FRONT_COUNT_SWEEP: [usize; 3] = [2, 3, 4];
const SHAPE_GATED_LOCAL_FRONT_COUNT_SWEEP: [usize; 4] = [2, 3, 4, 5];
const SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW: usize = 4;
const SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT: usize = 4;
const SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_ASPECT_RATIO: f64 = 3.0;
const SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_DOMINANT_SPAN: usize = 4;
const SHAPE_GATED_FRONT_PROGRESSION_UNIFORM_33_67_100: FrontProgressionProfileContract =
    FrontProgressionProfileContract {
        label: "uniform-33-67-100",
        cumulative_tonnage_targets: [1.0 / 3.0, 2.0 / 3.0, 1.0],
    };
const SHAPE_GATED_FRONT_PROGRESSION_FRONT_LOADED_45_80_100: FrontProgressionProfileContract =
    FrontProgressionProfileContract {
        label: "front-loaded-45-80-100",
        cumulative_tonnage_targets: [0.45, 0.80, 1.0],
    };
const SHAPE_GATED_FRONT_PROGRESSION_FRONT_LOADED_55_85_100: FrontProgressionProfileContract =
    FrontProgressionProfileContract {
        label: "front-loaded-55-85-100",
        cumulative_tonnage_targets: [0.55, 0.85, 1.0],
    };
const FOCUSED_MR187_INPUT_AGGREGATION_PROVENANCE_CHAIN: &str = "selected_block_source + selected_block_count -> preferred phase-plan / preferred nested-shell proxy -> localized-cut builder -> promoted family";
const MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE: FrontProgressionProfileContract =
    SHAPE_GATED_FRONT_PROGRESSION_UNIFORM_33_67_100;
const LP_BZ_LOCAL_FRONT_COUNT: usize = SHAPE_GATED_FRONT_COUNT;
const LP_BZ_LOCAL_ACCESS_WINDOW_COUNT: usize = SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT;
const LP_BZ_LOCAL_RULE_MIN_ASPECT_RATIO: f64 = SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_ASPECT_RATIO;
const LP_BZ_LOCAL_RULE_MIN_DOMINANT_SPAN: usize = SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_DOMINANT_SPAN;
const LP_BZ_UNIT_GRANULARITY_LABEL: &str = "shape-gated-local-front-phase";
const LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH: usize = 3;
const LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_SWEEP_WIDTHS: [usize; 4] = [1, 2, 3, 4];
const LP_BZ_V9_UNIT_GRANULARITY_LABEL: &str = "shape-gated-local-front-period-band-phase";
const PUSHBACK_BENCH_LOCALIZED_CUT_MIN_ASPECT_RATIO: f64 = 2.0;
const PUSHBACK_BENCH_LOCALIZED_CUT_MIN_DOMINANT_SPAN: usize = 2;
const PUSHBACK_BENCH_LOCALIZED_CUT_LOCAL_PREDECESSOR_COUNT: usize = 4;
const PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL: &str =
    "pushback-bench-localized-cut-phase";
const PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL: &str = "pushback-bench-localized-mining-cuts";
const PUSHBACK_BENCH_LOCALIZED_CUT_FOCUSED_SWEEP: [PushbackBenchLocalizedCutSweepConfig; 6] = [
    PushbackBenchLocalizedCutSweepConfig {
        label: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
        build_config: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
    },
    PushbackBenchLocalizedCutSweepConfig {
        label: "front2-ar2.0-span2-n4",
        build_config: PushbackBenchLocalizedCutBuildConfig {
            max_front_count: 2,
            min_aspect_ratio: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_ASPECT_RATIO,
            min_dominant_span: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_DOMINANT_SPAN,
            include_touching_neighbors: true,
            max_local_predecessor_count: Some(PUSHBACK_BENCH_LOCALIZED_CUT_LOCAL_PREDECESSOR_COUNT),
            predecessor_cut_link_policy:
                PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            front_progression: PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        },
    },
    PushbackBenchLocalizedCutSweepConfig {
        label: "front4-ar2.0-span2-n4",
        build_config: PushbackBenchLocalizedCutBuildConfig {
            max_front_count: 4,
            min_aspect_ratio: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_ASPECT_RATIO,
            min_dominant_span: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_DOMINANT_SPAN,
            include_touching_neighbors: true,
            max_local_predecessor_count: Some(PUSHBACK_BENCH_LOCALIZED_CUT_LOCAL_PREDECESSOR_COUNT),
            predecessor_cut_link_policy:
                PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            front_progression: PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        },
    },
    PushbackBenchLocalizedCutSweepConfig {
        label: "front3-ar2.0-span2-n3",
        build_config: PushbackBenchLocalizedCutBuildConfig {
            max_front_count: SHAPE_GATED_FRONT_COUNT,
            min_aspect_ratio: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_ASPECT_RATIO,
            min_dominant_span: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_DOMINANT_SPAN,
            include_touching_neighbors: true,
            max_local_predecessor_count: Some(3),
            predecessor_cut_link_policy:
                PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            front_progression: PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        },
    },
    PushbackBenchLocalizedCutSweepConfig {
        label: "front3-ar2.0-span2-n5",
        build_config: PushbackBenchLocalizedCutBuildConfig {
            max_front_count: SHAPE_GATED_FRONT_COUNT,
            min_aspect_ratio: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_ASPECT_RATIO,
            min_dominant_span: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_DOMINANT_SPAN,
            include_touching_neighbors: true,
            max_local_predecessor_count: Some(5),
            predecessor_cut_link_policy:
                PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            front_progression: PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        },
    },
    PushbackBenchLocalizedCutSweepConfig {
        label: "front3-ar2.0-span2-n6",
        build_config: PushbackBenchLocalizedCutBuildConfig {
            max_front_count: SHAPE_GATED_FRONT_COUNT,
            min_aspect_ratio: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_ASPECT_RATIO,
            min_dominant_span: PUSHBACK_BENCH_LOCALIZED_CUT_MIN_DOMINANT_SPAN,
            include_touching_neighbors: true,
            max_local_predecessor_count: Some(6),
            predecessor_cut_link_policy:
                PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            front_progression: PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        },
    },
];
const SHAPE_GATED_DYNAMIC_LOCAL_WINDOW_SWEEP: [(&str, f64, usize); 4] = [
    ("aspect-2.5-n5", 2.5, 5),
    ("aspect-2.5-n6", 2.5, 6),
    ("aspect-3.0-n6", 3.0, 6),
    ("aspect-3.0-n8", 3.0, 8),
];
const SHAPE_GATED_LOCAL_ACCESS_TOUCHING_SWEEP: [bool; 2] = [true, false];
const SHAPE_GATED_FRONT_PROGRESSION_SWEEP: [FrontProgressionProfileContract; 3] = [
    SHAPE_GATED_FRONT_PROGRESSION_UNIFORM_33_67_100,
    SHAPE_GATED_FRONT_PROGRESSION_FRONT_LOADED_45_80_100,
    SHAPE_GATED_FRONT_PROGRESSION_FRONT_LOADED_55_85_100,
];
const SHAPE_GATED_CONDITIONAL_PROGRESSIVE_PROFILE: [f64; 3] = [0.45, 0.80, 1.0];
const SHAPE_GATED_CONDITIONAL_PROGRESSIVE_ASPECT_SWEEP: [f64; 3] = [2.5, 3.0, 3.5];
const SHAPE_GATED_LOCAL_PREDECESSOR_WINDOW_SWEEP: [usize; 6] = [1, 2, 3, 4, 5, 6];
const MARVIN_END_TO_END_FACTOR_COUNT: usize = 7;
const MARVIN_FACTOR_SWEEP_COUNTS: [usize; 4] = [3, 5, 7, 9];
const LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT: usize = 8;
const LP_BZ_KERNEL_REPORT_ROW_KIND_SAMPLE_LIMIT: usize = 2;
const MARVIN_BENCHMARK_MODE_ENV: &str = "MARVIN_BENCHMARK_MODE";
const MARVIN_BENCHMARK_FULL_REPORT_FILE: &str = "comparison-report.json";
const MARVIN_BENCHMARK_FOCUSED_MR187_REPORT_FILE: &str = "mr187-focused-refresh-report.json";
const MARVIN_BENCHMARK_FULL_MODE_LABEL: &str = "full";
const MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL: &str = "focused-mr187";

#[derive(Debug, Serialize)]
struct MarvinBenchmarkOutput {
    dataset_dir: String,
    reference_prec_path: String,
    reference_upit_solution_path: String,
    reference_upit_objective_path: String,
    reference_cpit_problem_path: String,
    reference_cpit_solution_path: String,
    reference_pcpsp_problem_path: String,
    reference_pcpsp_solution_path: String,
    reference_lp_cpit_solution_path: String,
    reference_lp_pcpsp_solution_path: String,
    value_column: String,
    tonnage_column: String,
    candidate_predecessor_offsets: Vec<(isize, isize, isize)>,
    reference_precedence: PrecedenceArtifactSummary,
    candidate_precedence: PrecedenceArtifactSummary,
    precedence_comparison: CompactPrecedenceComparison,
    reference_upit: MembershipArtifactSummary,
    candidate_upit: MembershipArtifactSummary,
    exact_upit: MembershipArtifactSummary,
    upit_membership_comparison: CompactMembershipComparison,
    exact_upit_membership_comparison: CompactMembershipComparison,
    upit_metric_comparison: NumericMetricComparisonReport,
    exact_upit_metric_comparison: NumericMetricComparisonReport,
    cpit_reference: ScheduleReferenceArtifactSummary,
    pcpsp_reference: ScheduleReferenceArtifactSummary,
    lp_cpit_reference: ScheduleReferenceArtifactSummary,
    lp_pcpsp_reference: ScheduleReferenceArtifactSummary,
    lp_bz_inputs: LpBzInputArtifact,
    lp_bz_bound_artifact: LpBzBoundArtifact,
    lp_bz_lp_kernel_artifact: CompactLpBzLpKernelArtifact,
    lp_bz_lp_solve_artifact: LpBzLpSolveArtifact,
    lp_bz_integer_candidate_artifact: SchedulingBaselineSummary,
    lp_bz_rounder_v6_local_optimizer_diagnostics: LpBzRounderV6LocalOptimizerDiagnostics,
    lp_bz_gap_metrics: LpBzGapMetrics,
    lp_shell_seeded_baseline: SchedulingBaselineSummary,
    lp_target_period_seeded_baseline: SchedulingBaselineSummary,
    lp_staggered_target_seeded_baseline: SchedulingBaselineSummary,
    lp_windowed_exact_baseline: SchedulingBaselineSummary,
    lp_cut_target_seeded_baseline: SchedulingBaselineSummary,
    lp_quantile_cut_target_seeded_baseline: SchedulingBaselineSummary,
    geometric_component_target_seeded_baseline: SchedulingBaselineSummary,
    geometric_local_component_target_seeded_baseline: SchedulingBaselineSummary,
    geometric_component_stripe_target_seeded_baseline: SchedulingBaselineSummary,
    directional_front_band_target_seeded_baseline: SchedulingBaselineSummary,
    directional_local_front_band_target_seeded_baseline: SchedulingBaselineSummary,
    adaptive_component_front_target_seeded_baseline: SchedulingBaselineSummary,
    adaptive_component_front_threshold_sweep: Vec<AdaptiveComponentFrontSweepEntry>,
    shape_gated_front_target_seeded_baseline: SchedulingBaselineSummary,
    shape_gated_local_front_target_seeded_baseline: SchedulingBaselineSummary,
    shape_gated_front_rule_sweep: Vec<ShapeGatedFrontSweepEntry>,
    shape_gated_local_rule_window_sweep: Vec<ShapeGatedFrontSweepEntry>,
    shape_gated_local_rule_front_count_sweep: Vec<ShapeGatedFrontCountSweepEntry>,
    shape_gated_front_count_sweep: Vec<ShapeGatedFrontCountSweepEntry>,
    shape_gated_local_front_count_sweep: Vec<ShapeGatedFrontCountSweepEntry>,
    shape_gated_local_overlap_front_count_sweep: Vec<ShapeGatedFrontCountSweepEntry>,
    shape_gated_local_access_sweep: Vec<ShapeGatedLocalAccessSweepEntry>,
    shape_gated_local_access_window_sweep: Vec<ShapeGatedLocalAccessSweepEntry>,
    shape_gated_front_progression_sweep: Vec<ShapeGatedFrontProgressionSweepEntry>,
    shape_gated_front_progression_window_sweep: Vec<ShapeGatedFrontProgressionSweepEntry>,
    shape_gated_conditional_progression_sweep: Vec<ShapeGatedConditionalProgressionSweepEntry>,
    shape_gated_conditional_window_progression_sweep:
        Vec<ShapeGatedConditionalProgressionSweepEntry>,
    shape_gated_local_window_sweep: Vec<ShapeGatedLocalWindowSweepEntry>,
    shape_gated_dynamic_local_window_sweep: Vec<ShapeGatedDynamicLocalWindowSweepEntry>,
    strict_shell_factor_sweep: Vec<ShellFactorSweepEntry>,
    shell_access_sweep: Vec<ShellAccessSweepEntry>,
    lp_cut_band_width_sweep: Vec<LpCutBandSweepEntry>,
    mine_rs_end_to_end: MineRsEndToEndSummary,
    mine_rs_vs_cpit_metric_comparison: NumericMetricComparisonReport,
    mine_rs_vs_cpit_membership_comparison: CompactPeriodMembershipComparison,
    mine_rs_vs_cpit_period_metric_comparison: NumericMetricComparisonReport,
    mine_rs_vs_pcpsp_metric_comparison: NumericMetricComparisonReport,
    mine_rs_vs_pcpsp_membership_comparison: CompactPeriodMembershipComparison,
    mine_rs_vs_pcpsp_period_metric_comparison: NumericMetricComparisonReport,
    assumptions: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FocusedMr187RefreshOutput {
    report_mode: String,
    dataset_dir: String,
    reference_prec_path: String,
    reference_pcpsp_problem_path: String,
    reference_pcpsp_solution_path: String,
    reference_lp_pcpsp_solution_path: String,
    value_column: String,
    tonnage_column: String,
    pcpsp_reference: ScheduleReferenceArtifactSummary,
    lp_pcpsp_reference: ScheduleReferenceArtifactSummary,
    lp_bz_inputs: LpBzInputArtifact,
    lp_bz_bound_artifact: LpBzBoundArtifact,
    lp_bz_lp_kernel_artifact: CompactLpBzLpKernelArtifact,
    lp_bz_lp_solve_artifact: LpBzLpSolveArtifact,
    lp_bz_front_progression_label: String,
    lp_bz_integer_candidate_artifact: SchedulingBaselineSummary,
    lp_bz_rounder_v6_local_optimizer_diagnostics: LpBzRounderV6LocalOptimizerDiagnostics,
    lp_bz_gap_metrics: LpBzGapMetrics,
    lp_bz_pushback_bench_localized_cut_experiment: FocusedPushbackBenchLocalizedCutExperiment,
    lp_bz_v9_local_front_band_experiment: FocusedLpBzVariantExperiment,
    lp_bz_v9_local_front_band_width_sweep: Vec<LpBzLocalFrontBandWidthSweepEntry>,
    lp_bz_v9_local_front_band_link_policy_sweep: Vec<LpBzLocalFrontBandLinkPolicySweepEntry>,
    comparison_classification: String,
    comparability_gaps: Vec<String>,
    assumptions: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FocusedLpBzVariantExperiment {
    unit_granularity_label: String,
    predecessor_cut_link_policy: String,
    front_progression_label: String,
    lp_bz_inputs: LpBzInputArtifact,
    lp_bz_bound_artifact: LpBzBoundArtifact,
    lp_bz_integer_candidate_artifact: SchedulingBaselineSummary,
    lp_bz_rounder_v6_local_optimizer_diagnostics: LpBzRounderV6LocalOptimizerDiagnostics,
    lp_bz_gap_metrics: LpBzGapMetrics,
    phase_refinement_diagnostics: LpBzPeriodBandRefinementDiagnostics,
}

#[derive(Debug, Serialize)]
struct FocusedPushbackBenchLocalizedCutExperiment {
    builder_label: String,
    calibrated_candidate_label: String,
    first_builder_point_label: String,
    best_sweep_candidate_label: String,
    unit_granularity_label: String,
    unit_family_traceability: PushbackBenchLocalizedCutUnitFamilyTraceability,
    input_aggregation_traceability_summary: String,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    localized_access_mode: String,
    max_local_predecessor_count: usize,
    first_builder_point_discounted_objective: f64,
    best_sweep_candidate_vs_first_builder_point_objective_delta: f64,
    phase_count_delta_vs_v8_local_front: isize,
    candidate_vs_v8_local_front_objective_delta: f64,
    calibration_sweep: Vec<PushbackBenchLocalizedCutSweepEntry>,
    lp_bz_inputs: LpBzInputArtifact,
    lp_bz_bound_artifact: LpBzBoundArtifact,
    lp_bz_integer_candidate_artifact: SchedulingBaselineSummary,
    lp_bz_rounder_v6_local_optimizer_diagnostics: LpBzRounderV6LocalOptimizerDiagnostics,
    lp_bz_gap_metrics: LpBzGapMetrics,
    phase_refinement_diagnostics: PushbackBenchLocalizedCutRefinementDiagnostics,
}

#[derive(Debug, Serialize)]
struct PushbackBenchLocalizedCutSweepEntry {
    candidate_label: String,
    is_first_builder_point: bool,
    is_best_candidate: bool,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    localized_access_mode: String,
    max_local_predecessor_count: usize,
    phase_count: usize,
    unit_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    candidate_vs_first_builder_point_objective_delta: f64,
    candidate_vs_v8_local_front_objective_delta: f64,
    candidate_vs_pcpsp_reference_objective_gap: f64,
    bound_to_candidate_relative_gap: f64,
    repaired_phase_target_count: usize,
    repaired_unit_target_count: usize,
    used_period_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LpBzBandPredecessorLinkPolicy {
    PredecessorLastCut,
    PredecessorFirstCut,
    AllPredecessorCuts,
}

impl LpBzBandPredecessorLinkPolicy {
    fn label(self) -> &'static str {
        match self {
            Self::PredecessorLastCut => "predecessor-last-cut",
            Self::PredecessorFirstCut => "predecessor-first-cut",
            Self::AllPredecessorCuts => "all-predecessor-cuts",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarvinBenchmarkMode {
    Full,
    FocusedMr187,
}

impl MarvinBenchmarkMode {
    fn parse(value: &str, source: &str) -> Result<Self, MineError> {
        match value.trim() {
            MARVIN_BENCHMARK_FULL_MODE_LABEL => Ok(Self::Full),
            MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL => Ok(Self::FocusedMr187),
            other => Err(MineError::validation(format!(
                "Unsupported Marvin benchmark mode `{other}` from {source}. Expected `{MARVIN_BENCHMARK_FULL_MODE_LABEL}` or `{MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL}`."
            ))),
        }
    }

    fn report_mode_label(self) -> &'static str {
        match self {
            Self::Full => MARVIN_BENCHMARK_FULL_MODE_LABEL,
            Self::FocusedMr187 => MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL,
        }
    }

    fn default_output_file_name(self) -> &'static str {
        match self {
            Self::Full => MARVIN_BENCHMARK_FULL_REPORT_FILE,
            Self::FocusedMr187 => MARVIN_BENCHMARK_FOCUSED_MR187_REPORT_FILE,
        }
    }
}

#[derive(Debug)]
struct MarvinBenchmarkCli {
    mode: MarvinBenchmarkMode,
    dataset_dir: PathBuf,
    output_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct PrecedenceArtifactSummary {
    node_count: usize,
    edge_count: usize,
}

#[derive(Debug, Serialize)]
struct MembershipArtifactSummary {
    block_count: usize,
    /// Suma de proc_profit × tonelaje para los bloques seleccionados.
    total_proc_profit_x_tonnage: f64,
    /// Objetivo económico UPIT: sum((max(proc_profit, 0) - mine_cost) × tonnage).
    total_economic_objective: f64,
    total_tonnage: f64,
}

#[derive(Debug, Serialize)]
struct CompactPrecedenceComparison {
    shared_nodes: usize,
    shared_edges: usize,
    reference_only_edge_count: usize,
    candidate_only_edge_count: usize,
    node_jaccard_index: f64,
    edge_jaccard_index: f64,
    reference_only_edge_examples: Vec<(usize, usize)>,
    candidate_only_edge_examples: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize)]
struct CompactMembershipComparison {
    shared_blocks: usize,
    reference_only_block_count: usize,
    candidate_only_block_count: usize,
    jaccard_index: f64,
    reference_only_block_examples: Vec<usize>,
    candidate_only_block_examples: Vec<usize>,
}

#[derive(Debug, Serialize)]
struct ScheduleReferenceArtifactSummary {
    period_count: usize,
    destination_count: usize,
    resource_constraint_count: usize,
    discount_rate: f64,
    official_objective: f64,
    objective_gap_vs_official: f64,
    solution_summary: MarvinScheduleSolutionSummary,
}

#[derive(Debug, Serialize)]
struct LpBzGapMetrics {
    effective_discounted_objective_bound: f64,
    effective_bound_source: String,
    native_lp_kernel_discounted_objective_bound: Option<f64>,
    bound_to_candidate_absolute_gap: f64,
    bound_to_candidate_relative_gap: f64,
    candidate_vs_pcpsp_reference_objective_gap: f64,
    candidate_vs_ready_frontier_objective_gap: f64,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelArtifact {
    kernel_label: String,
    period_count: usize,
    unit_count: usize,
    destination_count: usize,
    discount_rate: f64,
    variable_index: CompactLpBzLpKernelVariableIndexArtifact,
    objective: CompactLpBzLpKernelObjectiveArtifact,
    constraints: CompactLpBzLpKernelConstraintArtifact,
    access: CompactLpBzLpKernelAccessArtifact,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelVariableIndexArtifact {
    variable_count: usize,
    sampled_entry_count: usize,
    omitted_entry_count: usize,
    period_labels: Vec<String>,
    destination_ids: Vec<String>,
    unit_id_examples: Vec<String>,
    sample_entries: Vec<CompactLpBzLpKernelVariableEntry>,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelVariableEntry {
    variable_index: usize,
    unit_id: String,
    destination_id: String,
    period_index: usize,
    period_label: String,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelObjectiveArtifact {
    coefficient_count: usize,
    non_zero_coefficient_count: usize,
    sampled_coefficient_count: usize,
    omitted_coefficient_count: usize,
    min_coefficient: Option<f64>,
    max_coefficient: Option<f64>,
    min_discount_factor: Option<f64>,
    max_discount_factor: Option<f64>,
    sample_coefficients: Vec<CompactLpBzLpKernelObjectiveCoefficient>,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelObjectiveCoefficient {
    variable_index: usize,
    unit_id: String,
    destination_id: String,
    period_label: String,
    coefficient: f64,
    undiscounted_value: f64,
    discount_factor: f64,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelConstraintArtifact {
    row_count: usize,
    capacity_row_count: usize,
    activation_row_count: usize,
    precedence_row_count: usize,
    sampled_row_count: usize,
    omitted_row_count: usize,
    total_term_count: usize,
    max_term_count: usize,
    period_labels: Vec<String>,
    resource_ids: Vec<String>,
    sample_rows: Vec<CompactLpBzLpKernelConstraintRow>,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelConstraintRow {
    row_index: usize,
    row_id: String,
    kind: LpBzLpKernelConstraintKind,
    sense: LpBzLpKernelConstraintSense,
    rhs: f64,
    period_index: Option<usize>,
    period_label: Option<String>,
    resource_id: Option<String>,
    unit_id: Option<String>,
    predecessor_unit_id: Option<String>,
    successor_unit_id: Option<String>,
    term_count: usize,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelAccessArtifact {
    unit_profile_count: usize,
    sampled_profile_count: usize,
    omitted_profile_count: usize,
    max_direct_predecessor_count: usize,
    max_transitive_predecessor_count: usize,
    max_closure_unit_count: usize,
    closure_resource_ids: Vec<String>,
    sample_unit_profiles: Vec<CompactLpBzLpKernelAccessUnitProfile>,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelAccessUnitProfile {
    unit_id: String,
    bench: Option<i64>,
    shell_index: Option<usize>,
    direct_predecessor_count: usize,
    transitive_predecessor_count: usize,
    closure_unit_count: usize,
    closure_resource_count: usize,
    closure_resources: Vec<CompactLpBzLpKernelAccessClosureResource>,
}

#[derive(Debug, Serialize)]
struct CompactLpBzLpKernelAccessClosureResource {
    resource_id: String,
    minimum_total_requirement: f64,
}

#[derive(Debug, Serialize)]
struct SchedulingBaselineSummary {
    baseline_name: String,
    phase_count: usize,
    candidate_pcpsp_summary: MarvinScheduleSolutionSummary,
    candidate_vs_reference_metrics: NumericMetricComparisonReport,
    candidate_vs_reference_membership_comparison: CompactPeriodMembershipComparison,
}

#[derive(Debug, Serialize)]
struct LpBzPeriodBandRefinementDiagnostics {
    period_band_width: usize,
    localized_front_phase_count: usize,
    refined_localized_front_phase_count: usize,
    total_period_band_phase_count: usize,
    additional_phase_count: usize,
    max_cut_count_per_localized_front: usize,
    average_cut_count_per_localized_front: f64,
    refined_localized_front_examples: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LpBzRounderV6LocalOptimizerDiagnostics {
    rounder_strategy_label: String,
    local_optimizer_strategy_label: String,
    local_optimizer_max_iteration_count: usize,
    local_optimizer_executed_iteration_count: usize,
    local_optimizer_improving_move_count: usize,
    local_optimizer_termination_reason: String,
    repaired_phase_target_count: usize,
    repaired_unit_target_count: usize,
    horizon_clamp_count: usize,
    phase_target_count: usize,
    unit_target_count: usize,
}

#[derive(Debug, Serialize)]
struct ShellFactorSweepEntry {
    factor_count: usize,
    shell_count: usize,
    phase_count: usize,
    schedule_npv: f64,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShellAccessSweepEntry {
    access_policy_label: String,
    factor_count: usize,
    shell_count: usize,
    phase_count: usize,
    schedule_npv: f64,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct LpCutBandSweepEntry {
    period_band_width: usize,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct LpBzLocalFrontBandWidthSweepEntry {
    period_band_width: usize,
    phase_count: usize,
    refined_localized_front_phase_count: usize,
    additional_phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    candidate_vs_pcpsp_reference_objective_gap: f64,
    bound_to_candidate_relative_gap: f64,
    repaired_phase_target_count: usize,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct LpBzLocalFrontBandLinkPolicySweepEntry {
    predecessor_cut_link_policy: String,
    period_band_width: usize,
    phase_count: usize,
    direct_predecessor_edge_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    candidate_vs_pcpsp_reference_objective_gap: f64,
    bound_to_candidate_relative_gap: f64,
    repaired_phase_target_count: usize,
    repaired_unit_target_count: usize,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct AdaptiveComponentFrontSweepEntry {
    min_component_share: f64,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedFrontSweepEntry {
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedFrontCountSweepEntry {
    max_front_count: usize,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedLocalAccessSweepEntry {
    access_mode_label: String,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedFrontProgressionSweepEntry {
    progression_label: String,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedConditionalProgressionSweepEntry {
    min_progression_aspect_ratio: f64,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedLocalWindowSweepEntry {
    max_local_predecessor_count: usize,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

#[derive(Debug, Serialize)]
struct ShapeGatedDynamicLocalWindowSweepEntry {
    window_rule_label: String,
    min_dynamic_window_aspect_ratio: f64,
    promoted_local_predecessor_count: usize,
    phase_count: usize,
    candidate_pcpsp_discounted_objective: f64,
    used_period_count: usize,
}

struct LpBzBandRefinementArtifacts {
    benchmark: LpBzBenchmarkArtifacts,
    phase_refinement_diagnostics: LpBzPeriodBandRefinementDiagnostics,
}

#[derive(Debug, Clone, Copy)]
struct PushbackBenchLocalizedCutSweepConfig {
    label: &'static str,
    build_config: PushbackBenchLocalizedCutBuildConfig,
}

struct FocusedPushbackBenchLocalizedCutSweepBuild {
    config: PushbackBenchLocalizedCutSweepConfig,
    benchmark: PushbackBenchLocalizedCutBuildArtifacts<SchedulingProblem>,
    lp_bz_inputs: LpBzInputArtifact,
    lp_bz_bound_artifact: LpBzBoundArtifact,
    lp_bz_integer_candidate_artifact: SchedulingBaselineSummary,
    lp_bz_rounder_v6_local_optimizer_diagnostics: LpBzRounderV6LocalOptimizerDiagnostics,
    lp_bz_gap_metrics: LpBzGapMetrics,
}

#[derive(Debug, Serialize)]
struct MineRsEndToEndSummary {
    phase_count: usize,
    total_block_count: usize,
    schedule_period_count: usize,
    schedule_entry_count: usize,
    schedule_violation_count: usize,
    total_tonnage: f64,
    total_cashflow: f64,
    npv: f64,
    periods: Vec<MineRsPeriodSummary>,
}

#[derive(Debug, Serialize)]
struct MineRsPeriodSummary {
    period_label: String,
    tonnage: f64,
    cashflow: f64,
    discounted_cashflow: f64,
}

#[derive(Debug, Serialize)]
struct CompactPeriodMembershipComparison {
    shared_assignments: usize,
    reference_only_assignment_count: usize,
    candidate_only_assignment_count: usize,
    jaccard_index: f64,
    reference_only_assignment_examples: Vec<(String, usize)>,
    candidate_only_assignment_examples: Vec<(String, usize)>,
}

fn parse_marvin_benchmark_cli(repo_root: &Path) -> Result<MarvinBenchmarkCli, MineError> {
    let env_mode = env::var(MARVIN_BENCHMARK_MODE_ENV).ok();
    parse_marvin_benchmark_cli_args(repo_root, env_mode.as_deref(), env::args_os().skip(1))
}

fn parse_marvin_benchmark_cli_args<I, S>(
    repo_root: &Path,
    env_mode: Option<&str>,
    args: I,
) -> Result<MarvinBenchmarkCli, MineError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut mode = match env_mode {
        Some(value) => MarvinBenchmarkMode::parse(value, MARVIN_BENCHMARK_MODE_ENV)?,
        None => MarvinBenchmarkMode::Full,
    };
    let mut positional_args = Vec::new();
    let mut args = args.into_iter().map(Into::into);

    while let Some(arg) = args.next() {
        let arg_text = arg.to_string_lossy();
        if let Some(value) = arg_text.strip_prefix("--mode=") {
            mode = MarvinBenchmarkMode::parse(value, "--mode")?;
            continue;
        }
        if arg_text == "--mode" {
            let value = args.next().ok_or_else(|| {
                MineError::validation(
                    "Expected a mode value after `--mode` (`full` or `focused-mr187`).".to_owned(),
                )
            })?;
            mode = MarvinBenchmarkMode::parse(&value.to_string_lossy(), "--mode")?;
            continue;
        }
        positional_args.push(PathBuf::from(arg));
    }

    if positional_args.len() > 2 {
        return Err(MineError::validation(format!(
            "Expected at most 2 positional arguments (`dataset_dir` and `output_path`), received {}.",
            positional_args.len()
        )));
    }

    let dataset_dir = positional_args
        .first()
        .cloned()
        .unwrap_or_else(|| repo_root.join("datasets").join("benchmarks").join("marvin"));
    let output_path = positional_args.get(1).cloned().unwrap_or_else(|| {
        dataset_dir
            .join("outputs")
            .join(mode.default_output_file_name())
    });

    Ok(MarvinBenchmarkCli {
        mode,
        dataset_dir,
        output_path,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let cli = parse_marvin_benchmark_cli(&repo_root)?;
    let dataset_dir = cli.dataset_dir;
    let output_path = cli.output_path;
    let blocks_path = dataset_dir.join("marvin.blocks");
    let references_dir = dataset_dir.join("references");
    let prec_path = references_dir.join("marvin.prec");
    let upit_solution_path = references_dir.join("marvin_upit.sol");
    let upit_objective_path = references_dir.join("marvin.upit");
    let cpit_problem_path = references_dir.join("marvin.cpit");
    let cpit_solution_path = references_dir.join("marvin_cpit_gmunoz120723.sol");
    let pcpsp_problem_path = references_dir.join("marvin.pcpsp");
    let pcpsp_solution_path = references_dir.join("marvin_pcpsp_gmunoz120723.sol");
    let lp_cpit_solution_path = references_dir.join("marvin.LPcpit");
    let lp_pcpsp_solution_path = references_dir.join("marvin.LPpcpsp");

    if cli.mode == MarvinBenchmarkMode::FocusedMr187 {
        let output = build_focused_mr187_refresh_output(
            &repo_root,
            &dataset_dir,
            &prec_path,
            &blocks_path,
            &pcpsp_problem_path,
            &pcpsp_solution_path,
            &lp_pcpsp_solution_path,
        )?;
        write_report_to_path(&output_path, &output)?;
        maybe_print_report(&output)?;
        return Ok(());
    }

    let model = read_benchmark_blocks(&blocks_path, "marvin")?;
    let reference_prec = read_marvin_precedence_graph(&prec_path, &model)?;
    let reference_upit_membership = read_marvin_upit_solution(&upit_solution_path, &model)?;
    let exact_upit_weights = read_marvin_upit_block_values(&upit_objective_path, &model)?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let cpit_problem = read_marvin_cpit_problem(&cpit_problem_path, &model)?;
    let cpit_solution = read_marvin_cpit_solution(&cpit_solution_path, &model)?;
    let pcpsp_problem = read_marvin_pcpsp_problem(&pcpsp_problem_path, &model)?;
    let pcpsp_solution = read_marvin_pcpsp_solution(&pcpsp_solution_path, &model)?;
    let lp_cpit_solution = read_marvin_lp_cpit_solution(&lp_cpit_solution_path, &model)?;
    let lp_pcpsp_solution = read_marvin_lp_pcpsp_solution(&lp_pcpsp_solution_path, &model)?;
    let cpit_summary = summarize_marvin_schedule_solution(&cpit_problem, &cpit_solution)?;
    let pcpsp_summary = summarize_marvin_schedule_solution(&pcpsp_problem, &pcpsp_solution)?;
    let lp_cpit_summary = summarize_marvin_schedule_solution(&cpit_problem, &lp_cpit_solution)?;
    let lp_pcpsp_summary = summarize_marvin_schedule_solution(&pcpsp_problem, &lp_pcpsp_solution)?;

    let template = marvin_slope_template()?;
    let candidate_prec = build_block_precedence_graph(&model, &template)?;
    let precedence_comparison = compact_precedence_comparison(mine_sdk::compare_precedence_graphs(
        &reference_prec,
        &candidate_prec,
    ));

    let value_column = ColumnId::new("field_7")?;
    let tonnage_column = ColumnId::new("field_4")?;
    let exact_upit_result = solve_upl_exact(&build_max_closure_graph(
        &exact_upit_weights,
        &reference_prec,
    )?)?;
    let candidate_upit = build_upit_prototype(
        &model,
        &candidate_prec,
        &value_column,
        Some(&tonnage_column),
    )?;
    let upit_membership_comparison = compact_membership_comparison(compare_block_memberships(
        &reference_upit_membership,
        &candidate_upit.selected_linear_indices,
    ));
    let exact_upit_membership_comparison =
        compact_membership_comparison(compare_block_memberships(
            &reference_upit_membership,
            &exact_upit_result.selected_blocks,
        ));

    let reference_upit_metrics = membership_metrics(
        &model,
        &reference_upit_membership,
        &value_column,
        &tonnage_column,
    )?;
    let candidate_upit_metrics = {
        let mut m = membership_metrics(
            &model,
            &candidate_upit.selected_linear_indices,
            &value_column,
            &tonnage_column,
        )?;
        m.insert("block_count".to_owned(), candidate_upit.block_count as f64);
        m
    };
    let exact_upit_metrics = {
        let mut m = membership_metrics(
            &model,
            &exact_upit_result.selected_blocks,
            &value_column,
            &tonnage_column,
        )?;
        m.insert(
            "block_count".to_owned(),
            exact_upit_result.selected_block_count as f64,
        );
        m
    };
    let upit_metric_comparison = compare_named_numeric_metrics(
        &reference_upit_metrics,
        &candidate_upit_metrics,
        &BTreeMap::from([
            (
                "block_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_proc_profit_x_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_economic_objective".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
        ]),
    );
    let exact_upit_metric_comparison = compare_named_numeric_metrics(
        &reference_upit_metrics,
        &exact_upit_metrics,
        &BTreeMap::from([
            (
                "block_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_proc_profit_x_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_economic_objective".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "total_tonnage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
        ]),
    );
    let mine_rs_end_to_end =
        build_mine_rs_end_to_end_artifacts(&model, &reference_prec, &pcpsp_problem)?;
    let tonnage_by_linear_index = build_linear_index_float_lookup(&model, &tonnage_column)?;
    let lp_bz_benchmark = build_lp_bz_access_progression_artifacts(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &pcpsp_problem,
        &tonnage_by_linear_index,
    )?;
    let (lp_bz_round_repair_artifacts, lp_bz_round_repair_schedule) =
        build_target_period_seeded_schedule_from_lp_round_repair_v6(
            &lp_bz_benchmark.phase_plan,
            &lp_bz_benchmark.scheduling_problem,
            &lp_pcpsp_solution,
            None,
            Metadata::new(),
        )?;
    let lp_bz_rounder_v6_local_optimizer_diagnostics =
        build_lp_bz_rounder_v6_local_optimizer_diagnostics(&lp_bz_round_repair_artifacts);
    let lp_bz_round_repair_period_memberships = build_candidate_period_memberships(
        &model,
        &lp_bz_benchmark.phase_plan,
        &lp_bz_round_repair_schedule,
        &tonnage_column,
    )?;
    let lp_bz_round_repair_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &lp_bz_round_repair_period_memberships)?;
    let lp_bz_round_repair_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_bz_round_repair_solution)?;
    let lp_representative_period_by_block = representative_period_by_block(&lp_pcpsp_solution);
    let lp_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &mine_rs_end_to_end.phase_plan,
        &lp_pcpsp_solution,
    )?;
    let lp_shell_seeded_period_memberships = build_phase_period_memberships_from_phase_targets(
        &mine_rs_end_to_end.phase_plan,
        &lp_phase_target_periods,
    )?;
    let lp_shell_seeded_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &lp_shell_seeded_period_memberships)?;
    let lp_shell_seeded_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_shell_seeded_solution)?;
    let lp_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &mine_rs_end_to_end.scheduling_problem,
        &lp_phase_target_periods,
    )?;
    let lp_target_period_seeded_schedule = build_target_period_seeded_long_term_schedule(
        &mine_rs_end_to_end.scheduling_problem,
        &lp_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let lp_target_period_seeded_period_memberships = build_candidate_period_memberships(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &lp_target_period_seeded_schedule,
        &tonnage_column,
    )?;
    let lp_target_period_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &lp_target_period_seeded_period_memberships,
    )?;
    let lp_target_period_seeded_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_target_period_seeded_solution)?;
    let lp_staggered_target_period_by_unit =
        build_staggered_unit_target_periods_from_phase_targets(
            &mine_rs_end_to_end.scheduling_problem,
            &lp_phase_target_periods,
        )?;
    let lp_staggered_target_seeded_schedule = build_target_period_seeded_long_term_schedule(
        &mine_rs_end_to_end.scheduling_problem,
        &lp_staggered_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let lp_staggered_target_seeded_period_memberships = build_candidate_period_memberships(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &lp_staggered_target_seeded_schedule,
        &tonnage_column,
    )?;
    let lp_staggered_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &lp_staggered_target_seeded_period_memberships,
    )?;
    let lp_staggered_target_seeded_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_staggered_target_seeded_solution)?;
    let lp_windowed_exact_schedule = build_target_period_windowed_long_term_schedule(
        &mine_rs_end_to_end.scheduling_problem,
        &lp_staggered_target_period_by_unit,
        LP_WINDOW_CANDIDATE_SIZE,
        None,
        Metadata::new(),
    )?;
    let lp_windowed_exact_period_memberships = build_candidate_period_memberships(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &lp_windowed_exact_schedule,
        &tonnage_column,
    )?;
    let lp_windowed_exact_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &lp_windowed_exact_period_memberships)?;
    let lp_windowed_exact_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_windowed_exact_solution)?;
    let lp_cut_phase_plan = split_phase_plan_by_representative_period_bands(
        &mine_rs_end_to_end.phase_plan,
        &lp_representative_period_by_block,
        &tonnage_by_linear_index,
        LP_CUT_PERIOD_BAND_WIDTH,
    )?;
    let lp_cut_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        &model,
        &lp_cut_phase_plan,
        &pcpsp_problem,
    )?;
    let lp_cut_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&lp_cut_phase_plan, &lp_pcpsp_solution)?;
    let lp_cut_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &lp_cut_scheduling_problem,
        &lp_cut_phase_target_periods,
    )?;
    let lp_cut_target_seeded_schedule = build_target_period_seeded_long_term_schedule(
        &lp_cut_scheduling_problem,
        &lp_cut_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let lp_cut_target_seeded_period_memberships = build_candidate_period_memberships(
        &model,
        &lp_cut_phase_plan,
        &lp_cut_target_seeded_schedule,
        &tonnage_column,
    )?;
    let lp_cut_target_seeded_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &lp_cut_target_seeded_period_memberships)?;
    let lp_cut_target_seeded_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_cut_target_seeded_solution)?;
    let lp_quantile_cut_phase_plan = split_phase_plan_by_representative_period_quantiles(
        &mine_rs_end_to_end.phase_plan,
        &lp_representative_period_by_block,
        &tonnage_by_linear_index,
    )?;
    let lp_quantile_cut_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        &model,
        &lp_quantile_cut_phase_plan,
        &pcpsp_problem,
    )?;
    let lp_quantile_cut_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &lp_quantile_cut_phase_plan,
        &lp_pcpsp_solution,
    )?;
    let lp_quantile_cut_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &lp_quantile_cut_scheduling_problem,
        &lp_quantile_cut_phase_target_periods,
    )?;
    let lp_quantile_cut_target_seeded_schedule = build_target_period_seeded_long_term_schedule(
        &lp_quantile_cut_scheduling_problem,
        &lp_quantile_cut_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let lp_quantile_cut_target_seeded_period_memberships = build_candidate_period_memberships(
        &model,
        &lp_quantile_cut_phase_plan,
        &lp_quantile_cut_target_seeded_schedule,
        &tonnage_column,
    )?;
    let lp_quantile_cut_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &lp_quantile_cut_target_seeded_period_memberships,
    )?;
    let lp_quantile_cut_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &lp_quantile_cut_target_seeded_solution,
    )?;
    let geometric_component_phase_plan = split_phase_plan_by_planar_connected_components(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &tonnage_by_linear_index,
    )?;
    let geometric_component_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &geometric_component_phase_plan,
            &pcpsp_problem,
        )?;
    let geometric_component_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &geometric_component_phase_plan,
        &lp_pcpsp_solution,
    )?;
    let geometric_component_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &geometric_component_scheduling_problem,
        &geometric_component_phase_target_periods,
    )?;
    let geometric_component_target_seeded_schedule = build_target_period_seeded_long_term_schedule(
        &geometric_component_scheduling_problem,
        &geometric_component_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let geometric_component_target_seeded_period_memberships = build_candidate_period_memberships(
        &model,
        &geometric_component_phase_plan,
        &geometric_component_target_seeded_schedule,
        &tonnage_column,
    )?;
    let geometric_component_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &geometric_component_target_seeded_period_memberships,
    )?;
    let geometric_component_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &geometric_component_target_seeded_solution,
    )?;
    let geometric_local_component_phase_plan =
        split_phase_plan_by_planar_connected_components_with_local_predecessors(
            &model,
            &mine_rs_end_to_end.phase_plan,
            &tonnage_by_linear_index,
        )?;
    let geometric_local_component_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &geometric_local_component_phase_plan,
            &pcpsp_problem,
        )?;
    let geometric_local_component_phase_target_periods =
        build_phase_target_periods_from_lp_solution(
            &geometric_local_component_phase_plan,
            &lp_pcpsp_solution,
        )?;
    let geometric_local_component_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &geometric_local_component_scheduling_problem,
            &geometric_local_component_phase_target_periods,
        )?;
    let geometric_local_component_target_seeded_schedule =
        build_target_period_seeded_long_term_schedule(
            &geometric_local_component_scheduling_problem,
            &geometric_local_component_target_period_by_unit,
            None,
            Metadata::new(),
        )?;
    let geometric_local_component_target_seeded_period_memberships =
        build_candidate_period_memberships(
            &model,
            &geometric_local_component_phase_plan,
            &geometric_local_component_target_seeded_schedule,
            &tonnage_column,
        )?;
    let geometric_local_component_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &geometric_local_component_target_seeded_period_memberships,
    )?;
    let geometric_local_component_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &geometric_local_component_target_seeded_solution,
    )?;
    let geometric_component_stripe_phase_plan = split_phase_plan_by_planar_component_stripes(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &tonnage_by_linear_index,
        GEOMETRIC_COMPONENT_STRIPE_COUNT,
    )?;
    let geometric_component_stripe_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &geometric_component_stripe_phase_plan,
            &pcpsp_problem,
        )?;
    let geometric_component_stripe_phase_target_periods =
        build_phase_target_periods_from_lp_solution(
            &geometric_component_stripe_phase_plan,
            &lp_pcpsp_solution,
        )?;
    let geometric_component_stripe_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &geometric_component_stripe_scheduling_problem,
            &geometric_component_stripe_phase_target_periods,
        )?;
    let geometric_component_stripe_target_seeded_schedule =
        build_target_period_seeded_long_term_schedule(
            &geometric_component_stripe_scheduling_problem,
            &geometric_component_stripe_target_period_by_unit,
            None,
            Metadata::new(),
        )?;
    let geometric_component_stripe_target_seeded_period_memberships =
        build_candidate_period_memberships(
            &model,
            &geometric_component_stripe_phase_plan,
            &geometric_component_stripe_target_seeded_schedule,
            &tonnage_column,
        )?;
    let geometric_component_stripe_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &geometric_component_stripe_target_seeded_period_memberships,
    )?;
    let geometric_component_stripe_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &geometric_component_stripe_target_seeded_solution,
    )?;
    let directional_front_band_phase_plan = split_phase_plan_by_directional_front_bands(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &tonnage_by_linear_index,
        DIRECTIONAL_FRONT_BAND_COUNT,
    )?;
    let directional_front_band_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &directional_front_band_phase_plan,
            &pcpsp_problem,
        )?;
    let directional_front_band_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &directional_front_band_phase_plan,
        &lp_pcpsp_solution,
    )?;
    let directional_front_band_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &directional_front_band_scheduling_problem,
            &directional_front_band_phase_target_periods,
        )?;
    let directional_front_band_target_seeded_schedule =
        build_target_period_seeded_long_term_schedule(
            &directional_front_band_scheduling_problem,
            &directional_front_band_target_period_by_unit,
            None,
            Metadata::new(),
        )?;
    let directional_front_band_target_seeded_period_memberships =
        build_candidate_period_memberships(
            &model,
            &directional_front_band_phase_plan,
            &directional_front_band_target_seeded_schedule,
            &tonnage_column,
        )?;
    let directional_front_band_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &directional_front_band_target_seeded_period_memberships,
    )?;
    let directional_front_band_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &directional_front_band_target_seeded_solution,
    )?;
    let directional_local_front_band_phase_plan =
        split_phase_plan_by_directional_front_bands_with_local_access(
            &model,
            &mine_rs_end_to_end.phase_plan,
            &tonnage_by_linear_index,
            DIRECTIONAL_FRONT_BAND_COUNT,
        )?;
    let directional_local_front_band_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &directional_local_front_band_phase_plan,
            &pcpsp_problem,
        )?;
    let directional_local_front_band_phase_target_periods =
        build_phase_target_periods_from_lp_solution(
            &directional_local_front_band_phase_plan,
            &lp_pcpsp_solution,
        )?;
    let directional_local_front_band_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &directional_local_front_band_scheduling_problem,
            &directional_local_front_band_phase_target_periods,
        )?;
    let directional_local_front_band_target_seeded_schedule =
        build_target_period_seeded_long_term_schedule(
            &directional_local_front_band_scheduling_problem,
            &directional_local_front_band_target_period_by_unit,
            None,
            Metadata::new(),
        )?;
    let directional_local_front_band_target_seeded_period_memberships =
        build_candidate_period_memberships(
            &model,
            &directional_local_front_band_phase_plan,
            &directional_local_front_band_target_seeded_schedule,
            &tonnage_column,
        )?;
    let directional_local_front_band_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &directional_local_front_band_target_seeded_period_memberships,
    )?;
    let directional_local_front_band_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &directional_local_front_band_target_seeded_solution,
    )?;
    let adaptive_component_front_phase_plan = split_phase_plan_by_adaptive_component_fronts(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &tonnage_by_linear_index,
        ADAPTIVE_COMPONENT_FRONT_COUNT,
        ADAPTIVE_COMPONENT_FRONT_MIN_SHARE,
    )?;
    let adaptive_component_front_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &adaptive_component_front_phase_plan,
            &pcpsp_problem,
        )?;
    let adaptive_component_front_phase_target_periods =
        build_phase_target_periods_from_lp_solution(
            &adaptive_component_front_phase_plan,
            &lp_pcpsp_solution,
        )?;
    let adaptive_component_front_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &adaptive_component_front_scheduling_problem,
            &adaptive_component_front_phase_target_periods,
        )?;
    let adaptive_component_front_target_seeded_schedule =
        build_target_period_seeded_long_term_schedule(
            &adaptive_component_front_scheduling_problem,
            &adaptive_component_front_target_period_by_unit,
            None,
            Metadata::new(),
        )?;
    let adaptive_component_front_target_seeded_period_memberships =
        build_candidate_period_memberships(
            &model,
            &adaptive_component_front_phase_plan,
            &adaptive_component_front_target_seeded_schedule,
            &tonnage_column,
        )?;
    let adaptive_component_front_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &adaptive_component_front_target_seeded_period_memberships,
    )?;
    let adaptive_component_front_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &adaptive_component_front_target_seeded_solution,
    )?;
    let shape_gated_front_phase_plan = split_phase_plan_by_shape_gated_component_fronts(
        &model,
        &mine_rs_end_to_end.phase_plan,
        &tonnage_by_linear_index,
        SHAPE_GATED_FRONT_COUNT,
        SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
        SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
    )?;
    let shape_gated_front_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        &model,
        &shape_gated_front_phase_plan,
        &pcpsp_problem,
    )?;
    let shape_gated_front_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &shape_gated_front_phase_plan,
        &lp_pcpsp_solution,
    )?;
    let shape_gated_front_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_front_scheduling_problem,
        &shape_gated_front_phase_target_periods,
    )?;
    let shape_gated_front_target_seeded_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_front_scheduling_problem,
        &shape_gated_front_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_front_target_seeded_period_memberships = build_candidate_period_memberships(
        &model,
        &shape_gated_front_phase_plan,
        &shape_gated_front_target_seeded_schedule,
        &tonnage_column,
    )?;
    let shape_gated_front_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &shape_gated_front_target_seeded_period_memberships,
    )?;
    let shape_gated_front_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &shape_gated_front_target_seeded_solution,
    )?;
    let shape_gated_local_front_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            &model,
            &mine_rs_end_to_end.phase_plan,
            &tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            None,
            None,
            None,
            None,
        )?;
    let shape_gated_local_front_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            &model,
            &shape_gated_local_front_phase_plan,
            &pcpsp_problem,
        )?;
    let shape_gated_local_front_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &shape_gated_local_front_phase_plan,
        &lp_pcpsp_solution,
    )?;
    let shape_gated_local_front_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &shape_gated_local_front_scheduling_problem,
            &shape_gated_local_front_phase_target_periods,
        )?;
    let shape_gated_local_front_target_seeded_schedule =
        build_target_period_seeded_long_term_schedule(
            &shape_gated_local_front_scheduling_problem,
            &shape_gated_local_front_target_period_by_unit,
            None,
            Metadata::new(),
        )?;
    let shape_gated_local_front_target_seeded_period_memberships =
        build_candidate_period_memberships(
            &model,
            &shape_gated_local_front_phase_plan,
            &shape_gated_local_front_target_seeded_schedule,
            &tonnage_column,
        )?;
    let shape_gated_local_front_target_seeded_solution = build_candidate_pcpsp_solution(
        &pcpsp_problem,
        &shape_gated_local_front_target_seeded_period_memberships,
    )?;
    let shape_gated_local_front_target_seeded_summary = summarize_marvin_schedule_solution(
        &pcpsp_problem,
        &shape_gated_local_front_target_seeded_solution,
    )?;
    let shape_gated_front_rule_sweep = SHAPE_GATED_FRONT_ASPECT_RATIO_SWEEP
        .into_iter()
        .flat_map(|min_aspect_ratio| {
            SHAPE_GATED_FRONT_DOMINANT_SPAN_SWEEP
                .into_iter()
                .map(move |min_dominant_span| (min_aspect_ratio, min_dominant_span))
        })
        .map(|(min_aspect_ratio, min_dominant_span)| {
            build_shape_gated_front_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                min_aspect_ratio,
                min_dominant_span,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_rule_window_sweep = SHAPE_GATED_FRONT_ASPECT_RATIO_SWEEP
        .into_iter()
        .flat_map(|min_aspect_ratio| {
            SHAPE_GATED_FRONT_DOMINANT_SPAN_SWEEP
                .into_iter()
                .map(move |min_dominant_span| (min_aspect_ratio, min_dominant_span))
        })
        .map(|(min_aspect_ratio, min_dominant_span)| {
            build_shape_gated_local_rule_window_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                min_aspect_ratio,
                min_dominant_span,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_rule_front_count_sweep = SHAPE_GATED_LOCAL_FRONT_COUNT_SWEEP
        .into_iter()
        .map(|max_front_count| {
            build_shape_gated_local_rule_front_count_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                max_front_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_front_count_sweep = SHAPE_GATED_FRONT_COUNT_SWEEP
        .into_iter()
        .map(|max_front_count| {
            build_shape_gated_front_count_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                max_front_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_front_count_sweep = SHAPE_GATED_LOCAL_FRONT_COUNT_SWEEP
        .into_iter()
        .map(|max_front_count| {
            build_shape_gated_local_front_count_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                max_front_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_overlap_front_count_sweep = SHAPE_GATED_LOCAL_FRONT_COUNT_SWEEP
        .into_iter()
        .map(|max_front_count| {
            build_shape_gated_local_overlap_front_count_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                max_front_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_access_sweep = SHAPE_GATED_LOCAL_ACCESS_TOUCHING_SWEEP
        .into_iter()
        .map(|include_touching_neighbors| {
            build_shape_gated_local_access_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                include_touching_neighbors,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_access_window_sweep = SHAPE_GATED_LOCAL_ACCESS_TOUCHING_SWEEP
        .into_iter()
        .map(|include_touching_neighbors| {
            build_shape_gated_local_access_window_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                include_touching_neighbors,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_front_progression_sweep = SHAPE_GATED_FRONT_PROGRESSION_SWEEP
        .into_iter()
        .map(|profile| {
            build_shape_gated_front_progression_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                profile.label,
                &profile.cumulative_tonnage_targets,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_front_progression_window_sweep = SHAPE_GATED_FRONT_PROGRESSION_SWEEP
        .into_iter()
        .map(|profile| {
            build_shape_gated_front_progression_window_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                profile.label,
                &profile.cumulative_tonnage_targets,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_conditional_progression_sweep =
        SHAPE_GATED_CONDITIONAL_PROGRESSIVE_ASPECT_SWEEP
            .into_iter()
            .map(|min_progression_aspect_ratio| {
                build_shape_gated_conditional_progression_sweep_entry(
                    &model,
                    &mine_rs_end_to_end,
                    &pcpsp_problem,
                    &lp_pcpsp_solution,
                    &tonnage_by_linear_index,
                    min_progression_aspect_ratio,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_conditional_window_progression_sweep =
        SHAPE_GATED_CONDITIONAL_PROGRESSIVE_ASPECT_SWEEP
            .into_iter()
            .map(|min_progression_aspect_ratio| {
                build_shape_gated_conditional_window_progression_sweep_entry(
                    &model,
                    &mine_rs_end_to_end,
                    &pcpsp_problem,
                    &lp_pcpsp_solution,
                    &tonnage_by_linear_index,
                    min_progression_aspect_ratio,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_local_window_sweep = SHAPE_GATED_LOCAL_PREDECESSOR_WINDOW_SWEEP
        .into_iter()
        .map(|max_local_predecessor_count| {
            build_shape_gated_local_window_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                max_local_predecessor_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape_gated_dynamic_local_window_sweep = SHAPE_GATED_DYNAMIC_LOCAL_WINDOW_SWEEP
        .into_iter()
        .map(
            |(
                window_rule_label,
                min_dynamic_window_aspect_ratio,
                promoted_local_predecessor_count,
            )| {
                build_shape_gated_dynamic_local_window_sweep_entry(
                    &model,
                    &mine_rs_end_to_end,
                    &pcpsp_problem,
                    &lp_pcpsp_solution,
                    &tonnage_by_linear_index,
                    window_rule_label,
                    min_dynamic_window_aspect_ratio,
                    promoted_local_predecessor_count,
                )
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let adaptive_component_front_threshold_sweep = ADAPTIVE_COMPONENT_FRONT_SHARE_SWEEP
        .into_iter()
        .map(|min_component_share| {
            build_adaptive_component_front_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &tonnage_by_linear_index,
                min_component_share,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lp_cut_band_width_sweep = LP_CUT_BAND_SWEEP_WIDTHS
        .into_iter()
        .map(|period_band_width| {
            build_lp_cut_band_width_sweep_entry(
                &model,
                &mine_rs_end_to_end,
                &pcpsp_problem,
                &lp_pcpsp_solution,
                &lp_representative_period_by_block,
                &tonnage_by_linear_index,
                period_band_width,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mine_rs_vs_cpit_metric_comparison = compare_named_numeric_metrics(
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                cpit_summary.discounted_objective,
            ),
            (
                "used_period_count".to_owned(),
                cpit_summary.used_period_count as f64,
            ),
            (
                "unique_block_count".to_owned(),
                cpit_summary.unique_block_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                cpit_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 0)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
        ]),
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                mine_rs_end_to_end.report.npv,
            ),
            (
                "used_period_count".to_owned(),
                mine_rs_end_to_end
                    .summary
                    .periods
                    .iter()
                    .filter(|period| period.tonnage > 0.0)
                    .count() as f64,
            ),
            (
                "unique_block_count".to_owned(),
                mine_rs_end_to_end.summary.total_block_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                mine_rs_end_to_end
                    .summary
                    .periods
                    .iter()
                    .map(|period| period.tonnage)
                    .fold(0.0_f64, f64::max),
            ),
        ]),
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "used_period_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "unique_block_count".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
            (
                "max_mine_period_usage".to_owned(),
                NumericMetricTolerance {
                    absolute: Some(0.0),
                    relative: Some(0.0),
                },
            ),
        ]),
    );
    let mine_rs_vs_cpit_membership_comparison = compare_period_memberships(
        &build_reference_period_memberships(&cpit_solution),
        &mine_rs_end_to_end.period_memberships,
    );
    let mine_rs_vs_cpit_period_metric_comparison = compare_named_numeric_metrics(
        &build_reference_period_metric_map(&model, &cpit_problem, &cpit_solution, &tonnage_column)?,
        &build_candidate_period_metric_map(
            &mine_rs_end_to_end.report,
            &mine_rs_end_to_end.period_memberships,
        ),
        &BTreeMap::new(),
    );
    let candidate_pcpsp_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &mine_rs_end_to_end.period_memberships)?;
    let candidate_pcpsp_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &candidate_pcpsp_solution)?;
    let strict_shell_factor_sweep = MARVIN_FACTOR_SWEEP_COUNTS
        .into_iter()
        .map(|factor_count| {
            build_strict_shell_factor_sweep_entry(
                &model,
                &candidate_prec,
                &pcpsp_problem,
                &pcpsp_problem,
                factor_count,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shell_access_sweep = vec![
        ("strict-shell-sequential", marvin_shell_access_rules()),
        ("open-bench-lag-0", NestingAccessRules::default_open()),
        (
            "open-bench-lag-1",
            NestingAccessRules {
                min_bench_lag: Some(1),
                require_complete_outer_before_inner: false,
                simultaneous_access: true,
            },
        ),
        (
            "open-bench-lag-2",
            NestingAccessRules {
                min_bench_lag: Some(2),
                require_complete_outer_before_inner: false,
                simultaneous_access: true,
            },
        ),
    ]
    .into_iter()
    .map(|(access_policy_label, nesting_rules)| {
        build_shell_access_sweep_entry(
            &model,
            &candidate_prec,
            &pcpsp_problem,
            &pcpsp_problem,
            MARVIN_END_TO_END_FACTOR_COUNT,
            access_policy_label,
            nesting_rules,
        )
    })
    .collect::<Result<Vec<_>, _>>()?;
    let mine_rs_vs_pcpsp_metric_comparison = compare_named_numeric_metrics(
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                pcpsp_summary.discounted_objective,
            ),
            (
                "used_period_count".to_owned(),
                pcpsp_summary.used_period_count as f64,
            ),
            (
                "unique_block_count".to_owned(),
                pcpsp_summary.unique_block_count as f64,
            ),
            (
                "used_destination_count".to_owned(),
                pcpsp_summary.used_destination_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 0)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
            (
                "max_process_period_usage".to_owned(),
                pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 1)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
        ]),
        &BTreeMap::from([
            (
                "discounted_objective".to_owned(),
                candidate_pcpsp_summary.discounted_objective,
            ),
            (
                "used_period_count".to_owned(),
                candidate_pcpsp_summary.used_period_count as f64,
            ),
            (
                "unique_block_count".to_owned(),
                candidate_pcpsp_summary.unique_block_count as f64,
            ),
            (
                "used_destination_count".to_owned(),
                candidate_pcpsp_summary.used_destination_count as f64,
            ),
            (
                "max_mine_period_usage".to_owned(),
                candidate_pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 0)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
            (
                "max_process_period_usage".to_owned(),
                candidate_pcpsp_summary
                    .resource_summaries
                    .iter()
                    .find(|summary| summary.resource_index == 1)
                    .map(|summary| summary.max_period_usage)
                    .unwrap_or(0.0),
            ),
        ]),
        &BTreeMap::new(),
    );
    let mine_rs_vs_pcpsp_membership_comparison = compare_period_memberships(
        &build_reference_period_destination_memberships(&pcpsp_solution),
        &build_reference_period_destination_memberships(&candidate_pcpsp_solution),
    );
    let mine_rs_vs_pcpsp_period_metric_comparison = compare_named_numeric_metrics(
        &build_reference_period_metric_map(
            &model,
            &pcpsp_problem,
            &pcpsp_solution,
            &tonnage_column,
        )?,
        &build_reference_period_metric_map(
            &model,
            &pcpsp_problem,
            &candidate_pcpsp_solution,
            &tonnage_column,
        )?,
        &BTreeMap::new(),
    );
    let lp_bz_integer_candidate_artifact = build_baseline_summary(
        "lp-bz-round-repair-local-front-seeded",
        lp_bz_benchmark.phase_plan.phase_count,
        &pcpsp_summary,
        &pcpsp_solution,
        &lp_bz_round_repair_summary,
        &lp_bz_round_repair_solution,
    );
    let lp_bz_bound_artifacts = compute_lp_bz_bound_artifacts(
        &pcpsp_problem,
        &lp_pcpsp_solution,
        &lp_pcpsp_solution_path,
        &repo_root,
        lp_bz_benchmark.scheduling_problem.units().len(),
        lp_bz_benchmark
            .scheduling_problem
            .units()
            .iter()
            .map(|unit| unit.predecessor_unit_ids().len())
            .sum(),
        LP_BZ_UNIT_GRANULARITY_LABEL,
    )?;
    let lp_bz_lp_kernel_artifact =
        build_lp_bz_lp_kernel_artifact(&lp_bz_benchmark.scheduling_problem)?;
    let lp_bz_lp_solve_artifact = solve_lp_bz_lp_kernel_artifact(&lp_bz_lp_kernel_artifact)?;
    validate_lp_bz_artifact_coherence(
        &lp_bz_bound_artifacts.lp_bz_inputs,
        &lp_bz_bound_artifacts.lp_bz_bound_artifact,
        &lp_bz_lp_kernel_artifact,
    )?;
    let lp_bz_gap_metrics = build_lp_bz_gap_metrics(
        &lp_bz_bound_artifacts.lp_bz_bound_artifact,
        &lp_bz_lp_solve_artifact,
        &lp_bz_integer_candidate_artifact.candidate_pcpsp_summary,
        pcpsp_summary.discounted_objective,
        candidate_pcpsp_summary.discounted_objective,
    );

    let output = MarvinBenchmarkOutput {
        dataset_dir: relative_or_display(&dataset_dir, &repo_root),
        reference_prec_path: relative_or_display(&prec_path, &repo_root),
        reference_upit_solution_path: relative_or_display(&upit_solution_path, &repo_root),
        reference_upit_objective_path: relative_or_display(&upit_objective_path, &repo_root),
        reference_cpit_problem_path: relative_or_display(&cpit_problem_path, &repo_root),
        reference_cpit_solution_path: relative_or_display(&cpit_solution_path, &repo_root),
        reference_pcpsp_problem_path: relative_or_display(&pcpsp_problem_path, &repo_root),
        reference_pcpsp_solution_path: relative_or_display(&pcpsp_solution_path, &repo_root),
        reference_lp_cpit_solution_path: relative_or_display(&lp_cpit_solution_path, &repo_root),
        reference_lp_pcpsp_solution_path: relative_or_display(
            &lp_pcpsp_solution_path,
            &repo_root,
        ),
        value_column: value_column.to_string(),
        tonnage_column: tonnage_column.to_string(),
        candidate_predecessor_offsets: template
            .predecessor_offsets()
            .iter()
            .map(|offset| (offset.di(), offset.dj(), offset.dk()))
            .collect(),
        reference_precedence: PrecedenceArtifactSummary {
            node_count: reference_prec.nodes().len(),
            edge_count: reference_prec.edges().len(),
        },
        candidate_precedence: PrecedenceArtifactSummary {
            node_count: candidate_prec.nodes().len(),
            edge_count: candidate_prec.edges().len(),
        },
        precedence_comparison,
        reference_upit: MembershipArtifactSummary {
            block_count: reference_upit_metrics["block_count"] as usize,
            total_proc_profit_x_tonnage: reference_upit_metrics["total_proc_profit_x_tonnage"],
            total_economic_objective: reference_upit_metrics["total_economic_objective"],
            total_tonnage: reference_upit_metrics["total_tonnage"],
        },
        candidate_upit: MembershipArtifactSummary {
            block_count: candidate_upit_metrics["block_count"] as usize,
            total_proc_profit_x_tonnage: candidate_upit_metrics["total_proc_profit_x_tonnage"],
            total_economic_objective: candidate_upit_metrics["total_economic_objective"],
            total_tonnage: candidate_upit_metrics["total_tonnage"],
        },
        exact_upit: MembershipArtifactSummary {
            block_count: exact_upit_metrics["block_count"] as usize,
            total_proc_profit_x_tonnage: exact_upit_metrics["total_proc_profit_x_tonnage"],
            total_economic_objective: exact_upit_metrics["total_economic_objective"],
            total_tonnage: exact_upit_metrics["total_tonnage"],
        },
        upit_membership_comparison,
        exact_upit_membership_comparison,
        upit_metric_comparison,
        exact_upit_metric_comparison,
        cpit_reference: ScheduleReferenceArtifactSummary {
            period_count: cpit_problem.period_count,
            destination_count: cpit_problem.destination_count,
            resource_constraint_count: cpit_problem.resource_constraint_count,
            discount_rate: cpit_problem.discount_rate,
            official_objective: OFFICIAL_CPIT_OBJECTIVE,
            objective_gap_vs_official: (cpit_summary.discounted_objective - OFFICIAL_CPIT_OBJECTIVE)
                .abs(),
            solution_summary: cpit_summary,
        },
        pcpsp_reference: ScheduleReferenceArtifactSummary {
            period_count: pcpsp_problem.period_count,
            destination_count: pcpsp_problem.destination_count,
            resource_constraint_count: pcpsp_problem.resource_constraint_count,
            discount_rate: pcpsp_problem.discount_rate,
            official_objective: OFFICIAL_PCPSP_OBJECTIVE,
            objective_gap_vs_official: (pcpsp_summary.discounted_objective
                - OFFICIAL_PCPSP_OBJECTIVE)
                .abs(),
            solution_summary: pcpsp_summary.clone(),
        },
        lp_cpit_reference: ScheduleReferenceArtifactSummary {
            period_count: cpit_problem.period_count,
            destination_count: cpit_problem.destination_count,
            resource_constraint_count: cpit_problem.resource_constraint_count,
            discount_rate: cpit_problem.discount_rate,
            official_objective: OFFICIAL_LP_CPIT_OBJECTIVE,
            objective_gap_vs_official: (lp_cpit_summary.discounted_objective
                - OFFICIAL_LP_CPIT_OBJECTIVE)
                .abs(),
            solution_summary: lp_cpit_summary,
        },
        lp_pcpsp_reference: ScheduleReferenceArtifactSummary {
            period_count: pcpsp_problem.period_count,
            destination_count: pcpsp_problem.destination_count,
            resource_constraint_count: pcpsp_problem.resource_constraint_count,
            discount_rate: pcpsp_problem.discount_rate,
            official_objective: OFFICIAL_LP_PCPSP_OBJECTIVE,
            objective_gap_vs_official: (lp_pcpsp_summary.discounted_objective
                - OFFICIAL_LP_PCPSP_OBJECTIVE)
                .abs(),
            solution_summary: lp_pcpsp_summary,
        },
        lp_bz_inputs: lp_bz_bound_artifacts.lp_bz_inputs,
        lp_bz_bound_artifact: lp_bz_bound_artifacts.lp_bz_bound_artifact,
        lp_bz_lp_kernel_artifact: compact_lp_bz_lp_kernel_artifact(&lp_bz_lp_kernel_artifact),
        lp_bz_lp_solve_artifact,
        lp_bz_integer_candidate_artifact,
        lp_bz_rounder_v6_local_optimizer_diagnostics,
        lp_bz_gap_metrics,
        lp_shell_seeded_baseline: build_baseline_summary(
            "lp-shell-seeded",
            mine_rs_end_to_end.phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_shell_seeded_summary,
            &lp_shell_seeded_solution,
        ),
        lp_target_period_seeded_baseline: build_baseline_summary(
            "lp-target-period-seeded",
            mine_rs_end_to_end.phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_target_period_seeded_summary,
            &lp_target_period_seeded_solution,
        ),
        lp_staggered_target_seeded_baseline: build_baseline_summary(
            "lp-staggered-target-seeded",
            mine_rs_end_to_end.phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_staggered_target_seeded_summary,
            &lp_staggered_target_seeded_solution,
        ),
        lp_windowed_exact_baseline: build_baseline_summary(
            "lp-windowed-exact",
            mine_rs_end_to_end.phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_windowed_exact_summary,
            &lp_windowed_exact_solution,
        ),
        lp_cut_target_seeded_baseline: build_baseline_summary(
            "lp-cut-target-seeded",
            lp_cut_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_cut_target_seeded_summary,
            &lp_cut_target_seeded_solution,
        ),
        lp_quantile_cut_target_seeded_baseline: build_baseline_summary(
            "lp-quantile-cut-target-seeded",
            lp_quantile_cut_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_quantile_cut_target_seeded_summary,
            &lp_quantile_cut_target_seeded_solution,
        ),
        geometric_component_target_seeded_baseline: build_baseline_summary(
            "geometric-component-target-seeded",
            geometric_component_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &geometric_component_target_seeded_summary,
            &geometric_component_target_seeded_solution,
        ),
        geometric_local_component_target_seeded_baseline: build_baseline_summary(
            "geometric-local-component-target-seeded",
            geometric_local_component_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &geometric_local_component_target_seeded_summary,
            &geometric_local_component_target_seeded_solution,
        ),
        geometric_component_stripe_target_seeded_baseline: build_baseline_summary(
            "geometric-component-stripe-target-seeded",
            geometric_component_stripe_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &geometric_component_stripe_target_seeded_summary,
            &geometric_component_stripe_target_seeded_solution,
        ),
        directional_front_band_target_seeded_baseline: build_baseline_summary(
            "directional-front-band-target-seeded",
            directional_front_band_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &directional_front_band_target_seeded_summary,
            &directional_front_band_target_seeded_solution,
        ),
        directional_local_front_band_target_seeded_baseline: build_baseline_summary(
            "directional-local-front-band-target-seeded",
            directional_local_front_band_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &directional_local_front_band_target_seeded_summary,
            &directional_local_front_band_target_seeded_solution,
        ),
        adaptive_component_front_target_seeded_baseline: build_baseline_summary(
            "adaptive-component-front-target-seeded",
            adaptive_component_front_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &adaptive_component_front_target_seeded_summary,
            &adaptive_component_front_target_seeded_solution,
        ),
        shape_gated_front_target_seeded_baseline: build_baseline_summary(
            "shape-gated-front-target-seeded",
            shape_gated_front_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &shape_gated_front_target_seeded_summary,
            &shape_gated_front_target_seeded_solution,
        ),
        shape_gated_local_front_target_seeded_baseline: build_baseline_summary(
            "shape-gated-local-front-target-seeded",
            shape_gated_local_front_phase_plan.phase_count,
            &pcpsp_summary,
            &pcpsp_solution,
            &shape_gated_local_front_target_seeded_summary,
            &shape_gated_local_front_target_seeded_solution,
        ),
        shape_gated_front_rule_sweep,
        shape_gated_local_rule_window_sweep,
        shape_gated_local_rule_front_count_sweep,
        shape_gated_front_count_sweep,
        shape_gated_local_front_count_sweep,
        shape_gated_local_overlap_front_count_sweep,
        shape_gated_local_access_sweep,
        shape_gated_local_access_window_sweep,
        shape_gated_front_progression_sweep,
        shape_gated_front_progression_window_sweep,
        shape_gated_conditional_progression_sweep,
        shape_gated_conditional_window_progression_sweep,
        shape_gated_local_window_sweep,
        shape_gated_dynamic_local_window_sweep,
        adaptive_component_front_threshold_sweep,
        strict_shell_factor_sweep,
        shell_access_sweep,
        lp_cut_band_width_sweep,
        mine_rs_end_to_end: mine_rs_end_to_end.summary,
        mine_rs_vs_cpit_metric_comparison,
        mine_rs_vs_cpit_membership_comparison,
        mine_rs_vs_cpit_period_metric_comparison,
        mine_rs_vs_pcpsp_metric_comparison,
        mine_rs_vs_pcpsp_membership_comparison,
        mine_rs_vs_pcpsp_period_metric_comparison,
        assumptions: vec![
            "marving-info.txt was used to confirm that field_4 is tonnage and field_7 is proc_profit ($/ton), and that mine_cost = 0.9 $/ton.".to_owned(),
            "The candidate precedence template uses the 17-offset Marvin slope pattern (45°/8-niveles): 5 cross at dk=1, 4 diagonal corners at dk=3, 8 near-circle at dk=5.".to_owned(),
            "total_economic_objective = sum((max(proc_profit, 0) - 0.9) × tonnage). Official UPIT target: 1,415,655,436.".to_owned(),
            "The exact UPL comparison uses `marvin.prec` + `marvin.upit` block objective values and solves max-closure directly with the exact backend.".to_owned(),
            "CPIT/PCPSP objective audits apply the MineLib-style discounted objective sum(value × fraction / (1 + discount_rate)^period) over the normalized reference problems and solutions.".to_owned(),
            format!(
                "The internal mine-rs end-to-end candidate rebuilds Marvin economics from field_4/field_5/field_6 and now derives nested-shell × bench phases from a bounded {MARVIN_END_TO_END_FACTOR_COUNT}-factor revenue/cost-aware sweep before routing with ready-frontier."
            ),
            format!(
                "The `strict_shell_factor_sweep` probe reruns the Marvin shell-driven ready-frontier candidate with strict shell-to-shell access over factor counts {:?}.",
                MARVIN_FACTOR_SWEEP_COUNTS
            ),
            format!(
                "The `shell_access_sweep` probe reruns the promoted {MARVIN_END_TO_END_FACTOR_COUNT}-factor shell family under strict sequencing plus several bench-aligned open-access lag settings."
            ),
            format!(
                "The `lp_cut_band_width_sweep` probe reruns LP-guided cut seeding over representative-period band widths {:?}.",
                LP_CUT_BAND_SWEEP_WIDTHS
            ),
            "The `lp-quantile-cut-target-seeded` baseline sorts each phase by representative LP period and then re-cuts it into tonnage-balanced quantiles before target-period seeding.".to_owned(),
            "The `geometric-component-target-seeded` baseline splits each shell×bench phase into planar connected components before target-period seeding.".to_owned(),
            "The `geometric-local-component-target-seeded` baseline keeps the same planar connected components but localizes predecessor links to overlapping/touching predecessor components in plant view before target-period seeding.".to_owned(),
            format!("The `geometric-component-stripe-target-seeded` baseline splits each planar component into up to {GEOMETRIC_COMPONENT_STRIPE_COUNT} dominant-axis stripes before target-period seeding."),
            format!("The `directional-front-band-target-seeded` baseline splits each shell×bench phase into up to {DIRECTIONAL_FRONT_BAND_COUNT} dominant-axis front bands before target-period seeding."),
            format!("The `directional-local-front-band-target-seeded` baseline keeps those front bands but localizes predecessor links by planar overlap/adjacency before target-period seeding."),
            format!("The `adaptive-component-front-target-seeded` baseline only splits large planar components (>= {:.0}% of phase tonnage) into up to {ADAPTIVE_COMPONENT_FRONT_COUNT} dominant-axis fronts before target-period seeding.", ADAPTIVE_COMPONENT_FRONT_MIN_SHARE * 100.0),
            format!("The `shape-gated-front-target-seeded` baseline only splits planar components whose dominant-span is at least {SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN} and aspect ratio is at least {SHAPE_GATED_FRONT_MIN_ASPECT_RATIO:.1}, with up to {SHAPE_GATED_FRONT_COUNT} fronts per selected component, before target-period seeding."),
            "The `shape-gated-local-front-target-seeded` baseline keeps the same shape-gated front split but localizes predecessor links by planar overlap/adjacency between promoted front units before target-period seeding.".to_owned(),
            format!("The `shape_gated_front_rule_sweep` probe reruns that geometric gate over aspect ratios {:?} and dominant spans {:?}.", SHAPE_GATED_FRONT_ASPECT_RATIO_SWEEP, SHAPE_GATED_FRONT_DOMINANT_SPAN_SWEEP),
            format!("The `shape_gated_local_rule_window_sweep` probe reruns that same geometric gate grid under localized predecessors with fixed closest-N windows (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`)."),
            format!("The `shape_gated_local_rule_front_count_sweep` probe reruns localized shape-gated fronts over front-count caps {:?} while fixing the promoted geometric gate (`aspect_ratio >= {SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_ASPECT_RATIO:.1}`, `dominant-span >= {SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_DOMINANT_SPAN}`) and fixed closest-N windows (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`).", SHAPE_GATED_LOCAL_FRONT_COUNT_SWEEP),
            format!("The `shape_gated_front_count_sweep` probe reruns the promoted shape gate over front-count caps {:?}.", SHAPE_GATED_FRONT_COUNT_SWEEP),
            format!("The `shape_gated_local_front_count_sweep` probe reruns localized shape-gated fronts with fixed closest-N predecessor windows (`N={SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW}`) over front-count caps {:?}.", SHAPE_GATED_LOCAL_FRONT_COUNT_SWEEP),
            format!("The `shape_gated_local_overlap_front_count_sweep` probe reruns that same localized front-count sweep with overlap-only predecessors (no touching-neighbor adjacency) and fixed closest-N windows (`N={SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW}`)."),
            "The `shape_gated_local_access_sweep` probe reruns localized shape-gated fronts under two predecessor filters: overlap+adjacency and overlap-only.".to_owned(),
            format!("The `shape_gated_local_access_window_sweep` probe reruns that local-access filter comparison with a fixed closest-N predecessor window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`)."),
            format!(
                "The `shape_gated_front_progression_sweep` probe reruns localized shape-gated fronts over front-progression profiles {:?}.",
                SHAPE_GATED_FRONT_PROGRESSION_SWEEP.map(|profile| profile.label)
            ),
            format!("The `shape_gated_front_progression_window_sweep` probe reruns those front-progression profiles under a fixed closest-N predecessor window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`)."),
            format!("The `shape_gated_conditional_progression_sweep` probe applies the front-loaded profile {:?} only when component aspect ratio exceeds each threshold in {:?}.", SHAPE_GATED_CONDITIONAL_PROGRESSIVE_PROFILE, SHAPE_GATED_CONDITIONAL_PROGRESSIVE_ASPECT_SWEEP),
            format!("The `shape_gated_conditional_window_progression_sweep` probe reruns that same conditional progression under a fixed closest-N predecessor window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`)."),
            format!("The `shape_gated_local_window_sweep` probe reruns localized shape-gated fronts while capping predecessor windows to the closest-N local fronts for N in {:?}.", SHAPE_GATED_LOCAL_PREDECESSOR_WINDOW_SWEEP),
            format!("The `shape_gated_dynamic_local_window_sweep` probe reruns localized shape-gated fronts with a dynamic closest-N predecessor policy: base `N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}` and promoted N by aspect-ratio thresholds {:?}.", SHAPE_GATED_DYNAMIC_LOCAL_WINDOW_SWEEP.map(|(label, _, _)| label)),
            format!("The `adaptive_component_front_threshold_sweep` probe reruns that selective front family over tonnage-share thresholds {:?}.", ADAPTIVE_COMPONENT_FRONT_SHARE_SWEEP),
            "The `lp_bz_bound_artifact` now comes from the in-harness `lp_bz_bound` module: it recomputes the LP proxy objective from normalized assignments and applies a conservative envelope tightened by native resource knapsack plus LP-proxy safeguard metadata.".to_owned(),
            format!("The `lp_bz_lp_kernel_artifact` now comes from the in-harness `lp_bz_lp_kernel` module over a dedicated v8 `{LP_BZ_UNIT_GRANULARITY_LABEL}` schedule normalization, but the benchmark report serializes a compact evidence view: deterministic key counts/labels, representative variable/objective/constraint samples, access-closure diagnostics, and full summary counts instead of every LP row/term."),
            "The `lp_bz_lp_solve_artifact` now records a native in-harness minilp solve over that v8 local-front access/progression LP kernel: full per-period precedence rows plus deterministic cumulative precedence/access-closure capacity-prefix cut diagnostics, with status, solved discounted objective upper bound, and variable-activity diagnostics.".to_owned(),
            "The `lp_bz_integer_candidate_artifact` now comes from the in-harness `lp_bz_rounder` v6 path over the same v8 localized local-front phase plan, plus the deterministic adjacent-swap, period-ejection, and precedence-chain local optimizer and seeded schedule construction via SDK helpers.".to_owned(),
            "The report now exposes explicit `lp_bz_rounder_v6_local_optimizer_diagnostics` for that v8 local optimizer (strategy label, move/improvement counters, iteration budget usage and termination reason) wired directly from the rounder artifacts.".to_owned(),
            format!("The `lp-windowed-exact` baseline solves an exact one-period packing over a rolling LP-guided window of up to {LP_WINDOW_CANDIDATE_SIZE} ready units per iteration."),
            format!("The `lp-cut-target-seeded` baseline first splits shell×bench phases into LP-guided period bands of width {LP_CUT_PERIOD_BAND_WIDTH} before seeding the chunked scheduling problem."),
        ],
        limitations: vec![
            "The internal end-to-end candidate now builds a destination-aware ready-frontier schedule over a normalized Marvin `SchedulingProblem`, but the economic evaluator still aggregates phase cashflow using each block's best destination rather than the routed destination in the candidate schedule.".to_owned(),
            "The heuristic UPIT path is still reported because it remains useful as a cheap baseline; exact parity now comes from the dedicated exact UPL comparison built on `marvin.upit` + `marvin.prec`.".to_owned(),
            format!(
                "The shell-driven phase plan is still a bounded {MARVIN_END_TO_END_FACTOR_COUNT}-factor revenue/cost-aware sweep for Marvin, so it is a reproducible stepping stone toward literature-grade pushbacks rather than the final bibliographic reproduction pipeline."
            ),
            "The `strict_shell_factor_sweep` only varies factor-count density under the current strict shell-to-shell access policy; it does not yet sweep alternative access-lag calibrations or literature-calibrated pushback geometries.".to_owned(),
            "The `shell_access_sweep` only compares the current bench-aligned access implementation against a few small lag settings; it does not yet encode a paper-validated shell-access law or access windows tied to real ramps/geometries.".to_owned(),
            "The `lp_cut_band_width_sweep` only varies LP representative-period bandwidth inside the current shell×bench plan; it does not yet change the cut geometry, predecessor law, or any literature-grounded mining-cut construction.".to_owned(),
            "The `lp-quantile-cut-target-seeded` baseline only balances tonnage inside LP-sorted cuts; it still does not encode any literature-grounded geometric cut construction or mining access surface.".to_owned(),
            "The `geometric-component-target-seeded` baseline only uses planar connected components inside each bench; it still lacks the richer bench-phase/access design described in the literature.".to_owned(),
            "The `geometric-local-component-target-seeded` baseline localizes predecessor links with simple planar overlap/adjacency only; it is still a benchmark-side geometric access heuristic, not yet a literature-calibrated bench-phase/ramp design.".to_owned(),
            format!("The `geometric-component-stripe-target-seeded` baseline uses a fixed cap of {GEOMETRIC_COMPONENT_STRIPE_COUNT} dominant-axis stripes per planar component; it is still a benchmark-side geometric front-progression heuristic, not yet a calibrated bench-phase design."),
            format!("The `directional-front-band-target-seeded` baseline uses a fixed cap of {DIRECTIONAL_FRONT_BAND_COUNT} whole-phase front bands along the dominant planar axis; it is still a benchmark-side heuristic, not yet a calibrated bench-phase design."),
            "The `directional-local-front-band-target-seeded` baseline still relies on simple planar overlap/adjacency to localize whole-phase front predecessors; it is a benchmark-side heuristic, not yet a calibrated access law.".to_owned(),
            format!("The `adaptive-component-front-target-seeded` baseline uses a fixed {:.0}% tonnage-share threshold and a fixed cap of {ADAPTIVE_COMPONENT_FRONT_COUNT} local fronts, so it remains a benchmark-side heuristic rather than a literature-calibrated component/front design.", ADAPTIVE_COMPONENT_FRONT_MIN_SHARE * 100.0),
            format!("The `shape-gated-front-target-seeded` baseline uses fixed geometric gates (dominant-span >= {SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN}, aspect ratio >= {SHAPE_GATED_FRONT_MIN_ASPECT_RATIO:.1}) and a fixed front cap ({SHAPE_GATED_FRONT_COUNT}) rather than a calibrated mining-front rule, so it remains a benchmark-side heuristic."),
            "The `shape-gated-local-front-target-seeded` baseline only localizes predecessor links with planar overlap/adjacency over the current shape-gated fronts; it is still a benchmark-side access heuristic, not yet a paper-calibrated ramp/access law.".to_owned(),
            "The `shape_gated_front_rule_sweep` only calibrates two simple geometric gates for the current shape-aware split heuristic; it does not yet vary the front count or introduce a paper-calibrated access law.".to_owned(),
            format!("The `shape_gated_local_rule_window_sweep` only repeats the same geometric gate calibration under fixed localized predecessor windows (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`); it still does not model dynamic, ramp-calibrated access laws."),
            format!("The `shape_gated_local_rule_front_count_sweep` only varies front-count under one promoted local geometric gate (`aspect_ratio >= {SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_ASPECT_RATIO:.1}`, `dominant-span >= {SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_DOMINANT_SPAN}`) and a fixed closest-N window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`); it still does not model dynamic, ramp-calibrated access laws."),
            "The `shape_gated_front_count_sweep` only calibrates the front-count cap under the current promoted shape gate; it does not yet tune access-law behavior or literature-derived front progression rules.".to_owned(),
            format!("The `shape_gated_local_front_count_sweep` only varies front-count with localized predecessors under a fixed closest-N window (`N={SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW}`); it does not yet calibrate dynamic access windows or ramp-reachability constraints."),
            format!("The `shape_gated_local_overlap_front_count_sweep` only repeats the localized front-count calibration under overlap-only predecessor filtering with fixed closest-N windows (`N={SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW}`); it still does not encode ramp-aware reachability or time-varying access laws."),
            "The `shape_gated_local_access_sweep` only toggles overlap-only vs overlap+adjacency predecessor filters under the current promoted shape gate; it does not yet model literature-derived ramp availability or temporal access windows.".to_owned(),
            format!("The `shape_gated_local_access_window_sweep` only toggles overlap-only vs overlap+adjacency under a fixed closest-N window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`); it still does not encode ramp-aware reachability or time-varying access laws."),
            "The `shape_gated_front_progression_sweep` only toggles fixed cumulative tonnage profiles for three fronts under the current promoted shape gate/localization; it does not yet encode a literature-calibrated dynamic front progression law.".to_owned(),
            format!("The `shape_gated_front_progression_window_sweep` only combines the same fixed front-progression profiles with a fixed closest-N predecessor window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`); it still does not model dynamic, ramp-calibrated progression/access laws."),
            "The `shape_gated_conditional_progression_sweep` only gates one front-loaded profile by aspect-ratio thresholds; it does not yet condition progression on richer geometry, bench context, or access dynamics.".to_owned(),
            format!("The `shape_gated_conditional_window_progression_sweep` only combines the existing conditional profile with a fixed closest-N predecessor window (`N={SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT}`); it still does not model dynamic, ramp-calibrated access/progression laws."),
            "The `shape_gated_local_window_sweep` only caps localized predecessor fan-in by closest-N fronts; it does not yet encode ramp-aware reachability or time-dependent access windows.".to_owned(),
            format!("The `shape_gated_dynamic_local_window_sweep` only applies a simple aspect-ratio-triggered closest-N promotion on top of the current localized shape-gated split; it still does not model true ramp network reachability or temporal access dynamics."),
            "The `adaptive_component_front_threshold_sweep` only calibrates the tonnage-share trigger for the current selective split heuristic; it does not yet vary the front count, axis rule, or a paper-calibrated access law.".to_owned(),
            format!("The current `lp_bz_bound_artifact` still reports the conservative native resource envelope; although the new `lp_bz_lp_solve_artifact` now runs on a dedicated v8 `{LP_BZ_UNIT_GRANULARITY_LABEL}` normalization and provides a tighter in-harness LP kernel objective for gap reporting, full LP/BZ cut families are still pending."),
            format!("The current `lp_bz_lp_solve_artifact` already enforces full per-period precedence plus the current cumulative precedence/access-closure capacity-prefix cut family on the v8 `{LP_BZ_UNIT_GRANULARITY_LABEL}` access/progression plan, but it still stops short of a bibliographic LP/BZ model with richer external cut families."),
            "The `lp_bz_integer_candidate_artifact` still comes from deterministic benchmark-side topological round/repair via `lp_bz_rounder` v6 on that same localized local-front plan, plus the v8 adjacent-swap, period-ejection, and precedence-chain local optimizer; it is explicit and reproducible, but still not a full bibliographic LP/BZ optimizer/rounder implementation.".to_owned(),
            "The `lp-shell-seeded` baseline aggregates LPpcpsp block periods onto shell-driven phases and repairs precedence by delaying successor phases when needed; it is still a benchmark-side seeding heuristic, not yet a full LP/BZ rounder.".to_owned(),
            "The `lp-target-period-seeded` baseline keeps the same LP-derived phase targets but schedules chunked units with a target-aware ready-frontier over the normalized `SchedulingProblem`; it is still a heuristic repair, not a bibliographic LP/BZ implementation.".to_owned(),
            "The `lp-staggered-target-seeded` baseline further staggers chunk targets backward inside each phase chain before applying the same target-aware ready-frontier; it is still a heuristic repair, not a bibliographic LP/BZ implementation.".to_owned(),
            "The `lp-windowed-exact` baseline is still a rolling-horizon heuristic: each period solves only a local exact packing over a capped ready window, so it is not yet a full-horizon LP/BZ rounder.".to_owned(),
            "The `lp-cut-target-seeded` baseline is still benchmark-side and uses LP representative periods to define intra-phase cuts; it is closer to mining cuts than the base shell×bench plan, but it is not yet a literature-calibrated pushback/cut generator.".to_owned(),
            "The prec template was reverse-engineered from the reference file; formal proof of completeness against the MineLib algorithm is pending (pending MR-154).".to_owned(),
        ],
    };

    write_report_to_path(&output_path, &output)?;
    maybe_print_report(&output)?;

    Ok(())
}

fn build_focused_mr187_refresh_output(
    repo_root: &Path,
    dataset_dir: &Path,
    prec_path: &Path,
    blocks_path: &Path,
    pcpsp_problem_path: &Path,
    pcpsp_solution_path: &Path,
    lp_pcpsp_solution_path: &Path,
) -> Result<FocusedMr187RefreshOutput, MineError> {
    let model = read_benchmark_blocks(blocks_path, "marvin")?;
    let reference_prec = read_marvin_precedence_graph(prec_path, &model)?;
    let pcpsp_problem = read_marvin_pcpsp_problem(pcpsp_problem_path, &model)?;
    let pcpsp_solution = read_marvin_pcpsp_solution(pcpsp_solution_path, &model)?;
    let lp_pcpsp_solution = read_marvin_lp_pcpsp_solution(lp_pcpsp_solution_path, &model)?;
    let pcpsp_summary = summarize_marvin_schedule_solution(&pcpsp_problem, &pcpsp_solution)?;
    let lp_pcpsp_summary = summarize_marvin_schedule_solution(&pcpsp_problem, &lp_pcpsp_solution)?;
    let value_column = ColumnId::new("field_7")?;
    let tonnage_column = ColumnId::new("field_4")?;
    let base_phase_plan = build_mine_rs_end_to_end_phase_plan(&model, &reference_prec)?;
    let tonnage_by_linear_index = build_linear_index_float_lookup(&model, &tonnage_column)?;
    let lp_representative_period_by_block = representative_period_by_block(&lp_pcpsp_solution);
    let lp_bz_benchmark = build_lp_bz_access_progression_artifacts(
        &model,
        &base_phase_plan,
        &pcpsp_problem,
        &tonnage_by_linear_index,
    )?;
    let (lp_bz_round_repair_artifacts, lp_bz_round_repair_schedule) =
        build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
            &lp_bz_benchmark.phase_plan,
            &lp_bz_benchmark.scheduling_problem,
            &lp_pcpsp_solution,
            None,
            Metadata::new(),
        )?;
    let lp_bz_rounder_v6_local_optimizer_diagnostics =
        build_lp_bz_rounder_v6_local_optimizer_diagnostics(&lp_bz_round_repair_artifacts);
    let lp_bz_round_repair_period_memberships = build_candidate_period_memberships(
        &model,
        &lp_bz_benchmark.phase_plan,
        &lp_bz_round_repair_schedule,
        &tonnage_column,
    )?;
    let lp_bz_round_repair_solution =
        build_candidate_pcpsp_solution(&pcpsp_problem, &lp_bz_round_repair_period_memberships)?;
    let lp_bz_round_repair_summary =
        summarize_marvin_schedule_solution(&pcpsp_problem, &lp_bz_round_repair_solution)?;
    let lp_bz_integer_candidate_artifact = build_baseline_summary(
        "lp-bz-round-repair-local-front-seeded",
        lp_bz_benchmark.phase_plan.phase_count,
        &pcpsp_summary,
        &pcpsp_solution,
        &lp_bz_round_repair_summary,
        &lp_bz_round_repair_solution,
    );
    let lp_bz_pushback_bench_localized_cut_experiment =
        build_focused_pushback_bench_localized_cut_experiment(
            &model,
            &base_phase_plan,
            &pcpsp_problem,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_pcpsp_solution,
            lp_pcpsp_solution_path,
            repo_root,
            &tonnage_column,
            &tonnage_by_linear_index,
            &lp_bz_integer_candidate_artifact,
        )?;
    let mut lp_bz_v9_local_front_band_experiment = None::<FocusedLpBzVariantExperiment>;
    let mut lp_bz_v9_local_front_band_width_sweep = Vec::new();
    for period_band_width in LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_SWEEP_WIDTHS {
        let experiment = build_focused_lp_bz_local_front_band_experiment(
            &model,
            &base_phase_plan,
            &pcpsp_problem,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_pcpsp_solution,
            lp_pcpsp_solution_path,
            repo_root,
            &tonnage_column,
            &tonnage_by_linear_index,
            &lp_representative_period_by_block,
            period_band_width,
            LpBzBandPredecessorLinkPolicy::PredecessorLastCut,
        )?;
        if period_band_width == LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH {
            lp_bz_v9_local_front_band_experiment = Some(experiment);
        } else {
            lp_bz_v9_local_front_band_width_sweep
                .push(build_lp_bz_local_front_band_width_sweep_entry(&experiment));
        }
    }
    let lp_bz_v9_local_front_band_experiment =
        lp_bz_v9_local_front_band_experiment.ok_or_else(|| {
            MineError::validation(format!(
                "Focused MR-187 refresh failed to build the v9 local-front period-band experiment for width {}.",
                LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH
            ))
        })?;
    lp_bz_v9_local_front_band_width_sweep.push(build_lp_bz_local_front_band_width_sweep_entry(
        &lp_bz_v9_local_front_band_experiment,
    ));
    lp_bz_v9_local_front_band_width_sweep.sort_by_key(|entry| entry.period_band_width);
    let mut lp_bz_v9_local_front_band_link_policy_sweep =
        vec![build_lp_bz_local_front_band_link_policy_sweep_entry(
            &lp_bz_v9_local_front_band_experiment,
        )];
    for predecessor_cut_link_policy in [
        LpBzBandPredecessorLinkPolicy::PredecessorFirstCut,
        LpBzBandPredecessorLinkPolicy::AllPredecessorCuts,
    ] {
        let experiment = build_focused_lp_bz_local_front_band_experiment(
            &model,
            &base_phase_plan,
            &pcpsp_problem,
            &pcpsp_summary,
            &pcpsp_solution,
            &lp_pcpsp_solution,
            lp_pcpsp_solution_path,
            repo_root,
            &tonnage_column,
            &tonnage_by_linear_index,
            &lp_representative_period_by_block,
            LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH,
            predecessor_cut_link_policy,
        )?;
        lp_bz_v9_local_front_band_link_policy_sweep.push(
            build_lp_bz_local_front_band_link_policy_sweep_entry(&experiment),
        );
    }
    let lp_bz_bound_artifacts = compute_lp_bz_bound_artifacts(
        &pcpsp_problem,
        &lp_pcpsp_solution,
        lp_pcpsp_solution_path,
        repo_root,
        lp_bz_benchmark.scheduling_problem.units().len(),
        lp_bz_benchmark
            .scheduling_problem
            .units()
            .iter()
            .map(|unit| unit.predecessor_unit_ids().len())
            .sum(),
        LP_BZ_UNIT_GRANULARITY_LABEL,
    )?;
    let lp_bz_lp_kernel_artifact =
        build_lp_bz_lp_kernel_artifact(&lp_bz_benchmark.scheduling_problem)?;
    let lp_bz_lp_solve_artifact =
        build_skipped_focused_lp_bz_lp_solve_artifact(&lp_bz_lp_kernel_artifact);
    validate_lp_bz_artifact_coherence(
        &lp_bz_bound_artifacts.lp_bz_inputs,
        &lp_bz_bound_artifacts.lp_bz_bound_artifact,
        &lp_bz_lp_kernel_artifact,
    )?;
    let lp_bz_gap_metrics = build_lp_bz_gap_metrics(
        &lp_bz_bound_artifacts.lp_bz_bound_artifact,
        &lp_bz_lp_solve_artifact,
        &lp_bz_integer_candidate_artifact.candidate_pcpsp_summary,
        pcpsp_summary.discounted_objective,
        lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective,
    );
    let output = FocusedMr187RefreshOutput {
        report_mode: MarvinBenchmarkMode::FocusedMr187
            .report_mode_label()
            .to_owned(),
        dataset_dir: relative_or_display(dataset_dir, repo_root),
        reference_prec_path: relative_or_display(prec_path, repo_root),
        reference_pcpsp_problem_path: relative_or_display(pcpsp_problem_path, repo_root),
        reference_pcpsp_solution_path: relative_or_display(pcpsp_solution_path, repo_root),
        reference_lp_pcpsp_solution_path: relative_or_display(lp_pcpsp_solution_path, repo_root),
        value_column: value_column.to_string(),
        tonnage_column: tonnage_column.to_string(),
        pcpsp_reference: build_schedule_reference_artifact_summary(
            &pcpsp_problem,
            &pcpsp_summary,
            OFFICIAL_PCPSP_OBJECTIVE,
        ),
        lp_pcpsp_reference: build_schedule_reference_artifact_summary(
            &pcpsp_problem,
            &lp_pcpsp_summary,
            OFFICIAL_LP_PCPSP_OBJECTIVE,
        ),
        lp_bz_inputs: lp_bz_bound_artifacts.lp_bz_inputs,
        lp_bz_bound_artifact: lp_bz_bound_artifacts.lp_bz_bound_artifact,
        lp_bz_lp_kernel_artifact: compact_lp_bz_lp_kernel_artifact(&lp_bz_lp_kernel_artifact),
        lp_bz_lp_solve_artifact,
        lp_bz_front_progression_label: MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE
            .label
            .to_owned(),
        lp_bz_integer_candidate_artifact,
        lp_bz_rounder_v6_local_optimizer_diagnostics,
        lp_bz_gap_metrics,
        lp_bz_pushback_bench_localized_cut_experiment,
        lp_bz_v9_local_front_band_experiment,
        lp_bz_v9_local_front_band_width_sweep,
        lp_bz_v9_local_front_band_link_policy_sweep,
        comparison_classification: "exploratory-local".to_owned(),
        comparability_gaps: build_mr187_refresh_comparability_gaps(),
        assumptions: build_mr187_refresh_assumptions(),
        limitations: build_mr187_refresh_limitations(),
    };
    validate_focused_mr187_refresh_output(&output)?;
    Ok(output)
}

fn build_schedule_reference_artifact_summary(
    problem: &MarvinScheduleProblem,
    summary: &MarvinScheduleSolutionSummary,
    official_objective: f64,
) -> ScheduleReferenceArtifactSummary {
    ScheduleReferenceArtifactSummary {
        period_count: problem.period_count,
        destination_count: problem.destination_count,
        resource_constraint_count: problem.resource_constraint_count,
        discount_rate: problem.discount_rate,
        official_objective,
        objective_gap_vs_official: (summary.discounted_objective - official_objective).abs(),
        solution_summary: summary.clone(),
    }
}

fn build_mr187_refresh_assumptions() -> Vec<String> {
    vec![
        "marving-info.txt se reutiliza para fijar `field_4` como tonelaje y `field_7` como `proc_profit` ($/ton) durante el refresh focalizado.".to_owned(),
        format!(
            "El modo `focused-mr187` ya mantiene {} como ruta LP/BZ benchmark-side activa del refresh; `{LP_BZ_UNIT_GRANULARITY_LABEL}` queda solo como scaffold/reference explícito del optimizador local, mientras la relajación `marvin.LPpcpsp` se sigue usando para recalcular bound, kernel LP y candidato entero sin reejecutar sweeps/baselines ajenos a MR-187.",
            format_promoted_lp_bz_family_status_summary(
                PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL,
                MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
                LP_BZ_UNIT_GRANULARITY_LABEL,
            ),
        ),
        format!(
            "El mismo refresh focalizado también reporta un sweep experimental v9 `{LP_BZ_V9_UNIT_GRANULARITY_LABEL}` para anchos de banda LP {:?} sobre los localized fronts v8 ya fijados al contrato `{}`, como evidencia benchmark-side side-by-side sobre si la granularidad extra mejora el candidato en este contexto.",
            LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_SWEEP_WIDTHS,
            MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE.label,
        ),
        format!(
            "Sobre el ancho v9 focalizado `period_band_width = {LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH}`, el reporte ahora compara side-by-side políticas benchmark-side de enlace entre cuts predecesores (`predecessor-last-cut`, `predecessor-first-cut`, `all-predecessor-cuts`) para aislar si la degradación proviene del cableado de precedencia/acceso."
        ),
        format!(
            "La familia promotora `{PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL}` del refresh focalizado reutiliza shell×bench + acceso local v8, pero permite dividir componentes elongadas aunque una fase tenga un solo componente plano, para acercarse a mining cuts bench-localized más paper-like; la procedencia input/aggregation sigue siendo benchmark-side y queda separada entre el bloque fuente auditado y el ruteo/destino ya materializado por el refresh."
        ),
        format!(
            "El modo focalizado preserva la misma familia base de fases nested-shell × bench de {MARVIN_END_TO_END_FACTOR_COUNT} revenue factors que usa el reporte completo, pero no reejecuta la baseline `ready frontier`; la ruta activa se limita al refresh LP/BZ."
        ),
    ]
}

fn build_mr187_refresh_limitations() -> Vec<String> {
    vec![
        "El modo `focused-mr187` omite sweeps y baselines ajenos al refresh LP/BZ; usar el modo `full` cuando se necesite el `comparison-report.json` exhaustivo.".to_owned(),
        format!(
            "La ruta LP/BZ principal del refresh focalizado mantiene {}; por eso la familia promovida sigue siendo benchmark-side `exploratory-local` y no debe leerse como cierre paper-grade ni como lógica reusable del core.",
            format_promoted_lp_bz_family_status_summary(
                PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL,
                MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
                LP_BZ_UNIT_GRANULARITY_LABEL,
            ),
        ),
        "El `lp_bz_lp_solve_artifact` del modo focalizado queda marcado explícitamente como `skipped` para mantener el refresh operativo en este entorno; por eso el gap efectivo usa solo el `native-resource-envelope` y no el solve nativo del kernel.".to_owned(),
        "El campo `candidate_vs_ready_frontier_objective_gap` se fija en `0.0` en el modo focalizado porque la baseline `ready frontier` no se reejecuta; el refresh queda orientado a bound/kernel/candidato LP/BZ y no a la comparación completa contra todas las baselines.".to_owned(),
        "El `lp_bz_integer_candidate_artifact` del modo focalizado conserva el round/repair topológico v6 y el schedule seeded, pero omite el optimizador local v8 más costoso; los diagnósticos lo marcan explícitamente como `skipped-focused-refresh-runtime`, así que el candidato puede quedar más conservador que en `full`.".to_owned(),
        format!(
            "El sweep v9 experimental `{LP_BZ_V9_UNIT_GRANULARITY_LABEL}` del refresh focalizado todavía no reemplaza la ruta base v8 con contrato `{}`: solo agrega evidencia side-by-side por ancho de banda LP sobre granularidad, round/repair y gap antes de decidir una promoción.",
            MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE.label,
        ),
        "El nuevo `lp_bz_v9_local_front_band_link_policy_sweep` sigue siendo puramente benchmark-side: solo cambia cómo el primer cut de cada fase refinada se enlaza con los cuts de sus fases predecesoras, sin introducir una ley bibliográfica de accesos/rampas ni mover lógica al core.".to_owned(),
    ]
}

fn build_mr187_refresh_comparability_gaps() -> Vec<String> {
    vec![
        format!(
            "La ruta LP/BZ auditada ya se promueve como {}; aun así sigue siendo una familia benchmark-side derivada de selección `cpit-solution` + `nested-shell-bench`, no un set bibliográfico de mining cuts/pushbacks calibrados."
            ,
            format_promoted_lp_bz_family_status_summary(
                PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL,
                MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
                LP_BZ_UNIT_GRANULARITY_LABEL,
            )
        ),
        format!(
            "La variante v9 `{LP_BZ_V9_UNIT_GRANULARITY_LABEL}` sigue siendo igualmente benchmark-side: añade bandas de periodo LP sobre los localized fronts, pero todavía no implementa un generador bibliográfico de mining cuts/pushbacks calibrados."
        ),
        "El modo focalizado omite el solve LP nativo in-harness para mantener el refresh operativo en este entorno; por eso el gap efectivo vuelve al `native-resource-envelope` y no al `min(native-resource-envelope, native-lp-kernel)` del reporte completo.".to_owned(),
        "La baseline `ready frontier` tampoco se recalcula en el modo focalizado; por eso `candidate_vs_ready_frontier_objective_gap` queda neutralizado en `0.0` y el refresh se centra en bound/kernel/candidato LP/BZ.".to_owned(),
        "El candidato entero del modo focalizado se obtiene por round/repair determinista benchmark-side sin reejecutar el optimizador local v8; por eso el objetivo/gap refrescado debe leerse como una evidencia conservadora frente al `full` actual.".to_owned(),
    ]
}

fn build_skipped_focused_lp_bz_lp_solve_artifact(
    artifact: &LpBzLpKernelArtifact,
) -> LpBzLpSolveArtifact {
    LpBzLpSolveArtifact {
        solver_label: "skipped-focused-refresh".to_owned(),
        solve_status: LpBzLpSolveStatus::Skipped,
        discounted_objective_bound: None,
        variable_count: artifact.variable_index.variable_count,
        active_variable_count: 0,
        min_positive_variable_value: None,
        max_variable_value: None,
        precedence_diagnostics: LpBzPrecedenceSolveDiagnostics {
            strategy: LpBzPrecedenceEnforcementStrategy::None,
            max_enforced_precedence_rows: 0,
            total_precedence_rows: artifact.constraints.summary.precedence_row_count,
            enforced_precedence_rows: 0,
            skipped_precedence_rows: artifact.constraints.summary.precedence_row_count,
            enforced_period_indices: Vec::new(),
            skipped_period_indices: (0..artifact.period_count).collect(),
        },
        cut_diagnostics: LpBzCutSolveDiagnostics {
            strategy: LpBzCutTighteningStrategy::None,
            total_generated_row_count: 0,
            total_applied_row_count: 0,
            total_skipped_row_count: 0,
            families: Vec::new(),
        },
        limitations: vec![
            "Focused MR-187 refresh intentionally skips the native LP solve so the benchmark can regenerate backlog evidence in constrained environments; use full mode for the in-harness minilp solve artifact."
                .to_owned(),
        ],
    }
}

fn validate_focused_mr187_refresh_output(
    output: &FocusedMr187RefreshOutput,
) -> Result<(), MineError> {
    validate_mr187_refresh_contract(
        &output.report_mode,
        &output
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary,
        &output.lp_bz_gap_metrics,
        &output.comparison_classification,
        &output.comparability_gaps,
    )?;
    validate_mr187_refresh_contract(
        &output.report_mode,
        &output
            .lp_bz_pushback_bench_localized_cut_experiment
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary,
        &output
            .lp_bz_pushback_bench_localized_cut_experiment
            .lp_bz_gap_metrics,
        &output.comparison_classification,
        &output.comparability_gaps,
    )?;
    validate_pushback_bench_localized_cut_refinement_diagnostics(
        &output
            .lp_bz_pushback_bench_localized_cut_experiment
            .phase_refinement_diagnostics,
    )?;
    validate_pushback_bench_localized_cut_calibration_sweep(
        &output
            .lp_bz_pushback_bench_localized_cut_experiment
            .calibration_sweep,
    )?;
    validate_mr187_refresh_contract(
        &output.report_mode,
        &output
            .lp_bz_v9_local_front_band_experiment
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary,
        &output
            .lp_bz_v9_local_front_band_experiment
            .lp_bz_gap_metrics,
        &output.comparison_classification,
        &output.comparability_gaps,
    )?;
    validate_lp_bz_period_band_refinement_diagnostics(
        &output
            .lp_bz_v9_local_front_band_experiment
            .phase_refinement_diagnostics,
    )?;
    if output.lp_bz_v9_local_front_band_link_policy_sweep.len() < 3 {
        return Err(MineError::validation(
            "Focused MR-187 refresh must report predecessor-link evidence for the v9 local-front period-band sweep."
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_pushback_bench_localized_cut_refinement_diagnostics(
    diagnostics: &PushbackBenchLocalizedCutRefinementDiagnostics,
) -> Result<(), MineError> {
    if diagnostics.base_phase_count == 0 {
        return Err(MineError::validation(
            "Pushback bench-localized cut diagnostics require at least one shell×bench base phase."
                .to_owned(),
        ));
    }
    if diagnostics.total_cut_phase_count < diagnostics.base_phase_count {
        return Err(MineError::validation(
            "Pushback bench-localized cut diagnostics cannot reduce the number of shell×bench phases."
                .to_owned(),
        ));
    }
    if diagnostics.refined_base_phase_count == 0 {
        return Err(MineError::validation(
            "Pushback bench-localized cut diagnostics must show at least one shell×bench phase refined into multiple cuts."
                .to_owned(),
        ));
    }
    if diagnostics.refined_single_component_phase_count == 0 {
        return Err(MineError::validation(
            "Pushback bench-localized cut diagnostics must show at least one single-component shell×bench phase refined into multiple cuts."
                .to_owned(),
        ));
    }
    if diagnostics.max_cut_count_per_base_phase < 2 {
        return Err(MineError::validation(
            "Pushback bench-localized cut diagnostics must report at least one shell×bench phase with two or more cuts."
                .to_owned(),
        ));
    }
    if diagnostics.average_cut_count_per_base_phase < 1.0 {
        return Err(MineError::validation(
            "Pushback bench-localized cut average cut count per shell×bench phase must be at least 1.0."
                .to_owned(),
        ));
    }
    if diagnostics.realized_front_count_histogram.is_empty() {
        return Err(MineError::validation(
            "Pushback bench-localized cut diagnostics must report a realized-front histogram."
                .to_owned(),
        ));
    }
    if diagnostics.exact_three_front_failure_count > diagnostics.exact_three_front_candidate_count {
        return Err(MineError::validation(
            "Pushback bench-localized cut exact-three-front failures cannot exceed exact-three candidates."
                .to_owned(),
        ));
    }
    let failure_histogram_total = diagnostics
        .exact_three_front_failure_realized_front_histogram
        .values()
        .copied()
        .sum::<usize>();
    if failure_histogram_total != diagnostics.exact_three_front_failure_count {
        return Err(MineError::validation(
            "Pushback bench-localized cut exact-three-front failure histogram must sum to the reported failure count."
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_pushback_bench_localized_cut_calibration_sweep(
    sweep: &[PushbackBenchLocalizedCutSweepEntry],
) -> Result<(), MineError> {
    if sweep.len() < 5 {
        return Err(MineError::validation(
            "Pushback bench-localized cut refresh must report a focused calibration sweep with at least five candidates."
                .to_owned(),
        ));
    }
    let first_builder_point_count = sweep
        .iter()
        .filter(|entry| entry.is_first_builder_point)
        .count();
    if first_builder_point_count != 1 {
        return Err(MineError::validation(format!(
            "Pushback bench-localized cut calibration sweep must flag exactly one first builder point (got {first_builder_point_count})."
        )));
    }
    let best_candidate_count = sweep.iter().filter(|entry| entry.is_best_candidate).count();
    if best_candidate_count != 1 {
        return Err(MineError::validation(format!(
            "Pushback bench-localized cut calibration sweep must flag exactly one best candidate (got {best_candidate_count})."
        )));
    }
    Ok(())
}

fn validate_lp_bz_period_band_refinement_diagnostics(
    diagnostics: &LpBzPeriodBandRefinementDiagnostics,
) -> Result<(), MineError> {
    if diagnostics.localized_front_phase_count == 0 {
        return Err(MineError::validation(
            "LP/BZ v9 period-band diagnostics require at least one localized-front phase."
                .to_owned(),
        ));
    }
    if diagnostics.total_period_band_phase_count < diagnostics.localized_front_phase_count {
        return Err(MineError::validation(
            "LP/BZ v9 period-band diagnostics cannot reduce the number of localized-front phases."
                .to_owned(),
        ));
    }
    if diagnostics.refined_localized_front_phase_count == 0 {
        return Err(MineError::validation(
            "LP/BZ v9 period-band diagnostics must show at least one localized-front phase refined into multiple LP period bands."
                .to_owned(),
        ));
    }
    if diagnostics.max_cut_count_per_localized_front < 2 {
        return Err(MineError::validation(
            "LP/BZ v9 period-band diagnostics must report at least one localized-front phase with two or more cuts."
                .to_owned(),
        ));
    }
    if diagnostics.average_cut_count_per_localized_front < 1.0 {
        return Err(MineError::validation(
            "LP/BZ v9 average cut count per localized front must be at least 1.0.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_mr187_refresh_contract(
    report_mode: &str,
    candidate_summary: &MarvinScheduleSolutionSummary,
    gap_metrics: &LpBzGapMetrics,
    comparison_classification: &str,
    comparability_gaps: &[String],
) -> Result<(), MineError> {
    if report_mode != MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL {
        return Err(MineError::validation(format!(
            "Focused MR-187 refresh report_mode must be `{MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL}`, received `{report_mode}`."
        )));
    }
    if candidate_summary.fractional_assignment_count != 0 {
        return Err(MineError::validation(
            "Focused MR-187 refresh candidate must remain integral (`fractional_assignment_count = 0`)."
                .to_owned(),
        ));
    }
    let expected_gap =
        gap_metrics.effective_discounted_objective_bound - candidate_summary.discounted_objective;
    if (gap_metrics.bound_to_candidate_absolute_gap - expected_gap).abs() > 1.0e-6 {
        return Err(MineError::validation(
            "Focused MR-187 refresh gap metrics are inconsistent with the effective LP/BZ bound and candidate objective."
                .to_owned(),
        ));
    }
    if gap_metrics.bound_to_candidate_relative_gap < -1.0e-9 {
        return Err(MineError::validation(
            "Focused MR-187 refresh relative gap must be non-negative.".to_owned(),
        ));
    }
    if comparison_classification == "paper-comparable" && !comparability_gaps.is_empty() {
        return Err(MineError::validation(
            "Focused MR-187 refresh comparability gaps must be empty for `paper-comparable` runs."
                .to_owned(),
        ));
    }
    if comparison_classification == "exploratory-local" && comparability_gaps.is_empty() {
        return Err(MineError::validation(
            "Focused MR-187 refresh must explain comparability gaps when classified as `exploratory-local`."
                .to_owned(),
        ));
    }
    Ok(())
}

fn write_report_to_path<T: Serialize>(
    output_path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report_file = fs::File::create(output_path)?;
    let mut report_writer = BufWriter::new(report_file);
    write_pretty_json(&mut report_writer, value)?;
    report_writer.flush()?;
    eprintln!("comparison report written to {}", output_path.display());
    Ok(())
}

fn maybe_print_report<T: Serialize>(value: &T) -> Result<(), Box<dyn std::error::Error>> {
    if env::var_os("MARVIN_BENCHMARK_PRINT_REPORT").is_some() {
        let stdout = std::io::stdout();
        let mut stdout_lock = stdout.lock();
        write_pretty_json(&mut stdout_lock, value)?;
        writeln!(&mut stdout_lock)?;
    }
    Ok(())
}

fn write_pretty_json<W: Write, T: Serialize>(writer: W, value: &T) -> serde_json::Result<()> {
    serde_json::to_writer_pretty(writer, value)
}

fn compact_lp_bz_lp_kernel_artifact(
    artifact: &LpBzLpKernelArtifact,
) -> CompactLpBzLpKernelArtifact {
    let sampled_entry_count = artifact
        .variable_index
        .entries
        .len()
        .min(LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT);
    let period_labels = artifact
        .variable_index
        .entries
        .iter()
        .map(|entry| entry.period_label.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let destination_ids = artifact
        .variable_index
        .entries
        .iter()
        .map(|entry| entry.key.destination_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let unit_id_examples = artifact
        .variable_index
        .entries
        .iter()
        .map(|entry| entry.key.unit_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT)
        .collect();
    let sample_entries = artifact
        .variable_index
        .entries
        .iter()
        .take(LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT)
        .map(|entry| CompactLpBzLpKernelVariableEntry {
            variable_index: entry.variable_index,
            unit_id: entry.key.unit_id.clone(),
            destination_id: entry.key.destination_id.clone(),
            period_index: entry.key.period_index,
            period_label: entry.period_label.clone(),
        })
        .collect();

    let variable_lookup = artifact
        .variable_index
        .entries
        .iter()
        .map(|entry| (entry.variable_index, entry))
        .collect::<BTreeMap<_, _>>();
    let min_coefficient = artifact
        .objective
        .coefficients
        .iter()
        .map(|coefficient| coefficient.coefficient)
        .reduce(f64::min);
    let max_coefficient = artifact
        .objective
        .coefficients
        .iter()
        .map(|coefficient| coefficient.coefficient)
        .reduce(f64::max);
    let min_discount_factor = artifact
        .objective
        .coefficients
        .iter()
        .map(|coefficient| coefficient.discount_factor)
        .reduce(f64::min);
    let max_discount_factor = artifact
        .objective
        .coefficients
        .iter()
        .map(|coefficient| coefficient.discount_factor)
        .reduce(f64::max);
    let sample_coefficients = artifact
        .objective
        .coefficients
        .iter()
        .take(LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT)
        .filter_map(|coefficient| {
            variable_lookup
                .get(&coefficient.variable_index)
                .map(|entry| CompactLpBzLpKernelObjectiveCoefficient {
                    variable_index: coefficient.variable_index,
                    unit_id: entry.key.unit_id.clone(),
                    destination_id: entry.key.destination_id.clone(),
                    period_label: entry.period_label.clone(),
                    coefficient: coefficient.coefficient,
                    undiscounted_value: coefficient.undiscounted_value,
                    discount_factor: coefficient.discount_factor,
                })
        })
        .collect::<Vec<_>>();

    let mut capacity_row_examples = 0usize;
    let mut activation_row_examples = 0usize;
    let mut precedence_row_examples = 0usize;
    let sample_rows = artifact
        .constraints
        .rows
        .iter()
        .filter(|row| match row.kind {
            LpBzLpKernelConstraintKind::CapacityUpper
            | LpBzLpKernelConstraintKind::CapacityLower => {
                if capacity_row_examples < LP_BZ_KERNEL_REPORT_ROW_KIND_SAMPLE_LIMIT {
                    capacity_row_examples += 1;
                    true
                } else {
                    false
                }
            }
            LpBzLpKernelConstraintKind::ActivationUpper => {
                if activation_row_examples < LP_BZ_KERNEL_REPORT_ROW_KIND_SAMPLE_LIMIT {
                    activation_row_examples += 1;
                    true
                } else {
                    false
                }
            }
            LpBzLpKernelConstraintKind::PrecedenceActivation => {
                if precedence_row_examples < LP_BZ_KERNEL_REPORT_ROW_KIND_SAMPLE_LIMIT {
                    precedence_row_examples += 1;
                    true
                } else {
                    false
                }
            }
        })
        .map(|row| CompactLpBzLpKernelConstraintRow {
            row_index: row.row_index,
            row_id: row.row_id.clone(),
            kind: row.kind,
            sense: row.sense,
            rhs: row.rhs,
            period_index: row.period_index,
            period_label: row.period_label.clone(),
            resource_id: row.resource_id.clone(),
            unit_id: row.unit_id.clone(),
            predecessor_unit_id: row.predecessor_unit_id.clone(),
            successor_unit_id: row.successor_unit_id.clone(),
            term_count: row.terms.len(),
        })
        .collect::<Vec<_>>();
    let total_term_count = artifact
        .constraints
        .rows
        .iter()
        .map(|row| row.terms.len())
        .sum();
    let max_term_count = artifact
        .constraints
        .rows
        .iter()
        .map(|row| row.terms.len())
        .max()
        .unwrap_or(0);
    let constraint_period_labels = artifact
        .constraints
        .rows
        .iter()
        .filter_map(|row| row.period_label.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let resource_ids = artifact
        .constraints
        .rows
        .iter()
        .filter_map(|row| row.resource_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let max_direct_predecessor_count = artifact
        .access
        .unit_profiles
        .iter()
        .map(|profile| profile.direct_predecessor_count)
        .max()
        .unwrap_or(0);
    let max_transitive_predecessor_count = artifact
        .access
        .unit_profiles
        .iter()
        .map(|profile| profile.transitive_predecessor_count)
        .max()
        .unwrap_or(0);
    let max_closure_unit_count = artifact
        .access
        .unit_profiles
        .iter()
        .map(|profile| profile.closure_unit_count)
        .max()
        .unwrap_or(0);
    let closure_resource_ids = artifact
        .access
        .unit_profiles
        .iter()
        .flat_map(|profile| {
            profile
                .closure_resources
                .iter()
                .map(|resource| resource.resource_id.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let sample_unit_profiles = artifact
        .access
        .unit_profiles
        .iter()
        .take(LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT)
        .map(|profile| CompactLpBzLpKernelAccessUnitProfile {
            unit_id: profile.unit_id.clone(),
            bench: profile.bench,
            shell_index: profile.shell_index,
            direct_predecessor_count: profile.direct_predecessor_count,
            transitive_predecessor_count: profile.transitive_predecessor_count,
            closure_unit_count: profile.closure_unit_count,
            closure_resource_count: profile.closure_resources.len(),
            closure_resources: profile
                .closure_resources
                .iter()
                .map(|resource| CompactLpBzLpKernelAccessClosureResource {
                    resource_id: resource.resource_id.clone(),
                    minimum_total_requirement: resource.minimum_total_requirement,
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    CompactLpBzLpKernelArtifact {
        kernel_label: artifact.kernel_label.clone(),
        period_count: artifact.period_count,
        unit_count: artifact.unit_count,
        destination_count: artifact.destination_count,
        discount_rate: artifact.discount_rate,
        variable_index: CompactLpBzLpKernelVariableIndexArtifact {
            variable_count: artifact.variable_index.variable_count,
            sampled_entry_count,
            omitted_entry_count: artifact
                .variable_index
                .entries
                .len()
                .saturating_sub(sampled_entry_count),
            period_labels,
            destination_ids,
            unit_id_examples,
            sample_entries,
        },
        objective: CompactLpBzLpKernelObjectiveArtifact {
            coefficient_count: artifact.objective.summary.coefficient_count,
            non_zero_coefficient_count: artifact.objective.summary.non_zero_coefficient_count,
            sampled_coefficient_count: sample_coefficients.len(),
            omitted_coefficient_count: artifact
                .objective
                .coefficients
                .len()
                .saturating_sub(sample_coefficients.len()),
            min_coefficient,
            max_coefficient,
            min_discount_factor,
            max_discount_factor,
            sample_coefficients,
        },
        constraints: CompactLpBzLpKernelConstraintArtifact {
            row_count: artifact.constraints.summary.row_count,
            capacity_row_count: artifact.constraints.summary.capacity_row_count,
            activation_row_count: artifact.constraints.summary.activation_row_count,
            precedence_row_count: artifact.constraints.summary.precedence_row_count,
            sampled_row_count: sample_rows.len(),
            omitted_row_count: artifact
                .constraints
                .rows
                .len()
                .saturating_sub(sample_rows.len()),
            total_term_count,
            max_term_count,
            period_labels: constraint_period_labels,
            resource_ids,
            sample_rows,
        },
        access: CompactLpBzLpKernelAccessArtifact {
            unit_profile_count: artifact.access.unit_profile_count,
            sampled_profile_count: sample_unit_profiles.len(),
            omitted_profile_count: artifact
                .access
                .unit_profiles
                .len()
                .saturating_sub(sample_unit_profiles.len()),
            max_direct_predecessor_count,
            max_transitive_predecessor_count,
            max_closure_unit_count,
            closure_resource_ids,
            sample_unit_profiles,
        },
        limitations: artifact.limitations.clone(),
    }
}

fn compact_precedence_comparison(
    report: mine_sdk::PrecedenceGraphComparisonReport,
) -> CompactPrecedenceComparison {
    CompactPrecedenceComparison {
        shared_nodes: report.shared_nodes,
        shared_edges: report.shared_edges,
        reference_only_edge_count: report.reference_only_edges.len(),
        candidate_only_edge_count: report.candidate_only_edges.len(),
        node_jaccard_index: report.node_jaccard_index,
        edge_jaccard_index: report.edge_jaccard_index,
        reference_only_edge_examples: report
            .reference_only_edges
            .into_iter()
            .filter_map(block_edge_tuple)
            .take(10)
            .collect(),
        candidate_only_edge_examples: report
            .candidate_only_edges
            .into_iter()
            .filter_map(block_edge_tuple)
            .take(10)
            .collect(),
    }
}

fn compact_membership_comparison(
    report: mine_sdk::BlockMembershipComparisonReport,
) -> CompactMembershipComparison {
    CompactMembershipComparison {
        shared_blocks: report.shared_blocks,
        reference_only_block_count: report.reference_only_blocks.len(),
        candidate_only_block_count: report.candidate_only_blocks.len(),
        jaccard_index: report.jaccard_index,
        reference_only_block_examples: report.reference_only_blocks.into_iter().take(10).collect(),
        candidate_only_block_examples: report.candidate_only_blocks.into_iter().take(10).collect(),
    }
}

fn block_edge_tuple(edge: mine_sdk::PrecedenceEdge) -> Option<(usize, usize)> {
    match (edge.predecessor(), edge.successor()) {
        (PrecedenceNode::Block(predecessor), PrecedenceNode::Block(successor)) => {
            Some((*predecessor, *successor))
        }
        _ => None,
    }
}

fn relative_or_display(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root).map_or_else(
        |_| path.display().to_string(),
        |relative| relative.display().to_string(),
    )
}

fn marvin_slope_template() -> Result<BlockPrecedenceTemplate, mine_sdk::MineError> {
    BlockPrecedenceTemplate::new(vec![
        // dk=1: patrón cardinal (5 bloques)
        PrecedenceOffset::new(0, 0, 1)?,
        PrecedenceOffset::new(-1, 0, 1)?,
        PrecedenceOffset::new(1, 0, 1)?,
        PrecedenceOffset::new(0, -1, 1)?,
        PrecedenceOffset::new(0, 1, 1)?,
        // dk=3: esquinas diagonales (4 bloques)
        PrecedenceOffset::new(-2, -2, 3)?,
        PrecedenceOffset::new(-2, 2, 3)?,
        PrecedenceOffset::new(2, -2, 3)?,
        PrecedenceOffset::new(2, 2, 3)?,
        // dk=5: arco semicircular (8 bloques)
        PrecedenceOffset::new(-4, -3, 5)?,
        PrecedenceOffset::new(-4, 3, 5)?,
        PrecedenceOffset::new(-3, -4, 5)?,
        PrecedenceOffset::new(-3, 4, 5)?,
        PrecedenceOffset::new(3, -4, 5)?,
        PrecedenceOffset::new(3, 4, 5)?,
        PrecedenceOffset::new(4, -3, 5)?,
        PrecedenceOffset::new(4, 3, 5)?,
    ])
}

fn membership_metrics(
    model: &BlockModel,
    selected_linear_indices: &[usize],
    value_column: &ColumnId,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, f64>, mine_sdk::MineError> {
    const MINE_COST_PER_TON: f64 = 0.9;
    let proc_profit_values = float_column(model, value_column)?;
    let tonnage_values = float_column(model, tonnage_column)?;
    let mut total_proc_profit_x_tonnage = 0.0;
    let mut total_economic_objective = 0.0;
    let mut total_tonnage = 0.0;

    for linear_index in selected_linear_indices {
        let row_index = row_index_for_linear_index(model, *linear_index)?;
        let proc_profit = proc_profit_values[row_index];
        let tonnage = tonnage_values[row_index];
        total_proc_profit_x_tonnage += proc_profit * tonnage;
        total_economic_objective += (proc_profit.max(0.0) - MINE_COST_PER_TON) * tonnage;
        total_tonnage += tonnage;
    }

    Ok(BTreeMap::from([
        (
            "block_count".to_owned(),
            selected_linear_indices.len() as f64,
        ),
        (
            "total_proc_profit_x_tonnage".to_owned(),
            total_proc_profit_x_tonnage,
        ),
        (
            "total_economic_objective".to_owned(),
            total_economic_objective,
        ),
        ("total_tonnage".to_owned(), total_tonnage),
    ]))
}

fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
) -> Result<&'a [f64], mine_sdk::MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(mine_sdk::MineError::schema(format!(
            "column `{column_id}` does not exist in block model storage"
        )));
    };
    let ColumnData::Floats(values) = column_data else {
        return Err(mine_sdk::MineError::schema(format!(
            "column `{column_id}` must be a float column"
        )));
    };
    Ok(values)
}

fn row_index_for_linear_index(
    model: &BlockModel,
    linear_index: usize,
) -> Result<usize, mine_sdk::MineError> {
    for row_index in 0..model.block_count() {
        if model.linear_index_at(row_index)? == linear_index {
            return Ok(row_index);
        }
    }

    Err(mine_sdk::MineError::validation(format!(
        "linear index `{linear_index}` is not materialized in the block model"
    )))
}

struct MineRsEndToEndArtifacts {
    phase_plan: PushbackPlan,
    scheduling_problem: SchedulingProblem,
    summary: MineRsEndToEndSummary,
    report: LongTermScheduleEconomicsReport,
    period_memberships: BTreeMap<String, BTreeSet<usize>>,
}

struct LpBzBenchmarkArtifacts {
    phase_plan: PushbackPlan,
    scheduling_problem: SchedulingProblem,
}

fn marvin_shell_access_rules() -> NestingAccessRules {
    NestingAccessRules::strict_sequential()
}

fn build_mine_rs_end_to_end_artifacts(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    marvin_problem: &MarvinScheduleProblem,
) -> Result<MineRsEndToEndArtifacts, mine_sdk::MineError> {
    let phase_plan = build_mine_rs_end_to_end_phase_plan(model, precedence_graph)?;
    build_mine_rs_end_to_end_artifacts_from_phase_plan(model, marvin_problem, phase_plan)
}

fn build_mine_rs_end_to_end_phase_plan(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let revenue_factors = uniform_revenue_factors(MARVIN_END_TO_END_FACTOR_COUNT)?;
    Ok(build_marvin_phase_plan_from_revenue_factor_shells(
        model,
        precedence_graph,
        &revenue_factors,
        marvin_shell_access_rules(),
        &format!(
            "Marvin end-to-end benchmark uses a bounded {MARVIN_END_TO_END_FACTOR_COUNT}-factor nested-shell × bench phase plan derived from revenue/cost-aware factor scenarios rebuilt from the open benchmark columns."
        ),
    )?
    .phase_plan)
}

fn build_mine_rs_end_to_end_artifacts_from_phase_plan(
    model: &BlockModel,
    marvin_problem: &MarvinScheduleProblem,
    phase_plan: PushbackPlan,
) -> Result<MineRsEndToEndArtifacts, mine_sdk::MineError> {
    let tonnage_column = ColumnId::new("field_4")?;
    let scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(model, &phase_plan, marvin_problem)?;
    let schedule = solve_decomposed_scheduling_problem(
        &scheduling_problem,
        &DecomposedSchedulingConfig::ready_frontier(),
        Metadata::new(),
    )?
    .final_schedule()
    .clone();
    let economic_model = build_marvin_economic_block_model(model)?;
    let report = evaluate_long_term_schedule_economics(
        &schedule,
        &phase_plan,
        &economic_model,
        marvin_problem.discount_rate,
    )?;
    let period_memberships =
        build_candidate_period_memberships(model, &phase_plan, &schedule, &tonnage_column)?;
    let summary = compact_end_to_end_summary(&phase_plan, &schedule, &report);

    Ok(MineRsEndToEndArtifacts {
        phase_plan,
        scheduling_problem,
        summary,
        report,
        period_memberships,
    })
}

fn build_lp_bz_access_progression_artifacts(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    marvin_problem: &MarvinScheduleProblem,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
) -> Result<LpBzBenchmarkArtifacts, mine_sdk::MineError> {
    let phase_plan = split_phase_plan_by_shape_gated_component_fronts_with_local_access(
        model,
        base_phase_plan,
        tonnage_by_linear_index,
        LP_BZ_LOCAL_FRONT_COUNT,
        LP_BZ_LOCAL_RULE_MIN_ASPECT_RATIO,
        LP_BZ_LOCAL_RULE_MIN_DOMINANT_SPAN,
        true,
        Some(LP_BZ_LOCAL_ACCESS_WINDOW_COUNT),
        Some(&MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE.cumulative_tonnage_targets),
        None,
        None,
    )?;
    let scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(model, &phase_plan, marvin_problem)?;
    Ok(LpBzBenchmarkArtifacts {
        phase_plan,
        scheduling_problem,
    })
}

fn build_pushback_bench_localized_cut_artifacts_with_config(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    marvin_problem: &MarvinScheduleProblem,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    config: PushbackBenchLocalizedCutBuildConfig,
) -> Result<PushbackBenchLocalizedCutBuildArtifacts<SchedulingProblem>, mine_sdk::MineError> {
    build_pushback_bench_localized_cut_benchmark_artifacts(
        model,
        base_phase_plan,
        tonnage_by_linear_index,
        config,
        |phase_plan| {
            build_phase_scheduling_problem_from_marvin_problem(model, phase_plan, marvin_problem)
        },
    )
}

fn build_lp_bz_access_progression_band_artifacts(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    marvin_problem: &MarvinScheduleProblem,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    representative_period_by_block: &BTreeMap<usize, f64>,
    period_band_width: usize,
) -> Result<LpBzBandRefinementArtifacts, mine_sdk::MineError> {
    build_lp_bz_access_progression_band_artifacts_with_link_policy(
        model,
        base_phase_plan,
        marvin_problem,
        tonnage_by_linear_index,
        representative_period_by_block,
        period_band_width,
        LpBzBandPredecessorLinkPolicy::PredecessorLastCut,
    )
}

fn build_lp_bz_access_progression_band_artifacts_with_link_policy(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    marvin_problem: &MarvinScheduleProblem,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    representative_period_by_block: &BTreeMap<usize, f64>,
    period_band_width: usize,
    predecessor_cut_link_policy: LpBzBandPredecessorLinkPolicy,
) -> Result<LpBzBandRefinementArtifacts, mine_sdk::MineError> {
    let localized_front_benchmark = build_lp_bz_access_progression_artifacts(
        model,
        base_phase_plan,
        marvin_problem,
        tonnage_by_linear_index,
    )?;
    let band_phase_plan = split_phase_plan_by_representative_period_bands_with_link_policy(
        &localized_front_benchmark.phase_plan,
        representative_period_by_block,
        tonnage_by_linear_index,
        period_band_width,
        predecessor_cut_link_policy,
    )?;
    let phase_refinement_diagnostics = build_lp_bz_period_band_refinement_diagnostics(
        &localized_front_benchmark.phase_plan,
        &band_phase_plan,
        period_band_width,
    );
    let scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &band_phase_plan,
        marvin_problem,
    )?;
    Ok(LpBzBandRefinementArtifacts {
        benchmark: LpBzBenchmarkArtifacts {
            phase_plan: band_phase_plan,
            scheduling_problem,
        },
        phase_refinement_diagnostics,
    })
}

fn build_lp_bz_period_band_refinement_diagnostics(
    localized_front_phase_plan: &PushbackPlan,
    period_band_phase_plan: &PushbackPlan,
    period_band_width: usize,
) -> LpBzPeriodBandRefinementDiagnostics {
    let mut cut_count_by_localized_front = BTreeMap::<String, usize>::new();
    for phase in &period_band_phase_plan.phases {
        let localized_front_phase_id = phase
            .phase_id
            .rsplit_once("::cut-p")
            .map(|(phase_id, _)| phase_id)
            .unwrap_or(&phase.phase_id)
            .to_owned();
        *cut_count_by_localized_front
            .entry(localized_front_phase_id)
            .or_default() += 1;
    }
    let refined_localized_front_examples = localized_front_phase_plan
        .phases
        .iter()
        .filter_map(|phase| {
            (cut_count_by_localized_front
                .get(&phase.phase_id)
                .copied()
                .unwrap_or(0)
                > 1)
            .then(|| phase.phase_id.clone())
        })
        .take(8)
        .collect::<Vec<_>>();
    let localized_front_phase_count = localized_front_phase_plan.phase_count;
    let refined_localized_front_phase_count = cut_count_by_localized_front
        .values()
        .filter(|&&cut_count| cut_count > 1)
        .count();
    let total_period_band_phase_count = period_band_phase_plan.phase_count;
    let max_cut_count_per_localized_front = cut_count_by_localized_front
        .values()
        .copied()
        .max()
        .unwrap_or(0);
    let average_cut_count_per_localized_front = if localized_front_phase_count == 0 {
        0.0
    } else {
        total_period_band_phase_count as f64 / localized_front_phase_count as f64
    };

    LpBzPeriodBandRefinementDiagnostics {
        period_band_width,
        localized_front_phase_count,
        refined_localized_front_phase_count,
        total_period_band_phase_count,
        additional_phase_count: total_period_band_phase_count
            .saturating_sub(localized_front_phase_count),
        max_cut_count_per_localized_front,
        average_cut_count_per_localized_front,
        refined_localized_front_examples,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_focused_pushback_bench_localized_cut_experiment(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    pcpsp_problem: &MarvinScheduleProblem,
    pcpsp_summary: &MarvinScheduleSolutionSummary,
    pcpsp_solution: &MarvinScheduleSolution,
    lp_pcpsp_solution: &MarvinScheduleSolution,
    lp_pcpsp_solution_path: &Path,
    repo_root: &Path,
    tonnage_column: &ColumnId,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    v8_local_front_candidate_artifact: &SchedulingBaselineSummary,
) -> Result<FocusedPushbackBenchLocalizedCutExperiment, MineError> {
    let sweep_builds = PUSHBACK_BENCH_LOCALIZED_CUT_FOCUSED_SWEEP
        .into_iter()
        .map(|config| {
            build_focused_pushback_bench_localized_cut_sweep_build(
                model,
                base_phase_plan,
                pcpsp_problem,
                pcpsp_summary,
                pcpsp_solution,
                lp_pcpsp_solution,
                lp_pcpsp_solution_path,
                repo_root,
                tonnage_column,
                tonnage_by_linear_index,
                config,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let first_builder_index = sweep_builds
        .iter()
        .position(|build| {
            build.config.label == MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL
        })
        .ok_or_else(|| {
            MineError::validation(
                "Pushback bench-localized cut sweep is missing the first builder calibration point."
                    .to_owned(),
            )
        })?;
    let first_builder_point_discounted_objective = sweep_builds[first_builder_index]
        .lp_bz_integer_candidate_artifact
        .candidate_pcpsp_summary
        .discounted_objective;
    let best_sweep_index = sweep_builds
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            left.lp_bz_integer_candidate_artifact
                .candidate_pcpsp_summary
                .discounted_objective
                .partial_cmp(
                    &right
                        .lp_bz_integer_candidate_artifact
                        .candidate_pcpsp_summary
                        .discounted_objective,
                )
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    right
                        .benchmark
                        .benchmark
                        .phase_plan
                        .phase_count
                        .cmp(&left.benchmark.benchmark.phase_plan.phase_count)
                })
        })
        .map(|(index, _)| index)
        .ok_or_else(|| {
            MineError::validation(
                "Pushback bench-localized cut sweep produced no calibration candidates.".to_owned(),
            )
        })?;
    let best_sweep_candidate_label = sweep_builds[best_sweep_index].config.label.to_owned();
    let calibration_sweep = build_pushback_bench_localized_cut_calibration_sweep(
        &sweep_builds,
        first_builder_point_discounted_objective,
        v8_local_front_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective,
        pcpsp_summary.discounted_objective,
        best_sweep_index,
        first_builder_index,
    );
    let best_sweep_build = sweep_builds
        .into_iter()
        .nth(best_sweep_index)
        .ok_or_else(|| {
            MineError::validation(
                "Pushback bench-localized cut sweep lost the best calibration candidate."
                    .to_owned(),
            )
        })?;
    let preferred_nested_shell_family_contract =
        build_marvin_preferred_nested_shell_family_contract_for_phase_plan(
            MARVIN_END_TO_END_FACTOR_COUNT,
            base_phase_plan,
        )?;
    let unit_family_traceability = build_promoted_pushback_bench_localized_cut_unit_family_traceability(
        "cpit-solution",
        pcpsp_summary.unique_block_count,
        &preferred_nested_shell_family_contract.aggregation_strategy,
        Some(&preferred_nested_shell_family_contract),
        LP_BZ_UNIT_GRANULARITY_LABEL,
        PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL,
        best_sweep_build.config.label,
        MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE.label,
    );
    let input_aggregation_traceability_summary =
        format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary(
            &unit_family_traceability,
            best_sweep_build.benchmark.benchmark.phase_plan.phase_count,
            best_sweep_build.lp_bz_inputs.precedence_units.unit_count,
        );

    Ok(FocusedPushbackBenchLocalizedCutExperiment {
        builder_label: PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL.to_owned(),
        calibrated_candidate_label: best_sweep_build.config.label.to_owned(),
        first_builder_point_label: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL
            .to_owned(),
        best_sweep_candidate_label,
        unit_granularity_label: PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL.to_owned(),
        unit_family_traceability,
        input_aggregation_traceability_summary,
        max_front_count: best_sweep_build.config.build_config.max_front_count,
        min_aspect_ratio: best_sweep_build.config.build_config.min_aspect_ratio,
        min_dominant_span: best_sweep_build.config.build_config.min_dominant_span,
        localized_access_mode: localized_access_mode_label(
            best_sweep_build
                .config
                .build_config
                .include_touching_neighbors,
        )
        .to_owned(),
        max_local_predecessor_count: best_sweep_build
            .config
            .build_config
            .max_local_predecessor_count
            .unwrap_or(0),
        first_builder_point_discounted_objective,
        best_sweep_candidate_vs_first_builder_point_objective_delta: best_sweep_build
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective
            - first_builder_point_discounted_objective,
        phase_count_delta_vs_v8_local_front: best_sweep_build
            .benchmark
            .benchmark
            .phase_plan
            .phase_count as isize
            - v8_local_front_candidate_artifact.phase_count as isize,
        candidate_vs_v8_local_front_objective_delta: best_sweep_build
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective
            - v8_local_front_candidate_artifact
                .candidate_pcpsp_summary
                .discounted_objective,
        calibration_sweep,
        lp_bz_inputs: best_sweep_build.lp_bz_inputs,
        lp_bz_bound_artifact: best_sweep_build.lp_bz_bound_artifact,
        lp_bz_integer_candidate_artifact: best_sweep_build.lp_bz_integer_candidate_artifact,
        lp_bz_rounder_v6_local_optimizer_diagnostics: best_sweep_build
            .lp_bz_rounder_v6_local_optimizer_diagnostics,
        lp_bz_gap_metrics: best_sweep_build.lp_bz_gap_metrics,
        phase_refinement_diagnostics: best_sweep_build.benchmark.phase_refinement_diagnostics,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_focused_pushback_bench_localized_cut_sweep_build(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    pcpsp_problem: &MarvinScheduleProblem,
    pcpsp_summary: &MarvinScheduleSolutionSummary,
    pcpsp_solution: &MarvinScheduleSolution,
    lp_pcpsp_solution: &MarvinScheduleSolution,
    lp_pcpsp_solution_path: &Path,
    repo_root: &Path,
    tonnage_column: &ColumnId,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    config: PushbackBenchLocalizedCutSweepConfig,
) -> Result<FocusedPushbackBenchLocalizedCutSweepBuild, MineError> {
    let benchmark = build_pushback_bench_localized_cut_artifacts_with_config(
        model,
        base_phase_plan,
        pcpsp_problem,
        tonnage_by_linear_index,
        config.build_config,
    )?;
    let (round_repair_artifacts, round_repair_schedule) =
        build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
            &benchmark.benchmark.phase_plan,
            &benchmark.benchmark.scheduling_problem,
            lp_pcpsp_solution,
            None,
            Metadata::new(),
        )?;
    let rounder_v6_local_optimizer_diagnostics =
        build_lp_bz_rounder_v6_local_optimizer_diagnostics(&round_repair_artifacts);
    let round_repair_period_memberships = build_candidate_period_memberships(
        model,
        &benchmark.benchmark.phase_plan,
        &round_repair_schedule,
        tonnage_column,
    )?;
    let round_repair_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &round_repair_period_memberships)?;
    let round_repair_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &round_repair_solution)?;
    let integer_candidate_artifact = build_baseline_summary(
        &format!(
            "lp-bz-round-repair-pushback-bench-localized-cut-seeded::{}",
            config.label
        ),
        benchmark.benchmark.phase_plan.phase_count,
        pcpsp_summary,
        pcpsp_solution,
        &round_repair_summary,
        &round_repair_solution,
    );
    let bound_artifacts = compute_lp_bz_bound_artifacts(
        pcpsp_problem,
        lp_pcpsp_solution,
        lp_pcpsp_solution_path,
        repo_root,
        benchmark.benchmark.scheduling_problem.units().len(),
        benchmark
            .benchmark
            .scheduling_problem
            .units()
            .iter()
            .map(|unit| unit.predecessor_unit_ids().len())
            .sum(),
        PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL,
    )?;
    let lp_kernel_artifact =
        build_lp_bz_lp_kernel_artifact(&benchmark.benchmark.scheduling_problem)?;
    let lp_solve_artifact = build_skipped_focused_lp_bz_lp_solve_artifact(&lp_kernel_artifact);
    validate_lp_bz_artifact_coherence(
        &bound_artifacts.lp_bz_inputs,
        &bound_artifacts.lp_bz_bound_artifact,
        &lp_kernel_artifact,
    )?;
    let gap_metrics = build_lp_bz_gap_metrics(
        &bound_artifacts.lp_bz_bound_artifact,
        &lp_solve_artifact,
        &integer_candidate_artifact.candidate_pcpsp_summary,
        pcpsp_summary.discounted_objective,
        integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective,
    );

    Ok(FocusedPushbackBenchLocalizedCutSweepBuild {
        config,
        benchmark,
        lp_bz_inputs: bound_artifacts.lp_bz_inputs,
        lp_bz_bound_artifact: bound_artifacts.lp_bz_bound_artifact,
        lp_bz_integer_candidate_artifact: integer_candidate_artifact,
        lp_bz_rounder_v6_local_optimizer_diagnostics: rounder_v6_local_optimizer_diagnostics,
        lp_bz_gap_metrics: gap_metrics,
    })
}

fn build_pushback_bench_localized_cut_calibration_sweep(
    sweep_builds: &[FocusedPushbackBenchLocalizedCutSweepBuild],
    first_builder_point_discounted_objective: f64,
    v8_local_front_discounted_objective: f64,
    pcpsp_reference_discounted_objective: f64,
    best_sweep_index: usize,
    first_builder_index: usize,
) -> Vec<PushbackBenchLocalizedCutSweepEntry> {
    let mut entries = sweep_builds
        .iter()
        .enumerate()
        .map(|(index, build)| PushbackBenchLocalizedCutSweepEntry {
            candidate_label: build.config.label.to_owned(),
            is_first_builder_point: index == first_builder_index,
            is_best_candidate: index == best_sweep_index,
            max_front_count: build.config.build_config.max_front_count,
            min_aspect_ratio: build.config.build_config.min_aspect_ratio,
            min_dominant_span: build.config.build_config.min_dominant_span,
            localized_access_mode: localized_access_mode_label(
                build.config.build_config.include_touching_neighbors,
            )
            .to_owned(),
            max_local_predecessor_count: build
                .config
                .build_config
                .max_local_predecessor_count
                .unwrap_or(0),
            phase_count: build.benchmark.benchmark.phase_plan.phase_count,
            unit_count: build.lp_bz_inputs.precedence_units.unit_count,
            candidate_pcpsp_discounted_objective: build
                .lp_bz_integer_candidate_artifact
                .candidate_pcpsp_summary
                .discounted_objective,
            candidate_vs_first_builder_point_objective_delta: build
                .lp_bz_integer_candidate_artifact
                .candidate_pcpsp_summary
                .discounted_objective
                - first_builder_point_discounted_objective,
            candidate_vs_v8_local_front_objective_delta: build
                .lp_bz_integer_candidate_artifact
                .candidate_pcpsp_summary
                .discounted_objective
                - v8_local_front_discounted_objective,
            candidate_vs_pcpsp_reference_objective_gap: pcpsp_reference_discounted_objective
                - build
                    .lp_bz_integer_candidate_artifact
                    .candidate_pcpsp_summary
                    .discounted_objective,
            bound_to_candidate_relative_gap: build
                .lp_bz_gap_metrics
                .bound_to_candidate_relative_gap,
            repaired_phase_target_count: build
                .lp_bz_rounder_v6_local_optimizer_diagnostics
                .repaired_phase_target_count,
            repaired_unit_target_count: build
                .lp_bz_rounder_v6_local_optimizer_diagnostics
                .repaired_unit_target_count,
            used_period_count: build
                .lp_bz_integer_candidate_artifact
                .candidate_pcpsp_summary
                .used_period_count,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .candidate_pcpsp_discounted_objective
            .partial_cmp(&left.candidate_pcpsp_discounted_objective)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.phase_count.cmp(&right.phase_count))
            .then_with(|| left.candidate_label.cmp(&right.candidate_label))
    });
    entries
}

#[allow(clippy::too_many_arguments)]
fn build_focused_lp_bz_local_front_band_experiment(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    pcpsp_problem: &MarvinScheduleProblem,
    pcpsp_summary: &MarvinScheduleSolutionSummary,
    pcpsp_solution: &MarvinScheduleSolution,
    lp_pcpsp_solution: &MarvinScheduleSolution,
    lp_pcpsp_solution_path: &Path,
    repo_root: &Path,
    tonnage_column: &ColumnId,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    representative_period_by_block: &BTreeMap<usize, f64>,
    period_band_width: usize,
    predecessor_cut_link_policy: LpBzBandPredecessorLinkPolicy,
) -> Result<FocusedLpBzVariantExperiment, MineError> {
    let band_benchmark =
        if predecessor_cut_link_policy == LpBzBandPredecessorLinkPolicy::PredecessorLastCut {
            build_lp_bz_access_progression_band_artifacts(
                model,
                base_phase_plan,
                pcpsp_problem,
                tonnage_by_linear_index,
                representative_period_by_block,
                period_band_width,
            )?
        } else {
            build_lp_bz_access_progression_band_artifacts_with_link_policy(
                model,
                base_phase_plan,
                pcpsp_problem,
                tonnage_by_linear_index,
                representative_period_by_block,
                period_band_width,
                predecessor_cut_link_policy,
            )?
        };
    let (round_repair_artifacts, round_repair_schedule) =
        build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
            &band_benchmark.benchmark.phase_plan,
            &band_benchmark.benchmark.scheduling_problem,
            lp_pcpsp_solution,
            None,
            Metadata::new(),
        )?;
    let rounder_v6_local_optimizer_diagnostics =
        build_lp_bz_rounder_v6_local_optimizer_diagnostics(&round_repair_artifacts);
    let round_repair_period_memberships = build_candidate_period_memberships(
        model,
        &band_benchmark.benchmark.phase_plan,
        &round_repair_schedule,
        tonnage_column,
    )?;
    let round_repair_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &round_repair_period_memberships)?;
    let round_repair_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &round_repair_solution)?;
    let integer_candidate_artifact = build_baseline_summary(
        &format!(
            "lp-bz-round-repair-local-front-period-band-seeded-w{period_band_width}-{}",
            predecessor_cut_link_policy.label()
        ),
        band_benchmark.benchmark.phase_plan.phase_count,
        pcpsp_summary,
        pcpsp_solution,
        &round_repair_summary,
        &round_repair_solution,
    );
    let bound_artifacts = compute_lp_bz_bound_artifacts(
        pcpsp_problem,
        lp_pcpsp_solution,
        lp_pcpsp_solution_path,
        repo_root,
        band_benchmark.benchmark.scheduling_problem.units().len(),
        band_benchmark
            .benchmark
            .scheduling_problem
            .units()
            .iter()
            .map(|unit| unit.predecessor_unit_ids().len())
            .sum(),
        LP_BZ_V9_UNIT_GRANULARITY_LABEL,
    )?;
    let lp_kernel_artifact =
        build_lp_bz_lp_kernel_artifact(&band_benchmark.benchmark.scheduling_problem)?;
    let lp_solve_artifact = build_skipped_focused_lp_bz_lp_solve_artifact(&lp_kernel_artifact);
    validate_lp_bz_artifact_coherence(
        &bound_artifacts.lp_bz_inputs,
        &bound_artifacts.lp_bz_bound_artifact,
        &lp_kernel_artifact,
    )?;
    let gap_metrics = build_lp_bz_gap_metrics(
        &bound_artifacts.lp_bz_bound_artifact,
        &lp_solve_artifact,
        &integer_candidate_artifact.candidate_pcpsp_summary,
        pcpsp_summary.discounted_objective,
        integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective,
    );

    Ok(FocusedLpBzVariantExperiment {
        unit_granularity_label: LP_BZ_V9_UNIT_GRANULARITY_LABEL.to_owned(),
        predecessor_cut_link_policy: predecessor_cut_link_policy.label().to_owned(),
        front_progression_label: MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE
            .label
            .to_owned(),
        lp_bz_inputs: bound_artifacts.lp_bz_inputs,
        lp_bz_bound_artifact: bound_artifacts.lp_bz_bound_artifact,
        lp_bz_integer_candidate_artifact: integer_candidate_artifact,
        lp_bz_rounder_v6_local_optimizer_diagnostics: rounder_v6_local_optimizer_diagnostics,
        lp_bz_gap_metrics: gap_metrics,
        phase_refinement_diagnostics: band_benchmark.phase_refinement_diagnostics,
    })
}

fn build_lp_bz_local_front_band_width_sweep_entry(
    experiment: &FocusedLpBzVariantExperiment,
) -> LpBzLocalFrontBandWidthSweepEntry {
    LpBzLocalFrontBandWidthSweepEntry {
        period_band_width: experiment.phase_refinement_diagnostics.period_band_width,
        phase_count: experiment.lp_bz_integer_candidate_artifact.phase_count,
        refined_localized_front_phase_count: experiment
            .phase_refinement_diagnostics
            .refined_localized_front_phase_count,
        additional_phase_count: experiment
            .phase_refinement_diagnostics
            .additional_phase_count,
        candidate_pcpsp_discounted_objective: experiment
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective,
        candidate_vs_pcpsp_reference_objective_gap: experiment
            .lp_bz_gap_metrics
            .candidate_vs_pcpsp_reference_objective_gap,
        bound_to_candidate_relative_gap: experiment
            .lp_bz_gap_metrics
            .bound_to_candidate_relative_gap,
        repaired_phase_target_count: experiment
            .lp_bz_rounder_v6_local_optimizer_diagnostics
            .repaired_phase_target_count,
        used_period_count: experiment
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .used_period_count,
    }
}

fn build_lp_bz_local_front_band_link_policy_sweep_entry(
    experiment: &FocusedLpBzVariantExperiment,
) -> LpBzLocalFrontBandLinkPolicySweepEntry {
    LpBzLocalFrontBandLinkPolicySweepEntry {
        predecessor_cut_link_policy: experiment.predecessor_cut_link_policy.clone(),
        period_band_width: experiment.phase_refinement_diagnostics.period_band_width,
        phase_count: experiment.lp_bz_integer_candidate_artifact.phase_count,
        direct_predecessor_edge_count: experiment.lp_bz_inputs.precedence_units.edge_count,
        candidate_pcpsp_discounted_objective: experiment
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .discounted_objective,
        candidate_vs_pcpsp_reference_objective_gap: experiment
            .lp_bz_gap_metrics
            .candidate_vs_pcpsp_reference_objective_gap,
        bound_to_candidate_relative_gap: experiment
            .lp_bz_gap_metrics
            .bound_to_candidate_relative_gap,
        repaired_phase_target_count: experiment
            .lp_bz_rounder_v6_local_optimizer_diagnostics
            .repaired_phase_target_count,
        repaired_unit_target_count: experiment
            .lp_bz_rounder_v6_local_optimizer_diagnostics
            .repaired_unit_target_count,
        used_period_count: experiment
            .lp_bz_integer_candidate_artifact
            .candidate_pcpsp_summary
            .used_period_count,
    }
}

fn build_shell_benchmark_candidate(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    marvin_problem: &MarvinScheduleProblem,
    pcpsp_problem: &MarvinScheduleProblem,
    factor_count: usize,
    nesting_rules: NestingAccessRules,
    limitation_note: &str,
) -> Result<
    (
        usize,
        MineRsEndToEndArtifacts,
        MarvinScheduleSolutionSummary,
    ),
    mine_sdk::MineError,
> {
    let revenue_factors = uniform_revenue_factors(factor_count)?;
    let shell_artifacts = build_marvin_phase_plan_from_revenue_factor_shells(
        model,
        precedence_graph,
        &revenue_factors,
        nesting_rules,
        limitation_note,
    )?;
    let candidate = build_mine_rs_end_to_end_artifacts_from_phase_plan(
        model,
        marvin_problem,
        shell_artifacts.phase_plan,
    )?;
    let candidate_pcpsp_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &candidate.period_memberships)?;
    let candidate_pcpsp_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &candidate_pcpsp_solution)?;

    Ok((
        shell_artifacts.shell_set.unique_shell_count,
        candidate,
        candidate_pcpsp_summary,
    ))
}

fn build_strict_shell_factor_sweep_entry(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    marvin_problem: &MarvinScheduleProblem,
    pcpsp_problem: &MarvinScheduleProblem,
    factor_count: usize,
) -> Result<ShellFactorSweepEntry, mine_sdk::MineError> {
    let (shell_count, candidate, candidate_pcpsp_summary) = build_shell_benchmark_candidate(
        model,
        precedence_graph,
        marvin_problem,
        pcpsp_problem,
        factor_count,
        marvin_shell_access_rules(),
        &format!(
            "Strict shell-access sweep with {factor_count} revenue factors for Marvin calibration."
        ),
    )?;

    Ok(ShellFactorSweepEntry {
        factor_count,
        shell_count,
        phase_count: candidate.phase_plan.phase_count,
        schedule_npv: candidate.report.npv,
        candidate_pcpsp_discounted_objective: candidate_pcpsp_summary.discounted_objective,
        used_period_count: candidate_pcpsp_summary.used_period_count,
    })
}

fn build_shell_access_sweep_entry(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    marvin_problem: &MarvinScheduleProblem,
    pcpsp_problem: &MarvinScheduleProblem,
    factor_count: usize,
    access_policy_label: &str,
    nesting_rules: NestingAccessRules,
) -> Result<ShellAccessSweepEntry, mine_sdk::MineError> {
    let (shell_count, candidate, candidate_pcpsp_summary) = build_shell_benchmark_candidate(
        model,
        precedence_graph,
        marvin_problem,
        pcpsp_problem,
        factor_count,
        nesting_rules,
        &format!(
            "Shell-access sweep `{access_policy_label}` with {factor_count} revenue factors for Marvin calibration."
        ),
    )?;

    Ok(ShellAccessSweepEntry {
        access_policy_label: access_policy_label.to_owned(),
        factor_count,
        shell_count,
        phase_count: candidate.phase_plan.phase_count,
        schedule_npv: candidate.report.npv,
        candidate_pcpsp_discounted_objective: candidate_pcpsp_summary.discounted_objective,
        used_period_count: candidate_pcpsp_summary.used_period_count,
    })
}

fn build_lp_cut_band_width_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    representative_period_by_block: &BTreeMap<usize, f64>,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    period_band_width: usize,
) -> Result<LpCutBandSweepEntry, mine_sdk::MineError> {
    let cut_phase_plan = split_phase_plan_by_representative_period_bands(
        &mine_rs_end_to_end.phase_plan,
        representative_period_by_block,
        tonnage_by_linear_index,
        period_band_width,
    )?;
    let cut_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(model, &cut_phase_plan, pcpsp_problem)?;
    let cut_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&cut_phase_plan, lp_solution)?;
    let cut_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &cut_scheduling_problem,
        &cut_phase_target_periods,
    )?;
    let cut_schedule = build_target_period_seeded_long_term_schedule(
        &cut_scheduling_problem,
        &cut_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let cut_period_memberships = build_candidate_period_memberships(
        model,
        &cut_phase_plan,
        &cut_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let cut_solution = build_candidate_pcpsp_solution(pcpsp_problem, &cut_period_memberships)?;
    let cut_summary = summarize_marvin_schedule_solution(pcpsp_problem, &cut_solution)?;

    Ok(LpCutBandSweepEntry {
        period_band_width,
        phase_count: cut_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: cut_summary.discounted_objective,
        used_period_count: cut_summary.used_period_count,
    })
}

fn build_adaptive_component_front_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    min_component_share: f64,
) -> Result<AdaptiveComponentFrontSweepEntry, mine_sdk::MineError> {
    let adaptive_phase_plan = split_phase_plan_by_adaptive_component_fronts(
        model,
        &mine_rs_end_to_end.phase_plan,
        tonnage_by_linear_index,
        ADAPTIVE_COMPONENT_FRONT_COUNT,
        min_component_share,
    )?;
    let adaptive_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &adaptive_phase_plan,
        pcpsp_problem,
    )?;
    let adaptive_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&adaptive_phase_plan, lp_solution)?;
    let adaptive_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &adaptive_scheduling_problem,
        &adaptive_phase_target_periods,
    )?;
    let adaptive_schedule = build_target_period_seeded_long_term_schedule(
        &adaptive_scheduling_problem,
        &adaptive_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let adaptive_period_memberships = build_candidate_period_memberships(
        model,
        &adaptive_phase_plan,
        &adaptive_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let adaptive_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &adaptive_period_memberships)?;
    let adaptive_summary = summarize_marvin_schedule_solution(pcpsp_problem, &adaptive_solution)?;

    Ok(AdaptiveComponentFrontSweepEntry {
        min_component_share,
        phase_count: adaptive_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: adaptive_summary.discounted_objective,
        used_period_count: adaptive_summary.used_period_count,
    })
}

fn build_shape_gated_front_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
) -> Result<ShapeGatedFrontSweepEntry, mine_sdk::MineError> {
    let shape_gated_phase_plan = split_phase_plan_by_shape_gated_component_fronts(
        model,
        &mine_rs_end_to_end.phase_plan,
        tonnage_by_linear_index,
        SHAPE_GATED_FRONT_COUNT,
        min_aspect_ratio,
        min_dominant_span,
    )?;
    let shape_gated_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_phase_plan, lp_solution)?;
    let shape_gated_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_scheduling_problem,
        &shape_gated_phase_target_periods,
    )?;
    let shape_gated_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_scheduling_problem,
        &shape_gated_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_phase_plan,
        &shape_gated_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_period_memberships)?;
    let shape_gated_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_solution)?;

    Ok(ShapeGatedFrontSweepEntry {
        min_aspect_ratio,
        min_dominant_span,
        phase_count: shape_gated_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_summary.discounted_objective,
        used_period_count: shape_gated_summary.used_period_count,
    })
}

fn build_shape_gated_local_rule_window_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
) -> Result<ShapeGatedFrontSweepEntry, mine_sdk::MineError> {
    let shape_gated_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            min_aspect_ratio,
            min_dominant_span,
            true,
            Some(SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT),
            None,
            None,
            None,
        )?;
    let shape_gated_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_phase_plan, lp_solution)?;
    let shape_gated_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_scheduling_problem,
        &shape_gated_phase_target_periods,
    )?;
    let shape_gated_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_scheduling_problem,
        &shape_gated_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_phase_plan,
        &shape_gated_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_period_memberships)?;
    let shape_gated_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_solution)?;

    Ok(ShapeGatedFrontSweepEntry {
        min_aspect_ratio,
        min_dominant_span,
        phase_count: shape_gated_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_summary.discounted_objective,
        used_period_count: shape_gated_summary.used_period_count,
    })
}

fn build_shape_gated_front_count_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
) -> Result<ShapeGatedFrontCountSweepEntry, mine_sdk::MineError> {
    let shape_gated_phase_plan = split_phase_plan_by_shape_gated_component_fronts(
        model,
        &mine_rs_end_to_end.phase_plan,
        tonnage_by_linear_index,
        max_front_count,
        SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
        SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
    )?;
    let shape_gated_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_phase_plan, lp_solution)?;
    let shape_gated_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_scheduling_problem,
        &shape_gated_phase_target_periods,
    )?;
    let shape_gated_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_scheduling_problem,
        &shape_gated_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_phase_plan,
        &shape_gated_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_period_memberships)?;
    let shape_gated_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_solution)?;

    Ok(ShapeGatedFrontCountSweepEntry {
        max_front_count,
        phase_count: shape_gated_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_summary.discounted_objective,
        used_period_count: shape_gated_summary.used_period_count,
    })
}

fn build_shape_gated_local_rule_front_count_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
) -> Result<ShapeGatedFrontCountSweepEntry, mine_sdk::MineError> {
    let shape_gated_local_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            max_front_count,
            SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_ASPECT_RATIO,
            SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_DOMINANT_SPAN,
            true,
            Some(SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT),
            None,
            None,
            None,
        )?;
    let shape_gated_local_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_local_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_local_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_local_phase_plan, lp_solution)?;
    let shape_gated_local_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_phase_target_periods,
    )?;
    let shape_gated_local_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_local_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_local_phase_plan,
        &shape_gated_local_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_local_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_local_period_memberships)?;
    let shape_gated_local_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_local_solution)?;

    Ok(ShapeGatedFrontCountSweepEntry {
        max_front_count,
        phase_count: shape_gated_local_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_local_summary.discounted_objective,
        used_period_count: shape_gated_local_summary.used_period_count,
    })
}

fn build_shape_gated_local_front_count_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
) -> Result<ShapeGatedFrontCountSweepEntry, mine_sdk::MineError> {
    let shape_gated_local_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            max_front_count,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            Some(SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW),
            None,
            None,
            None,
        )?;
    let shape_gated_local_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_local_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_local_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_local_phase_plan, lp_solution)?;
    let shape_gated_local_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_phase_target_periods,
    )?;
    let shape_gated_local_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_local_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_local_phase_plan,
        &shape_gated_local_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_local_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_local_period_memberships)?;
    let shape_gated_local_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_local_solution)?;

    Ok(ShapeGatedFrontCountSweepEntry {
        max_front_count,
        phase_count: shape_gated_local_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_local_summary.discounted_objective,
        used_period_count: shape_gated_local_summary.used_period_count,
    })
}

fn build_shape_gated_local_overlap_front_count_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
) -> Result<ShapeGatedFrontCountSweepEntry, mine_sdk::MineError> {
    let shape_gated_local_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            max_front_count,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            false,
            Some(SHAPE_GATED_LOCAL_FRONT_COUNT_WINDOW),
            None,
            None,
            None,
        )?;
    let shape_gated_local_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_local_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_local_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_local_phase_plan, lp_solution)?;
    let shape_gated_local_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_phase_target_periods,
    )?;
    let shape_gated_local_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_local_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_local_phase_plan,
        &shape_gated_local_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_local_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_local_period_memberships)?;
    let shape_gated_local_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_local_solution)?;

    Ok(ShapeGatedFrontCountSweepEntry {
        max_front_count,
        phase_count: shape_gated_local_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_local_summary.discounted_objective,
        used_period_count: shape_gated_local_summary.used_period_count,
    })
}

fn build_shape_gated_local_access_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    include_touching_neighbors: bool,
) -> Result<ShapeGatedLocalAccessSweepEntry, mine_sdk::MineError> {
    let shape_gated_local_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            include_touching_neighbors,
            None,
            None,
            None,
            None,
        )?;
    let shape_gated_local_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_local_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_local_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_local_phase_plan, lp_solution)?;
    let shape_gated_local_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_phase_target_periods,
    )?;
    let shape_gated_local_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_local_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_local_phase_plan,
        &shape_gated_local_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_local_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_local_period_memberships)?;
    let shape_gated_local_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_local_solution)?;

    Ok(ShapeGatedLocalAccessSweepEntry {
        access_mode_label: localized_access_mode_label(include_touching_neighbors).to_owned(),
        phase_count: shape_gated_local_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_local_summary.discounted_objective,
        used_period_count: shape_gated_local_summary.used_period_count,
    })
}

fn build_shape_gated_local_access_window_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    include_touching_neighbors: bool,
) -> Result<ShapeGatedLocalAccessSweepEntry, mine_sdk::MineError> {
    let shape_gated_local_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            include_touching_neighbors,
            Some(SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT),
            None,
            None,
            None,
        )?;
    let shape_gated_local_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_local_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_local_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_local_phase_plan, lp_solution)?;
    let shape_gated_local_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_phase_target_periods,
    )?;
    let shape_gated_local_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_local_scheduling_problem,
        &shape_gated_local_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_local_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_local_phase_plan,
        &shape_gated_local_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_local_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_local_period_memberships)?;
    let shape_gated_local_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_local_solution)?;

    Ok(ShapeGatedLocalAccessSweepEntry {
        access_mode_label: localized_access_mode_label(include_touching_neighbors).to_owned(),
        phase_count: shape_gated_local_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_local_summary.discounted_objective,
        used_period_count: shape_gated_local_summary.used_period_count,
    })
}

fn known_front_progression_profile_contract(
    cumulative_tonnage_targets: &[f64],
) -> Option<FrontProgressionProfileContract> {
    SHAPE_GATED_FRONT_PROGRESSION_SWEEP
        .into_iter()
        .find(|profile| {
            profile.cumulative_tonnage_targets.len() == cumulative_tonnage_targets.len()
                && profile
                    .cumulative_tonnage_targets
                    .iter()
                    .zip(cumulative_tonnage_targets.iter())
                    .all(|(expected, candidate)| (*expected - *candidate).abs() <= 1.0e-9)
        })
}

fn front_progression_contract_label(
    front_progression_cumulative_targets: Option<&[f64]>,
) -> String {
    match front_progression_cumulative_targets {
        Some(cumulative_tonnage_targets) => {
            match known_front_progression_profile_contract(cumulative_tonnage_targets) {
                Some(profile) => format!("`{}`", profile.label),
                None => format!("custom cumulative targets {:?}", cumulative_tonnage_targets),
            }
        }
        None => "uniform tonnage-balanced".to_owned(),
    }
}

fn build_shape_gated_front_progression_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    progression_label: &str,
    cumulative_tonnage_targets: &[f64],
) -> Result<ShapeGatedFrontProgressionSweepEntry, mine_sdk::MineError> {
    let shape_gated_progression_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            None,
            Some(cumulative_tonnage_targets),
            None,
            None,
        )?;
    let shape_gated_progression_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            model,
            &shape_gated_progression_phase_plan,
            pcpsp_problem,
        )?;
    let shape_gated_progression_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &shape_gated_progression_phase_plan,
        lp_solution,
    )?;
    let shape_gated_progression_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &shape_gated_progression_scheduling_problem,
            &shape_gated_progression_phase_target_periods,
        )?;
    let shape_gated_progression_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_progression_scheduling_problem,
        &shape_gated_progression_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_progression_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_progression_phase_plan,
        &shape_gated_progression_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_progression_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_progression_period_memberships)?;
    let shape_gated_progression_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_progression_solution)?;

    Ok(ShapeGatedFrontProgressionSweepEntry {
        progression_label: progression_label.to_owned(),
        phase_count: shape_gated_progression_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_progression_summary.discounted_objective,
        used_period_count: shape_gated_progression_summary.used_period_count,
    })
}

fn build_shape_gated_front_progression_window_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    progression_label: &str,
    cumulative_tonnage_targets: &[f64],
) -> Result<ShapeGatedFrontProgressionSweepEntry, mine_sdk::MineError> {
    let shape_gated_progression_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            Some(SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT),
            Some(cumulative_tonnage_targets),
            None,
            None,
        )?;
    let shape_gated_progression_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            model,
            &shape_gated_progression_phase_plan,
            pcpsp_problem,
        )?;
    let shape_gated_progression_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &shape_gated_progression_phase_plan,
        lp_solution,
    )?;
    let shape_gated_progression_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &shape_gated_progression_scheduling_problem,
            &shape_gated_progression_phase_target_periods,
        )?;
    let shape_gated_progression_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_progression_scheduling_problem,
        &shape_gated_progression_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_progression_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_progression_phase_plan,
        &shape_gated_progression_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_progression_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_progression_period_memberships)?;
    let shape_gated_progression_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_progression_solution)?;

    Ok(ShapeGatedFrontProgressionSweepEntry {
        progression_label: progression_label.to_owned(),
        phase_count: shape_gated_progression_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_progression_summary.discounted_objective,
        used_period_count: shape_gated_progression_summary.used_period_count,
    })
}

fn build_shape_gated_conditional_progression_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    min_progression_aspect_ratio: f64,
) -> Result<ShapeGatedConditionalProgressionSweepEntry, mine_sdk::MineError> {
    let shape_gated_conditional_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            None,
            Some(&SHAPE_GATED_CONDITIONAL_PROGRESSIVE_PROFILE),
            Some(min_progression_aspect_ratio),
            None,
        )?;
    let shape_gated_conditional_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            model,
            &shape_gated_conditional_phase_plan,
            pcpsp_problem,
        )?;
    let shape_gated_conditional_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &shape_gated_conditional_phase_plan,
        lp_solution,
    )?;
    let shape_gated_conditional_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &shape_gated_conditional_scheduling_problem,
            &shape_gated_conditional_phase_target_periods,
        )?;
    let shape_gated_conditional_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_conditional_scheduling_problem,
        &shape_gated_conditional_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_conditional_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_conditional_phase_plan,
        &shape_gated_conditional_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_conditional_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_conditional_period_memberships)?;
    let shape_gated_conditional_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_conditional_solution)?;

    Ok(ShapeGatedConditionalProgressionSweepEntry {
        min_progression_aspect_ratio,
        phase_count: shape_gated_conditional_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_conditional_summary.discounted_objective,
        used_period_count: shape_gated_conditional_summary.used_period_count,
    })
}

fn build_shape_gated_conditional_window_progression_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    min_progression_aspect_ratio: f64,
) -> Result<ShapeGatedConditionalProgressionSweepEntry, mine_sdk::MineError> {
    let shape_gated_conditional_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            Some(SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT),
            Some(&SHAPE_GATED_CONDITIONAL_PROGRESSIVE_PROFILE),
            Some(min_progression_aspect_ratio),
            None,
        )?;
    let shape_gated_conditional_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            model,
            &shape_gated_conditional_phase_plan,
            pcpsp_problem,
        )?;
    let shape_gated_conditional_phase_target_periods = build_phase_target_periods_from_lp_solution(
        &shape_gated_conditional_phase_plan,
        lp_solution,
    )?;
    let shape_gated_conditional_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &shape_gated_conditional_scheduling_problem,
            &shape_gated_conditional_phase_target_periods,
        )?;
    let shape_gated_conditional_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_conditional_scheduling_problem,
        &shape_gated_conditional_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_conditional_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_conditional_phase_plan,
        &shape_gated_conditional_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_conditional_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_conditional_period_memberships)?;
    let shape_gated_conditional_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_conditional_solution)?;

    Ok(ShapeGatedConditionalProgressionSweepEntry {
        min_progression_aspect_ratio,
        phase_count: shape_gated_conditional_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_conditional_summary.discounted_objective,
        used_period_count: shape_gated_conditional_summary.used_period_count,
    })
}

fn build_shape_gated_local_window_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_local_predecessor_count: usize,
) -> Result<ShapeGatedLocalWindowSweepEntry, mine_sdk::MineError> {
    let shape_gated_window_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_FRONT_MIN_ASPECT_RATIO,
            SHAPE_GATED_FRONT_MIN_DOMINANT_SPAN,
            true,
            Some(max_local_predecessor_count),
            None,
            None,
            None,
        )?;
    let shape_gated_window_scheduling_problem = build_phase_scheduling_problem_from_marvin_problem(
        model,
        &shape_gated_window_phase_plan,
        pcpsp_problem,
    )?;
    let shape_gated_window_phase_target_periods =
        build_phase_target_periods_from_lp_solution(&shape_gated_window_phase_plan, lp_solution)?;
    let shape_gated_window_target_period_by_unit = build_unit_target_periods_from_phase_targets(
        &shape_gated_window_scheduling_problem,
        &shape_gated_window_phase_target_periods,
    )?;
    let shape_gated_window_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_window_scheduling_problem,
        &shape_gated_window_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_window_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_window_phase_plan,
        &shape_gated_window_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_window_solution =
        build_candidate_pcpsp_solution(pcpsp_problem, &shape_gated_window_period_memberships)?;
    let shape_gated_window_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_window_solution)?;

    Ok(ShapeGatedLocalWindowSweepEntry {
        max_local_predecessor_count,
        phase_count: shape_gated_window_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_window_summary.discounted_objective,
        used_period_count: shape_gated_window_summary.used_period_count,
    })
}

fn build_shape_gated_dynamic_local_window_sweep_entry(
    model: &BlockModel,
    mine_rs_end_to_end: &MineRsEndToEndArtifacts,
    pcpsp_problem: &MarvinScheduleProblem,
    lp_solution: &marvin_support::MarvinScheduleSolution,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    window_rule_label: &str,
    min_dynamic_window_aspect_ratio: f64,
    promoted_local_predecessor_count: usize,
) -> Result<ShapeGatedDynamicLocalWindowSweepEntry, mine_sdk::MineError> {
    let shape_gated_dynamic_window_phase_plan =
        split_phase_plan_by_shape_gated_component_fronts_with_local_access(
            model,
            &mine_rs_end_to_end.phase_plan,
            tonnage_by_linear_index,
            SHAPE_GATED_FRONT_COUNT,
            SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_ASPECT_RATIO,
            SHAPE_GATED_LOCAL_RULE_WINDOW_MIN_DOMINANT_SPAN,
            true,
            Some(SHAPE_GATED_LOCAL_ACCESS_WINDOW_COUNT),
            None,
            None,
            Some((
                min_dynamic_window_aspect_ratio,
                promoted_local_predecessor_count,
            )),
        )?;
    let shape_gated_dynamic_window_scheduling_problem =
        build_phase_scheduling_problem_from_marvin_problem(
            model,
            &shape_gated_dynamic_window_phase_plan,
            pcpsp_problem,
        )?;
    let shape_gated_dynamic_window_phase_target_periods =
        build_phase_target_periods_from_lp_solution(
            &shape_gated_dynamic_window_phase_plan,
            lp_solution,
        )?;
    let shape_gated_dynamic_window_target_period_by_unit =
        build_unit_target_periods_from_phase_targets(
            &shape_gated_dynamic_window_scheduling_problem,
            &shape_gated_dynamic_window_phase_target_periods,
        )?;
    let shape_gated_dynamic_window_schedule = build_target_period_seeded_long_term_schedule(
        &shape_gated_dynamic_window_scheduling_problem,
        &shape_gated_dynamic_window_target_period_by_unit,
        None,
        Metadata::new(),
    )?;
    let shape_gated_dynamic_window_period_memberships = build_candidate_period_memberships(
        model,
        &shape_gated_dynamic_window_phase_plan,
        &shape_gated_dynamic_window_schedule,
        &ColumnId::new("field_4")?,
    )?;
    let shape_gated_dynamic_window_solution = build_candidate_pcpsp_solution(
        pcpsp_problem,
        &shape_gated_dynamic_window_period_memberships,
    )?;
    let shape_gated_dynamic_window_summary =
        summarize_marvin_schedule_solution(pcpsp_problem, &shape_gated_dynamic_window_solution)?;

    Ok(ShapeGatedDynamicLocalWindowSweepEntry {
        window_rule_label: window_rule_label.to_owned(),
        min_dynamic_window_aspect_ratio,
        promoted_local_predecessor_count,
        phase_count: shape_gated_dynamic_window_phase_plan.phase_count,
        candidate_pcpsp_discounted_objective: shape_gated_dynamic_window_summary
            .discounted_objective,
        used_period_count: shape_gated_dynamic_window_summary.used_period_count,
    })
}

fn build_phase_scheduling_problem_from_marvin_problem(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    marvin_problem: &MarvinScheduleProblem,
) -> Result<SchedulingProblem, mine_sdk::MineError> {
    let tonnage_by_linear_index =
        build_linear_index_float_lookup(model, &ColumnId::new("field_4")?)?;
    let phase_by_linear_index = phase_plan
        .phases
        .iter()
        .flat_map(|phase| {
            phase
                .block_indices
                .iter()
                .copied()
                .map(move |linear_index| (linear_index, phase.phase_id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut objective_by_phase_destination = BTreeMap::<(String, usize), f64>::new();
    let mut requirements_by_phase_resource_destination =
        BTreeMap::<(String, usize, usize), f64>::new();
    let mut objective_by_block_destination = BTreeMap::<(usize, usize), f64>::new();
    let mut requirements_by_block_resource_destination =
        BTreeMap::<(usize, usize, usize), f64>::new();

    for term in &marvin_problem.objective_terms {
        objective_by_block_destination.insert(
            (term.linear_index, term.destination_index),
            term.objective_value,
        );
        let Some(phase_id) = phase_by_linear_index.get(&term.linear_index) else {
            continue;
        };
        *objective_by_phase_destination
            .entry((phase_id.clone(), term.destination_index))
            .or_insert(0.0) += term.objective_value;
    }

    for coefficient in &marvin_problem.resource_coefficients {
        if coefficient.coefficient < -1.0e-9 {
            return Err(mine_sdk::MineError::validation(format!(
                "Marvin resource coefficient for block `{}` resource `{}` destination `{}` must be non-negative to build an aggregated SchedulingProblem",
                coefficient.linear_index, coefficient.resource_index, coefficient.destination_index
            )));
        }
        if coefficient.coefficient <= 1.0e-9 {
            continue;
        }
        requirements_by_block_resource_destination.insert(
            (
                coefficient.linear_index,
                coefficient.destination_index,
                coefficient.resource_index,
            ),
            coefficient.coefficient,
        );
        let Some(phase_id) = phase_by_linear_index.get(&coefficient.linear_index) else {
            continue;
        };
        *requirements_by_phase_resource_destination
            .entry((
                phase_id.clone(),
                coefficient.resource_index,
                coefficient.destination_index,
            ))
            .or_insert(0.0) += coefficient.coefficient;
    }

    let periods = build_periods_from_marvin_problem(marvin_problem)?;
    let destination_ids = (0..marvin_problem.destination_count)
        .map(marvin_destination_id)
        .collect::<Result<Vec<_>, _>>()?;
    let mut max_limit_by_resource = BTreeMap::<usize, f64>::new();
    for limit in &marvin_problem.resource_constraint_limits {
        if !matches!(limit.relation, 'L' | 'E') || limit.limit <= 1.0e-9 {
            continue;
        }
        max_limit_by_resource
            .entry(limit.resource_index)
            .and_modify(|current| *current = (*current).min(limit.limit))
            .or_insert(limit.limit);
    }

    let mut units = Vec::new();
    let mut objective_terms = Vec::new();
    let mut resource_requirements = Vec::new();
    let mut last_chunk_id_by_phase = BTreeMap::<String, SchedulingUnitId>::new();

    for phase in &phase_plan.phases {
        let total_tonnage = phase
            .total_tonnage
            .ok_or_else(|| mine_sdk::MineError::Planning {
                message: format!(
                    "phase `{}` requires total_tonnage to build a Marvin scheduling problem",
                    phase.phase_id
                ),
            })?;
        let candidate_destination_indices = (0..marvin_problem.destination_count)
            .filter(|destination_index| {
                objective_by_phase_destination
                    .contains_key(&(phase.phase_id.clone(), *destination_index))
                    || requirements_by_phase_resource_destination.keys().any(
                        |(phase_id, _, requirement_destination_index)| {
                            phase_id == &phase.phase_id
                                && requirement_destination_index == destination_index
                        },
                    )
            })
            .collect::<Vec<_>>();
        let candidate_destinations = candidate_destination_indices
            .iter()
            .copied()
            .map(marvin_destination_id)
            .collect::<Result<Vec<_>, _>>()?;
        let mut chunk_count = 1usize;

        if let Some(max_limit) = max_limit_by_resource.get(&0) {
            chunk_count = chunk_count.max((total_tonnage / max_limit).ceil() as usize);
        }
        for ((phase_id, resource_index, _), amount) in &requirements_by_phase_resource_destination {
            if phase_id != &phase.phase_id {
                continue;
            }
            if let Some(max_limit) = max_limit_by_resource.get(resource_index) {
                chunk_count = chunk_count.max((amount / max_limit).ceil() as usize);
            }
        }
        chunk_count = chunk_count.max(1);

        let chunk_blocks = partition_block_indices_by_tonnage(
            &phase.block_indices,
            &tonnage_by_linear_index,
            chunk_count,
        )
        .into_iter()
        .filter(|chunk_block_indices| !chunk_block_indices.is_empty())
        .collect::<Vec<_>>();
        let actual_chunk_count = chunk_blocks.len().max(1);
        let mut previous_chunk_id = None::<SchedulingUnitId>;

        for (chunk_index, chunk_block_indices) in chunk_blocks.iter().enumerate() {
            let unit_name = if actual_chunk_count == 1 {
                phase.phase_id.clone()
            } else {
                format!("{}::part-{:02}", phase.phase_id, chunk_index + 1)
            };
            let unit_id = SchedulingUnitId::new(unit_name)?;
            let predecessor_unit_ids = if let Some(previous_chunk_id) = &previous_chunk_id {
                vec![previous_chunk_id.clone()]
            } else {
                phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|phase_id| {
                        last_chunk_id_by_phase.get(phase_id).cloned().ok_or_else(|| {
                            mine_sdk::MineError::Planning {
                                message: format!(
                                    "phase `{}` references predecessor `{phase_id}` before it was chunked",
                                    phase.phase_id
                                ),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            let unit_metadata = Metadata::from_entries(vec![(
                "phase_id".to_owned(),
                MetadataValue::Text(phase.phase_id.clone()),
            )])?;

            units.push(SchedulingUnit::new(
                unit_id.clone(),
                chunk_block_indices
                    .iter()
                    .filter_map(|linear_index| tonnage_by_linear_index.get(linear_index).copied())
                    .sum::<f64>(),
                chunk_block_indices.len(),
                predecessor_unit_ids,
                candidate_destinations.clone(),
                Vec::new(),
                chunk_block_indices.clone(),
                phase.bench,
                phase.shell_index,
                unit_metadata,
            )?);

            for destination_index in &candidate_destination_indices {
                let phase_objective = objective_by_phase_destination
                    .get(&(phase.phase_id.clone(), *destination_index));
                let chunk_objective = chunk_block_indices
                    .iter()
                    .filter_map(|linear_index| {
                        objective_by_block_destination
                            .get(&(*linear_index, *destination_index))
                            .copied()
                    })
                    .sum::<f64>();
                if phase_objective.is_some() && chunk_objective.abs() > 1.0e-9 {
                    objective_terms.push(SchedulingObjectiveTerm::new(
                        unit_id.clone(),
                        Some(marvin_destination_id(*destination_index)?),
                        chunk_objective,
                    )?);
                }
            }

            for ((phase_id, resource_index, destination_index), amount) in
                &requirements_by_phase_resource_destination
            {
                if phase_id != &phase.phase_id || *amount <= 1.0e-9 {
                    continue;
                }
                let chunk_amount = chunk_block_indices
                    .iter()
                    .filter_map(|linear_index| {
                        requirements_by_block_resource_destination
                            .get(&(*linear_index, *destination_index, *resource_index))
                            .copied()
                    })
                    .sum::<f64>();
                if chunk_amount <= 1.0e-9 {
                    continue;
                }
                resource_requirements.push(SchedulingResourceRequirement::new(
                    unit_id.clone(),
                    marvin_resource_id(*resource_index)?,
                    Some(marvin_destination_id(*destination_index)?),
                    chunk_amount,
                )?);
            }

            previous_chunk_id = Some(unit_id.clone());
            last_chunk_id_by_phase.insert(phase.phase_id.clone(), unit_id);
        }
    }

    SchedulingProblem::new(
        ScenarioId::new("marvin-candidate")?,
        ModelId::new("marvin")?,
        periods,
        units,
        objective_terms,
        resource_requirements,
        destination_ids,
        Vec::new(),
        marvin_problem.discount_rate,
        Metadata::new(),
        vec![
            "Revenue/cost-aware nested-shell × bench phases are chunked with block-preserving tonnage quantiles before routing, so the scheduling objective is chunk-level rather than a direct block-by-block reproduction of MineLib PCPSP.".to_owned(),
        ],
    )
}

fn build_periods_from_marvin_problem(
    marvin_problem: &MarvinScheduleProblem,
) -> Result<Vec<SchedulingPeriod>, mine_sdk::MineError> {
    let mut bounds_by_period =
        vec![BTreeMap::<usize, (Option<f64>, Option<f64>)>::new(); marvin_problem.period_count];

    for limit in &marvin_problem.resource_constraint_limits {
        let period_bounds = bounds_by_period
            .get_mut(limit.period_index)
            .ok_or_else(|| {
                mine_sdk::MineError::validation(format!(
                    "Marvin resource limit references period `{}` outside declared range 0..{}",
                    limit.period_index, marvin_problem.period_count
                ))
            })?;
        let bound = period_bounds
            .entry(limit.resource_index)
            .or_insert((None, None));
        match limit.relation {
            'L' => {
                bound.1 = Some(
                    bound
                        .1
                        .map_or(limit.limit, |current| current.min(limit.limit)),
                );
            }
            'G' => {
                bound.0 = Some(
                    bound
                        .0
                        .map_or(limit.limit, |current| current.max(limit.limit)),
                );
            }
            'E' => {
                bound.0 = Some(
                    bound
                        .0
                        .map_or(limit.limit, |current| current.max(limit.limit)),
                );
                bound.1 = Some(
                    bound
                        .1
                        .map_or(limit.limit, |current| current.min(limit.limit)),
                );
            }
            relation => {
                return Err(mine_sdk::MineError::validation(format!(
                    "Marvin resource limit uses unsupported relation `{relation}`"
                )));
            }
        }
    }

    bounds_by_period
        .into_iter()
        .enumerate()
        .map(|(period_index, resource_bounds)| {
            SchedulingPeriod::new(
                format!("P{:02}", period_index + 1),
                resource_bounds
                    .into_iter()
                    .map(|(resource_index, (min_total, max_total))| {
                        SchedulingResourceBound::new(
                            marvin_resource_id(resource_index)?,
                            min_total,
                            max_total,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                Vec::new(),
                Vec::new(),
            )
        })
        .collect()
}

fn marvin_resource_id(resource_index: usize) -> Result<SchedulingResourceId, mine_sdk::MineError> {
    match resource_index {
        0 => SchedulingResourceId::new("mine_tonnage"),
        1 => SchedulingResourceId::new("plant_tonnage"),
        _ => SchedulingResourceId::new(format!("resource-{resource_index:02}")),
    }
}

fn marvin_destination_id(
    destination_index: usize,
) -> Result<ScheduleDestinationId, mine_sdk::MineError> {
    ScheduleDestinationId::new(format!("dest-{destination_index:02}"))
}

fn partition_block_indices_by_tonnage(
    block_indices: &[usize],
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    parts: usize,
) -> Vec<Vec<usize>> {
    if parts <= 1 || block_indices.is_empty() {
        return vec![block_indices.to_vec()];
    }

    let total_tonnage = block_indices
        .iter()
        .filter_map(|linear_index| tonnage_by_linear_index.get(linear_index).copied())
        .sum::<f64>();
    if total_tonnage <= 1.0e-9 {
        let mut result = Vec::with_capacity(parts);
        let mut assigned = 0usize;
        for part_index in 0..parts {
            let next_assigned = if part_index + 1 == parts {
                block_indices.len()
            } else {
                (((part_index + 1) as f64 / parts as f64) * block_indices.len() as f64).round()
                    as usize
            };
            result.push(block_indices[assigned..next_assigned.min(block_indices.len())].to_vec());
            assigned = next_assigned.min(block_indices.len());
        }
        return result;
    }

    let mut result = Vec::with_capacity(parts);
    let mut chunk = Vec::<usize>::new();
    let mut cumulative_tonnage = 0.0_f64;
    let mut previous_threshold = 0.0_f64;

    for &linear_index in block_indices {
        chunk.push(linear_index);
        cumulative_tonnage += tonnage_by_linear_index
            .get(&linear_index)
            .copied()
            .unwrap_or(0.0);
        let next_threshold = total_tonnage * ((result.len() + 1) as f64 / parts as f64);
        if result.len() + 1 < parts
            && !chunk.is_empty()
            && cumulative_tonnage >= next_threshold
            && cumulative_tonnage > previous_threshold + 1.0e-9
        {
            result.push(std::mem::take(&mut chunk));
            previous_threshold = cumulative_tonnage;
        }
    }
    result.push(chunk);

    while result.len() < parts {
        result.push(Vec::new());
    }
    result
}

fn build_marvin_economic_block_model(
    model: &BlockModel,
) -> Result<EconomicBlockModel, mine_sdk::MineError> {
    let tonnage_column = ColumnId::new("field_4")?;
    let au_column = ColumnId::new("field_5")?;
    let cu_column = ColumnId::new("field_6")?;
    let tonne_unit = MeasurementUnit::new("t")?;
    let process_destination = DestinationAssumptions::new(
        DestinationId::new("process")?,
        DestinationKind::Mill,
        0.9,
        4.0,
        vec![
            DestinationRecovery::new(au_column.clone(), 0.6)?,
            DestinationRecovery::new(cu_column.clone(), 0.88)?,
        ],
        vec![
            DestinationPayability::new(au_column.clone(), (12.0 - 0.2) / 12.0)?,
            DestinationPayability::new(cu_column.clone(), (20.0 - 7.2) / 20.0)?,
        ],
        DestinationCapacity::new(None, tonne_unit.clone())?,
        BTreeMap::from([
            (au_column.as_str().to_owned(), 12.0),
            (cu_column.as_str().to_owned(), 20.0),
        ]),
    )?;
    let waste_destination = DestinationAssumptions::new(
        DestinationId::new("waste")?,
        DestinationKind::Waste,
        0.9,
        0.0,
        Vec::new(),
        Vec::new(),
        DestinationCapacity::new(None, tonne_unit)?,
        BTreeMap::new(),
    )?;

    EconomicBlockModel::build(
        model.clone(),
        EconomicBlockModelConfig {
            tonnage_column,
            grade_columns: vec![au_column, cu_column],
            destinations: DestinationAssumptionSet::new(vec![
                process_destination,
                waste_destination,
            ])?,
        },
    )
}

fn build_reference_period_memberships(
    solution: &marvin_support::MarvinScheduleSolution,
) -> BTreeMap<String, BTreeSet<usize>> {
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();
    for assignment in &solution.assignments {
        if assignment.fraction <= 0.0 {
            continue;
        }
        memberships
            .entry(format!("P{:02}", assignment.period_index + 1))
            .or_default()
            .insert(assignment.linear_index);
    }
    memberships
}

fn build_phase_target_periods_from_lp_solution(
    phase_plan: &PushbackPlan,
    lp_solution: &marvin_support::MarvinScheduleSolution,
) -> Result<BTreeMap<String, usize>, mine_sdk::MineError> {
    let representative_period_by_block = representative_period_by_block(lp_solution);
    let mut period_by_phase = BTreeMap::<String, usize>::new();

    for phase in &phase_plan.phases {
        let mut weighted_period_sum = 0.0_f64;
        let mut weighted_block_count = 0.0_f64;
        for linear_index in &phase.block_indices {
            let Some(period_index) = representative_period_by_block.get(linear_index) else {
                continue;
            };
            weighted_period_sum += *period_index;
            weighted_block_count += 1.0;
        }
        let representative_phase_period = if weighted_block_count <= 1.0e-9 {
            0
        } else {
            representative_period_index(weighted_period_sum / weighted_block_count)
        };
        period_by_phase.insert(phase.phase_id.clone(), representative_phase_period);
    }

    for phase in &phase_plan.phases {
        let predecessor_period = phase
            .predecessor_phase_ids
            .iter()
            .filter_map(|phase_id| period_by_phase.get(phase_id).copied())
            .max()
            .unwrap_or(0);
        let entry = period_by_phase.entry(phase.phase_id.clone()).or_insert(0);
        *entry = (*entry).max(predecessor_period);
    }

    Ok(period_by_phase)
}

fn build_phase_period_memberships_from_phase_targets(
    phase_plan: &PushbackPlan,
    period_by_phase: &BTreeMap<String, usize>,
) -> Result<BTreeMap<String, BTreeSet<usize>>, mine_sdk::MineError> {
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();
    for phase in &phase_plan.phases {
        let period_index = period_by_phase
            .get(&phase.phase_id)
            .copied()
            .ok_or_else(|| mine_sdk::MineError::Planning {
                message: format!(
                    "LP-seeded phase period mapping is missing phase `{}`",
                    phase.phase_id
                ),
            })?;
        let period_label = format!("P{:02}", period_index + 1);
        memberships
            .entry(period_label)
            .or_default()
            .extend(phase.block_indices.iter().copied());
    }
    Ok(memberships)
}

fn build_unit_target_periods_from_phase_targets(
    scheduling_problem: &SchedulingProblem,
    period_by_phase: &BTreeMap<String, usize>,
) -> Result<BTreeMap<SchedulingUnitId, usize>, mine_sdk::MineError> {
    scheduling_problem
        .units()
        .iter()
        .map(|unit| {
            let phase_id = unit
                .unit_id()
                .as_str()
                .split("::part-")
                .next()
                .unwrap_or_else(|| unit.unit_id().as_str());
            let period_index = period_by_phase.get(phase_id).copied().ok_or_else(|| {
                mine_sdk::MineError::Planning {
                    message: format!(
                        "LP target-period mapping is missing chunk source phase `{phase_id}` for unit `{}`",
                        unit.unit_id()
                    ),
                }
            })?;
            Ok((unit.unit_id().clone(), period_index))
        })
        .collect()
}

fn split_phase_plan_by_representative_period_bands(
    phase_plan: &PushbackPlan,
    representative_period_by_block: &BTreeMap<usize, f64>,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    period_band_width: usize,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    split_phase_plan_by_representative_period_bands_with_link_policy(
        phase_plan,
        representative_period_by_block,
        tonnage_by_linear_index,
        period_band_width,
        LpBzBandPredecessorLinkPolicy::PredecessorLastCut,
    )
}

fn split_phase_plan_by_representative_period_bands_with_link_policy(
    phase_plan: &PushbackPlan,
    representative_period_by_block: &BTreeMap<usize, f64>,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    period_band_width: usize,
    predecessor_cut_link_policy: LpBzBandPredecessorLinkPolicy,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    if period_band_width == 0 {
        return Err(mine_sdk::MineError::invalid_parameter(
            "period_band_width",
            "must be greater than zero",
        ));
    }

    let mut cut_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut cut_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let mut blocks_by_period_band = BTreeMap::<usize, Vec<usize>>::new();
        for &linear_index in &phase.block_indices {
            let representative_period = representative_period_by_block
                .get(&linear_index)
                .copied()
                .unwrap_or(0.0);
            let period_band =
                representative_period_index(representative_period) / period_band_width;
            blocks_by_period_band
                .entry(period_band)
                .or_default()
                .push(linear_index);
        }

        let mut phase_cut_ids = Vec::<String>::new();
        for (period_band, mut block_indices) in blocks_by_period_band {
            block_indices.sort_unstable();
            let cut_phase_id = format!("{}::cut-p{:02}", phase.phase_id, period_band + 1);
            let predecessor_phase_ids = if let Some(previous_cut_phase_id) = phase_cut_ids.last() {
                vec![previous_cut_phase_id.clone()]
            } else {
                phase
                    .predecessor_phase_ids
                    .iter()
                    .try_fold(
                        Vec::new(),
                        |mut predecessor_cut_ids, predecessor_phase_id| -> Result<
                        Vec<String>,
                        mine_sdk::MineError,
                        > {
                        let predecessor_phase_cut_ids = cut_phase_ids_by_phase
                            .get(predecessor_phase_id)
                            .ok_or_else(|| mine_sdk::MineError::Planning {
                                message: format!(
                                    "LP-guided cut split is missing predecessor cuts for phase `{predecessor_phase_id}`"
                                ),
                            })?;
                        match predecessor_cut_link_policy {
                            LpBzBandPredecessorLinkPolicy::PredecessorLastCut => {
                                predecessor_cut_ids.push(
                                    predecessor_phase_cut_ids
                                        .last()
                                        .cloned()
                                        .ok_or_else(|| mine_sdk::MineError::Planning {
                                            message: format!(
                                                "LP-guided cut split is missing predecessor cuts for phase `{predecessor_phase_id}`"
                                            ),
                                        })?,
                                );
                            }
                            LpBzBandPredecessorLinkPolicy::PredecessorFirstCut => {
                                predecessor_cut_ids.push(
                                    predecessor_phase_cut_ids
                                        .first()
                                        .cloned()
                                        .ok_or_else(|| mine_sdk::MineError::Planning {
                                            message: format!(
                                                "LP-guided cut split is missing predecessor cuts for phase `{predecessor_phase_id}`"
                                            ),
                                        })?,
                                );
                            }
                            LpBzBandPredecessorLinkPolicy::AllPredecessorCuts => {
                                predecessor_cut_ids
                                    .extend(predecessor_phase_cut_ids.iter().cloned());
                            }
                        }
                        Ok(predecessor_cut_ids)
                        },
                    )?
            };

            phase_cut_ids.push(cut_phase_id.clone());
            cut_phases.push(PhaseDesign {
                phase_id: cut_phase_id,
                pushback_index: phase.pushback_index,
                shell_index: phase.shell_index,
                revenue_factor: phase.revenue_factor,
                bench: phase.bench,
                block_count: block_indices.len(),
                total_tonnage: Some(
                    block_indices
                        .iter()
                        .filter_map(|linear_index| {
                            tonnage_by_linear_index.get(linear_index).copied()
                        })
                        .sum::<f64>(),
                ),
                block_indices,
                predecessor_phase_ids,
            });
        }
        cut_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_cut_ids);
    }

    Ok(PushbackPlan {
        phase_count: cut_phases.len(),
        total_block_count: cut_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            cut_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: cut_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(
                "LP-guided cuts split each shell×bench phase by representative LP period bands; this remains a benchmark-side proxy rather than a calibrated mining-cut generator.".to_owned(),
            ))
            .collect(),
    })
}

fn split_phase_plan_by_representative_period_quantiles(
    phase_plan: &PushbackPlan,
    representative_period_by_block: &BTreeMap<usize, f64>,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut cut_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut cut_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let mut ordered_block_indices = phase.block_indices.clone();
        ordered_block_indices.sort_by_key(|linear_index| {
            (
                representative_period_index(
                    representative_period_by_block
                        .get(linear_index)
                        .copied()
                        .unwrap_or(0.0),
                ),
                *linear_index,
            )
        });
        let distinct_period_count = ordered_block_indices
            .iter()
            .map(|linear_index| {
                representative_period_index(
                    representative_period_by_block
                        .get(linear_index)
                        .copied()
                        .unwrap_or(0.0),
                )
            })
            .collect::<BTreeSet<_>>()
            .len()
            .max(1);
        let quantile_cuts = partition_block_indices_by_tonnage(
            &ordered_block_indices,
            tonnage_by_linear_index,
            distinct_period_count.min(ordered_block_indices.len().max(1)),
        );

        let mut phase_cut_ids = Vec::<String>::new();
        for (cut_index, mut block_indices) in quantile_cuts.into_iter().enumerate() {
            if block_indices.is_empty() {
                continue;
            }
            block_indices.sort_unstable();
            let cut_phase_id = format!("{}::qcut-{:02}", phase.phase_id, cut_index + 1);
            let predecessor_phase_ids = if let Some(previous_cut_phase_id) = phase_cut_ids.last() {
                vec![previous_cut_phase_id.clone()]
            } else {
                phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|predecessor_phase_id| {
                        cut_phase_ids_by_phase
                            .get(predecessor_phase_id)
                            .and_then(|cut_ids| cut_ids.last())
                            .cloned()
                            .ok_or_else(|| mine_sdk::MineError::Planning {
                                message: format!(
                                    "LP-guided quantile cut split is missing predecessor cuts for phase `{predecessor_phase_id}`"
                                ),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };

            phase_cut_ids.push(cut_phase_id.clone());
            cut_phases.push(PhaseDesign {
                phase_id: cut_phase_id,
                pushback_index: phase.pushback_index,
                shell_index: phase.shell_index,
                revenue_factor: phase.revenue_factor,
                bench: phase.bench,
                block_count: block_indices.len(),
                total_tonnage: Some(
                    block_indices
                        .iter()
                        .filter_map(|linear_index| {
                            tonnage_by_linear_index.get(linear_index).copied()
                        })
                        .sum::<f64>(),
                ),
                block_indices,
                predecessor_phase_ids,
            });
        }
        cut_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_cut_ids);
    }

    Ok(PushbackPlan {
        phase_count: cut_phases.len(),
        total_block_count: cut_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            cut_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: cut_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(
                "LP-guided quantile cuts sort each shell×bench phase by representative LP period and then partition it into tonnage-balanced cuts; this remains a benchmark-side proxy rather than a calibrated mining-cut generator.".to_owned(),
            ))
            .collect(),
    })
}

fn split_block_indices_by_planar_connected_components(
    model: &BlockModel,
    block_indices: &[usize],
) -> Result<Vec<Vec<usize>>, mine_sdk::MineError> {
    if block_indices.is_empty() {
        return Ok(Vec::new());
    }

    let mut linear_index_by_ijk = BTreeMap::<(usize, usize, usize), usize>::new();
    for &linear_index in block_indices {
        let grid_index = linear_to_ijk(model.grid(), linear_index)?;
        linear_index_by_ijk.insert(
            (grid_index.i(), grid_index.j(), grid_index.k()),
            linear_index,
        );
    }

    let mut remaining = block_indices.iter().copied().collect::<BTreeSet<_>>();
    let mut components = Vec::<Vec<usize>>::new();
    while let Some(&start_linear_index) = remaining.iter().next() {
        remaining.remove(&start_linear_index);
        let mut stack = vec![start_linear_index];
        let mut component = vec![start_linear_index];

        while let Some(current_linear_index) = stack.pop() {
            let grid_index = linear_to_ijk(model.grid(), current_linear_index)?;
            for (di, dj) in [(-1_isize, 0_isize), (1, 0), (0, -1), (0, 1)] {
                let next_i = grid_index.i() as isize + di;
                let next_j = grid_index.j() as isize + dj;
                if next_i < 0 || next_j < 0 {
                    continue;
                }
                let Some(&neighbor_linear_index) =
                    linear_index_by_ijk.get(&(next_i as usize, next_j as usize, grid_index.k()))
                else {
                    continue;
                };
                if remaining.remove(&neighbor_linear_index) {
                    stack.push(neighbor_linear_index);
                    component.push(neighbor_linear_index);
                }
            }
        }

        component.sort_unstable();
        components.push(component);
    }

    components.sort_by_key(|component| component[0]);
    Ok(components)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlanarComponentBounds {
    min_i: usize,
    max_i: usize,
    min_j: usize,
    max_j: usize,
}

impl PlanarComponentBounds {
    fn from_block_indices(
        model: &BlockModel,
        block_indices: &[usize],
    ) -> Result<Self, mine_sdk::MineError> {
        let mut min_i = usize::MAX;
        let mut max_i = 0usize;
        let mut min_j = usize::MAX;
        let mut max_j = 0usize;

        for &linear_index in block_indices {
            let grid_index = linear_to_ijk(model.grid(), linear_index)?;
            min_i = min_i.min(grid_index.i());
            max_i = max_i.max(grid_index.i());
            min_j = min_j.min(grid_index.j());
            max_j = max_j.max(grid_index.j());
        }

        Ok(Self {
            min_i,
            max_i,
            min_j,
            max_j,
        })
    }

    fn touches_or_overlaps(&self, other: &Self) -> bool {
        self.max_i.saturating_add(1) >= other.min_i
            && other.max_i.saturating_add(1) >= self.min_i
            && self.max_j.saturating_add(1) >= other.min_j
            && other.max_j.saturating_add(1) >= self.min_j
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.max_i >= other.min_i
            && other.max_i >= self.min_i
            && self.max_j >= other.min_j
            && other.max_j >= self.min_j
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlanarComponentDescriptor {
    phase_id: String,
    bounds: PlanarComponentBounds,
}

fn select_localized_planar_predecessors(
    current_bounds: &PlanarComponentBounds,
    predecessor_components: &[PlanarComponentDescriptor],
    include_touching_neighbors: bool,
    max_local_predecessor_count: Option<usize>,
) -> Vec<String> {
    let mut localized = predecessor_components
        .iter()
        .filter(|descriptor| {
            if include_touching_neighbors {
                descriptor.bounds.touches_or_overlaps(current_bounds)
            } else {
                descriptor.bounds.overlaps(current_bounds)
            }
        })
        .map(|descriptor| {
            (
                descriptor.phase_id.clone(),
                planar_bounds_center_distance_key(current_bounds, &descriptor.bounds),
            )
        })
        .collect::<Vec<_>>();
    if let Some(max_count) = max_local_predecessor_count {
        if max_count > 0 && localized.len() > max_count {
            localized
                .sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
            localized.truncate(max_count);
        }
    }
    if localized.is_empty() {
        predecessor_components
            .iter()
            .map(|descriptor| descriptor.phase_id.clone())
            .collect()
    } else {
        localized
            .into_iter()
            .map(|(phase_id, _)| phase_id)
            .collect()
    }
}

fn planar_bounds_center_distance_key(
    current_bounds: &PlanarComponentBounds,
    candidate_bounds: &PlanarComponentBounds,
) -> u128 {
    let current_center_i = current_bounds.min_i as i128 + current_bounds.max_i as i128;
    let current_center_j = current_bounds.min_j as i128 + current_bounds.max_j as i128;
    let candidate_center_i = candidate_bounds.min_i as i128 + candidate_bounds.max_i as i128;
    let candidate_center_j = candidate_bounds.min_j as i128 + candidate_bounds.max_j as i128;
    let delta_i = current_center_i - candidate_center_i;
    let delta_j = current_center_j - candidate_center_j;
    (delta_i.pow(2) + delta_j.pow(2)) as u128
}

fn localized_access_mode_label(include_touching_neighbors: bool) -> &'static str {
    if include_touching_neighbors {
        "overlap-plus-adjacency"
    } else {
        "overlap-only"
    }
}

fn split_phase_plan_by_planar_connected_components(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut component_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut component_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let predecessor_phase_ids = phase
            .predecessor_phase_ids
            .iter()
            .map(|predecessor_phase_id| {
                component_phase_ids_by_phase
                    .get(predecessor_phase_id)
                    .cloned()
                    .ok_or_else(|| mine_sdk::MineError::Planning {
                        message: format!(
                            "geometric component split is missing predecessor components for phase `{predecessor_phase_id}`"
                        ),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut phase_component_ids = Vec::<String>::new();

        for (component_index, mut block_indices) in components.into_iter().enumerate() {
            if block_indices.is_empty() {
                continue;
            }
            block_indices.sort_unstable();
            let component_phase_id = format!("{}::gcut-{:02}", phase.phase_id, component_index + 1);
            phase_component_ids.push(component_phase_id.clone());
            component_phases.push(PhaseDesign {
                phase_id: component_phase_id,
                pushback_index: phase.pushback_index,
                shell_index: phase.shell_index,
                revenue_factor: phase.revenue_factor,
                bench: phase.bench,
                block_count: block_indices.len(),
                total_tonnage: Some(
                    block_indices
                        .iter()
                        .filter_map(|linear_index| {
                            tonnage_by_linear_index.get(linear_index).copied()
                        })
                        .sum::<f64>(),
                ),
                block_indices,
                predecessor_phase_ids: predecessor_phase_ids.clone(),
            });
        }

        if phase_component_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "geometric component split produced no components for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        component_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_component_ids);
    }

    Ok(PushbackPlan {
        phase_count: component_phases.len(),
        total_block_count: component_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            component_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: component_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(
                "Geometric component cuts split each shell×bench phase into planar connected components; this is closer to bench-phase geometry than the LP-period proxies, but it remains a benchmark-side approximation.".to_owned(),
            ))
            .collect(),
    })
}

fn split_phase_plan_by_planar_connected_components_with_local_predecessors(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut component_descriptors_by_phase =
        BTreeMap::<String, Vec<PlanarComponentDescriptor>>::new();
    let mut component_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let mut phase_component_ids = Vec::<String>::new();
        let mut phase_component_descriptors = Vec::<PlanarComponentDescriptor>::new();

        for (component_index, mut block_indices) in components.into_iter().enumerate() {
            if block_indices.is_empty() {
                continue;
            }
            block_indices.sort_unstable();
            let bounds = PlanarComponentBounds::from_block_indices(model, &block_indices)?;
            let predecessor_phase_ids = phase
                .predecessor_phase_ids
                .iter()
                .map(|predecessor_phase_id| {
                    component_descriptors_by_phase
                        .get(predecessor_phase_id)
                        .map(|descriptors| {
                            select_localized_planar_predecessors(&bounds, descriptors, true, None)
                        })
                        .ok_or_else(|| mine_sdk::MineError::Planning {
                            message: format!(
                                "localized geometric component split is missing predecessor components for phase `{predecessor_phase_id}`"
                            ),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let component_phase_id =
                format!("{}::glocal-{:02}", phase.phase_id, component_index + 1);
            phase_component_ids.push(component_phase_id.clone());
            phase_component_descriptors.push(PlanarComponentDescriptor {
                phase_id: component_phase_id.clone(),
                bounds,
            });
            component_phases.push(PhaseDesign {
                phase_id: component_phase_id,
                pushback_index: phase.pushback_index,
                shell_index: phase.shell_index,
                revenue_factor: phase.revenue_factor,
                bench: phase.bench,
                block_count: block_indices.len(),
                total_tonnage: Some(
                    block_indices
                        .iter()
                        .filter_map(|linear_index| {
                            tonnage_by_linear_index.get(linear_index).copied()
                        })
                        .sum::<f64>(),
                ),
                block_indices,
                predecessor_phase_ids,
            });
        }

        if phase_component_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "localized geometric component split produced no components for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        component_descriptors_by_phase.insert(phase.phase_id.clone(), phase_component_descriptors);
    }

    Ok(PushbackPlan {
        phase_count: component_phases.len(),
        total_block_count: component_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            component_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: component_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(
                "Localized geometric component cuts split each shell×bench phase into planar connected components and only link predecessor components that overlap/touch in plant view, falling back to all predecessors when no local match exists.".to_owned(),
            ))
            .collect(),
    })
}

fn split_component_by_dominant_axis_stripes(
    model: &BlockModel,
    block_indices: &[usize],
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_stripe_count: usize,
) -> Result<Vec<Vec<usize>>, mine_sdk::MineError> {
    if block_indices.is_empty() {
        return Ok(Vec::new());
    }

    let bounds = PlanarComponentBounds::from_block_indices(model, block_indices)?;
    let split_by_i =
        bounds.max_i.saturating_sub(bounds.min_i) >= bounds.max_j.saturating_sub(bounds.min_j);
    let mut ordered_block_indices = block_indices
        .iter()
        .map(|&linear_index| {
            let grid_index = linear_to_ijk(model.grid(), linear_index)?;
            Ok((
                if split_by_i {
                    grid_index.i()
                } else {
                    grid_index.j()
                },
                if split_by_i {
                    grid_index.j()
                } else {
                    grid_index.i()
                },
                linear_index,
            ))
        })
        .collect::<Result<Vec<_>, mine_sdk::MineError>>()?;
    ordered_block_indices.sort_unstable();
    let ordered_block_indices = ordered_block_indices
        .into_iter()
        .map(|(_, _, linear_index)| linear_index)
        .collect::<Vec<_>>();
    let distinct_axis_coordinates = ordered_block_indices
        .iter()
        .map(|&linear_index| {
            let grid_index = linear_to_ijk(model.grid(), linear_index)?;
            Ok(if split_by_i {
                grid_index.i()
            } else {
                grid_index.j()
            })
        })
        .collect::<Result<BTreeSet<_>, mine_sdk::MineError>>()?;
    let stripe_count = max_stripe_count
        .min(distinct_axis_coordinates.len().max(1))
        .min(ordered_block_indices.len().max(1));

    Ok(partition_block_indices_by_tonnage(
        &ordered_block_indices,
        tonnage_by_linear_index,
        stripe_count,
    ))
}

fn split_component_by_dominant_axis_stripes_with_cumulative_targets(
    model: &BlockModel,
    block_indices: &[usize],
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_stripe_count: usize,
    cumulative_tonnage_targets: &[f64],
) -> Result<Vec<Vec<usize>>, mine_sdk::MineError> {
    if block_indices.is_empty() {
        return Ok(Vec::new());
    }

    let bounds = PlanarComponentBounds::from_block_indices(model, block_indices)?;
    let split_by_i =
        bounds.max_i.saturating_sub(bounds.min_i) >= bounds.max_j.saturating_sub(bounds.min_j);
    let mut ordered_block_indices = block_indices
        .iter()
        .map(|&linear_index| {
            let grid_index = linear_to_ijk(model.grid(), linear_index)?;
            Ok((
                if split_by_i {
                    grid_index.i()
                } else {
                    grid_index.j()
                },
                if split_by_i {
                    grid_index.j()
                } else {
                    grid_index.i()
                },
                linear_index,
            ))
        })
        .collect::<Result<Vec<_>, mine_sdk::MineError>>()?;
    ordered_block_indices.sort_unstable();
    let ordered_block_indices = ordered_block_indices
        .into_iter()
        .map(|(_, _, linear_index)| linear_index)
        .collect::<Vec<_>>();
    let distinct_axis_coordinates = ordered_block_indices
        .iter()
        .map(|&linear_index| {
            let grid_index = linear_to_ijk(model.grid(), linear_index)?;
            Ok(if split_by_i {
                grid_index.i()
            } else {
                grid_index.j()
            })
        })
        .collect::<Result<BTreeSet<_>, mine_sdk::MineError>>()?;
    let stripe_count = max_stripe_count
        .min(distinct_axis_coordinates.len().max(1))
        .min(ordered_block_indices.len().max(1));
    if stripe_count <= 1 {
        return Ok(vec![ordered_block_indices]);
    }

    if cumulative_tonnage_targets.len() < stripe_count {
        return Err(mine_sdk::MineError::Planning {
            message: format!(
                "custom front progression expected {stripe_count} cumulative targets but got {}",
                cumulative_tonnage_targets.len()
            ),
        });
    }
    let adjusted_targets = if cumulative_tonnage_targets.len() == stripe_count {
        cumulative_tonnage_targets.to_vec()
    } else {
        let mut targets = cumulative_tonnage_targets[..(stripe_count - 1)].to_vec();
        targets.push(1.0);
        targets
    };

    partition_block_indices_by_cumulative_tonnage_targets(
        &ordered_block_indices,
        tonnage_by_linear_index,
        &adjusted_targets,
    )
}

fn partition_block_indices_by_cumulative_tonnage_targets(
    block_indices: &[usize],
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    cumulative_tonnage_targets: &[f64],
) -> Result<Vec<Vec<usize>>, mine_sdk::MineError> {
    if block_indices.is_empty() {
        return Ok(Vec::new());
    }
    if cumulative_tonnage_targets.is_empty() {
        return Err(mine_sdk::MineError::Planning {
            message: "custom front progression requires at least one cumulative target".to_owned(),
        });
    }
    if cumulative_tonnage_targets
        .iter()
        .any(|target| !target.is_finite() || *target <= 0.0 || *target > 1.0 + 1.0e-9)
    {
        return Err(mine_sdk::MineError::Planning {
            message: "custom front progression targets must be finite values in (0, 1]".to_owned(),
        });
    }
    if cumulative_tonnage_targets
        .windows(2)
        .any(|window| window[1] <= window[0] + 1.0e-9)
    {
        return Err(mine_sdk::MineError::Planning {
            message: "custom front progression targets must be strictly increasing".to_owned(),
        });
    }
    if (cumulative_tonnage_targets[cumulative_tonnage_targets.len() - 1] - 1.0).abs() > 1.0e-6 {
        return Err(mine_sdk::MineError::Planning {
            message: "custom front progression targets must end at 1.0".to_owned(),
        });
    }

    let total_tonnage = block_indices
        .iter()
        .filter_map(|linear_index| tonnage_by_linear_index.get(linear_index).copied())
        .sum::<f64>();
    let parts = cumulative_tonnage_targets.len();

    if total_tonnage <= 1.0e-9 {
        let mut result = Vec::with_capacity(parts);
        let mut assigned = 0usize;
        for cumulative_target in cumulative_tonnage_targets {
            let next_assigned = (*cumulative_target * block_indices.len() as f64).round() as usize;
            result.push(block_indices[assigned..next_assigned.min(block_indices.len())].to_vec());
            assigned = next_assigned.min(block_indices.len());
        }
        return Ok(result);
    }

    let mut result = Vec::with_capacity(parts);
    let mut chunk = Vec::<usize>::new();
    let mut cumulative_tonnage = 0.0_f64;
    let mut target_index = 0usize;

    for &linear_index in block_indices {
        chunk.push(linear_index);
        cumulative_tonnage += tonnage_by_linear_index
            .get(&linear_index)
            .copied()
            .unwrap_or(0.0);
        while target_index + 1 < parts
            && !chunk.is_empty()
            && cumulative_tonnage >= total_tonnage * cumulative_tonnage_targets[target_index]
        {
            result.push(std::mem::take(&mut chunk));
            target_index += 1;
        }
    }
    result.push(chunk);
    while result.len() < parts {
        result.push(Vec::new());
    }
    Ok(result)
}

fn split_phase_plan_by_planar_component_stripes(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_stripe_count: usize,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut stripe_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut stripe_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let predecessor_phase_ids = phase
            .predecessor_phase_ids
            .iter()
            .map(|predecessor_phase_id| {
                stripe_phase_ids_by_phase
                    .get(predecessor_phase_id)
                    .cloned()
                    .ok_or_else(|| mine_sdk::MineError::Planning {
                        message: format!(
                            "geometric component stripe split is missing predecessor stripes for phase `{predecessor_phase_id}`"
                        ),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut phase_stripe_ids = Vec::<String>::new();

        for (component_index, component_block_indices) in components.into_iter().enumerate() {
            let stripes = split_component_by_dominant_axis_stripes(
                model,
                &component_block_indices,
                tonnage_by_linear_index,
                max_stripe_count,
            )?;
            let mut previous_stripe_phase_id = None::<String>;

            for (stripe_index, mut block_indices) in stripes.into_iter().enumerate() {
                if block_indices.is_empty() {
                    continue;
                }
                block_indices.sort_unstable();
                let stripe_phase_id = format!(
                    "{}::gstripe-c{:02}s{:02}",
                    phase.phase_id,
                    component_index + 1,
                    stripe_index + 1
                );
                let stripe_predecessor_phase_ids =
                    if let Some(previous_stripe_phase_id) = &previous_stripe_phase_id {
                        vec![previous_stripe_phase_id.clone()]
                    } else {
                        predecessor_phase_ids.clone()
                    };

                phase_stripe_ids.push(stripe_phase_id.clone());
                stripe_phases.push(PhaseDesign {
                    phase_id: stripe_phase_id.clone(),
                    pushback_index: phase.pushback_index,
                    shell_index: phase.shell_index,
                    revenue_factor: phase.revenue_factor,
                    bench: phase.bench,
                    block_count: block_indices.len(),
                    total_tonnage: Some(
                        block_indices
                            .iter()
                            .filter_map(|linear_index| {
                                tonnage_by_linear_index.get(linear_index).copied()
                            })
                            .sum::<f64>(),
                    ),
                    block_indices,
                    predecessor_phase_ids: stripe_predecessor_phase_ids,
                });
                previous_stripe_phase_id = Some(stripe_phase_id);
            }
        }

        if phase_stripe_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "geometric component stripe split produced no stripes for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        stripe_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_stripe_ids);
    }

    Ok(PushbackPlan {
        phase_count: stripe_phases.len(),
        total_block_count: stripe_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            stripe_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: stripe_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "Geometric component stripes split each shell×bench phase into planar connected components and then into up to {max_stripe_count} tonnage-balanced stripes along each component's dominant planar axis."
            )))
            .collect(),
    })
}

fn split_phase_plan_by_directional_front_bands(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    band_count: usize,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut front_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut front_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let front_bands = split_component_by_dominant_axis_stripes(
            model,
            &phase.block_indices,
            tonnage_by_linear_index,
            band_count,
        )?;
        let mut phase_front_ids = Vec::<String>::new();

        for (front_index, mut block_indices) in front_bands.into_iter().enumerate() {
            if block_indices.is_empty() {
                continue;
            }
            block_indices.sort_unstable();
            let front_phase_id = format!("{}::gfront-{:02}", phase.phase_id, front_index + 1);
            let predecessor_phase_ids = if let Some(previous_front_phase_id) =
                phase_front_ids.last()
            {
                vec![previous_front_phase_id.clone()]
            } else {
                phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|predecessor_phase_id| {
                        front_phase_ids_by_phase
                            .get(predecessor_phase_id)
                            .and_then(|front_ids| front_ids.last())
                            .cloned()
                            .ok_or_else(|| mine_sdk::MineError::Planning {
                                message: format!(
                                    "directional front split is missing predecessor bands for phase `{predecessor_phase_id}`"
                                ),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };

            phase_front_ids.push(front_phase_id.clone());
            front_phases.push(PhaseDesign {
                phase_id: front_phase_id,
                pushback_index: phase.pushback_index,
                shell_index: phase.shell_index,
                revenue_factor: phase.revenue_factor,
                bench: phase.bench,
                block_count: block_indices.len(),
                total_tonnage: Some(
                    block_indices
                        .iter()
                        .filter_map(|linear_index| {
                            tonnage_by_linear_index.get(linear_index).copied()
                        })
                        .sum::<f64>(),
                ),
                block_indices,
                predecessor_phase_ids,
            });
        }

        if phase_front_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "directional front split produced no bands for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        front_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_front_ids);
    }

    Ok(PushbackPlan {
        phase_count: front_phases.len(),
        total_block_count: front_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            front_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: front_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "Directional front bands split each shell×bench phase into up to {band_count} tonnage-balanced bands along the phase's dominant planar axis, chaining those bands inside the phase."
            )))
            .collect(),
    })
}

fn split_phase_plan_by_directional_front_bands_with_local_access(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    band_count: usize,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut front_descriptors_by_phase = BTreeMap::<String, Vec<PlanarComponentDescriptor>>::new();
    let mut front_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let front_bands = split_component_by_dominant_axis_stripes(
            model,
            &phase.block_indices,
            tonnage_by_linear_index,
            band_count,
        )?;
        let mut phase_front_ids = Vec::<String>::new();
        let mut phase_front_descriptors = Vec::<PlanarComponentDescriptor>::new();

        for (front_index, mut block_indices) in front_bands.into_iter().enumerate() {
            if block_indices.is_empty() {
                continue;
            }
            block_indices.sort_unstable();
            let bounds = PlanarComponentBounds::from_block_indices(model, &block_indices)?;
            let front_phase_id = format!("{}::glfront-{:02}", phase.phase_id, front_index + 1);
            let mut predecessor_phase_ids = phase
                .predecessor_phase_ids
                .iter()
                .map(|predecessor_phase_id| {
                    front_descriptors_by_phase
                        .get(predecessor_phase_id)
                        .map(|descriptors| {
                            select_localized_planar_predecessors(&bounds, descriptors, true, None)
                        })
                        .ok_or_else(|| mine_sdk::MineError::Planning {
                            message: format!(
                                "localized directional front split is missing predecessor bands for phase `{predecessor_phase_id}`"
                            ),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            if let Some(previous_front_phase_id) = phase_front_ids.last() {
                predecessor_phase_ids.push(previous_front_phase_id.clone());
            }
            predecessor_phase_ids = predecessor_phase_ids
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            phase_front_ids.push(front_phase_id.clone());
            phase_front_descriptors.push(PlanarComponentDescriptor {
                phase_id: front_phase_id.clone(),
                bounds,
            });
            front_phases.push(PhaseDesign {
                phase_id: front_phase_id,
                pushback_index: phase.pushback_index,
                shell_index: phase.shell_index,
                revenue_factor: phase.revenue_factor,
                bench: phase.bench,
                block_count: block_indices.len(),
                total_tonnage: Some(
                    block_indices
                        .iter()
                        .filter_map(|linear_index| {
                            tonnage_by_linear_index.get(linear_index).copied()
                        })
                        .sum::<f64>(),
                ),
                block_indices,
                predecessor_phase_ids,
            });
        }

        if phase_front_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "localized directional front split produced no bands for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        front_descriptors_by_phase.insert(phase.phase_id.clone(), phase_front_descriptors);
    }

    Ok(PushbackPlan {
        phase_count: front_phases.len(),
        total_block_count: front_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            front_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: front_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "Localized directional front bands split each shell×bench phase into up to {band_count} dominant-axis front bands and localize predecessor links by planar overlap/adjacency instead of using only whole-phase chains."
            )))
            .collect(),
    })
}

fn split_phase_plan_by_adaptive_component_fronts(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
    min_component_share: f64,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut adaptive_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut adaptive_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let predecessor_phase_ids = phase
            .predecessor_phase_ids
            .iter()
            .map(|predecessor_phase_id| {
                adaptive_phase_ids_by_phase
                    .get(predecessor_phase_id)
                    .cloned()
                    .ok_or_else(|| mine_sdk::MineError::Planning {
                        message: format!(
                            "adaptive component front split is missing predecessor units for phase `{predecessor_phase_id}`"
                        ),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let phase_total_tonnage = phase.total_tonnage.unwrap_or_else(|| {
            phase
                .block_indices
                .iter()
                .filter_map(|linear_index| tonnage_by_linear_index.get(linear_index).copied())
                .sum::<f64>()
        });
        let mut phase_adaptive_ids = Vec::<String>::new();
        let phase_has_multiple_components = components.len() > 1;

        for (component_index, component_block_indices) in components.into_iter().enumerate() {
            let component_tonnage = component_block_indices
                .iter()
                .filter_map(|linear_index| tonnage_by_linear_index.get(linear_index).copied())
                .sum::<f64>();
            let candidate_fronts = split_component_by_dominant_axis_stripes(
                model,
                &component_block_indices,
                tonnage_by_linear_index,
                max_front_count,
            )?;
            let should_split = phase_has_multiple_components
                && phase_total_tonnage > 1.0e-9
                && component_tonnage / phase_total_tonnage >= min_component_share
                && candidate_fronts.len() > 1;
            let adaptive_fronts = if should_split {
                candidate_fronts
            } else {
                vec![component_block_indices]
            };
            let mut previous_front_phase_id = None::<String>;

            for (front_index, mut block_indices) in adaptive_fronts.into_iter().enumerate() {
                if block_indices.is_empty() {
                    continue;
                }
                block_indices.sort_unstable();
                let adaptive_phase_id = if should_split {
                    format!(
                        "{}::afront-c{:02}s{:02}",
                        phase.phase_id,
                        component_index + 1,
                        front_index + 1
                    )
                } else {
                    format!("{}::afront-c{:02}", phase.phase_id, component_index + 1)
                };
                let adaptive_predecessor_phase_ids =
                    if let Some(previous_front_phase_id) = &previous_front_phase_id {
                        vec![previous_front_phase_id.clone()]
                    } else {
                        predecessor_phase_ids.clone()
                    };

                phase_adaptive_ids.push(adaptive_phase_id.clone());
                adaptive_phases.push(PhaseDesign {
                    phase_id: adaptive_phase_id.clone(),
                    pushback_index: phase.pushback_index,
                    shell_index: phase.shell_index,
                    revenue_factor: phase.revenue_factor,
                    bench: phase.bench,
                    block_count: block_indices.len(),
                    total_tonnage: Some(
                        block_indices
                            .iter()
                            .filter_map(|linear_index| {
                                tonnage_by_linear_index.get(linear_index).copied()
                            })
                            .sum::<f64>(),
                    ),
                    block_indices,
                    predecessor_phase_ids: adaptive_predecessor_phase_ids,
                });
                previous_front_phase_id = Some(adaptive_phase_id);
            }
        }

        if phase_adaptive_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "adaptive component front split produced no units for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        adaptive_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_adaptive_ids);
    }

    Ok(PushbackPlan {
        phase_count: adaptive_phases.len(),
        total_block_count: adaptive_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            adaptive_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: adaptive_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "Adaptive component fronts start from planar connected components and only split components whose tonnage share is at least {:.0}% of their phase into up to {max_front_count} dominant-axis fronts.",
                min_component_share * 100.0
            )))
            .collect(),
    })
}

fn split_phase_plan_by_shape_gated_component_fronts(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut gated_phase_ids_by_phase = BTreeMap::<String, Vec<String>>::new();
    let mut gated_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let predecessor_phase_ids = phase
            .predecessor_phase_ids
            .iter()
            .map(|predecessor_phase_id| {
                gated_phase_ids_by_phase
                    .get(predecessor_phase_id)
                    .cloned()
                    .ok_or_else(|| mine_sdk::MineError::Planning {
                        message: format!(
                            "shape-gated front split is missing predecessor units for phase `{predecessor_phase_id}`"
                        ),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let phase_has_multiple_components = components.len() > 1;
        let mut phase_gated_ids = Vec::<String>::new();

        for (component_index, component_block_indices) in components.into_iter().enumerate() {
            let bounds =
                PlanarComponentBounds::from_block_indices(model, &component_block_indices)?;
            let span_i = bounds.max_i.saturating_sub(bounds.min_i);
            let span_j = bounds.max_j.saturating_sub(bounds.min_j);
            let dominant_span = span_i.max(span_j);
            let minor_span = span_i.min(span_j);
            let aspect_ratio = if minor_span == 0 {
                dominant_span as f64
            } else {
                dominant_span as f64 / minor_span as f64
            };
            let candidate_fronts = split_component_by_dominant_axis_stripes(
                model,
                &component_block_indices,
                tonnage_by_linear_index,
                max_front_count,
            )?;
            let should_split = phase_has_multiple_components
                && dominant_span >= min_dominant_span
                && aspect_ratio >= min_aspect_ratio
                && candidate_fronts.len() > 1;
            let gated_fronts = if should_split {
                candidate_fronts
            } else {
                vec![component_block_indices]
            };
            let mut previous_front_phase_id = None::<String>;

            for (front_index, mut block_indices) in gated_fronts.into_iter().enumerate() {
                if block_indices.is_empty() {
                    continue;
                }
                block_indices.sort_unstable();
                let gated_phase_id = if should_split {
                    format!(
                        "{}::sfront-c{:02}s{:02}",
                        phase.phase_id,
                        component_index + 1,
                        front_index + 1
                    )
                } else {
                    format!("{}::sfront-c{:02}", phase.phase_id, component_index + 1)
                };
                let gated_predecessor_phase_ids =
                    if let Some(previous_front_phase_id) = &previous_front_phase_id {
                        vec![previous_front_phase_id.clone()]
                    } else {
                        predecessor_phase_ids.clone()
                    };

                phase_gated_ids.push(gated_phase_id.clone());
                gated_phases.push(PhaseDesign {
                    phase_id: gated_phase_id.clone(),
                    pushback_index: phase.pushback_index,
                    shell_index: phase.shell_index,
                    revenue_factor: phase.revenue_factor,
                    bench: phase.bench,
                    block_count: block_indices.len(),
                    total_tonnage: Some(
                        block_indices
                            .iter()
                            .filter_map(|linear_index| {
                                tonnage_by_linear_index.get(linear_index).copied()
                            })
                            .sum::<f64>(),
                    ),
                    block_indices,
                    predecessor_phase_ids: gated_predecessor_phase_ids,
                });
                previous_front_phase_id = Some(gated_phase_id);
            }
        }

        if phase_gated_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "shape-gated front split produced no units for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        gated_phase_ids_by_phase.insert(phase.phase_id.clone(), phase_gated_ids);
    }

    Ok(PushbackPlan {
        phase_count: gated_phases.len(),
        total_block_count: gated_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            gated_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: gated_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "Shape-gated fronts start from planar connected components and only split components with dominant-span >= {min_dominant_span} and aspect ratio >= {min_aspect_ratio:.1} into up to {max_front_count} dominant-axis fronts."
            )))
            .collect(),
    })
}

fn split_phase_plan_by_shape_gated_component_fronts_with_local_access(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    include_touching_neighbors: bool,
    max_local_predecessor_count: Option<usize>,
    front_progression_cumulative_targets: Option<&[f64]>,
    conditional_progression_min_aspect_ratio: Option<f64>,
    conditional_local_predecessor_count: Option<(f64, usize)>,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let mut front_descriptors_by_phase = BTreeMap::<String, Vec<PlanarComponentDescriptor>>::new();
    let mut gated_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let phase_has_multiple_components = components.len() > 1;
        let mut phase_gated_ids = Vec::<String>::new();
        let mut phase_gated_descriptors = Vec::<PlanarComponentDescriptor>::new();

        for (component_index, component_block_indices) in components.into_iter().enumerate() {
            let bounds =
                PlanarComponentBounds::from_block_indices(model, &component_block_indices)?;
            let span_i = bounds.max_i.saturating_sub(bounds.min_i);
            let span_j = bounds.max_j.saturating_sub(bounds.min_j);
            let dominant_span = span_i.max(span_j);
            let minor_span = span_i.min(span_j);
            let aspect_ratio = if minor_span == 0 {
                dominant_span as f64
            } else {
                dominant_span as f64 / minor_span as f64
            };
            let apply_custom_progression = conditional_progression_min_aspect_ratio
                .map(|threshold| aspect_ratio >= threshold)
                .unwrap_or(true);
            let candidate_fronts = if apply_custom_progression {
                if let Some(cumulative_targets) = front_progression_cumulative_targets {
                    split_component_by_dominant_axis_stripes_with_cumulative_targets(
                        model,
                        &component_block_indices,
                        tonnage_by_linear_index,
                        max_front_count,
                        cumulative_targets,
                    )?
                } else {
                    split_component_by_dominant_axis_stripes(
                        model,
                        &component_block_indices,
                        tonnage_by_linear_index,
                        max_front_count,
                    )?
                }
            } else {
                split_component_by_dominant_axis_stripes(
                    model,
                    &component_block_indices,
                    tonnage_by_linear_index,
                    max_front_count,
                )?
            };
            let should_split = phase_has_multiple_components
                && dominant_span >= min_dominant_span
                && aspect_ratio >= min_aspect_ratio
                && candidate_fronts.len() > 1;
            let gated_fronts = if should_split {
                candidate_fronts
            } else {
                vec![component_block_indices]
            };
            let mut previous_front_phase_id = None::<String>;

            for (front_index, mut block_indices) in gated_fronts.into_iter().enumerate() {
                if block_indices.is_empty() {
                    continue;
                }
                block_indices.sort_unstable();
                let front_bounds =
                    PlanarComponentBounds::from_block_indices(model, &block_indices)?;
                let gated_phase_id = if should_split {
                    format!(
                        "{}::slfront-c{:02}s{:02}",
                        phase.phase_id,
                        component_index + 1,
                        front_index + 1
                    )
                } else {
                    format!("{}::slfront-c{:02}", phase.phase_id, component_index + 1)
                };
                let local_predecessor_count = conditional_local_predecessor_count
                    .map(|(min_aspect_ratio_for_window, promoted_count)| {
                        if aspect_ratio >= min_aspect_ratio_for_window {
                            Some(promoted_count)
                        } else {
                            max_local_predecessor_count
                        }
                    })
                    .unwrap_or(max_local_predecessor_count);
                let mut predecessor_phase_ids = phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|predecessor_phase_id| {
                        front_descriptors_by_phase
                            .get(predecessor_phase_id)
                            .map(|descriptors| {
                                select_localized_planar_predecessors(
                                    &front_bounds,
                                    descriptors,
                                    include_touching_neighbors,
                                    local_predecessor_count,
                                )
                            })
                            .ok_or_else(|| mine_sdk::MineError::Planning {
                                message: format!(
                                    "localized shape-gated front split is missing predecessor units for phase `{predecessor_phase_id}`"
                                ),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if let Some(previous_front_phase_id) = &previous_front_phase_id {
                    predecessor_phase_ids.push(previous_front_phase_id.clone());
                }
                predecessor_phase_ids = predecessor_phase_ids
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();

                phase_gated_ids.push(gated_phase_id.clone());
                phase_gated_descriptors.push(PlanarComponentDescriptor {
                    phase_id: gated_phase_id.clone(),
                    bounds: front_bounds,
                });
                gated_phases.push(PhaseDesign {
                    phase_id: gated_phase_id.clone(),
                    pushback_index: phase.pushback_index,
                    shell_index: phase.shell_index,
                    revenue_factor: phase.revenue_factor,
                    bench: phase.bench,
                    block_count: block_indices.len(),
                    total_tonnage: Some(
                        block_indices
                            .iter()
                            .filter_map(|linear_index| {
                                tonnage_by_linear_index.get(linear_index).copied()
                            })
                            .sum::<f64>(),
                    ),
                    block_indices,
                    predecessor_phase_ids,
                });
                previous_front_phase_id = Some(gated_phase_id);
            }
        }

        if phase_gated_ids.is_empty() {
            return Err(mine_sdk::MineError::Planning {
                message: format!(
                    "localized shape-gated front split produced no units for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        front_descriptors_by_phase.insert(phase.phase_id.clone(), phase_gated_descriptors);
    }

    Ok(PushbackPlan {
        phase_count: gated_phases.len(),
        total_block_count: gated_phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(
            gated_phases
                .iter()
                .filter_map(|phase| phase.total_tonnage)
                .sum::<f64>(),
        ),
        phases: gated_phases,
        nesting_rules: phase_plan.nesting_rules.clone(),
        limitations: phase_plan
            .limitations
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "Localized shape-gated fronts keep the shape gate (dominant-span >= {min_dominant_span}, aspect ratio >= {min_aspect_ratio:.1}), split selected components into up to {max_front_count} dominant-axis fronts, localize predecessor links with `{}` filtering, use `{}` predecessor-window policy, and use `{}` front progression with `{}` activation.",
                localized_access_mode_label(include_touching_neighbors),
                match conditional_local_predecessor_count {
                    Some((min_aspect_ratio_for_window, promoted_count)) => format!(
                        "base `{}` with promoted closest-N={promoted_count} for aspect-ratio >= {min_aspect_ratio_for_window:.1}",
                        match max_local_predecessor_count {
                            Some(base_count) => format!("closest-N={base_count}"),
                            None => "unbounded predecessor fan-in".to_owned(),
                        }
                    ),
                    None => match max_local_predecessor_count {
                        Some(base_count) => format!("fixed closest-N={base_count}"),
                        None => "unbounded predecessor fan-in".to_owned(),
                    },
                },
                front_progression_contract_label(front_progression_cumulative_targets),
                match conditional_progression_min_aspect_ratio {
                    Some(min_aspect_ratio_for_progression) =>
                        format!("aspect-ratio >= {min_aspect_ratio_for_progression:.1}"),
                    None => "always-on".to_owned(),
                }
            )))
            .collect(),
    })
}

fn build_staggered_unit_target_periods_from_phase_targets(
    scheduling_problem: &SchedulingProblem,
    period_by_phase: &BTreeMap<String, usize>,
) -> Result<BTreeMap<SchedulingUnitId, usize>, mine_sdk::MineError> {
    let mut units_by_phase = BTreeMap::<String, Vec<SchedulingUnitId>>::new();
    for unit in scheduling_problem.units() {
        let phase_id = unit
            .unit_id()
            .as_str()
            .split("::part-")
            .next()
            .unwrap_or_else(|| unit.unit_id().as_str())
            .to_owned();
        units_by_phase
            .entry(phase_id)
            .or_default()
            .push(unit.unit_id().clone());
    }

    let mut staggered_target_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    for (phase_id, unit_ids) in units_by_phase {
        let phase_target_period = period_by_phase.get(&phase_id).copied().ok_or_else(|| {
            mine_sdk::MineError::Planning {
                message: format!(
                    "LP target-period mapping is missing staggered source phase `{phase_id}`"
                ),
            }
        })?;
        let last_index = unit_ids.len().saturating_sub(1);
        for (unit_index, unit_id) in unit_ids.into_iter().enumerate() {
            staggered_target_by_unit.insert(
                unit_id,
                phase_target_period.saturating_sub(last_index.saturating_sub(unit_index)),
            );
        }
    }

    let mut repaired_target_by_unit = BTreeMap::<SchedulingUnitId, usize>::new();
    for unit in scheduling_problem.units() {
        let predecessor_target_period = unit
            .predecessor_unit_ids()
            .iter()
            .filter_map(|predecessor_id| repaired_target_by_unit.get(predecessor_id).copied())
            .max()
            .unwrap_or(0);
        let target_period = staggered_target_by_unit
            .get(unit.unit_id())
            .copied()
            .ok_or_else(|| mine_sdk::MineError::Planning {
                message: format!(
                    "staggered LP target-period mapping is missing unit `{}`",
                    unit.unit_id()
                ),
            })?
            .max(predecessor_target_period);
        repaired_target_by_unit.insert(unit.unit_id().clone(), target_period);
    }

    Ok(repaired_target_by_unit)
}

fn representative_period_by_block(
    solution: &marvin_support::MarvinScheduleSolution,
) -> BTreeMap<usize, f64> {
    solution
        .assignments
        .iter()
        .filter(|assignment| assignment.fraction > 1.0e-9)
        .fold(
            BTreeMap::<usize, (f64, f64)>::new(),
            |mut acc, assignment| {
                let entry = acc.entry(assignment.linear_index).or_insert((0.0, 0.0));
                entry.0 += assignment.period_index as f64 * assignment.fraction;
                entry.1 += assignment.fraction;
                acc
            },
        )
        .into_iter()
        .map(|(linear_index, (weighted_period_sum, total_fraction))| {
            (
                linear_index,
                weighted_period_sum / total_fraction.max(1.0e-9),
            )
        })
        .collect()
}

fn representative_period_index(period_index: f64) -> usize {
    period_index.round().max(0.0) as usize
}

fn build_reference_period_destination_memberships(
    solution: &marvin_support::MarvinScheduleSolution,
) -> BTreeMap<String, BTreeSet<usize>> {
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();
    for assignment in &solution.assignments {
        memberships
            .entry(format!(
                "P{:02}.D{:02}",
                assignment.period_index + 1,
                assignment.destination_index
            ))
            .or_default()
            .insert(assignment.linear_index);
    }
    memberships
}

fn compare_period_memberships(
    reference: &BTreeMap<String, BTreeSet<usize>>,
    candidate: &BTreeMap<String, BTreeSet<usize>>,
) -> CompactPeriodMembershipComparison {
    let reference_assignments = reference
        .iter()
        .flat_map(|(period_label, blocks)| {
            blocks
                .iter()
                .map(move |linear_index| (period_label.clone(), *linear_index))
        })
        .collect::<BTreeSet<_>>();
    let candidate_assignments = candidate
        .iter()
        .flat_map(|(period_label, blocks)| {
            blocks
                .iter()
                .map(move |linear_index| (period_label.clone(), *linear_index))
        })
        .collect::<BTreeSet<_>>();
    let shared_assignments = reference_assignments
        .intersection(&candidate_assignments)
        .count();
    let reference_only = reference_assignments
        .difference(&candidate_assignments)
        .cloned()
        .collect::<Vec<_>>();
    let candidate_only = candidate_assignments
        .difference(&reference_assignments)
        .cloned()
        .collect::<Vec<_>>();
    let union = reference_assignments.len() + candidate_only.len();
    let jaccard_index = if union == 0 {
        1.0
    } else {
        shared_assignments as f64 / union as f64
    };

    CompactPeriodMembershipComparison {
        shared_assignments,
        reference_only_assignment_count: reference_only.len(),
        candidate_only_assignment_count: candidate_only.len(),
        jaccard_index,
        reference_only_assignment_examples: reference_only.into_iter().take(10).collect(),
        candidate_only_assignment_examples: candidate_only.into_iter().take(10).collect(),
    }
}

fn build_reference_period_metric_map(
    model: &BlockModel,
    problem: &MarvinScheduleProblem,
    solution: &marvin_support::MarvinScheduleSolution,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, f64>, mine_sdk::MineError> {
    let objective_lookup = problem
        .objective_terms
        .iter()
        .map(|term| {
            (
                (term.linear_index, term.destination_index),
                term.objective_value,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let tonnage_values = float_column(model, tonnage_column)?;
    let mut tonnage_by_period = BTreeMap::<String, f64>::new();
    let mut discounted_objective_by_period = BTreeMap::<String, f64>::new();
    let mut block_membership_by_period = BTreeMap::<String, BTreeSet<usize>>::new();

    for assignment in &solution.assignments {
        let period_label = format!("P{:02}", assignment.period_index + 1);
        let row_index = row_index_for_linear_index(model, assignment.linear_index)?;
        let tonnage = tonnage_values[row_index] * assignment.fraction;
        let objective_value = objective_lookup
            .get(&(assignment.linear_index, assignment.destination_index))
            .copied()
            .ok_or_else(|| {
                mine_sdk::MineError::validation(format!(
                    "missing Marvin objective term for block {} and destination {}",
                    assignment.linear_index, assignment.destination_index
                ))
            })?;

        *tonnage_by_period.entry(period_label.clone()).or_insert(0.0) += tonnage;
        *discounted_objective_by_period
            .entry(period_label.clone())
            .or_insert(0.0) += objective_value * assignment.fraction
            / (1.0 + problem.discount_rate).powi(assignment.period_index as i32);
        block_membership_by_period
            .entry(period_label)
            .or_default()
            .insert(assignment.linear_index);
    }

    let mut metrics = BTreeMap::new();
    for period_index in 0..problem.period_count {
        let period_label = format!("P{:02}", period_index + 1);
        metrics.insert(
            format!("{period_label}.tonnage"),
            tonnage_by_period.get(&period_label).copied().unwrap_or(0.0),
        );
        metrics.insert(
            format!("{period_label}.discounted_objective"),
            discounted_objective_by_period
                .get(&period_label)
                .copied()
                .unwrap_or(0.0),
        );
        metrics.insert(
            format!("{period_label}.block_count"),
            block_membership_by_period
                .get(&period_label)
                .map_or(0usize, BTreeSet::len) as f64,
        );
    }
    Ok(metrics)
}

fn build_candidate_pcpsp_solution(
    problem: &MarvinScheduleProblem,
    period_memberships: &BTreeMap<String, BTreeSet<usize>>,
) -> Result<MarvinScheduleSolution, mine_sdk::MineError> {
    let objective_lookup = problem.objective_terms.iter().fold(
        BTreeMap::<usize, Vec<(usize, f64)>>::new(),
        |mut acc, term| {
            acc.entry(term.linear_index)
                .or_default()
                .push((term.destination_index, term.objective_value));
            acc
        },
    );
    let resource_coefficients = problem.resource_coefficients.iter().fold(
        BTreeMap::<(usize, usize, usize), f64>::new(),
        |mut acc, coefficient| {
            acc.insert(
                (
                    coefficient.linear_index,
                    coefficient.destination_index,
                    coefficient.resource_index,
                ),
                coefficient.coefficient,
            );
            acc
        },
    );

    let mut assignments = Vec::new();
    for period_index in 0..problem.period_count {
        let period_label = format!("P{:02}", period_index + 1);
        let mut blocks = period_memberships
            .get(&period_label)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| {
            let left_best = objective_lookup
                .get(left)
                .and_then(|destinations| {
                    destinations
                        .iter()
                        .map(|(_, objective)| *objective)
                        .max_by(|a, b| a.partial_cmp(b).expect("objective should be finite"))
                })
                .unwrap_or(f64::NEG_INFINITY);
            let right_best = objective_lookup
                .get(right)
                .and_then(|destinations| {
                    destinations
                        .iter()
                        .map(|(_, objective)| *objective)
                        .max_by(|a, b| a.partial_cmp(b).expect("objective should be finite"))
                })
                .unwrap_or(f64::NEG_INFINITY);
            right_best
                .partial_cmp(&left_best)
                .expect("objective should be finite")
                .then_with(|| left.cmp(right))
        });

        let mut remaining_limits = problem
            .resource_constraint_limits
            .iter()
            .filter(|limit| limit.period_index == period_index && limit.relation == 'L')
            .map(|limit| (limit.resource_index, limit.limit))
            .collect::<BTreeMap<_, _>>();

        for linear_index in blocks {
            let mut destinations =
                objective_lookup
                    .get(&linear_index)
                    .cloned()
                    .ok_or_else(|| {
                        mine_sdk::MineError::validation(format!(
                            "missing Marvin objective terms for candidate block `{linear_index}`"
                        ))
                    })?;
            destinations.sort_by(|left, right| {
                right
                    .1
                    .partial_cmp(&left.1)
                    .expect("objective should be finite")
                    .then_with(|| left.0.cmp(&right.0))
            });

            let selected_destination = destinations
                .iter()
                .find(|(destination_index, _)| {
                    remaining_limits
                        .iter()
                        .all(|(resource_index, remaining_limit)| {
                            let coefficient = resource_coefficients
                                .get(&(linear_index, *destination_index, *resource_index))
                                .copied()
                                .unwrap_or(0.0);
                            coefficient <= *remaining_limit + 1.0e-9
                        })
                })
                .map(|(destination_index, _)| *destination_index)
                .unwrap_or(destinations[0].0);

            for (resource_index, remaining_limit) in &mut remaining_limits {
                let coefficient = resource_coefficients
                    .get(&(linear_index, selected_destination, *resource_index))
                    .copied()
                    .unwrap_or(0.0);
                *remaining_limit -= coefficient;
            }

            assignments.push(MarvinScheduleAssignment {
                linear_index,
                destination_index: selected_destination,
                period_index,
                fraction: 1.0,
            });
        }
    }

    Ok(MarvinScheduleSolution {
        kind: problem.kind,
        unique_block_count: assignments
            .iter()
            .map(|assignment| assignment.linear_index)
            .collect::<BTreeSet<_>>()
            .len(),
        assignments,
    })
}

fn build_lp_bz_gap_metrics(
    bound_artifact: &LpBzBoundArtifact,
    lp_solve_artifact: &LpBzLpSolveArtifact,
    candidate_summary: &MarvinScheduleSolutionSummary,
    pcpsp_reference_discounted_objective: f64,
    ready_frontier_discounted_objective: f64,
) -> LpBzGapMetrics {
    let native_lp_kernel_discounted_objective_bound =
        if lp_solve_artifact.solve_status == LpBzLpSolveStatus::Optimal {
            lp_solve_artifact.discounted_objective_bound
        } else {
            None
        };
    let (effective_discounted_objective_bound, effective_bound_source) =
        select_effective_lp_bz_bound(
            bound_artifact,
            native_lp_kernel_discounted_objective_bound,
            candidate_summary.discounted_objective,
        );
    let bound_to_candidate_absolute_gap =
        effective_discounted_objective_bound - candidate_summary.discounted_objective;
    let bound_to_candidate_relative_gap =
        if effective_discounted_objective_bound.abs() <= f64::EPSILON {
            0.0
        } else {
            bound_to_candidate_absolute_gap.abs() / effective_discounted_objective_bound.abs()
        };

    LpBzGapMetrics {
        effective_discounted_objective_bound,
        effective_bound_source,
        native_lp_kernel_discounted_objective_bound,
        bound_to_candidate_absolute_gap,
        bound_to_candidate_relative_gap,
        candidate_vs_pcpsp_reference_objective_gap: pcpsp_reference_discounted_objective
            - candidate_summary.discounted_objective,
        candidate_vs_ready_frontier_objective_gap: candidate_summary.discounted_objective
            - ready_frontier_discounted_objective,
    }
}

fn select_effective_lp_bz_bound(
    bound_artifact: &LpBzBoundArtifact,
    native_lp_kernel_discounted_objective_bound: Option<f64>,
    candidate_discounted_objective: f64,
) -> (f64, String) {
    if let Some(native_lp_bound) = native_lp_kernel_discounted_objective_bound {
        if native_lp_bound.is_finite() && native_lp_bound + 1.0e-6 >= candidate_discounted_objective
        {
            return (
                bound_artifact
                    .discounted_objective_bound
                    .min(native_lp_bound),
                "min(native-resource-envelope, native-lp-kernel)".to_owned(),
            );
        }
    }
    (
        bound_artifact.discounted_objective_bound,
        "native-resource-envelope".to_owned(),
    )
}

fn validate_lp_bz_artifact_coherence(
    lp_bz_inputs: &LpBzInputArtifact,
    lp_bz_bound_artifact: &LpBzBoundArtifact,
    lp_bz_lp_kernel_artifact: &LpBzLpKernelArtifact,
) -> Result<(), MineError> {
    let normalized_period_count = lp_bz_inputs.problem_normalization.period_count;
    let normalized_destination_count = lp_bz_inputs.problem_normalization.destination_count;
    let normalized_unit_count = lp_bz_inputs.precedence_units.unit_count;
    let normalized_discount_rate = lp_bz_inputs.problem_normalization.discount_rate;

    if lp_bz_bound_artifact.period_count != normalized_period_count {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_bound_artifact.period_count={} but lp_bz_inputs.problem_normalization.period_count={normalized_period_count}",
            lp_bz_bound_artifact.period_count
        )));
    }
    if lp_bz_bound_artifact.destination_count != normalized_destination_count {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_bound_artifact.destination_count={} but lp_bz_inputs.problem_normalization.destination_count={normalized_destination_count}",
            lp_bz_bound_artifact.destination_count
        )));
    }
    if lp_bz_bound_artifact.unit_count != normalized_unit_count {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_bound_artifact.unit_count={} but lp_bz_inputs.precedence_units.unit_count={normalized_unit_count}",
            lp_bz_bound_artifact.unit_count
        )));
    }
    if lp_bz_lp_kernel_artifact.period_count != normalized_period_count {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_lp_kernel_artifact.period_count={} but lp_bz_inputs.problem_normalization.period_count={normalized_period_count}",
            lp_bz_lp_kernel_artifact.period_count
        )));
    }
    if lp_bz_lp_kernel_artifact.destination_count != normalized_destination_count {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_lp_kernel_artifact.destination_count={} but lp_bz_inputs.problem_normalization.destination_count={normalized_destination_count}",
            lp_bz_lp_kernel_artifact.destination_count
        )));
    }
    if lp_bz_lp_kernel_artifact.unit_count != normalized_unit_count {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_lp_kernel_artifact.unit_count={} but lp_bz_inputs.precedence_units.unit_count={normalized_unit_count}",
            lp_bz_lp_kernel_artifact.unit_count
        )));
    }
    if (lp_bz_lp_kernel_artifact.discount_rate - normalized_discount_rate).abs() > 1.0e-12 {
        return Err(MineError::validation(format!(
            "LP/BZ artifact coherence error: lp_bz_lp_kernel_artifact.discount_rate={} but lp_bz_inputs.problem_normalization.discount_rate={normalized_discount_rate}",
            lp_bz_lp_kernel_artifact.discount_rate
        )));
    }

    Ok(())
}

fn build_lp_bz_rounder_v6_local_optimizer_diagnostics(
    artifacts: &lp_bz_rounder::LpBzRoundRepairArtifacts,
) -> LpBzRounderV6LocalOptimizerDiagnostics {
    LpBzRounderV6LocalOptimizerDiagnostics {
        rounder_strategy_label: "lp-bz-rounder-v6-topological-round-repair".to_owned(),
        local_optimizer_strategy_label: artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .strategy_label
            .clone(),
        local_optimizer_max_iteration_count: artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .max_iteration_count,
        local_optimizer_executed_iteration_count: artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .executed_iteration_count,
        local_optimizer_improving_move_count: artifacts
            .unit_round_repair
            .local_improvement_move_count,
        local_optimizer_termination_reason: artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .termination_reason
            .clone(),
        repaired_phase_target_count: artifacts.repaired_phase_target_count,
        repaired_unit_target_count: artifacts.unit_round_repair.repaired_unit_target_count,
        horizon_clamp_count: artifacts.unit_round_repair.horizon_clamp_count,
        phase_target_count: artifacts.phase_target_period_by_phase.len(),
        unit_target_count: artifacts.unit_round_repair.target_period_by_unit.len(),
    }
}

fn build_baseline_summary(
    baseline_name: &str,
    phase_count: usize,
    pcpsp_summary: &MarvinScheduleSolutionSummary,
    pcpsp_solution: &MarvinScheduleSolution,
    candidate_summary: &MarvinScheduleSolutionSummary,
    candidate_solution: &MarvinScheduleSolution,
) -> SchedulingBaselineSummary {
    SchedulingBaselineSummary {
        baseline_name: baseline_name.to_owned(),
        phase_count,
        candidate_pcpsp_summary: candidate_summary.clone(),
        candidate_vs_reference_metrics: compare_named_numeric_metrics(
            &BTreeMap::from([
                (
                    "discounted_objective".to_owned(),
                    pcpsp_summary.discounted_objective,
                ),
                (
                    "used_period_count".to_owned(),
                    pcpsp_summary.used_period_count as f64,
                ),
                (
                    "unique_block_count".to_owned(),
                    pcpsp_summary.unique_block_count as f64,
                ),
                (
                    "used_destination_count".to_owned(),
                    pcpsp_summary.used_destination_count as f64,
                ),
            ]),
            &BTreeMap::from([
                (
                    "discounted_objective".to_owned(),
                    candidate_summary.discounted_objective,
                ),
                (
                    "used_period_count".to_owned(),
                    candidate_summary.used_period_count as f64,
                ),
                (
                    "unique_block_count".to_owned(),
                    candidate_summary.unique_block_count as f64,
                ),
                (
                    "used_destination_count".to_owned(),
                    candidate_summary.used_destination_count as f64,
                ),
            ]),
            &BTreeMap::new(),
        ),
        candidate_vs_reference_membership_comparison: compare_period_memberships(
            &build_reference_period_destination_memberships(pcpsp_solution),
            &build_reference_period_destination_memberships(candidate_solution),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT, LP_BZ_UNIT_GRANULARITY_LABEL,
        LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH, LpBzBandPredecessorLinkPolicy, LpBzGapMetrics,
        LpBzPeriodBandRefinementDiagnostics, MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL,
        MARVIN_BENCHMARK_FOCUSED_MR187_REPORT_FILE, MARVIN_BENCHMARK_FULL_REPORT_FILE,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
        MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE, MarvinBenchmarkMode,
        PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL, PlanarComponentBounds,
        PlanarComponentDescriptor, PushbackBenchLocalizedCutRefinementDiagnostics,
        PushbackBenchLocalizedCutSweepEntry, benchmark_blocks_support,
        build_linear_index_float_lookup, build_lp_bz_access_progression_artifacts,
        build_lp_bz_access_progression_band_artifacts, build_mine_rs_end_to_end_artifacts,
        build_mr187_refresh_assumptions, build_mr187_refresh_comparability_gaps,
        build_mr187_refresh_limitations, compact_lp_bz_lp_kernel_artifact,
        front_progression_contract_label, lp_bz_lp_kernel, marvin_support,
        parse_marvin_benchmark_cli_args, partition_block_indices_by_cumulative_tonnage_targets,
        partition_block_indices_by_tonnage, representative_period_by_block,
        select_localized_planar_predecessors, split_phase_plan_by_representative_period_bands,
        split_phase_plan_by_representative_period_bands_with_link_policy,
        split_phase_plan_by_representative_period_quantiles,
        validate_lp_bz_period_band_refinement_diagnostics, validate_mr187_refresh_contract,
        validate_pushback_bench_localized_cut_calibration_sweep,
        validate_pushback_bench_localized_cut_refinement_diagnostics, write_pretty_json,
    };
    use mine_sdk::{ColumnId, NestingAccessRules, PhaseDesign, PushbackPlan};
    use serde_json::json;
    use std::{collections::BTreeMap, path::PathBuf};

    fn benchmark_path(instance: &str, file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("datasets")
            .join("benchmarks")
            .join(instance)
            .join(file_name)
    }

    fn repo_root_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    #[test]
    fn parse_cli_preserves_full_default_output_path() {
        let repo_root = repo_root_path();

        let cli = parse_marvin_benchmark_cli_args(&repo_root, None, std::iter::empty::<&str>())
            .expect("default Marvin CLI should parse");

        assert_eq!(cli.mode, MarvinBenchmarkMode::Full);
        assert_eq!(
            cli.output_path,
            repo_root
                .join("datasets")
                .join("benchmarks")
                .join("marvin")
                .join("outputs")
                .join(MARVIN_BENCHMARK_FULL_REPORT_FILE)
        );
    }

    #[test]
    fn parse_cli_uses_focused_default_output_path_and_cli_overrides_env() {
        let repo_root = repo_root_path();

        let cli = parse_marvin_benchmark_cli_args(
            &repo_root,
            Some("full"),
            ["--mode", MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL],
        )
        .expect("focused Marvin CLI should parse");

        assert_eq!(cli.mode, MarvinBenchmarkMode::FocusedMr187);
        assert_eq!(
            cli.output_path,
            repo_root
                .join("datasets")
                .join("benchmarks")
                .join("marvin")
                .join("outputs")
                .join(MARVIN_BENCHMARK_FOCUSED_MR187_REPORT_FILE)
        );
    }

    #[test]
    fn partition_block_indices_by_tonnage_preserves_blocks_and_balances_mass() {
        let partitions = partition_block_indices_by_tonnage(
            &[10, 11, 12, 13],
            &BTreeMap::from([(10usize, 4.0), (11, 1.0), (12, 3.0), (13, 2.0)]),
            2,
        );

        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0], vec![10, 11]);
        assert_eq!(partitions[1], vec![12, 13]);
    }

    #[test]
    fn partition_block_indices_by_cumulative_targets_respects_front_loaded_profile() {
        let partitions = partition_block_indices_by_cumulative_tonnage_targets(
            &[10, 11, 12, 13, 14, 15],
            &BTreeMap::from([
                (10usize, 5.0),
                (11, 4.0),
                (12, 3.0),
                (13, 2.0),
                (14, 1.0),
                (15, 1.0),
            ]),
            &[0.55, 0.85, 1.0],
        )
        .expect("front-loaded progression should partition");

        assert_eq!(partitions.len(), 3);
        assert_eq!(partitions[0], vec![10, 11]);
        assert_eq!(partitions[1], vec![12, 13]);
        assert_eq!(partitions[2], vec![14, 15]);
    }

    #[test]
    fn front_progression_contract_label_prefers_known_profile_names() {
        assert_eq!(
            front_progression_contract_label(Some(
                &MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE.cumulative_tonnage_targets
            )),
            format!("`{}`", MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_PROFILE.label)
        );
        assert_eq!(
            front_progression_contract_label(Some(&[0.4, 0.75, 1.0])),
            "custom cumulative targets [0.4, 0.75, 1.0]"
        );
    }

    #[test]
    fn mr187_refresh_assumptions_keep_promoted_family_as_active_route() {
        let assumptions = build_mr187_refresh_assumptions();

        assert!(assumptions.iter().any(|entry| {
            entry.contains(PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL)
                && entry.contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL)
                && entry.contains(LP_BZ_UNIT_GRANULARITY_LABEL)
        }));
    }

    #[test]
    fn mr187_refresh_limitations_keep_promoted_family_benchmark_side() {
        let limitations = build_mr187_refresh_limitations();

        assert!(limitations.iter().any(|entry| {
            entry.contains(PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL)
                && entry.contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL)
                && entry.contains(LP_BZ_UNIT_GRANULARITY_LABEL)
                && entry.contains("exploratory-local")
        }));
    }

    #[test]
    fn mr187_refresh_comparability_gaps_reference_promoted_family() {
        let gaps = build_mr187_refresh_comparability_gaps();

        assert!(gaps.iter().any(|entry| {
            entry.contains(PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_GRANULARITY_LABEL)
                && entry.contains(MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL)
                && entry.contains(LP_BZ_UNIT_GRANULARITY_LABEL)
                && entry.contains("cpit-solution")
                && entry.contains("nested-shell-bench")
        }));
    }

    #[test]
    fn split_phase_plan_by_representative_period_bands_creates_ordered_cut_chain() {
        let phase_plan = PushbackPlan {
            phases: vec![
                PhaseDesign {
                    phase_id: "phase-a".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(0.8),
                    bench: Some(100),
                    block_indices: vec![10, 11, 12],
                    block_count: 3,
                    total_tonnage: Some(6.0),
                    predecessor_phase_ids: Vec::new(),
                },
                PhaseDesign {
                    phase_id: "phase-b".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(0.8),
                    bench: Some(99),
                    block_indices: vec![20, 21],
                    block_count: 2,
                    total_tonnage: Some(4.0),
                    predecessor_phase_ids: vec!["phase-a".to_owned()],
                },
            ],
            phase_count: 2,
            total_block_count: 5,
            total_tonnage: Some(10.0),
            nesting_rules: NestingAccessRules::default_open(),
            limitations: Vec::new(),
        };

        let cut_plan = split_phase_plan_by_representative_period_bands(
            &phase_plan,
            &BTreeMap::from([(10usize, 0.0), (11, 1.0), (12, 3.0), (20, 2.0), (21, 3.0)]),
            &BTreeMap::from([(10usize, 2.0), (11, 2.0), (12, 2.0), (20, 2.0), (21, 2.0)]),
            2,
        )
        .expect("cut split should build");

        assert_eq!(cut_plan.phase_count, 3);
        assert_eq!(cut_plan.phases[0].phase_id, "phase-a::cut-p01");
        assert_eq!(cut_plan.phases[1].phase_id, "phase-a::cut-p02");
        assert_eq!(
            cut_plan.phases[1].predecessor_phase_ids,
            vec!["phase-a::cut-p01"]
        );
        assert_eq!(cut_plan.phases[2].phase_id, "phase-b::cut-p02");
        assert_eq!(
            cut_plan.phases[2].predecessor_phase_ids,
            vec!["phase-a::cut-p02"]
        );
    }

    #[test]
    fn split_phase_plan_by_representative_period_bands_supports_alternative_predecessor_links() {
        let phase_plan = PushbackPlan {
            phases: vec![
                PhaseDesign {
                    phase_id: "phase-a".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(0.8),
                    bench: Some(100),
                    block_indices: vec![10, 11, 12],
                    block_count: 3,
                    total_tonnage: Some(6.0),
                    predecessor_phase_ids: Vec::new(),
                },
                PhaseDesign {
                    phase_id: "phase-b".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(0.8),
                    bench: Some(99),
                    block_indices: vec![20, 21],
                    block_count: 2,
                    total_tonnage: Some(4.0),
                    predecessor_phase_ids: vec!["phase-a".to_owned()],
                },
            ],
            phase_count: 2,
            total_block_count: 5,
            total_tonnage: Some(10.0),
            nesting_rules: NestingAccessRules::default_open(),
            limitations: Vec::new(),
        };
        let representative_periods =
            BTreeMap::from([(10usize, 0.0), (11, 1.0), (12, 3.0), (20, 2.0), (21, 3.0)]);
        let tonnage_lookup =
            BTreeMap::from([(10usize, 2.0), (11, 2.0), (12, 2.0), (20, 2.0), (21, 2.0)]);

        let first_cut_plan = split_phase_plan_by_representative_period_bands_with_link_policy(
            &phase_plan,
            &representative_periods,
            &tonnage_lookup,
            2,
            LpBzBandPredecessorLinkPolicy::PredecessorFirstCut,
        )
        .expect("first-cut predecessor policy should build");
        let all_cuts_plan = split_phase_plan_by_representative_period_bands_with_link_policy(
            &phase_plan,
            &representative_periods,
            &tonnage_lookup,
            2,
            LpBzBandPredecessorLinkPolicy::AllPredecessorCuts,
        )
        .expect("all-predecessor-cuts policy should build");

        assert_eq!(
            first_cut_plan.phases[2].predecessor_phase_ids,
            vec!["phase-a::cut-p01"]
        );
        assert_eq!(
            all_cuts_plan.phases[2].predecessor_phase_ids,
            vec!["phase-a::cut-p01", "phase-a::cut-p02"]
        );
    }

    #[test]
    fn lp_bz_access_progression_artifacts_refine_marvin_phase_plan() {
        let model = benchmark_blocks_support::read_benchmark_blocks(
            benchmark_path("marvin", "marvin.blocks"),
            "marvin",
        )
        .expect("marvin blocks should load");
        let precedence_graph = marvin_support::read_marvin_precedence_graph(
            benchmark_path("marvin", "references\\marvin.prec"),
            &model,
        )
        .expect("marvin precedence should load");
        let pcpsp_problem = marvin_support::read_marvin_pcpsp_problem(
            benchmark_path("marvin", "references\\marvin.pcpsp"),
            &model,
        )
        .expect("marvin pcpsp should load");
        let base_artifacts =
            build_mine_rs_end_to_end_artifacts(&model, &precedence_graph, &pcpsp_problem)
                .expect("base Marvin artifacts should build");
        let tonnage_lookup = build_linear_index_float_lookup(
            &model,
            &ColumnId::new("field_4").expect("field_4 column id should be valid"),
        )
        .expect("tonnage lookup should build");

        let lp_bz_artifacts = build_lp_bz_access_progression_artifacts(
            &model,
            &base_artifacts.phase_plan,
            &pcpsp_problem,
            &tonnage_lookup,
        )
        .expect("lp_bz access/progression artifacts should build");

        assert!(
            lp_bz_artifacts.phase_plan.phase_count > base_artifacts.phase_plan.phase_count,
            "v8 LP/BZ phase plan should refine the coarse shell×bench plan"
        );
        assert!(
            lp_bz_artifacts
                .phase_plan
                .phases
                .iter()
                .any(|phase| phase.phase_id.contains("::slfront")),
            "v8 LP/BZ phase plan should contain localized front splits"
        );
        assert!(
            lp_bz_artifacts
                .phase_plan
                .limitations
                .iter()
                .any(|limitation| limitation.contains("Localized shape-gated fronts")),
            "refined LP/BZ plan should record the localized-front access/progression rule"
        );
    }

    #[test]
    fn lp_bz_access_progression_band_artifacts_refine_localized_fronts() {
        let model = benchmark_blocks_support::read_benchmark_blocks(
            benchmark_path("marvin", "marvin.blocks"),
            "marvin",
        )
        .expect("marvin blocks should load");
        let precedence_graph = marvin_support::read_marvin_precedence_graph(
            benchmark_path("marvin", "references\\marvin.prec"),
            &model,
        )
        .expect("marvin precedence should load");
        let pcpsp_problem = marvin_support::read_marvin_pcpsp_problem(
            benchmark_path("marvin", "references\\marvin.pcpsp"),
            &model,
        )
        .expect("marvin pcpsp should load");
        let lp_pcpsp_solution = marvin_support::read_marvin_lp_pcpsp_solution(
            benchmark_path("marvin", "references\\marvin.LPpcpsp"),
            &model,
        )
        .expect("marvin LPpcpsp should load");
        let base_artifacts =
            build_mine_rs_end_to_end_artifacts(&model, &precedence_graph, &pcpsp_problem)
                .expect("base Marvin artifacts should build");
        let tonnage_lookup = build_linear_index_float_lookup(
            &model,
            &ColumnId::new("field_4").expect("field_4 column id should be valid"),
        )
        .expect("tonnage lookup should build");
        let representative_period_lookup = representative_period_by_block(&lp_pcpsp_solution);
        let localized_front_artifacts = build_lp_bz_access_progression_artifacts(
            &model,
            &base_artifacts.phase_plan,
            &pcpsp_problem,
            &tonnage_lookup,
        )
        .expect("v8 localized-front artifacts should build");

        let band_artifacts = build_lp_bz_access_progression_band_artifacts(
            &model,
            &base_artifacts.phase_plan,
            &pcpsp_problem,
            &tonnage_lookup,
            &representative_period_lookup,
            LP_BZ_V9_LOCAL_FRONT_PERIOD_BAND_WIDTH,
        )
        .expect("v9 localized-front period-band artifacts should build");

        assert!(
            band_artifacts.benchmark.phase_plan.phase_count
                >= localized_front_artifacts.phase_plan.phase_count,
            "v9 composed LP/BZ plan should not reduce phase granularity"
        );
        assert!(
            band_artifacts
                .benchmark
                .phase_plan
                .phases
                .iter()
                .any(|phase| phase.phase_id.contains("::slfront")
                    && phase.phase_id.contains("::cut-p")),
            "v9 composed LP/BZ plan should preserve localized-front ids and append LP cut bands"
        );
        assert!(
            band_artifacts
                .phase_refinement_diagnostics
                .refined_localized_front_phase_count
                > 0,
            "v9 diagnostics should confirm that at least one localized front was split into multiple period bands"
        );
        assert!(
            band_artifacts
                .phase_refinement_diagnostics
                .total_period_band_phase_count
                > band_artifacts
                    .phase_refinement_diagnostics
                    .localized_front_phase_count,
            "v9 diagnostics should show additional period-band phases beyond the v8 localized fronts"
        );
        assert!(
            band_artifacts
                .benchmark
                .phase_plan
                .limitations
                .iter()
                .any(|limitation| limitation.contains(
                    "LP-guided cuts split each shell×bench phase by representative LP period bands"
                )),
            "v9 composed LP/BZ plan should record the LP-guided period-band refinement"
        );
    }

    #[test]
    fn validate_mr187_refresh_contract_accepts_integral_candidate_and_consistent_gap() {
        let candidate_summary = marvin_support::MarvinScheduleSolutionSummary {
            assignment_count: 4,
            unique_block_count: 4,
            fractional_assignment_count: 0,
            used_period_count: 2,
            used_destination_count: 1,
            total_fraction: 4.0,
            min_block_fraction_sum: 1.0,
            max_block_fraction_sum: 1.0,
            undiscounted_objective: 90.0,
            discounted_objective: 90.0,
            resource_summaries: vec![marvin_support::MarvinScheduleResourceSummary {
                resource_index: 0,
                active_period_count: 2,
                max_period_usage: 10.0,
                max_period_limit: Some(12.0),
                max_period_excess: 0.0,
            }],
        };
        let gap_metrics = LpBzGapMetrics {
            effective_discounted_objective_bound: 100.0,
            effective_bound_source: "native-resource-envelope".to_owned(),
            native_lp_kernel_discounted_objective_bound: Some(100.0),
            bound_to_candidate_absolute_gap: 10.0,
            bound_to_candidate_relative_gap: 0.1,
            candidate_vs_pcpsp_reference_objective_gap: 5.0,
            candidate_vs_ready_frontier_objective_gap: 1.5,
        };

        validate_mr187_refresh_contract(
            MARVIN_BENCHMARK_FOCUSED_MR187_MODE_LABEL,
            &candidate_summary,
            &gap_metrics,
            "exploratory-local",
            &[String::from("benchmark-side local-front normalization")],
        )
        .expect("synthetic focused MR-187 contract should validate");
    }

    #[test]
    fn validate_pushback_bench_localized_cut_refinement_diagnostics_rejects_missing_single_component_split()
     {
        let diagnostics = PushbackBenchLocalizedCutRefinementDiagnostics {
            base_phase_count: 4,
            refined_base_phase_count: 2,
            refined_single_component_phase_count: 0,
            total_cut_phase_count: 6,
            additional_phase_count: 2,
            max_cut_count_per_base_phase: 2,
            average_cut_count_per_base_phase: 1.5,
            realized_front_count_histogram: BTreeMap::from([(1usize, 2usize), (2usize, 2usize)]),
            readiness_reason_histogram: BTreeMap::from([
                ("paper-like-three-front-ready".to_owned(), 1usize),
                ("blocked-low-aspect-ratio".to_owned(), 1usize),
            ]),
            exact_three_front_candidate_count: 2,
            exact_three_front_failure_count: 1,
            exact_three_front_failure_realized_front_histogram: BTreeMap::from([(2usize, 1usize)]),
            exact_three_front_failure_reason_histogram: BTreeMap::from([(
                "exact-three-front-infeasible-collapsed-target-partition".to_owned(),
                1usize,
            )]),
            refined_base_phase_examples: vec![String::from("phase-a")],
            refined_single_component_phase_examples: Vec::new(),
        };

        let error = validate_pushback_bench_localized_cut_refinement_diagnostics(&diagnostics)
            .expect_err("builder diagnostics without single-component evidence should be rejected");
        let message = format!("{error}");
        assert!(
            message.contains("single-component shell×bench phase"),
            "validation error should explain the missing single-component refinement, got `{message}`"
        );
    }

    #[test]
    fn validate_pushback_bench_localized_cut_refinement_diagnostics_rejects_inconsistent_exact_three_failure_histogram()
     {
        let diagnostics = PushbackBenchLocalizedCutRefinementDiagnostics {
            base_phase_count: 4,
            refined_base_phase_count: 2,
            refined_single_component_phase_count: 1,
            total_cut_phase_count: 6,
            additional_phase_count: 2,
            max_cut_count_per_base_phase: 2,
            average_cut_count_per_base_phase: 1.5,
            realized_front_count_histogram: BTreeMap::from([(1usize, 2usize), (2usize, 2usize)]),
            readiness_reason_histogram: BTreeMap::from([
                ("paper-like-three-front-ready".to_owned(), 1usize),
                ("blocked-low-aspect-ratio".to_owned(), 1usize),
            ]),
            exact_three_front_candidate_count: 2,
            exact_three_front_failure_count: 1,
            exact_three_front_failure_realized_front_histogram: BTreeMap::new(),
            exact_three_front_failure_reason_histogram: BTreeMap::from([(
                "exact-three-front-infeasible-collapsed-target-partition".to_owned(),
                1usize,
            )]),
            refined_base_phase_examples: vec![String::from("phase-a")],
            refined_single_component_phase_examples: vec![String::from("phase-a")],
        };

        let error = validate_pushback_bench_localized_cut_refinement_diagnostics(&diagnostics)
            .expect_err(
                "builder diagnostics with mismatched exact-three histogram should be rejected",
            );
        let message = format!("{error}");
        assert!(
            message.contains("failure histogram"),
            "validation error should mention the exact-three failure histogram, got `{message}`"
        );
    }

    #[test]
    fn validate_pushback_bench_localized_cut_calibration_sweep_rejects_missing_first_point() {
        let sweep = (0..5)
            .map(|index| PushbackBenchLocalizedCutSweepEntry {
                candidate_label: String::from("candidate-a"),
                is_first_builder_point: false,
                is_best_candidate: index == 0,
                max_front_count: 3,
                min_aspect_ratio: 3.0,
                min_dominant_span: 4,
                localized_access_mode: String::from("overlap-plus-adjacency"),
                max_local_predecessor_count: 4,
                phase_count: 800,
                unit_count: 804,
                candidate_pcpsp_discounted_objective: 662_900_000.0,
                candidate_vs_first_builder_point_objective_delta: 10_000.0,
                candidate_vs_v8_local_front_objective_delta: -50_000.0,
                candidate_vs_pcpsp_reference_objective_gap: 223_000_000.0,
                bound_to_candidate_relative_gap: 0.6,
                repaired_phase_target_count: 400,
                repaired_unit_target_count: 404,
                used_period_count: 10,
            })
            .collect::<Vec<_>>();

        let error = validate_pushback_bench_localized_cut_calibration_sweep(&sweep)
            .expect_err("sweep without a flagged first builder point should be rejected");
        let message = format!("{error}");
        assert!(
            message.contains("first builder point"),
            "validation error should explain the missing first point, got `{message}`"
        );
    }

    #[test]
    fn validate_lp_bz_period_band_refinement_diagnostics_rejects_noop_refinement() {
        let diagnostics = LpBzPeriodBandRefinementDiagnostics {
            period_band_width: 3,
            localized_front_phase_count: 4,
            refined_localized_front_phase_count: 0,
            total_period_band_phase_count: 4,
            additional_phase_count: 0,
            max_cut_count_per_localized_front: 1,
            average_cut_count_per_localized_front: 1.0,
            refined_localized_front_examples: Vec::new(),
        };

        let error = validate_lp_bz_period_band_refinement_diagnostics(&diagnostics)
            .expect_err("no-op v9 refinement should be rejected");
        let message = format!("{error}");
        assert!(
            message.contains("multiple LP period bands"),
            "validation error should explain the missing refinement, got `{message}`"
        );
    }

    #[test]
    fn split_phase_plan_by_representative_period_quantiles_creates_ordered_cut_chain() {
        let phase_plan = PushbackPlan {
            phases: vec![
                PhaseDesign {
                    phase_id: "phase-a".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(0.8),
                    bench: Some(100),
                    block_indices: vec![10, 11, 12, 13],
                    block_count: 4,
                    total_tonnage: Some(10.0),
                    predecessor_phase_ids: Vec::new(),
                },
                PhaseDesign {
                    phase_id: "phase-b".to_owned(),
                    pushback_index: 0,
                    shell_index: Some(0),
                    revenue_factor: Some(0.8),
                    bench: Some(99),
                    block_indices: vec![20, 21],
                    block_count: 2,
                    total_tonnage: Some(4.0),
                    predecessor_phase_ids: vec!["phase-a".to_owned()],
                },
            ],
            phase_count: 2,
            total_block_count: 6,
            total_tonnage: Some(14.0),
            nesting_rules: NestingAccessRules::default_open(),
            limitations: Vec::new(),
        };

        let cut_plan = split_phase_plan_by_representative_period_quantiles(
            &phase_plan,
            &BTreeMap::from([
                (10usize, 0.0),
                (11, 1.0),
                (12, 3.0),
                (13, 3.0),
                (20, 2.0),
                (21, 3.0),
            ]),
            &BTreeMap::from([
                (10usize, 4.0),
                (11, 1.0),
                (12, 3.0),
                (13, 2.0),
                (20, 2.0),
                (21, 2.0),
            ]),
        )
        .expect("quantile cut split should build");

        assert_eq!(cut_plan.phase_count, 5);
        assert_eq!(cut_plan.phases[0].phase_id, "phase-a::qcut-01");
        assert_eq!(cut_plan.phases[1].phase_id, "phase-a::qcut-02");
        assert_eq!(
            cut_plan.phases[1].predecessor_phase_ids,
            vec!["phase-a::qcut-01"]
        );
        assert_eq!(cut_plan.phases[4].phase_id, "phase-b::qcut-02");
        assert_eq!(
            cut_plan.phases[4].predecessor_phase_ids,
            vec!["phase-b::qcut-01"]
        );
    }

    #[test]
    fn localized_planar_predecessors_prefer_overlapping_components_and_fallback_otherwise() {
        let predecessor_components = vec![
            PlanarComponentDescriptor {
                phase_id: "phase-a::glocal-01".to_owned(),
                bounds: PlanarComponentBounds {
                    min_i: 0,
                    max_i: 1,
                    min_j: 0,
                    max_j: 1,
                },
            },
            PlanarComponentDescriptor {
                phase_id: "phase-a::glocal-02".to_owned(),
                bounds: PlanarComponentBounds {
                    min_i: 5,
                    max_i: 6,
                    min_j: 5,
                    max_j: 6,
                },
            },
            PlanarComponentDescriptor {
                phase_id: "phase-a::glocal-03".to_owned(),
                bounds: PlanarComponentBounds {
                    min_i: 2,
                    max_i: 3,
                    min_j: 0,
                    max_j: 1,
                },
            },
        ];

        assert_eq!(
            select_localized_planar_predecessors(
                &PlanarComponentBounds {
                    min_i: 1,
                    max_i: 2,
                    min_j: 0,
                    max_j: 1,
                },
                &predecessor_components,
                true,
                None,
            ),
            vec!["phase-a::glocal-01", "phase-a::glocal-03"]
        );
        assert_eq!(
            select_localized_planar_predecessors(
                &PlanarComponentBounds {
                    min_i: 1,
                    max_i: 2,
                    min_j: 0,
                    max_j: 1,
                },
                &predecessor_components,
                true,
                Some(1),
            ),
            vec!["phase-a::glocal-01"]
        );
        assert_eq!(
            select_localized_planar_predecessors(
                &PlanarComponentBounds {
                    min_i: 2,
                    max_i: 3,
                    min_j: 0,
                    max_j: 1,
                },
                &predecessor_components,
                false,
                None,
            ),
            vec!["phase-a::glocal-03"]
        );
        assert_eq!(
            select_localized_planar_predecessors(
                &PlanarComponentBounds {
                    min_i: 10,
                    max_i: 11,
                    min_j: 10,
                    max_j: 11,
                },
                &predecessor_components,
                true,
                None,
            ),
            vec![
                "phase-a::glocal-01",
                "phase-a::glocal-02",
                "phase-a::glocal-03"
            ]
        );
    }

    #[test]
    fn write_pretty_json_matches_serde_pretty_contract() {
        let value = json!({
            "dataset_dir": "marvin",
            "metrics": {
                "npv": 123.45,
                "periods": ["p01", "p02"]
            }
        });
        let mut buffer = Vec::new();

        write_pretty_json(&mut buffer, &value).expect("pretty JSON should serialize");

        assert_eq!(
            String::from_utf8(buffer).expect("serialized JSON should be UTF-8"),
            serde_json::to_string_pretty(&value).expect("pretty JSON should serialize"),
        );
    }

    #[test]
    fn compact_lp_bz_kernel_artifact_preserves_evidence_and_drops_bulk_arrays() {
        let entries = (0usize..10)
            .map(|index| lp_bz_lp_kernel::LpBzLpKernelVariableEntry {
                variable_index: index,
                key: lp_bz_lp_kernel::LpBzLpKernelVariableKey {
                    unit_id: format!("unit-{:02}", index % 5),
                    destination_id: format!("dest-{:02}", index % 3),
                    period_index: index % 4,
                },
                period_label: format!("P{:02}", (index % 4) + 1),
            })
            .collect::<Vec<_>>();
        let coefficients = entries
            .iter()
            .map(|entry| lp_bz_lp_kernel::LpBzLpKernelObjectiveCoefficient {
                variable_index: entry.variable_index,
                coefficient: 100.0 - entry.variable_index as f64,
                undiscounted_value: 150.0 - entry.variable_index as f64,
                discount_factor: 1.0 + entry.key.period_index as f64 * 0.1,
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for period_index in 0usize..3 {
            rows.push(lp_bz_lp_kernel::LpBzLpKernelConstraintRow {
                row_index: rows.len(),
                row_id: format!("cap-p{:02}", period_index + 1),
                kind: lp_bz_lp_kernel::LpBzLpKernelConstraintKind::CapacityUpper,
                sense: lp_bz_lp_kernel::LpBzLpKernelConstraintSense::LessEqual,
                rhs: 1000.0,
                period_index: Some(period_index),
                period_label: Some(format!("P{:02}", period_index + 1)),
                resource_id: Some("res-mining".to_owned()),
                unit_id: None,
                predecessor_unit_id: None,
                successor_unit_id: None,
                terms: (0usize..5)
                    .map(|offset| lp_bz_lp_kernel::LpBzLpKernelConstraintTerm {
                        variable_index: period_index + offset,
                        coefficient: 1.0,
                    })
                    .collect(),
            });
        }
        for unit_index in 0usize..3 {
            rows.push(lp_bz_lp_kernel::LpBzLpKernelConstraintRow {
                row_index: rows.len(),
                row_id: format!("act-unit-{:02}", unit_index + 1),
                kind: lp_bz_lp_kernel::LpBzLpKernelConstraintKind::ActivationUpper,
                sense: lp_bz_lp_kernel::LpBzLpKernelConstraintSense::LessEqual,
                rhs: 1.0,
                period_index: Some(unit_index),
                period_label: Some(format!("P{:02}", unit_index + 1)),
                resource_id: None,
                unit_id: Some(format!("unit-{:02}", unit_index + 1)),
                predecessor_unit_id: None,
                successor_unit_id: None,
                terms: (0usize..4)
                    .map(|offset| lp_bz_lp_kernel::LpBzLpKernelConstraintTerm {
                        variable_index: unit_index + offset,
                        coefficient: 1.0,
                    })
                    .collect(),
            });
        }
        for pair_index in 0usize..4 {
            rows.push(lp_bz_lp_kernel::LpBzLpKernelConstraintRow {
                row_index: rows.len(),
                row_id: format!("prec-{:02}", pair_index + 1),
                kind: lp_bz_lp_kernel::LpBzLpKernelConstraintKind::PrecedenceActivation,
                sense: lp_bz_lp_kernel::LpBzLpKernelConstraintSense::LessEqual,
                rhs: 0.0,
                period_index: Some(pair_index % 3),
                period_label: Some(format!("P{:02}", (pair_index % 3) + 1)),
                resource_id: None,
                unit_id: None,
                predecessor_unit_id: Some(format!("pred-{:02}", pair_index + 1)),
                successor_unit_id: Some(format!("succ-{:02}", pair_index + 1)),
                terms: (0usize..6)
                    .map(|offset| lp_bz_lp_kernel::LpBzLpKernelConstraintTerm {
                        variable_index: (pair_index + offset) % entries.len(),
                        coefficient: if offset == 0 { 1.0 } else { -1.0 },
                    })
                    .collect(),
            });
        }
        let artifact = lp_bz_lp_kernel::LpBzLpKernelArtifact {
            kernel_label: "lp-bz-lp-kernel-v8".to_owned(),
            period_count: 4,
            unit_count: 5,
            destination_count: 3,
            discount_rate: 0.1,
            variable_index: lp_bz_lp_kernel::LpBzLpKernelVariableIndexArtifact {
                variable_count: entries.len(),
                entries,
            },
            objective: lp_bz_lp_kernel::LpBzLpKernelObjectiveArtifact {
                summary: lp_bz_lp_kernel::LpBzLpKernelObjectiveSummary {
                    coefficient_count: coefficients.len(),
                    non_zero_coefficient_count: coefficients.len(),
                },
                coefficients,
            },
            constraints: lp_bz_lp_kernel::LpBzLpKernelConstraintArtifact {
                summary: lp_bz_lp_kernel::LpBzLpKernelConstraintSummary {
                    row_count: rows.len(),
                    capacity_row_count: 3,
                    activation_row_count: 3,
                    precedence_row_count: 4,
                },
                rows,
            },
            access: lp_bz_lp_kernel::LpBzLpKernelAccessArtifact {
                unit_profile_count: 10,
                unit_profiles: (0usize..10)
                    .map(|index| lp_bz_lp_kernel::LpBzLpKernelAccessUnitProfile {
                        unit_id: format!("unit-{:02}", index + 1),
                        bench: Some(100 - index as i64),
                        shell_index: Some(index % 3),
                        direct_predecessor_count: index,
                        transitive_predecessor_count: index + 2,
                        closure_unit_count: index + 3,
                        closure_resources: vec![
                            lp_bz_lp_kernel::LpBzLpKernelAccessClosureResource {
                                resource_id: "res-mining".to_owned(),
                                minimum_total_requirement: 10.0 + index as f64,
                            },
                        ],
                    })
                    .collect(),
            },
            limitations: vec!["summary-only-report".to_owned()],
        };

        let compact = compact_lp_bz_lp_kernel_artifact(&artifact);
        let full_json = serde_json::to_vec(&artifact).expect("full artifact should serialize");
        let compact_json = serde_json::to_vec(&compact).expect("compact artifact should serialize");

        assert_eq!(
            compact.variable_index.variable_count,
            artifact.variable_index.variable_count
        );
        assert_eq!(
            compact.objective.coefficient_count,
            artifact.objective.summary.coefficient_count
        );
        assert_eq!(
            compact.constraints.row_count,
            artifact.constraints.summary.row_count
        );
        assert_eq!(
            compact.access.unit_profile_count,
            artifact.access.unit_profile_count
        );
        assert_eq!(
            compact.variable_index.omitted_entry_count,
            artifact
                .variable_index
                .entries
                .len()
                .saturating_sub(LP_BZ_KERNEL_REPORT_SAMPLE_LIMIT)
        );
        assert_eq!(compact.constraints.sampled_row_count, 6);
        assert!(
            compact_json.len() < full_json.len(),
            "compact JSON ({} bytes) should be smaller than full JSON ({} bytes)",
            compact_json.len(),
            full_json.len()
        );
    }
}

fn build_candidate_period_metric_map(
    report: &LongTermScheduleEconomicsReport,
    period_memberships: &BTreeMap<String, BTreeSet<usize>>,
) -> BTreeMap<String, f64> {
    let mut metrics = BTreeMap::new();
    for period in &report.periods {
        metrics.insert(format!("{}.tonnage", period.period_label), period.tonnage);
        metrics.insert(
            format!("{}.discounted_objective", period.period_label),
            period.discounted_cashflow,
        );
        metrics.insert(
            format!("{}.block_count", period.period_label),
            period_memberships
                .get(&period.period_label)
                .map_or(0usize, BTreeSet::len) as f64,
        );
    }
    metrics
}

fn build_candidate_period_memberships(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    schedule: &mine_sdk::LongTermSchedule,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, BTreeSet<usize>>, mine_sdk::MineError> {
    let tonnage_values = float_column(model, tonnage_column)?;
    let linear_index_to_row_index = (0..model.block_count())
        .map(|row_index| Ok((model.linear_index_at(row_index)?, row_index)))
        .collect::<Result<BTreeMap<_, _>, mine_sdk::MineError>>()?;
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();

    for phase in &phase_plan.phases {
        let mut entries = schedule
            .entries()
            .iter()
            .filter(|entry| entry.phase_id() == Some(phase.phase_id.as_str()))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            continue;
        }
        entries.sort_by_key(|entry| entry.period_label().to_owned());

        let mut entry_index = 0usize;
        let mut remaining_tonnage = entries[entry_index].tonnage();
        let mut blocks = phase.block_indices.clone();
        blocks.sort_unstable();

        for linear_index in blocks {
            while remaining_tonnage <= 1e-9 && entry_index + 1 < entries.len() {
                entry_index += 1;
                remaining_tonnage = entries[entry_index].tonnage();
            }

            let row_index = *linear_index_to_row_index
                .get(&linear_index)
                .ok_or_else(|| {
                    mine_sdk::MineError::validation(format!(
                        "linear index `{linear_index}` is not materialized in the block model"
                    ))
                })?;
            let block_tonnage = tonnage_values[row_index];
            memberships
                .entry(entries[entry_index].period_label().to_owned())
                .or_default()
                .insert(linear_index);
            remaining_tonnage -= block_tonnage;
        }
    }

    Ok(memberships)
}

fn compact_end_to_end_summary(
    phase_plan: &PushbackPlan,
    schedule: &mine_sdk::LongTermSchedule,
    report: &LongTermScheduleEconomicsReport,
) -> MineRsEndToEndSummary {
    MineRsEndToEndSummary {
        phase_count: phase_plan.phase_count,
        total_block_count: phase_plan.total_block_count,
        schedule_period_count: report.periods.len(),
        schedule_entry_count: schedule.entries().len(),
        schedule_violation_count: schedule.violations().len(),
        total_tonnage: report.total_tonnage,
        total_cashflow: report.total_cashflow,
        npv: report.npv,
        periods: report
            .periods
            .iter()
            .map(|period| MineRsPeriodSummary {
                period_label: period.period_label.clone(),
                tonnage: period.tonnage,
                cashflow: period.cashflow,
                discounted_cashflow: period.discounted_cashflow,
            })
            .collect(),
    }
}
#[derive(Debug, Clone, Copy)]
struct FrontProgressionProfileContract {
    label: &'static str,
    cumulative_tonnage_targets: [f64; 3],
}
