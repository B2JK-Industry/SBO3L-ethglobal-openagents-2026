//! Fuzz the AuditEvent + SignedAuditEvent deserializers.
//!
//! Goal: parsing an adversarial audit-row blob (e.g. one synthesized by
//! an attacker who wants the verifier to crash before checking the
//! signature) must NEVER panic. Parse errors are fine; panics are bugs.
//!
//! Adversarial shapes to surface:
//! - Missing required fields (version, seq, prev_event_hash, payload_hash)
//! - Numeric overflows on `seq`
//! - Invalid hex in `payload_hash` / `prev_event_hash`
//! - Unicode in `actor` / `subject_id` / `event_type`
//! - Deep object nesting in `metadata`
//!
//! Run: `cargo fuzz run audit_event -- -max_total_time=600`

#![no_main]

use libfuzzer_sys::fuzz_target;
use sbo3l_core::audit::{AuditEvent, SignedAuditEvent};

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<AuditEvent>(data);
    let _ = serde_json::from_slice::<SignedAuditEvent>(data);

    // Also fuzz the canonical-hash recompute on whatever we parsed.
    if let Ok(event) = serde_json::from_slice::<AuditEvent>(data) {
        let _ = event.canonical_hash();
    }
});
