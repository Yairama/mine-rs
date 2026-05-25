#![allow(missing_docs)]
//! Smoke benchmark para la infraestructura base del workspace.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use foundation_smoke::sample_layer_batch;

fn bench_sample_layer_batch(c: &mut Criterion) {
    c.bench_function("foundation_smoke/sample_layer_batch", |b| {
        b.iter(|| sample_layer_batch(black_box(1_024)))
    });
}

criterion_group!(benches, bench_sample_layer_batch);
criterion_main!(benches);
