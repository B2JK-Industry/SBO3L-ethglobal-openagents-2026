//! Criterion benchmark: audit-event canonical hash + signature throughput.
//!
//! Measures the per-event cost of computing `canonical_hash()` on an
//! `AuditEvent`. This is the hot path in `Storage::finalize_decision` —
//! every policy decision pays this cost before the row commits.
//!
//! Run: `cargo bench --bench audit_chain_append`

use chrono::{TimeZone, Utc};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sbo3l_core::audit::AuditEvent;

fn make_event(seq: u64) -> AuditEvent {
    AuditEvent {
        version: 1,
        seq,
        id: format!("01HCHADAUD{:016}", seq),
        ts: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        event_type: "policy_decision".to_string(),
        actor: "agent-bench".to_string(),
        subject_id: "subject-bench".to_string(),
        payload_hash: "a".repeat(64),
        metadata: serde_json::Map::new(),
        policy_version: Some(1),
        policy_hash: Some("b".repeat(64)),
        attestation_ref: None,
        prev_event_hash: "c".repeat(64),
    }
}

fn bench_canonical_hash(c: &mut Criterion) {
    let event = make_event(42);
    c.bench_function("audit_event_canonical_hash", |b| {
        b.iter(|| {
            let _ = black_box(&event).canonical_hash();
        });
    });
}

fn bench_chain_walk(c: &mut Criterion) {
    let chain: Vec<AuditEvent> = (1..=1000).map(make_event).collect();
    c.bench_function("audit_chain_canonical_walk_1000", |b| {
        b.iter(|| {
            for event in black_box(&chain) {
                let _ = event.canonical_hash();
            }
        });
    });
}

criterion_group!(benches, bench_canonical_hash, bench_chain_walk);
criterion_main!(benches);
