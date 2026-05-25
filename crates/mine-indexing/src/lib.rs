//! Conversiones deterministas entre coordenadas, índices y orden lineal.

use std::collections::BTreeSet;

use mine_core::{Coordinate3D, GridDefinition, MineError};
use serde::{Deserialize, Serialize};

/// Índice tridimensional dentro de una grilla regular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridIndex {
    i: usize,
    j: usize,
    k: usize,
}

impl GridIndex {
    /// Construye un índice tridimensional.
    #[must_use]
    pub const fn new(i: usize, j: usize, k: usize) -> Self {
        Self { i, j, k }
    }

    /// Devuelve el índice `i`.
    #[must_use]
    pub const fn i(&self) -> usize {
        self.i
    }

    /// Devuelve el índice `j`.
    #[must_use]
    pub const fn j(&self) -> usize {
        self.j
    }

    /// Devuelve el índice `k`.
    #[must_use]
    pub const fn k(&self) -> usize {
        self.k
    }
}

/// Regla de conectividad usada para consultar vecinos de un bloque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeighborConnectivity {
    /// Vecinos por cara compartida.
    Face6,
    /// Vecinos por cara o arista compartida.
    Edge18,
    /// Vecinos por cara, arista o esquina compartida.
    Corner26,
}

impl NeighborConnectivity {
    const fn includes_offset(self, non_zero_axes: usize) -> bool {
        match self {
            Self::Face6 => non_zero_axes == 1,
            Self::Edge18 => matches!(non_zero_axes, 1 | 2),
            Self::Corner26 => non_zero_axes > 0,
        }
    }
}

/// Convierte una coordenada espacial a índices `i`, `j`, `k`.
pub fn xyz_to_ijk(
    grid: &GridDefinition,
    coordinate: Coordinate3D,
    tolerance: f64,
) -> Result<GridIndex, MineError> {
    validate_tolerance(tolerance)?;
    let (x_origin, y_origin, z_origin, x_coordinate, y_coordinate, z_coordinate) =
        if uses_rotation(grid) {
            let local_coordinate = local_coordinate_from_world(grid, coordinate)?;
            (
                0.0,
                0.0,
                0.0,
                local_coordinate.x(),
                local_coordinate.y(),
                local_coordinate.z(),
            )
        } else {
            (
                grid.origin().x(),
                grid.origin().y(),
                grid.origin().z(),
                coordinate.x(),
                coordinate.y(),
                coordinate.z(),
            )
        };

    Ok(GridIndex::new(
        locate_axis(
            x_origin,
            grid.block_dimensions().dx(),
            grid.shape().nx(),
            x_coordinate,
            tolerance,
            "x",
        )?,
        locate_axis(
            y_origin,
            grid.block_dimensions().dy(),
            grid.shape().ny(),
            y_coordinate,
            tolerance,
            "y",
        )?,
        locate_axis(
            z_origin,
            grid.block_dimensions().dz(),
            grid.shape().nz(),
            z_coordinate,
            tolerance,
            "z",
        )?,
    ))
}

/// Convierte índices `i`, `j`, `k` a la coordenada del centro del bloque.
pub fn ijk_to_xyz(grid: &GridDefinition, index: GridIndex) -> Result<Coordinate3D, MineError> {
    validate_index(grid, index)?;

    let local_coordinate = Coordinate3D::new(
        (index.i() as f64 + 0.5) * grid.block_dimensions().dx(),
        (index.j() as f64 + 0.5) * grid.block_dimensions().dy(),
        (index.k() as f64 + 0.5) * grid.block_dimensions().dz(),
    )?;

    world_coordinate_from_local(grid, local_coordinate)
}

fn world_coordinate_from_local(
    grid: &GridDefinition,
    local_coordinate: Coordinate3D,
) -> Result<Coordinate3D, MineError> {
    let origin = grid.origin();
    let (sine, cosine) = rotation_sine_cosine(grid);

    Coordinate3D::new(
        origin.x() + (local_coordinate.x() * cosine) - (local_coordinate.y() * sine),
        origin.y() + (local_coordinate.x() * sine) + (local_coordinate.y() * cosine),
        origin.z() + local_coordinate.z(),
    )
}

fn local_coordinate_from_world(
    grid: &GridDefinition,
    coordinate: Coordinate3D,
) -> Result<Coordinate3D, MineError> {
    let origin = grid.origin();
    let translated_x = coordinate.x() - origin.x();
    let translated_y = coordinate.y() - origin.y();
    let translated_z = coordinate.z() - origin.z();
    let (sine, cosine) = rotation_sine_cosine(grid);

    Coordinate3D::new(
        (translated_x * cosine) + (translated_y * sine),
        (-translated_x * sine) + (translated_y * cosine),
        translated_z,
    )
}

/// Convierte índices `i`, `j`, `k` a un índice lineal con orden `i + nx * (j + ny * k)`.
pub fn ijk_to_linear(grid: &GridDefinition, index: GridIndex) -> Result<usize, MineError> {
    validate_index(grid, index)?;

    index
        .k()
        .checked_mul(grid.shape().ny())
        .and_then(|base| base.checked_add(index.j()))
        .and_then(|plane| plane.checked_mul(grid.shape().nx()))
        .and_then(|base| base.checked_add(index.i()))
        .ok_or_else(|| MineError::grid("linear index overflowed grid capacity"))
}

/// Convierte un índice lineal a índices `i`, `j`, `k`.
pub fn linear_to_ijk(grid: &GridDefinition, linear_index: usize) -> Result<GridIndex, MineError> {
    let total_cells = grid.shape().total_cells();

    if linear_index >= total_cells {
        return Err(MineError::grid(format!(
            "linear index `{linear_index}` is outside grid capacity `{total_cells}`"
        )));
    }

    let nx = grid.shape().nx();
    let ny = grid.shape().ny();
    let plane_size = nx * ny;
    let k = linear_index / plane_size;
    let plane_offset = linear_index % plane_size;
    let j = plane_offset / nx;
    let i = plane_offset % nx;

    Ok(GridIndex::new(i, j, k))
}

/// Devuelve vecinos 6/18/26 respetando límites de la grilla y ocupación opcional.
pub fn neighboring_blocks(
    grid: &GridDefinition,
    index: GridIndex,
    connectivity: NeighborConnectivity,
    occupied_linear_indices: Option<&[usize]>,
) -> Result<Vec<GridIndex>, MineError> {
    validate_index(grid, index)?;
    let occupied_linear_indices = validate_occupied_linear_indices(grid, occupied_linear_indices)?;
    let mut neighbors = Vec::new();

    for dk in -1_isize..=1 {
        for dj in -1_isize..=1 {
            for di in -1_isize..=1 {
                let non_zero_axes = [di, dj, dk]
                    .into_iter()
                    .filter(|offset| *offset != 0)
                    .count();

                if !connectivity.includes_offset(non_zero_axes) {
                    continue;
                }

                let Some(neighbor) = offset_index(grid, index, di, dj, dk) else {
                    continue;
                };
                let linear_index = ijk_to_linear(grid, neighbor)?;

                if occupied_linear_indices
                    .as_ref()
                    .is_some_and(|occupied| !occupied.contains(&linear_index))
                {
                    continue;
                }

                neighbors.push(neighbor);
            }
        }
    }

    neighbors.sort_by_key(|neighbor| {
        ijk_to_linear(grid, *neighbor).expect("validated neighbor should linearize")
    });

    Ok(neighbors)
}

fn rotation_sine_cosine(grid: &GridDefinition) -> (f64, f64) {
    grid.rotation_degrees()
        .unwrap_or(0.0)
        .to_radians()
        .sin_cos()
}

fn uses_rotation(grid: &GridDefinition) -> bool {
    grid.rotation_degrees()
        .is_some_and(|rotation_degrees| rotation_degrees != 0.0)
}

fn validate_tolerance(tolerance: f64) -> Result<(), MineError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        Err(MineError::numeric(
            "tolerance must be finite and greater than or equal to zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_index(grid: &GridDefinition, index: GridIndex) -> Result<(), MineError> {
    if index.i() >= grid.shape().nx() {
        return Err(MineError::grid(format!(
            "index `i={}` is outside grid extent `0..{}`",
            index.i(),
            grid.shape().nx()
        )));
    }

    if index.j() >= grid.shape().ny() {
        return Err(MineError::grid(format!(
            "index `j={}` is outside grid extent `0..{}`",
            index.j(),
            grid.shape().ny()
        )));
    }

    if index.k() >= grid.shape().nz() {
        return Err(MineError::grid(format!(
            "index `k={}` is outside grid extent `0..{}`",
            index.k(),
            grid.shape().nz()
        )));
    }

    Ok(())
}

fn validate_occupied_linear_indices(
    grid: &GridDefinition,
    occupied_linear_indices: Option<&[usize]>,
) -> Result<Option<BTreeSet<usize>>, MineError> {
    let Some(occupied_linear_indices) = occupied_linear_indices else {
        return Ok(None);
    };
    let total_cells = grid.shape().total_cells();
    let mut occupied = BTreeSet::new();

    for linear_index in occupied_linear_indices {
        if *linear_index >= total_cells {
            return Err(MineError::grid(format!(
                "occupied linear index `{linear_index}` is outside grid capacity `{total_cells}`"
            )));
        }

        occupied.insert(*linear_index);
    }

    Ok(Some(occupied))
}

fn offset_index(
    grid: &GridDefinition,
    index: GridIndex,
    di: isize,
    dj: isize,
    dk: isize,
) -> Option<GridIndex> {
    let i = index.i() as isize + di;
    let j = index.j() as isize + dj;
    let k = index.k() as isize + dk;

    (i >= 0
        && j >= 0
        && k >= 0
        && i < grid.shape().nx() as isize
        && j < grid.shape().ny() as isize
        && k < grid.shape().nz() as isize)
        .then_some(GridIndex::new(i as usize, j as usize, k as usize))
}

fn locate_axis(
    origin: f64,
    block_size: f64,
    axis_count: usize,
    coordinate: f64,
    tolerance: f64,
    axis_name: &'static str,
) -> Result<usize, MineError> {
    let max_coordinate = origin + (block_size * axis_count as f64);

    if coordinate < origin - tolerance || coordinate > max_coordinate + tolerance {
        return Err(MineError::grid(format!(
            "coordinate `{axis_name}={coordinate}` is outside the grid extent `{origin}..{max_coordinate}`"
        )));
    }

    if coordinate <= origin {
        return Ok(0);
    }

    if (coordinate - max_coordinate).abs() <= tolerance {
        return Ok(axis_count - 1);
    }

    let relative = coordinate - origin;
    let index = (relative / block_size).floor() as usize;

    if index >= axis_count {
        Err(MineError::grid(format!(
            "coordinate `{axis_name}={coordinate}` maps outside the grid extent"
        )))
    } else {
        Ok(index)
    }
}
