//! Harness base para benchmarks del workspace `mine-rs`.

use mine_core::{LayerDescriptor, core_layer};

/// Construye un lote pequeño de descriptores para benchmarks de fundación.
#[must_use]
pub fn sample_layer_batch(size: usize) -> Vec<LayerDescriptor> {
    vec![core_layer(); size]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_requested_batch_size() {
        let batch = sample_layer_batch(8);

        assert_eq!(batch.len(), 8);
        assert!(batch.iter().all(|layer| layer.name == "mine-core"));
    }
}
