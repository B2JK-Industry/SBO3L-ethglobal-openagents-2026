//! Criterion benchmark: APRP canonical request_hash throughput.
//!
//! Measures the cost of `request_hash(&Value)` — JCS canonicalization +
//! SHA-256. This runs on every incoming APRP at the daemon boundary;
//! daemon throughput is bounded by this operation when the policy
//! evaluation is otherwise trivial.
//!
//! Run: `cargo bench --bench request_hash`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sbo3l_core::hashing::{canonical_json, request_hash, sha256_hex};

const SAMPLE_APRP_JSON: &str = r#"{
    "agent_id": "agent-bench-001",
    "task_id": "task-bench-001",
    "intent": "purchase_api_call",
    "amount": { "value": "10.00", "currency": "USD" },
    "token": "USDC",
    "destination": {
        "type": "x402_endpoint",
        "url": "https://api.example.com/v1/data",
        "method": "GET"
    },
    "payment_protocol": "x402",
    "chain": "ethereum-sepolia",
    "provider_url": "https://sepolia.infura.io/v3/example",
    "expiry": "2026-12-31T23:59:59Z",
    "nonce": "01HCHA0BENCH00000000000000",
    "risk_class": "low"
}"#;

fn bench_request_hash(c: &mut Criterion) {
    let value: serde_json::Value =
        serde_json::from_str(SAMPLE_APRP_JSON).expect("sample APRP must parse");
    c.bench_function("aprp_request_hash", |b| {
        b.iter(|| {
            let _ = request_hash(black_box(&value));
        });
    });
}

fn bench_canonical_json(c: &mut Criterion) {
    let value: serde_json::Value =
        serde_json::from_str(SAMPLE_APRP_JSON).expect("sample APRP must parse");
    c.bench_function("aprp_canonical_json", |b| {
        b.iter(|| {
            let _ = canonical_json(black_box(&value));
        });
    });
}

fn bench_sha256_baseline(c: &mut Criterion) {
    let bytes = SAMPLE_APRP_JSON.as_bytes();
    c.bench_function("sha256_raw_bytes_baseline", |b| {
        b.iter(|| {
            let _ = sha256_hex(black_box(bytes));
        });
    });
}

criterion_group!(
    benches,
    bench_request_hash,
    bench_canonical_json,
    bench_sha256_baseline
);
criterion_main!(benches);
