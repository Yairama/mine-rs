use std::collections::BTreeSet;

use mine_core::{ColumnId, Metadata, MineError, ModelId, ScenarioId};
use serde::{Deserialize, Serialize};

use crate::benches::BenchParameters;

/// Periodo discreto dentro de un escenario minero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPeriod {
    label: String,
    target_tonnage: Option<f64>,
    target_blocks: Option<usize>,
}

impl ScenarioPeriod {
    /// Construye un periodo validando nombre y targets opcionales.
    pub fn new(
        label: impl Into<String>,
        target_tonnage: Option<f64>,
        target_blocks: Option<usize>,
    ) -> Result<Self, MineError> {
        let label = label.into();

        if label.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "label",
                "scenario period label must not be empty",
            ));
        }

        if let Some(target_tonnage) = target_tonnage
            && (!target_tonnage.is_finite() || target_tonnage <= 0.0)
        {
            return Err(MineError::invalid_parameter(
                "target_tonnage",
                "scenario period target tonnage must be finite and greater than zero",
            ));
        }

        if matches!(target_blocks, Some(0)) {
            return Err(MineError::invalid_parameter(
                "target_blocks",
                "scenario period target blocks must be greater than zero",
            ));
        }

        Ok(Self {
            label,
            target_tonnage,
            target_blocks,
        })
    }

    /// Nombre legible del periodo.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Target de tonelaje opcional del periodo.
    #[must_use]
    pub const fn target_tonnage(&self) -> Option<f64> {
        self.target_tonnage
    }

    /// Target opcional de bloques del periodo.
    #[must_use]
    pub const fn target_blocks(&self) -> Option<usize> {
        self.target_blocks
    }
}

/// Reglas explícitas usadas para construir o interpretar un escenario.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRules {
    phase_column: Option<ColumnId>,
    bench_parameters: Option<BenchParameters>,
}

impl ScenarioRules {
    /// Construye reglas explícitas de escenario.
    #[must_use]
    pub fn new(phase_column: Option<ColumnId>, bench_parameters: Option<BenchParameters>) -> Self {
        Self {
            phase_column,
            bench_parameters,
        }
    }

    /// Columna categórica opcional que representa fases existentes.
    #[must_use]
    pub fn phase_column(&self) -> Option<&ColumnId> {
        self.phase_column.as_ref()
    }

    /// Parámetros opcionales para discretización por bancos.
    #[must_use]
    pub fn bench_parameters(&self) -> Option<&BenchParameters> {
        self.bench_parameters.as_ref()
    }
}

/// Restricciones explícitas de un escenario minero.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScenarioConstraints {
    max_vertical_advance: Option<f64>,
    max_active_phases: Option<usize>,
}

impl ScenarioConstraints {
    /// Construye restricciones validadas para un escenario.
    pub fn new(
        max_vertical_advance: Option<f64>,
        max_active_phases: Option<usize>,
    ) -> Result<Self, MineError> {
        if let Some(max_vertical_advance) = max_vertical_advance
            && (!max_vertical_advance.is_finite() || max_vertical_advance <= 0.0)
        {
            return Err(MineError::invalid_parameter(
                "max_vertical_advance",
                "scenario max vertical advance must be finite and greater than zero",
            ));
        }

        if matches!(max_active_phases, Some(0)) {
            return Err(MineError::invalid_parameter(
                "max_active_phases",
                "scenario max active phases must be greater than zero",
            ));
        }

        Ok(Self {
            max_vertical_advance,
            max_active_phases,
        })
    }

    /// Máximo avance vertical permitido por periodo.
    #[must_use]
    pub const fn max_vertical_advance(&self) -> Option<f64> {
        self.max_vertical_advance
    }

    /// Máximo de fases activas simultáneamente.
    #[must_use]
    pub const fn max_active_phases(&self) -> Option<usize> {
        self.max_active_phases
    }
}

/// Escenario minero serializable con referencias, reglas y restricciones explícitas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MiningScenario {
    scenario_id: ScenarioId,
    model_id: ModelId,
    periods: Vec<ScenarioPeriod>,
    rules: ScenarioRules,
    constraints: ScenarioConstraints,
    assumptions: Metadata,
}

impl MiningScenario {
    /// Construye un escenario validando periodos obligatorios y nombres únicos.
    pub fn new(
        scenario_id: ScenarioId,
        model_id: ModelId,
        periods: Vec<ScenarioPeriod>,
        rules: ScenarioRules,
        constraints: ScenarioConstraints,
        assumptions: Metadata,
    ) -> Result<Self, MineError> {
        if periods.is_empty() {
            return Err(MineError::invalid_parameter(
                "periods",
                "mining scenario must contain at least one period",
            ));
        }

        let mut labels = BTreeSet::new();
        for period in &periods {
            if !labels.insert(period.label().to_owned()) {
                return Err(MineError::invalid_parameter(
                    "periods",
                    format!("scenario period label `{}` is duplicated", period.label()),
                ));
            }
        }

        Ok(Self {
            scenario_id,
            model_id,
            periods,
            rules,
            constraints,
            assumptions,
        })
    }

    /// Identificador estable del escenario.
    #[must_use]
    pub fn scenario_id(&self) -> &ScenarioId {
        &self.scenario_id
    }

    /// Identificador del modelo sobre el que se define el escenario.
    #[must_use]
    pub fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    /// Periodos discretos del escenario.
    #[must_use]
    pub fn periods(&self) -> &[ScenarioPeriod] {
        &self.periods
    }

    /// Reglas explícitas asociadas al escenario.
    #[must_use]
    pub const fn rules(&self) -> &ScenarioRules {
        &self.rules
    }

    /// Restricciones explícitas asociadas al escenario.
    #[must_use]
    pub const fn constraints(&self) -> &ScenarioConstraints {
        &self.constraints
    }

    /// Supuestos serializables asociados al escenario.
    #[must_use]
    pub const fn assumptions(&self) -> &Metadata {
        &self.assumptions
    }
}
