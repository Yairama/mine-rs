use mine_core::ColumnLogicalType;
use serde::{Deserialize, Serialize};

/// Columna almacenada de forma columnar y tipada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColumnData {
    /// Valores enteros.
    Integers(Vec<i64>),
    /// Valores flotantes.
    Floats(Vec<f64>),
    /// Valores booleanos.
    Booleans(Vec<bool>),
    /// Valores de texto.
    Texts(Vec<String>),
}

impl ColumnData {
    /// Devuelve el número de filas almacenadas en la columna.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Integers(values) => values.len(),
            Self::Floats(values) => values.len(),
            Self::Booleans(values) => values.len(),
            Self::Texts(values) => values.len(),
        }
    }

    /// Indica si la columna no contiene filas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Devuelve el tipo lógico compatible con la columna almacenada.
    #[must_use]
    pub fn logical_type(&self) -> ColumnLogicalType {
        match self {
            Self::Integers(_) => ColumnLogicalType::Integer,
            Self::Floats(_) => ColumnLogicalType::Float,
            Self::Booleans(_) => ColumnLogicalType::Boolean,
            Self::Texts(_) => ColumnLogicalType::Text,
        }
    }
}
