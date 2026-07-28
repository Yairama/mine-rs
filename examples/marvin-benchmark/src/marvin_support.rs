use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use mine_sdk::{
    BlockModel, ColumnData, ColumnId, MineError, PrecedenceEdge, PrecedenceGraph, PrecedenceNode,
};
use serde::{Deserialize, Serialize};

/// Lee `marvin.prec` y lo normaliza a `PrecedenceGraph` usando los índices lineales del modelo.
pub fn read_marvin_precedence_graph(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<PrecedenceGraph, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin precedence file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin precedence line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 2 {
            return Err(MineError::validation(format!(
                "Marvin precedence line {line_number} must contain at least block id and predecessor count"
            )));
        }

        let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
        let predecessor_count = parse_usize_field(fields[1], line_number, "predecessor_count")?;
        if fields.len() != predecessor_count + 2 {
            return Err(MineError::validation(format!(
                "Marvin precedence line {line_number} declares {predecessor_count} predecessors but contains {} ids",
                fields.len().saturating_sub(2)
            )));
        }

        let successor_linear_index = map_block_id(source_block_id, &block_id_to_linear_index)?;
        let successor_node = PrecedenceNode::Block(successor_linear_index);
        nodes.insert(successor_node.clone());

        for predecessor_id_text in &fields[2..] {
            let predecessor_block_id =
                parse_i64_field(predecessor_id_text, line_number, "predecessor_block_id")?;
            let predecessor_linear_index =
                map_block_id(predecessor_block_id, &block_id_to_linear_index)?;
            let predecessor_node = PrecedenceNode::Block(predecessor_linear_index);
            nodes.insert(predecessor_node.clone());
            edges.insert(PrecedenceEdge::new(
                predecessor_node,
                successor_node.clone(),
            ));
        }
    }

    PrecedenceGraph::from_nodes_and_edges(nodes.into_iter().collect(), edges.into_iter().collect())
}

/// Lee `marvin_upit.sol` y lo normaliza como membresía de índices lineales.
pub fn read_marvin_upit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<Vec<usize>, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin upit solution file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut selected = BTreeSet::new();

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin upit solution line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let source_block_id = parse_i64_field(trimmed, line_number, "block_id")?;
        selected.insert(map_block_id(source_block_id, &block_id_to_linear_index)?);
    }

    Ok(selected.into_iter().collect())
}

/// Lee `marvin.upit` y normaliza los valores objetivo por bloque.
///
/// Retorna un vector de `(linear_index, block_objective_value)` para todos los bloques
/// del modelo, en el mismo orden que el archivo.
#[cfg_attr(not(test), allow(dead_code))]
pub fn read_marvin_upit_block_values(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<Vec<(usize, f64)>, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin upit block values file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut result = Vec::new();
    let mut in_data = false;

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin upit block values line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Header keywords
        if trimmed.starts_with("NAME:")
            || trimmed.starts_with("TYPE:")
            || trimmed.starts_with("NBLOCKS:")
            || trimmed.starts_with("OBJECTIVE_FUNCTION:")
        {
            in_data = true;
            continue;
        }
        if trimmed == "EOF" {
            break;
        }
        if !in_data {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 2 {
            return Err(MineError::validation(format!(
                "Marvin upit block values line {line_number} must contain block_id and value (got {} fields)",
                fields.len()
            )));
        }

        let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
        let block_value = fields[1].parse::<f64>().map_err(|error| {
            MineError::validation(format!(
                "Marvin upit block values line {line_number} contains invalid float in block_value: {error}"
            ))
        })?;
        let linear_index = map_block_id(source_block_id, &block_id_to_linear_index)?;
        result.push((linear_index, block_value));
    }

    Ok(result)
}

/// Alias MineLib del tipo de problema de scheduling normalizado.
#[allow(dead_code)]
pub type MinelibScheduleProblemKind = MarvinScheduleProblemKind;

/// Alias MineLib del contrato abierto de scheduling.
#[allow(dead_code)]
pub type MinelibScheduleProblem = MarvinScheduleProblem;

/// Alias MineLib de una asignacion normalizada.
#[allow(dead_code)]
pub type MinelibScheduleAssignment = MarvinScheduleAssignment;

/// Alias MineLib de una solucion normalizada.
#[allow(dead_code)]
pub type MinelibScheduleSolution = MarvinScheduleSolution;

/// Alias MineLib del resumen auditado de una solucion.
#[allow(dead_code)]
pub type MinelibScheduleSolutionSummary = MarvinScheduleSolutionSummary;

/// Lee una lista de precedencias MineLib y la normaliza a `PrecedenceGraph`.
#[allow(dead_code)]
pub fn read_minelib_precedence_graph(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<PrecedenceGraph, MineError> {
    read_marvin_precedence_graph(path, model)
}

/// Lee una solucion UPIT MineLib y la normaliza como indices lineales seleccionados.
#[allow(dead_code)]
pub fn read_minelib_upit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<Vec<usize>, MineError> {
    read_marvin_upit_solution(path, model)
}

/// Lee la tabla de valores objetivo UPIT MineLib por bloque.
#[allow(dead_code)]
pub fn read_minelib_upit_block_values(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<Vec<(usize, f64)>, MineError> {
    read_marvin_upit_block_values(path, model)
}

/// Lee un problema CPIT MineLib y lo normaliza a contrato abierto.
#[allow(dead_code)]
pub fn read_minelib_cpit_problem(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MinelibScheduleProblem, MineError> {
    read_marvin_cpit_problem(path, model)
}

/// Lee un problema PCPSP MineLib y lo normaliza a contrato abierto.
#[allow(dead_code)]
pub fn read_minelib_pcpsp_problem(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MinelibScheduleProblem, MineError> {
    read_marvin_pcpsp_problem(path, model)
}

/// Lee una solucion CPIT MineLib y la normaliza a contrato abierto.
#[allow(dead_code)]
pub fn read_minelib_cpit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MinelibScheduleSolution, MineError> {
    read_marvin_cpit_solution(path, model)
}

/// Lee una solucion PCPSP MineLib y la normaliza a contrato abierto.
#[allow(dead_code)]
pub fn read_minelib_pcpsp_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MinelibScheduleSolution, MineError> {
    read_marvin_pcpsp_solution(path, model)
}

/// Lee una solucion relajada LP CPIT MineLib.
#[allow(dead_code)]
pub fn read_minelib_lp_cpit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MinelibScheduleSolution, MineError> {
    read_marvin_lp_cpit_solution(path, model)
}

/// Lee una solucion relajada LP PCPSP MineLib.
#[allow(dead_code)]
pub fn read_minelib_lp_pcpsp_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MinelibScheduleSolution, MineError> {
    read_marvin_lp_pcpsp_solution(path, model)
}

/// Resume una solucion MineLib normalizada con el mismo auditor usado en Marvin.
#[allow(dead_code)]
pub fn summarize_minelib_schedule_solution(
    problem: &MinelibScheduleProblem,
    solution: &MinelibScheduleSolution,
) -> Result<MinelibScheduleSolutionSummary, MineError> {
    summarize_marvin_schedule_solution(problem, solution)
}

/// Tipo de problema de scheduling Marvin normalizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarvinScheduleProblemKind {
    /// Constrained pit limit problem con una sola alternativa de destino.
    Cpit,
    /// Period-constrained production scheduling problem con múltiples destinos.
    Pcpsp,
}

impl MarvinScheduleProblemKind {
    fn expected_type_name(self) -> &'static str {
        match self {
            MarvinScheduleProblemKind::Cpit => "CPIT",
            MarvinScheduleProblemKind::Pcpsp => "PCPSP",
        }
    }
}

/// Límite de recurso por periodo dentro del contrato Marvin normalizado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinResourceConstraintLimit {
    /// Índice del recurso.
    pub resource_index: usize,
    /// Índice del periodo.
    pub period_index: usize,
    /// Relación del límite (`L`, `G`, `E`).
    pub relation: char,
    /// Valor límite configurado.
    pub limit: f64,
}

/// Término de objetivo por bloque y destino dentro del benchmark Marvin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinObjectiveTerm {
    /// Índice lineal del bloque dentro del `BlockModel`.
    pub linear_index: usize,
    /// Índice del destino asociado.
    pub destination_index: usize,
    /// Valor económico no descontado del bloque en ese destino.
    pub objective_value: f64,
}

/// Coeficiente de recurso por bloque y destino dentro del benchmark Marvin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinResourceCoefficient {
    /// Índice lineal del bloque dentro del `BlockModel`.
    pub linear_index: usize,
    /// Índice del destino asociado.
    pub destination_index: usize,
    /// Índice del recurso asociado.
    pub resource_index: usize,
    /// Coeficiente del recurso para esa combinación.
    pub coefficient: f64,
}

/// Problema de scheduling Marvin normalizado a un contrato abierto y serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinScheduleProblem {
    /// Tipo de problema fuente.
    pub kind: MarvinScheduleProblemKind,
    /// Nombre de la instancia.
    pub name: String,
    /// Número de bloques declarados por el benchmark.
    pub block_count: usize,
    /// Número de periodos declarados por el benchmark.
    pub period_count: usize,
    /// Número de destinos soportados por el benchmark.
    pub destination_count: usize,
    /// Número de restricciones laterales de recursos.
    pub resource_constraint_count: usize,
    /// Número de restricciones laterales generales.
    pub general_constraint_count: usize,
    /// Tasa de descuento usada por el benchmark.
    pub discount_rate: f64,
    /// Límites de recursos por periodo.
    pub resource_constraint_limits: Vec<MarvinResourceConstraintLimit>,
    /// Términos del objetivo por bloque/destino.
    pub objective_terms: Vec<MarvinObjectiveTerm>,
    /// Coeficientes de recursos por bloque/destino.
    pub resource_coefficients: Vec<MarvinResourceCoefficient>,
}

/// Asignación normalizada dentro de una solución Marvin CPIT/PCPSP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinScheduleAssignment {
    /// Índice lineal del bloque asignado.
    pub linear_index: usize,
    /// Índice del destino asignado.
    pub destination_index: usize,
    /// Índice del periodo asignado.
    pub period_index: usize,
    /// Fracción de la asignación para ese bloque/destino/periodo.
    pub fraction: f64,
}

/// Solución CPIT/PCPSP normalizada a un contrato abierto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinScheduleSolution {
    /// Tipo de problema al que corresponde la solución.
    pub kind: MarvinScheduleProblemKind,
    /// Asignaciones discretas o fraccionales del benchmark.
    pub assignments: Vec<MarvinScheduleAssignment>,
    /// Cantidad de bloques únicos presentes en la solución.
    pub unique_block_count: usize,
}

/// Resumen agregado de uso de un recurso dentro de una solución Marvin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinScheduleResourceSummary {
    /// Índice del recurso resumido.
    pub resource_index: usize,
    /// Cantidad de periodos donde el recurso tuvo uso positivo.
    pub active_period_count: usize,
    /// Uso máximo observado en un periodo.
    pub max_period_usage: f64,
    /// Límite máximo configurado para ese recurso.
    pub max_period_limit: Option<f64>,
    /// Mayor exceso observado respecto al límite configurado.
    pub max_period_excess: f64,
}

/// Resumen agregado de una solución Marvin evaluada contra su problema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarvinScheduleSolutionSummary {
    /// Cantidad total de asignaciones presentes en la solución.
    pub assignment_count: usize,
    /// Cantidad de bloques únicos presentes en la solución.
    pub unique_block_count: usize,
    /// Cantidad de asignaciones con fracción distinta de `1.0`.
    pub fractional_assignment_count: usize,
    /// Cantidad de periodos utilizados por la solución.
    pub used_period_count: usize,
    /// Cantidad de destinos utilizados por la solución.
    pub used_destination_count: usize,
    /// Suma de fracciones de todas las asignaciones.
    pub total_fraction: f64,
    /// Menor suma de fracciones por bloque.
    pub min_block_fraction_sum: f64,
    /// Mayor suma de fracciones por bloque.
    pub max_block_fraction_sum: f64,
    /// Valor económico no descontado de la solución.
    pub undiscounted_objective: f64,
    /// Valor económico descontado usando `1 / (1 + r)^period`.
    pub discounted_objective: f64,
    /// Resumen agregado de recursos por solución.
    pub resource_summaries: Vec<MarvinScheduleResourceSummary>,
}

/// Lee `marvin.cpit` y lo normaliza como contrato abierto de scheduling.
pub fn read_marvin_cpit_problem(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MarvinScheduleProblem, MineError> {
    read_marvin_schedule_problem(path, model, MarvinScheduleProblemKind::Cpit)
}

/// Lee `marvin.pcpsp` y lo normaliza como contrato abierto de scheduling.
pub fn read_marvin_pcpsp_problem(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MarvinScheduleProblem, MineError> {
    read_marvin_schedule_problem(path, model, MarvinScheduleProblemKind::Pcpsp)
}

/// Lee `marvin_cpit_gmunoz120723.sol` y lo normaliza como asignaciones CPIT.
pub fn read_marvin_cpit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MarvinScheduleSolution, MineError> {
    read_marvin_schedule_solution(path, model, MarvinScheduleProblemKind::Cpit)
}

/// Lee `marvin_pcpsp_gmunoz120723.sol` y lo normaliza como asignaciones PCPSP.
pub fn read_marvin_pcpsp_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MarvinScheduleSolution, MineError> {
    read_marvin_schedule_solution(path, model, MarvinScheduleProblemKind::Pcpsp)
}

/// Lee `marvin.LPcpit` y lo normaliza como solución relajada CPIT.
#[allow(dead_code)]
pub fn read_marvin_lp_cpit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MarvinScheduleSolution, MineError> {
    read_marvin_schedule_solution(path, model, MarvinScheduleProblemKind::Cpit)
}

/// Lee `marvin.LPpcpsp` y lo normaliza como solución relajada PCPSP.
#[allow(dead_code)]
pub fn read_marvin_lp_pcpsp_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<MarvinScheduleSolution, MineError> {
    read_marvin_schedule_solution(path, model, MarvinScheduleProblemKind::Pcpsp)
}

/// Resume una solución Marvin verificando rangos, objetivo y uso de recursos.
pub fn summarize_marvin_schedule_solution(
    problem: &MarvinScheduleProblem,
    solution: &MarvinScheduleSolution,
) -> Result<MarvinScheduleSolutionSummary, MineError> {
    if problem.kind != solution.kind {
        return Err(MineError::validation(format!(
            "Marvin solution kind `{:?}` does not match problem kind `{:?}`",
            solution.kind, problem.kind
        )));
    }

    let objective_lookup = problem
        .objective_terms
        .iter()
        .map(|term| {
            (
                (term.linear_index, term.destination_index),
                term.objective_value,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let resource_lookup = problem
        .resource_coefficients
        .iter()
        .map(|coefficient| {
            (
                (
                    coefficient.linear_index,
                    coefficient.destination_index,
                    coefficient.resource_index,
                ),
                coefficient.coefficient,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let limit_lookup = problem
        .resource_constraint_limits
        .iter()
        .map(|limit| ((limit.resource_index, limit.period_index), limit.limit))
        .collect::<BTreeMap<_, _>>();
    let relation_lookup = problem
        .resource_constraint_limits
        .iter()
        .map(|limit| ((limit.resource_index, limit.period_index), limit.relation))
        .collect::<BTreeMap<_, _>>();

    let mut used_periods = BTreeSet::new();
    let mut used_destinations = BTreeSet::new();
    let mut total_fraction = 0.0_f64;
    let mut undiscounted_objective = 0.0_f64;
    let mut discounted_objective = 0.0_f64;
    let mut fractional_assignment_count = 0usize;
    let mut block_fraction_sums = BTreeMap::<usize, f64>::new();
    let mut period_resource_usage =
        vec![vec![0.0_f64; problem.resource_constraint_count]; problem.period_count];

    for assignment in &solution.assignments {
        if assignment.destination_index >= problem.destination_count {
            return Err(MineError::validation(format!(
                "Marvin solution destination {} is outside problem destination range 0..{}",
                assignment.destination_index,
                problem.destination_count.saturating_sub(1)
            )));
        }
        if assignment.period_index >= problem.period_count {
            return Err(MineError::validation(format!(
                "Marvin solution period {} is outside problem period range 0..{}",
                assignment.period_index,
                problem.period_count.saturating_sub(1)
            )));
        }

        let objective_value = objective_lookup
            .get(&(assignment.linear_index, assignment.destination_index))
            .copied()
            .ok_or_else(|| {
                MineError::validation(format!(
                    "Marvin solution references block {} destination {} without objective term",
                    assignment.linear_index, assignment.destination_index
                ))
            })?;
        let discount_factor = (1.0 + problem.discount_rate).powi(assignment.period_index as i32);
        undiscounted_objective += objective_value * assignment.fraction;
        discounted_objective += objective_value * assignment.fraction / discount_factor;
        total_fraction += assignment.fraction;
        used_periods.insert(assignment.period_index);
        used_destinations.insert(assignment.destination_index);

        if (assignment.fraction - 1.0).abs() > 1e-9 {
            fractional_assignment_count += 1;
        }

        *block_fraction_sums
            .entry(assignment.linear_index)
            .or_insert(0.0) += assignment.fraction;

        for (resource_index, resource_usage) in period_resource_usage[assignment.period_index]
            .iter_mut()
            .enumerate()
            .take(problem.resource_constraint_count)
        {
            let coefficient = resource_lookup
                .get(&(
                    assignment.linear_index,
                    assignment.destination_index,
                    resource_index,
                ))
                .copied()
                .unwrap_or(0.0);
            *resource_usage += coefficient * assignment.fraction;
        }
    }

    for (linear_index, fraction_sum) in &block_fraction_sums {
        if *fraction_sum > 1.000_001 {
            return Err(MineError::validation(format!(
                "Marvin solution block {linear_index} sums to {fraction_sum}, exceeding 1.0"
            )));
        }
    }

    let min_block_fraction_sum = block_fraction_sums
        .values()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);
    let max_block_fraction_sum = block_fraction_sums
        .values()
        .copied()
        .reduce(f64::max)
        .unwrap_or(0.0);

    let resource_summaries = (0..problem.resource_constraint_count)
        .map(|resource_index| {
            let active_period_count = (0..problem.period_count)
                .filter(|period_index| period_resource_usage[*period_index][resource_index] > 0.0)
                .count();
            let max_period_usage = (0..problem.period_count)
                .map(|period_index| period_resource_usage[period_index][resource_index])
                .reduce(f64::max)
                .unwrap_or(0.0);
            let max_period_limit = (0..problem.period_count)
                .filter_map(|period_index| {
                    limit_lookup.get(&(resource_index, period_index)).copied()
                })
                .reduce(f64::max);
            let max_period_excess = (0..problem.period_count)
                .filter_map(|period_index| {
                    let limit = limit_lookup.get(&(resource_index, period_index)).copied()?;
                    let relation = relation_lookup
                        .get(&(resource_index, period_index))
                        .copied()?;
                    let usage = period_resource_usage[period_index][resource_index];
                    let excess = match relation {
                        'L' => (usage - limit).max(0.0),
                        'G' => (limit - usage).max(0.0),
                        'E' => (usage - limit).abs(),
                        _ => {
                            return Some(Err(MineError::validation(format!(
                                "Marvin resource relation `{relation}` is not supported"
                            ))));
                        }
                    };
                    Some(Ok(excess))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .reduce(f64::max)
                .unwrap_or(0.0);

            Ok(MarvinScheduleResourceSummary {
                resource_index,
                active_period_count,
                max_period_usage,
                max_period_limit,
                max_period_excess,
            })
        })
        .collect::<Result<Vec<_>, MineError>>()?;

    Ok(MarvinScheduleSolutionSummary {
        assignment_count: solution.assignments.len(),
        unique_block_count: block_fraction_sums.len(),
        fractional_assignment_count,
        used_period_count: used_periods.len(),
        used_destination_count: used_destinations.len(),
        total_fraction,
        min_block_fraction_sum,
        max_block_fraction_sum,
        undiscounted_objective,
        discounted_objective,
        resource_summaries,
    })
}

fn read_marvin_schedule_problem(
    path: impl AsRef<Path>,
    model: &BlockModel,
    kind: MarvinScheduleProblemKind,
) -> Result<MarvinScheduleProblem, MineError> {
    let lines = read_text_lines(path.as_ref(), "Marvin scheduling problem")?;
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;

    let mut name = None;
    let mut declared_type = None;
    let mut block_count = None;
    let mut period_count = None;
    let mut destination_count = None;
    let mut resource_constraint_count = None;
    let mut general_constraint_count = None;
    let mut discount_rate = None;
    let mut resource_limits_marker = None;
    let mut objective_marker = None;
    let mut resource_coefficients_marker = None;

    for (line_offset, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(value) = trimmed.strip_prefix("NAME:") {
            name = Some(value.trim().to_owned());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("TYPE:") {
            declared_type = Some(value.trim().to_owned());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("NBLOCKS:") {
            block_count = Some(parse_usize_field(value.trim(), line_offset + 1, "NBLOCKS")?);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("NPERIODS:") {
            period_count = Some(parse_usize_field(
                value.trim(),
                line_offset + 1,
                "NPERIODS",
            )?);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("NDESTINATIONS:") {
            destination_count = Some(parse_usize_field(
                value.trim(),
                line_offset + 1,
                "NDESTINATIONS",
            )?);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("NRESOURCE_SIDE_CONSTRAINTS:") {
            resource_constraint_count = Some(parse_usize_field(
                value.trim(),
                line_offset + 1,
                "NRESOURCE_SIDE_CONSTRAINTS",
            )?);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("NGENERAL_SIDE_CONSTRAINTS:") {
            general_constraint_count = Some(parse_usize_field(
                value.trim(),
                line_offset + 1,
                "NGENERAL_SIDE_CONSTRAINTS",
            )?);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("DISCOUNT_RATE:") {
            discount_rate = Some(parse_f64_field(
                value.trim(),
                line_offset + 1,
                "DISCOUNT_RATE",
            )?);
            continue;
        }
        if trimmed.starts_with("RESOURCE_CONSTRAINT_LIMITS:") {
            resource_limits_marker = Some(line_offset);
            continue;
        }
        if trimmed.starts_with("OBJECTIVE_FUNCTION:") {
            objective_marker = Some(line_offset);
            continue;
        }
        if trimmed.starts_with("RESOURCE_CONSTRAINT_COEFFICIENTS:") {
            resource_coefficients_marker = Some(line_offset);
        }
    }

    let Some(name) = name else {
        return Err(MineError::validation(
            "Marvin scheduling problem must declare NAME".to_owned(),
        ));
    };
    let Some(declared_type) = declared_type else {
        return Err(MineError::validation(
            "Marvin scheduling problem must declare TYPE".to_owned(),
        ));
    };
    if declared_type != kind.expected_type_name() {
        return Err(MineError::validation(format!(
            "Marvin scheduling problem TYPE `{declared_type}` does not match expected `{}`",
            kind.expected_type_name()
        )));
    }

    let block_count = block_count.ok_or_else(|| {
        MineError::validation("Marvin scheduling problem must declare NBLOCKS".to_owned())
    })?;
    if block_count != model.block_count() {
        return Err(MineError::validation(format!(
            "Marvin scheduling problem declares {block_count} blocks but model contains {}",
            model.block_count()
        )));
    }

    let period_count = period_count.ok_or_else(|| {
        MineError::validation("Marvin scheduling problem must declare NPERIODS".to_owned())
    })?;
    let resource_constraint_count = resource_constraint_count.ok_or_else(|| {
        MineError::validation(
            "Marvin scheduling problem must declare NRESOURCE_SIDE_CONSTRAINTS".to_owned(),
        )
    })?;
    let discount_rate = discount_rate.ok_or_else(|| {
        MineError::validation("Marvin scheduling problem must declare DISCOUNT_RATE".to_owned())
    })?;
    if !discount_rate.is_finite() || discount_rate < 0.0 {
        return Err(MineError::validation(
            "Marvin scheduling problem DISCOUNT_RATE must be finite and non-negative".to_owned(),
        ));
    }

    let destination_count = match kind {
        MarvinScheduleProblemKind::Cpit => 1,
        MarvinScheduleProblemKind::Pcpsp => destination_count.ok_or_else(|| {
            MineError::validation("Marvin PCPSP problem must declare NDESTINATIONS".to_owned())
        })?,
    };
    let general_constraint_count = general_constraint_count.unwrap_or(0);

    let resource_limits_marker = resource_limits_marker.ok_or_else(|| {
        MineError::validation(
            "Marvin scheduling problem must declare RESOURCE_CONSTRAINT_LIMITS".to_owned(),
        )
    })?;
    let objective_marker = objective_marker.ok_or_else(|| {
        MineError::validation(
            "Marvin scheduling problem must declare OBJECTIVE_FUNCTION".to_owned(),
        )
    })?;
    let resource_coefficients_marker = resource_coefficients_marker.ok_or_else(|| {
        MineError::validation(
            "Marvin scheduling problem must declare RESOURCE_CONSTRAINT_COEFFICIENTS".to_owned(),
        )
    })?;

    let resource_constraint_limits = parse_resource_constraint_limits(
        &lines[(resource_limits_marker + 1)..objective_marker],
        resource_constraint_count,
        period_count,
    )?;
    let objective_terms = parse_objective_terms(
        &lines[(objective_marker + 1)..resource_coefficients_marker],
        &block_id_to_linear_index,
        kind,
        destination_count,
    )?;
    let resource_coefficients = parse_resource_coefficients(
        &lines[(resource_coefficients_marker + 1)..],
        &block_id_to_linear_index,
        kind,
        destination_count,
        resource_constraint_count,
    )?;

    let expected_objective_terms = block_count * destination_count;
    if objective_terms.len() != expected_objective_terms {
        return Err(MineError::validation(format!(
            "Marvin scheduling problem contains {} objective terms but expected {expected_objective_terms}",
            objective_terms.len()
        )));
    }

    Ok(MarvinScheduleProblem {
        kind,
        name,
        block_count,
        period_count,
        destination_count,
        resource_constraint_count,
        general_constraint_count,
        discount_rate,
        resource_constraint_limits,
        objective_terms,
        resource_coefficients,
    })
}

fn read_marvin_schedule_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
    kind: MarvinScheduleProblemKind,
) -> Result<MarvinScheduleSolution, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin schedule solution file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut assignments = Vec::new();
    let mut blocks = BTreeSet::new();

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin schedule solution line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(MineError::validation(format!(
                "Marvin schedule solution line {line_number} must contain block_id destination period fraction"
            )));
        }

        let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
        let destination_index = parse_usize_field(fields[1], line_number, "destination_index")?;
        let period_index = parse_usize_field(fields[2], line_number, "period_index")?;
        let fraction = parse_f64_field(fields[3], line_number, "fraction")?;

        if kind == MarvinScheduleProblemKind::Cpit && destination_index != 0 {
            return Err(MineError::validation(format!(
                "Marvin CPIT solution line {line_number} must use destination 0"
            )));
        }
        if !fraction.is_finite() || fraction <= 0.0 || fraction > 1.0 + 1e-9 {
            return Err(MineError::validation(format!(
                "Marvin schedule solution line {line_number} contains invalid fraction `{fraction}`"
            )));
        }

        let linear_index = map_block_id(source_block_id, &block_id_to_linear_index)?;
        assignments.push(MarvinScheduleAssignment {
            linear_index,
            destination_index,
            period_index,
            fraction,
        });
        blocks.insert(linear_index);
    }

    if assignments.is_empty() {
        return Err(MineError::validation(
            "Marvin schedule solution must contain at least one assignment".to_owned(),
        ));
    }

    Ok(MarvinScheduleSolution {
        kind,
        assignments,
        unique_block_count: blocks.len(),
    })
}

fn parse_resource_constraint_limits(
    lines: &[String],
    resource_constraint_count: usize,
    period_count: usize,
) -> Result<Vec<MarvinResourceConstraintLimit>, MineError> {
    let mut limits = Vec::new();
    let mut seen = BTreeSet::new();

    for (line_offset, line) in lines.iter().enumerate() {
        let line_number = line_offset + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(MineError::validation(format!(
                "Marvin resource constraint limit line {line_number} must contain resource period relation limit"
            )));
        }

        let resource_index = parse_usize_field(fields[0], line_number, "resource_index")?;
        let period_index = parse_usize_field(fields[1], line_number, "period_index")?;
        let relation = parse_relation_field(fields[2], line_number)?;
        let limit = parse_f64_field(fields[3], line_number, "limit")?;

        if resource_index >= resource_constraint_count {
            return Err(MineError::validation(format!(
                "Marvin resource constraint limit line {line_number} references resource {resource_index} outside 0..{}",
                resource_constraint_count.saturating_sub(1)
            )));
        }
        if period_index >= period_count {
            return Err(MineError::validation(format!(
                "Marvin resource constraint limit line {line_number} references period {period_index} outside 0..{}",
                period_count.saturating_sub(1)
            )));
        }
        if !seen.insert((resource_index, period_index)) {
            return Err(MineError::validation(format!(
                "Marvin resource constraint limit duplicates resource {resource_index} period {period_index}"
            )));
        }

        limits.push(MarvinResourceConstraintLimit {
            resource_index,
            period_index,
            relation,
            limit,
        });
    }

    let expected_limit_count = resource_constraint_count * period_count;
    if limits.len() != expected_limit_count {
        return Err(MineError::validation(format!(
            "Marvin resource limits contain {} rows but expected {expected_limit_count}",
            limits.len()
        )));
    }

    Ok(limits)
}

fn parse_objective_terms(
    lines: &[String],
    block_id_to_linear_index: &BTreeMap<i64, usize>,
    kind: MarvinScheduleProblemKind,
    destination_count: usize,
) -> Result<Vec<MarvinObjectiveTerm>, MineError> {
    let mut terms = Vec::new();
    let mut seen = BTreeSet::new();

    for (line_offset, line) in lines.iter().enumerate() {
        let line_number = line_offset + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
        let linear_index = map_block_id(source_block_id, block_id_to_linear_index)?;

        match kind {
            MarvinScheduleProblemKind::Cpit => {
                if fields.len() != 2 {
                    return Err(MineError::validation(format!(
                        "Marvin CPIT objective line {line_number} must contain block_id and objective_value"
                    )));
                }
                let objective_value = parse_f64_field(fields[1], line_number, "objective_value")?;
                if !seen.insert((linear_index, 0usize)) {
                    return Err(MineError::validation(format!(
                        "Marvin objective duplicates block {linear_index} destination 0"
                    )));
                }
                terms.push(MarvinObjectiveTerm {
                    linear_index,
                    destination_index: 0,
                    objective_value,
                });
            }
            MarvinScheduleProblemKind::Pcpsp => {
                if fields.len() != destination_count + 1 {
                    return Err(MineError::validation(format!(
                        "Marvin PCPSP objective line {line_number} must contain block_id plus {destination_count} destination values"
                    )));
                }
                for destination_index in 0..destination_count {
                    let objective_value = parse_f64_field(
                        fields[destination_index + 1],
                        line_number,
                        "objective_value",
                    )?;
                    if !seen.insert((linear_index, destination_index)) {
                        return Err(MineError::validation(format!(
                            "Marvin objective duplicates block {linear_index} destination {destination_index}"
                        )));
                    }
                    terms.push(MarvinObjectiveTerm {
                        linear_index,
                        destination_index,
                        objective_value,
                    });
                }
            }
        }
    }

    Ok(terms)
}

fn parse_resource_coefficients(
    lines: &[String],
    block_id_to_linear_index: &BTreeMap<i64, usize>,
    kind: MarvinScheduleProblemKind,
    destination_count: usize,
    resource_constraint_count: usize,
) -> Result<Vec<MarvinResourceCoefficient>, MineError> {
    let mut coefficients = Vec::new();
    let mut seen = BTreeSet::new();

    for (line_offset, line) in lines.iter().enumerate() {
        let line_number = line_offset + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "EOF" {
            break;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        match kind {
            MarvinScheduleProblemKind::Cpit => {
                if fields.len() != 3 {
                    return Err(MineError::validation(format!(
                        "Marvin CPIT resource coefficient line {line_number} must contain block_id resource_index coefficient"
                    )));
                }
                let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
                let linear_index = map_block_id(source_block_id, block_id_to_linear_index)?;
                let resource_index = parse_usize_field(fields[1], line_number, "resource_index")?;
                if resource_index >= resource_constraint_count {
                    return Err(MineError::validation(format!(
                        "Marvin CPIT resource coefficient line {line_number} references resource {resource_index} outside 0..{}",
                        resource_constraint_count.saturating_sub(1)
                    )));
                }
                let coefficient = parse_f64_field(fields[2], line_number, "coefficient")?;
                if !seen.insert((linear_index, 0usize, resource_index)) {
                    return Err(MineError::validation(format!(
                        "Marvin resource coefficient duplicates block {linear_index} destination 0 resource {resource_index}"
                    )));
                }
                coefficients.push(MarvinResourceCoefficient {
                    linear_index,
                    destination_index: 0,
                    resource_index,
                    coefficient,
                });
            }
            MarvinScheduleProblemKind::Pcpsp => {
                if fields.len() != 4 {
                    return Err(MineError::validation(format!(
                        "Marvin PCPSP resource coefficient line {line_number} must contain block_id destination_index resource_index coefficient"
                    )));
                }
                let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
                let linear_index = map_block_id(source_block_id, block_id_to_linear_index)?;
                let destination_index =
                    parse_usize_field(fields[1], line_number, "destination_index")?;
                let resource_index = parse_usize_field(fields[2], line_number, "resource_index")?;
                if destination_index >= destination_count {
                    return Err(MineError::validation(format!(
                        "Marvin PCPSP resource coefficient line {line_number} references destination {destination_index} outside 0..{}",
                        destination_count.saturating_sub(1)
                    )));
                }
                if resource_index >= resource_constraint_count {
                    return Err(MineError::validation(format!(
                        "Marvin PCPSP resource coefficient line {line_number} references resource {resource_index} outside 0..{}",
                        resource_constraint_count.saturating_sub(1)
                    )));
                }
                let coefficient = parse_f64_field(fields[3], line_number, "coefficient")?;
                if !seen.insert((linear_index, destination_index, resource_index)) {
                    return Err(MineError::validation(format!(
                        "Marvin resource coefficient duplicates block {linear_index} destination {destination_index} resource {resource_index}"
                    )));
                }
                coefficients.push(MarvinResourceCoefficient {
                    linear_index,
                    destination_index,
                    resource_index,
                    coefficient,
                });
            }
        }
    }

    Ok(coefficients)
}

fn read_text_lines(path: &Path, label: &str) -> Result<Vec<String>, MineError> {
    let file = File::open(path).map_err(|error| MineError::Io {
        message: format!("unable to open {label}: {error}"),
    })?;
    let reader = BufReader::new(file);
    reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| MineError::Io {
            message: format!("unable to read {label}: {error}"),
        })
}

fn source_block_id_to_linear_index(model: &BlockModel) -> Result<BTreeMap<i64, usize>, MineError> {
    let source_block_id = ColumnId::new("source_block_id")?;
    let Some(column) = model.column(&source_block_id) else {
        return Err(MineError::schema(
            "Marvin normalization requires `source_block_id` column in the block model",
        ));
    };
    let ColumnData::Integers(source_ids) = column else {
        return Err(MineError::schema(
            "Marvin normalization requires `source_block_id` to be an integer column",
        ));
    };

    let mut mapping = BTreeMap::new();
    for (row_index, source_id) in source_ids.iter().enumerate() {
        let linear_index = model.linear_index_at(row_index)?;
        if mapping.insert(*source_id, linear_index).is_some() {
            return Err(MineError::validation(format!(
                "duplicate Marvin source_block_id `{source_id}` found in model"
            )));
        }
    }

    Ok(mapping)
}

fn map_block_id(
    source_block_id: i64,
    block_id_to_linear_index: &BTreeMap<i64, usize>,
) -> Result<usize, MineError> {
    block_id_to_linear_index
        .get(&source_block_id)
        .copied()
        .ok_or_else(|| {
            MineError::validation(format!(
                "Marvin benchmark artifact references unknown source_block_id `{source_block_id}`"
            ))
        })
}

fn parse_i64_field(value: &str, line_number: usize, field_name: &str) -> Result<i64, MineError> {
    value.parse::<i64>().map_err(|error| {
        MineError::validation(format!(
            "Marvin benchmark line {line_number} contains invalid integer in {field_name}: {error}"
        ))
    })
}

fn parse_f64_field(value: &str, line_number: usize, field_name: &str) -> Result<f64, MineError> {
    value.parse::<f64>().map_err(|error| {
        MineError::validation(format!(
            "Marvin benchmark line {line_number} contains invalid float in {field_name}: {error}"
        ))
    })
}

fn parse_usize_field(
    value: &str,
    line_number: usize,
    field_name: &str,
) -> Result<usize, MineError> {
    value.parse::<usize>().map_err(|error| {
        MineError::validation(format!(
            "Marvin benchmark line {line_number} contains invalid usize in {field_name}: {error}"
        ))
    })
}

fn parse_relation_field(value: &str, line_number: usize) -> Result<char, MineError> {
    let mut chars = value.chars();
    let Some(relation) = chars.next() else {
        return Err(MineError::validation(format!(
            "Marvin benchmark line {line_number} contains empty relation field"
        )));
    };
    if chars.next().is_some() {
        return Err(MineError::validation(format!(
            "Marvin benchmark line {line_number} relation `{value}` must be a single character"
        )));
    }
    match relation {
        'L' | 'G' | 'E' => Ok(relation),
        _ => Err(MineError::validation(format!(
            "Marvin benchmark line {line_number} contains unsupported relation `{relation}`"
        ))),
    }
}
