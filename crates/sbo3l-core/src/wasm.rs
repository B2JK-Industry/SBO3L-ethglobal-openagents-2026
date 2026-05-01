//! wasm-bindgen JS bridge for the #110 marketing-site verifier.
//!
//! Compiled only when `target_arch = "wasm32"`. Native targets never
//! pull `wasm-bindgen` or `serde-wasm-bindgen` — see the
//! `[target.'cfg(target_arch = "wasm32")'.dependencies]` block in
//! `crates/sbo3l-core/Cargo.toml`.
//!
//! # Surface
//!
//! Two exported functions, both consumed by `apps/marketing/`'s
//! `/proof` page after `wasm-pack build --target web` runs:
//!
//! - [`verify_capsule_json`] — runs the structural verifier
//!   ([`crate::passport::verify_capsule`]) over a JSON-string capsule.
//!   Returns `null` on success; throws a JS string with the
//!   `(capsule.<code>)` shape on failure.
//! - [`verify_capsule_strict_json`] — runs the strict cryptographic
//!   verifier ([`crate::passport::verify_capsule_strict`]) over a
//!   JSON-string capsule with no auxiliary inputs (i.e. v2
//!   self-contained mode). Returns the structured 6-check report as
//!   a JS object.
//!
//! # Why no aux inputs in the strict variant
//!
//! The browser bundle is the v2 self-contained verifier — the
//! capsule embeds `policy.policy_snapshot` + `audit.audit_segment`,
//! so the wasm module never needs `--policy <path>` or `--audit-bundle
//! <path>`. F-6's `verify_capsule_strict(capsule, &Default::default())`
//! is exactly this path. v1 capsules pass structural + request_hash
//! and report SKIPPED on the other 4 checks (reflecting the
//! capsule's lack of self-contained crypto material).

use serde_json::Value;
use wasm_bindgen::prelude::*;

use crate::passport::{verify_capsule, verify_capsule_strict, StrictVerifyOpts};

/// Structural verify entry point. JS calls
/// `verify_capsule_json(capsuleJsonString)`. Resolves to `null` on
/// success; rejects with the `capsule.<code>` string on failure.
#[wasm_bindgen(js_name = verify_capsule_json)]
pub fn verify_capsule_json_js(capsule_json: &str) -> Result<JsValue, JsValue> {
    let value: Value = serde_json::from_str(capsule_json)
        .map_err(|e| JsValue::from_str(&format!("capsule.parse_error: {e}")))?;
    match verify_capsule(&value) {
        Ok(()) => Ok(JsValue::NULL),
        Err(e) => Err(JsValue::from_str(&format!("{} ({})", e, e.code()))),
    }
}

/// Strict (cryptographic) verify entry point. JS calls
/// `verify_capsule_strict_json(capsuleJsonString)` and gets a
/// structured object back:
///
/// ```ignore
/// {
///   ok: boolean,             // true iff every check passed (no failures, no skips)
///   any_failed: boolean,     // true iff at least one check failed
///   checks: [
///     { label: "structural", outcome: "PASSED" | "SKIPPED" | "FAILED", detail?: string },
///     ...
///   ]
/// }
/// ```
///
/// No auxiliary inputs are accepted — this is the v2 self-contained
/// path. v1 capsules + v2 capsules with missing embedded fields will
/// see SKIPPED outcomes for the aux-dependent checks, which is the
/// expected honest-disclosure behaviour from F-6.
#[wasm_bindgen(js_name = verify_capsule_strict_json)]
pub fn verify_capsule_strict_json_js(capsule_json: &str) -> Result<JsValue, JsValue> {
    let value: Value = serde_json::from_str(capsule_json)
        .map_err(|e| JsValue::from_str(&format!("capsule.parse_error: {e}")))?;
    let report = verify_capsule_strict(&value, &StrictVerifyOpts::default());

    use crate::passport::{CheckOutcome, StrictVerifyReport};
    let labels = StrictVerifyReport::labels();
    let outcomes: Vec<&CheckOutcome> = report.iter().collect();
    let checks: Vec<Value> = labels
        .iter()
        .zip(outcomes.iter())
        .map(|(label, outcome)| {
            let (status, detail) = match outcome {
                CheckOutcome::Passed => ("PASSED", None),
                CheckOutcome::Skipped(d) => ("SKIPPED", Some(d.clone())),
                CheckOutcome::Failed(d) => ("FAILED", Some(d.clone())),
            };
            let mut obj = serde_json::Map::new();
            obj.insert("label".into(), Value::String((*label).to_string()));
            obj.insert("outcome".into(), Value::String(status.to_string()));
            if let Some(d) = detail {
                obj.insert("detail".into(), Value::String(d));
            }
            Value::Object(obj)
        })
        .collect();
    let any_failed = outcomes.iter().any(|o| o.is_failed());
    let any_skipped = outcomes.iter().any(|o| o.is_skipped());
    let mut root = serde_json::Map::new();
    root.insert("ok".into(), Value::Bool(report.is_fully_ok()));
    root.insert("any_failed".into(), Value::Bool(any_failed));
    root.insert("any_skipped".into(), Value::Bool(any_skipped));
    root.insert("checks".into(), Value::Array(checks));
    serde_wasm_bindgen::to_value(&Value::Object(root))
        .map_err(|e| JsValue::from_str(&format!("serialize_report: {e}")))
}

/// Crate version exposed to JS so the marketing site can show
/// "verifier built from sbo3l-core v0.1.0" honestly.
#[wasm_bindgen(js_name = sbo3l_core_version)]
pub fn sbo3l_core_version_js() -> String {
    crate::version().to_string()
}
