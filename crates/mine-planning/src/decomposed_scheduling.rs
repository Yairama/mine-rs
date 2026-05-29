//! Capa decomposed para scheduling de largo plazo.
//!
//! Esta capa separa explicitamente tres etapas deterministas:
//! - seleccion temporal de unidades;
//! - materializacion/ruteo a `LongTermSchedule`;
//! - aplicacion opcional de politicas de stockpile/reclaim.
//!
//! El objetivo es preparar una ruta reusable para CPIT/PCPSP donde pueda
//! compararse un candidato heuristico contra una referencia exacta pequena
//! antes de introducir relajaciones o solvers mas avanzados.
//!
//! # References
//! - Caccetta, L., Hill, S. P. (2003). *An Application of Branch and Cut to
//!   Open Pit Mine Scheduling*. <https://doi.org/10.1007/A:1024835022186>
//! - Lambert, W. B., Brickey, A., Newman, A. M., Eurek, K. (2014).
//!   *Open-Pit Block-Sequencing Formulations: A Tutorial*.
//!   <https://doi.org/10.1287/inte.2013.0731>
//! - Moreno, E., Rezakhah, M., Newman, A. M., Ferreira, F. C. L. (2017).
//!   *Linear models for stockpiling in open-pit mine production scheduling problems*.
//!   <https://doi.org/10.1016/j.ejor.2016.12.014>
//! - Chicoisne, R., Espinoza, D., Goycoolea, M., Moreno, E., Rubio, E. (2012).
//!   *A new algorithm for the open-pit mine production scheduling problem*.
//!   <https://doi.org/10.1287/opre.1120.1072>
//! - Cullenbine, C., Wood, R. K., Newman, A. M. (2011).
//!   *A Sliding Time Window Heuristic for Open Pit Mine Block Sequencing*.
//!   <https://doi.org/10.1007/s11590-011-0306-2>
//! - Boland, N., Dumitrescu, I., Froyland, G., Gleixner, A. (2009).
//!   *LP-based disaggregation approaches to solving the open pit mining production scheduling problem with block processing selectivity*.
//!   <https://doi.org/10.1016/j.cor.2008.01.005>

use mine_core::{Metadata, MineError};
use serde::{Deserialize, Serialize};

use crate::long_term_schedule::LongTermSchedule;
use crate::scheduling_problem::SchedulingProblem;
use crate::small_scheduling::{
    SmallSchedulingSolution, build_ready_frontier_schedule, enrich_problem_for_ready_frontier,
    materialize_long_term_schedule, solve_small_scheduling_problem,
};
use crate::stockpile_policy::{LongTermStockpilePolicy, apply_long_term_stockpile_policy};

/// Metodo temporal determinista dentro de la arquitectura decomposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecomposedTemporalSolver {
    /// Heuristica de frontera lista con valor descontado.
    ReadyFrontier,
    /// Baseline exacta pequena como referencia/upper bound local.
    SmallExact,
}

/// Configuracion del pipeline decomposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecomposedSchedulingConfig {
    temporal_solver: DecomposedTemporalSolver,
    reference_bound_solver: Option<DecomposedTemporalSolver>,
    max_vertical_advance: Option<i64>,
    stockpile_policy: Option<LongTermStockpilePolicy>,
}

impl DecomposedSchedulingConfig {
    /// Configuracion minima basada en ready frontier.
    #[must_use]
    pub fn ready_frontier() -> Self {
        Self {
            temporal_solver: DecomposedTemporalSolver::ReadyFrontier,
            reference_bound_solver: None,
            max_vertical_advance: None,
            stockpile_policy: None,
        }
    }

    /// Solver temporal principal.
    #[must_use]
    pub const fn temporal_solver(&self) -> DecomposedTemporalSolver {
        self.temporal_solver
    }

    /// Solver de referencia opcional para bound local.
    #[must_use]
    pub const fn reference_bound_solver(&self) -> Option<DecomposedTemporalSolver> {
        self.reference_bound_solver
    }

    /// Restriccion opcional de avance vertical.
    #[must_use]
    pub const fn max_vertical_advance(&self) -> Option<i64> {
        self.max_vertical_advance
    }

    /// Politica opcional de stockpile/reclaim.
    #[must_use]
    pub fn stockpile_policy(&self) -> Option<&LongTermStockpilePolicy> {
        self.stockpile_policy.as_ref()
    }

    /// Reemplaza el solver temporal.
    #[must_use]
    pub fn with_temporal_solver(mut self, temporal_solver: DecomposedTemporalSolver) -> Self {
        self.temporal_solver = temporal_solver;
        self
    }

    /// Agrega un solver opcional para referencia exacta/local.
    #[must_use]
    pub fn with_reference_bound_solver(
        mut self,
        reference_bound_solver: Option<DecomposedTemporalSolver>,
    ) -> Self {
        self.reference_bound_solver = reference_bound_solver;
        self
    }

    /// Agrega un limite opcional de avance vertical.
    #[must_use]
    pub fn with_max_vertical_advance(mut self, max_vertical_advance: Option<i64>) -> Self {
        self.max_vertical_advance = max_vertical_advance;
        self
    }

    /// Agrega una politica opcional de stockpile.
    #[must_use]
    pub fn with_stockpile_policy(
        mut self,
        stockpile_policy: Option<LongTermStockpilePolicy>,
    ) -> Self {
        self.stockpile_policy = stockpile_policy;
        self
    }
}

/// Artefactos producidos por el pipeline decomposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecomposedSchedulingArtifacts {
    temporal_candidate: SmallSchedulingSolution,
    routed_schedule: LongTermSchedule,
    final_schedule: LongTermSchedule,
    reference_bound: Option<SmallSchedulingSolution>,
}

impl DecomposedSchedulingArtifacts {
    /// Candidato temporal principal.
    #[must_use]
    pub fn temporal_candidate(&self) -> &SmallSchedulingSolution {
        &self.temporal_candidate
    }

    /// Schedule materializado tras la etapa de ruteo.
    #[must_use]
    pub fn routed_schedule(&self) -> &LongTermSchedule {
        &self.routed_schedule
    }

    /// Schedule final, despues de aplicar stockpiles si corresponde.
    #[must_use]
    pub fn final_schedule(&self) -> &LongTermSchedule {
        &self.final_schedule
    }

    /// Referencia/bound local opcional.
    #[must_use]
    pub fn reference_bound(&self) -> Option<&SmallSchedulingSolution> {
        self.reference_bound.as_ref()
    }
}

/// Resuelve un `SchedulingProblem` mediante una arquitectura decomposed.
pub fn solve_decomposed_scheduling_problem(
    problem: &SchedulingProblem,
    config: &DecomposedSchedulingConfig,
    metadata: Metadata,
) -> Result<DecomposedSchedulingArtifacts, MineError> {
    let enriched_problem = enrich_problem_for_ready_frontier(problem)?;
    let temporal_candidate = solve_temporal_problem(&enriched_problem, config.temporal_solver())?;
    let routed_schedule = materialize_long_term_schedule(
        problem,
        &temporal_candidate,
        config.max_vertical_advance(),
        metadata.clone(),
    )?;
    let final_schedule = match config.stockpile_policy() {
        Some(stockpile_policy) => {
            apply_long_term_stockpile_policy(&routed_schedule, stockpile_policy, metadata)?
        }
        None => routed_schedule.clone(),
    };
    let reference_bound = config
        .reference_bound_solver()
        .map(|solver| solve_temporal_problem(&enriched_problem, solver))
        .transpose()?;

    Ok(DecomposedSchedulingArtifacts {
        temporal_candidate,
        routed_schedule,
        final_schedule,
        reference_bound,
    })
}

fn solve_temporal_problem(
    problem: &SchedulingProblem,
    solver: DecomposedTemporalSolver,
) -> Result<SmallSchedulingSolution, MineError> {
    match solver {
        DecomposedTemporalSolver::ReadyFrontier => build_ready_frontier_schedule(problem),
        DecomposedTemporalSolver::SmallExact => solve_small_scheduling_problem(problem),
    }
}
