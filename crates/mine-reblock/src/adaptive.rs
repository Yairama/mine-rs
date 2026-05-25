use std::collections::{BTreeMap, BTreeSet};

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

/// Estrategia experimental de resolución por zona para adaptive reblocking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdaptiveResolutionStrategy {
    /// Conserva la resolución actual.
    Preserve,
    /// Recomienda un superblocking con factores enteros por eje.
    Superblock {
        /// Factores enteros `(x, y, z)` sugeridos para reagregación.
        factors: (usize, usize, usize),
    },
    /// Recomienda un subblocking con factores enteros por eje.
    Subblock {
        /// Factores enteros `(x, y, z)` sugeridos para subdivisión.
        factors: (usize, usize, usize),
    },
}

/// Regla explícita que asigna una estrategia a un valor de zona.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveZoneRule {
    zone_value: String,
    strategy: AdaptiveResolutionStrategy,
}

impl AdaptiveZoneRule {
    /// Construye una regla adaptativa para un valor de zona.
    pub fn new(
        zone_value: impl Into<String>,
        strategy: AdaptiveResolutionStrategy,
    ) -> Result<Self, MineError> {
        let zone_value = zone_value.into().trim().to_owned();
        if zone_value.is_empty() {
            return Err(MineError::invalid_parameter(
                "zone_value",
                "adaptive reblock zone value must not be empty",
            ));
        }

        Ok(Self {
            zone_value,
            strategy,
        })
    }

    /// Valor de zona al que aplica la regla.
    #[must_use]
    pub fn zone_value(&self) -> &str {
        &self.zone_value
    }

    /// Estrategia propuesta para la zona.
    #[must_use]
    pub const fn strategy(&self) -> &AdaptiveResolutionStrategy {
        &self.strategy
    }
}

/// Resumen serializable por zona dentro del prototipo adaptativo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveZonePrototype {
    /// Valor de zona evaluado.
    pub zone_value: String,
    /// Estrategia propuesta para la zona.
    pub strategy: AdaptiveResolutionStrategy,
    /// Bloques materializados observados en la zona.
    pub block_count: usize,
    /// Tonelaje total observado cuando se provee columna de tonelaje.
    pub total_tonnage: Option<f64>,
}

/// Prototipo serializable para planificar reblocking variable por zonas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveReblockPrototype {
    /// Columna categórica usada para segmentar zonas.
    pub zone_column: ColumnId,
    /// Columna de tonelaje usada para resumir masa, cuando aplica.
    pub tonnage_column: Option<ColumnId>,
    /// Estrategias propuestas por zona.
    pub zones: Vec<AdaptiveZonePrototype>,
    /// Limitaciones conocidas del prototipo.
    pub limitations: Vec<String>,
    /// Siguientes pasos sugeridos.
    pub next_steps: Vec<String>,
}

/// Construye un prototipo adaptativo agrupando bloques materializados por una zona categórica.
pub fn build_adaptive_reblock_prototype(
    model: &BlockModel,
    zone_column: &ColumnId,
    tonnage_column: Option<&ColumnId>,
    rules: &[AdaptiveZoneRule],
) -> Result<AdaptiveReblockPrototype, MineError> {
    if rules.is_empty() {
        return Err(MineError::invalid_parameter(
            "rules",
            "adaptive reblock prototype requires at least one zone rule",
        ));
    }

    let zone_data = model.column(zone_column).ok_or_else(|| {
        MineError::schema(format!(
            "adaptive reblock zone column `{zone_column}` does not exist in block model storage"
        ))
    })?;
    let tonnage_values = tonnage_column
        .map(|column| {
            let data = model.column(column).ok_or_else(|| {
                MineError::schema(format!(
                    "adaptive reblock tonnage column `{column}` does not exist in block model storage"
                ))
            })?;

            match data {
                ColumnData::Floats(values) => Ok(values),
                _ => Err(MineError::invalid_parameter(
                    "columns",
                    format!("adaptive reblock requires float tonnage column `{column}`"),
                )),
            }
        })
        .transpose()?;

    let mut counts_by_zone = BTreeMap::<String, (usize, Option<f64>)>::new();
    for row_index in 0..model.block_count() {
        let zone_value = zone_value_at(zone_data, row_index, zone_column)?;
        let entry = counts_by_zone
            .entry(zone_value)
            .or_insert((0, tonnage_values.map(|_| 0.0)));
        entry.0 += 1;

        if let Some(values) = tonnage_values {
            let tonnage = *values.get(row_index).ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{row_index}` is outside tonnage column `{}`",
                    tonnage_column.expect("tonnage column should exist")
                ))
            })?;
            if !tonnage.is_finite() {
                return Err(MineError::numeric(
                    "adaptive reblock requires finite tonnage values",
                ));
            }
            entry.1 = Some(entry.1.unwrap_or(0.0) + tonnage);
        }
    }

    let mut seen_zone_rules = BTreeSet::new();
    let mut zones = Vec::with_capacity(rules.len());
    for rule in rules {
        if !seen_zone_rules.insert(rule.zone_value().to_owned()) {
            return Err(MineError::invalid_parameter(
                "rules",
                format!(
                    "adaptive reblock zone rule `{}` is duplicated",
                    rule.zone_value()
                ),
            ));
        }

        let Some((block_count, total_tonnage)) = counts_by_zone.get(rule.zone_value()) else {
            return Err(MineError::invalid_parameter(
                "rules",
                format!(
                    "adaptive reblock zone rule `{}` does not match any materialized block",
                    rule.zone_value()
                ),
            ));
        };

        zones.push(AdaptiveZonePrototype {
            zone_value: rule.zone_value().to_owned(),
            strategy: rule.strategy().clone(),
            block_count: *block_count,
            total_tonnage: *total_tonnage,
        });
    }

    Ok(AdaptiveReblockPrototype {
        zone_column: zone_column.clone(),
        tonnage_column: tonnage_column.cloned(),
        zones,
        limitations: vec![
            "Prototype plans strategies by explicit zone rules only; it does not execute mixed-resolution reblocking yet.".to_owned(),
            "Zones must already exist in a categorical column; density heuristics and geometric clustering are out of scope in this iteration.".to_owned(),
            "Conflicts between neighboring strategies, transition buffers and reconciliation across mixed resolutions are not resolved automatically.".to_owned(),
        ],
        next_steps: vec![
            "Add deterministic execution that partitions the model by zone and composes superblocking/subblocking outputs.".to_owned(),
            "Introduce explicit transition rules between adjacent zones with different target resolutions.".to_owned(),
            "Evaluate density-driven strategies in addition to categorical domains once reconciliation thresholds are available.".to_owned(),
        ],
    })
}

fn zone_value_at(
    zone_data: &ColumnData,
    row_index: usize,
    zone_column: &ColumnId,
) -> Result<String, MineError> {
    match zone_data {
        ColumnData::Texts(values) => values.get(row_index).cloned().ok_or_else(|| {
            MineError::validation(format!(
                "row index `{row_index}` is outside zone column `{zone_column}`"
            ))
        }),
        ColumnData::Integers(values) => {
            values
                .get(row_index)
                .map(ToString::to_string)
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "row index `{row_index}` is outside zone column `{zone_column}`"
                    ))
                })
        }
        ColumnData::Booleans(values) => {
            values
                .get(row_index)
                .map(ToString::to_string)
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "row index `{row_index}` is outside zone column `{zone_column}`"
                    ))
                })
        }
        ColumnData::Floats(_) => Err(MineError::invalid_parameter(
            "columns",
            format!("adaptive reblock requires categorical zone column `{zone_column}`"),
        )),
    }
}
