//! Contratos de pushbacks y fases derivados de shells anidados.
//!
//! En planificación minera:
//! - **Shell**: límite de pit definido por el solver UPL a un revenue-factor dado.
//! - **Pushback**: volumen incremental entre dos shells consecutivos (S_k \ S_{k-1}).
//! - **Fase**: unidad operativa de minería, potencialmente un subconjunto de un pushback.
//!
//! Este módulo implementa los contratos serializables para `PushbackPlan` y `PhaseDesign`,
//! con una ruta determinista para derivarlos desde un `PitShellSet`.

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::pit_shells::PitShellSet;

// ── Reglas de anidamiento y acceso ────────────────────────────────────────────

/// Reglas explícitas de anidamiento y acceso entre pushbacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestingAccessRules {
    /// Mínimo lag de banco entre el inicio de un pushback exterior y el interior.
    pub min_bench_lag: Option<i64>,
    /// Cuando es `true`, el pushback exterior debe completarse antes de iniciar el interior.
    pub require_complete_outer_before_inner: bool,
    /// Cuando es `true`, todos los bloques de un pushback son accesibles simultáneamente.
    pub simultaneous_access: bool,
}

impl NestingAccessRules {
    /// Reglas por defecto: sin restricciones de lag ni secuencia forzada.
    #[must_use]
    pub fn default_open() -> Self {
        NestingAccessRules {
            min_bench_lag: None,
            require_complete_outer_before_inner: false,
            simultaneous_access: true,
        }
    }

    /// Reglas estrictas: el pushback exterior debe completarse antes del interior.
    #[must_use]
    pub fn strict_sequential() -> Self {
        NestingAccessRules {
            min_bench_lag: None,
            require_complete_outer_before_inner: true,
            simultaneous_access: false,
        }
    }
}

impl Default for NestingAccessRules {
    fn default() -> Self {
        Self::default_open()
    }
}

// ── PhaseDesign ───────────────────────────────────────────────────────────────

/// Diseño de una fase operativa de minería.
///
/// Una fase puede derivarse desde un pushback completo o desde un subconjunto
/// geométrico del mismo (sub-split por banco, zona geotécnica, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseDesign {
    /// Identificador único de la fase dentro del plan.
    pub phase_id: String,
    /// Índice del pushback fuente dentro del `PushbackPlan`.
    pub pushback_index: usize,
    /// Índice del shell en el `PitShellSet` fuente (si la fase deriva de shells).
    pub shell_index: Option<usize>,
    /// Revenue factor del shell fuente (si aplica).
    pub revenue_factor: Option<f64>,
    /// Bloques pertenecientes a esta fase (índices lineales).
    pub block_indices: Vec<usize>,
    /// Número de bloques.
    pub block_count: usize,
    /// Tonelaje total (cuando se provee).
    pub total_tonnage: Option<f64>,
    /// Fases predecesoras que deben completarse antes de iniciar esta.
    pub predecessor_phase_ids: Vec<String>,
}

// ── PushbackPlan ──────────────────────────────────────────────────────────────

/// Plan de pushbacks derivado de una familia de shells anidados.
///
/// Cada pushback representa el volumen incremental entre dos shells consecutivos.
/// El pushback 0 corresponde al shell más interno; los pushbacks sucesivos son
/// los anillos incrementales hacia el pit final.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushbackPlan {
    /// Fases en orden de extracción (pushback 0 = shell más interno).
    pub phases: Vec<PhaseDesign>,
    /// Número total de fases/pushbacks.
    pub phase_count: usize,
    /// Número total de bloques cubiertos por el plan.
    pub total_block_count: usize,
    /// Tonelaje total del plan (cuando se provee).
    pub total_tonnage: Option<f64>,
    /// Reglas de anidamiento y acceso aplicadas.
    pub nesting_rules: NestingAccessRules,
    /// Limitaciones conocidas de este plan.
    pub limitations: Vec<String>,
}

// ── Derivación de pushbacks desde shells anidados ─────────────────────────────

/// Deriva un `PushbackPlan` desde un `PitShellSet`.
///
/// Cada pushback es el conjunto incremental de bloques entre dos shells consecutivos.
/// El pushback 0 es el shell más pequeño (primer shell con el menor revenue factor).
/// Los pushbacks subsecuentes son los anillos entre shells k-1 y k.
///
/// La función acepta un slice opcional `tonnage_per_block` indexado por linear index
/// para calcular el tonelaje por fase.
///
/// # Errores
///
/// Retorna error si `shell_set` no tiene shells.
pub fn derive_pushbacks_from_nested_shells(
    shell_set: &PitShellSet,
    tonnage_per_block: Option<&[f64]>,
    nesting_rules: NestingAccessRules,
) -> Result<PushbackPlan, MineError> {
    if shell_set.shells.is_empty() {
        return Err(MineError::invalid_parameter(
            "shell_set",
            "pushback derivation requires at least one shell",
        ));
    }

    let mut phases: Vec<PhaseDesign> = Vec::new();
    let mut prev_blocks: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    let mut total_tonnage_acc = 0.0_f64;
    let mut total_block_count = 0usize;

    for (shell_idx, shell) in shell_set.shells.iter().enumerate() {
        let current_set: std::collections::BTreeSet<usize> =
            shell.selected_blocks.iter().copied().collect();

        // Incremental blocks: in current shell but not in any previous shell
        let incremental: Vec<usize> = current_set.difference(&prev_blocks).copied().collect();

        let tonnage: Option<f64> = tonnage_per_block.map(|t| {
            incremental
                .iter()
                .filter_map(|&li| t.get(li).copied())
                .sum::<f64>()
        });

        if let Some(t) = tonnage {
            total_tonnage_acc += t;
        }
        total_block_count += incremental.len();

        // Predecessor: the immediately preceding phase (if any)
        let predecessor_phase_ids = if shell_idx > 0 {
            vec![format!("phase-{:02}", shell_idx - 1)]
        } else {
            vec![]
        };

        let block_count = incremental.len();
        phases.push(PhaseDesign {
            phase_id: format!("phase-{:02}", shell_idx),
            pushback_index: shell_idx,
            shell_index: Some(shell_idx),
            revenue_factor: Some(shell.revenue_factor),
            block_indices: incremental,
            block_count,
            total_tonnage: tonnage,
            predecessor_phase_ids,
        });

        prev_blocks = current_set;
    }

    let has_tonnage = tonnage_per_block.is_some();
    let phase_count = phases.len();

    Ok(PushbackPlan {
        phases,
        phase_count,
        total_block_count,
        total_tonnage: if has_tonnage {
            Some(total_tonnage_acc)
        } else {
            None
        },
        nesting_rules,
        limitations: vec![
            "Pushbacks are derived purely from shell membership differences; no geotechnical or operational constraints are applied.".to_owned(),
            "Phase boundaries follow revenue-factor-derived shells; they do not model ramp access, equipment constraints or bench geometry.".to_owned(),
            "Predecessor relationships are set to strictly sequential (inner shell first); override nesting_rules for alternative sequencing.".to_owned(),
        ],
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pit_shells::{PitShell, PitShellSet};

    fn make_shell_set_two_shells() -> PitShellSet {
        // Shell 0 (factor 0.5): blocks [0, 1]
        // Shell 1 (factor 1.0): blocks [0, 1, 2, 3]
        PitShellSet {
            shells: vec![
                PitShell {
                    revenue_factor: 0.5,
                    selected_blocks: vec![0, 1],
                    pit_value: 5.0,
                    block_count: 2,
                },
                PitShell {
                    revenue_factor: 1.0,
                    selected_blocks: vec![0, 1, 2, 3],
                    pit_value: 8.0,
                    block_count: 4,
                },
            ],
            total_block_count: 4,
            factors_evaluated: 2,
            unique_shell_count: 2,
        }
    }

    #[test]
    fn derive_pushbacks_produces_incremental_phases() {
        let shell_set = make_shell_set_two_shells();
        let plan = derive_pushbacks_from_nested_shells(&shell_set, None, NestingAccessRules::default())
            .expect("derivation should succeed");

        assert_eq!(plan.phase_count, 2);
        // Phase 0: blocks [0, 1] (innermost shell)
        assert_eq!(plan.phases[0].block_count, 2);
        assert!(plan.phases[0].selected_blocks_contain(0));
        assert!(plan.phases[0].selected_blocks_contain(1));
        assert_eq!(plan.phases[0].predecessor_phase_ids, Vec::<String>::new());

        // Phase 1: incremental blocks [2, 3]
        assert_eq!(plan.phases[1].block_count, 2);
        assert!(plan.phases[1].selected_blocks_contain(2));
        assert!(plan.phases[1].selected_blocks_contain(3));
        assert_eq!(plan.phases[1].predecessor_phase_ids, vec!["phase-00"]);
    }

    #[test]
    fn total_block_count_matches_union_of_shells() {
        let shell_set = make_shell_set_two_shells();
        let plan = derive_pushbacks_from_nested_shells(&shell_set, None, NestingAccessRules::default())
            .expect("derivation should succeed");

        assert_eq!(plan.total_block_count, 4);
    }

    #[test]
    fn tonnage_accumulates_correctly() {
        let shell_set = make_shell_set_two_shells();
        // tonnage per linear index (index = value * 10 for simplicity)
        let tonnes = vec![100.0, 200.0, 150.0, 300.0];
        let plan = derive_pushbacks_from_nested_shells(
            &shell_set,
            Some(&tonnes),
            NestingAccessRules::default(),
        )
        .expect("derivation should succeed");

        let phase0_tonnage = plan.phases[0].total_tonnage.unwrap();
        assert!((phase0_tonnage - 300.0).abs() < 1e-9); // blocks 0+1: 100+200

        let phase1_tonnage = plan.phases[1].total_tonnage.unwrap();
        assert!((phase1_tonnage - 450.0).abs() < 1e-9); // blocks 2+3: 150+300

        assert!((plan.total_tonnage.unwrap() - 750.0).abs() < 1e-9);
    }

    #[test]
    fn empty_shell_set_returns_error() {
        let empty = PitShellSet {
            shells: vec![],
            total_block_count: 0,
            factors_evaluated: 0,
            unique_shell_count: 0,
        };

        let err = derive_pushbacks_from_nested_shells(&empty, None, NestingAccessRules::default())
            .expect_err("empty shell set should fail");

        assert!(err.to_string().contains("shell"));
    }

    #[test]
    fn phase_design_is_serializable() {
        let shell_set = make_shell_set_two_shells();
        let plan = derive_pushbacks_from_nested_shells(&shell_set, None, NestingAccessRules::strict_sequential())
            .expect("derivation should succeed");

        let json = serde_json::to_string(&plan).expect("plan should serialize");
        assert!(json.contains("phase-00"));
        assert!(json.contains("require_complete_outer_before_inner"));
    }
}

// Helper for tests only — not part of public API
impl PhaseDesign {
    #[cfg(test)]
    fn selected_blocks_contain(&self, li: usize) -> bool {
        self.block_indices.contains(&li)
    }
}
