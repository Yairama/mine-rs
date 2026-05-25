//! Tipos y contratos base compartidos por el workspace `mine-rs`.

mod error;
mod grid;
mod ids;
mod metadata;
mod schema;

use serde::Serialize;

pub use error::MineError;
pub use grid::{BlockDimensions, Coordinate3D, GridDefinition, GridShape};
pub use ids::{ArtifactId, BlockId, ColumnId, ModelId, ScenarioId};
pub use metadata::{Metadata, MetadataValue};
pub use schema::{
    ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet, MeasurementUnit,
    RequiredColumn,
};

/// Describe una capa pública del workspace y su responsabilidad principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LayerDescriptor {
    /// Nombre estable de la capa o crate.
    pub name: &'static str,
    /// Responsabilidad arquitectónica principal de la capa.
    pub responsibility: &'static str,
}

/// Devuelve la descripción base de la capa core compartida.
#[must_use]
pub const fn core_layer() -> LayerDescriptor {
    LayerDescriptor {
        name: "mine-core",
        responsibility: "Tipos, errores y contratos deterministas compartidos.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_core_layer() {
        let layer = core_layer();

        assert_eq!(layer.name, "mine-core");
        assert!(layer.responsibility.contains("deterministas"));
    }

    #[test]
    fn serialize_core_layer_to_json() {
        let json = serde_json::to_string(&core_layer()).expect("layer should serialize");

        assert!(json.contains("mine-core"));
    }
}
