use mine_sdk::{BlockModel, ModelSummary};
use serde::{Deserialize, Serialize};

use crate::contract::{ToolDescriptor, ToolResponse};

pub(crate) const INSPECT_MODEL_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "inspect_model",
    description: "Perfila shape, extents, columnas, metadata y advertencias iniciales del modelo.",
    input_version: "1",
    output_version: "1",
};

/// Entrada para `inspect_model`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectModelInput;

/// Salida de `inspect_model`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectModelOutput {
    /// Resumen estructurado del modelo inspeccionado.
    pub summary: ModelSummary,
    /// Advertencias iniciales derivadas del perfil del modelo.
    pub warnings: Vec<String>,
}

/// Perfila un modelo y devuelve un resumen estructurado listo para serializar.
#[must_use]
pub fn inspect_model(
    model: &BlockModel,
    _input: &InspectModelInput,
) -> ToolResponse<InspectModelOutput> {
    match model.summary() {
        Ok(summary) => {
            let warnings = build_inspection_warnings(model, &summary);
            ToolResponse::success(
                INSPECT_MODEL_DESCRIPTOR,
                InspectModelOutput { summary, warnings },
            )
        }
        Err(error) => ToolResponse::failure(INSPECT_MODEL_DESCRIPTOR, error),
    }
}

fn build_inspection_warnings(model: &BlockModel, summary: &ModelSummary) -> Vec<String> {
    let mut warnings = Vec::new();

    if summary.rotation_degrees.is_some() {
        warnings.push(
            "El modelo declara una grilla rotada; algunas rutas de exportacion siguen restringidas para este caso.".to_owned(),
        );
    }

    if model.is_sparse() {
        warnings.push(
            "El modelo usa materializacion sparse experimental; parte del IO y de la validacion historica aun asume grillas densas.".to_owned(),
        );
    }

    if summary.columns.iter().any(|column| column.nullable) {
        warnings.push(
            "El schema declara columnas nullable, pero el storage columnar actual no materializa nulls explicitos.".to_owned(),
        );
    }

    warnings
}
