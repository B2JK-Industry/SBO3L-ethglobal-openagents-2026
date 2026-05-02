//! R17 P1 — offline capsule builder for the browser playground.
//!
//! Native-callable (no wasm-bindgen) so unit tests can exercise the
//! builder + strict verifier directly. The wasm-bindgen wrapper in
//! [`crate::wasm::build_capsule_wasm`] is a thin shim around
//! [`build_capsule_v2_self_contained`].
//!
//! # What this builds
//!
//! A fully self-contained `sbo3l.passport_capsule.v2` capsule whose
//! strict verifier ([`sbo3l_core::passport::verify_capsule_strict`])
//! passes ALL 6 checks WITHOUT auxiliary input:
//!
//! - **structural** — schema + cross-field invariants (request_hash
//!   agreement, decision agreement, etc.).
//! - **request_hash_recompute** — JCS+SHA-256 over `request.aprp`.
//! - **policy_hash_recompute** — JCS+SHA-256 over the embedded
//!   `policy.policy_snapshot`.
//! - **receipt_signature** — Ed25519 verify under the embedded
//!   audit bundle's `verification_keys.receipt_signer_pubkey_hex`.
//! - **audit_chain** — single-event chain (seq=1, prev=ZERO_HASH)
//!   walked under `verification_keys.audit_signer_pubkey_hex`.
//! - **audit_event_link** — `decision.audit_event_id` matches the
//!   chain's lone signed event.
//!
//! The audit + receipt signers are the SAME caller-supplied seed —
//! a single key keeps the embedded `verification_keys` block honest
//! (no risk of pubkey drift between roles in the playground).
//!
//! # Determinism
//!
//! - Audit event id is `evt-` + Crockford-Base32 string derived from
//!   `sha256(canonical_aprp || canonical_policy || decision_str)`,
//!   masked to fit `^[0-7][0-9A-HJKMNP-TV-Z]{25}$`.
//! - Wall-clock fields take the caller-supplied `issued_at`. Same
//!   inputs → byte-identical output capsule (test pin in
//!   `deterministic_byte_for_byte_across_runs`).

use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::audit::{AuditEvent, SignedAuditEvent, ZERO_HASH};
use sbo3l_core::audit_bundle::{AuditBundle, BundleSummary, BundleType, VerificationKeys};
use sbo3l_core::error::{CoreError, Result};
use sbo3l_core::hashing::{request_hash, sha256_hex};
use sbo3l_core::receipt::{Decision, UnsignedReceipt};
use sbo3l_core::signer::DevSigner;

/// Inputs the playground builder accepts. All `Value`-shaped so the
/// browser side can pass straight from `JSON.parse`.
#[derive(Debug, Clone)]
pub struct OfflineBuildArgs {
    pub aprp: PaymentRequest,
    pub decision: Decision,
    pub matched_rule_id: Option<String>,
    pub deny_code: Option<String>,
    /// Canonical policy JSON (the *same* JSON `Policy::parse_json`
    /// accepts). Embedded under `policy.policy_snapshot` so the
    /// strict verifier can recompute `policy_hash` without aux input.
    pub policy_json: Value,
    /// 32-byte Ed25519 seed (raw bytes). The builder derives a
    /// `DevSigner` from this and uses it to sign BOTH the audit event
    /// and the receipt — single-key keeps the embedded
    /// `verification_keys` block honest.
    pub signing_seed: [u8; 32],
    /// Stable key-id label embedded in signatures.
    pub key_id: String,
    /// Wall-clock the capsule + audit event get stamped with.
    pub issued_at: DateTime<Utc>,
}

/// Build a fully self-contained `sbo3l.passport_capsule.v2` JSON
/// value. Returns the capsule as a `Value` so callers can either
/// re-serialise it or pass straight back as a JS object.
///
/// On `Decision::RequiresHuman` returns `Err` — the capsule's
/// `decision.result` enum is `{allow, deny}` only.
pub fn build_capsule_v2_self_contained(args: OfflineBuildArgs) -> Result<Value> {
    if matches!(args.decision, Decision::RequiresHuman) {
        return Err(CoreError::Schema(sbo3l_core::SchemaError::InvalidRoot {
            detail:
                "capsule.requires_human: decision.requires_human cannot be embedded in a passport capsule"
                    .to_string(),
        }));
    }

    let signer = DevSigner::from_seed(args.key_id.clone(), args.signing_seed);
    let verifying_key_hex = signer.verifying_key_hex();

    // 1. APRP serialised + request_hash.
    let aprp_value = serde_json::to_value(&args.aprp).map_err(CoreError::Json)?;
    let req_hash = request_hash(&aprp_value)?;

    // 2. Canonical policy snapshot + JCS+SHA-256 hash.
    let policy_canonical_bytes =
        serde_json_canonicalizer::to_string(&args.policy_json).map_err(CoreError::Json)?;
    let policy_hash = sha256_hex(policy_canonical_bytes.as_bytes());

    // 3. Synthesise the audit event (single-event chain, genesis prev_hash).
    let event_id = derive_audit_event_id(&aprp_value, &policy_canonical_bytes, &args.decision);
    let mut metadata = Map::new();
    metadata.insert(
        "decision".into(),
        Value::String(decision_str(&args.decision).to_string()),
    );
    if let Some(rule) = &args.matched_rule_id {
        metadata.insert("matched_rule".into(), Value::String(rule.clone()));
    }
    if let Some(code) = &args.deny_code {
        metadata.insert("deny_code".into(), Value::String(code.clone()));
    }
    let audit_event = AuditEvent {
        version: 1,
        seq: 1,
        id: event_id.clone(),
        ts: args.issued_at,
        event_type: "policy_decided".to_string(),
        actor: "playground".to_string(),
        subject_id: format!("pr-{}", &event_id[4..]),
        payload_hash: req_hash.clone(),
        metadata,
        policy_version: Some(1),
        policy_hash: Some(policy_hash.clone()),
        attestation_ref: None,
        prev_event_hash: ZERO_HASH.to_string(),
    };
    let signed_event = SignedAuditEvent::sign(audit_event, &signer)?;

    // 4. Build + sign the policy receipt.
    let receipt = UnsignedReceipt {
        agent_id: args.aprp.agent_id.clone(),
        decision: args.decision.clone(),
        deny_code: args.deny_code.clone(),
        request_hash: req_hash.clone(),
        policy_hash: policy_hash.clone(),
        policy_version: Some(1),
        audit_event_id: event_id.clone(),
        execution_ref: None,
        issued_at: args.issued_at,
        expires_at: None,
    }
    .sign(&signer)?;

    // 5. Wrap into the AuditBundle the strict verifier consumes.
    // `audit_chain_root` is the FIRST event's event_hash (NOT ZERO_HASH —
    // ZERO_HASH is the genesis prev_event_hash, a different sentinel).
    // Single-event chain → root == latest.
    let chain_root = signed_event.event_hash.clone();
    let chain_latest = signed_event.event_hash.clone();
    let bundle_summary = BundleSummary {
        decision: args.decision.clone(),
        deny_code: args.deny_code.clone(),
        request_hash: req_hash.clone(),
        policy_hash: policy_hash.clone(),
        audit_event_id: event_id.clone(),
        audit_event_hash: signed_event.event_hash.clone(),
        audit_chain_root: chain_root,
        audit_chain_latest: chain_latest,
    };
    let bundle = AuditBundle {
        bundle_type: BundleType::AuditBundleV1,
        version: 1,
        exported_at: args.issued_at,
        receipt: receipt.clone(),
        audit_event: signed_event.clone(),
        audit_chain_segment: vec![signed_event.clone()],
        verification_keys: VerificationKeys {
            receipt_signer_pubkey_hex: verifying_key_hex.clone(),
            audit_signer_pubkey_hex: verifying_key_hex.clone(),
        },
        summary: bundle_summary,
    };

    // 6. Assemble the v2 capsule.
    let receipt_value = serde_json::to_value(&receipt).map_err(CoreError::Json)?;
    let receipt_signature_hex = receipt.signature.signature_hex.clone();

    let agent_block = json!({
        "agent_id": args.aprp.agent_id,
        "ens_name": "playground.local",
        // Schema enum: {"offline-fixture", "live-ens"}. We're the
        // offline browser playground so "offline-fixture" is the
        // honest value (we have NO ENS resolver attached).
        "resolver": "offline-fixture",
        "records": {
            "sbo3l:policy_hash": policy_hash,
            "sbo3l:audit_root": signed_event.event_hash,
            "sbo3l:passport_schema": "sbo3l.passport_capsule.v2",
        }
    });
    let request_block = json!({
        "aprp": aprp_value,
        "request_hash": req_hash,
        "idempotency_key": Value::Null,
        "nonce": args.aprp.nonce,
    });
    let policy_block = json!({
        "policy_hash": policy_hash,
        "policy_version": 1,
        "activated_at": args.issued_at.to_rfc3339(),
        "source": "playground-input",
        "policy_snapshot": args.policy_json,
    });
    let decision_block = json!({
        "result": decision_str(&args.decision),
        "matched_rule": args.matched_rule_id,
        "deny_code": args.deny_code,
        "receipt": receipt_value,
        "receipt_signature": receipt_signature_hex,
    });
    let execution_block = if matches!(args.decision, Decision::Deny) {
        json!({
            "executor": "none",
            "mode": "mock",
            "status": "not_called",
            "sponsor_payload_hash": null,
            "live_evidence": null,
        })
    } else {
        json!({
            "executor": "none",
            "mode": "mock",
            "execution_ref": format!("pg-{}", &event_id[4..]),
            "status": "submitted",
            "sponsor_payload_hash": null,
            "live_evidence": null,
        })
    };
    let bundle_value = serde_json::to_value(&bundle).map_err(CoreError::Json)?;
    // checkpoint.mock_anchor_ref must match `^local-mock-anchor-[0-9a-f]{16}$`
    // per `schemas/sbo3l.passport_capsule.v2.json`. Derive from the
    // first 16 hex chars of the event hash so it's deterministic +
    // unique per capsule.
    let mock_anchor_ref = format!("local-mock-anchor-{}", &signed_event.event_hash[..16]);
    let audit_block = json!({
        "audit_event_id": event_id,
        "prev_event_hash": ZERO_HASH,
        "event_hash": signed_event.event_hash,
        "bundle_ref": "sbo3l.audit_bundle.v1",
        "audit_segment": bundle_value,
        "checkpoint": {
            "schema": "sbo3l.audit_checkpoint.v1",
            "sequence": 1u64,
            "latest_event_id": event_id,
            "latest_event_hash": signed_event.event_hash,
            "chain_digest": signed_event.event_hash,
            "mock_anchor": true,
            "mock_anchor_ref": mock_anchor_ref,
            "created_at": args.issued_at.to_rfc3339(),
        }
    });

    let verification_block = json!({
        "doctor_status": "ok",
        "offline_verifiable": true,
        "live_claims": [],
    });

    Ok(json!({
        "schema": "sbo3l.passport_capsule.v2",
        "generated_at": args.issued_at.to_rfc3339(),
        "agent": agent_block,
        "request": request_block,
        "policy": policy_block,
        "decision": decision_block,
        "execution": execution_block,
        "audit": audit_block,
        "verification": verification_block,
    }))
}

fn decision_str(d: &Decision) -> &'static str {
    match d {
        Decision::Allow => "allow",
        Decision::Deny => "deny",
        Decision::RequiresHuman => "requires_human",
    }
}

/// Derive a Crockford-Base32-shaped `evt-…` id deterministically from
/// the inputs. Format matches the schema regex
/// `^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$`.
fn derive_audit_event_id(aprp: &Value, policy_canon: &str, decision: &Decision) -> String {
    let mut buf = Vec::with_capacity(512);
    if let Ok(s) = serde_json_canonicalizer::to_string(aprp) {
        buf.extend_from_slice(s.as_bytes());
    }
    buf.extend_from_slice(b"|");
    buf.extend_from_slice(policy_canon.as_bytes());
    buf.extend_from_slice(b"|");
    buf.extend_from_slice(decision_str(decision).as_bytes());
    let digest_hex = sha256_hex(&buf);
    encode_crockford_ulid_shape(&digest_hex[..32])
}

const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn encode_crockford_ulid_shape(hex32: &str) -> String {
    let mut bytes = [0u8; 16];
    for (i, byte) in bytes.iter_mut().enumerate() {
        let off = i * 2;
        *byte = u8::from_str_radix(&hex32[off..off + 2], 16).unwrap_or(0);
    }
    let mut value: u128 = 0;
    for b in &bytes {
        value = (value << 8) | (*b as u128);
    }
    let mut out = [0u8; 26];
    for slot in out.iter_mut().rev() {
        *slot = CROCKFORD[(value & 0x1f) as usize];
        value >>= 5;
    }
    // First char must be 0-7 (3-bit head).
    let head_idx = (out[0] as char).to_digit(36).unwrap_or(0) & 0x7;
    out[0] = CROCKFORD[head_idx as usize];
    format!("evt-{}", std::str::from_utf8(&out).unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sbo3l_core::aprp::{
        Currency, Destination, HttpMethod, Intent, Money, PaymentProtocol, PaymentRequest,
        RiskClass,
    };
    use sbo3l_core::passport::{verify_capsule, verify_capsule_strict, StrictVerifyOpts};

    fn fixture_aprp() -> PaymentRequest {
        PaymentRequest {
            agent_id: "research-agent-01".to_string(),
            task_id: "demo-task".to_string(),
            intent: Intent::PurchaseApiCall,
            amount: Money {
                value: "0.05".to_string(),
                currency: Currency::USD,
            },
            token: "USDC".to_string(),
            destination: Destination::X402Endpoint {
                url: "https://api.example.com/v1/inference".to_string(),
                method: HttpMethod::Post,
                expected_recipient: Some("0x1111111111111111111111111111111111111111".to_string()),
            },
            payment_protocol: PaymentProtocol::X402,
            chain: "base".to_string(),
            provider_url: "https://api.example.com".to_string(),
            x402_payload: None,
            expiry: chrono::DateTime::parse_from_rfc3339("2099-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM".to_string(),
            expected_result: None,
            risk_class: RiskClass::Low,
        }
    }

    fn fixture_args(decision: Decision) -> OfflineBuildArgs {
        OfflineBuildArgs {
            aprp: fixture_aprp(),
            decision,
            matched_rule_id: Some("allow-low-risk-x402".to_string()),
            deny_code: None,
            policy_json: serde_json::json!({
                "version": 1,
                "default_decision": "deny",
                "agents": [{"agent_id": "research-agent-01", "status": "active"}],
                "providers": [],
                "recipients": [],
                "rules": [
                    {"id": "allow-low-risk-x402", "when": "request.risk_class == \"low\"", "effect": "allow"}
                ]
            }),
            signing_seed: [42u8; 32],
            key_id: "playground-mock-v1".to_string(),
            issued_at: chrono::DateTime::parse_from_rfc3339("2026-05-02T17:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    #[test]
    fn allow_capsule_passes_structural_verify() {
        let capsule =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("allow build");
        verify_capsule(&capsule).expect("structural verify");
    }

    #[test]
    fn deny_capsule_passes_structural_verify() {
        let mut args = fixture_args(Decision::Deny);
        args.deny_code = Some("policy.unknown_provider".to_string());
        args.matched_rule_id = Some("deny-default".to_string());
        let capsule = build_capsule_v2_self_contained(args).expect("deny build");
        verify_capsule(&capsule).expect("structural verify");
    }

    #[test]
    fn requires_human_is_rejected() {
        let err = build_capsule_v2_self_contained(fixture_args(Decision::RequiresHuman))
            .expect_err("requires_human must reject");
        let msg = err.to_string();
        assert!(
            msg.contains("requires_human") || msg.contains("capsule.requires_human"),
            "error must name the cause; got: {msg}"
        );
    }

    #[test]
    fn allow_capsule_passes_strict_self_contained() {
        let capsule =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("allow build");
        let report = verify_capsule_strict(&capsule, &StrictVerifyOpts::default());
        assert!(
            report.is_fully_ok(),
            "all 6 strict checks must PASS without aux input; report = {report:?}"
        );
    }

    #[test]
    fn deny_capsule_passes_strict_self_contained() {
        let mut args = fixture_args(Decision::Deny);
        args.deny_code = Some("policy.unknown_provider".to_string());
        args.matched_rule_id = Some("deny-default".to_string());
        let capsule = build_capsule_v2_self_contained(args).expect("deny build");
        let report = verify_capsule_strict(&capsule, &StrictVerifyOpts::default());
        assert!(
            report.is_fully_ok(),
            "deny capsule strict must fully pass; report = {report:?}"
        );
    }

    #[test]
    fn deterministic_byte_for_byte_across_runs() {
        let a =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("first build");
        let b =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("second build");
        let a_bytes = serde_json::to_vec(&a).unwrap();
        let b_bytes = serde_json::to_vec(&b).unwrap();
        assert_eq!(a_bytes, b_bytes);
    }

    #[test]
    fn different_aprp_produces_different_audit_event_id() {
        let a =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("first build");
        let mut args2 = fixture_args(Decision::Allow);
        args2.aprp.task_id = "demo-task-different".to_string();
        let b = build_capsule_v2_self_contained(args2).expect("second build");
        let id_a = a["audit"]["audit_event_id"].as_str().unwrap();
        let id_b = b["audit"]["audit_event_id"].as_str().unwrap();
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn audit_event_id_matches_crockford_ulid_shape() {
        let capsule =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("build");
        let id = capsule["audit"]["audit_event_id"].as_str().unwrap();
        assert!(id.starts_with("evt-"));
        let body = &id[4..];
        assert_eq!(body.len(), 26);
        let head = body.chars().next().unwrap();
        assert!(('0'..='7').contains(&head), "head must be 0-7: {head}");
        for c in body.chars().skip(1) {
            assert!(
                !matches!(c, 'I' | 'L' | 'O' | 'U'),
                "Crockford forbids I/L/O/U; got {c} in {id}"
            );
        }
    }

    #[test]
    fn tampered_request_hash_fails_structural() {
        let mut capsule =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("build");
        capsule["request"]["request_hash"] = Value::String("0".repeat(64));
        verify_capsule(&capsule).expect_err("tamper must fail");
    }

    #[test]
    fn tampered_policy_snapshot_fails_strict() {
        // Mutate one field in the embedded policy_snapshot. The
        // strict verifier's policy_hash_recompute should FAIL because
        // the recomputed JCS hash no longer matches policy.policy_hash.
        let mut capsule =
            build_capsule_v2_self_contained(fixture_args(Decision::Allow)).expect("build");
        if let Some(snap) = capsule["policy"]["policy_snapshot"].as_object_mut() {
            snap.insert("malicious_field".into(), Value::Bool(true));
        }
        let report = verify_capsule_strict(&capsule, &StrictVerifyOpts::default());
        assert!(
            report.policy_hash_recompute.is_failed(),
            "policy_hash_recompute must FAIL on tampered snapshot; got {:?}",
            report.policy_hash_recompute
        );
    }
}
