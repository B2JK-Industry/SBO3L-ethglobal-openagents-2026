//! Criterion benchmark: capsule verification cost (cold path).
//!
//! Measures `verify_capsule(&Value)` cost. This is the operation
//! invoked on the `/proof` page WASM and by `sbo3l verify-capsule`.
//! End-to-end cost includes:
//!   - JSON parse
//!   - 6 strict checks (structural, hash linkage, signature, schema,
//!     timestamp, policy hash)
//!
//! Run: `cargo bench --bench capsule_verify`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sbo3l_core::passport::verify_capsule;

const SAMPLE_CAPSULE_JSON: &str = include_str!("../fixtures/sample_capsule.json");

fn bench_capsule_verify_cold(c: &mut Criterion) {
    let value: serde_json::Value = serde_json::from_str(SAMPLE_CAPSULE_JSON)
        .expect("sample_capsule.json must parse");
    c.bench_function("capsule_verify_cold", |b| {
        b.iter(|| {
            let _ = verify_capsule(black_box(&value));
        });
    });
}

fn bench_json_parse_capsule(c: &mut Criterion) {
    c.bench_function("capsule_json_parse_only", |b| {
        b.iter(|| {
            let _: Result<serde_json::Value, _> =
                serde_json::from_str(black_box(SAMPLE_CAPSULE_JSON));
        });
    });
}

criterion_group!(benches, bench_capsule_verify_cold, bench_json_parse_capsule);
criterion_main!(benches);
