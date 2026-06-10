//! Telemetría de runtime para reportes benchmark (MR-215).
//!
//! Los papers de referencia de MineLib ([R29] Espinoza et al. doi
//! 10.1007/s10479-012-1258-3, [R34] Muñoz et al. doi 10.1007/s10589-017-9946-1,
//! [R35] Chicoisne et al. doi 10.1287/opre.1120.1072, [R37] Rivera Letelier et
//! al. doi 10.1287/opre.2019.1965) reportan tiempos de CPU por instancia y
//! método. Este módulo agrega la contraparte estructural en los artefactos del
//! repo: tiempos de pared por etapa, paralelismo disponible y metadata de
//! plataforma, dejando explícito que los tiempos NO son comparables entre
//! máquinas distintas sin normalización.

use std::time::Instant;

use serde::Serialize;

/// Versión del contrato de telemetría benchmark-side.
pub const RUNTIME_TELEMETRY_CONTRACT_VERSION: &str = "mr215-v1";

/// Nota fija de comparabilidad para lectores del artefacto.
pub const RUNTIME_TELEMETRY_COMPARABILITY_NOTE: &str = "Wall-clock timings are \
machine-dependent and are NOT directly comparable against published CPU times \
without hardware normalization; they document order-of-magnitude behaviour and \
regression tracking for this repository only.";

/// Medición de una etapa individual del pipeline benchmark.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStageTiming {
    /// Etiqueta estable de la etapa medida.
    pub stage: String,
    /// Duración de pared en milisegundos.
    pub wall_clock_ms: f64,
}

/// Telemetría estructurada de una corrida benchmark.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeTelemetry {
    /// Versión del contrato de telemetría.
    pub contract_version: String,
    /// Sistema operativo reportado por el toolchain.
    pub os: String,
    /// Arquitectura reportada por el toolchain.
    pub arch: String,
    /// Paralelismo disponible reportado por el runtime (hilos lógicos).
    pub available_parallelism: Option<usize>,
    /// Nota de comparabilidad contra tiempos publicados.
    pub comparability_note: String,
    /// Limitaciones explícitas de la medición actual.
    pub limitations: Vec<String>,
    /// Tiempos de pared por etapa, en orden de ejecución.
    pub stage_timings: Vec<RuntimeStageTiming>,
    /// Tiempo total de pared en milisegundos (suma de etapas medidas).
    pub total_wall_clock_ms: f64,
}

/// Cronómetro acumulativo por etapas para construir `RuntimeTelemetry`.
#[derive(Debug)]
pub struct StageTimer {
    stage_start: Instant,
    timings: Vec<RuntimeStageTiming>,
}

impl StageTimer {
    /// Inicia el cronómetro de etapas.
    #[must_use]
    pub fn start() -> Self {
        Self {
            stage_start: Instant::now(),
            timings: Vec::new(),
        }
    }

    /// Cierra la etapa actual con la etiqueta dada y abre la siguiente.
    pub fn record_stage(&mut self, stage: impl Into<String>) {
        let elapsed_ms = self.stage_start.elapsed().as_secs_f64() * 1_000.0;
        self.timings.push(RuntimeStageTiming {
            stage: stage.into(),
            wall_clock_ms: elapsed_ms,
        });
        self.stage_start = Instant::now();
    }

    /// Consume el cronómetro y produce la telemetría estructurada.
    #[must_use]
    pub fn finish(self) -> RuntimeTelemetry {
        let total_wall_clock_ms = self.timings.iter().map(|timing| timing.wall_clock_ms).sum();
        RuntimeTelemetry {
            contract_version: RUNTIME_TELEMETRY_CONTRACT_VERSION.to_owned(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            available_parallelism: std::thread::available_parallelism()
                .ok()
                .map(std::num::NonZeroUsize::get),
            comparability_note: RUNTIME_TELEMETRY_COMPARABILITY_NOTE.to_owned(),
            limitations: vec![
                "peak memory usage is not measured yet; only wall-clock per stage is recorded"
                    .to_owned(),
                "timings use a single process run without warmup or repetition statistics"
                    .to_owned(),
            ],
            stage_timings: self.timings,
            total_wall_clock_ms,
        }
    }
}
