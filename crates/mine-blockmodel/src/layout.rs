use mine_core::{GridDefinition, MineError};
use serde::{Deserialize, Serialize};

/// Describe cómo se materializan filas del modelo respecto de la grilla subyacente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockLayout {
    /// Todas las celdas de la grilla están materializadas en orden lineal.
    Dense,
    /// Solo una subsecuencia explícita de índices lineales está materializada.
    Sparse {
        /// Índices lineales materializados en orden estrictamente creciente.
        materialized_linear_indices: Vec<usize>,
    },
}

impl BlockLayout {
    /// Construye la layout densa por defecto.
    #[must_use]
    pub const fn dense() -> Self {
        Self::Dense
    }

    /// Construye una layout sparse validando índices lineales explícitos.
    pub fn sparse(
        grid: &GridDefinition,
        materialized_linear_indices: Vec<usize>,
    ) -> Result<Self, MineError> {
        validate_sparse_linear_indices(grid, &materialized_linear_indices)?;
        Ok(Self::Sparse {
            materialized_linear_indices,
        })
    }

    #[must_use]
    pub(crate) fn materialized_block_count(&self, grid: &GridDefinition) -> usize {
        match self {
            Self::Dense => grid.shape().total_cells(),
            Self::Sparse {
                materialized_linear_indices,
            } => materialized_linear_indices.len(),
        }
    }

    #[must_use]
    pub(crate) fn linear_index_at(&self, grid: &GridDefinition, row_index: usize) -> Option<usize> {
        match self {
            Self::Dense => (row_index < grid.shape().total_cells()).then_some(row_index),
            Self::Sparse {
                materialized_linear_indices,
            } => materialized_linear_indices.get(row_index).copied(),
        }
    }

    #[must_use]
    pub(crate) fn missing_linear_indices(&self, grid: &GridDefinition) -> Vec<usize> {
        let Self::Sparse {
            materialized_linear_indices,
        } = self
        else {
            return Vec::new();
        };
        let total_cells = grid.shape().total_cells();
        let mut missing =
            Vec::with_capacity(total_cells.saturating_sub(materialized_linear_indices.len()));
        let mut sparse_position = 0_usize;

        for linear_index in 0..total_cells {
            if sparse_position < materialized_linear_indices.len()
                && materialized_linear_indices[sparse_position] == linear_index
            {
                sparse_position += 1;
            } else {
                missing.push(linear_index);
            }
        }

        missing
    }

    /// Indica si la layout representa una materialización sparse.
    #[must_use]
    pub const fn is_sparse(&self) -> bool {
        matches!(self, Self::Sparse { .. })
    }
}

fn validate_sparse_linear_indices(
    grid: &GridDefinition,
    materialized_linear_indices: &[usize],
) -> Result<(), MineError> {
    let total_cells = grid.shape().total_cells();
    let mut previous = None;

    for linear_index in materialized_linear_indices {
        if *linear_index >= total_cells {
            return Err(MineError::grid(format!(
                "sparse materialized linear index `{linear_index}` is outside grid capacity `{total_cells}`"
            )));
        }

        if let Some(previous) = previous {
            if *linear_index == previous {
                return Err(MineError::validation(format!(
                    "sparse materialized linear index `{linear_index}` is duplicated"
                )));
            }

            if *linear_index < previous {
                return Err(MineError::validation(
                    "sparse materialized linear indices must be strictly increasing",
                ));
            }
        }

        previous = Some(*linear_index);
    }

    Ok(())
}
