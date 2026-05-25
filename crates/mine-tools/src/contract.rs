use serde::{Deserialize, Serialize};

pub(crate) const TOOL_CONTRACT_VERSION: &str = "0.1.0";

/// Descriptor estable de una tool disponible dentro de `mine-tools`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ToolDescriptor {
    /// Nombre estable de la tool.
    pub name: &'static str,
    /// Descripción resumida de su responsabilidad.
    pub description: &'static str,
    /// Versión del schema de entrada.
    pub input_version: &'static str,
    /// Versión del schema de salida.
    pub output_version: &'static str,
}

/// Referencia estructurada a un artefacto producido o consumido por una tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    /// Identificador lógico del artefacto.
    pub artifact_id: String,
    /// Tipo de artefacto referido.
    pub artifact_type: String,
    /// Descripción legible del contenido o propósito.
    pub description: String,
}

/// Metadata común incluida en toda respuesta de tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionMetadata {
    /// Nombre estable de la tool ejecutada.
    pub tool_name: String,
    /// Versión del contrato compartido de `mine-tools`.
    pub contract_version: String,
    /// Versión del schema de entrada usada por la tool.
    pub input_version: String,
    /// Versión del schema de salida usada por la tool.
    pub output_version: String,
    /// Referencias a artefactos lógicos producidos o reutilizados.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_references: Vec<ArtifactReference>,
}

impl ToolExecutionMetadata {
    pub(crate) fn new(descriptor: ToolDescriptor) -> Self {
        Self {
            tool_name: descriptor.name.to_owned(),
            contract_version: TOOL_CONTRACT_VERSION.to_owned(),
            input_version: descriptor.input_version.to_owned(),
            output_version: descriptor.output_version.to_owned(),
            artifact_references: Vec::new(),
        }
    }
}

/// Error serializable devuelto por una tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolError {
    /// Código estable del error.
    pub code: &'static str,
    /// Mensaje legible para humanos.
    pub message: String,
}

/// Envelope común para respuestas de tools deterministas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolResponse<T> {
    /// Metadata común de ejecución.
    pub metadata: ToolExecutionMetadata,
    /// Indica si la ejecución terminó sin errores.
    pub success: bool,
    /// Resultado estructurado de la tool cuando fue exitoso.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<T>,
    /// Errores estructurados cuando la tool no pudo completarse.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ToolError>,
}

impl<T> ToolResponse<T> {
    pub(crate) fn success(descriptor: ToolDescriptor, output: T) -> Self {
        Self {
            metadata: ToolExecutionMetadata::new(descriptor),
            success: true,
            output: Some(output),
            errors: Vec::new(),
        }
    }

    pub(crate) fn failure(descriptor: ToolDescriptor, error: mine_sdk::MineError) -> Self {
        Self {
            metadata: ToolExecutionMetadata::new(descriptor),
            success: false,
            output: None,
            errors: vec![tool_error_from_mine_error(error)],
        }
    }
}

pub(crate) fn tool_error_from_mine_error(error: mine_sdk::MineError) -> ToolError {
    let code = match &error {
        mine_sdk::MineError::Io { .. } => "io_error",
        mine_sdk::MineError::Schema { .. } => "schema_error",
        mine_sdk::MineError::Grid { .. } => "grid_error",
        mine_sdk::MineError::Validation { .. } => "validation_error",
        mine_sdk::MineError::Reblock { .. } => "reblock_error",
        mine_sdk::MineError::Economics { .. } => "economics_error",
        mine_sdk::MineError::Planning { .. } => "planning_error",
        mine_sdk::MineError::InvalidParameter { .. } => "invalid_parameter",
        mine_sdk::MineError::Numeric { .. } => "numeric_error",
    };

    ToolError {
        code,
        message: error.to_string(),
    }
}
