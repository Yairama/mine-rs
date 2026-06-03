use std::collections::{BTreeMap, BTreeSet};

use mine_core::{Metadata, MetadataValue, MineError, ModelId, ScenarioId};
use mine_planning::{
    LongTermSchedulePeriodCapacity, LongTermScheduleStockpile, PhaseDesign, PushbackPlan,
    ScheduleDestinationId, ScheduleStockpileId, SchedulingObjectiveTerm, SchedulingPeriod,
    SchedulingProblem, SchedulingResourceBound, SchedulingResourceId,
    SchedulingResourceRequirement, SchedulingUnit, destination_capacity_resource_id,
    stockpile_reclaim_capacity_resource_id,
};
use serde::{Deserialize, Serialize};

use crate::{
    BlockGrades, DestinationAssumptionSet, DestinationId, DestinationKind, EconomicBlockModel,
    StockpileSchedulingStage, StockpileTargetParcel,
    block_valuation::value_block_by_destinations,
    schedule_economics::{PhaseEconomicSummary, build_grade_column_slices, build_row_index_map},
    stage_pushback_plan_for_stockpile_readiness,
};

const MINE_TONNAGE_RESOURCE_ID: &str = "mine_tonnage";
const PLANT_TONNAGE_RESOURCE_ID: &str = "plant_tonnage";

/// Perfil downstream explícito permitido para promover reclaim staged sin semánticas ocultas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StagedStockpileReclaimDownstreamProfile {
    /// Reusa la economía determinista del destino económico final declarado.
    EconomicDestination,
}

impl StagedStockpileReclaimDownstreamProfile {
    /// Serializa el perfil downstream estable usado en metadata auditable.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EconomicDestination => "economic-destination",
        }
    }
}

/// Regla serializable para colapsar reclaim staged a una única ruta auditable por stockpile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedStockpileReclaimRule {
    stockpile_id: DestinationId,
    reclaim_destination_id: DestinationId,
    downstream_profile: StagedStockpileReclaimDownstreamProfile,
}

impl StagedStockpileReclaimRule {
    /// Crea una regla explícita de reclaim staged para un stockpile.
    pub fn new(
        stockpile_id: DestinationId,
        reclaim_destination_id: DestinationId,
        downstream_profile: StagedStockpileReclaimDownstreamProfile,
    ) -> Result<Self, MineError> {
        Ok(Self {
            stockpile_id,
            reclaim_destination_id,
            downstream_profile,
        })
    }

    /// Stockpile al que aplica la regla.
    #[must_use]
    pub const fn stockpile_id(&self) -> &DestinationId {
        &self.stockpile_id
    }

    /// Destino final único que debe usarse al promover reclaim desde el stockpile.
    #[must_use]
    pub const fn reclaim_destination_id(&self) -> &DestinationId {
        &self.reclaim_destination_id
    }

    /// Perfil downstream explícito que debe conservar el adapter.
    #[must_use]
    pub const fn downstream_profile(&self) -> StagedStockpileReclaimDownstreamProfile {
        self.downstream_profile
    }
}

/// Contrato reusable y serializable para resolver reclaim staged ambiguo por stockpile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedStockpileReclaimPolicy {
    rules: Vec<StagedStockpileReclaimRule>,
}

impl StagedStockpileReclaimPolicy {
    /// Crea una policy reusable validando que no existan reglas duplicadas por stockpile.
    pub fn new(rules: Vec<StagedStockpileReclaimRule>) -> Result<Self, MineError> {
        let mut seen = BTreeSet::<DestinationId>::new();
        for rule in &rules {
            if !seen.insert(rule.stockpile_id.clone()) {
                return Err(MineError::invalid_parameter(
                    "staged_stockpile_reclaim_policy.rules",
                    "must not contain duplicate stockpile rules",
                ));
            }
        }
        Ok(Self { rules })
    }

    /// Reglas declaradas por la policy.
    #[must_use]
    pub fn rules(&self) -> &[StagedStockpileReclaimRule] {
        &self.rules
    }

    /// Busca la regla aplicable a un stockpile específico.
    #[must_use]
    pub fn rule_for_stockpile(
        &self,
        stockpile_id: &DestinationId,
    ) -> Option<&StagedStockpileReclaimRule> {
        self.rules
            .iter()
            .find(|rule| rule.stockpile_id() == stockpile_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReclaimRouteSource {
    Inference,
    Policy,
}

impl ReclaimRouteSource {
    #[must_use]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Inference => "inference",
            Self::Policy => "policy",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RepresentableStagedParcel {
    pub(crate) staged_parcel_id: String,
    pub(crate) parent_phase_id: String,
    pub(crate) stockpile_id: ScheduleStockpileId,
    pub(crate) destination_id: ScheduleDestinationId,
    pub(crate) destination_kind: DestinationKind,
    pub(crate) total_tonnage: f64,
    pub(crate) block_indices: Vec<usize>,
    pub(crate) revenue: f64,
    pub(crate) payable_metal: BTreeMap<String, f64>,
    pub(crate) mining_cost_carryover: f64,
    pub(crate) downstream_cost: f64,
    pub(crate) stockpile_inventory_delta_tonnage: f64,
    pub(crate) reclaim_route_source: ReclaimRouteSource,
    pub(crate) reclaim_downstream_profile: StagedStockpileReclaimDownstreamProfile,
}

#[derive(Debug, Clone)]
struct RepresentableStagedParcelEconomics {
    total_tonnage: f64,
    revenue: f64,
    payable_metal: BTreeMap<String, f64>,
    mining_cost_carryover: f64,
    downstream_cost: f64,
}

/// Deriva un `SchedulingProblem` explícito desde `PushbackPlan` y `EconomicBlockModel`.
///
/// Soporta fases destino-puro y promueve parcels staged representables hacia unidades
/// reclaim/direct-feed con `stockpile_inventory_delta_tonnage` firmado. Los casos
/// staged que todavía requieran semánticas no representables siguen fallando de forma
/// explícita y auditable.
#[allow(clippy::too_many_arguments)]
pub fn build_scheduling_problem_from_economic_block_model(
    scenario_id: ScenarioId,
    model_id: ModelId,
    phase_plan: &PushbackPlan,
    capacities: Vec<LongTermSchedulePeriodCapacity>,
    stockpiles: Vec<LongTermScheduleStockpile>,
    economic_model: &EconomicBlockModel,
    discount_rate: f64,
    metadata: Metadata,
) -> Result<SchedulingProblem, MineError> {
    build_scheduling_problem_from_economic_block_model_with_reclaim_policy(
        scenario_id,
        model_id,
        phase_plan,
        capacities,
        stockpiles,
        economic_model,
        discount_rate,
        metadata,
        None,
    )
}

/// Variante explícita del adapter que puede consumir una policy serializable de reclaim staged.
#[allow(clippy::too_many_arguments)]
pub fn build_scheduling_problem_from_economic_block_model_with_reclaim_policy(
    scenario_id: ScenarioId,
    model_id: ModelId,
    phase_plan: &PushbackPlan,
    capacities: Vec<LongTermSchedulePeriodCapacity>,
    stockpiles: Vec<LongTermScheduleStockpile>,
    economic_model: &EconomicBlockModel,
    discount_rate: f64,
    metadata: Metadata,
    reclaim_policy: Option<&StagedStockpileReclaimPolicy>,
) -> Result<SchedulingProblem, MineError> {
    let staged_phase_plan =
        stage_pushback_plan_for_stockpile_readiness(phase_plan, economic_model)?;
    let representable_staged_parcels =
        build_representable_staged_parcels(&staged_phase_plan, economic_model, reclaim_policy)?;
    let phase_summaries = staged_phase_plan
        .direct_phase_refinements
        .iter()
        .map(|phase| {
            (
                phase.phase_id.clone(),
                PhaseEconomicSummary {
                    total_tonnage: phase.total_tonnage,
                    revenue: phase.revenue,
                    cost: phase.cost,
                    destination_tonnage: BTreeMap::from([(
                        phase.destination_id.as_str().to_owned(),
                        phase.total_tonnage,
                    )]),
                    payable_metal: phase.payable_metal.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let periods = capacities
        .iter()
        .map(build_scheduling_period_with_destination_resources)
        .collect::<Result<Vec<_>, _>>()?;
    let declared_resource_ids = periods
        .iter()
        .flat_map(SchedulingPeriod::resource_bounds)
        .map(SchedulingResourceBound::resource_id)
        .cloned()
        .collect::<BTreeSet<_>>();

    let mine_resource = declared_resource_ids
        .contains(&SchedulingResourceId::new(MINE_TONNAGE_RESOURCE_ID)?)
        .then(|| SchedulingResourceId::new(MINE_TONNAGE_RESOURCE_ID))
        .transpose()?;
    let plant_resource = declared_resource_ids
        .contains(&SchedulingResourceId::new(PLANT_TONNAGE_RESOURCE_ID)?)
        .then(|| SchedulingResourceId::new(PLANT_TONNAGE_RESOURCE_ID))
        .transpose()?;
    let parent_phase_by_id = phase_plan
        .phases
        .iter()
        .map(|phase| (phase.phase_id.as_str(), phase))
        .collect::<BTreeMap<_, _>>();
    let direct_parent_by_unit_id = staged_phase_plan
        .direct_phase_refinements
        .iter()
        .map(|phase| (phase.phase_id.as_str(), phase.parent_phase_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let parent_to_promoted_unit_ids =
        build_parent_to_promoted_unit_ids(&staged_phase_plan, &representable_staged_parcels)?;

    let mut destination_ids = BTreeSet::<ScheduleDestinationId>::new();
    let mut units = Vec::with_capacity(
        staged_phase_plan.direct_phase_refinements.len() + representable_staged_parcels.len(),
    );
    let mut objective_term_values = BTreeMap::<(String, Option<String>), f64>::new();
    let mut resource_requirement_values = BTreeMap::<(String, String, Option<String>), f64>::new();

    for direct_phase in &staged_phase_plan.direct_phase_refinements {
        let phase_summary = phase_summaries.get(direct_phase.phase_id.as_str()).ok_or_else(|| {
            MineError::Economics {
                message: format!(
                    "phase `{}` is missing from the economic summaries used to build the scheduling problem",
                    direct_phase.phase_id
                ),
            }
        })?;
        validate_phase_tonnage(
            direct_phase.phase_id.as_str(),
            direct_phase.total_tonnage,
            phase_summary,
        )?;
        let parent_phase_id = direct_parent_by_unit_id
            .get(direct_phase.phase_id.as_str())
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "direct refined phase `{}` is missing its parent phase identifier",
                    direct_phase.phase_id
                ),
            })?;
        let parent_phase = parent_phase_by_id
            .get(parent_phase_id)
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "parent phase `{parent_phase_id}` required by direct refined phase `{}` is missing from the PushbackPlan",
                    direct_phase.phase_id
                ),
            })?;
        let destination_id =
            single_phase_destination_id(direct_phase.phase_id.as_str(), phase_summary)?;
        let destination = economic_model
            .destinations()
            .get(&DestinationId::new(destination_id.as_str().to_owned())?)
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "destination `{}` is missing from the economic assumptions",
                    destination_id
                ),
            })?;
        destination_ids.insert(destination_id.clone());

        units.push(SchedulingUnit::new(
            mine_planning::SchedulingUnitId::new(direct_phase.phase_id.clone())?,
            direct_phase.total_tonnage,
            direct_phase.block_indices.len(),
            expanded_predecessor_unit_ids(
                parent_phase,
                &parent_to_promoted_unit_ids,
                direct_phase.phase_id.as_str(),
            )?,
            vec![destination_id.clone()],
            Vec::new(),
            direct_phase.block_indices.clone(),
            parent_phase.bench,
            parent_phase.shell_index,
            Metadata::new(),
        )?);

        insert_unique_value(
            &mut objective_term_values,
            (direct_phase.phase_id.clone(), None),
            -phase_summary.cost,
        )?;
        insert_unique_value(
            &mut objective_term_values,
            (
                direct_phase.phase_id.clone(),
                Some(destination_id.as_str().to_owned()),
            ),
            phase_summary.revenue,
        )?;

        if let Some(resource_id) = &mine_resource {
            insert_unique_value(
                &mut resource_requirement_values,
                (
                    direct_phase.phase_id.clone(),
                    resource_id.as_str().to_owned(),
                    None,
                ),
                phase_summary.total_tonnage,
            )?;
        }

        if let Some(resource_id) = &plant_resource
            && destination_consumes_plant_tonnage(destination.kind())
        {
            insert_unique_value(
                &mut resource_requirement_values,
                (
                    direct_phase.phase_id.clone(),
                    resource_id.as_str().to_owned(),
                    Some(destination_id.as_str().to_owned()),
                ),
                phase_summary.total_tonnage,
            )?;
        }

        let destination_capacity_resource = destination_capacity_resource_id(&destination_id)?;
        if declared_resource_ids.contains(&destination_capacity_resource) {
            insert_unique_value(
                &mut resource_requirement_values,
                (
                    direct_phase.phase_id.clone(),
                    destination_capacity_resource.as_str().to_owned(),
                    Some(destination_id.as_str().to_owned()),
                ),
                phase_summary.total_tonnage,
            )?;
        }
    }

    for staged_parcel in &representable_staged_parcels {
        let parent_phase = parent_phase_by_id
            .get(staged_parcel.parent_phase_id.as_str())
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "parent phase `{}` required by staged parcel `{}` is missing from the PushbackPlan",
                    staged_parcel.parent_phase_id, staged_parcel.staged_parcel_id
                ),
            })?;
        destination_ids.insert(staged_parcel.destination_id.clone());

        units.push(
            SchedulingUnit::new(
                mine_planning::SchedulingUnitId::new(staged_parcel.staged_parcel_id.clone())?,
                staged_parcel.total_tonnage,
                staged_parcel.block_indices.len(),
                expanded_predecessor_unit_ids(
                    parent_phase,
                    &parent_to_promoted_unit_ids,
                    staged_parcel.staged_parcel_id.as_str(),
                )?,
                vec![staged_parcel.destination_id.clone()],
                vec![staged_parcel.stockpile_id.clone()],
                staged_parcel.block_indices.clone(),
                parent_phase.bench,
                parent_phase.shell_index,
                reclaim_cost_split_metadata(staged_parcel)?,
            )?
            .with_stockpile_inventory_delta_tonnage(Some(
                staged_parcel.stockpile_inventory_delta_tonnage,
            ))?,
        );

        insert_unique_value(
            &mut objective_term_values,
            (staged_parcel.staged_parcel_id.clone(), None),
            -staged_parcel.downstream_cost,
        )?;
        insert_unique_value(
            &mut objective_term_values,
            (
                staged_parcel.staged_parcel_id.clone(),
                Some(staged_parcel.destination_id.as_str().to_owned()),
            ),
            staged_parcel.revenue,
        )?;

        if let Some(resource_id) = &plant_resource
            && destination_consumes_plant_tonnage(staged_parcel.destination_kind)
        {
            insert_unique_value(
                &mut resource_requirement_values,
                (
                    staged_parcel.staged_parcel_id.clone(),
                    resource_id.as_str().to_owned(),
                    Some(staged_parcel.destination_id.as_str().to_owned()),
                ),
                staged_parcel.total_tonnage,
            )?;
        }

        let destination_capacity_resource =
            destination_capacity_resource_id(&staged_parcel.destination_id)?;
        if declared_resource_ids.contains(&destination_capacity_resource) {
            insert_unique_value(
                &mut resource_requirement_values,
                (
                    staged_parcel.staged_parcel_id.clone(),
                    destination_capacity_resource.as_str().to_owned(),
                    Some(staged_parcel.destination_id.as_str().to_owned()),
                ),
                staged_parcel.total_tonnage,
            )?;
        }

        let reclaim_capacity_resource =
            stockpile_reclaim_capacity_resource_id(&staged_parcel.stockpile_id)?;
        if declared_resource_ids.contains(&reclaim_capacity_resource) {
            insert_unique_value(
                &mut resource_requirement_values,
                (
                    staged_parcel.staged_parcel_id.clone(),
                    reclaim_capacity_resource.as_str().to_owned(),
                    None,
                ),
                -staged_parcel.stockpile_inventory_delta_tonnage,
            )?;
        }
    }

    let objective_terms = objective_term_values
        .into_iter()
        .map(|((unit_id, destination_id), value)| {
            SchedulingObjectiveTerm::new(
                mine_planning::SchedulingUnitId::new(unit_id)?,
                destination_id.map(ScheduleDestinationId::new).transpose()?,
                value,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let resource_requirements = resource_requirement_values
        .into_iter()
        .map(|((unit_id, resource_id, destination_id), amount)| {
            SchedulingResourceRequirement::new(
                mine_planning::SchedulingUnitId::new(unit_id)?,
                SchedulingResourceId::new(resource_id)?,
                destination_id.map(ScheduleDestinationId::new).transpose()?,
                amount,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut limitations = staged_phase_plan
        .limitations
        .iter()
        .filter(|item| {
            *item
                != "stockpile-target parcels are staged separately from direct schedulable units until reclaim/inventory semantics exist in SchedulingProblem"
                && *item
                    != "the direct pushback subset omits stockpile-target parcels and must not be promoted to SchedulingProblem while staged stockpile material remains"
        })
        .cloned()
        .collect::<Vec<_>>();
    limitations.extend([
        "This scheduling contract derives explicit objective terms and resource coefficients from EconomicBlockModel for single-destination phases.".to_owned(),
        "Mixed-destination phases are refined into destination-pure child phases before building the SchedulingProblem.".to_owned(),
        "Representable staged stockpile parcels are promoted as reclaim/direct-feed scheduling units with explicit stockpile inventory deltas and final destination routing.".to_owned(),
        "Promoted reclaim/direct-feed units expose mining-cost carryover in unit metadata while their objective cost coefficients include only downstream destination costs.".to_owned(),
        "Promoted staged stockpile parcels currently consume downstream destination resources without adding mine tonnage requirements in the reusable scheduling contract.".to_owned(),
        "Plant tonnage coefficients are currently derived only for Mill and Leach destinations.".to_owned(),
    ]);
    limitations.sort();
    limitations.dedup();

    SchedulingProblem::new(
        scenario_id,
        model_id,
        periods,
        units,
        objective_terms,
        resource_requirements,
        destination_ids.into_iter().collect(),
        stockpiles,
        discount_rate,
        metadata,
        limitations,
    )
}

fn build_scheduling_period_with_destination_resources(
    capacity: &LongTermSchedulePeriodCapacity,
) -> Result<SchedulingPeriod, MineError> {
    let period = SchedulingPeriod::from_long_term_capacity(capacity)?;
    let mut resource_bounds = period.resource_bounds().to_vec();

    for destination_capacity in period.destination_capacities() {
        let Some(max_total) = destination_capacity.max_tonnage() else {
            continue;
        };
        let resource_id = destination_capacity_resource_id(destination_capacity.destination_id())?;
        if resource_bounds
            .iter()
            .any(|bound| bound.resource_id() == &resource_id)
        {
            continue;
        }
        resource_bounds.push(SchedulingResourceBound::new(
            resource_id,
            None,
            Some(max_total),
        )?);
    }
    for stockpile_capacity in period.stockpile_capacities() {
        let Some(max_total) = stockpile_capacity.max_reclaim_tonnage() else {
            continue;
        };
        let resource_id =
            stockpile_reclaim_capacity_resource_id(stockpile_capacity.stockpile_id())?;
        if resource_bounds
            .iter()
            .any(|bound| bound.resource_id() == &resource_id)
        {
            continue;
        }
        resource_bounds.push(SchedulingResourceBound::new(
            resource_id,
            None,
            Some(max_total),
        )?);
    }

    SchedulingPeriod::new(
        period.period_label(),
        resource_bounds,
        period.destination_capacities().to_vec(),
        period.stockpile_capacities().to_vec(),
    )
}

fn validate_phase_tonnage(
    phase_id: &str,
    tonnage: f64,
    phase_summary: &PhaseEconomicSummary,
) -> Result<(), MineError> {
    let tolerance = phase_summary.total_tonnage.abs().max(1.0) * 1.0e-9;
    if (tonnage - phase_summary.total_tonnage).abs() > tolerance {
        return Err(MineError::Economics {
            message: format!(
                "phase `{}` tonnage mismatch: scheduling unit carries {} t but EconomicBlockModel aggregates {} t",
                phase_id, tonnage, phase_summary.total_tonnage,
            ),
        });
    }
    Ok(())
}

fn single_phase_destination_id(
    phase_id: &str,
    phase_summary: &PhaseEconomicSummary,
) -> Result<ScheduleDestinationId, MineError> {
    match phase_summary.destination_tonnage.len() {
        0 => Err(MineError::Economics {
            message: format!(
                "phase `{phase_id}` has no economic destination tonnage in EconomicBlockModel"
            ),
        }),
        1 => {
            let destination_id = phase_summary
                .destination_tonnage
                .keys()
                .next()
                .expect("single destination phase should expose its destination");
            ScheduleDestinationId::new(destination_id.clone())
        }
        _ => Err(MineError::Economics {
            message: format!(
                "phase `{phase_id}` spans more than one economic destination; MR-199 currently supports only single economic destination phases"
            ),
        }),
    }
}

fn destination_consumes_plant_tonnage(kind: DestinationKind) -> bool {
    matches!(kind, DestinationKind::Mill | DestinationKind::Leach)
}

fn insert_unique_value<K>(
    target: &mut BTreeMap<K, f64>,
    key: K,
    value: f64,
) -> Result<(), MineError>
where
    K: Ord,
{
    if target.insert(key, value).is_some() {
        return Err(MineError::Economics {
            message:
                "scheduling problem adapter generated duplicate coefficients for the same scope"
                    .to_owned(),
        });
    }
    Ok(())
}

fn build_parent_to_promoted_unit_ids(
    staged_phase_plan: &StockpileSchedulingStage,
    representable_staged_parcels: &[RepresentableStagedParcel],
) -> Result<BTreeMap<String, Vec<mine_planning::SchedulingUnitId>>, MineError> {
    let mut parent_to_unit_ids =
        BTreeMap::<String, BTreeSet<mine_planning::SchedulingUnitId>>::new();

    for direct_phase in &staged_phase_plan.direct_phase_refinements {
        parent_to_unit_ids
            .entry(direct_phase.parent_phase_id.clone())
            .or_default()
            .insert(mine_planning::SchedulingUnitId::new(
                direct_phase.phase_id.clone(),
            )?);
    }

    for staged_parcel in representable_staged_parcels {
        parent_to_unit_ids
            .entry(staged_parcel.parent_phase_id.clone())
            .or_default()
            .insert(mine_planning::SchedulingUnitId::new(
                staged_parcel.staged_parcel_id.clone(),
            )?);
    }

    parent_to_unit_ids
        .into_iter()
        .map(|(parent_phase_id, unit_ids)| {
            if unit_ids.is_empty() {
                return Err(MineError::Economics {
                    message: format!(
                        "parent phase `{parent_phase_id}` does not expose any promotable scheduling units after economics staging"
                    ),
                });
            }
            Ok((parent_phase_id, unit_ids.into_iter().collect()))
        })
        .collect()
}

fn expanded_predecessor_unit_ids(
    parent_phase: &PhaseDesign,
    parent_to_promoted_unit_ids: &BTreeMap<String, Vec<mine_planning::SchedulingUnitId>>,
    unit_id: &str,
) -> Result<Vec<mine_planning::SchedulingUnitId>, MineError> {
    let mut predecessor_unit_ids = Vec::new();
    for predecessor_phase_id in &parent_phase.predecessor_phase_ids {
        let child_unit_ids = parent_to_promoted_unit_ids.get(predecessor_phase_id).ok_or_else(|| {
            MineError::Economics {
                message: format!(
                    "phase `{unit_id}` depends on parent phase `{predecessor_phase_id}` but economics staging did not produce any promotable child units for that predecessor"
                ),
            }
        })?;
        predecessor_unit_ids.extend(child_unit_ids.iter().cloned());
    }
    predecessor_unit_ids.sort();
    predecessor_unit_ids.dedup();
    Ok(predecessor_unit_ids)
}

pub(crate) fn build_representable_staged_parcels(
    staged_phase_plan: &StockpileSchedulingStage,
    economic_model: &EconomicBlockModel,
    reclaim_policy: Option<&StagedStockpileReclaimPolicy>,
) -> Result<Vec<RepresentableStagedParcel>, MineError> {
    let row_by_linear_index = build_row_index_map(economic_model)?;
    let grade_columns = build_grade_column_slices(economic_model)?;
    let staged_parcel_count = staged_phase_plan.stockpile_target_parcels.len();

    staged_phase_plan
        .stockpile_target_parcels
        .iter()
        .map(|parcel| {
            build_representable_staged_parcel(
                parcel,
                staged_parcel_count,
                economic_model,
                &row_by_linear_index,
                &grade_columns,
                reclaim_policy,
            )
        })
        .collect()
}

fn build_representable_staged_parcel(
    parcel: &StockpileTargetParcel,
    staged_parcel_count: usize,
    economic_model: &EconomicBlockModel,
    row_by_linear_index: &BTreeMap<usize, usize>,
    grade_columns: &BTreeMap<String, &[f64]>,
    reclaim_policy: Option<&StagedStockpileReclaimPolicy>,
) -> Result<RepresentableStagedParcel, MineError> {
    let (reclaim_destination_id, reclaim_destination_kind, reclaim_route_source) =
        resolve_reclaim_route(parcel, staged_parcel_count, economic_model, reclaim_policy)?;
    if !parcel.reclaim_inventory_delta_tonnage.is_finite()
        || parcel.reclaim_inventory_delta_tonnage >= 0.0
    {
        return Err(MineError::Economics {
            message: format!(
                "economic scheduling staging found {} stockpile-target parcel(s); staged parcel `{}` from parent `{}` routes to stockpile `{}` and final destination `{}` but carries non-reclaim inventory delta {} t",
                staged_parcel_count,
                parcel.staged_parcel_id,
                parcel.parent_phase_id,
                parcel.stockpile_id.as_str(),
                reclaim_destination_id.as_str(),
                parcel.reclaim_inventory_delta_tonnage,
            ),
        });
    }

    let economics = summarize_staged_parcel_for_destination(
        parcel,
        &reclaim_destination_id,
        economic_model,
        row_by_linear_index,
        grade_columns,
    )?;

    Ok(RepresentableStagedParcel {
        staged_parcel_id: parcel.staged_parcel_id.clone(),
        parent_phase_id: parcel.parent_phase_id.clone(),
        stockpile_id: ScheduleStockpileId::new(parcel.stockpile_id.as_str().to_owned())?,
        destination_id: ScheduleDestinationId::new(reclaim_destination_id.as_str().to_owned())?,
        destination_kind: reclaim_destination_kind,
        total_tonnage: economics.total_tonnage,
        block_indices: parcel.block_indices.clone(),
        revenue: economics.revenue,
        payable_metal: economics.payable_metal,
        mining_cost_carryover: economics.mining_cost_carryover,
        downstream_cost: economics.downstream_cost,
        stockpile_inventory_delta_tonnage: parcel.reclaim_inventory_delta_tonnage,
        reclaim_route_source,
        reclaim_downstream_profile: StagedStockpileReclaimDownstreamProfile::EconomicDestination,
    })
}

fn resolve_reclaim_route(
    parcel: &StockpileTargetParcel,
    staged_parcel_count: usize,
    economic_model: &EconomicBlockModel,
    reclaim_policy: Option<&StagedStockpileReclaimPolicy>,
) -> Result<(DestinationId, DestinationKind, ReclaimRouteSource), MineError> {
    let inferred_route = match (
        parcel.reclaim_destination_id.as_ref(),
        parcel.reclaim_destination_kind,
    ) {
        (Some(destination_id), Some(destination_kind)) => {
            Some((destination_id.clone(), destination_kind))
        }
        (Some(_), None) => {
            return Err(MineError::Economics {
                message: format!(
                    "economic scheduling staging found {} stockpile-target parcel(s); staged parcel `{}` from parent `{}` routes to stockpile `{}` but is missing the reclaim destination kind required for representable promotion",
                    staged_parcel_count,
                    parcel.staged_parcel_id,
                    parcel.parent_phase_id,
                    parcel.stockpile_id.as_str(),
                ),
            });
        }
        (None, Some(_)) => {
            return Err(MineError::Economics {
                message: format!(
                    "economic scheduling staging found {} stockpile-target parcel(s); staged parcel `{}` from parent `{}` routes to stockpile `{}` but is missing the reclaim destination id required for representable promotion",
                    staged_parcel_count,
                    parcel.staged_parcel_id,
                    parcel.parent_phase_id,
                    parcel.stockpile_id.as_str(),
                ),
            });
        }
        (None, None) => None,
    };
    let policy_route = reclaim_policy
        .and_then(|policy| policy.rule_for_stockpile(&parcel.stockpile_id))
        .map(|rule| validate_policy_route(rule, parcel, economic_model))
        .transpose()?;

    if let Some((destination_id, destination_kind)) = inferred_route {
        if let Some((policy_destination_id, _policy_destination_kind, _profile)) = &policy_route
            && policy_destination_id != &destination_id
        {
            return Err(MineError::Economics {
                message: format!(
                    "staged stockpile reclaim policy for stockpile `{}` conflicts with the uniquely inferred reclaim destination for parcel `{}`: policy declares `{}` but deterministic inference already resolves `{}`",
                    parcel.stockpile_id.as_str(),
                    parcel.staged_parcel_id,
                    policy_destination_id.as_str(),
                    destination_id.as_str(),
                ),
            });
        }
        return Ok((
            destination_id,
            destination_kind,
            ReclaimRouteSource::Inference,
        ));
    }

    if let Some((destination_id, destination_kind, _profile)) = policy_route {
        return Ok((destination_id, destination_kind, ReclaimRouteSource::Policy));
    }

    let limitation = parcel
        .reclaim_promotion_limitations
        .first()
        .cloned()
        .unwrap_or_else(|| {
            "staged parcel still lacks a representable reclaim/direct-feed route".to_owned()
        });
    Err(MineError::Economics {
        message: format!(
            "economic scheduling staging found {} stockpile-target parcel(s); first staged parcel `{}` from parent `{}` routes to stockpile `{}` and cannot yet be promoted to SchedulingProblem. {limitation}",
            staged_parcel_count,
            parcel.staged_parcel_id,
            parcel.parent_phase_id,
            parcel.stockpile_id.as_str(),
        ),
    })
}

fn validate_policy_route(
    rule: &StagedStockpileReclaimRule,
    parcel: &StockpileTargetParcel,
    economic_model: &EconomicBlockModel,
) -> Result<
    (
        DestinationId,
        DestinationKind,
        StagedStockpileReclaimDownstreamProfile,
    ),
    MineError,
> {
    let destination = economic_model
        .destinations()
        .get(rule.reclaim_destination_id())
        .ok_or_else(|| MineError::Economics {
            message: format!(
                "staged stockpile reclaim policy for stockpile `{}` references destination `{}` for parcel `{}`, but that destination is missing from the economic assumptions",
                rule.stockpile_id().as_str(),
                rule.reclaim_destination_id().as_str(),
                parcel.staged_parcel_id,
            ),
        })?;
    if matches!(
        destination.kind(),
        DestinationKind::Stockpile | DestinationKind::Waste
    ) {
        return Err(MineError::Economics {
            message: format!(
                "staged stockpile reclaim policy for stockpile `{}` declares destination `{}` with unsupported kind `{:?}` for parcel `{}`; only non-stockpile, non-waste reclaim destinations are representable",
                rule.stockpile_id().as_str(),
                destination.id().as_str(),
                destination.kind(),
                parcel.staged_parcel_id,
            ),
        });
    }
    Ok((
        destination.id().clone(),
        destination.kind(),
        rule.downstream_profile(),
    ))
}

fn summarize_staged_parcel_for_destination(
    parcel: &StockpileTargetParcel,
    destination_id: &DestinationId,
    economic_model: &EconomicBlockModel,
    row_by_linear_index: &BTreeMap<usize, usize>,
    grade_columns: &BTreeMap<String, &[f64]>,
) -> Result<RepresentableStagedParcelEconomics, MineError> {
    let destination = economic_model
        .destinations()
        .get(destination_id)
        .cloned()
        .ok_or_else(|| MineError::Economics {
            message: format!(
                "destination `{}` is missing from the economic assumptions",
                destination_id.as_str()
            ),
        })?;
    let destination_set = DestinationAssumptionSet::new(vec![destination.clone()])?;
    let summary_by_linear_index = economic_model
        .block_summaries()
        .iter()
        .map(|summary| (summary.linear_index, summary))
        .collect::<BTreeMap<_, _>>();
    let mut total_tonnage = 0.0;
    let mut revenue = 0.0;
    let mut payable_metal = BTreeMap::<String, f64>::new();
    let mut mining_cost_carryover = 0.0;
    let mut downstream_cost = 0.0;

    for &linear_index in &parcel.block_indices {
        let block_summary = summary_by_linear_index
            .get(&linear_index)
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "staged parcel `{}` references block `{linear_index}` that is missing from the economic block model",
                    parcel.staged_parcel_id
                ),
            })?;
        let row_index = row_by_linear_index
            .get(&linear_index)
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "block `{linear_index}` cannot be mapped back to a materialized row for staged parcel `{}`",
                    parcel.staged_parcel_id
                ),
            })?;
        let block_grades = build_block_grades(
            block_summary.tonnage,
            row_index,
            destination.id(),
            grade_columns,
        )?;
        let valuation = value_block_by_destinations(&block_grades, &destination_set)?;
        let destination_value = valuation.for_destination(destination.id()).expect(
            "single-destination valuation should expose the requested reclaim/direct-feed destination",
        );
        total_tonnage += block_summary.tonnage;
        revenue += destination_value.nsr_per_tonne * block_summary.tonnage;
        mining_cost_carryover += destination_value.mining_cost_per_tonne * block_summary.tonnage;
        downstream_cost += destination_value.downstream_cost_per_tonne * block_summary.tonnage;

        for recovery in destination.recoveries() {
            let metal_key = recovery.metal_column().as_str();
            let payability = destination
                .payabilities()
                .iter()
                .find(|payability| payability.metal_column() == recovery.metal_column())
                .map(|payability| payability.payability_fraction())
                .unwrap_or(1.0);
            let grades = grade_columns.get(metal_key).ok_or_else(|| MineError::Economics {
                message: format!(
                    "grade column `{metal_key}` required by destination `{}` is missing from the economic block model",
                    destination.id().as_str()
                ),
            })?;
            let grade = grades.get(row_index).copied().ok_or_else(|| MineError::Economics {
                message: format!(
                    "grade column `{metal_key}` is missing row `{row_index}` required for staged parcel `{}`",
                    parcel.staged_parcel_id
                ),
            })?;
            *payable_metal.entry(metal_key.to_owned()).or_insert(0.0) +=
                block_summary.tonnage * grade * recovery.recovery_fraction() * payability;
        }
    }

    Ok(RepresentableStagedParcelEconomics {
        total_tonnage,
        revenue,
        payable_metal,
        mining_cost_carryover,
        downstream_cost,
    })
}

fn reclaim_cost_split_metadata(parcel: &RepresentableStagedParcel) -> Result<Metadata, MineError> {
    validate_reclaim_cost_split_inputs(parcel)?;
    let mut metadata = Metadata::new();
    metadata.insert(
        "economics.cost_split_contract",
        MetadataValue::Text("reclaim-downstream-cost-plus-mining-carryover".to_owned()),
    )?;
    metadata.insert(
        "economics.mining_cost_carryover",
        MetadataValue::Float(parcel.mining_cost_carryover),
    )?;
    metadata.insert(
        "economics.reclaim_downstream_cost",
        MetadataValue::Float(parcel.downstream_cost),
    )?;
    metadata.insert(
        "economics.mining_cost_carryover_per_tonne",
        MetadataValue::Float(parcel.mining_cost_carryover / parcel.total_tonnage),
    )?;
    metadata.insert(
        "economics.reclaim_downstream_cost_per_tonne",
        MetadataValue::Float(parcel.downstream_cost / parcel.total_tonnage),
    )?;
    metadata.insert(
        "economics.reclaim_route_source",
        MetadataValue::Text(parcel.reclaim_route_source.as_str().to_owned()),
    )?;
    metadata.insert(
        "economics.reclaim_downstream_profile",
        MetadataValue::Text(parcel.reclaim_downstream_profile.as_str().to_owned()),
    )?;
    Ok(metadata)
}

fn validate_reclaim_cost_split_inputs(parcel: &RepresentableStagedParcel) -> Result<(), MineError> {
    if !parcel.total_tonnage.is_finite() || parcel.total_tonnage <= 0.0 {
        return Err(MineError::Economics {
            message: format!(
                "staged parcel `{}` for stockpile `{}` and destination `{}` cannot populate `reclaim-downstream-cost-plus-mining-carryover` metadata because total tonnage is {} t; this accounting contract requires a positive finite promoted tonnage",
                parcel.staged_parcel_id,
                parcel.stockpile_id.as_str(),
                parcel.destination_id.as_str(),
                parcel.total_tonnage,
            ),
        });
    }

    for (field_name, value) in [
        ("revenue", parcel.revenue),
        ("mining_cost_carryover", parcel.mining_cost_carryover),
        ("reclaim_downstream_cost", parcel.downstream_cost),
    ] {
        if !value.is_finite() {
            return Err(MineError::Economics {
                message: format!(
                    "staged parcel `{}` for stockpile `{}` and destination `{}` cannot populate `reclaim-downstream-cost-plus-mining-carryover` metadata because `{field_name}` is non-finite ({value}); this accounting contract requires explicit finite reclaim cost-split inputs",
                    parcel.staged_parcel_id,
                    parcel.stockpile_id.as_str(),
                    parcel.destination_id.as_str(),
                ),
            });
        }
    }

    Ok(())
}

fn build_block_grades(
    tonnage: f64,
    row_index: usize,
    destination_id: &DestinationId,
    grade_columns: &BTreeMap<String, &[f64]>,
) -> Result<BlockGrades, MineError> {
    let grades: BTreeMap<String, f64> = grade_columns
        .iter()
        .map(|(column_id, values)| -> Result<(String, f64), MineError> {
            let grade = values.get(row_index).copied().ok_or_else(|| MineError::Economics {
                message: format!(
                    "grade column `{column_id}` is missing row `{row_index}` required for destination `{}`",
                    destination_id.as_str()
                ),
            })?;
            Ok((column_id.clone(), grade))
        })
        .collect::<Result<_, _>>()?;
    BlockGrades::new(tonnage, grades)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_blockmodel::{BlockModel, ColumnData};
    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        MetadataValue, ModelId, ScenarioId,
    };
    use mine_planning::{
        LongTermSchedulePeriodCapacity, LongTermScheduleStockpile, NestingAccessRules, PhaseDesign,
        PushbackPlan, ScheduleDestinationCapacity, ScheduleDestinationId,
        ScheduleStockpileCapacity, ScheduleStockpileId, SchedulingUnit,
        build_ready_frontier_long_term_schedule, build_ready_frontier_schedule,
    };

    use crate::{
        DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
        DestinationKind, DestinationPayability, DestinationRecovery, EconomicBlockModel,
        EconomicBlockModelConfig, evaluate_long_term_schedule_economics,
        evaluate_long_term_schedule_economics_with_reclaim_policy,
    };

    use super::{
        StagedStockpileReclaimDownstreamProfile, StagedStockpileReclaimPolicy,
        StagedStockpileReclaimRule, build_scheduling_problem_from_economic_block_model,
        build_scheduling_problem_from_economic_block_model_with_reclaim_policy,
    };

    fn small_grid(block_count: usize) -> GridDefinition {
        let origin = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let shape = GridShape::new(block_count, 1, 1).expect("shape should be valid");
        GridDefinition::new(origin, dims, shape, None).expect("grid should be valid")
    }

    fn small_model(cu_grades: Vec<f64>, tonnages: Vec<f64>) -> BlockModel {
        let grid = small_grid(cu_grades.len());
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let ton = ColumnId::new("ton").expect("column id should be valid");
        let unit_pct = MeasurementUnit::new("%").expect("% is valid");
        let unit_t = MeasurementUnit::new("t").expect("t is valid");

        let schema = ColumnSchemaSet::from_columns(vec![
            ColumnSchema::new(
                cu.clone(),
                ColumnLogicalType::Float,
                Some(unit_pct),
                false,
                ColumnMiningRole::Grade,
            ),
            ColumnSchema::new(
                ton.clone(),
                ColumnLogicalType::Float,
                Some(unit_t),
                false,
                ColumnMiningRole::Tonnage,
            ),
        ])
        .expect("schema should be valid");

        let mut cols = BTreeMap::new();
        cols.insert(cu, ColumnData::Floats(cu_grades));
        cols.insert(ton, ColumnData::Floats(tonnages));

        BlockModel::new(grid, schema, Metadata::new(), cols).expect("model should be valid")
    }

    fn two_destination_set() -> DestinationAssumptionSet {
        let cu = ColumnId::new("cu").expect("column id should be valid");

        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            2.0,
            8.0,
            vec![DestinationRecovery::new(cu.clone(), 0.88).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 0.97).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9000.0)]),
        )
        .expect("mill should be valid");

        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");

        DestinationAssumptionSet::new(vec![mill, waste]).expect("set should be valid")
    }

    fn phase_plan(phases: Vec<PhaseDesign>, total_tonnage: f64) -> PushbackPlan {
        PushbackPlan {
            phase_count: phases.len(),
            total_block_count: phases.iter().map(|phase| phase.block_indices.len()).sum(),
            total_tonnage: Some(total_tonnage),
            phases,
            nesting_rules: NestingAccessRules::strict_sequential(),
            limitations: vec![],
        }
    }

    fn single_phase(
        phase_id: &str,
        block_indices: Vec<usize>,
        tonnage: f64,
        predecessors: Vec<String>,
    ) -> PhaseDesign {
        let block_count = block_indices.len();
        PhaseDesign {
            phase_id: phase_id.to_owned(),
            pushback_index: 0,
            shell_index: Some(0),
            revenue_factor: Some(1.0),
            bench: Some(100),
            block_indices,
            block_count,
            total_tonnage: Some(tonnage),
            predecessor_phase_ids: predecessors,
        }
    }

    fn capacities_with_mill_limits(
        include_destination_capacity: bool,
    ) -> Vec<LongTermSchedulePeriodCapacity> {
        let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
        let destination_capacities = if include_destination_capacity {
            vec![
                ScheduleDestinationCapacity::new(mill, Some(1_000.0))
                    .expect("destination capacity should be valid"),
            ]
        } else {
            Vec::new()
        };

        vec![
            LongTermSchedulePeriodCapacity::new(
                "P1",
                Some(1_000.0),
                Some(1_000.0),
                destination_capacities.clone(),
                vec![],
            )
            .expect("capacity should be valid"),
            LongTermSchedulePeriodCapacity::new(
                "P2",
                Some(1_000.0),
                Some(1_000.0),
                destination_capacities,
                vec![],
            )
            .expect("capacity should be valid"),
        ]
    }

    fn capacities_with_mill_and_stockpile_limits(
        include_destination_capacity: bool,
        stockpile_id: &str,
    ) -> Vec<LongTermSchedulePeriodCapacity> {
        let mut periods = capacities_with_mill_limits(include_destination_capacity);
        let stockpile_capacity = ScheduleStockpileCapacity::new(
            ScheduleStockpileId::new(stockpile_id).expect("stockpile id should be valid"),
            Some(1_000.0),
            Some(1_000.0),
        )
        .expect("stockpile capacity should be valid");
        for period in &mut periods {
            *period = LongTermSchedulePeriodCapacity::new(
                period.period_label(),
                period.max_mine_tonnage(),
                period.max_plant_tonnage(),
                period.destination_capacities().to_vec(),
                vec![stockpile_capacity.clone()],
            )
            .expect("capacity should stay valid");
        }
        periods
    }

    fn objective_term_value(
        problem: &mine_planning::SchedulingProblem,
        unit_id: &str,
        destination_id: Option<&str>,
    ) -> f64 {
        problem
            .objective_terms()
            .iter()
            .find(|term| {
                term.unit_id().as_str() == unit_id
                    && term.destination_id().map(|id| id.as_str()) == destination_id
            })
            .expect("objective term should exist")
            .value()
    }

    fn metadata_float(unit: &SchedulingUnit, key: &str) -> f64 {
        match unit.metadata().get(key) {
            Some(MetadataValue::Float(value)) => *value,
            other => panic!("expected float metadata for `{key}`, got {other:?}"),
        }
    }

    fn metadata_text<'a>(unit: &'a SchedulingUnit, key: &str) -> &'a str {
        match unit.metadata().get(key) {
            Some(MetadataValue::Text(value)) => value.as_str(),
            other => panic!("expected text metadata for `{key}`, got {other:?}"),
        }
    }

    fn resource_requirement_amount(
        problem: &mine_planning::SchedulingProblem,
        unit_id: &str,
        resource_id: &str,
        destination_id: Option<&str>,
    ) -> f64 {
        problem
            .resource_requirements()
            .iter()
            .find(|requirement| {
                requirement.unit_id().as_str() == unit_id
                    && requirement.resource_id().as_str() == resource_id
                    && requirement.destination_id().map(|id| id.as_str()) == destination_id
            })
            .expect("resource requirement should exist")
            .amount()
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn adapter_populates_problem_and_prioritizes_higher_value_phase() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.35, 1.10], vec![1_000.0, 1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: two_destination_set(),
            },
        )
        .expect("economic block model should build");
        let phase_plan = phase_plan(
            vec![
                single_phase("phase-low", vec![0], 1_000.0, vec![]),
                single_phase("phase-high", vec![1], 1_000.0, vec![]),
            ],
            2_000.0,
        );

        let problem = build_scheduling_problem_from_economic_block_model(
            ScenarioId::new("mr199-scenario").expect("scenario should be valid"),
            ModelId::new("mr199-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_limits(true),
            vec![
                LongTermScheduleStockpile::new(
                    mine_planning::ScheduleStockpileId::new("sp-main")
                        .expect("stockpile id should be valid"),
                    0.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
        )
        .expect("problem should build");

        assert_eq!(problem.units().len(), 2);
        assert_eq!(problem.objective_terms().len(), 4);
        assert_eq!(problem.resource_requirements().len(), 6);
        assert_eq!(
            problem.units()[0].eligible_destination_ids()[0].as_str(),
            "mill"
        );
        assert_close(
            objective_term_value(&problem, "phase-high", None),
            -10_000.0,
        );
        assert_close(
            objective_term_value(&problem, "phase-high", Some("mill")),
            9_000.0 * 0.88 * 0.97 * 1.10 * 1_000.0,
        );
        assert!(problem.units()[0].eligible_stockpile_ids().is_empty());
        assert!(
            problem
                .limitations()
                .iter()
                .any(|item| item.contains("single-destination phases"))
        );

        let solution = build_ready_frontier_schedule(&problem).expect("heuristic should build");
        assert_eq!(solution.assignments()[0].period_label(), "P1");
        assert_eq!(solution.assignments()[0].unit_id().as_str(), "phase-high");
        assert_eq!(
            solution.assignments()[0]
                .destination_id()
                .expect("assignment should carry destination")
                .as_str(),
            "mill"
        );
    }

    #[test]
    fn adapter_declares_destinations_even_without_capacity_rows() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.80], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: two_destination_set(),
            },
        )
        .expect("economic block model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-a", vec![0], 1_000.0, vec![])],
            1_000.0,
        );

        let problem = build_scheduling_problem_from_economic_block_model(
            ScenarioId::new("mr199-no-cap-dest").expect("scenario should be valid"),
            ModelId::new("mr199-no-cap-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_limits(false),
            vec![],
            &economic_model,
            0.10,
            Metadata::new(),
        )
        .expect("problem should build without destination capacities");

        assert_eq!(problem.destination_ids().len(), 1);
        assert_eq!(problem.destination_ids()[0].as_str(), "mill");
        assert_eq!(
            problem.units()[0].eligible_destination_ids()[0].as_str(),
            "mill"
        );
    }

    #[test]
    fn adapter_refines_mixed_destination_phase() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.80, 0.0], vec![1_000.0, 1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: two_destination_set(),
            },
        )
        .expect("economic block model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-mixed", vec![0, 1], 2_000.0, vec![])],
            2_000.0,
        );

        let problem = build_scheduling_problem_from_economic_block_model(
            ScenarioId::new("mr199-mixed").expect("scenario should be valid"),
            ModelId::new("mr199-mixed-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_limits(false),
            vec![],
            &economic_model,
            0.10,
            Metadata::new(),
        )
        .expect("mixed-destination phase should be refined");

        assert_eq!(problem.units().len(), 2);
        assert_eq!(
            problem
                .units()
                .iter()
                .map(|unit| unit.unit_id().as_str())
                .collect::<Vec<_>>(),
            vec!["phase-mixed::dest-mill", "phase-mixed::dest-waste"]
        );
    }

    #[test]
    fn adapter_promotes_representable_staged_stockpile_parcel() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            40.0,
            10.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 1_000.0)]),
        )
        .expect("mill should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![mill, stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-stockpile", vec![0], 1_000.0, vec![])],
            1_000.0,
        );

        let problem = build_scheduling_problem_from_economic_block_model(
            ScenarioId::new("mr201-stockpile").expect("scenario should be valid"),
            ModelId::new("mr201-stockpile-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_and_stockpile_limits(true, "sp"),
            vec![
                LongTermScheduleStockpile::new(
                    ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
                    1_000.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
        )
        .expect("representable staged stockpile parcel should build");

        assert_eq!(problem.units().len(), 1);
        let unit = &problem.units()[0];
        assert_eq!(unit.unit_id().as_str(), "phase-stockpile::stockpile-sp");
        assert_eq!(unit.eligible_destination_ids()[0].as_str(), "mill");
        assert_eq!(unit.eligible_stockpile_ids()[0].as_str(), "sp");
        assert_eq!(unit.stockpile_inventory_delta_tonnage(), Some(-1_000.0));
        assert_eq!(
            problem.periods()[0]
                .resource_bounds()
                .iter()
                .find(|bound| { bound.resource_id().as_str() == "stockpile_reclaim_capacity::sp" })
                .expect("reclaim capacity resource bound should exist")
                .max_total(),
            Some(1_000.0)
        );
        assert_close(
            objective_term_value(&problem, "phase-stockpile::stockpile-sp", None),
            -10_000.0,
        );
        assert_close(
            objective_term_value(&problem, "phase-stockpile::stockpile-sp", Some("mill")),
            100_000.0,
        );
        assert_close(
            metadata_float(unit, "economics.mining_cost_carryover"),
            40_000.0,
        );
        assert_close(
            metadata_float(unit, "economics.reclaim_downstream_cost"),
            10_000.0,
        );
        assert_close(
            metadata_float(unit, "economics.mining_cost_carryover_per_tonne"),
            40.0,
        );
        assert_close(
            metadata_float(unit, "economics.reclaim_downstream_cost_per_tonne"),
            10.0,
        );
        assert!(
            problem
                .resource_requirements()
                .iter()
                .all(|requirement| !(requirement.unit_id() == unit.unit_id()
                    && requirement.resource_id().as_str() == "mine_tonnage"))
        );
        assert_close(
            resource_requirement_amount(
                &problem,
                "phase-stockpile::stockpile-sp",
                "plant_tonnage",
                Some("mill"),
            ),
            1_000.0,
        );
        assert_close(
            resource_requirement_amount(
                &problem,
                "phase-stockpile::stockpile-sp",
                "destination_capacity::mill",
                Some("mill"),
            ),
            1_000.0,
        );
        assert_close(
            resource_requirement_amount(
                &problem,
                "phase-stockpile::stockpile-sp",
                "stockpile_reclaim_capacity::sp",
                None,
            ),
            1_000.0,
        );
        assert!(
            problem.limitations().iter().any(|item| item.contains(
                "Representable staged stockpile parcels are promoted as reclaim/direct-feed scheduling units"
            ))
        );

        let schedule = build_ready_frontier_long_term_schedule(&problem, None, Metadata::new())
            .expect("reclaim schedule should build");
        assert_eq!(schedule.entries().len(), 1);
        assert_eq!(
            schedule.entries()[0]
                .phase_id()
                .expect("entry should preserve promoted unit lineage"),
            "phase-stockpile::stockpile-sp"
        );
        assert_eq!(
            schedule.entries()[0]
                .destination_id()
                .expect("entry should expose its reclaim destination")
                .as_str(),
            "mill"
        );
        assert_eq!(
            schedule.entries()[0]
                .reclaim_stockpile_id()
                .expect("entry should expose its reclaim stockpile")
                .as_str(),
            "sp"
        );

        let report =
            evaluate_long_term_schedule_economics(&schedule, &phase_plan, &economic_model, 0.10)
                .expect("economic report should support promoted staged parcels");
        assert_eq!(report.periods.len(), 2);
        assert_eq!(report.periods[0].period_label, "P1");
        assert_eq!(
            report.periods[0].phase_ids,
            vec!["phase-stockpile::stockpile-sp".to_owned()]
        );
        assert_close(report.periods[0].tonnage, 1_000.0);
        assert_close(report.periods[0].revenue, 100_000.0);
        assert_close(report.periods[0].cost, 50_000.0);
        assert_close(report.periods[0].cashflow, 50_000.0);
        assert_close(report.periods[0].destination_tonnage["mill"], 1_000.0);
        assert_close(report.periods[0].payable_metal["cu"], 100.0);
        assert_eq!(report.periods[1].phase_ids, Vec::<String>::new());
        assert_close(report.total_revenue, 100_000.0);
        assert_close(report.total_cost, 50_000.0);
        assert_close(report.total_cashflow, 50_000.0);
        assert_close(report.destination_tonnage["mill"], 1_000.0);
        assert_close(report.payable_metal["cu"], 100.0);
    }

    #[test]
    fn adapter_uses_reclaim_policy_for_ambiguous_staged_stockpile_parcel() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            40.0,
            10.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 1_000.0)]),
        )
        .expect("mill should be valid");
        let leach = DestinationAssumptions::new(
            DestinationId::new("leach").expect("id should be valid"),
            DestinationKind::Leach,
            150.0,
            20.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_000.0)]),
        )
        .expect("leach should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10, 0.60], vec![500.0, 500.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![mill, leach, stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-stockpile", vec![0, 1], 1_000.0, vec![])],
            1_000.0,
        );
        let reclaim_policy = StagedStockpileReclaimPolicy::new(vec![
            StagedStockpileReclaimRule::new(
                DestinationId::new("sp").expect("stockpile id should be valid"),
                DestinationId::new("mill").expect("destination id should be valid"),
                StagedStockpileReclaimDownstreamProfile::EconomicDestination,
            )
            .expect("policy rule should be valid"),
        ])
        .expect("policy should be valid");

        let problem = build_scheduling_problem_from_economic_block_model_with_reclaim_policy(
            ScenarioId::new("mr201-stockpile-policy").expect("scenario should be valid"),
            ModelId::new("mr201-stockpile-policy-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_and_stockpile_limits(false, "sp"),
            vec![
                LongTermScheduleStockpile::new(
                    ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
                    1_000.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
            Some(&reclaim_policy),
        )
        .expect("policy should resolve the ambiguous reclaim route");

        let unit = &problem.units()[0];
        assert_eq!(unit.eligible_destination_ids()[0].as_str(), "mill");
        assert_eq!(
            metadata_text(unit, "economics.reclaim_route_source"),
            "policy"
        );
        assert_eq!(
            metadata_text(unit, "economics.reclaim_downstream_profile"),
            "economic-destination"
        );

        let schedule = build_ready_frontier_long_term_schedule(&problem, None, Metadata::new())
            .expect("schedule should build with policy");
        let report = evaluate_long_term_schedule_economics_with_reclaim_policy(
            &schedule,
            &phase_plan,
            &economic_model,
            0.10,
            Some(&reclaim_policy),
        )
        .expect("economic report should reuse the same reclaim policy");
        assert_close(report.destination_tonnage["mill"], 1_000.0);
    }

    #[test]
    fn build_representable_staged_parcel_uses_policy_when_reclaim_destination_is_missing() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            40.0,
            10.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 1_000.0)]),
        )
        .expect("mill should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![mill, stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let row_by_linear_index = crate::schedule_economics::build_row_index_map(&economic_model)
            .expect("row map should build");
        let grade_columns = crate::schedule_economics::build_grade_column_slices(&economic_model)
            .expect("grade slices should build");
        let parcel = crate::StockpileTargetParcel {
            staged_parcel_id: "phase-stockpile::stockpile-sp".to_owned(),
            parent_phase_id: "phase-stockpile".to_owned(),
            source_destination_id: DestinationId::new("sp")
                .expect("source destination id should be valid"),
            source_destination_kind: DestinationKind::Stockpile,
            stockpile_id: DestinationId::new("sp").expect("stockpile id should be valid"),
            block_indices: vec![0],
            total_tonnage: 1_000.0,
            revenue: 0.0,
            cost: 0.0,
            payable_metal: BTreeMap::new(),
            reclaim_inventory_delta_tonnage: -1_000.0,
            reclaim_destination_id: None,
            reclaim_destination_kind: None,
            reclaim_promotion_limitations: vec![
                "no eligible non-stockpile feed destination".to_owned(),
            ],
        };
        let reclaim_policy = StagedStockpileReclaimPolicy::new(vec![
            StagedStockpileReclaimRule::new(
                DestinationId::new("sp").expect("stockpile id should be valid"),
                DestinationId::new("mill").expect("destination id should be valid"),
                StagedStockpileReclaimDownstreamProfile::EconomicDestination,
            )
            .expect("policy rule should be valid"),
        ])
        .expect("policy should be valid");

        let representable = super::build_representable_staged_parcel(
            &parcel,
            1,
            &economic_model,
            &row_by_linear_index,
            &grade_columns,
            Some(&reclaim_policy),
        )
        .expect("policy should supply the missing reclaim route");

        assert_eq!(representable.destination_id.as_str(), "mill");
        assert_eq!(
            representable.reclaim_route_source,
            super::ReclaimRouteSource::Policy
        );
    }

    #[test]
    fn reclaim_policy_rejects_missing_destination_for_ambiguous_staged_parcel() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            40.0,
            10.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 1_000.0)]),
        )
        .expect("mill should be valid");
        let leach = DestinationAssumptions::new(
            DestinationId::new("leach").expect("id should be valid"),
            DestinationKind::Leach,
            150.0,
            20.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_000.0)]),
        )
        .expect("leach should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10, 0.60], vec![500.0, 500.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![mill, leach, stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-stockpile", vec![0, 1], 1_000.0, vec![])],
            1_000.0,
        );
        let reclaim_policy = StagedStockpileReclaimPolicy::new(vec![
            StagedStockpileReclaimRule::new(
                DestinationId::new("sp").expect("stockpile id should be valid"),
                DestinationId::new("crusher").expect("destination id should be valid"),
                StagedStockpileReclaimDownstreamProfile::EconomicDestination,
            )
            .expect("policy rule should be valid"),
        ])
        .expect("policy should be valid");

        let error = build_scheduling_problem_from_economic_block_model_with_reclaim_policy(
            ScenarioId::new("mr201-stockpile-policy-missing").expect("scenario should be valid"),
            ModelId::new("mr201-stockpile-policy-missing-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_and_stockpile_limits(false, "sp"),
            vec![
                LongTermScheduleStockpile::new(
                    ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
                    1_000.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
            Some(&reclaim_policy),
        )
        .expect_err("policy should reject unknown destinations");

        assert!(format!("{error}").contains("missing from the economic assumptions"));
    }

    #[test]
    fn reclaim_policy_rejects_conflicts_with_unique_inference() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            40.0,
            10.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 1_000.0)]),
        )
        .expect("mill should be valid");
        let leach = DestinationAssumptions::new(
            DestinationId::new("leach").expect("id should be valid"),
            DestinationKind::Leach,
            150.0,
            20.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_000.0)]),
        )
        .expect("leach should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![mill, leach, stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-stockpile", vec![0], 1_000.0, vec![])],
            1_000.0,
        );
        let reclaim_policy = StagedStockpileReclaimPolicy::new(vec![
            StagedStockpileReclaimRule::new(
                DestinationId::new("sp").expect("stockpile id should be valid"),
                DestinationId::new("leach").expect("destination id should be valid"),
                StagedStockpileReclaimDownstreamProfile::EconomicDestination,
            )
            .expect("policy rule should be valid"),
        ])
        .expect("policy should be valid");

        let error = build_scheduling_problem_from_economic_block_model_with_reclaim_policy(
            ScenarioId::new("mr201-stockpile-policy-conflict").expect("scenario should be valid"),
            ModelId::new("mr201-stockpile-policy-conflict-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_and_stockpile_limits(false, "sp"),
            vec![
                LongTermScheduleStockpile::new(
                    ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
                    1_000.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
            Some(&reclaim_policy),
        )
        .expect_err("conflicting policy should fail explicitly");

        assert!(
            format!("{error}").contains("conflicts with the uniquely inferred reclaim destination")
        );
    }

    #[test]
    fn adapter_rejects_staged_stockpile_parcel_without_non_stockpile_feed_destination() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-stockpile", vec![0], 1_000.0, vec![])],
            1_000.0,
        );

        let error = build_scheduling_problem_from_economic_block_model(
            ScenarioId::new("mr201-stockpile-no-feed").expect("scenario should be valid"),
            ModelId::new("mr201-stockpile-no-feed-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_and_stockpile_limits(false, "sp"),
            vec![
                LongTermScheduleStockpile::new(
                    ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
                    1_000.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
        )
        .expect_err("unsupported reclaim routing should stay explicit");

        let message = format!("{error}");
        assert!(message.contains("stockpile-target parcel"));
        assert!(message.contains("phase-stockpile::stockpile-sp"));
        assert!(message.contains("stockpile `sp`"));
        assert!(message.contains("no eligible non-stockpile feed destination"));
    }

    #[test]
    fn adapter_rejects_unrepresentable_staged_stockpile_parcel() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            40.0,
            10.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 1_000.0)]),
        )
        .expect("mill should be valid");
        let leach = DestinationAssumptions::new(
            DestinationId::new("leach").expect("id should be valid"),
            DestinationKind::Leach,
            150.0,
            20.0,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_000.0)]),
        )
        .expect("leach should be valid");
        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            1.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 1.0).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 2_500.0)]),
        )
        .expect("stockpile should be valid");
        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10, 0.60], vec![500.0, 500.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: DestinationAssumptionSet::new(vec![mill, leach, stockpile, waste])
                    .expect("destination set should be valid"),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![single_phase("phase-stockpile", vec![0, 1], 1_000.0, vec![])],
            1_000.0,
        );

        let error = build_scheduling_problem_from_economic_block_model(
            ScenarioId::new("mr201-stockpile-unsupported").expect("scenario should be valid"),
            ModelId::new("mr201-stockpile-unsupported-model").expect("model should be valid"),
            &phase_plan,
            capacities_with_mill_and_stockpile_limits(false, "sp"),
            vec![
                LongTermScheduleStockpile::new(
                    ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
                    1_000.0,
                    Metadata::new(),
                )
                .expect("stockpile should be valid"),
            ],
            &economic_model,
            0.10,
            Metadata::new(),
        )
        .expect_err("ambiguous reclaim routing should stay explicit");

        let message = format!("{error}");
        assert!(message.contains("stockpile-target parcel"));
        assert!(message.contains("phase-stockpile::stockpile-sp"));
        assert!(message.contains("stockpile `sp`"));
        assert!(message.contains("multiple candidate non-stockpile feed destinations"));
        assert!(message.contains("leach"));
        assert!(message.contains("mill"));
    }

    #[test]
    fn adapter_rejects_reclaim_cost_split_without_positive_tonnage() {
        let error = super::reclaim_cost_split_metadata(&super::RepresentableStagedParcel {
            staged_parcel_id: "phase-stockpile::stockpile-sp".to_owned(),
            parent_phase_id: "phase-stockpile".to_owned(),
            stockpile_id: ScheduleStockpileId::new("sp").expect("stockpile id should be valid"),
            destination_id: ScheduleDestinationId::new("mill")
                .expect("destination id should be valid"),
            destination_kind: DestinationKind::Mill,
            total_tonnage: 0.0,
            block_indices: vec![0],
            revenue: 100_000.0,
            payable_metal: BTreeMap::new(),
            mining_cost_carryover: 40_000.0,
            downstream_cost: 10_000.0,
            stockpile_inventory_delta_tonnage: -1_000.0,
            reclaim_route_source: super::ReclaimRouteSource::Inference,
            reclaim_downstream_profile:
                StagedStockpileReclaimDownstreamProfile::EconomicDestination,
        })
        .expect_err("zero-tonnage reclaim accounting should stay explicit");

        let message = format!("{error}");
        assert!(message.contains("phase-stockpile::stockpile-sp"));
        assert!(message.contains("stockpile `sp`"));
        assert!(message.contains("destination `mill`"));
        assert!(message.contains("positive finite promoted tonnage"));
    }
}
