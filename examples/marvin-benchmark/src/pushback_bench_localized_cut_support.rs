use std::collections::{BTreeMap, BTreeSet};

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

pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL: &str =
    "front3-ar2.0-span2-n4";

pub const MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG:
    PushbackBenchLocalizedCutBuildConfig = PushbackBenchLocalizedCutBuildConfig {
    max_front_count: 3,
    min_aspect_ratio: 2.0,
    min_dominant_span: 2,
    include_touching_neighbors: true,
    max_local_predecessor_count: Some(4),
    predecessor_cut_link_policy: PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
    front_progression: PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
};

#[derive(Debug, Serialize)]
pub struct PushbackBenchLocalizedCutRefinementDiagnostics {
    pub base_phase_count: usize,
    pub refined_base_phase_count: usize,
    pub refined_single_component_phase_count: usize,
    pub total_cut_phase_count: usize,
    pub additional_phase_count: usize,
    pub max_cut_count_per_base_phase: usize,
    pub average_cut_count_per_base_phase: f64,
    pub realized_front_count_histogram: BTreeMap<usize, usize>,
    pub exact_three_front_candidate_count: usize,
    pub exact_three_front_failure_count: usize,
    pub exact_three_front_failure_realized_front_histogram: BTreeMap<usize, usize>,
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
    pub localized_access_mode: String,
    pub predecessor_window_policy: String,
    pub predecessor_cut_link_policy: String,
    pub front_progression: String,
    pub intra_component_activation: String,
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
) -> PushbackBenchLocalizedCutAccessPolicySummary {
    PushbackBenchLocalizedCutAccessPolicySummary {
        localized_access_mode: localized_access_mode_label(config.include_touching_neighbors)
            .to_owned(),
        predecessor_window_policy: match config.max_local_predecessor_count {
            Some(max_count) => format!("closest-N={max_count}"),
            None => "unbounded predecessor fan-in".to_owned(),
        },
        predecessor_cut_link_policy: config.predecessor_cut_link_policy.label().to_owned(),
        front_progression: config.front_progression.label().to_owned(),
        intra_component_activation: localized_cut_activation_label().to_owned(),
    }
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
    let mut exact_three_front_candidate_count = 0usize;
    let mut exact_three_front_failure_count = 0usize;
    let mut exact_three_front_failure_realized_front_histogram = BTreeMap::<usize, usize>::new();
    for component_diagnostic in component_diagnostics {
        *realized_front_count_histogram
            .entry(component_diagnostic.realized_front_count)
            .or_default() += 1;
        if component_diagnostic.exact_three_front_candidate {
            exact_three_front_candidate_count += 1;
            if component_diagnostic.realized_front_count != 3 {
                exact_three_front_failure_count += 1;
                *exact_three_front_failure_realized_front_histogram
                    .entry(component_diagnostic.realized_front_count)
                    .or_default() += 1;
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
        exact_three_front_candidate_count,
        exact_three_front_failure_count,
        exact_three_front_failure_realized_front_histogram,
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
    exact_three_front_candidate: bool,
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
            let candidate_fronts = split_component_by_dominant_axis_stripes(
                model,
                &component_block_indices,
                tonnage_by_linear_index,
                max_front_count,
                front_progression,
            )?;
            let realized_front_count =
                if dominant_span >= min_dominant_span && aspect_ratio >= min_aspect_ratio {
                    candidate_fronts
                        .iter()
                        .filter(|front| !front.is_empty())
                        .count()
                        .max(1)
                } else {
                    1
                };
            diagnostics.push(LocalizedCutComponentRefinementDiagnostic {
                realized_front_count,
                exact_three_front_candidate: max_front_count >= 3
                    && dominant_span >= min_dominant_span
                    && aspect_ratio >= min_aspect_ratio,
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
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
        PushbackBenchLocalizedCutBuildConfig, PushbackBenchLocalizedCutFrontProgression,
        PushbackBenchLocalizedCutPredecessorLinkPolicy,
        build_pushback_bench_localized_cut_benchmark_artifacts,
        split_phase_plan_by_pushback_bench_localized_mining_cuts,
        summarize_pushback_bench_localized_cut_build_config,
    };
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
    }

    #[test]
    fn pushback_bench_localized_cut_config_summary_reports_explicit_access_law_contract() {
        let summary = summarize_pushback_bench_localized_cut_build_config(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
        );

        assert_eq!(summary.localized_access_mode, "overlap-plus-adjacency");
        assert_eq!(summary.predecessor_window_policy, "closest-N=4");
        assert_eq!(summary.predecessor_cut_link_policy, "predecessor-last-cut");
        assert_eq!(summary.front_progression, "uniform-tonnage-balanced");
        assert_eq!(
            summary.intra_component_activation,
            "sequential-previous-cut"
        );
    }

    #[test]
    fn marvin_mr187_default_build_config_stays_on_shared_contract() {
        assert_eq!(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_LABEL,
            "front3-ar2.0-span2-n4"
        );
        assert_eq!(
            MARVIN_MR187_PUSHBACK_BENCH_LOCALIZED_CUT_DEFAULT_BUILD_CONFIG,
            PushbackBenchLocalizedCutBuildConfig {
                max_front_count: 3,
                min_aspect_ratio: 2.0,
                min_dominant_span: 2,
                include_touching_neighbors: true,
                max_local_predecessor_count: Some(4),
                predecessor_cut_link_policy:
                    PushbackBenchLocalizedCutPredecessorLinkPolicy::PredecessorLastCut,
                front_progression:
                    PushbackBenchLocalizedCutFrontProgression::UniformTonnageBalanced,
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
