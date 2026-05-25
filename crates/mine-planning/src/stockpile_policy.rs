//! Politicas explicitas de stockpile y reclaim sobre `LongTermSchedule`.
//!
//! Este modulo no resuelve una optimizacion de stockpiles. Su objetivo es
//! permitir politicas deterministas y configurables de:
//! - desvio de tonelaje desde un destino directo hacia un stockpile;
//! - reclaim posterior desde stockpile hacia un destino final;
//! - validacion de balances e inventarios usando el mismo contrato serializable
//!   del scheduler de largo plazo.
//!
//! # References
//! - Moreno, E., Rezakhah, M., Newman, A. M., Ferreira, F. C. L. (2017).
//!   *Linear models for stockpiling in open-pit mine production scheduling problems*.
//!   <https://doi.org/10.1016/j.ejor.2016.12.014>
//! - Rezakhah, M., Newman, A. M. (2020).
//!   *Open pit mine planning with degradation due to stockpiling*.
//!   <https://doi.org/10.1016/j.cor.2018.11.009>

use std::collections::{BTreeMap, BTreeSet};

use mine_core::{Metadata, MineError};
use serde::{Deserialize, Serialize};

use crate::long_term_schedule::{
    LongTermSchedule, LongTermScheduleEntry, LongTermScheduleStockpile, ScheduleDestinationId,
    ScheduleStockpileId, evaluate_long_term_schedule_material_flows,
};

const TONNAGE_TOLERANCE: f64 = 1.0e-9;

fn validate_period_label(value: impl Into<String>) -> Result<String, MineError> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(MineError::invalid_parameter(
            "period_label",
            "must not be empty or whitespace only",
        ));
    }
    if value.trim() != value {
        return Err(MineError::invalid_parameter(
            "period_label",
            "must not contain leading or trailing whitespace",
        ));
    }
    Ok(value)
}

fn validate_positive_tonnage(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than zero",
        ));
    }
    Ok(())
}

/// Politica de deposito desde un destino directo hacia un stockpile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermStockpileDepositPolicy {
    period_label: String,
    source_destination_id: ScheduleDestinationId,
    stockpile_id: ScheduleStockpileId,
    tonnage: f64,
}

impl LongTermStockpileDepositPolicy {
    /// Construye una regla de deposito validada.
    pub fn new(
        period_label: impl Into<String>,
        source_destination_id: ScheduleDestinationId,
        stockpile_id: ScheduleStockpileId,
        tonnage: f64,
    ) -> Result<Self, MineError> {
        validate_positive_tonnage("tonnage", tonnage)?;
        Ok(Self {
            period_label: validate_period_label(period_label)?,
            source_destination_id,
            stockpile_id,
            tonnage,
        })
    }

    /// Periodo donde se aplica el desvio.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Destino directo que se desviara al stockpile.
    #[must_use]
    pub fn source_destination_id(&self) -> &ScheduleDestinationId {
        &self.source_destination_id
    }

    /// Stockpile receptor del tonelaje desviado.
    #[must_use]
    pub fn stockpile_id(&self) -> &ScheduleStockpileId {
        &self.stockpile_id
    }

    /// Tonelaje a desviar en el periodo.
    #[must_use]
    pub const fn tonnage(&self) -> f64 {
        self.tonnage
    }
}

/// Politica de reclaim desde stockpile hacia un destino final.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermStockpileReclaimPolicy {
    period_label: String,
    stockpile_id: ScheduleStockpileId,
    destination_id: ScheduleDestinationId,
    tonnage: f64,
}

impl LongTermStockpileReclaimPolicy {
    /// Construye una regla de reclaim validada.
    pub fn new(
        period_label: impl Into<String>,
        stockpile_id: ScheduleStockpileId,
        destination_id: ScheduleDestinationId,
        tonnage: f64,
    ) -> Result<Self, MineError> {
        validate_positive_tonnage("tonnage", tonnage)?;
        Ok(Self {
            period_label: validate_period_label(period_label)?,
            stockpile_id,
            destination_id,
            tonnage,
        })
    }

    /// Periodo donde ocurre el reclaim.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Stockpile fuente del reclaim.
    #[must_use]
    pub fn stockpile_id(&self) -> &ScheduleStockpileId {
        &self.stockpile_id
    }

    /// Destino final del material reclaimado.
    #[must_use]
    pub fn destination_id(&self) -> &ScheduleDestinationId {
        &self.destination_id
    }

    /// Tonelaje reclaimado en el periodo.
    #[must_use]
    pub const fn tonnage(&self) -> f64 {
        self.tonnage
    }
}

/// Politica completa de stockpile aplicada sobre un `LongTermSchedule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermStockpilePolicy {
    deposit_policies: Vec<LongTermStockpileDepositPolicy>,
    reclaim_policies: Vec<LongTermStockpileReclaimPolicy>,
}

impl LongTermStockpilePolicy {
    /// Construye una politica validando duplicados y referencias internas.
    pub fn new(
        deposit_policies: Vec<LongTermStockpileDepositPolicy>,
        reclaim_policies: Vec<LongTermStockpileReclaimPolicy>,
    ) -> Result<Self, MineError> {
        let mut seen_deposits = BTreeSet::new();
        for deposit in &deposit_policies {
            let key = (
                deposit.period_label().to_owned(),
                deposit.source_destination_id().clone(),
                deposit.stockpile_id().clone(),
            );
            if !seen_deposits.insert(key) {
                return Err(MineError::validation(format!(
                    "duplicate stockpile deposit policy for period `{}` source destination `{}` and stockpile `{}`",
                    deposit.period_label(),
                    deposit.source_destination_id(),
                    deposit.stockpile_id()
                )));
            }
        }

        let mut seen_reclaims = BTreeSet::new();
        for reclaim in &reclaim_policies {
            let key = (
                reclaim.period_label().to_owned(),
                reclaim.stockpile_id().clone(),
                reclaim.destination_id().clone(),
            );
            if !seen_reclaims.insert(key) {
                return Err(MineError::validation(format!(
                    "duplicate stockpile reclaim policy for period `{}` stockpile `{}` and destination `{}`",
                    reclaim.period_label(),
                    reclaim.stockpile_id(),
                    reclaim.destination_id()
                )));
            }
        }

        Ok(Self {
            deposit_policies,
            reclaim_policies,
        })
    }

    /// Reglas de deposito configuradas.
    #[must_use]
    pub fn deposit_policies(&self) -> &[LongTermStockpileDepositPolicy] {
        &self.deposit_policies
    }

    /// Reglas de reclaim configuradas.
    #[must_use]
    pub fn reclaim_policies(&self) -> &[LongTermStockpileReclaimPolicy] {
        &self.reclaim_policies
    }
}

/// Aplica una politica explicita de stockpile sobre un `LongTermSchedule`.
pub fn apply_long_term_stockpile_policy(
    schedule: &LongTermSchedule,
    policy: &LongTermStockpilePolicy,
    metadata: Metadata,
) -> Result<LongTermSchedule, MineError> {
    validate_policy_against_schedule(schedule, policy)?;

    let mut entries_by_period = schedule
        .capacities()
        .iter()
        .map(|capacity| {
            (
                capacity.period_label().to_owned(),
                schedule
                    .entries()
                    .iter()
                    .filter(|entry| entry.period_label() == capacity.period_label())
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for deposit_policy in policy.deposit_policies() {
        let period_entries = entries_by_period
            .get_mut(deposit_policy.period_label())
            .ok_or_else(|| {
                MineError::validation(format!(
                    "stockpile policy references unknown period `{}`",
                    deposit_policy.period_label()
                ))
            })?;
        apply_deposit_policy(period_entries, deposit_policy)?;
    }

    let mut entries = Vec::new();
    for capacity in schedule.capacities() {
        let period_label = capacity.period_label();
        let mut period_entries = entries_by_period.remove(period_label).unwrap_or_default();
        entries.append(&mut period_entries);
        entries.extend(
            policy
                .reclaim_policies()
                .iter()
                .filter(|reclaim| reclaim.period_label() == period_label)
                .map(build_reclaim_entry)
                .collect::<Result<Vec<_>, _>>()?,
        );
    }

    let tentative = LongTermSchedule::new(
        schedule.scenario_id().clone(),
        schedule.model_id().clone(),
        entries.clone(),
        schedule.capacities().to_vec(),
        schedule.stockpiles().to_vec(),
        schedule.violations().to_vec(),
        metadata.clone(),
    )?;
    let flow_report = evaluate_long_term_schedule_material_flows(&tentative)?;

    LongTermSchedule::new(
        schedule.scenario_id().clone(),
        schedule.model_id().clone(),
        entries,
        schedule.capacities().to_vec(),
        schedule.stockpiles().to_vec(),
        flow_report.violations,
        metadata,
    )
}

fn validate_policy_against_schedule(
    schedule: &LongTermSchedule,
    policy: &LongTermStockpilePolicy,
) -> Result<(), MineError> {
    let known_periods = schedule
        .capacities()
        .iter()
        .map(|capacity| capacity.period_label().to_owned())
        .collect::<BTreeSet<_>>();
    let known_stockpiles = schedule
        .stockpiles()
        .iter()
        .map(LongTermScheduleStockpile::stockpile_id)
        .cloned()
        .collect::<BTreeSet<_>>();
    let known_destinations = schedule
        .capacities()
        .iter()
        .flat_map(|capacity| capacity.destination_capacities().iter())
        .map(|capacity| capacity.destination_id().clone())
        .chain(
            schedule
                .entries()
                .iter()
                .filter_map(LongTermScheduleEntry::destination_id)
                .cloned(),
        )
        .collect::<BTreeSet<_>>();

    for deposit in policy.deposit_policies() {
        if !known_periods.contains(deposit.period_label()) {
            return Err(MineError::validation(format!(
                "stockpile deposit policy references unknown period `{}`",
                deposit.period_label()
            )));
        }
        if !known_stockpiles.contains(deposit.stockpile_id()) {
            return Err(MineError::validation(format!(
                "stockpile deposit policy references unknown stockpile `{}`",
                deposit.stockpile_id()
            )));
        }
        if !known_destinations.contains(deposit.source_destination_id()) {
            return Err(MineError::validation(format!(
                "stockpile deposit policy references unknown source destination `{}`",
                deposit.source_destination_id()
            )));
        }
    }

    for reclaim in policy.reclaim_policies() {
        if !known_periods.contains(reclaim.period_label()) {
            return Err(MineError::validation(format!(
                "stockpile reclaim policy references unknown period `{}`",
                reclaim.period_label()
            )));
        }
        if !known_stockpiles.contains(reclaim.stockpile_id()) {
            return Err(MineError::validation(format!(
                "stockpile reclaim policy references unknown stockpile `{}`",
                reclaim.stockpile_id()
            )));
        }
        if !known_destinations.contains(reclaim.destination_id()) {
            return Err(MineError::validation(format!(
                "stockpile reclaim policy references unknown destination `{}`",
                reclaim.destination_id()
            )));
        }
    }

    Ok(())
}

fn apply_deposit_policy(
    entries: &mut Vec<LongTermScheduleEntry>,
    policy: &LongTermStockpileDepositPolicy,
) -> Result<(), MineError> {
    let available_tonnage = entries
        .iter()
        .filter(|entry| {
            entry.destination_id() == Some(policy.source_destination_id())
                && entry.reclaim_stockpile_id().is_none()
        })
        .map(LongTermScheduleEntry::tonnage)
        .sum::<f64>();
    if available_tonnage + TONNAGE_TOLERANCE < policy.tonnage() {
        return Err(MineError::validation(format!(
            "stockpile deposit policy requests {} t from destination `{}` in period `{}`, but only {} t are available",
            policy.tonnage(),
            policy.source_destination_id(),
            policy.period_label(),
            available_tonnage
        )));
    }

    let mut remaining_tonnage = policy.tonnage();
    let mut transformed_entries = Vec::with_capacity(entries.len() + 2);

    for entry in entries.drain(..) {
        if remaining_tonnage <= TONNAGE_TOLERANCE
            || entry.destination_id() != Some(policy.source_destination_id())
            || entry.reclaim_stockpile_id().is_some()
        {
            transformed_entries.push(entry);
            continue;
        }

        let diverted_tonnage = entry.tonnage().min(remaining_tonnage);
        let retained_tonnage = entry.tonnage() - diverted_tonnage;
        let diverted_block_count =
            allocate_diverted_block_count(entry.block_count(), diverted_tonnage, entry.tonnage());
        let retained_block_count = entry.block_count().saturating_sub(diverted_block_count);

        if retained_tonnage > TONNAGE_TOLERANCE {
            transformed_entries.push(LongTermScheduleEntry::new(
                entry.period_label(),
                entry.phase_id().map(ToOwned::to_owned),
                entry.shell_index(),
                entry.bench(),
                retained_tonnage,
                retained_block_count,
                entry.destination_id().cloned(),
                entry.stockpile_id().cloned(),
                entry.predecessor_phase_ids().to_vec(),
            )?);
        }

        transformed_entries.push(LongTermScheduleEntry::new(
            entry.period_label(),
            entry.phase_id().map(ToOwned::to_owned),
            entry.shell_index(),
            entry.bench(),
            diverted_tonnage,
            diverted_block_count,
            None,
            Some(policy.stockpile_id().clone()),
            entry.predecessor_phase_ids().to_vec(),
        )?);
        remaining_tonnage -= diverted_tonnage;
    }

    *entries = transformed_entries;
    Ok(())
}

fn allocate_diverted_block_count(
    total_blocks: usize,
    diverted_tonnage: f64,
    total_tonnage: f64,
) -> usize {
    if total_blocks == 0 || total_tonnage <= TONNAGE_TOLERANCE {
        return 0;
    }
    let diverted_fraction = (diverted_tonnage / total_tonnage).clamp(0.0, 1.0);
    ((total_blocks as f64) * diverted_fraction).round() as usize
}

fn build_reclaim_entry(
    policy: &LongTermStockpileReclaimPolicy,
) -> Result<LongTermScheduleEntry, MineError> {
    LongTermScheduleEntry::new_with_reclaim(
        policy.period_label(),
        None,
        None,
        None,
        policy.tonnage(),
        0,
        Some(policy.destination_id().clone()),
        None,
        Some(policy.stockpile_id().clone()),
        Vec::new(),
    )
}
