//! Fuzz the APRP wire-format parser.
//!
//! Goal: serde_json::from_str + serde_json::from_slice into PaymentRequest
//! must NEVER panic, regardless of input bytes. A `Result::Err` is fine —
//! a `panic!` is a bug.
//!
//! Coverage targets:
//! - All Intent / Destination / PaymentProtocol / RiskClass enum branches
//! - Optional field combinations (x402_payload, expected_result)
//! - Unicode in string fields
//! - Numeric overflow in `expiry`
//! - Adversarial JSON shapes (deeply nested, missing fields, extra fields)
//!
//! Run: `cargo fuzz run aprp_parser -- -max_total_time=600`
//! Long-run: `cargo fuzz run aprp_parser -- -runs=10000000`

#![no_main]

use libfuzzer_sys::fuzz_target;
use sbo3l_core::aprp::PaymentRequest;

fuzz_target!(|data: &[u8]| {
    // Path 1: from_slice (binary input).
    let _ = serde_json::from_slice::<PaymentRequest>(data);

    // Path 2: from_str (utf8 input). Skip if not valid utf8.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<PaymentRequest>(s);
    }
});
