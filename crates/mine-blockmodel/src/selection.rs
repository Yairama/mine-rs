use serde::{Deserialize, Serialize};

/// Selección explícita de bloques resultante de filtros sobre el modelo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSelection {
    indices: Vec<usize>,
}

impl BlockSelection {
    /// Crea una selección a partir de índices lineales ya validados.
    #[must_use]
    pub fn new(indices: Vec<usize>) -> Self {
        Self { indices }
    }

    /// Devuelve los índices lineales seleccionados.
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    /// Devuelve la cantidad de bloques seleccionados.
    #[must_use]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Indica si la selección está vacía.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}
