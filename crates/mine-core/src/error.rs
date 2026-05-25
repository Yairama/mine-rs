use thiserror::Error;

/// Error base del dominio compartido por `mine-core`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MineError {
    /// Error de entrada y salida.
    #[error("error de IO: {message}")]
    Io {
        /// Detalle del error de IO recibido.
        message: String,
    },
    /// Error de schema o contrato.
    #[error("error de schema: {message}")]
    Schema {
        /// Detalle del error de schema recibido.
        message: String,
    },
    /// Error de grilla o geometría espacial.
    #[error("error de grilla: {message}")]
    Grid {
        /// Detalle del error de grilla recibido.
        message: String,
    },
    /// Error de validación de dominio.
    #[error("error de validacion: {message}")]
    Validation {
        /// Detalle del error de validación recibido.
        message: String,
    },
    /// Error de reblocking o agregación espacial.
    #[error("error de reblocking: {message}")]
    Reblock {
        /// Detalle del error de reblocking recibido.
        message: String,
    },
    /// Error de evaluación económica.
    #[error("error de economia: {message}")]
    Economics {
        /// Detalle del error económico recibido.
        message: String,
    },
    /// Error de planeamiento.
    #[error("error de planeamiento: {message}")]
    Planning {
        /// Detalle del error de planeamiento recibido.
        message: String,
    },
    /// Error por parámetro inválido.
    #[error("parametro invalido `{parameter}`: {message}")]
    InvalidParameter {
        /// Nombre del parámetro inválido.
        parameter: &'static str,
        /// Mensaje detallado del problema.
        message: String,
    },
    /// Error numérico o de tolerancia.
    #[error("error numerico: {message}")]
    Numeric {
        /// Detalle del error numérico recibido.
        message: String,
    },
}

impl MineError {
    /// Construye un error de schema.
    #[must_use]
    pub fn schema(message: impl Into<String>) -> Self {
        Self::Schema {
            message: message.into(),
        }
    }

    /// Construye un error de grilla.
    #[must_use]
    pub fn grid(message: impl Into<String>) -> Self {
        Self::Grid {
            message: message.into(),
        }
    }

    /// Construye un error de validación.
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Construye un error por parámetro inválido.
    #[must_use]
    pub fn invalid_parameter(parameter: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidParameter {
            parameter,
            message: message.into(),
        }
    }

    /// Construye un error numérico.
    #[must_use]
    pub fn numeric(message: impl Into<String>) -> Self {
        Self::Numeric {
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for MineError {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_invalid_parameter_error() {
        let error = MineError::invalid_parameter("dx", "must be positive");

        assert_eq!(
            error.to_string(),
            "parametro invalido `dx`: must be positive"
        );
    }

    #[test]
    fn convert_io_error() {
        let error = MineError::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file missing",
        ));

        assert_eq!(
            error,
            MineError::Io {
                message: "file missing".to_owned(),
            }
        );
    }
}
