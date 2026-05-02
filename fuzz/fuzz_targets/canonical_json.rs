//! Fuzz the JCS-canonical-JSON serializer.
//!
//! Goal: `canonical_json` must NEVER panic. It's invoked on every
//! audit-row append, every PolicyReceipt sign, every capsule build —
//! a panic in this function takes down the daemon's request path.
//!
//! Run: `cargo fuzz run canonical_json -- -max_total_time=600`

#![no_main]

use libfuzzer_sys::fuzz_target;
use sbo3l_core::hashing::{canonical_json, request_hash};

fuzz_target!(|data: &[u8]| {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) {
        let _ = canonical_json(&v);
        let _ = request_hash(&v);
    }
});
