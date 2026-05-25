use std::collections::BTreeMap;

use mine_blockmodel::{BlockModel, SpatialExtent};
use mine_core::{Coordinate3D, GridDefinition, MineError};
use mine_indexing::{GridIndex, ijk_to_xyz, linear_to_ijk, xyz_to_ijk};

use crate::options::validate_tolerance;
use crate::{ValidationIssue, ValidationIssueCode, ValidationReport, ValidationSeverity};

/// Detecta índices `i/j/k` duplicados antes de materializar un `BlockModel`.
#[must_use]
pub fn validate_duplicate_block_indices(indices: &[GridIndex]) -> ValidationReport {
    build_duplicate_index_report(
        indices,
        "indices",
        "Deduplicate source rows or aggregate them before materializing a BlockModel.",
    )
}

/// Detecta coordenadas que se normalizan al mismo bloque según la grilla declarada.
pub fn validate_duplicate_block_coordinates(
    grid: &GridDefinition,
    coordinates: &[Coordinate3D],
    tolerance: f64,
) -> Result<ValidationReport, MineError> {
    validate_tolerance(tolerance)?;
    let normalized_indices = coordinates
        .iter()
        .copied()
        .map(|coordinate| xyz_to_ijk(grid, coordinate, tolerance))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(build_duplicate_index_report(
        &normalized_indices,
        "coordinates",
        "Deduplicate or snap source coordinates before materializing a BlockModel.",
    ))
}

/// Valida la consistencia espacial del modelo regular usando el motor de indexing.
#[must_use]
pub fn validate_block_model_regular_grid(model: &BlockModel, tolerance: f64) -> ValidationReport {
    let mut report = ValidationReport::new();
    let mut mismatches: Vec<(usize, GridIndex, GridIndex)> = Vec::new();

    for row_index in 0..model.block_count() {
        let linear_index = match model.linear_index_at(row_index) {
            Ok(linear_index) => linear_index,
            Err(error) => {
                report.push(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        ValidationIssueCode::GridIndexRoundtripMismatch,
                        error.to_string(),
                    )
                    .with_location(format!("row_index:{row_index}"))
                    .with_affected_count(1)
                    .with_recommendation(
                        "Check sparse materialization indices and row ordering for the block model.",
                    ),
                );
                continue;
            }
        };
        let index = match linear_to_ijk(model.grid(), linear_index) {
            Ok(index) => index,
            Err(error) => {
                report.push(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        ValidationIssueCode::GridIndexRoundtripMismatch,
                        error.to_string(),
                    )
                    .with_location(format!("linear_index:{linear_index}"))
                    .with_affected_count(1)
                    .with_recommendation(
                        "Check the grid shape and index ordering used to define the block model.",
                    ),
                );
                continue;
            }
        };

        let center = match ijk_to_xyz(model.grid(), index) {
            Ok(center) => center,
            Err(error) => {
                report.push(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        ValidationIssueCode::GridIndexRoundtripMismatch,
                        error.to_string(),
                    )
                    .with_location(format!("ijk:{},{},{}", index.i(), index.j(), index.k()))
                    .with_affected_count(1)
                    .with_recommendation(
                        "Check the grid definition and block dimensions for invalid geometry.",
                    ),
                );
                continue;
            }
        };

        let recovered = match xyz_to_ijk(model.grid(), center, tolerance) {
            Ok(recovered) => recovered,
            Err(error) => {
                report.push(
                    ValidationIssue::new(
                        ValidationSeverity::Error,
                        ValidationIssueCode::GridIndexRoundtripMismatch,
                        error.to_string(),
                    )
                    .with_location(format!("linear_index:{linear_index}"))
                    .with_affected_count(1)
                    .with_recommendation(
                        "Check that coordinates and index transforms use the same grid convention.",
                    ),
                );
                continue;
            }
        };

        if recovered != index {
            mismatches.push((linear_index, index, recovered));
        }
    }

    if !mismatches.is_empty() {
        let example_mismatch = &mismatches[0];
        report.push(
            ValidationIssue::new(
                ValidationSeverity::Error,
                ValidationIssueCode::GridIndexRoundtripMismatch,
                format!(
                    "grid indexing roundtrip failed for {} block(s); example linear index {} mapped from ({},{},{}) to ({},{},{})",
                    mismatches.len(),
                    example_mismatch.0,
                    example_mismatch.1.i(),
                    example_mismatch.1.j(),
                    example_mismatch.1.k(),
                    example_mismatch.2.i(),
                    example_mismatch.2.j(),
                    example_mismatch.2.k()
                ),
            )
            .with_affected_count(mismatches.len())
            .with_recommendation(
                "Review grid origin, dimensions, shape and tolerance to ensure indexing is reversible.",
            ),
        );
    }

    report
}

/// Detecta bloques faltantes cuando el modelo debería materializar una grilla densa completa.
#[must_use]
pub fn validate_block_model_missing_blocks(
    model: &BlockModel,
    sparse_allowed: bool,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    let missing_linear_indices = model.missing_linear_indices();

    if missing_linear_indices.is_empty() || sparse_allowed {
        return report;
    }

    let example_indices = missing_linear_indices
        .iter()
        .take(5)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    report.push(
        ValidationIssue::new(
            ValidationSeverity::Error,
            ValidationIssueCode::MissingBlocksDetected,
            format!(
                "block model materializes {} of {} grid cell(s); {} block(s) are missing (examples: {})",
                model.block_count(),
                model.grid_cell_count(),
                missing_linear_indices.len(),
                example_indices
            ),
        )
        .with_location("layout")
        .with_affected_count(missing_linear_indices.len())
        .with_recommendation(
            "Use a dense block model when gaps are invalid, or enable sparse validation explicitly when missing cells are intentional.",
        ),
    );

    report
}

/// Valida extents observados vs el envelope nominal esperado por la grilla.
#[must_use]
pub fn validate_block_model_extents(
    model: &BlockModel,
    tolerance: f64,
    sparse_allowed: bool,
) -> ValidationReport {
    let mut report = ValidationReport::new();

    if model.block_count() == 0 {
        if sparse_allowed {
            return report;
        }

        report.push(
            ValidationIssue::new(
                ValidationSeverity::Warning,
                ValidationIssueCode::IncompleteExtent,
                "block model has no materialized rows, so the observed extent is empty",
            )
            .with_location("extent")
            .with_affected_count(model.grid_cell_count())
            .with_recommendation(
                "Materialize at least one block or allow sparse layouts explicitly when partial coverage is intentional.",
            ),
        );

        return report;
    }

    let expected_extent = match expected_grid_extent(model) {
        Ok(extent) => extent,
        Err(error) => {
            report.push(
                ValidationIssue::new(
                    ValidationSeverity::Error,
                    ValidationIssueCode::OversizedExtent,
                    error.to_string(),
                )
                .with_location("extent")
                .with_affected_count(1)
                .with_recommendation(
                    "Review the grid definition before validating observed extents.",
                ),
            );
            return report;
        }
    };
    let observed_extent = match observed_model_extent(model) {
        Ok(extent) => extent,
        Err(error) => {
            report.push(
                ValidationIssue::new(
                    ValidationSeverity::Error,
                    ValidationIssueCode::OversizedExtent,
                    error.to_string(),
                )
                .with_location("extent")
                .with_affected_count(1)
                .with_recommendation(
                    "Check sparse materialization indices and row ordering before validating extents.",
                ),
            );
            return report;
        }
    };

    let minimum_outside = axis_outside(
        observed_extent.minimum.x(),
        expected_extent.minimum.x(),
        tolerance,
        true,
    ) || axis_outside(
        observed_extent.minimum.y(),
        expected_extent.minimum.y(),
        tolerance,
        true,
    ) || axis_outside(
        observed_extent.minimum.z(),
        expected_extent.minimum.z(),
        tolerance,
        true,
    );
    let maximum_outside = axis_outside(
        observed_extent.maximum.x(),
        expected_extent.maximum.x(),
        tolerance,
        false,
    ) || axis_outside(
        observed_extent.maximum.y(),
        expected_extent.maximum.y(),
        tolerance,
        false,
    ) || axis_outside(
        observed_extent.maximum.z(),
        expected_extent.maximum.z(),
        tolerance,
        false,
    );
    let incomplete = axis_inside_gap(
        observed_extent.minimum.x(),
        expected_extent.minimum.x(),
        tolerance,
        true,
    ) || axis_inside_gap(
        observed_extent.minimum.y(),
        expected_extent.minimum.y(),
        tolerance,
        true,
    ) || axis_inside_gap(
        observed_extent.minimum.z(),
        expected_extent.minimum.z(),
        tolerance,
        true,
    ) || axis_inside_gap(
        observed_extent.maximum.x(),
        expected_extent.maximum.x(),
        tolerance,
        false,
    ) || axis_inside_gap(
        observed_extent.maximum.y(),
        expected_extent.maximum.y(),
        tolerance,
        false,
    ) || axis_inside_gap(
        observed_extent.maximum.z(),
        expected_extent.maximum.z(),
        tolerance,
        false,
    );

    if !minimum_outside && !maximum_outside && !incomplete {
        return report;
    }

    let expected_spans = (
        expected_extent.maximum.x() - expected_extent.minimum.x(),
        expected_extent.maximum.y() - expected_extent.minimum.y(),
        expected_extent.maximum.z() - expected_extent.minimum.z(),
    );
    let observed_spans = (
        observed_extent.maximum.x() - observed_extent.minimum.x(),
        observed_extent.maximum.y() - observed_extent.minimum.y(),
        observed_extent.maximum.z() - observed_extent.minimum.z(),
    );
    let translated = spans_match(expected_spans.0, observed_spans.0, tolerance)
        && spans_match(expected_spans.1, observed_spans.1, tolerance)
        && spans_match(expected_spans.2, observed_spans.2, tolerance)
        && shifts_match(
            observed_extent.minimum.x() - expected_extent.minimum.x(),
            observed_extent.maximum.x() - expected_extent.maximum.x(),
            tolerance,
        )
        && shifts_match(
            observed_extent.minimum.y() - expected_extent.minimum.y(),
            observed_extent.maximum.y() - expected_extent.maximum.y(),
            tolerance,
        )
        && shifts_match(
            observed_extent.minimum.z() - expected_extent.minimum.z(),
            observed_extent.maximum.z() - expected_extent.maximum.z(),
            tolerance,
        );

    let (severity, code, recommendation) = if minimum_outside || maximum_outside {
        if translated {
            (
                ValidationSeverity::Error,
                ValidationIssueCode::DisplacedExtent,
                "Review origin, rotation and index ordering to confirm the observed blocks align with the declared grid.",
            )
        } else {
            (
                ValidationSeverity::Error,
                ValidationIssueCode::OversizedExtent,
                "Review coordinates, rotation and materialized indices because observed geometry exceeds the declared grid envelope.",
            )
        }
    } else if sparse_allowed {
        return report;
    } else {
        (
            ValidationSeverity::Warning,
            ValidationIssueCode::IncompleteExtent,
            "Complete the missing grid coverage or allow sparse layouts explicitly when a partial extent is intentional.",
        )
    };

    report.push(
        ValidationIssue::new(
            severity,
            code,
            format!(
                "observed extent {} does not match expected extent {} within tolerance {}",
                format_extent(&observed_extent),
                format_extent(&expected_extent),
                tolerance
            ),
        )
        .with_location("extent")
        .with_affected_count(model.block_count())
        .with_recommendation(recommendation),
    );

    report
}

fn build_duplicate_index_report(
    indices: &[GridIndex],
    location: &'static str,
    recommendation: &'static str,
) -> ValidationReport {
    let mut counts = BTreeMap::<(usize, usize, usize), usize>::new();

    for index in indices {
        *counts.entry((index.i(), index.j(), index.k())).or_insert(0) += 1;
    }

    let duplicate_groups = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect::<Vec<_>>();

    if duplicate_groups.is_empty() {
        return ValidationReport::new();
    }

    let duplicated_rows = duplicate_groups
        .iter()
        .map(|(_, count)| *count)
        .sum::<usize>();
    let examples = duplicate_groups
        .iter()
        .take(5)
        .map(|((i, j, k), count)| format!("({i}, {j}, {k}) x{count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut report = ValidationReport::new();

    report.push(
        ValidationIssue::new(
            ValidationSeverity::Error,
            ValidationIssueCode::DuplicateBlockDetected,
            format!(
                "detected {} duplicated block group(s) across {} row(s); examples: {}",
                duplicate_groups.len(),
                duplicated_rows,
                examples
            ),
        )
        .with_location(location)
        .with_affected_count(duplicated_rows)
        .with_recommendation(recommendation),
    );

    report
}

fn expected_grid_extent(model: &BlockModel) -> Result<SpatialExtent, MineError> {
    let grid = model.grid();
    let dimensions = grid.block_dimensions();
    let shape = grid.shape();
    let mut extent = ExtentAccumulator::new();

    for x in [0.0, shape.nx() as f64 * dimensions.dx()] {
        for y in [0.0, shape.ny() as f64 * dimensions.dy()] {
            for z in [0.0, shape.nz() as f64 * dimensions.dz()] {
                extent.include(local_offset_to_world(model, x, y, z)?)?;
            }
        }
    }

    extent.finish()
}

fn observed_model_extent(model: &BlockModel) -> Result<SpatialExtent, MineError> {
    let dimensions = model.grid().block_dimensions();
    let mut extent = ExtentAccumulator::new();

    for row_index in 0..model.block_count() {
        let linear_index = model.linear_index_at(row_index)?;
        let index = linear_to_ijk(model.grid(), linear_index)?;
        let minimum_local = (
            index.i() as f64 * dimensions.dx(),
            index.j() as f64 * dimensions.dy(),
            index.k() as f64 * dimensions.dz(),
        );
        let maximum_local = (
            minimum_local.0 + dimensions.dx(),
            minimum_local.1 + dimensions.dy(),
            minimum_local.2 + dimensions.dz(),
        );

        for x in [minimum_local.0, maximum_local.0] {
            for y in [minimum_local.1, maximum_local.1] {
                for z in [minimum_local.2, maximum_local.2] {
                    extent.include(local_offset_to_world(model, x, y, z)?)?;
                }
            }
        }
    }

    extent.finish()
}

fn local_offset_to_world(
    model: &BlockModel,
    local_x: f64,
    local_y: f64,
    local_z: f64,
) -> Result<Coordinate3D, MineError> {
    let grid = model.grid();
    let origin = grid.origin();
    let rotation_radians = grid.rotation_degrees().unwrap_or(0.0).to_radians();
    let cosine = rotation_radians.cos();
    let sine = rotation_radians.sin();

    Coordinate3D::new(
        origin.x() + local_x * cosine - local_y * sine,
        origin.y() + local_x * sine + local_y * cosine,
        origin.z() + local_z,
    )
}

fn axis_outside(observed: f64, expected: f64, tolerance: f64, lower_bound: bool) -> bool {
    if lower_bound {
        observed < expected - tolerance
    } else {
        observed > expected + tolerance
    }
}

fn axis_inside_gap(observed: f64, expected: f64, tolerance: f64, lower_bound: bool) -> bool {
    if lower_bound {
        observed > expected + tolerance
    } else {
        observed < expected - tolerance
    }
}

fn spans_match(expected: f64, observed: f64, tolerance: f64) -> bool {
    (observed - expected).abs() <= tolerance
}

fn shifts_match(minimum_shift: f64, maximum_shift: f64, tolerance: f64) -> bool {
    (minimum_shift - maximum_shift).abs() <= tolerance
        && (minimum_shift.abs() > tolerance || maximum_shift.abs() > tolerance)
}

fn format_extent(extent: &SpatialExtent) -> String {
    format!(
        "[({}, {}, {}), ({}, {}, {})]",
        extent.minimum.x(),
        extent.minimum.y(),
        extent.minimum.z(),
        extent.maximum.x(),
        extent.maximum.y(),
        extent.maximum.z()
    )
}

#[derive(Debug, Clone, Copy)]
struct ExtentAccumulator {
    minimum_x: f64,
    minimum_y: f64,
    minimum_z: f64,
    maximum_x: f64,
    maximum_y: f64,
    maximum_z: f64,
    initialized: bool,
}

impl ExtentAccumulator {
    const fn new() -> Self {
        Self {
            minimum_x: 0.0,
            minimum_y: 0.0,
            minimum_z: 0.0,
            maximum_x: 0.0,
            maximum_y: 0.0,
            maximum_z: 0.0,
            initialized: false,
        }
    }

    fn include(&mut self, point: Coordinate3D) -> Result<(), MineError> {
        if !self.initialized {
            self.minimum_x = point.x();
            self.minimum_y = point.y();
            self.minimum_z = point.z();
            self.maximum_x = point.x();
            self.maximum_y = point.y();
            self.maximum_z = point.z();
            self.initialized = true;
            return Ok(());
        }

        self.minimum_x = self.minimum_x.min(point.x());
        self.minimum_y = self.minimum_y.min(point.y());
        self.minimum_z = self.minimum_z.min(point.z());
        self.maximum_x = self.maximum_x.max(point.x());
        self.maximum_y = self.maximum_y.max(point.y());
        self.maximum_z = self.maximum_z.max(point.z());
        Ok(())
    }

    fn finish(self) -> Result<SpatialExtent, MineError> {
        if !self.initialized {
            return Err(MineError::validation(
                "unable to derive an observed extent from an empty block selection",
            ));
        }

        Ok(SpatialExtent {
            minimum: Coordinate3D::new(self.minimum_x, self.minimum_y, self.minimum_z)?,
            maximum: Coordinate3D::new(self.maximum_x, self.maximum_y, self.maximum_z)?,
        })
    }
}
