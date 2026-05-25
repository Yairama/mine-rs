use std::mem::size_of;

use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, Coordinate3D, GridShape,
    MeasurementUnit, MineError,
};
use serde::{Deserialize, Serialize};

use crate::{BlockModel, ColumnData};

/// Resume una columna del modelo para inspección rápida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSummary {
    /// Nombre de la columna.
    pub name: ColumnId,
    /// Tipo lógico asociado.
    pub logical_type: ColumnLogicalType,
    /// Unidad opcional declarada en el schema.
    pub unit: Option<MeasurementUnit>,
    /// Indica si el schema permite nulos.
    pub nullable: bool,
    /// Rol minero principal.
    pub mining_role: ColumnMiningRole,
    /// Cantidad de nulos observados en la columna.
    pub null_count: usize,
    /// Cantidad de filas de la columna.
    pub row_count: usize,
    /// Memoria aproximada consumida por la columna.
    pub approximate_memory_bytes: usize,
}

/// Extent espacial derivado de una grilla regular.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialExtent {
    /// Esquina mínima de la grilla.
    pub minimum: Coordinate3D,
    /// Esquina máxima de la grilla.
    pub maximum: Coordinate3D,
}

/// Perfil serializable de un block model para inspección y tools futuras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSummary {
    /// Número total de bloques del modelo.
    pub block_count: usize,
    /// Cantidad de columnas almacenadas.
    pub column_count: usize,
    /// Forma de la grilla del modelo.
    pub grid_shape: GridShape,
    /// Dimensiones de bloque del modelo.
    pub block_dimensions: BlockDimensions,
    /// Extent espacial derivado de la grilla.
    pub extent: SpatialExtent,
    /// Rotación opcional de la grilla.
    pub rotation_degrees: Option<f64>,
    /// Resumen por columna.
    pub columns: Vec<ColumnSummary>,
    /// Memoria aproximada total del modelo.
    pub approximate_memory_bytes: usize,
    /// Claves de metadata global relevantes.
    pub metadata_keys: Vec<String>,
}

impl BlockModel {
    /// Construye un perfil serializable del modelo.
    pub fn summary(&self) -> Result<ModelSummary, MineError> {
        let mut columns = Vec::with_capacity(self.columns.len());
        let mut approximate_memory_bytes = 0_usize;

        for (column_id, column_schema) in self.schema.iter() {
            let Some(column_data) = self.columns.get(column_id) else {
                return Err(MineError::schema(format!(
                    "column `{column_id}` is present in schema but missing from block model storage"
                )));
            };

            let column_memory = approximate_column_memory_bytes(column_data);
            approximate_memory_bytes += column_memory;

            columns.push(ColumnSummary {
                name: column_id.clone(),
                logical_type: column_schema.logical_type(),
                unit: column_schema.unit().cloned(),
                nullable: column_schema.nullable(),
                mining_role: column_schema.mining_role(),
                null_count: 0,
                row_count: column_data.len(),
                approximate_memory_bytes: column_memory,
            });
        }

        let metadata_keys = self
            .metadata
            .iter()
            .map(|(key, _)| key.to_owned())
            .collect::<Vec<_>>();

        Ok(ModelSummary {
            block_count: self.block_count(),
            column_count: self.columns.len(),
            grid_shape: self.grid.shape(),
            block_dimensions: self.grid.block_dimensions(),
            extent: build_extent(self.grid())?,
            rotation_degrees: self.grid.rotation_degrees(),
            columns,
            approximate_memory_bytes,
            metadata_keys,
        })
    }
}

fn approximate_column_memory_bytes(column_data: &ColumnData) -> usize {
    match column_data {
        ColumnData::Integers(values) => values.len() * size_of::<i64>(),
        ColumnData::Floats(values) => values.len() * size_of::<f64>(),
        ColumnData::Booleans(values) => values.len() * size_of::<bool>(),
        ColumnData::Texts(values) => values.iter().map(String::len).sum(),
    }
}

fn build_extent(grid: &mine_core::GridDefinition) -> Result<SpatialExtent, MineError> {
    let minimum = grid.origin();
    let block_dimensions = grid.block_dimensions();
    let shape = grid.shape();

    let maximum = Coordinate3D::new(
        minimum.x() + (shape.nx() as f64 * block_dimensions.dx()),
        minimum.y() + (shape.ny() as f64 * block_dimensions.dy()),
        minimum.z() + (shape.nz() as f64 * block_dimensions.dz()),
    )?;

    Ok(SpatialExtent { minimum, maximum })
}
