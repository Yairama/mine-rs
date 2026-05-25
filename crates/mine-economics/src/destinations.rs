//! Contratos explícitos para destinos mineros y sus supuestos económicos.

use std::collections::BTreeMap;

use mine_core::{ColumnId, MeasurementUnit, MineError};
use serde::{Deserialize, Serialize};

/// Identificador de un destino minero.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DestinationId(String);

impl DestinationId {
    /// Construye un identificador de destino validado.
    pub fn new(name: impl Into<String>) -> Result<Self, MineError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "destination_id",
                "destination id must not be empty",
            ));
        }
        Ok(Self(name))
    }

    /// Nombre del destino.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DestinationId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Tipo canónico de destino minero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestinationKind {
    /// Estéril: va a botadero, sin procesamiento.
    Waste,
    /// Mineral procesado por molienda/flotación/lixiviación en planta.
    Mill,
    /// Mineral en heap leach con recovery menor.
    Leach,
    /// Acopio con procesamiento diferido.
    Stockpile,
    /// Venta directa sin procesamiento adicional.
    DirectSell,
    /// Destino customizado definido por el usuario.
    Custom,
}

/// Recovery metalúrgica por metal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationRecovery {
    metal_column: ColumnId,
    recovery_fraction: f64,
}

impl DestinationRecovery {
    /// Construye una recovery validada (0.0 – 1.0).
    pub fn new(metal_column: ColumnId, recovery_fraction: f64) -> Result<Self, MineError> {
        if !recovery_fraction.is_finite() || !(0.0..=1.0).contains(&recovery_fraction) {
            return Err(MineError::invalid_parameter(
                "recovery_fraction",
                "recovery must be finite and between 0.0 and 1.0",
            ));
        }
        Ok(Self {
            metal_column,
            recovery_fraction,
        })
    }

    /// Columna de metal asociada.
    #[must_use]
    pub fn metal_column(&self) -> &ColumnId {
        &self.metal_column
    }

    /// Fracción de recuperación.
    #[must_use]
    pub const fn recovery_fraction(&self) -> f64 {
        self.recovery_fraction
    }
}

/// Payability por metal: fracción del precio de mercado efectivamente cobrada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationPayability {
    metal_column: ColumnId,
    payability_fraction: f64,
}

impl DestinationPayability {
    /// Construye una payability validada (0.0 – 1.0).
    pub fn new(metal_column: ColumnId, payability_fraction: f64) -> Result<Self, MineError> {
        if !payability_fraction.is_finite() || !(0.0..=1.0).contains(&payability_fraction) {
            return Err(MineError::invalid_parameter(
                "payability_fraction",
                "payability must be finite and between 0.0 and 1.0",
            ));
        }
        Ok(Self {
            metal_column,
            payability_fraction,
        })
    }

    /// Columna de metal asociada.
    #[must_use]
    pub fn metal_column(&self) -> &ColumnId {
        &self.metal_column
    }

    /// Fracción de payability.
    #[must_use]
    pub const fn payability_fraction(&self) -> f64 {
        self.payability_fraction
    }
}

/// Capacidad física de un destino por periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationCapacity {
    max_tonnes_per_period: Option<f64>,
    tonnage_unit: MeasurementUnit,
}

impl DestinationCapacity {
    /// Construye una capacidad de destino (ilimitada si `max_tonnes_per_period` es `None`).
    pub fn new(
        max_tonnes_per_period: Option<f64>,
        tonnage_unit: MeasurementUnit,
    ) -> Result<Self, MineError> {
        if let Some(max) = max_tonnes_per_period {
            if !max.is_finite() || max <= 0.0 {
                return Err(MineError::invalid_parameter(
                    "max_tonnes_per_period",
                    "destination capacity must be finite and positive",
                ));
            }
        }
        Ok(Self {
            max_tonnes_per_period,
            tonnage_unit,
        })
    }

    /// Capacidad máxima por periodo, si está restringida.
    #[must_use]
    pub const fn max_tonnes_per_period(&self) -> Option<f64> {
        self.max_tonnes_per_period
    }

    /// Unidad de tonelaje usada para esta capacidad.
    #[must_use]
    pub fn tonnage_unit(&self) -> &MeasurementUnit {
        &self.tonnage_unit
    }

    /// Retorna `true` si el destino no tiene restricción de capacidad.
    #[must_use]
    pub fn is_unlimited(&self) -> bool {
        self.max_tonnes_per_period.is_none()
    }
}

/// Supuestos económicos explícitos para un destino minero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationAssumptions {
    id: DestinationId,
    kind: DestinationKind,
    mining_cost_per_tonne: f64,
    processing_cost_per_tonne: f64,
    recoveries: Vec<DestinationRecovery>,
    payabilities: Vec<DestinationPayability>,
    capacity: DestinationCapacity,
    price_per_metal_unit: BTreeMap<String, f64>,
}

impl DestinationAssumptions {
    /// Construye supuestos económicos validados para un destino.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: DestinationId,
        kind: DestinationKind,
        mining_cost_per_tonne: f64,
        processing_cost_per_tonne: f64,
        recoveries: Vec<DestinationRecovery>,
        payabilities: Vec<DestinationPayability>,
        capacity: DestinationCapacity,
        price_per_metal_unit: BTreeMap<String, f64>,
    ) -> Result<Self, MineError> {
        if !mining_cost_per_tonne.is_finite() || mining_cost_per_tonne < 0.0 {
            return Err(MineError::invalid_parameter(
                "mining_cost_per_tonne",
                "mining cost per tonne must be finite and non-negative",
            ));
        }
        if !processing_cost_per_tonne.is_finite() || processing_cost_per_tonne < 0.0 {
            return Err(MineError::invalid_parameter(
                "processing_cost_per_tonne",
                "processing cost per tonne must be finite and non-negative",
            ));
        }
        for (metal_key, &price) in &price_per_metal_unit {
            if !price.is_finite() || price <= 0.0 {
                return Err(MineError::invalid_parameter(
                    "price_per_metal_unit",
                    format!("price for metal `{metal_key}` must be finite and positive"),
                ));
            }
        }

        Ok(Self {
            id,
            kind,
            mining_cost_per_tonne,
            processing_cost_per_tonne,
            recoveries,
            payabilities,
            capacity,
            price_per_metal_unit,
        })
    }

    /// Identificador del destino.
    #[must_use]
    pub fn id(&self) -> &DestinationId {
        &self.id
    }

    /// Tipo de destino.
    #[must_use]
    pub const fn kind(&self) -> DestinationKind {
        self.kind
    }

    /// Costo de minado por tonelada.
    #[must_use]
    pub const fn mining_cost_per_tonne(&self) -> f64 {
        self.mining_cost_per_tonne
    }

    /// Costo de procesamiento por tonelada.
    #[must_use]
    pub const fn processing_cost_per_tonne(&self) -> f64 {
        self.processing_cost_per_tonne
    }

    /// Recoveries por metal.
    #[must_use]
    pub fn recoveries(&self) -> &[DestinationRecovery] {
        &self.recoveries
    }

    /// Payabilities por metal.
    #[must_use]
    pub fn payabilities(&self) -> &[DestinationPayability] {
        &self.payabilities
    }

    /// Capacidad del destino.
    #[must_use]
    pub fn capacity(&self) -> &DestinationCapacity {
        &self.capacity
    }

    /// Precios por metal.
    #[must_use]
    pub fn price_per_metal_unit(&self) -> &BTreeMap<String, f64> {
        &self.price_per_metal_unit
    }

    /// Costo total por tonelada (mining + processing).
    #[must_use]
    pub fn total_cost_per_tonne(&self) -> f64 {
        self.mining_cost_per_tonne + self.processing_cost_per_tonne
    }
}

/// Conjunto indexado de supuestos de destinos reutilizable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationAssumptionSet {
    destinations: Vec<DestinationAssumptions>,
}

impl DestinationAssumptionSet {
    /// Construye un conjunto a partir de una lista de destinos validando que no existan IDs duplicados.
    pub fn new(destinations: Vec<DestinationAssumptions>) -> Result<Self, MineError> {
        let mut seen_ids = BTreeMap::<String, usize>::new();
        for (index, destination) in destinations.iter().enumerate() {
            let id_str = destination.id().as_str().to_owned();
            if let Some(previous) = seen_ids.get(&id_str) {
                return Err(MineError::validation(format!(
                    "duplicate destination id `{id_str}` at positions {previous} and {index}"
                )));
            }
            seen_ids.insert(id_str, index);
        }

        Ok(Self { destinations })
    }

    /// Retorna todos los destinos del conjunto.
    #[must_use]
    pub fn destinations(&self) -> &[DestinationAssumptions] {
        &self.destinations
    }

    /// Busca un destino por ID.
    #[must_use]
    pub fn get(&self, id: &DestinationId) -> Option<&DestinationAssumptions> {
        self.destinations
            .iter()
            .find(|destination| destination.id() == id)
    }

    /// Cantidad de destinos.
    #[must_use]
    pub fn len(&self) -> usize {
        self.destinations.len()
    }

    /// Retorna `true` si el conjunto no tiene destinos.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.destinations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use mine_core::{ColumnId, MeasurementUnit};

    use super::*;

    fn mill_destination() -> DestinationAssumptions {
        DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            2.0,
            8.0,
            vec![
                DestinationRecovery::new(
                    ColumnId::new("cu").expect("column id should be valid"),
                    0.88,
                )
                .expect("recovery should be valid"),
            ],
            vec![
                DestinationPayability::new(
                    ColumnId::new("cu").expect("column id should be valid"),
                    0.97,
                )
                .expect("payability should be valid"),
            ],
            DestinationCapacity::new(
                Some(5_000_000.0),
                MeasurementUnit::new("t").expect("t is valid"),
            )
            .expect("capacity should be valid"),
            std::collections::BTreeMap::from([("cu".to_owned(), 9000.0)]),
        )
        .expect("destination should be valid")
    }

    fn waste_destination() -> DestinationAssumptions {
        DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            std::collections::BTreeMap::new(),
        )
        .expect("destination should be valid")
    }

    #[test]
    fn destination_id_rejects_empty_name() {
        assert!(DestinationId::new("").is_err());
        assert!(DestinationId::new("   ").is_err());
    }

    #[test]
    fn destination_recovery_rejects_out_of_range() {
        let cu = ColumnId::new("cu").expect("column id should be valid");
        assert!(DestinationRecovery::new(cu.clone(), -0.1).is_err());
        assert!(DestinationRecovery::new(cu.clone(), 1.01).is_err());
        assert!(DestinationRecovery::new(cu, f64::NAN).is_err());
    }

    #[test]
    fn destination_capacity_unlimited_when_none() {
        let cap = DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
            .expect("capacity should be valid");
        assert!(cap.is_unlimited());
        assert!(cap.max_tonnes_per_period().is_none());
    }

    #[test]
    fn destination_assumptions_validates_negative_costs() {
        let result = DestinationAssumptions::new(
            DestinationId::new("bad").expect("id should be valid"),
            DestinationKind::Mill,
            -1.0,
            8.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            std::collections::BTreeMap::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn destination_assumption_set_rejects_duplicate_ids() {
        let mill1 = mill_destination();
        let mill2 = mill_destination();
        let result = DestinationAssumptionSet::new(vec![mill1, mill2]);
        assert!(result.is_err());
    }

    #[test]
    fn destination_assumption_set_retrieves_by_id() {
        let mill = mill_destination();
        let waste = waste_destination();
        let mill_id = DestinationId::new("mill").expect("id should be valid");

        let set = DestinationAssumptionSet::new(vec![mill, waste]).expect("set should be valid");

        let found = set.get(&mill_id).expect("mill should be found");
        assert_eq!(found.kind(), DestinationKind::Mill);
        assert_eq!(found.mining_cost_per_tonne(), 2.0);
        assert_eq!(found.processing_cost_per_tonne(), 8.0);
        assert_eq!(found.total_cost_per_tonne(), 10.0);
    }

    #[test]
    fn destination_serializes_to_json() {
        let set = DestinationAssumptionSet::new(vec![mill_destination(), waste_destination()])
            .expect("set should be valid");
        let json = serde_json::to_string(&set).expect("serialization should succeed");
        let deserialized: DestinationAssumptionSet =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(set, deserialized);
    }
}
