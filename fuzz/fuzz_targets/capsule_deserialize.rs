//! Fuzz the Passport capsule verifier.
//!
//! Goal: `verify_capsule` must NEVER panic on adversarial input. The
//! function is invoked by the marketing /proof page WASM and by the CLI;
//! both are reachable by untrusted input.
//!
//! Note: this does NOT fuzz the cryptographic primitives themselves
//! (Ed25519, SHA-256) — those are crates we depend on (`ed25519-dalek`,
//! `sha2`) and have their own fuzz harnesses upstream. We fuzz the
//! BOUNDARY: how SBO3L parses + dispatches against capsule-shaped input.
//!
//! Run: `cargo fuzz run capsule_deserialize -- -max_total_time=600`

#![no_main]

use libfuzzer_sys::fuzz_target;
use sbo3l_core::passport::verify_capsule;

fuzz_target!(|data: &[u8]| {
    // Try parsing as a JSON Value first — capsules are JSON objects.
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) {
        let _ = verify_capsule(&v);
    }
});
