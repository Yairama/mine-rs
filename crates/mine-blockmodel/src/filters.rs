use std::collections::BTreeMap;

use mine_core::{ColumnId, ColumnSchemaSet, Coordinate3D, MineError};
use mine_indexing::{ijk_to_xyz, linear_to_ijk};

use crate::{BlockModel, BlockSelection, ColumnData};

impl BlockModel {
    /// Filtra bloques cuya columna flotante sea mayor o igual a un umbral.
    pub fn filter_by_float_min(
        &self,
        column_id: &ColumnId,
        minimum: f64,
    ) -> Result<BlockSelection, MineError> {
        let Some(column_data) = self.columns.get(column_id) else {
            return Err(MineError::schema(format!(
                "column `{column_id}` does not exist in block model storage"
            )));
        };

        let ColumnData::Floats(values) = column_data else {
            return Err(MineError::schema(format!(
                "column `{column_id}` is not a float column"
            )));
        };

        Ok(BlockSelection::new(
            values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (*value >= minimum).then_some(index))
                .collect(),
        ))
    }

    /// Filtra bloques cuya columna de texto coincide exactamente con un valor.
    pub fn filter_by_text_match(
        &self,
        column_id: &ColumnId,
        expected: &str,
    ) -> Result<BlockSelection, MineError> {
        let Some(column_data) = self.columns.get(column_id) else {
            return Err(MineError::schema(format!(
                "column `{column_id}` does not exist in block model storage"
            )));
        };

        let ColumnData::Texts(values) = column_data else {
            return Err(MineError::schema(format!(
                "column `{column_id}` is not a text column"
            )));
        };

        Ok(BlockSelection::new(
            values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (value == expected).then_some(index))
                .collect(),
        ))
    }

    /// Filtra bloques cuyos centros caen dentro de un rango espacial inclusivo.
    pub fn filter_by_coordinate_range(
        &self,
        minimum: Coordinate3D,
        maximum: Coordinate3D,
    ) -> Result<BlockSelection, MineError> {
        if minimum.x() > maximum.x() || minimum.y() > maximum.y() || minimum.z() > maximum.z() {
            return Err(MineError::grid(
                "coordinate range minimum must be lower than or equal to maximum on every axis",
            ));
        }

        let mut indices = Vec::new();

        for row_index in 0..self.block_count() {
            let linear_index = self.linear_index_at(row_index)?;
            let grid_index = linear_to_ijk(self.grid(), linear_index)?;
            let center = ijk_to_xyz(self.grid(), grid_index)?;

            if center.x() >= minimum.x()
                && center.x() <= maximum.x()
                && center.y() >= minimum.y()
                && center.y() <= maximum.y()
                && center.z() >= minimum.z()
                && center.z() <= maximum.z()
            {
                indices.push(row_index);
            }
        }

        Ok(BlockSelection::new(indices))
    }

    /// Selecciona un subconjunto de columnas preservando grilla, schema y metadata.
    pub fn select_columns(&self, selected_columns: &[ColumnId]) -> Result<Self, MineError> {
        let mut selected_schema = Vec::with_capacity(selected_columns.len());
        let mut selected_storage = BTreeMap::new();

        for column_id in selected_columns {
            let Some(column_schema) = self.schema.get(column_id) else {
                return Err(MineError::schema(format!(
                    "column `{column_id}` does not exist in block model schema"
                )));
            };

            let Some(column_data) = self.columns.get(column_id) else {
                return Err(MineError::schema(format!(
                    "column `{column_id}` does not exist in block model storage"
                )));
            };

            selected_schema.push(column_schema.clone());
            selected_storage.insert(column_id.clone(), column_data.clone());
        }

        Self::new_with_layout(
            self.grid.clone(),
            ColumnSchemaSet::from_columns(selected_schema)?,
            self.metadata.clone(),
            self.layout.clone(),
            selected_storage,
        )
    }
}
