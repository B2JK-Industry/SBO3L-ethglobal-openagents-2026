//! Wire-shape types for the wasm-bindgen JS bridge.
//!
//! Lives outside [`crate::wasm`] (which is `target_arch = "wasm32"`-gated)
//! so the contract can be unit-tested on native targets without
//! pulling `wasm-pack test`. `serde_wasm_bindgen` serialises typed
//! structs the same way `serde_json` does — as plain objects, NOT
//! `Map`-like wrappers — so a native `serde_json::to_value` round-trip
//! is a faithful proxy for "what the JS side actually receives".
//!
//! The Codex P1 finding on PR #134 was that the previous impl
//! serialised a `serde_json::Value::Object` through
//! `serde_wasm_bindgen`, which produces a `Map`-like in JS (caller
//! has to use `Map.get(...)` instead of `obj.checks[0].outcome`).
//! Switching to typed structs fixes that and the
//! [`tests::strict_report_serialises_as_plain_object`] test pins it.

use serde::Serialize;

/// Top-level report returned by `verify_capsule_strict_json` on the
/// JS side. Property names match the F-6 wire contract documented on
/// `crate::wasm::verify_capsule_strict_json_js`.
#[derive(Serialize)]
pub struct WasmStrictReport {
    pub ok: bool,
    pub any_failed: bool,
    pub any_skipped: bool,
    pub checks: Vec<WasmStrictCheck>,
}

/// One row of the 6-check report.
#[derive(Serialize)]
pub struct WasmStrictCheck {
    pub label: &'static str,
    pub outcome: &'static str,
    /// Omitted from the wire when the check passed; carries the
    /// human-readable reason on SKIPPED / FAILED.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixture_report() -> WasmStrictReport {
        WasmStrictReport {
            ok: false,
            any_failed: false,
            any_skipped: true,
            checks: vec![
                WasmStrictCheck {
                    label: "structural",
                    outcome: "PASSED",
                    detail: None,
                },
                WasmStrictCheck {
                    label: "request_hash_recompute",
                    outcome: "PASSED",
                    detail: None,
                },
                WasmStrictCheck {
                    label: "policy_hash_recompute",
                    outcome: "SKIPPED",
                    detail: Some("no policy snapshot embedded".into()),
                },
                WasmStrictCheck {
                    label: "receipt_signature",
                    outcome: "PASSED",
                    detail: None,
                },
                WasmStrictCheck {
                    label: "audit_chain",
                    outcome: "SKIPPED",
                    detail: Some("no audit segment embedded".into()),
                },
                WasmStrictCheck {
                    label: "audit_event_link",
                    outcome: "PASSED",
                    detail: None,
                },
            ],
        }
    }

    /// Codex P1 regression — pins the JS-visible shape. Top-level
    /// must serialise as a JSON object with named properties (NOT a
    /// `Map`-like wrapper). `serde_wasm_bindgen::to_value(&typed)`
    /// produces the same plain-object shape `serde_json::to_value`
    /// produces, which is what JS callers expect when they write
    /// `report.checks[0].outcome`.
    #[test]
    fn strict_report_serialises_as_plain_object() {
        let payload = fixture_report();
        let v: Value = serde_json::to_value(&payload).expect("serialise");
        let obj = v.as_object().expect("top-level must be JSON object");
        assert!(obj.contains_key("ok"));
        assert!(obj.contains_key("any_failed"));
        assert!(obj.contains_key("any_skipped"));
        assert!(obj.contains_key("checks"));
        // `checks` is an Array; element is an Object accessible by
        // property — same shape JS sees.
        let checks = v["checks"].as_array().expect("checks must be Array");
        assert_eq!(checks.len(), 6);
        let first = checks[0].as_object().expect("check[0] must be Object");
        assert!(first.contains_key("label"));
        assert!(first.contains_key("outcome"));
    }

    /// `report.checks[0].outcome` access path (the exact one the
    /// marketing-site verifier uses) — pinned via the JSON
    /// equivalent. Catches any future change that wraps a check in
    /// a Map-like or renames the keys.
    #[test]
    fn check_outcome_accessible_via_direct_property_path() {
        let payload = fixture_report();
        let v: Value = serde_json::to_value(&payload).unwrap();
        // `obj.checks[0].outcome` in JS = `v["checks"][0]["outcome"]`
        // in serde_json terms. If serialisation flipped to a Map,
        // this access would fail (Map keys aren't string-indexed via
        // `.["foo"]`).
        assert_eq!(v["checks"][0]["outcome"], "PASSED");
        assert_eq!(v["checks"][2]["outcome"], "SKIPPED");
        assert_eq!(v["checks"][2]["detail"], "no policy snapshot embedded",);
    }

    /// `detail` is omitted when `None` (per
    /// `skip_serializing_if = "Option::is_none"`). JS callers
    /// distinguish "no detail" from "empty detail" via property
    /// presence — keeping this contract avoids surprising the UI.
    #[test]
    fn detail_omitted_when_check_passed() {
        let payload = fixture_report();
        let v: Value = serde_json::to_value(&payload).unwrap();
        // checks[0] is "structural" with PASSED + detail=None.
        let first = v["checks"][0].as_object().unwrap();
        assert!(
            !first.contains_key("detail"),
            "passed checks must not carry a `detail` key; got {first:?}"
        );
    }

    /// Wire-format key lock — adding a key is fine (additive);
    /// renaming or removing one is a wire break that the marketing
    /// site verifier consumes directly.
    #[test]
    fn report_top_level_keys_are_stable() {
        let payload = fixture_report();
        let v: Value = serde_json::to_value(&payload).unwrap();
        let obj = v.as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["any_failed", "any_skipped", "checks", "ok"]);
    }
}
