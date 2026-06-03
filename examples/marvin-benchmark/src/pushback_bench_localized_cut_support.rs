use std::collections::{BTreeMap, BTreeSet};

use crate::minelib_scheduling_support::MarvinPreferredNestedShellFamilyContract;
use mine_sdk::{BlockModel, MineError, PhaseDesign, PushbackPlan, linear_to_ijk};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushbackBenchLocalizedCutPredecessorLinkPolicy {
    PredecessorLastCut,
    PredecessorFirstCut,
    AllPredecessorCuts,
}

impl Default for PushbackBenchLocalizedCutPredecessorLinkPolicy {
    fn default() -> Self {
        Self::PredecessorLastCut
    }
}

impl PushbackBenchLocalizedCutPredecessorLinkPolicy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PredecessorLastCut => "predecessor-last-cut",
            Self::PredecessorFirstCut => "predecessor-first-cut",
            Self::AllPredecessorCuts => "all-predecessor-cuts",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PushbackBenchLocalizedCutFrontProgression {
    UniformTonnageBalanced,
    FixedThreeFrontCumulativeTargets {
        label: &'static str,
        cumulative_tonnage_targets: [f64; 3],
    },
    PreferredThreeFrontCumulativeTargetsWithUniformFallback {
        label: &'static str,
        cumulative_tonnage_targets: [f64; 3],
    },
}

impl Default for PushbackBenchLocalizedCutFrontProgression {
    fn default() -> Self {
        Self::UniformTonnageBalanced
    }
}

impl PushbackBenchLocalizedCutFrontProgression {
    pub const fn label(self) -> &'static str {
        match self {
            Self::UniformTonnageBalanced => "uniform-tonnage-balanced",
            Self::FixedThreeFrontCumulativeTargets { label, .. } => label,
            Self::PreferredThreeFrontCumulativeTargetsWithUniformFallback { label, .. } => label,
        }
    }

    pub const fn contract_kind(self) -> &'static str {
        match self {
            Self::UniformTonnageBalanced => "tonnage-balanced-heuristic",
            Self::FixedThreeFrontCumulativeTargets { .. } => "fixed-three-front-cumulative-targets",
            Self::PreferredThreeFrontCumulativeTargetsWithUniformFallback { .. } => {
                "preferred-fixed-three-front-cumulative-targets-with-uniform-fallback"
            }
        }
    }

    pub const fn cumulative_tonnage_targets(self) -> Option<[f64; 3]> {
        match self {
            Self::UniformTonnageBalanced => None,
            Self::FixedThreeFrontCumulativeTargets {
                cumulative_tonnage_targets,
                ..
            } => Some(cumulative_tonnage_targets),
            Self::PreferredThreeFrontCumulativeTargetsWithUniformFallback {
                cumulative_tonnage_targets,
                ..
            } => Some(cumulative_tonnage_targets),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PushbackBenchLocalizedCutBuildConfig {
    pub max_front_count: usize,
    pub min_aspect_ratio: f64,
    pub min_dominant_span: usize,
    pub include_touching_neighbors: bool,
    pub max_local_predecessor_count: Option<usize>,
    pub predecessor_cut_link_policy: PushbackBenchLocalizedCutPredecessorLinkPolicy,
    pub front_progression: PushbackBenchLocalizedCutFrontProgression,
}

pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_FAMILY_LABEL: &str =
    "pushback-bench-localized-cut-phase";
pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL: &str =
    "pushback-bench-localized-mining-cuts";
pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL: &str =
    "front3-ar2.0-span2-n6";
pub const MARVIN_MR187_PAPERLIKE_CANDIDATE_ROLE: &str =
    "single benchmark-side paper-like candidate family";
pub const MARVIN_MR187_PROMOTED_FAMILY_IS_ACTIVE_CANDIDATE: bool = true;
pub const MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL: &str =
    "shape-gated-local-front-phase";
pub const MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE: &str =
    "local optimizer scaffold / non-comparable baseline";

pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL: &str =
    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL;

pub const MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_LABEL: &str = "uniform-33-67-100";

pub const MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION: PushbackBenchLocalizedCutFrontProgression =
    PushbackBenchLocalizedCutFrontProgression::PreferredThreeFrontCumulativeTargetsWithUniformFallback {
        label: MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_LABEL,
        cumulative_tonnage_targets: [1.0 / 3.0, 2.0 / 3.0, 1.0],
    };

pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG:
    PushbackBenchLocalizedCutBuildConfig = PushbackBenchLocalizedCutBuildConfig {
    max_front_count: 3,
    min_aspect_ratio: 2.0,
    min_dominant_span: 2,
    include_touching_neighbors: true,
    max_local_predecessor_count: Some(6),
    predecessor_cut_link_policy: PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
    front_progression: MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION,
};

pub fn format_pushback_bench_localized_cut_candidate_label(build_label: &str) -> String {
    format!("pushback-bench-localized-cut::{build_label}")
}

#[derive(Debug, Clone, Serialize)]
pub struct PushbackBenchLocalizedCutRefinementDiagnostics {
    pub base_phase_count: usize,
    pub refined_base_phase_count: usize,
    pub refined_single_component_phase_count: usize,
    pub total_cut_phase_count: usize,
    pub additional_phase_count: usize,
    pub max_cut_count_per_base_phase: usize,
    pub average_cut_count_per_base_phase: f64,
    pub realized_front_count_histogram: BTreeMap<usize, usize>,
    pub readiness_reason_histogram: BTreeMap<String, usize>,
    pub exact_three_front_candidate_count: usize,
    pub exact_three_front_failure_count: usize,
    pub exact_three_front_failure_realized_front_histogram: BTreeMap<usize, usize>,
    pub exact_three_front_failure_reason_histogram: BTreeMap<String, usize>,
    pub refined_base_phase_examples: Vec<String>,
    pub refined_single_component_phase_examples: Vec<String>,
}

pub struct PushbackBenchLocalizedCutBenchmarkArtifacts<TSchedulingProblem> {
    pub phase_plan: PushbackPlan,
    pub scheduling_problem: TSchedulingProblem,
}

pub struct PushbackBenchLocalizedCutBuildArtifacts<TSchedulingProblem> {
    pub benchmark: PushbackBenchLocalizedCutBenchmarkArtifacts<TSchedulingProblem>,
    pub phase_refinement_diagnostics: PushbackBenchLocalizedCutRefinementDiagnostics,
}

#[derive(Debug, Serialize)]
pub struct PushbackBenchLocalizedCutAccessPolicySummary {
    pub unit_family_label: String,
    pub promoted_build_label: String,
    pub release_inter_phase_inter_cut: PushbackBenchLocalizedCutReleaseBehaviorSummary,
    pub local_predecessor_filter: PushbackBenchLocalizedCutLocalPredecessorFilterSummary,
    pub intra_phase_progression: PushbackBenchLocalizedCutIntraPhaseProgressionSummary,
    pub ramp_access_contract: PushbackBenchLocalizedCutRampAccessContractSummary,
    pub working_width_contract: PushbackBenchLocalizedCutWorkingWidthContractSummary,
    pub lineage_bench_continuity_contract:
        PushbackBenchLocalizedCutLineageBenchContinuityContractSummary,
    pub complete_cut_design_contract: PushbackBenchLocalizedCutCompleteCutDesignContractSummary,
    pub bibliographic_gap_contract: Vec<PushbackBenchLocalizedCutBibliographicGapSummary>,
    pub missing_bibliographic_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutUnitFamilyTraceability {
    pub selected_block_provenance: PushbackBenchLocalizedCutSelectedBlockSourceTraceability,
    pub preferred_phase_plan_proxy: PushbackBenchLocalizedCutPreferredPhasePlanProxyTraceability,
    pub localized_cut_builder_provenance: PushbackBenchLocalizedCutLocalizedCutBuilderTraceability,
    pub derivation_summary: String,
    pub derivation_steps: Vec<PushbackBenchLocalizedCutUnitFamilyTraceabilityStep>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutSelectedBlockSourceTraceability {
    pub selected_block_source: String,
    pub selected_block_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutPreferredPhasePlanProxyTraceability {
    pub aggregation_strategy: String,
    pub preferred_nested_shell_factor_count: Option<usize>,
    pub preferred_nested_shell_realized_shell_count: Option<usize>,
    pub preferred_nested_shell_access_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutLocalizedCutBuilderTraceability {
    pub localized_cut_builder_label: String,
    pub localized_cut_builder_build_label: String,
    pub scaffold_unit_family_label: String,
    pub promoted_unit_family_label: String,
    pub front_progression_label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutUnitFamilyTraceabilityStep {
    pub step_id: String,
    pub stage_label: String,
    pub summary: String,
}

#[derive(Debug)]
pub struct MarvinMr187PromotedPushbackBenchLocalizedCutContractSurfaces {
    pub promoted_build_label: &'static str,
    pub unit_family_traceability: PushbackBenchLocalizedCutUnitFamilyTraceability,
    pub access_law: PushbackBenchLocalizedCutAccessPolicySummary,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutBibliographicGapSummary {
    pub gap_id: String,
    pub missing_term_label: String,
    pub contract_surface: String,
    pub current_status: String,
}

#[derive(Debug, Serialize)]
pub struct PushbackBenchLocalizedCutReleaseBehaviorSummary {
    pub release_mode: String,
    pub predecessor_cut_link_policy: String,
    pub proxy_status: String,
}

#[derive(Debug, Serialize)]
pub struct PushbackBenchLocalizedCutLocalPredecessorFilterSummary {
    pub localized_access_mode: String,
    pub predecessor_window_policy: String,
    pub filter_scope: String,
}

#[derive(Debug, Serialize)]
pub struct PushbackBenchLocalizedCutIntraPhaseProgressionSummary {
    pub intra_component_activation: String,
    pub front_progression: String,
    pub front_progression_contract_kind: String,
    pub front_progression_targets: Option<[f64; 3]>,
    pub front_progression_fallback: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutRampAccessContractSummary {
    pub proxy_status: String,
    pub predecessor_cut_link_policy: String,
    pub localized_access_mode: String,
    pub predecessor_window_policy: String,
    pub intra_component_activation: String,
    pub contract_surface: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutLineageBenchContinuityContractSummary {
    pub proxy_status: String,
    pub parent_phase_scope: String,
    pub cut_phase_id_lineage_rule: String,
    pub bench_continuity_mode: String,
    pub predecessor_cut_link_policy: String,
    pub intra_component_activation: String,
    pub contract_surface: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PushbackBenchLocalizedCutCompleteCutDesignContractSummary {
    pub proxy_status: String,
    pub predecessor_cut_link_policy: String,
    pub localized_access_mode: String,
    pub predecessor_window_policy: String,
    pub intra_component_activation: String,
    pub front_progression: String,
    pub ramp_access_contract_surface: String,
    pub working_width_contract_surface: String,
    pub lineage_bench_continuity_contract_surface: String,
    pub missing_design_terms: Vec<String>,
    pub contract_surface: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PushbackBenchLocalizedCutWorkingWidthContractSummary {
    pub proxy_status: String,
    pub min_aspect_ratio_threshold: f64,
    pub min_dominant_span_threshold: usize,
    pub geometry_eligible_component_count: usize,
    pub geometry_blocked_component_count: usize,
    pub geometry_blocked_reason_histogram: BTreeMap<String, usize>,
    pub preferred_three_front_candidate_count: usize,
    pub preferred_three_front_fallback_count: usize,
    pub preferred_three_front_fallback_reason_histogram: BTreeMap<String, usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalizedCutReadinessReason {
    BlockedLowDominantSpanAndAspectRatio,
    BlockedLowDominantSpan,
    BlockedLowAspectRatio,
    ConfigCappedBelowPaperLikeThreeFront,
    ExactThreeFrontInfeasibleInsufficientAxisStripes,
    ExactThreeFrontInfeasibleCollapsedTargetPartition,
    PaperLikeThreeFrontReady,
    RefinedBeyondPaperLikeThreeFront,
}

impl LocalizedCutReadinessReason {
    fn label(self) -> &'static str {
        match self {
            Self::BlockedLowDominantSpanAndAspectRatio => {
                "blocked-low-dominant-span-and-aspect-ratio"
            }
            Self::BlockedLowDominantSpan => "blocked-low-dominant-span",
            Self::BlockedLowAspectRatio => "blocked-low-aspect-ratio",
            Self::ConfigCappedBelowPaperLikeThreeFront => {
                "config-capped-below-paper-like-three-front"
            }
            Self::ExactThreeFrontInfeasibleInsufficientAxisStripes => {
                "exact-three-front-infeasible-insufficient-axis-stripes"
            }
            Self::ExactThreeFrontInfeasibleCollapsedTargetPartition => {
                "exact-three-front-infeasible-collapsed-target-partition"
            }
            Self::PaperLikeThreeFrontReady => "paper-like-three-front-ready",
            Self::RefinedBeyondPaperLikeThreeFront => "refined-beyond-paper-like-three-front",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactThreeFrontFailureReason {
    InsufficientAxisStripes,
    CollapsedTargetPartition,
    OverThreeFrontRealization,
}

impl ExactThreeFrontFailureReason {
    fn label(self) -> &'static str {
        match self {
            Self::InsufficientAxisStripes => "insufficient-axis-stripes",
            Self::CollapsedTargetPartition => "collapsed-target-partition",
            Self::OverThreeFrontRealization => "over-three-front-realization",
        }
    }
}

pub fn build_pushback_bench_localized_cut_benchmark_artifacts<TSchedulingProblem, F>(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    config: PushbackBenchLocalizedCutBuildConfig,
    build_scheduling_problem: F,
) -> Result<PushbackBenchLocalizedCutBuildArtifacts<TSchedulingProblem>, MineError>
where
    F: FnOnce(&PushbackPlan) -> Result<TSchedulingProblem, MineError>,
{
    let phase_plan = split_phase_plan_by_pushback_bench_localized_mining_cuts(
        model,
        base_phase_plan,
        tonnage_by_linear_index,
        config.max_front_count,
        config.min_aspect_ratio,
        config.min_dominant_span,
        config.include_touching_neighbors,
        config.max_local_predecessor_count,
        config.predecessor_cut_link_policy,
        config.front_progression,
    )?;
    let phase_refinement_diagnostics = build_pushback_bench_localized_cut_refinement_diagnostics(
        model,
        base_phase_plan,
        &phase_plan,
        tonnage_by_linear_index,
        config.max_front_count,
        config.min_aspect_ratio,
        config.min_dominant_span,
        config.front_progression,
    )?;
    let scheduling_problem = build_scheduling_problem(&phase_plan)?;
    Ok(PushbackBenchLocalizedCutBuildArtifacts {
        benchmark: PushbackBenchLocalizedCutBenchmarkArtifacts {
            phase_plan,
            scheduling_problem,
        },
        phase_refinement_diagnostics,
    })
}

pub fn summarize_pushback_bench_localized_cut_build_config(
    config: PushbackBenchLocalizedCutBuildConfig,
    diagnostics: &PushbackBenchLocalizedCutRefinementDiagnostics,
) -> PushbackBenchLocalizedCutAccessPolicySummary {
    let ramp_access_contract = build_pushback_bench_localized_cut_ramp_access_contract(config);
    let working_width_contract =
        build_pushback_bench_localized_cut_working_width_contract(config, diagnostics);
    let lineage_bench_continuity_contract =
        build_pushback_bench_localized_cut_lineage_bench_continuity_contract(config);
    let complete_cut_design_contract =
        build_pushback_bench_localized_cut_complete_cut_design_contract(
            config,
            &ramp_access_contract,
            &working_width_contract,
            &lineage_bench_continuity_contract,
        );
    let bibliographic_gap_contract = pushback_bench_localized_cut_bibliographic_gap_contract(
        &ramp_access_contract,
        &working_width_contract,
        &lineage_bench_continuity_contract,
        &complete_cut_design_contract,
    );
    PushbackBenchLocalizedCutAccessPolicySummary {
        unit_family_label: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_FAMILY_LABEL.to_owned(),
        promoted_build_label: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL
            .to_owned(),
        release_inter_phase_inter_cut: PushbackBenchLocalizedCutReleaseBehaviorSummary {
            release_mode:
                "candidate cut release stays benchmark-side and is gated by localized predecessor cuts selected from predecessor phases".to_owned(),
            predecessor_cut_link_policy: config.predecessor_cut_link_policy.label().to_owned(),
            proxy_status:
                "best-current-local-proxy; `predecessor-last-cut` is preserved as the current release proxy, but v9 evidence ties it with `all-predecessor-cuts`, so this is not bibliographic closure".to_owned(),
        },
        local_predecessor_filter: PushbackBenchLocalizedCutLocalPredecessorFilterSummary {
            localized_access_mode: localized_access_mode_label(config.include_touching_neighbors)
                .to_owned(),
            predecessor_window_policy: predecessor_window_policy_label(
                config.max_local_predecessor_count,
            ),
            filter_scope:
                "local predecessor candidates come from overlap-plus-adjacency filtering before the explicit predecessor window is applied".to_owned(),
        },
        intra_phase_progression: PushbackBenchLocalizedCutIntraPhaseProgressionSummary {
            intra_component_activation: localized_cut_activation_label().to_owned(),
            front_progression: config.front_progression.label().to_owned(),
            front_progression_contract_kind: config.front_progression.contract_kind().to_owned(),
            front_progression_targets: config.front_progression.cumulative_tonnage_targets(),
            front_progression_fallback: front_progression_fallback_label(config.front_progression),
        },
        ramp_access_contract,
        working_width_contract,
        lineage_bench_continuity_contract,
        complete_cut_design_contract,
        missing_bibliographic_terms: bibliographic_gap_contract
            .iter()
            .map(|gap| gap.missing_term_label.clone())
            .collect(),
        bibliographic_gap_contract,
    }
}

pub fn summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law(
    diagnostics: &PushbackBenchLocalizedCutRefinementDiagnostics,
) -> PushbackBenchLocalizedCutAccessPolicySummary {
    summarize_pushback_bench_localized_cut_build_config(
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        diagnostics,
    )
}

pub fn build_marvin_mr187_promoted_pushback_bench_localized_cut_contract_surfaces(
    selected_block_source: &str,
    selected_block_count: usize,
    preferred_phase_plan_aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
    promoted_unit_family_label: &str,
    diagnostics: &PushbackBenchLocalizedCutRefinementDiagnostics,
) -> MarvinMr187PromotedPushbackBenchLocalizedCutContractSurfaces {
    let promoted_build_label = MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL;
    let scaffold_unit_family_label = MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_UNIT_FAMILY_LABEL;
    let front_progression_label = MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_LABEL;

    MarvinMr187PromotedPushbackBenchLocalizedCutContractSurfaces {
        promoted_build_label,
        unit_family_traceability:
            build_promoted_pushback_bench_localized_cut_unit_family_traceability(
                selected_block_source,
                selected_block_count,
                preferred_phase_plan_aggregation_strategy,
                preferred_nested_shell_family_contract,
                scaffold_unit_family_label,
                promoted_unit_family_label,
                promoted_build_label,
                front_progression_label,
            ),
        access_law: summarize_marvin_mr187_promoted_pushback_bench_localized_cut_access_law(
            diagnostics,
        ),
    }
}

pub fn build_promoted_pushback_bench_localized_cut_unit_family_traceability(
    selected_block_source: &str,
    selected_block_count: usize,
    preferred_phase_plan_aggregation_strategy: &str,
    preferred_nested_shell_family_contract: Option<&MarvinPreferredNestedShellFamilyContract>,
    scaffold_unit_family_label: &str,
    promoted_unit_family_label: &str,
    promoted_build_label: &str,
    front_progression_label: &str,
) -> PushbackBenchLocalizedCutUnitFamilyTraceability {
    let selected_block_provenance = PushbackBenchLocalizedCutSelectedBlockSourceTraceability {
        selected_block_source: selected_block_source.to_owned(),
        selected_block_count: Some(selected_block_count),
    };
    let preferred_phase_plan_proxy = PushbackBenchLocalizedCutPreferredPhasePlanProxyTraceability {
        aggregation_strategy: preferred_phase_plan_aggregation_strategy.to_owned(),
        preferred_nested_shell_factor_count: preferred_nested_shell_family_contract
            .map(|contract| contract.revenue_factor_count),
        preferred_nested_shell_realized_shell_count: preferred_nested_shell_family_contract
            .and_then(|contract| contract.realized_shell_count),
        preferred_nested_shell_access_mode: preferred_nested_shell_family_contract
            .map(|contract| contract.shell_access_mode.label().to_owned()),
    };
    let localized_cut_builder_provenance =
        PushbackBenchLocalizedCutLocalizedCutBuilderTraceability {
            localized_cut_builder_label: MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL
                .to_owned(),
            localized_cut_builder_build_label: promoted_build_label.to_owned(),
            scaffold_unit_family_label: scaffold_unit_family_label.to_owned(),
            promoted_unit_family_label: promoted_unit_family_label.to_owned(),
            front_progression_label: front_progression_label.to_owned(),
        };
    let preferred_phase_summary = match preferred_nested_shell_family_contract {
        Some(contract) => format!(
            "The preferred `{}` proxy phase family keeps {} revenue factors on `{}` access and realizes {} shells when lifted from the selected block set.",
            preferred_phase_plan_aggregation_strategy,
            contract.revenue_factor_count,
            contract.shell_access_mode.label(),
            contract
                .realized_shell_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "an unreported number of".to_owned()),
        ),
        None => format!(
            "The preferred `{}` proxy phase family remains the benchmark-side aggregation bridge from selected blocks into schedulable phases.",
            preferred_phase_plan_aggregation_strategy,
        ),
    };
    let derivation_summary = format!(
        "The promoted `{}` family keeps `selected_block_source = \"{}\"` ({selected_block_count} selected blocks) as its block provenance, reuses the preferred `{}` proxy phase family, and then applies `{}` / build `{}` to refine scaffold `{}` into the promoted localized-cut units under `{}` progression.",
        promoted_unit_family_label,
        selected_block_source,
        preferred_phase_plan_aggregation_strategy,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL,
        promoted_build_label,
        scaffold_unit_family_label,
        front_progression_label,
    );
    PushbackBenchLocalizedCutUnitFamilyTraceability {
        selected_block_provenance,
        preferred_phase_plan_proxy,
        localized_cut_builder_provenance,
        derivation_summary,
        derivation_steps: vec![
            PushbackBenchLocalizedCutUnitFamilyTraceabilityStep {
                step_id: "selected-block-source".to_owned(),
                stage_label: "Selected block source".to_owned(),
                summary: format!(
                    "The promoted family only traces the {selected_block_count} blocks already admitted by `selected_block_source = \"{}\"`.",
                    selected_block_source,
                ),
            },
            PushbackBenchLocalizedCutUnitFamilyTraceabilityStep {
                step_id: "preferred-phase-plan-proxy".to_owned(),
                stage_label: "Preferred nested-shell × bench proxy".to_owned(),
                summary: preferred_phase_summary,
            },
            PushbackBenchLocalizedCutUnitFamilyTraceabilityStep {
                step_id: "localized-cut-builder".to_owned(),
                stage_label: "Localized-cut builder".to_owned(),
                summary: format!(
                    "Builder `{}` / build `{}` refines scaffold `{}` into promoted `{}` units while preserving benchmark-side localized-cut semantics.",
                    MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL,
                    promoted_build_label,
                    scaffold_unit_family_label,
                    promoted_unit_family_label,
                ),
            },
        ],
    }
}

pub fn format_promoted_pushback_bench_localized_cut_family_summary(
    traceability: &PushbackBenchLocalizedCutUnitFamilyTraceability,
    unit_family_label: &str,
    promoted_build_label: &str,
    local_optimizer_scaffold_unit_family_label: &str,
    access_law: &PushbackBenchLocalizedCutAccessPolicySummary,
) -> String {
    let promoted_family_status = format_promoted_lp_bz_family_status_summary(
        unit_family_label,
        promoted_build_label,
        local_optimizer_scaffold_unit_family_label,
    );
    format!(
        "{} {}; `cut_access_law` keeps `{}` / `{}` / `{}` plus benchmark-side ramp-access, working-width, lineage/bench-continuity and complete-cut-design proxy reporting with {} structured bibliographic gaps.",
        traceability.derivation_summary,
        promoted_family_status,
        access_law
            .release_inter_phase_inter_cut
            .predecessor_cut_link_policy,
        access_law.local_predecessor_filter.localized_access_mode,
        access_law.intra_phase_progression.front_progression,
        access_law.bibliographic_gap_contract.len(),
    )
}

pub fn format_promoted_lp_bz_family_status_summary(
    unit_family_label: &str,
    promoted_build_label: &str,
    local_optimizer_scaffold_unit_family_label: &str,
) -> String {
    format!(
        "`{}` + build `{}` stays the `{}` for Marvin benchmark-side LP/BZ reporting, while `{}` remains only the `{}`",
        unit_family_label,
        promoted_build_label,
        MARVIN_MR187_PAPERLIKE_CANDIDATE_ROLE,
        local_optimizer_scaffold_unit_family_label,
        MARVIN_MR187_LOCAL_OPTIMIZER_SCAFFOLD_ROLE,
    )
}

pub fn format_promoted_lp_bz_bibliographic_gap_summary(
    bibliographic_gap_contract_path: &str,
    bibliographic_gap_count: usize,
    bibliographic_gap_ids: &str,
) -> String {
    format!(
        "The audited `{bibliographic_gap_contract_path}` keeps {bibliographic_gap_count} explicit bibliographic deltas [{bibliographic_gap_ids}], so ramp access, working width, lineage / bench continuity and complete cut design now all remain benchmark-side partial proxies rather than literature-grade closures.",
    )
}

pub fn format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary(
    traceability: &PushbackBenchLocalizedCutUnitFamilyTraceability,
    promoted_phase_count: usize,
    scheduling_unit_count: usize,
) -> String {
    let shell_bridge_summary = match (
        traceability
            .preferred_phase_plan_proxy
            .preferred_nested_shell_factor_count,
        traceability
            .preferred_phase_plan_proxy
            .preferred_nested_shell_access_mode
            .as_deref(),
        traceability
            .preferred_phase_plan_proxy
            .preferred_nested_shell_realized_shell_count,
    ) {
        (Some(factor_count), Some(access_mode), Some(realized_shell_count)) => format!(
            "the bounded `{}` bridge keeps {factor_count} revenue factors on `{access_mode}` access and realizes {realized_shell_count} shells before localized-cut refinement",
            traceability.preferred_phase_plan_proxy.aggregation_strategy,
        ),
        (Some(factor_count), Some(access_mode), None) => format!(
            "the bounded `{}` bridge keeps {factor_count} revenue factors on `{access_mode}` access before localized-cut refinement",
            traceability.preferred_phase_plan_proxy.aggregation_strategy,
        ),
        _ => format!(
            "the preferred `{}` bridge remains the benchmark-side aggregation proxy before localized-cut refinement",
            traceability.preferred_phase_plan_proxy.aggregation_strategy,
        ),
    };
    let aggregation_jump_summary =
        format_promoted_pushback_bench_localized_cut_aggregation_jump_summary(
            traceability,
            promoted_phase_count,
            scheduling_unit_count,
        );
    format!(
        "Input/aggregation traceability now stays explicit across three benchmark-side layers: `selected_block_source = \"{}\"` seeds the admissible block set; {}; and builder `{}` / build `{}` refines scaffold `{}` into promoted `{}` units under `{}` progression. {} The route remains `exploratory-local` because the block provenance still starts from a staged benchmark-side selection and the intermediate shell family is still a reproducible proxy rather than a paper-reproduced pushback/mining-cut pipeline.",
        traceability.selected_block_provenance.selected_block_source,
        shell_bridge_summary,
        traceability
            .localized_cut_builder_provenance
            .localized_cut_builder_label,
        traceability
            .localized_cut_builder_provenance
            .localized_cut_builder_build_label,
        traceability
            .localized_cut_builder_provenance
            .scaffold_unit_family_label,
        traceability
            .localized_cut_builder_provenance
            .promoted_unit_family_label,
        traceability
            .localized_cut_builder_provenance
            .front_progression_label,
        aggregation_jump_summary,
    )
}

pub fn format_promoted_pushback_bench_localized_cut_aggregation_jump_summary(
    traceability: &PushbackBenchLocalizedCutUnitFamilyTraceability,
    promoted_phase_count: usize,
    scheduling_unit_count: usize,
) -> String {
    match traceability.selected_block_provenance.selected_block_count {
        Some(selected_block_count) => format!(
            "Quantitatively, {selected_block_count} selected blocks currently compress into {promoted_phase_count} promoted phases and {scheduling_unit_count} LP/BZ scheduling units."
        ),
        None => format!(
            "Quantitatively, the promoted family currently exposes {promoted_phase_count} promoted phases and {scheduling_unit_count} LP/BZ scheduling units, but `selected_block_count` is still missing from provenance."
        ),
    }
}

pub fn validate_promoted_pushback_bench_localized_cut_unit_family_traceability(
    summary: &PushbackBenchLocalizedCutUnitFamilyTraceability,
    selected_block_source: &str,
    selected_block_count: usize,
    preferred_phase_plan_aggregation_strategy: &str,
    scaffold_unit_family_label: &str,
    promoted_unit_family_label: &str,
    promoted_build_label: &str,
) -> Result<(), MineError> {
    if summary.selected_block_provenance.selected_block_source != selected_block_source {
        return Err(MineError::validation(format!(
            "Promoted localized-cut unit family traceability must stay aligned with `selected_block_source = \"{selected_block_source}\"`, received `{}`.",
            summary.selected_block_provenance.selected_block_source
        )));
    }
    if summary.selected_block_provenance.selected_block_count != Some(selected_block_count) {
        return Err(MineError::validation(format!(
            "Promoted localized-cut unit family traceability must stay aligned with selected block count `{selected_block_count}`, received `{:?}`.",
            summary.selected_block_provenance.selected_block_count
        )));
    }
    if summary.preferred_phase_plan_proxy.aggregation_strategy
        != preferred_phase_plan_aggregation_strategy
    {
        return Err(MineError::validation(format!(
            "Promoted localized-cut unit family traceability must stay aligned with preferred aggregation `{preferred_phase_plan_aggregation_strategy}`, received `{}`.",
            summary.preferred_phase_plan_proxy.aggregation_strategy
        )));
    }
    if summary
        .localized_cut_builder_provenance
        .localized_cut_builder_label
        != MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL
    {
        return Err(MineError::validation(format!(
            "Promoted localized-cut unit family traceability must stay aligned with builder `{}`, received `{}`.",
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_BUILDER_LABEL,
            summary
                .localized_cut_builder_provenance
                .localized_cut_builder_label
        )));
    }
    if summary
        .localized_cut_builder_provenance
        .localized_cut_builder_build_label
        != promoted_build_label
    {
        return Err(MineError::validation(format!(
            "Promoted localized-cut unit family traceability must stay aligned with promoted build `{promoted_build_label}`, received `{}`.",
            summary
                .localized_cut_builder_provenance
                .localized_cut_builder_build_label
        )));
    }
    if summary
        .localized_cut_builder_provenance
        .scaffold_unit_family_label
        != scaffold_unit_family_label
        || summary
            .localized_cut_builder_provenance
            .promoted_unit_family_label
            != promoted_unit_family_label
    {
        return Err(MineError::validation(
            "Promoted localized-cut unit family traceability drifted away from the scaffold/promoted family labels."
                .to_owned(),
        ));
    }
    if summary
        .preferred_phase_plan_proxy
        .preferred_nested_shell_factor_count
        .is_none()
        || summary
            .preferred_phase_plan_proxy
            .preferred_nested_shell_access_mode
            .is_none()
    {
        return Err(MineError::validation(
            "Promoted localized-cut unit family traceability must keep explicit nested-shell factor/access evidence."
                .to_owned(),
        ));
    }
    let expected_ids = [
        "selected-block-source",
        "preferred-phase-plan-proxy",
        "localized-cut-builder",
    ];
    if summary.derivation_steps.len() != expected_ids.len() {
        return Err(MineError::validation(format!(
            "Promoted localized-cut unit family traceability must keep {} derivation steps, received {}.",
            expected_ids.len(),
            summary.derivation_steps.len()
        )));
    }
    for expected_id in expected_ids {
        if !summary
            .derivation_steps
            .iter()
            .any(|step| step.step_id == expected_id && !step.summary.is_empty())
        {
            return Err(MineError::validation(format!(
                "Promoted localized-cut unit family traceability is missing derivation step `{expected_id}`."
            )));
        }
    }
    if !summary.derivation_summary.contains(selected_block_source)
        || !summary
            .derivation_summary
            .contains(preferred_phase_plan_aggregation_strategy)
        || !summary.derivation_summary.contains(promoted_build_label)
        || !summary
            .derivation_summary
            .contains(promoted_unit_family_label)
    {
        return Err(MineError::validation(
            "Promoted localized-cut unit family traceability summary drifted away from the active source/aggregation/build labels."
                .to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_promoted_pushback_bench_localized_cut_access_law_contract(
    summary: &PushbackBenchLocalizedCutAccessPolicySummary,
    promoted_build_label: &str,
) -> Result<(), MineError> {
    if summary.unit_family_label != MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_UNIT_FAMILY_LABEL {
        return Err(MineError::validation(format!(
            "Promoted localized-cut access law must stay on `pushback-bench-localized-cut-phase`, received `{}`.",
            summary.unit_family_label
        )));
    }
    if summary.promoted_build_label != promoted_build_label {
        return Err(MineError::validation(format!(
            "Promoted localized-cut access law must stay aligned with promoted build `{promoted_build_label}`, received `{}`.",
            summary.promoted_build_label
        )));
    }
    if summary.missing_bibliographic_terms.is_empty() {
        return Err(MineError::validation(
            "Promoted localized-cut access law must keep explicit missing bibliographic terms."
                .to_owned(),
        ));
    }
    if summary.bibliographic_gap_contract.is_empty() {
        return Err(MineError::validation(
            "Promoted localized-cut access law must keep a structured bibliographic gap contract."
                .to_owned(),
        ));
    }
    let structured_terms = summary
        .bibliographic_gap_contract
        .iter()
        .map(|gap| gap.missing_term_label.as_str())
        .collect::<Vec<_>>();
    let flat_terms = summary
        .missing_bibliographic_terms
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if structured_terms != flat_terms {
        return Err(MineError::validation(format!(
            "Promoted localized-cut access law structured bibliographic gap contract must stay aligned with flat missing terms; structured={structured_terms:?}, flat={flat_terms:?}."
        )));
    }
    if summary
        .ramp_access_contract
        .proxy_status
        .contains("benchmark-side partial proxy")
        == false
    {
        return Err(MineError::validation(format!(
            "Promoted localized-cut access law must expose a benchmark-side partial ramp-access proxy, received `{}`.",
            summary.ramp_access_contract.proxy_status
        )));
    }
    if summary.ramp_access_contract.predecessor_cut_link_policy
        != summary
            .release_inter_phase_inter_cut
            .predecessor_cut_link_policy
    {
        return Err(MineError::validation(format!(
            "Ramp-access proxy predecessor policy `{}` must stay aligned with release policy `{}`.",
            summary.ramp_access_contract.predecessor_cut_link_policy,
            summary
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy
        )));
    }
    if summary.ramp_access_contract.localized_access_mode
        != summary.local_predecessor_filter.localized_access_mode
    {
        return Err(MineError::validation(format!(
            "Ramp-access proxy localized access mode `{}` must stay aligned with local predecessor filter `{}`.",
            summary.ramp_access_contract.localized_access_mode,
            summary.local_predecessor_filter.localized_access_mode
        )));
    }
    if summary.ramp_access_contract.predecessor_window_policy
        != summary.local_predecessor_filter.predecessor_window_policy
    {
        return Err(MineError::validation(format!(
            "Ramp-access proxy predecessor window `{}` must stay aligned with local predecessor filter `{}`.",
            summary.ramp_access_contract.predecessor_window_policy,
            summary.local_predecessor_filter.predecessor_window_policy
        )));
    }
    if summary.ramp_access_contract.intra_component_activation
        != summary.intra_phase_progression.intra_component_activation
    {
        return Err(MineError::validation(format!(
            "Ramp-access proxy intra-component activation `{}` must stay aligned with intra-phase progression `{}`.",
            summary.ramp_access_contract.intra_component_activation,
            summary.intra_phase_progression.intra_component_activation
        )));
    }
    if summary
        .working_width_contract
        .geometry_eligible_component_count
        == 0
    {
        return Err(MineError::validation(
            "Promoted localized-cut access law must expose a non-empty working-width proxy over eligible components."
                .to_owned(),
        ));
    }
    if summary
        .lineage_bench_continuity_contract
        .proxy_status
        .contains("benchmark-side partial proxy")
        == false
    {
        return Err(MineError::validation(format!(
            "Promoted localized-cut access law must expose a benchmark-side partial lineage/bench-continuity proxy, received `{}`.",
            summary.lineage_bench_continuity_contract.proxy_status
        )));
    }
    if summary.lineage_bench_continuity_contract.parent_phase_scope
        != "each localized cut inherits exactly one shell×bench parent phase before localized predecessor rewiring"
    {
        return Err(MineError::validation(format!(
            "Lineage/bench-continuity parent phase scope drifted to `{}`.",
            summary.lineage_bench_continuity_contract.parent_phase_scope
        )));
    }
    if summary
        .lineage_bench_continuity_contract
        .cut_phase_id_lineage_rule
        .contains("::pbcut-c")
        == false
    {
        return Err(MineError::validation(
            "Lineage/bench-continuity contract must keep explicit `::pbcut-c` phase-id lineage."
                .to_owned(),
        ));
    }
    if summary
        .lineage_bench_continuity_contract
        .predecessor_cut_link_policy
        != summary
            .release_inter_phase_inter_cut
            .predecessor_cut_link_policy
    {
        return Err(MineError::validation(format!(
            "Lineage/bench-continuity predecessor policy `{}` must stay aligned with release policy `{}`.",
            summary
                .lineage_bench_continuity_contract
                .predecessor_cut_link_policy,
            summary
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy
        )));
    }
    if summary
        .lineage_bench_continuity_contract
        .intra_component_activation
        != summary.intra_phase_progression.intra_component_activation
    {
        return Err(MineError::validation(format!(
            "Lineage/bench-continuity intra-component activation `{}` must stay aligned with intra-phase progression `{}`.",
            summary
                .lineage_bench_continuity_contract
                .intra_component_activation,
            summary.intra_phase_progression.intra_component_activation
        )));
    }
    if summary
        .complete_cut_design_contract
        .proxy_status
        .contains("benchmark-side partial proxy")
        == false
    {
        return Err(MineError::validation(format!(
            "Promoted localized-cut access law must expose a benchmark-side partial complete cut-design proxy, received `{}`.",
            summary.complete_cut_design_contract.proxy_status
        )));
    }
    if summary
        .complete_cut_design_contract
        .predecessor_cut_link_policy
        != summary
            .release_inter_phase_inter_cut
            .predecessor_cut_link_policy
    {
        return Err(MineError::validation(format!(
            "Complete cut-design predecessor policy `{}` must stay aligned with release policy `{}`.",
            summary
                .complete_cut_design_contract
                .predecessor_cut_link_policy,
            summary
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy
        )));
    }
    if summary.complete_cut_design_contract.localized_access_mode
        != summary.local_predecessor_filter.localized_access_mode
    {
        return Err(MineError::validation(format!(
            "Complete cut-design localized access mode `{}` must stay aligned with local predecessor filter `{}`.",
            summary.complete_cut_design_contract.localized_access_mode,
            summary.local_predecessor_filter.localized_access_mode
        )));
    }
    if summary
        .complete_cut_design_contract
        .predecessor_window_policy
        != summary.local_predecessor_filter.predecessor_window_policy
    {
        return Err(MineError::validation(format!(
            "Complete cut-design predecessor window `{}` must stay aligned with local predecessor filter `{}`.",
            summary
                .complete_cut_design_contract
                .predecessor_window_policy,
            summary.local_predecessor_filter.predecessor_window_policy
        )));
    }
    if summary
        .complete_cut_design_contract
        .intra_component_activation
        != summary.intra_phase_progression.intra_component_activation
    {
        return Err(MineError::validation(format!(
            "Complete cut-design intra-component activation `{}` must stay aligned with intra-phase progression `{}`.",
            summary
                .complete_cut_design_contract
                .intra_component_activation,
            summary.intra_phase_progression.intra_component_activation
        )));
    }
    if summary.complete_cut_design_contract.front_progression
        != summary.intra_phase_progression.front_progression
    {
        return Err(MineError::validation(format!(
            "Complete cut-design front progression `{}` must stay aligned with intra-phase progression `{}`.",
            summary.complete_cut_design_contract.front_progression,
            summary.intra_phase_progression.front_progression
        )));
    }
    if summary
        .complete_cut_design_contract
        .ramp_access_contract_surface
        .contains("cut_access_law.ramp_access_contract")
        == false
    {
        return Err(MineError::validation(
            "Complete cut-design contract must cite `cut_access_law.ramp_access_contract`."
                .to_owned(),
        ));
    }
    if summary
        .complete_cut_design_contract
        .working_width_contract_surface
        .contains("cut_access_law.working_width_contract")
        == false
    {
        return Err(MineError::validation(
            "Complete cut-design contract must cite `cut_access_law.working_width_contract`."
                .to_owned(),
        ));
    }
    if summary
        .complete_cut_design_contract
        .lineage_bench_continuity_contract_surface
        .contains("cut_access_law.lineage_bench_continuity_contract")
        == false
    {
        return Err(MineError::validation(
            "Complete cut-design contract must cite `cut_access_law.lineage_bench_continuity_contract`."
                .to_owned(),
        ));
    }
    if summary
        .complete_cut_design_contract
        .missing_design_terms
        .is_empty()
    {
        return Err(MineError::validation(
            "Complete cut-design contract must keep explicit missing design terms.".to_owned(),
        ));
    }
    let Some(working_width_gap) = summary
        .bibliographic_gap_contract
        .iter()
        .find(|gap| gap.gap_id == "working-width-minimum-operating-width")
    else {
        return Err(MineError::validation(
            "Promoted localized-cut access law must keep a working-width bibliographic gap entry."
                .to_owned(),
        ));
    };
    if working_width_gap.current_status != "benchmark-side-partial-proxy" {
        return Err(MineError::validation(format!(
            "Working-width bibliographic gap must stay benchmark-side partial proxy, received `{}`.",
            working_width_gap.current_status
        )));
    }
    let Some(ramp_access_gap) = summary
        .bibliographic_gap_contract
        .iter()
        .find(|gap| gap.gap_id == "ramp-access-sequencing")
    else {
        return Err(MineError::validation(
            "Promoted localized-cut access law must keep a ramp-access bibliographic gap entry."
                .to_owned(),
        ));
    };
    if ramp_access_gap.current_status != "benchmark-side-partial-proxy" {
        return Err(MineError::validation(format!(
            "Ramp-access bibliographic gap must stay benchmark-side partial proxy, received `{}`.",
            ramp_access_gap.current_status
        )));
    }
    if ramp_access_gap
        .contract_surface
        .contains("ramp_access_contract")
        == false
    {
        return Err(MineError::validation(
            "Ramp-access bibliographic gap must cite `cut_access_law.ramp_access_contract`."
                .to_owned(),
        ));
    }
    let Some(lineage_gap) = summary
        .bibliographic_gap_contract
        .iter()
        .find(|gap| gap.gap_id == "cut-design-lineage-bench-continuity")
    else {
        return Err(MineError::validation(
            "Promoted localized-cut access law must keep a lineage / bench-continuity bibliographic gap entry."
                .to_owned(),
        ));
    };
    if lineage_gap.current_status != "benchmark-side-partial-proxy" {
        return Err(MineError::validation(format!(
            "Lineage / bench-continuity bibliographic gap must stay benchmark-side partial proxy, received `{}`.",
            lineage_gap.current_status
        )));
    }
    if lineage_gap
        .contract_surface
        .contains("lineage_bench_continuity_contract")
        == false
    {
        return Err(MineError::validation(
            "Lineage / bench-continuity bibliographic gap must cite `cut_access_law.lineage_bench_continuity_contract`."
                .to_owned(),
        ));
    }
    let Some(complete_cut_design_gap) = summary
        .bibliographic_gap_contract
        .iter()
        .find(|gap| gap.gap_id == "complete-cut-design-law")
    else {
        return Err(MineError::validation(
            "Promoted localized-cut access law must keep a complete cut-design bibliographic gap entry."
                .to_owned(),
        ));
    };
    if complete_cut_design_gap.current_status != "benchmark-side-partial-proxy" {
        return Err(MineError::validation(format!(
            "Complete cut-design bibliographic gap must stay benchmark-side partial proxy, received `{}`.",
            complete_cut_design_gap.current_status
        )));
    }
    if complete_cut_design_gap
        .contract_surface
        .contains("complete_cut_design_contract")
        == false
    {
        return Err(MineError::validation(
            "Complete cut-design bibliographic gap must cite `cut_access_law.complete_cut_design_contract`."
                .to_owned(),
        ));
    }
    Ok(())
}

fn build_pushback_bench_localized_cut_refinement_diagnostics(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    cut_phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    front_progression: PushbackBenchLocalizedCutFrontProgression,
) -> Result<PushbackBenchLocalizedCutRefinementDiagnostics, MineError> {
    let mut cut_count_by_base_phase = BTreeMap::<String, usize>::new();
    for phase in &cut_phase_plan.phases {
        let base_phase_id = phase
            .phase_id
            .rsplit_once("::pbcut")
            .map(|(phase_id, _)| phase_id)
            .unwrap_or(&phase.phase_id)
            .to_owned();
        *cut_count_by_base_phase.entry(base_phase_id).or_default() += 1;
    }

    let refined_base_phase_examples = base_phase_plan
        .phases
        .iter()
        .filter_map(|phase| {
            (cut_count_by_base_phase
                .get(&phase.phase_id)
                .copied()
                .unwrap_or(0)
                > 1)
            .then(|| phase.phase_id.clone())
        })
        .take(8)
        .collect::<Vec<_>>();
    let refined_single_component_phase_examples = base_phase_plan
        .phases
        .iter()
        .map(|phase| -> Result<Option<String>, MineError> {
            let cut_count = cut_count_by_base_phase
                .get(&phase.phase_id)
                .copied()
                .unwrap_or(0);
            if cut_count <= 1 {
                return Ok(None);
            }
            let component_count =
                split_block_indices_by_planar_connected_components(model, &phase.block_indices)?
                    .len();
            Ok((component_count == 1).then(|| phase.phase_id.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let base_phase_count = base_phase_plan.phase_count;
    let refined_base_phase_count = cut_count_by_base_phase
        .values()
        .filter(|&&cut_count| cut_count > 1)
        .count();
    let component_diagnostics =
        collect_pushback_bench_localized_cut_component_refinement_diagnostics(
            model,
            base_phase_plan,
            tonnage_by_linear_index,
            max_front_count,
            min_aspect_ratio,
            min_dominant_span,
            front_progression,
        )?;
    let total_cut_phase_count = cut_phase_plan.phase_count;
    let max_cut_count_per_base_phase = cut_count_by_base_phase.values().copied().max().unwrap_or(0);
    let average_cut_count_per_base_phase = if base_phase_count == 0 {
        0.0
    } else {
        total_cut_phase_count as f64 / base_phase_count as f64
    };
    let mut realized_front_count_histogram = BTreeMap::<usize, usize>::new();
    let mut readiness_reason_histogram = BTreeMap::<String, usize>::new();
    let mut exact_three_front_candidate_count = 0usize;
    let mut exact_three_front_failure_count = 0usize;
    let mut exact_three_front_failure_realized_front_histogram = BTreeMap::<usize, usize>::new();
    let mut exact_three_front_failure_reason_histogram = BTreeMap::<String, usize>::new();
    for component_diagnostic in component_diagnostics {
        *realized_front_count_histogram
            .entry(component_diagnostic.realized_front_count)
            .or_default() += 1;
        *readiness_reason_histogram
            .entry(component_diagnostic.readiness_reason.label().to_owned())
            .or_default() += 1;
        if component_diagnostic.exact_three_front_candidate {
            exact_three_front_candidate_count += 1;
            if component_diagnostic.realized_front_count != 3 {
                exact_three_front_failure_count += 1;
                *exact_three_front_failure_realized_front_histogram
                    .entry(component_diagnostic.realized_front_count)
                    .or_default() += 1;
                if let Some(failure_reason) = component_diagnostic.exact_three_front_failure_reason
                {
                    *exact_three_front_failure_reason_histogram
                        .entry(failure_reason.label().to_owned())
                        .or_default() += 1;
                }
            }
        }
    }

    Ok(PushbackBenchLocalizedCutRefinementDiagnostics {
        base_phase_count,
        refined_base_phase_count,
        refined_single_component_phase_count: refined_single_component_phase_examples.len(),
        total_cut_phase_count,
        additional_phase_count: total_cut_phase_count.saturating_sub(base_phase_count),
        max_cut_count_per_base_phase,
        average_cut_count_per_base_phase,
        realized_front_count_histogram,
        readiness_reason_histogram,
        exact_three_front_candidate_count,
        exact_three_front_failure_count,
        exact_three_front_failure_realized_front_histogram,
        exact_three_front_failure_reason_histogram,
        refined_base_phase_examples,
        refined_single_component_phase_examples: refined_single_component_phase_examples
            .into_iter()
            .take(8)
            .collect(),
    })
}

#[derive(Debug)]
struct LocalizedCutComponentRefinementDiagnostic {
    realized_front_count: usize,
    readiness_reason: LocalizedCutReadinessReason,
    exact_three_front_candidate: bool,
    exact_three_front_failure_reason: Option<ExactThreeFrontFailureReason>,
}

fn collect_pushback_bench_localized_cut_component_refinement_diagnostics(
    model: &BlockModel,
    base_phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    front_progression: PushbackBenchLocalizedCutFrontProgression,
) -> Result<Vec<LocalizedCutComponentRefinementDiagnostic>, MineError> {
    let mut diagnostics = Vec::new();
    for phase in &base_phase_plan.phases {
        for component_block_indices in
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?
        {
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
            let split_by_i = span_i >= span_j;
            let distinct_axis_coordinate_count = component_block_indices
                .iter()
                .map(|&linear_index| {
                    let grid_index = linear_to_ijk(model.grid(), linear_index)?;
                    Ok(if split_by_i {
                        grid_index.i()
                    } else {
                        grid_index.j()
                    })
                })
                .collect::<Result<BTreeSet<_>, MineError>>()?
                .len();
            let max_realizable_front_count = distinct_axis_coordinate_count.min(3);
            let meets_dominant_span = dominant_span >= min_dominant_span;
            let meets_aspect_ratio = aspect_ratio >= min_aspect_ratio;
            let candidate_fronts = split_component_by_dominant_axis_stripes(
                model,
                &component_block_indices,
                tonnage_by_linear_index,
                max_front_count,
                front_progression,
            )?;
            let realized_front_count = if meets_dominant_span && meets_aspect_ratio {
                candidate_fronts
                    .iter()
                    .filter(|front| !front.is_empty())
                    .count()
                    .max(1)
            } else {
                1
            };
            let readiness_reason = if !meets_dominant_span && !meets_aspect_ratio {
                LocalizedCutReadinessReason::BlockedLowDominantSpanAndAspectRatio
            } else if !meets_dominant_span {
                LocalizedCutReadinessReason::BlockedLowDominantSpan
            } else if !meets_aspect_ratio {
                LocalizedCutReadinessReason::BlockedLowAspectRatio
            } else if max_front_count < 3 {
                LocalizedCutReadinessReason::ConfigCappedBelowPaperLikeThreeFront
            } else if max_realizable_front_count < 3 {
                LocalizedCutReadinessReason::ExactThreeFrontInfeasibleInsufficientAxisStripes
            } else if realized_front_count < 3 {
                LocalizedCutReadinessReason::ExactThreeFrontInfeasibleCollapsedTargetPartition
            } else if realized_front_count == 3 {
                LocalizedCutReadinessReason::PaperLikeThreeFrontReady
            } else {
                LocalizedCutReadinessReason::RefinedBeyondPaperLikeThreeFront
            };
            let exact_three_front_candidate =
                max_front_count >= 3 && meets_dominant_span && meets_aspect_ratio;
            let exact_three_front_failure_reason =
                if exact_three_front_candidate && realized_front_count != 3 {
                    Some(if max_realizable_front_count < 3 {
                        ExactThreeFrontFailureReason::InsufficientAxisStripes
                    } else if realized_front_count < 3 {
                        ExactThreeFrontFailureReason::CollapsedTargetPartition
                    } else {
                        ExactThreeFrontFailureReason::OverThreeFrontRealization
                    })
                } else {
                    None
                };
            diagnostics.push(LocalizedCutComponentRefinementDiagnostic {
                realized_front_count,
                readiness_reason,
                exact_three_front_candidate,
                exact_three_front_failure_reason,
            });
        }
    }
    Ok(diagnostics)
}

fn split_phase_plan_by_pushback_bench_localized_mining_cuts(
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_front_count: usize,
    min_aspect_ratio: f64,
    min_dominant_span: usize,
    include_touching_neighbors: bool,
    max_local_predecessor_count: Option<usize>,
    predecessor_cut_link_policy: PushbackBenchLocalizedCutPredecessorLinkPolicy,
    front_progression: PushbackBenchLocalizedCutFrontProgression,
) -> Result<PushbackPlan, MineError> {
    let mut cut_descriptors_by_phase = BTreeMap::<String, Vec<PlanarComponentDescriptor>>::new();
    let mut cut_phases = Vec::<PhaseDesign>::new();

    for phase in &phase_plan.phases {
        let components =
            split_block_indices_by_planar_connected_components(model, &phase.block_indices)?;
        let mut phase_cut_descriptors = Vec::<PlanarComponentDescriptor>::new();

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
                front_progression,
            )?;
            let should_split = dominant_span >= min_dominant_span
                && aspect_ratio >= min_aspect_ratio
                && candidate_fronts.len() > 1;
            let cut_fronts = if should_split {
                candidate_fronts
            } else {
                vec![component_block_indices]
            };
            let mut previous_cut_phase_id = None::<String>;

            for (front_index, mut block_indices) in cut_fronts.into_iter().enumerate() {
                if block_indices.is_empty() {
                    continue;
                }
                block_indices.sort_unstable();
                let front_bounds =
                    PlanarComponentBounds::from_block_indices(model, &block_indices)?;
                let cut_phase_id = if should_split {
                    format!(
                        "{}::pbcut-c{:02}s{:02}",
                        phase.phase_id,
                        component_index + 1,
                        front_index + 1
                    )
                } else {
                    format!("{}::pbcut-c{:02}", phase.phase_id, component_index + 1)
                };
                let mut predecessor_phase_ids = phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|predecessor_phase_id| {
                        cut_descriptors_by_phase
                            .get(predecessor_phase_id)
                            .ok_or_else(|| MineError::Planning {
                                message: format!(
                                    "pushback bench-localized cut split is missing predecessor cuts for phase `{predecessor_phase_id}`"
                                ),
                            })
                            .and_then(|descriptors| {
                                select_predecessor_cut_phase_ids(
                                    predecessor_phase_id,
                                    predecessor_cut_link_policy,
                                    &front_bounds,
                                    descriptors,
                                    include_touching_neighbors,
                                    max_local_predecessor_count,
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if let Some(previous_cut_phase_id) = &previous_cut_phase_id {
                    predecessor_phase_ids.push(previous_cut_phase_id.clone());
                }
                predecessor_phase_ids = predecessor_phase_ids
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();

                phase_cut_descriptors.push(PlanarComponentDescriptor {
                    phase_id: cut_phase_id.clone(),
                    bounds: front_bounds,
                });
                cut_phases.push(PhaseDesign {
                    phase_id: cut_phase_id.clone(),
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
                previous_cut_phase_id = Some(cut_phase_id);
            }
        }

        if phase_cut_descriptors.is_empty() {
            return Err(MineError::Planning {
                message: format!(
                    "pushback bench-localized cut split produced no cuts for phase `{}`",
                    phase.phase_id
                ),
            });
        }
        cut_descriptors_by_phase.insert(phase.phase_id.clone(), phase_cut_descriptors);
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
            .chain(std::iter::once(format!(
                "Pushback bench-localized mining cuts split any elongated shell×bench component (dominant-span >= {min_dominant_span}, aspect ratio >= {min_aspect_ratio:.1}) into up to {max_front_count} dominant-axis cuts, then localize predecessor links with `{}` filtering and `{}` predecessor-window policy.",
                localized_access_mode_label(include_touching_neighbors),
                match max_local_predecessor_count {
                    Some(max_count) => format!("closest-N={max_count}"),
                    None => "unbounded predecessor fan-in".to_owned(),
                }
            ) + &format!(
                ", `{}` predecessor-cut link policy, `{}` front progression, and fixed `{}` intra-component activation.",
                predecessor_cut_link_policy.label(),
                front_progression.label(),
                localized_cut_activation_label(),
            )))
            .collect(),
    })
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

fn split_block_indices_by_planar_connected_components(
    model: &BlockModel,
    block_indices: &[usize],
) -> Result<Vec<Vec<usize>>, MineError> {
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
    fn from_block_indices(model: &BlockModel, block_indices: &[usize]) -> Result<Self, MineError> {
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

fn select_predecessor_cut_phase_ids(
    predecessor_phase_id: &str,
    predecessor_cut_link_policy: PushbackBenchLocalizedCutPredecessorLinkPolicy,
    current_bounds: &PlanarComponentBounds,
    predecessor_components: &[PlanarComponentDescriptor],
    include_touching_neighbors: bool,
    max_local_predecessor_count: Option<usize>,
) -> Result<Vec<String>, MineError> {
    let localized_predecessor_phase_ids = select_localized_planar_predecessors(
        current_bounds,
        predecessor_components,
        include_touching_neighbors,
        max_local_predecessor_count,
    );
    let missing_predecessor_error = || MineError::Planning {
        message: format!(
            "pushback bench-localized cut split is missing predecessor cuts for phase `{predecessor_phase_id}`"
        ),
    };
    match predecessor_cut_link_policy {
        PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut => Ok(vec![
            localized_predecessor_phase_ids
                .last()
                .cloned()
                .ok_or_else(missing_predecessor_error)?,
        ]),
        PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorFirstCut => Ok(vec![
            localized_predecessor_phase_ids
                .first()
                .cloned()
                .ok_or_else(missing_predecessor_error)?,
        ]),
        PushbackBenchLocalizedCutPredecessorLinkPolicy::AllPredecessorCuts => {
            Ok(localized_predecessor_phase_ids)
        }
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

fn localized_cut_activation_label() -> &'static str {
    "sequential-previous-cut"
}

fn predecessor_window_policy_label(max_local_predecessor_count: Option<usize>) -> String {
    match max_local_predecessor_count {
        Some(max_count) => format!("closest-N={max_count}"),
        None => "unbounded predecessor fan-in".to_owned(),
    }
}

fn front_progression_fallback_label(
    front_progression: PushbackBenchLocalizedCutFrontProgression,
) -> Option<String> {
    match front_progression {
        PushbackBenchLocalizedCutFrontProgression::PreferredThreeFrontCumulativeTargetsWithUniformFallback { .. } => {
            Some("uniform-tonnage-balanced when realized front count differs from 3".to_owned())
        }
        _ => None,
    }
}

fn build_pushback_bench_localized_cut_working_width_contract(
    config: PushbackBenchLocalizedCutBuildConfig,
    diagnostics: &PushbackBenchLocalizedCutRefinementDiagnostics,
) -> PushbackBenchLocalizedCutWorkingWidthContractSummary {
    let geometry_blocked_reason_histogram = diagnostics
        .readiness_reason_histogram
        .iter()
        .filter(|(reason, _)| reason.starts_with("blocked-"))
        .map(|(reason, count)| (reason.clone(), *count))
        .collect::<BTreeMap<_, _>>();
    let geometry_blocked_component_count = geometry_blocked_reason_histogram.values().sum();
    let total_component_count = diagnostics
        .readiness_reason_histogram
        .values()
        .sum::<usize>();
    let geometry_eligible_component_count =
        total_component_count.saturating_sub(geometry_blocked_component_count);
    PushbackBenchLocalizedCutWorkingWidthContractSummary {
        proxy_status: format!(
            "benchmark-side partial proxy using `min_aspect_ratio >= {:.1}` and `min_dominant_span >= {}` gates plus preferred-three-front fallback diagnostics; this audits geometric operating-width intent without claiming a literature-grade working-width law",
            config.min_aspect_ratio, config.min_dominant_span
        ),
        min_aspect_ratio_threshold: config.min_aspect_ratio,
        min_dominant_span_threshold: config.min_dominant_span,
        geometry_eligible_component_count,
        geometry_blocked_component_count,
        geometry_blocked_reason_histogram,
        preferred_three_front_candidate_count: diagnostics.exact_three_front_candidate_count,
        preferred_three_front_fallback_count: diagnostics.exact_three_front_failure_count,
        preferred_three_front_fallback_reason_histogram: diagnostics
            .exact_three_front_failure_reason_histogram
            .clone(),
    }
}

fn format_pushback_bench_localized_cut_working_width_contract_surface(
    working_width_contract: &PushbackBenchLocalizedCutWorkingWidthContractSummary,
) -> String {
    format!(
        "`cut_access_law.working_width_contract` now serializes a benchmark-side proxy with `min_aspect_ratio >= {:.1}`, `min_dominant_span >= {}`, {} geometry-eligible components, {} geometry-blocked components and {} preferred-three-front fallbacks; it still does not implement a literature-grade operating-width law.",
        working_width_contract.min_aspect_ratio_threshold,
        working_width_contract.min_dominant_span_threshold,
        working_width_contract.geometry_eligible_component_count,
        working_width_contract.geometry_blocked_component_count,
        working_width_contract.preferred_three_front_fallback_count,
    )
}

fn build_pushback_bench_localized_cut_ramp_access_contract(
    config: PushbackBenchLocalizedCutBuildConfig,
) -> PushbackBenchLocalizedCutRampAccessContractSummary {
    let predecessor_cut_link_policy = config.predecessor_cut_link_policy.label().to_owned();
    let localized_access_mode = localized_access_mode_label(config.include_touching_neighbors);
    let predecessor_window_policy =
        predecessor_window_policy_label(config.max_local_predecessor_count);
    let intra_component_activation = localized_cut_activation_label().to_owned();
    PushbackBenchLocalizedCutRampAccessContractSummary {
        proxy_status: "benchmark-side partial proxy rooted in existing predecessor release and localized predecessor filtering; this audits ramp-access sequencing intent without claiming geometric ramp design".to_owned(),
        predecessor_cut_link_policy: predecessor_cut_link_policy.clone(),
        localized_access_mode: localized_access_mode.to_owned(),
        predecessor_window_policy: predecessor_window_policy.clone(),
        intra_component_activation: intra_component_activation.clone(),
        contract_surface: format!(
            "`cut_access_law.ramp_access_contract` serializes the current benchmark-side ramp-access proxy by combining `{predecessor_cut_link_policy}` release, `{localized_access_mode}` predecessor filtering, `{predecessor_window_policy}` windowing and `{intra_component_activation}` cut activation; it still omits geometric ramp layout, elevation continuity and literature-grade ramp sequencing."
        ),
    }
}

fn build_pushback_bench_localized_cut_complete_cut_design_contract(
    config: PushbackBenchLocalizedCutBuildConfig,
    ramp_access_contract: &PushbackBenchLocalizedCutRampAccessContractSummary,
    working_width_contract: &PushbackBenchLocalizedCutWorkingWidthContractSummary,
    lineage_bench_continuity_contract: &PushbackBenchLocalizedCutLineageBenchContinuityContractSummary,
) -> PushbackBenchLocalizedCutCompleteCutDesignContractSummary {
    let predecessor_cut_link_policy = config.predecessor_cut_link_policy.label().to_owned();
    let localized_access_mode = localized_access_mode_label(config.include_touching_neighbors);
    let predecessor_window_policy =
        predecessor_window_policy_label(config.max_local_predecessor_count);
    let intra_component_activation = localized_cut_activation_label().to_owned();
    let front_progression = config.front_progression.label().to_owned();
    let working_width_contract_surface =
        format_pushback_bench_localized_cut_working_width_contract_surface(working_width_contract);
    let missing_design_terms = vec![
        "geometric ramp layout and elevation continuity".to_owned(),
        "closure-grade coupling between access sequencing, working width and bench-by-bench cut release"
            .to_owned(),
        "literature-grade end-to-end pushback/cut/bench design optimization".to_owned(),
    ];
    PushbackBenchLocalizedCutCompleteCutDesignContractSummary {
        proxy_status: "benchmark-side partial proxy composed from audited release, predecessor filtering, intra-phase progression, ramp-access, working-width and lineage layers; this reduces the complete cut-design gap without claiming literature-grade closure".to_owned(),
        predecessor_cut_link_policy: predecessor_cut_link_policy.clone(),
        localized_access_mode: localized_access_mode.to_owned(),
        predecessor_window_policy: predecessor_window_policy.clone(),
        intra_component_activation: intra_component_activation.clone(),
        front_progression: front_progression.clone(),
        ramp_access_contract_surface: ramp_access_contract.contract_surface.clone(),
        working_width_contract_surface: working_width_contract_surface.clone(),
        lineage_bench_continuity_contract_surface: lineage_bench_continuity_contract
            .contract_surface
            .clone(),
        missing_design_terms: missing_design_terms.clone(),
        contract_surface: format!(
            "`cut_access_law.complete_cut_design_contract` composes `{predecessor_cut_link_policy}` release, `{localized_access_mode}` predecessor filtering, `{predecessor_window_policy}` windowing, `{intra_component_activation}` cut activation, `{front_progression}` front progression, `cut_access_law.ramp_access_contract`, `cut_access_law.working_width_contract` and `cut_access_law.lineage_bench_continuity_contract` into an explicit benchmark-side partial proxy for complete cut design; it still omits {}.",
            missing_design_terms.join(", ")
        ),
    }
}

fn build_pushback_bench_localized_cut_lineage_bench_continuity_contract(
    config: PushbackBenchLocalizedCutBuildConfig,
) -> PushbackBenchLocalizedCutLineageBenchContinuityContractSummary {
    let predecessor_cut_link_policy = config.predecessor_cut_link_policy.label().to_owned();
    let intra_component_activation = localized_cut_activation_label().to_owned();
    PushbackBenchLocalizedCutLineageBenchContinuityContractSummary {
        proxy_status: "benchmark-side partial proxy rooted in shell×bench parent inheritance and deterministic `::pbcut-c*` suffix lineage; this audits pushback→cut→bench continuity without claiming a literature-grade complete cut-design law".to_owned(),
        parent_phase_scope:
            "each localized cut inherits exactly one shell×bench parent phase before localized predecessor rewiring"
                .to_owned(),
        cut_phase_id_lineage_rule:
            "cut phase ids retain the parent shell×bench phase id and append deterministic `::pbcut-cNN` / `::pbcut-cNNsNN` suffixes"
                .to_owned(),
        bench_continuity_mode:
            "cut phases copy pushback_index, shell_index, revenue_factor and bench from the parent phase, so each cut remains benchmark-side inside one pushback/bench lineage"
                .to_owned(),
        predecessor_cut_link_policy: predecessor_cut_link_policy.clone(),
        intra_component_activation: intra_component_activation.clone(),
        contract_surface: format!(
            "`cut_access_law.lineage_bench_continuity_contract` serializes a benchmark-side pushback→cut→bench continuity proxy: each cut inherits one shell×bench parent phase, preserves deterministic `::pbcut-c*` lineage in `phase_id`, keeps the parent's pushback/shell/bench coordinates, and combines `{predecessor_cut_link_policy}` inter-phase cut linkage with fixed `{intra_component_activation}` intra-phase chaining. This is explicit partial progress toward cut design, but it is not yet a literature-grade complete-cut-design law."
        ),
    }
}

fn pushback_bench_localized_cut_bibliographic_gap_contract(
    ramp_access_contract: &PushbackBenchLocalizedCutRampAccessContractSummary,
    working_width_contract: &PushbackBenchLocalizedCutWorkingWidthContractSummary,
    lineage_bench_continuity_contract: &PushbackBenchLocalizedCutLineageBenchContinuityContractSummary,
    complete_cut_design_contract: &PushbackBenchLocalizedCutCompleteCutDesignContractSummary,
) -> Vec<PushbackBenchLocalizedCutBibliographicGapSummary> {
    vec![
        PushbackBenchLocalizedCutBibliographicGapSummary {
            gap_id: "ramp-access-sequencing".to_owned(),
            missing_term_label: "ramps / ramp access sequencing".to_owned(),
            contract_surface: format!("{}", ramp_access_contract.contract_surface),
            current_status: "benchmark-side-partial-proxy".to_owned(),
        },
        PushbackBenchLocalizedCutBibliographicGapSummary {
            gap_id: "working-width-minimum-operating-width".to_owned(),
            missing_term_label: "working width / minimum operating width".to_owned(),
            contract_surface: format_pushback_bench_localized_cut_working_width_contract_surface(
                working_width_contract,
            ),
            current_status: "benchmark-side-partial-proxy".to_owned(),
        },
        PushbackBenchLocalizedCutBibliographicGapSummary {
            gap_id: "cut-design-lineage-bench-continuity".to_owned(),
            missing_term_label: "cut-design lineage / bench continuity".to_owned(),
            contract_surface: lineage_bench_continuity_contract.contract_surface.clone(),
            current_status: "benchmark-side-partial-proxy".to_owned(),
        },
        PushbackBenchLocalizedCutBibliographicGapSummary {
            gap_id: "complete-cut-design-law".to_owned(),
            missing_term_label: "complete cut design law across pushbacks, cuts and benches"
                .to_owned(),
            contract_surface: complete_cut_design_contract.contract_surface.clone(),
            current_status: "benchmark-side-partial-proxy".to_owned(),
        },
    ]
}

fn split_component_by_dominant_axis_stripes(
    model: &BlockModel,
    block_indices: &[usize],
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    max_stripe_count: usize,
    front_progression: PushbackBenchLocalizedCutFrontProgression,
) -> Result<Vec<Vec<usize>>, MineError> {
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
        .collect::<Result<Vec<_>, MineError>>()?;
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
        .collect::<Result<BTreeSet<_>, MineError>>()?;
    let stripe_count = max_stripe_count
        .min(distinct_axis_coordinates.len().max(1))
        .min(ordered_block_indices.len().max(1));

    match front_progression {
        PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced => {
            Ok(partition_block_indices_by_tonnage(
                &ordered_block_indices,
                tonnage_by_linear_index,
                stripe_count,
            ))
        }
        PushbackBenchLocalizedCutFrontProgression::FixedThreeFrontCumulativeTargets {
            cumulative_tonnage_targets,
            ..
        } => {
            if stripe_count != 3 {
                return Err(MineError::Planning {
                    message: format!(
                        "pushback bench-localized cut front progression `{}` requires exactly 3 realized fronts but component realized {stripe_count}",
                        front_progression.label()
                    ),
                });
            }
            partition_block_indices_by_cumulative_tonnage_targets(
                &ordered_block_indices,
                tonnage_by_linear_index,
                &cumulative_tonnage_targets,
            )
        }
        PushbackBenchLocalizedCutFrontProgression::PreferredThreeFrontCumulativeTargetsWithUniformFallback {
            cumulative_tonnage_targets,
            ..
        } => {
            if stripe_count == 3 {
                partition_block_indices_by_cumulative_tonnage_targets(
                    &ordered_block_indices,
                    tonnage_by_linear_index,
                    &cumulative_tonnage_targets,
                )
            } else {
                Ok(partition_block_indices_by_tonnage(
                    &ordered_block_indices,
                    tonnage_by_linear_index,
                    stripe_count,
                ))
            }
        }
    }
}

fn partition_block_indices_by_cumulative_tonnage_targets(
    block_indices: &[usize],
    tonnage_by_linear_index: &BTreeMap<usize, f64>,
    cumulative_tonnage_targets: &[f64],
) -> Result<Vec<Vec<usize>>, MineError> {
    if block_indices.is_empty() {
        return Ok(Vec::new());
    }
    if cumulative_tonnage_targets.is_empty() {
        return Err(MineError::Planning {
            message: "pushback bench-localized cut custom progression requires at least one cumulative target".to_owned(),
        });
    }
    if cumulative_tonnage_targets
        .iter()
        .any(|target| !target.is_finite() || *target <= 0.0 || *target > 1.0 + 1.0e-9)
    {
        return Err(MineError::Planning {
            message:
                "pushback bench-localized cut progression targets must be finite values in (0, 1]"
                    .to_owned(),
        });
    }
    if cumulative_tonnage_targets
        .windows(2)
        .any(|window| window[1] <= window[0] + 1.0e-9)
    {
        return Err(MineError::Planning {
            message: "pushback bench-localized cut progression targets must be strictly increasing"
                .to_owned(),
        });
    }
    if (cumulative_tonnage_targets[cumulative_tonnage_targets.len() - 1] - 1.0).abs() > 1.0e-6 {
        return Err(MineError::Planning {
            message: "pushback bench-localized cut progression targets must end at 1.0".to_owned(),
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

#[cfg(test)]
mod tests {
    use super::{
        MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION,
        MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_LABEL,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        MarvinMr187PromotedPushbackBenchLocalizedCutContractSurfaces,
        PushbackBenchLocalizedCutBuildConfig, PushbackBenchLocalizedCutFrontProgression,
        PushbackBenchLocalizedCutPredecessorLinkPolicy,
        build_marvin_mr187_promoted_pushback_bench_localized_cut_contract_surfaces,
        build_promoted_pushback_bench_localized_cut_unit_family_traceability,
        build_pushback_bench_localized_cut_benchmark_artifacts,
        format_promoted_pushback_bench_localized_cut_family_summary,
        split_phase_plan_by_pushback_bench_localized_mining_cuts,
        summarize_pushback_bench_localized_cut_build_config,
        validate_promoted_pushback_bench_localized_cut_access_law_contract,
        validate_promoted_pushback_bench_localized_cut_unit_family_traceability,
    };
    use crate::minelib_scheduling_support::build_marvin_preferred_nested_shell_family_contract;
    use crate::{
        benchmark_blocks_support, build_linear_index_float_lookup,
        build_mine_rs_end_to_end_artifacts, build_phase_scheduling_problem_from_marvin_problem,
        marvin_support,
    };
    use mine_sdk::{
        BlockDimensions, BlockModel, ColumnId, ColumnSchemaSet, Coordinate3D, GridDefinition,
        GridIndex, GridShape, Metadata, NestingAccessRules, PhaseDesign, PushbackPlan,
        ijk_to_linear,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn benchmark_path(instance: &str, file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("datasets")
            .join("benchmarks")
            .join(instance)
            .join(file_name)
    }

    fn synthetic_localized_cut_model() -> BlockModel {
        BlockModel::new(
            GridDefinition::new(
                Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
                BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
                GridShape::new(5, 3, 2).expect("shape should be valid"),
                None,
            )
            .expect("grid should be valid"),
            ColumnSchemaSet::from_columns(vec![]).expect("empty schema should be valid"),
            Metadata::new(),
            BTreeMap::new(),
        )
        .expect("synthetic block model should be valid")
    }

    fn synthetic_linear_index(model: &BlockModel, i: usize, j: usize, k: usize) -> usize {
        ijk_to_linear(model.grid(), GridIndex::new(i, j, k)).expect("grid index should linearize")
    }

    fn synthetic_localized_cut_plan(model: &BlockModel) -> (PushbackPlan, BTreeMap<usize, f64>) {
        let predecessor_blocks = (0..5)
            .map(|i| synthetic_linear_index(model, i, 1, 0))
            .collect::<Vec<_>>();
        let successor_blocks = (0..5)
            .flat_map(|i| (0..3).map(move |j| synthetic_linear_index(model, i, j, 1)))
            .collect::<Vec<_>>();
        let tonnage_by_linear_index = predecessor_blocks
            .iter()
            .chain(successor_blocks.iter())
            .map(|&linear_index| (linear_index, 1.0))
            .collect();
        (
            PushbackPlan {
                phases: vec![
                    PhaseDesign {
                        phase_id: "phase-a".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(110),
                        block_count: predecessor_blocks.len(),
                        total_tonnage: Some(predecessor_blocks.len() as f64),
                        block_indices: predecessor_blocks,
                        predecessor_phase_ids: Vec::new(),
                    },
                    PhaseDesign {
                        phase_id: "phase-b".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(100),
                        block_count: successor_blocks.len(),
                        total_tonnage: Some(successor_blocks.len() as f64),
                        block_indices: successor_blocks,
                        predecessor_phase_ids: vec!["phase-a".to_owned()],
                    },
                ],
                phase_count: 2,
                total_block_count: 20,
                total_tonnage: Some(20.0),
                nesting_rules: NestingAccessRules::default_open(),
                limitations: Vec::new(),
            },
            tonnage_by_linear_index,
        )
    }

    fn synthetic_exact_three_front_diagnostics_plan(
        model: &BlockModel,
    ) -> (PushbackPlan, BTreeMap<usize, f64>) {
        let short_component_blocks = (0..2)
            .map(|i| synthetic_linear_index(model, i, 0, 0))
            .collect::<Vec<_>>();
        let long_component_blocks = (0..3)
            .map(|i| synthetic_linear_index(model, i, 2, 0))
            .collect::<Vec<_>>();
        let tonnage_by_linear_index = short_component_blocks
            .iter()
            .chain(long_component_blocks.iter())
            .map(|&linear_index| (linear_index, 1.0))
            .collect();
        (
            PushbackPlan {
                phases: vec![
                    PhaseDesign {
                        phase_id: "phase-short".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(110),
                        block_count: short_component_blocks.len(),
                        total_tonnage: Some(short_component_blocks.len() as f64),
                        block_indices: short_component_blocks,
                        predecessor_phase_ids: Vec::new(),
                    },
                    PhaseDesign {
                        phase_id: "phase-long".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(100),
                        block_count: long_component_blocks.len(),
                        total_tonnage: Some(long_component_blocks.len() as f64),
                        block_indices: long_component_blocks,
                        predecessor_phase_ids: vec!["phase-short".to_owned()],
                    },
                ],
                phase_count: 2,
                total_block_count: 5,
                total_tonnage: Some(5.0),
                nesting_rules: NestingAccessRules::default_open(),
                limitations: Vec::new(),
            },
            tonnage_by_linear_index,
        )
    }

    fn synthetic_readiness_reason_diagnostics_plan(
        model: &BlockModel,
    ) -> (PushbackPlan, BTreeMap<usize, f64>) {
        let low_span_blocks = (0..2)
            .map(|i| synthetic_linear_index(model, i, 0, 0))
            .collect::<Vec<_>>();
        let low_aspect_blocks = (0..3)
            .flat_map(|i| (0..3).map(move |j| synthetic_linear_index(model, i, j, 1)))
            .collect::<Vec<_>>();
        let ready_blocks = (0..3)
            .map(|i| synthetic_linear_index(model, i, 2, 0))
            .collect::<Vec<_>>();
        let tonnage_by_linear_index = low_span_blocks
            .iter()
            .chain(low_aspect_blocks.iter())
            .chain(ready_blocks.iter())
            .map(|&linear_index| (linear_index, 1.0))
            .collect();
        (
            PushbackPlan {
                phases: vec![
                    PhaseDesign {
                        phase_id: "phase-low-span".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(110),
                        block_count: low_span_blocks.len(),
                        total_tonnage: Some(low_span_blocks.len() as f64),
                        block_indices: low_span_blocks,
                        predecessor_phase_ids: Vec::new(),
                    },
                    PhaseDesign {
                        phase_id: "phase-low-aspect".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(100),
                        block_count: low_aspect_blocks.len(),
                        total_tonnage: Some(low_aspect_blocks.len() as f64),
                        block_indices: low_aspect_blocks,
                        predecessor_phase_ids: vec!["phase-low-span".to_owned()],
                    },
                    PhaseDesign {
                        phase_id: "phase-ready".to_owned(),
                        pushback_index: 0,
                        shell_index: Some(0),
                        revenue_factor: Some(0.8),
                        bench: Some(90),
                        block_count: ready_blocks.len(),
                        total_tonnage: Some(ready_blocks.len() as f64),
                        block_indices: ready_blocks,
                        predecessor_phase_ids: vec!["phase-low-aspect".to_owned()],
                    },
                ],
                phase_count: 3,
                total_block_count: 14,
                total_tonnage: Some(14.0),
                nesting_rules: NestingAccessRules::default_open(),
                limitations: Vec::new(),
            },
            tonnage_by_linear_index,
        )
    }

    #[test]
    fn pushback_bench_localized_cut_builder_supports_configurable_predecessor_cut_links() {
        let model = synthetic_localized_cut_model();
        let (phase_plan, tonnage_by_linear_index) = synthetic_localized_cut_plan(&model);

        let last_cut_plan = split_phase_plan_by_pushback_bench_localized_mining_cuts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            2,
            3.0,
            2,
            true,
            None,
            PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        )
        .expect("last-cut predecessor policy should build");
        let first_cut_plan = split_phase_plan_by_pushback_bench_localized_mining_cuts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            2,
            3.0,
            2,
            true,
            None,
            PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorFirstCut,
            PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        )
        .expect("first-cut predecessor policy should build");
        let all_cuts_plan = split_phase_plan_by_pushback_bench_localized_mining_cuts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            2,
            3.0,
            2,
            true,
            None,
            PushbackBenchLocalizedCutPredecessorLinkPolicy::AllPredecessorCuts,
            PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        )
        .expect("all-predecessor-cuts policy should build");

        let last_successor_predecessors = last_cut_plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-b::pbcut-c01")
            .expect("successor cut should exist")
            .predecessor_phase_ids
            .clone();
        let first_successor_predecessors = first_cut_plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-b::pbcut-c01")
            .expect("successor cut should exist")
            .predecessor_phase_ids
            .clone();
        let all_successor_predecessors = all_cuts_plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-b::pbcut-c01")
            .expect("successor cut should exist")
            .predecessor_phase_ids
            .clone();

        assert_eq!(last_successor_predecessors, vec!["phase-a::pbcut-c01s02"]);
        assert_eq!(first_successor_predecessors, vec!["phase-a::pbcut-c01s01"]);
        assert_eq!(
            all_successor_predecessors,
            vec!["phase-a::pbcut-c01s01", "phase-a::pbcut-c01s02"]
        );
    }

    #[test]
    fn pushback_bench_localized_cut_builder_records_access_law_scope_in_limitations() {
        let model = synthetic_localized_cut_model();
        let (phase_plan, tonnage_by_linear_index) = synthetic_localized_cut_plan(&model);
        let cut_plan = split_phase_plan_by_pushback_bench_localized_mining_cuts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            2,
            3.0,
            2,
            true,
            Some(2),
            PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorFirstCut,
            PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
        )
        .expect("localized cut plan should build");

        assert!(
            cut_plan.limitations.iter().any(|limitation| {
                limitation.contains("`predecessor-first-cut` predecessor-cut link policy")
                    && limitation.contains("`uniform-tonnage-balanced` front progression")
                    && limitation
                        .contains("fixed `sequential-previous-cut` intra-component activation")
            }),
            "limitations should document that this benchmark-side slice exposes access-law policy only"
        );
    }

    #[test]
    fn pushback_bench_localized_cut_builder_supports_explicit_three_front_progression_profiles() {
        let model = synthetic_localized_cut_model();
        let (phase_plan, tonnage_by_linear_index) = synthetic_localized_cut_plan(&model);
        let cut_plan = split_phase_plan_by_pushback_bench_localized_mining_cuts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            3,
            2.0,
            2,
            true,
            None,
            PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
            PushbackBenchLocalizedCutFrontProgression::FixedThreeFrontCumulativeTargets {
                label: "front-loaded-45-80-100",
                cumulative_tonnage_targets: [0.45, 0.80, 1.0],
            },
        )
        .expect("custom three-front progression should build");

        let successor_cut_sizes = cut_plan
            .phases
            .iter()
            .filter(|phase| phase.phase_id.starts_with("phase-b::pbcut-c01s"))
            .map(|phase| phase.block_count)
            .collect::<Vec<_>>();

        assert_eq!(successor_cut_sizes, vec![7, 5, 3]);
    }

    #[test]
    fn pushback_bench_localized_cut_builder_records_exact_three_front_feasibility_metrics() {
        let model = synthetic_localized_cut_model();
        let (phase_plan, tonnage_by_linear_index) =
            synthetic_exact_three_front_diagnostics_plan(&model);

        let diagnostics = build_pushback_bench_localized_cut_benchmark_artifacts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            PushbackBenchLocalizedCutBuildConfig {
                max_front_count: 3,
                min_aspect_ratio: 1.0,
                min_dominant_span: 1,
                include_touching_neighbors: true,
                max_local_predecessor_count: None,
                predecessor_cut_link_policy:
                    PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
                front_progression:
                    PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
            },
            |cut_phase_plan| Ok(cut_phase_plan.phase_count),
        )
        .expect("localized cut diagnostics should build")
        .phase_refinement_diagnostics;

        assert_eq!(
            diagnostics.realized_front_count_histogram,
            BTreeMap::from([(2usize, 1usize), (3usize, 1usize)])
        );
        assert_eq!(diagnostics.exact_three_front_candidate_count, 2);
        assert_eq!(diagnostics.exact_three_front_failure_count, 1);
        assert_eq!(
            diagnostics.exact_three_front_failure_realized_front_histogram,
            BTreeMap::from([(2usize, 1usize)])
        );
        assert_eq!(
            diagnostics.readiness_reason_histogram,
            BTreeMap::from([
                (
                    "exact-three-front-infeasible-insufficient-axis-stripes".to_owned(),
                    1usize,
                ),
                ("paper-like-three-front-ready".to_owned(), 1usize),
            ])
        );
        assert_eq!(
            diagnostics.exact_three_front_failure_reason_histogram,
            BTreeMap::from([("insufficient-axis-stripes".to_owned(), 1usize)])
        );
    }

    #[test]
    fn pushback_bench_localized_cut_builder_records_readiness_reason_histogram() {
        let model = synthetic_localized_cut_model();
        let (phase_plan, tonnage_by_linear_index) =
            synthetic_readiness_reason_diagnostics_plan(&model);

        let diagnostics = build_pushback_bench_localized_cut_benchmark_artifacts(
            &model,
            &phase_plan,
            &tonnage_by_linear_index,
            PushbackBenchLocalizedCutBuildConfig {
                max_front_count: 3,
                min_aspect_ratio: 2.0,
                min_dominant_span: 2,
                include_touching_neighbors: true,
                max_local_predecessor_count: None,
                predecessor_cut_link_policy:
                    PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
                front_progression:
                    PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
            },
            |cut_phase_plan| Ok(cut_phase_plan.phase_count),
        )
        .expect("readiness diagnostics should build")
        .phase_refinement_diagnostics;

        assert_eq!(
            diagnostics.readiness_reason_histogram,
            BTreeMap::from([
                (
                    "blocked-low-dominant-span-and-aspect-ratio".to_owned(),
                    1usize,
                ),
                ("blocked-low-aspect-ratio".to_owned(), 1usize),
                ("paper-like-three-front-ready".to_owned(), 1usize),
            ])
        );
        assert_eq!(
            diagnostics.realized_front_count_histogram,
            BTreeMap::from([(1usize, 2usize), (3usize, 1usize)])
        );
    }

    #[test]
    fn pushback_bench_localized_cut_config_summary_reports_explicit_access_law_contract() {
        let diagnostics = super::PushbackBenchLocalizedCutRefinementDiagnostics {
            base_phase_count: 10,
            refined_base_phase_count: 6,
            refined_single_component_phase_count: 4,
            total_cut_phase_count: 18,
            additional_phase_count: 8,
            max_cut_count_per_base_phase: 3,
            average_cut_count_per_base_phase: 1.8,
            realized_front_count_histogram: BTreeMap::from([(1, 4), (2, 3), (3, 3)]),
            readiness_reason_histogram: BTreeMap::from([
                ("paper-like-three-front-ready".to_owned(), 3),
                ("blocked-low-aspect-ratio".to_owned(), 4),
                ("refined-beyond-paper-like-three-front".to_owned(), 3),
            ]),
            exact_three_front_candidate_count: 5,
            exact_three_front_failure_count: 1,
            exact_three_front_failure_realized_front_histogram: BTreeMap::from([(2, 1)]),
            exact_three_front_failure_reason_histogram: BTreeMap::from([(
                "collapsed-target-partition".to_owned(),
                1,
            )]),
            refined_base_phase_examples: vec!["phase-a".to_owned()],
            refined_single_component_phase_examples: vec!["phase-b".to_owned()],
        };
        let summary = summarize_pushback_bench_localized_cut_build_config(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
            &diagnostics,
        );

        assert_eq!(
            summary.unit_family_label,
            "pushback-bench-localized-cut-phase"
        );
        assert_eq!(
            summary.promoted_build_label,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL
        );
        assert_eq!(
            summary
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy,
            "predecessor-last-cut"
        );
        assert!(
            summary
                .release_inter_phase_inter_cut
                .proxy_status
                .contains("best-current-local-proxy")
        );
        assert_eq!(
            summary.local_predecessor_filter.localized_access_mode,
            "overlap-plus-adjacency"
        );
        assert_eq!(
            summary.local_predecessor_filter.predecessor_window_policy,
            "closest-N=6"
        );
        assert_eq!(
            summary.intra_phase_progression.front_progression,
            MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_LABEL
        );
        assert_eq!(
            summary
                .intra_phase_progression
                .front_progression_contract_kind,
            "preferred-fixed-three-front-cumulative-targets-with-uniform-fallback"
        );
        assert_eq!(
            summary.intra_phase_progression.front_progression_targets,
            Some([1.0 / 3.0, 2.0 / 3.0, 1.0])
        );
        assert_eq!(
            summary
                .intra_phase_progression
                .front_progression_fallback
                .as_deref(),
            Some("uniform-tonnage-balanced when realized front count differs from 3")
        );
        assert_eq!(
            summary.intra_phase_progression.intra_component_activation,
            "sequential-previous-cut"
        );
        assert!(
            summary
                .ramp_access_contract
                .proxy_status
                .contains("benchmark-side partial proxy")
        );
        assert_eq!(
            summary.ramp_access_contract.predecessor_cut_link_policy,
            summary
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy
        );
        assert_eq!(
            summary.ramp_access_contract.localized_access_mode,
            summary.local_predecessor_filter.localized_access_mode
        );
        assert_eq!(
            summary.ramp_access_contract.predecessor_window_policy,
            summary.local_predecessor_filter.predecessor_window_policy
        );
        assert_eq!(
            summary.ramp_access_contract.intra_component_activation,
            summary.intra_phase_progression.intra_component_activation
        );
        assert!(
            summary
                .ramp_access_contract
                .contract_surface
                .contains("cut_access_law.ramp_access_contract")
        );
        assert!(
            summary
                .working_width_contract
                .proxy_status
                .contains("benchmark-side partial proxy")
        );
        assert_eq!(
            summary
                .working_width_contract
                .geometry_eligible_component_count,
            6
        );
        assert_eq!(
            summary
                .working_width_contract
                .geometry_blocked_component_count,
            4
        );
        assert_eq!(
            summary
                .working_width_contract
                .preferred_three_front_fallback_count,
            1
        );
        assert!(
            summary
                .lineage_bench_continuity_contract
                .proxy_status
                .contains("benchmark-side partial proxy")
        );
        assert_eq!(
            summary
                .lineage_bench_continuity_contract
                .predecessor_cut_link_policy,
            summary
                .release_inter_phase_inter_cut
                .predecessor_cut_link_policy
        );
        assert_eq!(
            summary
                .lineage_bench_continuity_contract
                .intra_component_activation,
            summary.intra_phase_progression.intra_component_activation
        );
        assert!(
            summary
                .lineage_bench_continuity_contract
                .contract_surface
                .contains("cut_access_law.lineage_bench_continuity_contract")
        );
        assert_eq!(
            summary.missing_bibliographic_terms,
            vec![
                "ramps / ramp access sequencing",
                "working width / minimum operating width",
                "cut-design lineage / bench continuity",
                "complete cut design law across pushbacks, cuts and benches"
            ]
        );
        assert_eq!(summary.bibliographic_gap_contract.len(), 4);
        assert_eq!(
            summary.bibliographic_gap_contract[0].gap_id,
            "ramp-access-sequencing"
        );
        assert_eq!(
            summary.bibliographic_gap_contract[1].gap_id,
            "working-width-minimum-operating-width"
        );
        assert_eq!(
            summary.bibliographic_gap_contract[2].gap_id,
            "cut-design-lineage-bench-continuity"
        );
        assert_eq!(
            summary.bibliographic_gap_contract[3].gap_id,
            "complete-cut-design-law"
        );
        assert!(
            summary
                .bibliographic_gap_contract
                .iter()
                .all(|gap| { gap.contract_surface.contains("cut_access_law") })
        );
        assert_eq!(
            summary.bibliographic_gap_contract[0].current_status,
            "benchmark-side-partial-proxy"
        );
        assert!(
            summary.bibliographic_gap_contract[0]
                .contract_surface
                .contains("ramp_access_contract")
        );
        assert_eq!(
            summary.bibliographic_gap_contract[1].current_status,
            "benchmark-side-partial-proxy"
        );
        assert!(
            summary.bibliographic_gap_contract[1]
                .contract_surface
                .contains("working_width_contract")
        );
        assert_eq!(
            summary.bibliographic_gap_contract[2].current_status,
            "benchmark-side-partial-proxy"
        );
        assert!(
            summary.bibliographic_gap_contract[2]
                .contract_surface
                .contains("lineage_bench_continuity_contract")
        );
        assert_eq!(
            summary.bibliographic_gap_contract[3].current_status,
            "benchmark-side-partial-proxy"
        );
        assert!(
            summary.bibliographic_gap_contract[3]
                .contract_surface
                .contains("complete_cut_design_contract")
        );
        assert!(
            validate_promoted_pushback_bench_localized_cut_access_law_contract(
                &summary,
                MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL
            )
            .is_ok()
        );
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred shell family should build")
            .with_realized_shell_count(5);
        let MarvinMr187PromotedPushbackBenchLocalizedCutContractSurfaces {
            promoted_build_label,
            unit_family_traceability: traceability,
            access_law,
        } = build_marvin_mr187_promoted_pushback_bench_localized_cut_contract_surfaces(
            "cpit-solution",
            8_516,
            "nested-shell-bench",
            Some(&preferred_shell_family),
            "pushback-bench-localized-cut-phase",
            &super::PushbackBenchLocalizedCutRefinementDiagnostics {
                base_phase_count: 10,
                refined_base_phase_count: 6,
                refined_single_component_phase_count: 4,
                total_cut_phase_count: 18,
                additional_phase_count: 8,
                max_cut_count_per_base_phase: 3,
                average_cut_count_per_base_phase: 1.8,
                realized_front_count_histogram: BTreeMap::from([(1, 4), (2, 3), (3, 3)]),
                readiness_reason_histogram: BTreeMap::from([
                    ("paper-like-three-front-ready".to_owned(), 3),
                    ("blocked-low-aspect-ratio".to_owned(), 4),
                    ("refined-beyond-paper-like-three-front".to_owned(), 3),
                ]),
                exact_three_front_candidate_count: 5,
                exact_three_front_failure_count: 1,
                exact_three_front_failure_realized_front_histogram: BTreeMap::from([(2, 1)]),
                exact_three_front_failure_reason_histogram: BTreeMap::from([(
                    "collapsed-target-partition".to_owned(),
                    1,
                )]),
                refined_base_phase_examples: vec!["phase-a".to_owned()],
                refined_single_component_phase_examples: vec!["phase-b".to_owned()],
            },
        );
        assert_eq!(
            promoted_build_label,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL
        );
        assert_eq!(access_law.promoted_build_label, promoted_build_label);
        assert!(
            validate_promoted_pushback_bench_localized_cut_unit_family_traceability(
                &traceability,
                "cpit-solution",
                8_516,
                "nested-shell-bench",
                "shape-gated-local-front-phase",
                "pushback-bench-localized-cut-phase",
                promoted_build_label,
            )
            .is_ok()
        );
        let family_summary = format_promoted_pushback_bench_localized_cut_family_summary(
            &traceability,
            "pushback-bench-localized-cut-phase",
            promoted_build_label,
            "shape-gated-local-front-phase",
            &access_law,
        );
        assert!(family_summary.contains("selected_block_source = \"cpit-solution\""));
        assert!(family_summary.contains("nested-shell-bench"));
        assert!(family_summary.contains("pushback-bench-localized-mining-cuts"));
        let input_gap_summary =
            super::format_promoted_pushback_bench_localized_cut_input_aggregation_gap_summary(
                &traceability,
                18,
                12,
            );
        assert!(input_gap_summary.contains("selected_block_source = \"cpit-solution\""));
        assert!(input_gap_summary.contains("8516"));
        assert!(input_gap_summary.contains("nested-shell-bench"));
        assert!(input_gap_summary.contains(
            "Quantitatively, 8516 selected blocks currently compress into 18 promoted phases and 12 LP/BZ scheduling units."
        ));
        assert!(input_gap_summary.contains(promoted_build_label));
        assert!(input_gap_summary.contains("shape-gated-local-front-phase"));
    }

    #[test]
    fn promoted_access_law_validation_rejects_ramp_contract_drift() {
        let mut summary = summarize_pushback_bench_localized_cut_build_config(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
            &super::PushbackBenchLocalizedCutRefinementDiagnostics {
                base_phase_count: 10,
                refined_base_phase_count: 6,
                refined_single_component_phase_count: 4,
                total_cut_phase_count: 18,
                additional_phase_count: 8,
                max_cut_count_per_base_phase: 3,
                average_cut_count_per_base_phase: 1.8,
                realized_front_count_histogram: BTreeMap::from([(1, 4), (2, 3), (3, 3)]),
                readiness_reason_histogram: BTreeMap::from([
                    ("paper-like-three-front-ready".to_owned(), 3),
                    ("blocked-low-aspect-ratio".to_owned(), 4),
                    ("refined-beyond-paper-like-three-front".to_owned(), 3),
                ]),
                exact_three_front_candidate_count: 5,
                exact_three_front_failure_count: 1,
                exact_three_front_failure_realized_front_histogram: BTreeMap::from([(2, 1)]),
                exact_three_front_failure_reason_histogram: BTreeMap::from([(
                    "collapsed-target-partition".to_owned(),
                    1,
                )]),
                refined_base_phase_examples: vec!["phase-a".to_owned()],
                refined_single_component_phase_examples: vec!["phase-b".to_owned()],
            },
        );
        summary.ramp_access_contract.predecessor_window_policy = "drifted-window".to_owned();

        let error = validate_promoted_pushback_bench_localized_cut_access_law_contract(
            &summary,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        )
        .expect_err("ramp-access drift should fail validation");
        assert!(
            error
                .to_string()
                .contains("Ramp-access proxy predecessor window")
        );
    }

    #[test]
    fn promoted_access_law_validation_rejects_lineage_contract_drift() {
        let mut summary = summarize_pushback_bench_localized_cut_build_config(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
            &super::PushbackBenchLocalizedCutRefinementDiagnostics {
                base_phase_count: 10,
                refined_base_phase_count: 6,
                refined_single_component_phase_count: 4,
                total_cut_phase_count: 18,
                additional_phase_count: 8,
                max_cut_count_per_base_phase: 3,
                average_cut_count_per_base_phase: 1.8,
                realized_front_count_histogram: BTreeMap::from([(1, 4), (2, 3), (3, 3)]),
                readiness_reason_histogram: BTreeMap::from([
                    ("paper-like-three-front-ready".to_owned(), 3),
                    ("blocked-low-aspect-ratio".to_owned(), 4),
                    ("refined-beyond-paper-like-three-front".to_owned(), 3),
                ]),
                exact_three_front_candidate_count: 5,
                exact_three_front_failure_count: 1,
                exact_three_front_failure_realized_front_histogram: BTreeMap::from([(2, 1)]),
                exact_three_front_failure_reason_histogram: BTreeMap::from([(
                    "collapsed-target-partition".to_owned(),
                    1,
                )]),
                refined_base_phase_examples: vec!["phase-a".to_owned()],
                refined_single_component_phase_examples: vec!["phase-b".to_owned()],
            },
        );
        summary
            .lineage_bench_continuity_contract
            .predecessor_cut_link_policy = "drifted-lineage-policy".to_owned();

        let error = validate_promoted_pushback_bench_localized_cut_access_law_contract(
            &summary,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        )
        .expect_err("lineage contract drift should fail validation");
        assert!(
            error
                .to_string()
                .contains("Lineage/bench-continuity predecessor policy")
        );
    }

    #[test]
    fn promoted_unit_family_traceability_validation_rejects_selected_block_source_drift() {
        let preferred_shell_family = build_marvin_preferred_nested_shell_family_contract(7)
            .expect("preferred shell family should build")
            .with_realized_shell_count(5);
        let mut traceability = build_promoted_pushback_bench_localized_cut_unit_family_traceability(
            "cpit-solution",
            8_516,
            "nested-shell-bench",
            Some(&preferred_shell_family),
            "shape-gated-local-front-phase",
            "pushback-bench-localized-cut-phase",
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
            MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION_LABEL,
        );
        traceability.selected_block_provenance.selected_block_source =
            "manual-selection".to_owned();

        let error = validate_promoted_pushback_bench_localized_cut_unit_family_traceability(
            &traceability,
            "cpit-solution",
            8_516,
            "nested-shell-bench",
            "shape-gated-local-front-phase",
            "pushback-bench-localized-cut-phase",
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_PROMOTED_BUILD_LABEL,
        )
        .expect_err("selected block source drift should fail validation");
        assert!(error.to_string().contains("selected_block_source"));
    }

    #[test]
    fn marvin_mr187_default_build_config_stays_on_shared_contract() {
        assert_eq!(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
            "front3-ar2.0-span2-n6"
        );
        assert_eq!(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
            PushbackBenchLocalizedCutBuildConfig {
                max_front_count: 3,
                min_aspect_ratio: 2.0,
                min_dominant_span: 2,
                include_touching_neighbors: true,
                max_local_predecessor_count: Some(6),
                predecessor_cut_link_policy:
                    PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
                front_progression: MARVIN_MR187_PROMOTED_LOCAL_FRONT_PROGRESSION,
            }
        );
    }

    #[test]
    fn pushback_bench_localized_cut_builder_refines_single_component_shell_benches() {
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

        let cut_artifacts = build_pushback_bench_localized_cut_benchmark_artifacts(
            &model,
            &base_artifacts.phase_plan,
            &tonnage_lookup,
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
            |phase_plan| {
                build_phase_scheduling_problem_from_marvin_problem(
                    &model,
                    phase_plan,
                    &pcpsp_problem,
                )
            },
        )
        .expect("pushback bench-localized cut artifacts should build");

        assert!(
            cut_artifacts.benchmark.phase_plan.phase_count > base_artifacts.phase_plan.phase_count,
            "pushback bench-localized cuts should refine the shell×bench base plan"
        );
        assert!(
            cut_artifacts
                .benchmark
                .phase_plan
                .phases
                .iter()
                .any(|phase| phase.phase_id.contains("::pbcut-c")),
            "pushback bench-localized cuts should emit pbcut phase ids"
        );
        assert!(
            cut_artifacts
                .phase_refinement_diagnostics
                .refined_single_component_phase_count
                > 0,
            "pushback bench-localized cuts should refine at least one single-component shell×bench phase"
        );
        assert!(
            cut_artifacts
                .benchmark
                .phase_plan
                .limitations
                .iter()
                .any(|limitation| limitation.contains("Pushback bench-localized mining cuts")),
            "pushback bench-localized cuts should record the benchmark-side mining-cut limitation"
        );
        assert!(
            !cut_artifacts
                .benchmark
                .scheduling_problem
                .units()
                .is_empty(),
            "pushback bench-localized cut builder should also materialize a scheduling problem"
        );
    }
}
