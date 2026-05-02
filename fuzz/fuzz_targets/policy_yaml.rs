//! Fuzz the policy YAML/JSON parser.
//!
//! Goal: `Policy::parse_yaml` and `Policy::parse_json` must NEVER panic
//! on adversarial input. Customer-supplied policy files are loaded by
//! the daemon at startup; a panic on a malformed policy is a DoS that
//! prevents the daemon from booting.
//!
//! Run: `cargo fuzz run policy_yaml -- -max_total_time=600`

#![no_main]

use libfuzzer_sys::fuzz_target;
use sbo3l_policy::model::Policy;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Policy::parse_yaml(s);
        let _ = Policy::parse_json(s);
    }
});
