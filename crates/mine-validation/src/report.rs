use mine_core::Metadata;
use serde::{Deserialize, Serialize};

/// Severidad de un issue de validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// El issue invalida el uso seguro del modelo.
    Error,
    /// El issue no invalida el modelo, pero requiere revisión.
    Warning,
    /// El issue es informativo.
    Info,
}

/// Código estable de issue de validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationIssueCode {
    /// Falta una columna requerida.
    MissingRequiredColumn,
    /// El tipo lógico de una columna no coincide con lo esperado.
    WrongLogicalType,
    /// Una columna con rol minero relevante no declara unidad.
    MissingMeasurementUnit,
    /// La grilla no es consistente con el roundtrip espacial esperado.
    GridIndexRoundtripMismatch,
    /// La validación espacial encontró una rotación aún no soportada.
    UnsupportedRotatedGrid,
    /// El modelo deja celdas de grilla sin materializar en un contexto denso.
    MissingBlocksDetected,
    /// Existen bloques duplicados en un artefacto previo a la materialización del modelo.
    DuplicateBlockDetected,
    /// El extent observado es menor al extent nominal esperado.
    IncompleteExtent,
    /// El extent observado mantiene tamaño similar pero aparece desplazado.
    DisplacedExtent,
    /// El extent observado excede el envelope nominal esperado.
    OversizedExtent,
    /// Una columna de ley contiene valores no finitos.
    NonFiniteGradeValue,
    /// Una columna de tonelaje contiene valores negativos o no finitos.
    InvalidTonnageValue,
    /// Una columna de densidad contiene valores no positivos o no finitos.
    InvalidDensityValue,
    /// Una columna de recuperación contiene valores fuera del rango 0..=1 o no finitos.
    InvalidRecoveryValue,
}

/// Issue individual dentro de un reporte de validación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Severidad del issue.
    pub severity: ValidationSeverity,
    /// Código estable del issue.
    pub code: ValidationIssueCode,
    /// Mensaje legible para humanos.
    pub message: String,
    /// Ubicación lógica afectada, típicamente una columna.
    pub location: Option<String>,
    /// Cantidad afectada cuando aplica.
    pub affected_count: Option<usize>,
    /// Recomendación accionable para corregir el problema.
    pub recommendation: Option<String>,
    /// Metadata estructurada asociada al issue.
    pub metadata: Metadata,
}

impl ValidationIssue {
    /// Construye un issue de validación.
    #[must_use]
    pub fn new(
        severity: ValidationSeverity,
        code: ValidationIssueCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            location: None,
            affected_count: None,
            recommendation: None,
            metadata: Metadata::new(),
        }
    }

    /// Agrega una ubicación lógica al issue.
    #[must_use]
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Agrega una cantidad afectada al issue.
    #[must_use]
    pub const fn with_affected_count(mut self, affected_count: usize) -> Self {
        self.affected_count = Some(affected_count);
        self
    }

    /// Agrega una recomendación accionable al issue.
    #[must_use]
    pub fn with_recommendation(mut self, recommendation: impl Into<String>) -> Self {
        self.recommendation = Some(recommendation.into());
        self
    }
}

/// Reporte estructurado de validación.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Issues acumulados por la ejecución de validadores.
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// Crea un reporte vacío.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Agrega un issue al reporte.
    pub fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// Agrega múltiples issues al reporte.
    pub fn extend<I>(&mut self, issues: I)
    where
        I: IntoIterator<Item = ValidationIssue>,
    {
        self.issues.extend(issues);
    }

    /// Indica si existe al menos un error.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == ValidationSeverity::Error)
    }

    /// Devuelve el número total de errores.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == ValidationSeverity::Error)
            .count()
    }

    /// Devuelve el número total de warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == ValidationSeverity::Warning)
            .count()
    }
}
