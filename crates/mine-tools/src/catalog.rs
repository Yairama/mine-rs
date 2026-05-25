use mine_sdk::{LayerDescriptor, public_layers};
use serde::Serialize;

use crate::{
    analytics::{AGGREGATE_BLOCKS_DESCRIPTOR, GRADE_TONNAGE_DESCRIPTOR},
    contract::ToolDescriptor,
    inspect::INSPECT_MODEL_DESCRIPTOR,
    query::QUERY_BLOCKS_DESCRIPTOR,
    scenarios::{
        COMPARE_SCENARIOS_DESCRIPTOR, CREATE_SCENARIO_DESCRIPTOR, EVALUATE_SCENARIO_DESCRIPTOR,
    },
    validation::VALIDATE_MODEL_DESCRIPTOR,
};

const AVAILABLE_TOOLS: [ToolDescriptor; 8] = [
    INSPECT_MODEL_DESCRIPTOR,
    VALIDATE_MODEL_DESCRIPTOR,
    QUERY_BLOCKS_DESCRIPTOR,
    AGGREGATE_BLOCKS_DESCRIPTOR,
    GRADE_TONNAGE_DESCRIPTOR,
    CREATE_SCENARIO_DESCRIPTOR,
    EVALUATE_SCENARIO_DESCRIPTOR,
    COMPARE_SCENARIOS_DESCRIPTOR,
];

/// Catalogo mínimo de tools y capas expuestas durante la fundación del workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolCatalog {
    /// Descriptor de la capa `mine-tools`.
    pub tool_layer: LayerDescriptor,
    /// Capas del SDK que la tool surface ya expone.
    pub exposed_layers: Vec<LayerDescriptor>,
    /// Tools deterministas disponibles en el catálogo actual.
    pub available_tools: Vec<ToolDescriptor>,
}

/// Devuelve el catálogo inicial de tools deterministas disponibles.
#[must_use]
pub fn initial_tool_catalog() -> ToolCatalog {
    ToolCatalog {
        tool_layer: LayerDescriptor {
            name: "mine-tools",
            responsibility: "Tools deterministas que consumen el SDK Rust.",
        },
        exposed_layers: public_layers().to_vec(),
        available_tools: AVAILABLE_TOOLS.to_vec(),
    }
}
