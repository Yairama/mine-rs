use std::collections::BTreeMap;

use mine_core::{ColumnId, ColumnSchemaSet, GridDefinition, Metadata, MineError};

use crate::{BlockLayout, ColumnData};

/// Representa un block model regular pequeño en memoria.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockModel {
    pub(crate) grid: GridDefinition,
    pub(crate) schema: ColumnSchemaSet,
    pub(crate) metadata: Metadata,
    pub(crate) layout: BlockLayout,
    pub(crate) columns: BTreeMap<ColumnId, ColumnData>,
}

impl BlockModel {
    /// Construye un block model validando schema, tipos y tamaños de columnas.
    pub fn new(
        grid: GridDefinition,
        schema: ColumnSchemaSet,
        metadata: Metadata,
        columns: BTreeMap<ColumnId, ColumnData>,
    ) -> Result<Self, MineError> {
        Self::new_with_layout(grid, schema, metadata, BlockLayout::dense(), columns)
    }

    /// Construye un block model sparse validando índices lineales materializados.
    pub fn new_sparse(
        grid: GridDefinition,
        schema: ColumnSchemaSet,
        metadata: Metadata,
        materialized_linear_indices: Vec<usize>,
        columns: BTreeMap<ColumnId, ColumnData>,
    ) -> Result<Self, MineError> {
        let layout = BlockLayout::sparse(&grid, materialized_linear_indices)?;
        Self::new_with_layout(grid, schema, metadata, layout, columns)
    }

    pub(crate) fn new_with_layout(
        grid: GridDefinition,
        schema: ColumnSchemaSet,
        metadata: Metadata,
        layout: BlockLayout,
        columns: BTreeMap<ColumnId, ColumnData>,
    ) -> Result<Self, MineError> {
        let expected_row_count = layout.materialized_block_count(&grid);

        for (column_id, column_schema) in schema.iter() {
            let Some(column_data) = columns.get(column_id) else {
                return Err(MineError::schema(format!(
                    "column `{column_id}` is declared in schema but missing from block model storage"
                )));
            };

            if column_data.logical_type() != column_schema.logical_type() {
                return Err(MineError::schema(format!(
                    "column `{column_id}` has logical type `{:?}` but schema expects `{:?}`",
                    column_data.logical_type(),
                    column_schema.logical_type()
                )));
            }

            if column_data.len() != expected_row_count {
                return Err(MineError::validation(format!(
                    "column `{column_id}` has {} rows but grid expects {expected_row_count}",
                    column_data.len()
                )));
            }
        }

        for column_id in columns.keys() {
            if schema.get(column_id).is_none() {
                return Err(MineError::schema(format!(
                    "column `{column_id}` is present in block model storage but missing from schema"
                )));
            }
        }

        Ok(Self {
            grid,
            schema,
            metadata,
            layout,
            columns,
        })
    }

    /// Devuelve la definición espacial de la grilla.
    #[must_use]
    pub const fn grid(&self) -> &GridDefinition {
        &self.grid
    }

    /// Devuelve el schema de columnas del modelo.
    #[must_use]
    pub const fn schema(&self) -> &ColumnSchemaSet {
        &self.schema
    }

    /// Devuelve la metadata global del modelo.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Devuelve el número total de bloques materializados.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.layout.materialized_block_count(&self.grid)
    }

    /// Devuelve la capacidad total de celdas de la grilla subyacente.
    #[must_use]
    pub fn grid_cell_count(&self) -> usize {
        self.grid.shape().total_cells()
    }

    /// Indica si el modelo materializa solo una parte de la grilla.
    #[must_use]
    pub const fn is_sparse(&self) -> bool {
        self.layout.is_sparse()
    }

    /// Devuelve una columna por nombre si existe.
    #[must_use]
    pub fn column(&self, column_id: &ColumnId) -> Option<&ColumnData> {
        self.columns.get(column_id)
    }

    /// Devuelve el índice lineal de la fila materializada indicada.
    pub fn linear_index_at(&self, row_index: usize) -> Result<usize, MineError> {
        self.layout
            .linear_index_at(self.grid(), row_index)
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{row_index}` is outside block model materialization"
                ))
            })
    }

    /// Devuelve los índices lineales faltantes respecto de la grilla base.
    #[must_use]
    pub fn missing_linear_indices(&self) -> Vec<usize> {
        self.layout.missing_linear_indices(self.grid())
    }
}
