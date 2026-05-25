use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{ColumnId, MineError};

/// Tipo lógico base soportado por una columna del modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnLogicalType {
    /// Entero con semántica discreta.
    Integer,
    /// Número de punto flotante.
    Float,
    /// Valor booleano.
    Boolean,
    /// Texto libre o categórico.
    Text,
}

/// Rol minero principal asociado a una columna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnMiningRole {
    /// Columna de ley.
    Grade,
    /// Columna de tonelaje.
    Tonnage,
    /// Columna de densidad.
    Density,
    /// Columna de recuperación metalúrgica o minera normalizada.
    Recovery,
    /// Columna de dominio.
    Domain,
    /// Columna de banco.
    Bench,
    /// Columna de fase.
    Phase,
    /// Columna sin rol minero especializado todavía.
    Other,
}

/// Unidad de medida explícita asociada a una columna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MeasurementUnit(String);

impl MeasurementUnit {
    /// Crea una unidad validada.
    pub fn new(value: impl Into<String>) -> Result<Self, MineError> {
        let value = value.into();

        if value.trim().is_empty() {
            return Err(MineError::schema(
                "measurement units must not be empty or whitespace only",
            ));
        }

        if value.trim() != value {
            return Err(MineError::schema(
                "measurement units must not contain leading or trailing whitespace",
            ));
        }

        if value.chars().any(char::is_control) {
            return Err(MineError::schema(
                "measurement units must not contain control characters",
            ));
        }

        Ok(Self(value))
    }

    /// Devuelve la unidad textual.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for MeasurementUnit {
    type Error = MineError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<MeasurementUnit> for String {
    fn from(value: MeasurementUnit) -> Self {
        value.0
    }
}

/// Define el schema mínimo de una columna de block model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSchema {
    name: ColumnId,
    logical_type: ColumnLogicalType,
    unit: Option<MeasurementUnit>,
    nullable: bool,
    mining_role: ColumnMiningRole,
}

impl ColumnSchema {
    /// Construye el schema de una columna.
    #[must_use]
    pub fn new(
        name: ColumnId,
        logical_type: ColumnLogicalType,
        unit: Option<MeasurementUnit>,
        nullable: bool,
        mining_role: ColumnMiningRole,
    ) -> Self {
        Self {
            name,
            logical_type,
            unit,
            nullable,
            mining_role,
        }
    }

    /// Devuelve el identificador de la columna.
    #[must_use]
    pub fn name(&self) -> &ColumnId {
        &self.name
    }

    /// Devuelve el tipo lógico de la columna.
    #[must_use]
    pub const fn logical_type(&self) -> ColumnLogicalType {
        self.logical_type
    }

    /// Devuelve la unidad opcional de la columna.
    #[must_use]
    pub fn unit(&self) -> Option<&MeasurementUnit> {
        self.unit.as_ref()
    }

    /// Indica si la columna acepta nulos.
    #[must_use]
    pub const fn nullable(&self) -> bool {
        self.nullable
    }

    /// Devuelve el rol minero principal de la columna.
    #[must_use]
    pub const fn mining_role(&self) -> ColumnMiningRole {
        self.mining_role
    }
}

/// Requisito mínimo para validar columnas obligatorias.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredColumn {
    name: ColumnId,
    logical_type: ColumnLogicalType,
}

impl RequiredColumn {
    /// Construye un requisito de columna obligatoria.
    #[must_use]
    pub fn new(name: ColumnId, logical_type: ColumnLogicalType) -> Self {
        Self { name, logical_type }
    }

    /// Devuelve el nombre de la columna requerida.
    #[must_use]
    pub fn name(&self) -> &ColumnId {
        &self.name
    }

    /// Devuelve el tipo lógico esperado.
    #[must_use]
    pub const fn logical_type(&self) -> ColumnLogicalType {
        self.logical_type
    }
}

/// Colección validada de schemas de columnas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSchemaSet(BTreeMap<ColumnId, ColumnSchema>);

impl ColumnSchemaSet {
    /// Construye un set de columnas rechazando nombres duplicados.
    pub fn from_columns(columns: Vec<ColumnSchema>) -> Result<Self, MineError> {
        let mut entries = BTreeMap::new();

        for column in columns {
            let name = column.name().clone();

            if entries.insert(name.clone(), column).is_some() {
                return Err(MineError::schema(format!(
                    "column schema `{name}` is duplicated"
                )));
            }
        }

        Ok(Self(entries))
    }

    /// Devuelve el schema de una columna si existe.
    #[must_use]
    pub fn get(&self, name: &ColumnId) -> Option<&ColumnSchema> {
        self.0.get(name)
    }

    /// Itera sobre los schemas definidos ordenados por nombre.
    pub fn iter(&self) -> impl Iterator<Item = (&ColumnId, &ColumnSchema)> {
        self.0.iter()
    }

    /// Devuelve la cantidad de columnas definidas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Indica si el set de columnas está vacío.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Valida presencia y tipo lógico de columnas requeridas.
    pub fn validate_required_columns(
        &self,
        required_columns: &[RequiredColumn],
    ) -> Result<(), MineError> {
        for required in required_columns {
            let Some(column) = self.0.get(required.name()) else {
                return Err(MineError::schema(format!(
                    "required column `{}` is missing",
                    required.name()
                )));
            };

            if column.logical_type() != required.logical_type() {
                return Err(MineError::schema(format!(
                    "column `{}` has logical type `{:?}` but `{:?}` was required",
                    required.name(),
                    column.logical_type(),
                    required.logical_type()
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_invalid_measurement_unit() {
        let error =
            MeasurementUnit::new(" t ").expect_err("units with outer whitespace should fail");

        assert_eq!(
            error,
            MineError::schema("measurement units must not contain leading or trailing whitespace")
        );
    }

    #[test]
    fn validate_required_columns() {
        let schema = ColumnSchemaSet::from_columns(vec![
            ColumnSchema::new(
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnLogicalType::Float,
                Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
                false,
                ColumnMiningRole::Grade,
            ),
            ColumnSchema::new(
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnLogicalType::Float,
                Some(MeasurementUnit::new("t").expect("unit should be valid")),
                false,
                ColumnMiningRole::Tonnage,
            ),
        ])
        .expect("schema set should be valid");

        let required = vec![
            RequiredColumn::new(
                ColumnId::new("cu").expect("column id should be valid"),
                ColumnLogicalType::Float,
            ),
            RequiredColumn::new(
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnLogicalType::Float,
            ),
        ];

        assert!(schema.validate_required_columns(&required).is_ok());
    }

    #[test]
    fn reject_missing_required_column() {
        let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Grade,
        )])
        .expect("schema set should be valid");

        let error = schema
            .validate_required_columns(&[RequiredColumn::new(
                ColumnId::new("density").expect("column id should be valid"),
                ColumnLogicalType::Float,
            )])
            .expect_err("missing column should fail");

        assert_eq!(
            error,
            MineError::schema("required column `density` is missing")
        );
    }

    #[test]
    fn reject_required_column_with_wrong_type() {
        let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
            ColumnId::new("phase").expect("column id should be valid"),
            ColumnLogicalType::Text,
            None,
            false,
            ColumnMiningRole::Phase,
        )])
        .expect("schema set should be valid");

        let error = schema
            .validate_required_columns(&[RequiredColumn::new(
                ColumnId::new("phase").expect("column id should be valid"),
                ColumnLogicalType::Integer,
            )])
            .expect_err("logical type mismatch should fail");

        assert_eq!(
            error,
            MineError::schema("column `phase` has logical type `Text` but `Integer` was required")
        );
    }
}
