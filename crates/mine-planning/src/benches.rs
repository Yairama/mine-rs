use mine_blockmodel::BlockModel;
use mine_core::MineError;
use mine_indexing::{ijk_to_xyz, linear_to_ijk};
use serde::{Deserialize, Serialize};

/// Parámetros explícitos para generar bancos desde un modelo regular.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchParameters {
    bench_height: f64,
    origin_elevation: f64,
    tolerance: f64,
}

impl BenchParameters {
    /// Construye parámetros validados para asignación de bancos.
    pub fn new(
        bench_height: f64,
        origin_elevation: f64,
        tolerance: f64,
    ) -> Result<Self, MineError> {
        if !bench_height.is_finite() || bench_height <= 0.0 {
            return Err(MineError::invalid_parameter(
                "bench_height",
                "bench height must be a finite positive value",
            ));
        }

        if !origin_elevation.is_finite() {
            return Err(MineError::invalid_parameter(
                "origin_elevation",
                "origin elevation must be finite",
            ));
        }

        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(MineError::invalid_parameter(
                "tolerance",
                "tolerance must be a finite value greater than or equal to zero",
            ));
        }

        Ok(Self {
            bench_height,
            origin_elevation,
            tolerance,
        })
    }

    /// Altura de banco usada para discretizar elevaciones.
    #[must_use]
    pub const fn bench_height(&self) -> f64 {
        self.bench_height
    }

    /// Elevación base desde la que se numeran bancos.
    #[must_use]
    pub const fn origin_elevation(&self) -> f64 {
        self.origin_elevation
    }

    /// Tolerancia en elevación para resolver fronteras entre bancos.
    #[must_use]
    pub const fn tolerance(&self) -> f64 {
        self.tolerance
    }
}

/// Asignación de un bloque a un banco calculado determinísticamente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchAssignment {
    /// Índice lineal del bloque dentro de la grilla.
    pub linear_index: usize,
    /// Número entero de banco asignado.
    pub bench: i64,
    /// Elevación del centro del bloque usada para la asignación.
    pub center_elevation: f64,
}

/// Asigna un número de banco a cada bloque del modelo usando la elevación de su centro.
pub fn assign_benches(
    model: &BlockModel,
    parameters: &BenchParameters,
) -> Result<Vec<BenchAssignment>, MineError> {
    let mut assignments = Vec::with_capacity(model.block_count());

    for row_index in 0..model.block_count() {
        let linear_index = model.linear_index_at(row_index)?;
        let grid_index = linear_to_ijk(model.grid(), linear_index)?;
        let center = ijk_to_xyz(model.grid(), grid_index)?;
        let center_elevation = center.z();

        assignments.push(BenchAssignment {
            linear_index,
            bench: bench_from_elevation(center_elevation, parameters),
            center_elevation,
        });
    }

    Ok(assignments)
}

fn bench_from_elevation(center_elevation: f64, parameters: &BenchParameters) -> i64 {
    let relative_elevation = center_elevation - parameters.origin_elevation;
    let raw_bench = relative_elevation / parameters.bench_height;
    let lower_bench = raw_bench.floor();
    let upper_boundary = (lower_bench + 1.0) * parameters.bench_height;

    if (upper_boundary - relative_elevation).abs() <= parameters.tolerance {
        lower_bench as i64 + 1
    } else {
        lower_bench as i64
    }
}
