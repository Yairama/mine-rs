//! Contratos de pushbacks y fases derivados de shells anidados.
//!
//! En planificación minera:
//! - **Shell**: límite de pit definido por el solver UPL a un revenue-factor dado.
//! - **Pushback**: volumen incremental entre dos shells consecutivos (S_k \ S_{k-1}).
//! - **Fase**: unidad operativa de minería, potencialmente un subconjunto de un pushback.
//!
//! Este módulo implementa los contratos serializables para `PushbackPlan` y `PhaseDesign`,
//! con una ruta determinista para derivarlos desde un `PitShellSet`.

use std::collections::{BTreeMap, BTreeSet};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::benches::BenchAssignment;
use crate::pit_shells::PitShellSet;
use crate::precedence::{PrecedenceGraph, PrecedenceNode};

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
    /// Banco fuente de la fase cuando se deriva desde asignaciones de bench.
    pub bench: Option<i64>,
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
    let tonnage_by_linear_index = tonnage_per_block.map(|tonnage_per_block| {
        tonnage_per_block
            .iter()
            .enumerate()
            .map(|(linear_index, tonnage)| (linear_index, *tonnage))
            .collect::<BTreeMap<_, _>>()
    });
    derive_pushbacks_from_nested_shells_from_map(
        shell_set,
        tonnage_by_linear_index.as_ref(),
        nesting_rules,
    )
}

/// Variante sparse-safe de `derive_pushbacks_from_nested_shells`.
///
/// Usa un mapa `linear_index -> tonnage`, adecuado para block models sparse donde
/// los índices lineales materializados no son necesariamente `0..N`.
pub fn derive_pushbacks_from_nested_shells_from_map(
    shell_set: &PitShellSet,
    tonnage_per_block: Option<&BTreeMap<usize, f64>>,
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
                .filter_map(|li| t.get(li).copied())
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
            bench: None,
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

/// Deriva fases auditables desde shells anidados, benches y precedencias bloque a bloque.
///
/// Reglas de esta primera ruta:
/// - cada shell incremental se subdivide por bench;
/// - las precedencias intra-shell entre benches se infieren desde `precedence_graph`;
/// - entre shells consecutivos, las precedencias siguen `nesting_rules`: por defecto se
///   alinean por bench y pueden reforzarse hasta una secuencia shell-a-shell completa.
pub fn derive_phase_design_from_nested_shells(
    shell_set: &PitShellSet,
    bench_assignments: &[BenchAssignment],
    precedence_graph: &PrecedenceGraph,
    tonnage_per_block: Option<&[f64]>,
    nesting_rules: NestingAccessRules,
) -> Result<PushbackPlan, MineError> {
    let tonnage_by_linear_index = tonnage_per_block.map(|tonnage_per_block| {
        tonnage_per_block
            .iter()
            .enumerate()
            .map(|(linear_index, tonnage)| (linear_index, *tonnage))
            .collect::<BTreeMap<_, _>>()
    });
    derive_phase_design_from_nested_shells_from_map(
        shell_set,
        bench_assignments,
        precedence_graph,
        tonnage_by_linear_index.as_ref(),
        nesting_rules,
    )
}

/// Variante sparse-safe de `derive_phase_design_from_nested_shells`.
///
/// Usa un mapa `linear_index -> tonnage`, adecuado para block models sparse y
/// benchmarks abiertos donde el layout materializado conserva huecos de grilla.
pub fn derive_phase_design_from_nested_shells_from_map(
    shell_set: &PitShellSet,
    bench_assignments: &[BenchAssignment],
    precedence_graph: &PrecedenceGraph,
    tonnage_per_block: Option<&BTreeMap<usize, f64>>,
    nesting_rules: NestingAccessRules,
) -> Result<PushbackPlan, MineError> {
    if shell_set.shells.is_empty() {
        return Err(MineError::invalid_parameter(
            "shell_set",
            "phase design requires at least one shell",
        ));
    }

    let bench_by_block = build_bench_lookup(bench_assignments)?;
    let mut phases = Vec::new();
    let mut prev_shell_blocks = BTreeSet::new();
    let mut total_tonnage_acc = 0.0_f64;
    let mut total_block_count = 0usize;
    let mut previous_shell_phase_ids_by_bench = BTreeMap::<i64, String>::new();

    for (shell_idx, shell) in shell_set.shells.iter().enumerate() {
        let current_blocks = shell
            .selected_blocks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let incremental_blocks = current_blocks
            .difference(&prev_shell_blocks)
            .copied()
            .collect::<Vec<_>>();
        let grouped_by_bench = group_blocks_by_bench(&incremental_blocks, &bench_by_block)?;

        let mut phase_ids_by_bench = BTreeMap::<i64, String>::new();
        let mut bench_phase_blocks = BTreeMap::<i64, BTreeSet<usize>>::new();

        for (&bench, blocks) in grouped_by_bench.iter().rev() {
            let mut block_indices = blocks.clone();
            block_indices.sort_unstable();
            let total_tonnage = tonnage_per_block.map(|tonnes| {
                block_indices
                    .iter()
                    .filter_map(|linear_index| tonnes.get(linear_index).copied())
                    .sum::<f64>()
            });
            if let Some(total_tonnage) = total_tonnage {
                total_tonnage_acc += total_tonnage;
            }
            total_block_count += block_indices.len();

            let phase_id = format!("phase-s{shell_idx:02}-b{bench}");
            phase_ids_by_bench.insert(bench, phase_id.clone());
            bench_phase_blocks.insert(bench, block_indices.iter().copied().collect());
            phases.push(PhaseDesign {
                phase_id,
                pushback_index: shell_idx,
                shell_index: Some(shell_idx),
                revenue_factor: Some(shell.revenue_factor),
                bench: Some(bench),
                block_indices: block_indices.clone(),
                block_count: block_indices.len(),
                total_tonnage,
                predecessor_phase_ids: Vec::new(),
            });
        }

        let intra_shell_predecessors = build_intra_shell_predecessors(
            precedence_graph,
            &bench_by_block,
            &incremental_blocks,
            &phase_ids_by_bench,
        );

        let mut ordered_benches = grouped_by_bench.keys().copied().collect::<Vec<_>>();
        ordered_benches.sort_by(|left, right| right.cmp(left));

        for (position, bench) in ordered_benches.iter().enumerate() {
            let phase_id = phase_ids_by_bench
                .get(bench)
                .expect("phase id must exist for every bench");
            let mut predecessor_phase_ids = BTreeSet::<String>::new();

            predecessor_phase_ids.extend(build_cross_shell_predecessors(
                &previous_shell_phase_ids_by_bench,
                *bench,
                &nesting_rules,
            ));

            if let Some(inferred) = intra_shell_predecessors.get(phase_id) {
                predecessor_phase_ids.extend(inferred.iter().cloned());
            } else if position > 0 {
                let upper_bench = ordered_benches[position - 1];
                let fallback = phase_ids_by_bench
                    .get(&upper_bench)
                    .expect("fallback predecessor phase must exist");
                predecessor_phase_ids.insert(fallback.clone());
            }

            if let Some(phase) = phases.iter_mut().find(|phase| phase.phase_id == *phase_id) {
                phase.predecessor_phase_ids = predecessor_phase_ids.into_iter().collect();
            }
        }

        previous_shell_phase_ids_by_bench = phase_ids_by_bench.clone();
        prev_shell_blocks = current_blocks;
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
            "Each shell increment is split by bench; no geometric sub-phasing inside a bench is attempted.".to_owned(),
            "Cross-shell sequencing follows bench-aligned nesting access rules; strict sequential mode still serializes shell k behind every phase in shell k-1.".to_owned(),
            "Bench continuity is inferred from explicit block precedence when available; otherwise the design falls back to descending bench order within the same shell.".to_owned(),
        ],
    })
}

fn build_bench_lookup(
    bench_assignments: &[BenchAssignment],
) -> Result<BTreeMap<usize, i64>, MineError> {
    let mut lookup = BTreeMap::new();
    for assignment in bench_assignments {
        if lookup
            .insert(assignment.linear_index, assignment.bench)
            .is_some()
        {
            return Err(MineError::validation(format!(
                "duplicate bench assignment for linear index `{}`",
                assignment.linear_index
            )));
        }
    }
    Ok(lookup)
}

fn group_blocks_by_bench(
    block_indices: &[usize],
    bench_by_block: &BTreeMap<usize, i64>,
) -> Result<BTreeMap<i64, Vec<usize>>, MineError> {
    let mut grouped = BTreeMap::<i64, Vec<usize>>::new();
    for &block_index in block_indices {
        let Some(&bench) = bench_by_block.get(&block_index) else {
            return Err(MineError::validation(format!(
                "missing bench assignment for shell block `{block_index}`"
            )));
        };
        grouped.entry(bench).or_default().push(block_index);
    }
    Ok(grouped)
}

fn build_intra_shell_predecessors(
    precedence_graph: &PrecedenceGraph,
    bench_by_block: &BTreeMap<usize, i64>,
    incremental_blocks: &[usize],
    phase_ids_by_bench: &BTreeMap<i64, String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let incremental_set = incremental_blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut predecessors = BTreeMap::<String, BTreeSet<String>>::new();

    for edge in precedence_graph.edges() {
        let (PrecedenceNode::Block(predecessor), PrecedenceNode::Block(successor)) =
            (edge.predecessor(), edge.successor())
        else {
            continue;
        };

        if !incremental_set.contains(predecessor) || !incremental_set.contains(successor) {
            continue;
        }

        let Some(&predecessor_bench) = bench_by_block.get(predecessor) else {
            continue;
        };
        let Some(&successor_bench) = bench_by_block.get(successor) else {
            continue;
        };
        if predecessor_bench == successor_bench {
            continue;
        }

        let Some(predecessor_phase_id) = phase_ids_by_bench.get(&predecessor_bench) else {
            continue;
        };
        let Some(successor_phase_id) = phase_ids_by_bench.get(&successor_bench) else {
            continue;
        };

        predecessors
            .entry(successor_phase_id.clone())
            .or_default()
            .insert(predecessor_phase_id.clone());
    }

    predecessors
}

fn build_cross_shell_predecessors(
    previous_shell_phase_ids_by_bench: &BTreeMap<i64, String>,
    current_bench: i64,
    nesting_rules: &NestingAccessRules,
) -> BTreeSet<String> {
    if previous_shell_phase_ids_by_bench.is_empty() {
        return BTreeSet::new();
    }

    if nesting_rules.require_complete_outer_before_inner {
        return previous_shell_phase_ids_by_bench
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
    }

    let minimum_predecessor_bench = current_bench + nesting_rules.min_bench_lag.unwrap_or_default();
    previous_shell_phase_ids_by_bench
        .range(minimum_predecessor_bench..)
        .map(|(_, phase_id)| phase_id.clone())
        .collect()
}

// Helper for tests only; not part of the public API.
impl PhaseDesign {
    #[cfg(test)]
    fn selected_blocks_contain(&self, li: usize) -> bool {
        self.block_indices.contains(&li)
    }
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
        let plan =
            derive_pushbacks_from_nested_shells(&shell_set, None, NestingAccessRules::default())
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
        let plan =
            derive_pushbacks_from_nested_shells(&shell_set, None, NestingAccessRules::default())
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

        let phase0_tonnage = plan.phases[0]
            .total_tonnage
            .expect("phase 0 tonnage should exist");
        assert!((phase0_tonnage - 300.0).abs() < 1e-9); // blocks 0+1: 100+200

        let phase1_tonnage = plan.phases[1]
            .total_tonnage
            .expect("phase 1 tonnage should exist");
        assert!((phase1_tonnage - 450.0).abs() < 1e-9); // blocks 2+3: 150+300

        assert!((plan.total_tonnage.expect("total tonnage should exist") - 750.0).abs() < 1e-9);
    }

    #[test]
    fn sparse_tonnage_map_accumulates_correctly() {
        let shell_set = PitShellSet {
            shells: vec![
                PitShell {
                    revenue_factor: 0.5,
                    selected_blocks: vec![10, 20],
                    pit_value: 5.0,
                    block_count: 2,
                },
                PitShell {
                    revenue_factor: 1.0,
                    selected_blocks: vec![10, 20, 30, 40],
                    pit_value: 8.0,
                    block_count: 4,
                },
            ],
            total_block_count: 4,
            factors_evaluated: 2,
            unique_shell_count: 2,
        };
        let tonnes = BTreeMap::from([
            (10_usize, 100.0_f64),
            (20_usize, 200.0_f64),
            (30_usize, 150.0_f64),
            (40_usize, 300.0_f64),
        ]);

        let plan = derive_pushbacks_from_nested_shells_from_map(
            &shell_set,
            Some(&tonnes),
            NestingAccessRules::default(),
        )
        .expect("sparse derivation should succeed");

        assert!(
            (plan.phases[0]
                .total_tonnage
                .expect("phase 0 tonnage should exist")
                - 300.0)
                .abs()
                < 1e-9
        );
        assert!(
            (plan.phases[1]
                .total_tonnage
                .expect("phase 1 tonnage should exist")
                - 450.0)
                .abs()
                < 1e-9
        );
        assert!((plan.total_tonnage.expect("total tonnage should exist") - 750.0).abs() < 1e-9);
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
        let plan = derive_pushbacks_from_nested_shells(
            &shell_set,
            None,
            NestingAccessRules::strict_sequential(),
        )
        .expect("derivation should succeed");

        let json = serde_json::to_string(&plan).expect("plan should serialize");
        assert!(json.contains("phase-00"));
        assert!(json.contains("require_complete_outer_before_inner"));
    }

    #[test]
    fn default_open_cross_shell_access_is_bench_aligned() {
        let shell_set = make_shell_set_two_shells();
        let bench_assignments = vec![
            BenchAssignment {
                linear_index: 0,
                bench: 2,
                center_elevation: 2.0,
            },
            BenchAssignment {
                linear_index: 1,
                bench: 1,
                center_elevation: 1.0,
            },
            BenchAssignment {
                linear_index: 2,
                bench: 2,
                center_elevation: 2.0,
            },
            BenchAssignment {
                linear_index: 3,
                bench: 1,
                center_elevation: 1.0,
            },
        ];
        let precedence_graph = PrecedenceGraph::new(vec![]).expect("empty graph should be valid");

        let plan = derive_phase_design_from_nested_shells(
            &shell_set,
            &bench_assignments,
            &precedence_graph,
            None,
            NestingAccessRules::default_open(),
        )
        .expect("phase plan should derive");

        let outer_upper = plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-s01-b2")
            .expect("outer upper bench phase should exist");
        assert_eq!(outer_upper.predecessor_phase_ids, vec!["phase-s00-b2"]);

        let outer_lower = plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-s01-b1")
            .expect("outer lower bench phase should exist");
        assert_eq!(
            outer_lower.predecessor_phase_ids,
            vec!["phase-s00-b1", "phase-s00-b2", "phase-s01-b2"]
        );
    }

    #[test]
    fn cross_shell_access_respects_bench_lag() {
        let shell_set = make_shell_set_two_shells();
        let bench_assignments = vec![
            BenchAssignment {
                linear_index: 0,
                bench: 2,
                center_elevation: 2.0,
            },
            BenchAssignment {
                linear_index: 1,
                bench: 1,
                center_elevation: 1.0,
            },
            BenchAssignment {
                linear_index: 2,
                bench: 2,
                center_elevation: 2.0,
            },
            BenchAssignment {
                linear_index: 3,
                bench: 1,
                center_elevation: 1.0,
            },
        ];
        let precedence_graph = PrecedenceGraph::new(vec![]).expect("empty graph should be valid");
        let lagged_rules = NestingAccessRules {
            min_bench_lag: Some(1),
            require_complete_outer_before_inner: false,
            simultaneous_access: true,
        };

        let plan = derive_phase_design_from_nested_shells(
            &shell_set,
            &bench_assignments,
            &precedence_graph,
            None,
            lagged_rules,
        )
        .expect("phase plan should derive");

        let outer_upper = plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-s01-b2")
            .expect("outer upper bench phase should exist");
        assert!(outer_upper.predecessor_phase_ids.is_empty());

        let outer_lower = plan
            .phases
            .iter()
            .find(|phase| phase.phase_id == "phase-s01-b1")
            .expect("outer lower bench phase should exist");
        assert_eq!(
            outer_lower.predecessor_phase_ids,
            vec!["phase-s00-b2", "phase-s01-b2"]
        );
    }
}
