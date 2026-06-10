//! Suite de microbenchmarks criterion para operaciones críticas de `mine-rs` (MR-216).
//!
//! Esta crate no expone API pública: solo agrupa los benches versionados en
//! `benches/`. Ejecutar con `cargo bench -p mine-benchmarks`.
//!
//! Los microbenchmarks complementan la telemetría macro de los harnesses
//! benchmark (MR-215): aquí se miden operaciones aisladas sobre fixtures
//! sintéticos pequeños; allá se miden pipelines completos sobre instancias
//! MineLib reales.
