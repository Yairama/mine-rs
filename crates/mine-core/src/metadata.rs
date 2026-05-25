use std::collections::BTreeMap;
use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::MineError;

/// Valor simple y serializable almacenado en metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetadataValue {
    /// Valor textual.
    Text(String),
    /// Valor booleano.
    Boolean(bool),
    /// Valor entero.
    Integer(i64),
    /// Valor flotante.
    Float(f64),
}

/// Metadata determinista preservable en IO y contratos del SDK.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Metadata(BTreeMap<String, MetadataValue>);

impl Metadata {
    /// Crea metadata vacía.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construye metadata validando claves inválidas y duplicadas.
    pub fn from_entries<I>(entries: I) -> Result<Self, MineError>
    where
        I: IntoIterator<Item = (String, MetadataValue)>,
    {
        let mut metadata = Self::new();

        for (key, value) in entries {
            metadata.insert(key, value)?;
        }

        Ok(metadata)
    }

    /// Inserta una entrada nueva; falla si la clave ya existe.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: MetadataValue,
    ) -> Result<(), MineError> {
        let key = validate_metadata_key(key.into())?;

        if self.0.contains_key(&key) {
            return Err(MineError::schema(format!(
                "metadata key `{key}` is duplicated"
            )));
        }

        self.0.insert(key, value);
        Ok(())
    }

    /// Devuelve el valor asociado a una clave.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&MetadataValue> {
        self.0.get(key)
    }

    /// Devuelve la cantidad de entradas almacenadas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Indica si la metadata está vacía.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Itera sobre las entradas ordenadas por clave.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &MetadataValue)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }
}

impl<'de> Deserialize<'de> for Metadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MetadataVisitor;

        impl<'de> Visitor<'de> for MetadataVisitor {
            type Value = Metadata;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a metadata object with unique and valid keys")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut entries = BTreeMap::new();

                while let Some((key, value)) = map.next_entry::<String, MetadataValue>()? {
                    let key = validate_metadata_key(key).map_err(serde::de::Error::custom)?;

                    if entries.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "metadata key `{key}` is duplicated"
                        )));
                    }
                }

                Ok(Metadata(entries))
            }
        }

        deserializer.deserialize_map(MetadataVisitor)
    }
}

fn validate_metadata_key(key: String) -> Result<String, MineError> {
    if key.trim().is_empty() {
        return Err(MineError::schema(
            "metadata keys must not be empty or whitespace only",
        ));
    }

    if key.trim() != key {
        return Err(MineError::schema(
            "metadata keys must not contain leading or trailing whitespace",
        ));
    }

    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err(MineError::schema("metadata keys must not be empty"));
    };

    if !first.is_ascii_alphanumeric() {
        return Err(MineError::schema(
            "metadata keys must start with an ASCII alphanumeric character",
        ));
    }

    if let Some(character) = chars.find(|character| {
        !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(MineError::schema(format!(
            "metadata key contains invalid character `{character}`"
        )));
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_metadata_json() {
        let metadata = Metadata::from_entries(vec![
            (
                "author".to_owned(),
                MetadataValue::Text("mine-rs".to_owned()),
            ),
            ("version".to_owned(), MetadataValue::Integer(1)),
        ])
        .expect("metadata should be valid");

        let json = serde_json::to_string(&metadata).expect("metadata should serialize");
        let decoded: Metadata = serde_json::from_str(&json).expect("metadata should deserialize");

        assert_eq!(decoded, metadata);
    }

    #[test]
    fn reject_invalid_metadata_key() {
        let error = Metadata::from_entries(vec![(
            " mine".to_owned(),
            MetadataValue::Text("invalid".to_owned()),
        )])
        .expect_err("metadata key should be rejected");

        assert_eq!(
            error,
            MineError::schema("metadata keys must not contain leading or trailing whitespace")
        );
    }

    #[test]
    fn reject_duplicate_metadata_key() {
        let error = Metadata::from_entries(vec![
            ("mine".to_owned(), MetadataValue::Text("a".to_owned())),
            ("mine".to_owned(), MetadataValue::Text("b".to_owned())),
        ])
        .expect_err("duplicate metadata key should fail");

        assert_eq!(
            error,
            MineError::schema("metadata key `mine` is duplicated")
        );
    }

    #[test]
    fn reject_duplicate_metadata_key_during_deserialization() {
        let error = serde_json::from_str::<Metadata>(r#"{"mine":"a","mine":"b"}"#)
            .expect_err("duplicate JSON key should fail");

        assert!(
            error
                .to_string()
                .contains("metadata key `mine` is duplicated")
        );
    }
}
