use std::collections::{BTreeMap, BTreeSet};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

/// Entrada elemental de schedule para un periodo, banco y fase opcional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    period_label: String,
    bench: i64,
    tonnage: f64,
    block_count: usize,
    phase: Option<String>,
}

impl ScheduleEntry {
    /// Construye una entrada de schedule validando periodo, tonelaje y fase opcional.
    pub fn new(
        period_label: impl Into<String>,
        bench: i64,
        tonnage: f64,
        block_count: usize,
        phase: Option<String>,
    ) -> Result<Self, MineError> {
        let period_label = period_label.into();

        if period_label.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "period_label",
                "schedule period label must not be empty",
            ));
        }

        if !tonnage.is_finite() || tonnage <= 0.0 {
            return Err(MineError::invalid_parameter(
                "tonnage",
                "schedule tonnage must be finite and greater than zero",
            ));
        }

        if block_count == 0 {
            return Err(MineError::invalid_parameter(
                "block_count",
                "schedule block count must be greater than zero",
            ));
        }

        if let Some(phase) = &phase
            && phase.trim().is_empty()
        {
            return Err(MineError::invalid_parameter(
                "phase",
                "schedule phase must not be empty when provided",
            ));
        }

        Ok(Self {
            period_label,
            bench,
            tonnage,
            block_count,
            phase,
        })
    }

    /// Periodo al que pertenece la entrada.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Banco asociado a la entrada.
    #[must_use]
    pub const fn bench(&self) -> i64 {
        self.bench
    }

    /// Tonelaje asignado en la entrada.
    #[must_use]
    pub const fn tonnage(&self) -> f64 {
        self.tonnage
    }

    /// Bloques representados por la entrada.
    #[must_use]
    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    /// Fase opcional asociada a la entrada.
    #[must_use]
    pub fn phase(&self) -> Option<&str> {
        self.phase.as_deref()
    }
}

/// Restricciones básicas aplicables a un schedule.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScheduleConstraints {
    max_period_tonnage: Option<f64>,
    max_vertical_advance: Option<i64>,
}

impl ScheduleConstraints {
    /// Construye restricciones validadas para un schedule.
    pub fn new(
        max_period_tonnage: Option<f64>,
        max_vertical_advance: Option<i64>,
    ) -> Result<Self, MineError> {
        if let Some(max_period_tonnage) = max_period_tonnage
            && (!max_period_tonnage.is_finite() || max_period_tonnage <= 0.0)
        {
            return Err(MineError::invalid_parameter(
                "max_period_tonnage",
                "schedule max period tonnage must be finite and greater than zero",
            ));
        }

        if let Some(max_vertical_advance) = max_vertical_advance
            && max_vertical_advance <= 0
        {
            return Err(MineError::invalid_parameter(
                "max_vertical_advance",
                "schedule max vertical advance must be greater than zero",
            ));
        }

        Ok(Self {
            max_period_tonnage,
            max_vertical_advance,
        })
    }

    /// Tonelaje máximo permitido por periodo.
    #[must_use]
    pub const fn max_period_tonnage(&self) -> Option<f64> {
        self.max_period_tonnage
    }

    /// Máximo avance vertical permitido entre periodos consecutivos.
    #[must_use]
    pub const fn max_vertical_advance(&self) -> Option<i64> {
        self.max_vertical_advance
    }
}

/// Resumen agregado por periodo dentro de un schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulePeriodSummary {
    /// Etiqueta del periodo.
    pub period_label: String,
    /// Tonelaje total asignado al periodo.
    pub total_tonnage: f64,
    /// Bloques totales asignados al periodo.
    pub total_blocks: usize,
    /// Bancos activos dentro del periodo.
    pub benches: Vec<i64>,
    /// Fases activas dentro del periodo.
    pub phases: Vec<String>,
}

/// Código estable de violación detectada en un schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleViolationCode {
    /// El tonelaje del periodo excede el máximo permitido.
    ExceedsPeriodTonnage,
    /// El avance vertical entre periodos excede el máximo permitido.
    ExceedsVerticalAdvance,
}

/// Violación estructurada detectada al construir o validar un schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleViolation {
    /// Código estable de la violación.
    pub code: ScheduleViolationCode,
    /// Periodo donde se detectó la violación.
    pub period_label: String,
    /// Fase asociada cuando aplica.
    pub phase: Option<String>,
    /// Banco asociado cuando aplica.
    pub bench: Option<i64>,
    /// Límite configurado que se violó.
    pub limit_value: f64,
    /// Valor observado durante la validación.
    pub observed_value: f64,
    /// Mensaje legible para humanos.
    pub message: String,
}

/// Schedule determinista con resúmenes y violaciones estructuradas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    entries: Vec<ScheduleEntry>,
    constraints: ScheduleConstraints,
    period_summaries: Vec<SchedulePeriodSummary>,
    violations: Vec<ScheduleViolation>,
}

impl Schedule {
    /// Entradas discretas del schedule.
    #[must_use]
    pub fn entries(&self) -> &[ScheduleEntry] {
        &self.entries
    }

    /// Restricciones aplicadas al schedule.
    #[must_use]
    pub const fn constraints(&self) -> &ScheduleConstraints {
        &self.constraints
    }

    /// Resúmenes agregados por periodo.
    #[must_use]
    pub fn period_summaries(&self) -> &[SchedulePeriodSummary] {
        &self.period_summaries
    }

    /// Violaciones detectadas durante la construcción o validación.
    #[must_use]
    pub fn violations(&self) -> &[ScheduleViolation] {
        &self.violations
    }
}

/// Valida avance vertical máximo entre periodos consecutivos por fase opcional.
pub fn validate_vertical_advance(
    entries: &[ScheduleEntry],
    max_vertical_advance: i64,
) -> Result<Vec<ScheduleViolation>, MineError> {
    if max_vertical_advance <= 0 {
        return Err(MineError::invalid_parameter(
            "max_vertical_advance",
            "schedule max vertical advance must be greater than zero",
        ));
    }

    Ok(build_vertical_advance_violations(
        entries,
        max_vertical_advance,
    ))
}

/// Construye un schedule con resúmenes agregados y violaciones de restricciones básicas.
pub fn build_schedule(
    entries: Vec<ScheduleEntry>,
    constraints: ScheduleConstraints,
) -> Result<Schedule, MineError> {
    if entries.is_empty() {
        return Err(MineError::invalid_parameter(
            "entries",
            "schedule must contain at least one entry",
        ));
    }

    let period_summaries = build_period_summaries(&entries);
    let mut violations = Vec::new();

    if let Some(max_period_tonnage) = constraints.max_period_tonnage() {
        for summary in &period_summaries {
            if summary.total_tonnage > max_period_tonnage {
                violations.push(ScheduleViolation {
                    code: ScheduleViolationCode::ExceedsPeriodTonnage,
                    period_label: summary.period_label.clone(),
                    phase: None,
                    bench: None,
                    limit_value: max_period_tonnage,
                    observed_value: summary.total_tonnage,
                    message: format!(
                        "period `{}` totals {} t, exceeding the configured limit of {} t",
                        summary.period_label, summary.total_tonnage, max_period_tonnage
                    ),
                });
            }
        }
    }

    if let Some(max_vertical_advance) = constraints.max_vertical_advance() {
        violations.extend(build_vertical_advance_violations(
            &entries,
            max_vertical_advance,
        ));
    }

    Ok(Schedule {
        entries,
        constraints,
        period_summaries,
        violations,
    })
}

fn build_period_summaries(entries: &[ScheduleEntry]) -> Vec<SchedulePeriodSummary> {
    let mut period_order = Vec::<String>::new();
    let mut summaries = BTreeMap::<String, (f64, usize, BTreeSet<i64>, BTreeSet<String>)>::new();

    for entry in entries {
        if !summaries.contains_key(entry.period_label()) {
            period_order.push(entry.period_label().to_owned());
            summaries.insert(
                entry.period_label().to_owned(),
                (0.0, 0, BTreeSet::new(), BTreeSet::new()),
            );
        }

        let summary = summaries
            .get_mut(entry.period_label())
            .expect("period summary should exist");
        summary.0 += entry.tonnage();
        summary.1 += entry.block_count();
        summary.2.insert(entry.bench());

        if let Some(phase) = entry.phase() {
            summary.3.insert(phase.to_owned());
        }
    }

    period_order
        .into_iter()
        .map(|period_label| {
            let (total_tonnage, total_blocks, benches, phases) = summaries
                .remove(&period_label)
                .expect("period summary should exist");

            SchedulePeriodSummary {
                period_label,
                total_tonnage,
                total_blocks,
                benches: benches.into_iter().collect(),
                phases: phases.into_iter().collect(),
            }
        })
        .collect()
}

fn build_vertical_advance_violations(
    entries: &[ScheduleEntry],
    max_vertical_advance: i64,
) -> Vec<ScheduleViolation> {
    let mut period_order = Vec::<String>::new();
    let mut max_bench_by_period_phase = BTreeMap::<(String, Option<String>), i64>::new();

    for entry in entries {
        if !period_order
            .iter()
            .any(|period| period == entry.period_label())
        {
            period_order.push(entry.period_label().to_owned());
        }

        let key = (
            entry.period_label().to_owned(),
            entry.phase().map(ToOwned::to_owned),
        );

        max_bench_by_period_phase
            .entry(key)
            .and_modify(|bench| *bench = (*bench).max(entry.bench()))
            .or_insert(entry.bench());
    }

    let phase_groups = max_bench_by_period_phase
        .keys()
        .map(|(_, phase)| phase.clone())
        .collect::<BTreeSet<_>>();
    let mut violations = Vec::new();

    for phase in phase_groups {
        let mut previous: Option<(String, i64)> = None;

        for period_label in &period_order {
            let Some(current_bench) =
                max_bench_by_period_phase.get(&(period_label.clone(), phase.clone()))
            else {
                continue;
            };

            if let Some((previous_period, previous_bench)) = &previous {
                let advance = *current_bench - *previous_bench;

                if advance > max_vertical_advance {
                    violations.push(ScheduleViolation {
                        code: ScheduleViolationCode::ExceedsVerticalAdvance,
                        period_label: period_label.clone(),
                        phase: phase.clone(),
                        bench: Some(*current_bench),
                        limit_value: max_vertical_advance as f64,
                        observed_value: advance as f64,
                        message: format!(
                            "vertical advance from period `{previous_period}` to `{period_label}` reached {advance} benches, exceeding the configured limit of {max_vertical_advance}"
                        ),
                    });
                }
            }

            previous = Some((period_label.clone(), *current_bench));
        }
    }

    violations
}
