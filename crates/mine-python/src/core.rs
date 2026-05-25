use mine_sdk::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, Coordinate3D,
    GridDefinition, GridShape, MeasurementUnit,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::to_py_mine_error;

/// Coordenada tridimensional expuesta a Python.
#[pyclass(module = "miners._native", name = "Coordinate3D")]
#[derive(Debug, Clone)]
pub(crate) struct PyCoordinate3D {
    pub(crate) inner: Coordinate3D,
}

impl PyCoordinate3D {
    pub(crate) fn new_inner(inner: Coordinate3D) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCoordinate3D {
    #[new]
    fn new(x: f64, y: f64, z: f64) -> PyResult<Self> {
        Ok(Self::new_inner(
            Coordinate3D::new(x, y, z).map_err(to_py_mine_error)?,
        ))
    }

    #[getter]
    fn x(&self) -> f64 {
        self.inner.x()
    }

    #[getter]
    fn y(&self) -> f64 {
        self.inner.y()
    }

    #[getter]
    fn z(&self) -> f64 {
        self.inner.z()
    }

    fn __repr__(&self) -> String {
        format!(
            "Coordinate3D(x={:?}, y={:?}, z={:?})",
            self.inner.x(),
            self.inner.y(),
            self.inner.z()
        )
    }
}

/// Dimensiones de bloque expuestas a Python.
#[pyclass(module = "miners._native", name = "BlockDimensions")]
#[derive(Debug, Clone)]
pub(crate) struct PyBlockDimensions {
    inner: BlockDimensions,
}

impl PyBlockDimensions {
    fn new_inner(inner: BlockDimensions) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBlockDimensions {
    #[new]
    fn new(dx: f64, dy: f64, dz: f64) -> PyResult<Self> {
        Ok(Self::new_inner(
            BlockDimensions::new(dx, dy, dz).map_err(to_py_mine_error)?,
        ))
    }

    #[getter]
    fn dx(&self) -> f64 {
        self.inner.dx()
    }

    #[getter]
    fn dy(&self) -> f64 {
        self.inner.dy()
    }

    #[getter]
    fn dz(&self) -> f64 {
        self.inner.dz()
    }

    fn volume(&self) -> f64 {
        self.inner.volume()
    }

    fn __repr__(&self) -> String {
        format!(
            "BlockDimensions(dx={:?}, dy={:?}, dz={:?})",
            self.inner.dx(),
            self.inner.dy(),
            self.inner.dz()
        )
    }
}

/// Definición de grilla expuesta a Python.
#[pyclass(module = "miners._native", name = "GridDefinition")]
#[derive(Debug, Clone)]
pub(crate) struct PyGridDefinition {
    pub(crate) inner: GridDefinition,
}

impl PyGridDefinition {
    fn new_inner(inner: GridDefinition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyGridDefinition {
    #[new]
    #[pyo3(signature = (origin, block_dimensions, shape, rotation_degrees=None))]
    fn new(
        origin: &PyCoordinate3D,
        block_dimensions: &PyBlockDimensions,
        shape: (usize, usize, usize),
        rotation_degrees: Option<f64>,
    ) -> PyResult<Self> {
        let grid = GridDefinition::new(
            origin.inner,
            block_dimensions.inner,
            GridShape::new(shape.0, shape.1, shape.2).map_err(to_py_mine_error)?,
            rotation_degrees,
        )
        .map_err(to_py_mine_error)?;

        Ok(Self::new_inner(grid))
    }

    #[getter]
    fn origin(&self) -> PyCoordinate3D {
        PyCoordinate3D::new_inner(self.inner.origin())
    }

    #[getter]
    fn block_dimensions(&self) -> PyBlockDimensions {
        PyBlockDimensions::new_inner(self.inner.block_dimensions())
    }

    #[getter]
    fn shape(&self) -> (usize, usize, usize) {
        (
            self.inner.shape().nx(),
            self.inner.shape().ny(),
            self.inner.shape().nz(),
        )
    }

    #[getter]
    fn rotation_degrees(&self) -> Option<f64> {
        self.inner.rotation_degrees()
    }

    fn __repr__(&self) -> String {
        format!(
            "GridDefinition(origin={:?}, block_dimensions={:?}, shape={:?}, rotation_degrees={:?})",
            PyCoordinate3D::new_inner(self.inner.origin()).__repr__(),
            PyBlockDimensions::new_inner(self.inner.block_dimensions()).__repr__(),
            self.shape(),
            self.rotation_degrees()
        )
    }
}

/// Schema de columna expuesto a Python.
#[pyclass(module = "miners._native", name = "ColumnSchema")]
#[derive(Debug, Clone)]
pub(crate) struct PyColumnSchema {
    pub(crate) inner: ColumnSchema,
}

impl PyColumnSchema {
    pub(crate) fn new_inner(inner: ColumnSchema) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyColumnSchema {
    #[new]
    #[pyo3(signature = (name, logical_type, unit=None, nullable=false, mining_role="other"))]
    fn new(
        name: &str,
        logical_type: &str,
        unit: Option<&str>,
        nullable: bool,
        mining_role: &str,
    ) -> PyResult<Self> {
        Ok(Self::new_inner(ColumnSchema::new(
            ColumnId::new(name).map_err(to_py_mine_error)?,
            parse_logical_type(logical_type)?,
            unit.map(MeasurementUnit::new)
                .transpose()
                .map_err(to_py_mine_error)?,
            nullable,
            parse_mining_role(mining_role)?,
        )))
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }

    #[getter]
    fn logical_type(&self) -> &'static str {
        logical_type_name(self.inner.logical_type())
    }

    #[getter]
    fn unit(&self) -> Option<String> {
        self.inner.unit().map(|unit| unit.as_str().to_owned())
    }

    #[getter]
    fn nullable(&self) -> bool {
        self.inner.nullable()
    }

    #[getter]
    fn mining_role(&self) -> &'static str {
        mining_role_name(self.inner.mining_role())
    }

    fn __repr__(&self) -> String {
        format!(
            "ColumnSchema(name={:?}, logical_type={:?}, unit={:?}, nullable={:?}, mining_role={:?})",
            self.name(),
            self.logical_type(),
            self.unit(),
            self.nullable(),
            self.mining_role()
        )
    }
}

pub(crate) fn parse_logical_type(logical_type: &str) -> PyResult<ColumnLogicalType> {
    match logical_type.to_ascii_lowercase().as_str() {
        "integer" => Ok(ColumnLogicalType::Integer),
        "float" => Ok(ColumnLogicalType::Float),
        "boolean" => Ok(ColumnLogicalType::Boolean),
        "text" => Ok(ColumnLogicalType::Text),
        _ => Err(pyo3::PyErr::new::<crate::PyMineError, _>(format!(
            "unknown logical type `{logical_type}`"
        ))),
    }
}

pub(crate) fn logical_type_name(logical_type: ColumnLogicalType) -> &'static str {
    match logical_type {
        ColumnLogicalType::Integer => "integer",
        ColumnLogicalType::Float => "float",
        ColumnLogicalType::Boolean => "boolean",
        ColumnLogicalType::Text => "text",
    }
}

pub(crate) fn parse_mining_role(mining_role: &str) -> PyResult<ColumnMiningRole> {
    match mining_role.to_ascii_lowercase().as_str() {
        "grade" => Ok(ColumnMiningRole::Grade),
        "tonnage" => Ok(ColumnMiningRole::Tonnage),
        "density" => Ok(ColumnMiningRole::Density),
        "recovery" => Ok(ColumnMiningRole::Recovery),
        "domain" => Ok(ColumnMiningRole::Domain),
        "bench" => Ok(ColumnMiningRole::Bench),
        "phase" => Ok(ColumnMiningRole::Phase),
        "other" => Ok(ColumnMiningRole::Other),
        _ => Err(pyo3::PyErr::new::<crate::PyMineError, _>(format!(
            "unknown mining role `{mining_role}`"
        ))),
    }
}

pub(crate) fn mining_role_name(mining_role: ColumnMiningRole) -> &'static str {
    match mining_role {
        ColumnMiningRole::Grade => "grade",
        ColumnMiningRole::Tonnage => "tonnage",
        ColumnMiningRole::Density => "density",
        ColumnMiningRole::Recovery => "recovery",
        ColumnMiningRole::Domain => "domain",
        ColumnMiningRole::Bench => "bench",
        ColumnMiningRole::Phase => "phase",
        ColumnMiningRole::Other => "other",
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCoordinate3D>()?;
    module.add_class::<PyBlockDimensions>()?;
    module.add_class::<PyGridDefinition>()?;
    module.add_class::<PyColumnSchema>()?;
    Ok(())
}
