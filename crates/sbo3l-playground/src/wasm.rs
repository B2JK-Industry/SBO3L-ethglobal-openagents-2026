//! R17 P1 — wasm-bindgen JS bridge for the playground bundle.
//!
//! Compiled only on `target_arch = "wasm32"`. Native sbo3l-playground
//! builds skip this module and don't pull `wasm-bindgen` /
//! `serde-wasm-bindgen` (kept as wasm-only deps in `Cargo.toml`).
//!
//! # Surface
//!
//! Three functions, all consumed by `apps/marketing/`'s `/playground`
//! page after `wasm-pack build --target web` runs:
//!
//! - [`decide_aprp_wasm`] — runs the REAL policy decide engine
//!   (`sbo3l_policy::decide`) over caller-supplied APRP + policy JSON.
//!   Returns a `{ decision, matched_rule, deny_code, policy_hash }`
//!   object. No mock — same engine sbo3l-server runs in production.
//! - [`build_capsule_wasm`] — synthesises a fully self-contained
//!   `sbo3l.passport_capsule.v2` capsule signed with a caller-supplied
//!   Ed25519 seed. The capsule passes the 6-check strict verifier
//!   *with no auxiliary input*. Same audit + receipt key (mock
//!   playground key) so the embedded `verification_keys` block stays
//!   honest.
//! - [`sbo3l_playground_version_js`] — crate version exposed to JS so
//!   the playground UI can show "engine built from sbo3l-playground
//!   v1.2.0" honestly.
//!
//! # Why not just expose `verify_capsule_strict_json`
//!
//! `sbo3l-core`'s wasm bundle (#110) already exposes that. The
//! playground page imports both bundles: this one for engine +
//! capsule build, the verifier bundle for the in-browser strict
//! verify. Keeping them separate avoids forcing every /proof page
//! visitor to download `sbo3l-policy`'s ~150KB even though they only
//! verify (no decide).

use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::receipt::Decision as ReceiptDecision;
use sbo3l_policy::{decide as policy_decide, Decision as EngineDecision, Policy};

use crate::passport_offline::{build_capsule_v2_self_contained, OfflineBuildArgs};

/// JS-visible decision response. Field names match the brief's
/// `DecisionResponse JSON` contract.
#[derive(Serialize)]
struct WasmDecisionResponse {
    /// `"allow" | "deny" | "requires_human"`.
    decision: &'static str,
    /// Rule id whose `when` clause fired, or `null` for the default
    /// fall-through.
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_rule: Option<String>,
    /// Deny code, populated only when decision == "deny".
    #[serde(skip_serializing_if = "Option::is_none")]
    deny_code: Option<String>,
    /// JCS+SHA-256 of the canonical policy JSON. Useful for the
    /// playground UI to display alongside the decision so users can
    /// tell two policies apart at a glance.
    policy_hash: String,
}

/// Run the real policy decide engine in the browser.
///
/// JS side calls `decide_aprp_wasm(aprpJson, policyJson)` and gets
/// back a plain object with `{ decision, matched_rule, deny_code,
/// policy_hash }`. Both inputs are JSON strings — `policyJson` accepts
/// the same shape `Policy::parse_json` does on the daemon side.
///
/// **Determinism:** no time, no randomness. Same inputs → same
/// output. The only state is the parsed Policy + APRP.
#[wasm_bindgen(js_name = decide_aprp_wasm)]
pub fn decide_aprp_wasm(aprp_json: &str, policy_json: &str) -> Result<JsValue, JsValue> {
    let aprp_value: Value = serde_json::from_str(aprp_json)
        .map_err(|e| JsValue::from_str(&format!("aprp.parse_error: {e}")))?;
    let aprp: PaymentRequest = serde_json::from_value(aprp_value)
        .map_err(|e| JsValue::from_str(&format!("aprp.schema_error: {e}")))?;
    let policy = Policy::parse_json(policy_json)
        .map_err(|e| JsValue::from_str(&format!("policy.parse_error: {e}")))?;

    let outcome = policy_decide(&policy, &aprp)
        .map_err(|e| JsValue::from_str(&format!("policy.decide_error: {e}")))?;

    let decision_str = match outcome.decision {
        EngineDecision::Allow => "allow",
        EngineDecision::Deny => "deny",
        EngineDecision::RequiresHuman => "requires_human",
    };
    let response = WasmDecisionResponse {
        decision: decision_str,
        matched_rule: outcome.matched_rule_id,
        deny_code: outcome.deny_code,
        policy_hash: outcome.policy_hash,
    };
    serde_wasm_bindgen::to_value(&response)
        .map_err(|e| JsValue::from_str(&format!("serialize_response: {e}")))
}

/// Build a self-contained `sbo3l.passport_capsule.v2` capsule from
/// the caller-supplied APRP + decision response + Ed25519 seed.
///
/// `signing_seed_hex` MUST be exactly 32 bytes (64 hex chars). The
/// builder uses the same seed for both the audit-event signer and the
/// receipt signer — keeping the embedded `verification_keys` block
/// honest (audit_signer_pubkey == receipt_signer_pubkey).
///
/// Returns the capsule as a plain JS object (NOT a JSON string —
/// callers can re-stringify via `JSON.stringify(capsule)` if they
/// need the canonical bytes).
///
/// On `decision == "requires_human"` returns a JS error — the v2
/// capsule's `decision.result` enum is `{allow, deny}` only. The
/// daemon rejects this case upstream; the playground does the same.
#[wasm_bindgen(js_name = build_capsule_wasm)]
pub fn build_capsule_wasm(
    aprp_json: &str,
    decision_response_json: &str,
    policy_json: &str,
    signing_seed_hex: &str,
    issued_at_rfc3339: &str,
) -> Result<JsValue, JsValue> {
    let aprp_value: Value = serde_json::from_str(aprp_json)
        .map_err(|e| JsValue::from_str(&format!("aprp.parse_error: {e}")))?;
    let aprp: PaymentRequest = serde_json::from_value(aprp_value)
        .map_err(|e| JsValue::from_str(&format!("aprp.schema_error: {e}")))?;

    let decision_response: Value = serde_json::from_str(decision_response_json)
        .map_err(|e| JsValue::from_str(&format!("decision.parse_error: {e}")))?;
    let decision_str = decision_response
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsValue::from_str("decision.missing_decision"))?;
    let decision = match decision_str {
        "allow" => ReceiptDecision::Allow,
        "deny" => ReceiptDecision::Deny,
        "requires_human" => ReceiptDecision::RequiresHuman,
        other => {
            return Err(JsValue::from_str(&format!(
                "decision.unknown: `{other}` (expected allow|deny|requires_human)"
            )))
        }
    };
    let matched_rule_id = decision_response
        .get("matched_rule")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let deny_code = decision_response
        .get("deny_code")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // Codex P2 finding (#357): validate the policy schema BEFORE
    // embedding it in a capsule. Without this, a caller could build a
    // structurally-/strictly-verifiable capsule from a malformed
    // policy that the daemon's `Policy::parse_json` would reject.
    // Mirrors what `decide_aprp_wasm` already does, keeping both
    // entry points symmetric.
    let _ = Policy::parse_json(policy_json)
        .map_err(|e| JsValue::from_str(&format!("policy.parse_error: {e}")))?;
    let policy_value: Value = serde_json::from_str(policy_json)
        .map_err(|e| JsValue::from_str(&format!("policy.parse_error: {e}")))?;

    let signing_seed = parse_seed_hex(signing_seed_hex)?;
    let issued_at = chrono::DateTime::parse_from_rfc3339(issued_at_rfc3339)
        .map_err(|e| JsValue::from_str(&format!("issued_at.parse_error: {e}")))?
        .with_timezone(&chrono::Utc);

    let capsule = build_capsule_v2_self_contained(OfflineBuildArgs {
        aprp,
        decision,
        matched_rule_id,
        deny_code,
        policy_json: policy_value,
        signing_seed,
        key_id: "playground-mock-v1".to_string(),
        issued_at,
    })
    .map_err(|e| JsValue::from_str(&format!("capsule.build_error: {e}")))?;

    serde_wasm_bindgen::to_value(&capsule)
        .map_err(|e| JsValue::from_str(&format!("serialize_capsule: {e}")))
}

/// Crate version exposed to JS so the playground UI can show
/// "engine built from sbo3l-playground v1.2.0" honestly.
#[wasm_bindgen(js_name = sbo3l_playground_version)]
pub fn sbo3l_playground_version_js() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn parse_seed_hex(s: &str) -> Result<[u8; 32], JsValue> {
    let bytes = hex::decode(s).map_err(|e| {
        JsValue::from_str(&format!(
            "signing_seed.hex_decode_error: {e} (expected 64 hex chars)"
        ))
    })?;
    if bytes.len() != 32 {
        return Err(JsValue::from_str(&format!(
            "signing_seed.length_error: expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}
