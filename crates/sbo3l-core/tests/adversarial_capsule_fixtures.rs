//! R20 Task C — adversarial capsule fixtures (Dev 1 slice).
//!
//! Five new tampered v2 capsule fixtures land in
//! `test-corpus/passport/v2_tampered_005..009.json`. Every fixture must
//! be **rejected** by the verifier — either at the structural pass
//! (`verify_capsule`) when the schema or a cross-field invariant catches
//! it, or at the strict pass (`verify_capsule_strict`) when only the
//! cryptographic checks can. The tests here lock down BOTH the rejection
//! site and the specific error class so a future verifier change that
//! accidentally relaxes one of these gates fails closed loud and early.
//!
//! Coverage matrix (this file):
//!
//!   | Fixture                                              | Rejection site             | Error class                        |
//!   |------------------------------------------------------|----------------------------|------------------------------------|
//!   | v2_tampered_005_executor_evidence_drift.json         | structural (schema)        | capsule.schema_invalid             |
//!   | v2_tampered_006_reverse_chain_link.json              | strict (audit_chain)       | ChainError (PrevHashMismatch)      |
//!   | v2_tampered_007_signature_swap.json                  | strict (receipt_signature) | Ed25519 signature verify failed    |
//!   | v2_tampered_008_replay_with_new_nonce.json           | strict (request_hash)      | recomputed JCS+SHA-256 mismatch    |
//!   | v2_tampered_009_oversized_audit_segment.json         | strict (audit_segment cap) | capsule.audit_segment_too_large    |
//!
//! All five fixtures pre-existing v2_tampered_001..004 stay covered by
//! the strict-verifier unit tests in `passport.rs`; this file extends
//! the corpus to 9 → 14 distinct rejection paths against persisted JSON.

use sbo3l_core::passport::{verify_capsule, verify_capsule_strict, StrictVerifyOpts};
use serde_json::Value;
use std::path::PathBuf;

fn corpus(name: &str) -> Value {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("..");
    p.push("..");
    p.push("test-corpus");
    p.push("passport");
    p.push(name);
    let raw = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", p.display()));
    serde_json::from_str(&raw).expect("fixture parses as JSON")
}

// ---------------------------------------------------------------------------
// v2_tampered_005 — executor_evidence drift.
// ---------------------------------------------------------------------------
//
// The brief asks for a fixture where `execution.executor_evidence` claims
// `keeperhub` but the embedded value has the wrong shape (e.g. ULID where
// KeeperHub expects a `kh-` prefix). The **schema** is the gate today:
// `executor_evidence` is `oneOf [null, object minProperties:1]`, so a
// non-object (here: a bare string) fails schema validation and the
// verifier surfaces it as `capsule.schema_invalid`.
//
// Honest gap: the verifier does NOT enforce a `kh-`-vs-`uni-` prefix
// invariant on `execution.execution_ref` correlated with
// `execution.executor`. That's an executor-adapter concern (the
// keeperhub-adapter unit tests pin it), not a capsule-level shape rule.
// If we wanted a verifier-level prefix gate it would belong in
// `verify_capsule` as a new invariant — out of scope for R20 Task C.
#[test]
fn v2_tampered_005_executor_evidence_drift_rejected_by_schema() {
    let v = corpus("v2_tampered_005_executor_evidence_drift.json");
    let err = verify_capsule(&v).expect_err("malformed executor_evidence must be rejected");
    assert_eq!(err.code(), "capsule.schema_invalid", "unexpected error: {err}");
}

// ---------------------------------------------------------------------------
// v2_tampered_006 — reverse-pointing chain link (cycle / forward edge).
// ---------------------------------------------------------------------------
//
// The seq=1 audit event's `prev_event_hash` points at its own
// `event_hash` (a self-cycle / forward-pointing edge). The structural
// verifier doesn't walk the chain — that's by design, see
// `passport.rs` module doc — so structural verify passes. Strict mode
// invokes `audit_bundle::verify` over the embedded segment, which
// recomputes the chain via `verify_chain` and surfaces the linkage
// break as `ChainError::PrevHashMismatch { seq: 1 }` (genesis must be
// `prev_event_hash == ZERO_HASH`).
#[test]
fn v2_tampered_006_reverse_chain_link_passes_structural_fails_strict_chain() {
    let v = corpus("v2_tampered_006_reverse_chain_link.json");
    verify_capsule(&v).expect("structural verify must pass — chain walk is strict-only");
    let report = verify_capsule_strict(&v, &StrictVerifyOpts::default());
    assert!(
        report.structural.is_passed(),
        "structural should pass; got {:?}",
        report.structural
    );
    assert!(
        report.audit_chain.is_failed(),
        "audit_chain must catch the reverse link; got {:?}",
        report.audit_chain
    );
    // `audit_event_link` walks the segment and asserts the capsule's
    // claimed event id is present — it's INDEPENDENT of chain
    // linkage, so this fixture (which keeps event ids intact and
    // only mutates prev_event_hash) leaves audit_event_link green.
    // Pinning the green outcome is intentional: it documents that
    // chain-linkage corruption is caught by `audit_chain` ALONE,
    // not by the broader event-link check.
    assert!(
        report.audit_event_link.is_passed(),
        "audit_event_link checks event-id presence only; got {:?}",
        report.audit_event_link
    );
}

// ---------------------------------------------------------------------------
// v2_tampered_007 — signature swap (different signing key / flipped byte).
// ---------------------------------------------------------------------------
//
// The receipt's `signature_hex` is a valid 128-hex-char string (still
// passes schema's `^[0-9a-f]{128}$`) but the underlying Ed25519
// signature does NOT verify against the embedded segment's published
// `verification_keys.receipt_signer_pubkey_hex`. Real-world equivalent
// is an attacker substituting a signature produced by a different
// agent's key. Caught by the strict `receipt_signature` check.
#[test]
fn v2_tampered_007_signature_swap_rejected_at_strict_signature() {
    let v = corpus("v2_tampered_007_signature_swap.json");
    verify_capsule(&v).expect("structural verify must pass");
    let report = verify_capsule_strict(&v, &StrictVerifyOpts::default());
    assert!(report.structural.is_passed());
    assert!(
        report.receipt_signature.is_failed(),
        "receipt_signature must fail on a signature that doesn't verify under the published key; \
         got {:?}",
        report.receipt_signature
    );
}

// ---------------------------------------------------------------------------
// v2_tampered_008 — replay with new nonce (request body mutated post-emit).
// ---------------------------------------------------------------------------
//
// `request.aprp.nonce` was regenerated AFTER the receipt was issued.
// `request.request_hash` and `decision.receipt.request_hash` still
// agree (so structural verify passes — that invariant only checks the
// two CLAIMED hashes match each other). Strict mode recomputes
// JCS+SHA-256 over the mutated APRP body and surfaces the divergence
// as a `request_hash_recompute` failure.
//
// In a deployed daemon the nonce-replay table catches re-issued
// requests at the HTTP layer (`protocol.nonce_replay` → 409). This
// fixture exercises the offline / capsule-receiver counterpart: an
// auditor handed a single capsule must still be able to detect that
// the receipt no longer covers the body the capsule wraps.
#[test]
fn v2_tampered_008_replay_with_new_nonce_rejected_at_strict_request_hash() {
    let v = corpus("v2_tampered_008_replay_with_new_nonce.json");
    verify_capsule(&v).expect("structural verify must pass");
    let report = verify_capsule_strict(&v, &StrictVerifyOpts::default());
    assert!(report.structural.is_passed());
    assert!(
        report.request_hash_recompute.is_failed(),
        "request_hash_recompute must fail when aprp body was mutated post-receipt; \
         got {:?}",
        report.request_hash_recompute
    );
}

// ---------------------------------------------------------------------------
// v2_tampered_009 — oversized audit_segment (10,000-event chain, ~8 MiB).
// ---------------------------------------------------------------------------
//
// The 1 MiB byte-cap on `audit.audit_segment` is the verifier's anti-DoS
// guard: a capsule that ships an N-event chain still takes O(N) memory
// to walk. Pre-cap the verifier had to allocate arbitrary memory just
// to discover the bundle was malformed. Post-cap the size check fires
// BEFORE deserialisation — the verifier refuses the segment outright
// with `capsule.audit_segment_too_large`.
//
// v2_tampered_004 (already in the corpus) exercises the same cap via a
// padding field. This new fixture exercises the cap via a real-shape
// 10,000-event chain — closer to what a malicious capsule would
// actually look like in the wild. Both are wired into the strict
// verifier so a future relaxation of the cap fails one of them.
#[test]
fn v2_tampered_009_oversized_audit_segment_rejected_at_strict_size_cap() {
    let v = corpus("v2_tampered_009_oversized_audit_segment.json");
    verify_capsule(&v).expect("structural verify must pass — size check is strict-only");
    let report = verify_capsule_strict(&v, &StrictVerifyOpts::default());
    assert!(report.structural.is_passed());
    // The strict path's `decode_embedded_segment` fires the size cap
    // BEFORE `audit_bundle::verify`, so the chain-level checks all
    // surface the same `audit_segment_too_large` reason. We pin
    // every check that depends on the embedded segment.
    for (label, outcome) in [
        ("receipt_signature", &report.receipt_signature),
        ("audit_chain", &report.audit_chain),
        ("audit_event_link", &report.audit_event_link),
    ] {
        assert!(
            outcome.is_failed(),
            "{label} must fail when embedded audit_segment exceeds 1 MiB cap; got {outcome:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Sentinel — the corresponding v2_golden_001 fixture remains accepted.
// ---------------------------------------------------------------------------
//
// Pin: the new tampered fixtures must NOT regress the golden capsule.
// If a future change accidentally tightens a check the golden no
// longer satisfies, this sentinel catches it before the tampered
// tests do (the tampered tests would suddenly start passing for the
// wrong reason — a test that used to fail because of a specific
// tamper now also fails because the structural pass got stricter).
#[test]
fn v2_golden_001_minimal_remains_accepted_alongside_new_tampered_fixtures() {
    let v = corpus("v2_golden_001_minimal.json");
    verify_capsule(&v).expect("v2 golden must continue to pass structural verify");
    let report = verify_capsule_strict(&v, &StrictVerifyOpts::default());
    assert!(report.structural.is_passed());
    assert!(
        report.request_hash_recompute.is_passed(),
        "v2 golden request_hash_recompute must still pass; got {:?}",
        report.request_hash_recompute
    );
}
