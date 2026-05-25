use serde::{Deserialize, Deserializer, Serialize};

use crate::MineError;

/// Coordenada espacial tridimensional validada.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Coordinate3D {
    x: f64,
    y: f64,
    z: f64,
}

impl Coordinate3D {
    /// Construye una coordenada espacial finita.
    pub fn new(x: f64, y: f64, z: f64) -> Result<Self, MineError> {
        validate_finite("x", x)?;
        validate_finite("y", y)?;
        validate_finite("z", z)?;

        Ok(Self { x, y, z })
    }

    /// Devuelve la coordenada `x`.
    #[must_use]
    pub const fn x(&self) -> f64 {
        self.x
    }

    /// Devuelve la coordenada `y`.
    #[must_use]
    pub const fn y(&self) -> f64 {
        self.y
    }

    /// Devuelve la coordenada `z`.
    #[must_use]
    pub const fn z(&self) -> f64 {
        self.z
    }

    /// Evalúa si dos coordenadas son equivalentes dentro de una tolerancia.
    pub fn is_within_tolerance(&self, other: &Self, tolerance: f64) -> Result<bool, MineError> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(MineError::numeric(
                "tolerance must be finite and greater than or equal to zero",
            ));
        }

        Ok((self.x - other.x).abs() <= tolerance
            && (self.y - other.y).abs() <= tolerance
            && (self.z - other.z).abs() <= tolerance)
    }
}

#[derive(Debug, Deserialize)]
struct Coordinate3DRaw {
    x: f64,
    y: f64,
    z: f64,
}

impl TryFrom<Coordinate3DRaw> for Coordinate3D {
    type Error = MineError;

    fn try_from(value: Coordinate3DRaw) -> Result<Self, Self::Error> {
        Self::new(value.x, value.y, value.z)
    }
}

impl<'de> Deserialize<'de> for Coordinate3D {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Coordinate3DRaw::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

/// Tamaños de bloque validados para los ejes `x`, `y` y `z`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BlockDimensions {
    dx: f64,
    dy: f64,
    dz: f64,
}

impl BlockDimensions {
    /// Construye tamaños de bloque positivos y finitos.
    pub fn new(dx: f64, dy: f64, dz: f64) -> Result<Self, MineError> {
        validate_positive_finite("dx", dx)?;
        validate_positive_finite("dy", dy)?;
        validate_positive_finite("dz", dz)?;

        Ok(Self { dx, dy, dz })
    }

    /// Devuelve el tamaño de bloque en `x`.
    #[must_use]
    pub const fn dx(&self) -> f64 {
        self.dx
    }

    /// Devuelve el tamaño de bloque en `y`.
    #[must_use]
    pub const fn dy(&self) -> f64 {
        self.dy
    }

    /// Devuelve el tamaño de bloque en `z`.
    #[must_use]
    pub const fn dz(&self) -> f64 {
        self.dz
    }

    /// Devuelve el volumen de un bloque regular.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.dx * self.dy * self.dz
    }
}

#[derive(Debug, Deserialize)]
struct BlockDimensionsRaw {
    dx: f64,
    dy: f64,
    dz: f64,
}

impl TryFrom<BlockDimensionsRaw> for BlockDimensions {
    type Error = MineError;

    fn try_from(value: BlockDimensionsRaw) -> Result<Self, Self::Error> {
        Self::new(value.dx, value.dy, value.dz)
    }
}

impl<'de> Deserialize<'de> for BlockDimensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        BlockDimensionsRaw::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

/// Cantidad de bloques por eje para una grilla regular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GridShape {
    nx: usize,
    ny: usize,
    nz: usize,
}

impl GridShape {
    /// Construye una forma de grilla con ejes mayores que cero.
    pub fn new(nx: usize, ny: usize, nz: usize) -> Result<Self, MineError> {
        validate_axis("nx", nx)?;
        validate_axis("ny", ny)?;
        validate_axis("nz", nz)?;

        if nx
            .checked_mul(ny)
            .and_then(|xy| xy.checked_mul(nz))
            .is_none()
        {
            return Err(MineError::grid(format!(
                "grid shape `{nx} x {ny} x {nz}` overflows total cell count"
            )));
        }

        Ok(Self { nx, ny, nz })
    }

    /// Devuelve el conteo de bloques en `x`.
    #[must_use]
    pub const fn nx(&self) -> usize {
        self.nx
    }

    /// Devuelve el conteo de bloques en `y`.
    #[must_use]
    pub const fn ny(&self) -> usize {
        self.ny
    }

    /// Devuelve el conteo de bloques en `z`.
    #[must_use]
    pub const fn nz(&self) -> usize {
        self.nz
    }

    /// Devuelve el número total de bloques de la grilla.
    #[must_use]
    pub fn total_cells(&self) -> usize {
        match self
            .nx
            .checked_mul(self.ny)
            .and_then(|xy| xy.checked_mul(self.nz))
        {
            Some(total_cells) => total_cells,
            None => unreachable!("grid shape was validated on construction"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GridShapeRaw {
    nx: usize,
    ny: usize,
    nz: usize,
}

impl TryFrom<GridShapeRaw> for GridShape {
    type Error = MineError;

    fn try_from(value: GridShapeRaw) -> Result<Self, Self::Error> {
        Self::new(value.nx, value.ny, value.nz)
    }
}

impl<'de> Deserialize<'de> for GridShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        GridShapeRaw::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

/// Define una grilla regular mediante origen, dimensiones, shape y rotación opcional.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridDefinition {
    origin: Coordinate3D,
    block_dimensions: BlockDimensions,
    shape: GridShape,
    rotation_degrees: Option<f64>,
}

impl GridDefinition {
    /// Construye una grilla regular validada.
    pub fn new(
        origin: Coordinate3D,
        block_dimensions: BlockDimensions,
        shape: GridShape,
        rotation_degrees: Option<f64>,
    ) -> Result<Self, MineError> {
        if let Some(rotation_degrees) = rotation_degrees {
            validate_finite("rotation_degrees", rotation_degrees)?;
        }

        Ok(Self {
            origin,
            block_dimensions,
            shape,
            rotation_degrees,
        })
    }

    /// Devuelve el origen de la grilla.
    #[must_use]
    pub const fn origin(&self) -> Coordinate3D {
        self.origin
    }

    /// Devuelve las dimensiones de bloque de la grilla.
    #[must_use]
    pub const fn block_dimensions(&self) -> BlockDimensions {
        self.block_dimensions
    }

    /// Devuelve la forma de la grilla.
    #[must_use]
    pub const fn shape(&self) -> GridShape {
        self.shape
    }

    /// Devuelve la rotación opcional de la grilla en grados.
    #[must_use]
    pub const fn rotation_degrees(&self) -> Option<f64> {
        self.rotation_degrees
    }
}

#[derive(Debug, Deserialize)]
struct GridDefinitionRaw {
    origin: Coordinate3D,
    block_dimensions: BlockDimensions,
    shape: GridShape,
    rotation_degrees: Option<f64>,
}

impl TryFrom<GridDefinitionRaw> for GridDefinition {
    type Error = MineError;

    fn try_from(value: GridDefinitionRaw) -> Result<Self, Self::Error> {
        Self::new(
            value.origin,
            value.block_dimensions,
            value.shape,
            value.rotation_degrees,
        )
    }
}

impl<'de> Deserialize<'de> for GridDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        GridDefinitionRaw::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

fn validate_finite(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(MineError::invalid_parameter(parameter, "must be finite"))
    }
}

fn validate_positive_finite(parameter: &'static str, value: f64) -> Result<(), MineError> {
    validate_finite(parameter, value)?;

    if value <= 0.0 {
        Err(MineError::invalid_parameter(
            parameter,
            "must be greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_axis(parameter: &'static str, value: usize) -> Result<(), MineError> {
    if value == 0 {
        Err(MineError::grid(format!(
            "grid axis `{parameter}` must be greater than zero"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_non_finite_coordinates() {
        let error = Coordinate3D::new(0.0, f64::NAN, 1.0).expect_err("NaN should fail");

        assert_eq!(error, MineError::invalid_parameter("y", "must be finite"));
    }

    #[test]
    fn compare_coordinates_with_tolerance() {
        let left = Coordinate3D::new(100.0, 200.0, 300.0).expect("coordinate should be valid");
        let right =
            Coordinate3D::new(100.001, 200.001, 300.001).expect("coordinate should be valid");

        assert!(
            left.is_within_tolerance(&right, 0.01)
                .expect("tolerance should be valid")
        );
        assert!(
            !left
                .is_within_tolerance(&right, 0.0001)
                .expect("tolerance should be valid")
        );
    }

    #[test]
    fn reject_invalid_block_dimensions() {
        let error =
            BlockDimensions::new(10.0, 0.0, 5.0).expect_err("zero-sized dimensions should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter("dy", "must be greater than zero")
        );
    }

    #[test]
    fn calculate_block_volume() {
        let dimensions = BlockDimensions::new(10.0, 12.0, 5.0).expect("dimensions should be valid");

        assert_eq!(dimensions.volume(), 600.0);
    }

    #[test]
    fn reject_zero_grid_axis() {
        let error = GridShape::new(10, 0, 5).expect_err("zero axis should fail");

        assert_eq!(
            error,
            MineError::grid("grid axis `ny` must be greater than zero")
        );
    }

    #[test]
    fn reject_grid_shape_overflow() {
        let error =
            GridShape::new(usize::MAX, 2, 1).expect_err("overflow should be rejected explicitly");

        assert_eq!(
            error,
            MineError::grid(format!(
                "grid shape `{usize_max} x {ny} x {nz}` overflows total cell count",
                usize_max = usize::MAX,
                ny = 2,
                nz = 1
            ))
        );
    }

    #[test]
    fn calculate_total_cells() {
        let shape = GridShape::new(10, 8, 4).expect("shape should be valid");

        assert_eq!(shape.total_cells(), 320);
    }

    #[test]
    fn serialize_and_deserialize_grid_definition() {
        let grid = GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
            BlockDimensions::new(10.0, 10.0, 5.0).expect("dimensions should be valid"),
            GridShape::new(100, 80, 20).expect("shape should be valid"),
            Some(15.0),
        )
        .expect("grid definition should be valid");

        let json = serde_json::to_string(&grid).expect("grid definition should serialize");
        let decoded: GridDefinition =
            serde_json::from_str(&json).expect("grid definition should deserialize");

        assert_eq!(decoded, grid);
    }

    #[test]
    fn reject_non_finite_rotation_during_deserialization() {
        let json = r#"{
            "origin": {"x": 0.0, "y": 0.0, "z": 0.0},
            "block_dimensions": {"dx": 10.0, "dy": 10.0, "dz": 10.0},
            "shape": {"nx": 1, "ny": 1, "nz": 1},
            "rotation_degrees": null
        }"#;

        let grid: GridDefinition =
            serde_json::from_str(json).expect("valid grid definition should deserialize");

        assert_eq!(grid.rotation_degrees(), None);
    }
}
