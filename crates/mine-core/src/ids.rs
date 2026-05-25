use serde::{Deserialize, Serialize};

use crate::MineError;

fn validate_identifier(parameter: &'static str, value: String) -> Result<String, MineError> {
    if value.trim().is_empty() {
        return Err(MineError::invalid_parameter(
            parameter,
            "must not be empty or whitespace only",
        ));
    }

    if value.trim() != value {
        return Err(MineError::invalid_parameter(
            parameter,
            "must not contain leading or trailing whitespace",
        ));
    }

    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(MineError::invalid_parameter(parameter, "must not be empty"));
    };

    if !first.is_ascii_alphanumeric() {
        return Err(MineError::invalid_parameter(
            parameter,
            "must start with an ASCII alphanumeric character",
        ));
    }

    if let Some(character) = chars.find(|character| {
        !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(MineError::invalid_parameter(
            parameter,
            format!(
                "contains invalid character `{character}`; use only ASCII letters, digits, '-', '_', '.' or ':'"
            ),
        ));
    }

    Ok(value)
}

macro_rules! define_identifier {
    ($name:ident, $label:literal, $parameter:literal) => {
        #[doc = concat!("Identificador validado de ", $label, ".")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Crea un identificador de ", $label, " validado.")]
            pub fn new(value: impl Into<String>) -> Result<Self, MineError> {
                Ok(Self(validate_identifier($parameter, value.into())?))
            }

            #[doc = concat!("Devuelve el valor textual del identificador de ", $label, ".")]
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::str::FromStr for $name {
            type Err = MineError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = MineError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = MineError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

define_identifier!(ModelId, "modelo", "model_id");
define_identifier!(BlockId, "bloque", "block_id");
define_identifier!(ColumnId, "columna", "column_id");
define_identifier!(ScenarioId, "escenario", "scenario_id");
define_identifier!(ArtifactId, "artefacto", "artifact_id");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_empty_identifier() {
        let error = ModelId::new("   ").expect_err("empty identifiers should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter("model_id", "must not be empty or whitespace only")
        );
    }

    #[test]
    fn reject_invalid_identifier_character() {
        let error = ArtifactId::new("artifact/01").expect_err("slash should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "artifact_id",
                "contains invalid character `/`; use only ASCII letters, digits, '-', '_', '.' or ':'"
            )
        );
    }

    #[test]
    fn serialize_and_deserialize_identifier() {
        let scenario_id = ScenarioId::new("scenario-01").expect("scenario id should be valid");
        let json = serde_json::to_string(&scenario_id).expect("scenario id should serialize");
        let decoded: ScenarioId =
            serde_json::from_str(&json).expect("scenario id should deserialize");

        assert_eq!(json, "\"scenario-01\"");
        assert_eq!(decoded, scenario_id);
    }
}
