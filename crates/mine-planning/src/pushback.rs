use std::collections::{BTreeMap, BTreeSet};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::schedule::Schedule;

/// Reglas explícitas para derivar un prototipo de pushbacks desde un `Schedule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushbackGenerationRules {
    require_phase: bool,
    max_pushbacks: Option<usize>,
}

impl PushbackGenerationRules {
    /// Construye reglas validadas para un prototipo de pushbacks.
    pub fn new(require_phase: bool, max_pushbacks: Option<usize>) -> Result<Self, MineError> {
        if matches!(max_pushbacks, Some(0)) {
            return Err(MineError::invalid_parameter(
                "max_pushbacks",
                "pushback max_pushbacks must be greater than zero when provided",
            ));
        }

        Ok(Self {
            require_phase,
            max_pushbacks,
        })
    }

    /// Indica si todas las entradas del schedule deben tener fase.
    #[must_use]
    pub const fn require_phase(&self) -> bool {
        self.require_phase
    }

    /// Máximo opcional de pushbacks permitidos en el prototipo.
    #[must_use]
    pub const fn max_pushbacks(&self) -> Option<usize> {
        self.max_pushbacks
    }
}

/// Resumen determinista de un pushback derivado desde un `Schedule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushbackPrototype {
    /// Identificador estable del pushback dentro del prototipo.
    pub pushback_id: String,
    /// Fase fuente cuando existe.
    pub phase: Option<String>,
    /// Periodos donde el pushback aparece activo.
    pub periods: Vec<String>,
    /// Bancos observados dentro del pushback.
    pub benches: Vec<i64>,
    /// Tonelaje total representado por el pushback.
    pub total_tonnage: f64,
    /// Bloques totales representados por el pushback.
    pub total_blocks: usize,
}

/// Artefacto serializable que documenta el alcance actual del diseño de pushbacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushbackPrototypeReport {
    /// Reglas usadas para derivar el prototipo.
    pub rules: PushbackGenerationRules,
    /// Pushbacks derivados de forma determinista.
    pub pushbacks: Vec<PushbackPrototype>,
    /// Limitaciones conocidas de este prototipo.
    pub limitations: Vec<String>,
    /// Siguientes pasos sugeridos para evolucionar el diseño.
    pub next_steps: Vec<String>,
}

/// Deriva un prototipo de pushbacks agrupando entradas del schedule por fase.
pub fn build_pushback_prototype(
    schedule: &Schedule,
    rules: &PushbackGenerationRules,
) -> Result<PushbackPrototypeReport, MineError> {
    let mut grouped =
        BTreeMap::<Option<String>, (BTreeSet<String>, BTreeSet<i64>, f64, usize)>::new();

    for entry in schedule.entries() {
        let phase = entry.phase().map(ToOwned::to_owned);
        if rules.require_phase() && phase.is_none() {
            return Err(MineError::invalid_parameter(
                "schedule",
                "pushback prototype requires every schedule entry to declare a phase",
            ));
        }

        let group = grouped
            .entry(phase)
            .or_insert_with(|| (BTreeSet::new(), BTreeSet::new(), 0.0, 0));
        group.0.insert(entry.period_label().to_owned());
        group.1.insert(entry.bench());
        group.2 += entry.tonnage();
        group.3 += entry.block_count();
    }

    if let Some(max_pushbacks) = rules.max_pushbacks()
        && grouped.len() > max_pushbacks
    {
        return Err(MineError::invalid_parameter(
            "schedule",
            format!(
                "pushback prototype derived {} pushbacks, exceeding configured limit of {max_pushbacks}",
                grouped.len()
            ),
        ));
    }

    let pushbacks = grouped
        .into_iter()
        .enumerate()
        .map(
            |(index, (phase, (periods, benches, total_tonnage, total_blocks)))| {
                let phase_suffix = phase.clone().unwrap_or_else(|| "unphased".to_owned());

                PushbackPrototype {
                    pushback_id: format!("pushback-{:02}-{}", index + 1, phase_suffix),
                    phase,
                    periods: periods.into_iter().collect(),
                    benches: benches.into_iter().collect(),
                    total_tonnage,
                    total_blocks,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok(PushbackPrototypeReport {
        rules: rules.clone(),
        pushbacks,
        limitations: vec![
            "Prototype groups schedule entries by phase only; it does not optimize shells or economics.".to_owned(),
            "Entries without phase are either rejected or grouped as a single unphased pushback depending on the rules.".to_owned(),
            "Bench geometry, nested shells and precedence between pushbacks are not inferred yet.".to_owned(),
        ],
        next_steps: vec![
            "Introduce explicit shell or geometry inputs to separate pushbacks beyond a single phase label.".to_owned(),
            "Connect pushbacks with precedence and scenario evaluation instead of treating them as independent grouped summaries.".to_owned(),
            "Add deterministic validation for nested pushbacks and bench continuity.".to_owned(),
        ],
    })
}
