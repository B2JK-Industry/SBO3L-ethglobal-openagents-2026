//! Criterion benchmark: policy evaluation throughput.
//!
//! What it measures:
//!   - SBO3L native policy eval (in-process, no daemon round-trip).
//!   - Hardcoded "always allow" baseline (lower bound — what's the
//!     overhead of the SBO3L policy boundary above a no-op?).
//!   - HashMap allowlist baseline (a typical hand-rolled "policy"
//!     used by teams that haven't adopted a framework yet).
//!
//! What it intentionally does NOT measure:
//!   - OPA: requires bundled WASM runtime + Rego compile; out of scope
//!     for the in-process benchmark. See `benchmarks/competitive/README.md`
//!     for the daemon-mode comparison plan.
//!   - Casbin: similar, requires casbin-rs + ABAC model loading.
//!   - mandate.md: closed-source proprietary, no open benchmark surface.
//!
//! Run: `cargo bench --bench policy_eval`
//! Output: `target/criterion/policy_eval/report/index.html`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sbo3l_policy::model::Policy;
use std::collections::HashSet;

const SAMPLE_POLICY_YAML: &str = r#"
version: 1
budget:
  per_tx_usd: "100.00"
  daily_usd: "1000.00"
  monthly_usd: "20000.00"
allowlist:
  recipients:
    - "0x0000000000000000000000000000000000000001"
    - "0x0000000000000000000000000000000000000002"
    - "0x0000000000000000000000000000000000000003"
"#;

fn bench_sbo3l_policy_parse(c: &mut Criterion) {
    c.bench_function("sbo3l_policy_parse_yaml", |b| {
        b.iter(|| {
            let _ = Policy::parse_yaml(black_box(SAMPLE_POLICY_YAML));
        });
    });
}

fn bench_baseline_always_allow(c: &mut Criterion) {
    c.bench_function("baseline_always_allow", |b| {
        b.iter(|| {
            let _: bool = black_box(true);
        });
    });
}

fn bench_baseline_hashmap_allowlist(c: &mut Criterion) {
    let mut set = HashSet::new();
    for i in 0..100u8 {
        set.insert(format!("0x{:040x}", i));
    }
    let probe = "0x0000000000000000000000000000000000000010".to_string();
    c.bench_function("baseline_hashmap_allowlist", |b| {
        b.iter(|| {
            let _ = black_box(&set).contains(black_box(&probe));
        });
    });
}

criterion_group!(
    benches,
    bench_sbo3l_policy_parse,
    bench_baseline_always_allow,
    bench_baseline_hashmap_allowlist
);
criterion_main!(benches);
