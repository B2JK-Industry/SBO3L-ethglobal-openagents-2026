//! Sponsor-execution trait + types.
//!
//! Hosting these in `sbo3l-core` (rather than `sbo3l-execution`) is the
//! IP-4 prerequisite from `docs/keeperhub-integration-paths.md`: a future
//! `sbo3l-keeperhub-adapter` crate can `cargo add sbo3l-core` and
//! implement [`GuardedExecutor`] without pulling the whole SBO3L
//! workspace.
//!
//! `sbo3l-execution` re-exports these symbols so existing call sites
//! (`sbo3l-server`, `sbo3l-cli`, `sbo3l-mcp`, `demo-agents/research-agent`)
//! continue to compile unchanged.

use crate::aprp::PaymentRequest;
use crate::receipt::{Decision, PolicyReceipt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("policy receipt rejected: decision={0:?}")]
    NotApproved(Decision),
    #[error("sponsor backend offline: {0}")]
    BackendOffline(String),
    #[error("integration: {0}")]
    Integration(String),
    /// A live sponsor backend was reachable but the round-trip failed
    /// at the protocol layer: non-2xx HTTP status, network error,
    /// timeout, or a 2xx body that couldn't be parsed into the
    /// expected response shape. Used by `KeeperHubExecutor::live()`
    /// (A8 / Round-3 KH-prize follow-up); distinct from
    /// `BackendOffline` (which means "live mode not configured at
    /// all") and `Integration` (which is for sponsor-internal errors
    /// surfaced *inside* a successful round-trip).
    #[error("protocol: {0}")]
    ProtocolError(String),
}

#[derive(Debug, Clone)]
pub struct ExecutionReceipt {
    pub sponsor: &'static str,
    pub execution_ref: String,
    pub mock: bool,
    pub note: String,
    /// Sponsor-specific evidence captured at execution time. Today this
    /// is populated by the Uniswap mock executor with a
    /// `UniswapQuoteEvidence` payload (P6.1 — see
    /// `sbo3l_execution::uniswap::UniswapQuoteEvidence`); KeeperHub
    /// leaves it `None`. The CLI's `passport run` reads this field and
    /// puts the value into the capsule's NEW
    /// `execution.executor_evidence` slot (P6.1 schema bump — distinct
    /// from the transport-level `live_evidence` slot, which stays
    /// strictly live-only via the verifier's bidirectional invariant).
    /// The schema requires `executor_evidence` to be either `null` /
    /// omitted, or a non-empty object (`minProperties: 1`,
    /// `additionalProperties: true`).
    ///
    /// `None` means "no sponsor evidence captured" and the CLI omits
    /// the field from the capsule's `execution` block (the schema
    /// permits the missing field via the `oneOf null / object` slot).
    /// To attach evidence, executors set this to
    /// `Some(serde_json::Value::Object(map))` where the map has at
    /// least one property.
    pub evidence: Option<serde_json::Value>,
}

/// Contract every sponsor adapter implements. An executor takes a
/// SBO3L-approved [`PolicyReceipt`] plus the original [`PaymentRequest`]
/// and returns an [`ExecutionReceipt`] callers can attach to the audit
/// log.
///
/// * **SBO3L decides.** The receipt is the proof of authorisation.
/// * **Sponsor executes.** Each adapter is a thin wrapper over the
///   partner's real interface (or a clearly-disclosed local mock when
///   credentials are not available).
pub trait GuardedExecutor {
    fn sponsor_id(&self) -> &'static str;
    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError>;
}

/// IP-1 upstream-proof envelope (KeeperHub Integration Path catalogue
/// §IP-1, see `docs/keeperhub-integration-paths.md`). Carried alongside
/// the APRP body + signed `PolicyReceipt` on every workflow-webhook
/// submission so a downstream auditor can re-verify the policy decision
/// **without trusting either side**:
///
/// - `sbo3l_request_hash` — JCS-canonical SHA-256 of the APRP body.
///   Same value any other SBO3L consumer can re-derive.
/// - `sbo3l_policy_hash` — canonical hash of the active policy.
///   Drift means the same agent produced this request under a different
///   rulebook.
/// - `sbo3l_receipt_signature` — Ed25519 signature on the
///   `PolicyReceipt`, verifiable against the receipt-signer pubkey
///   published in the agent's ENS / Passport.
/// - `sbo3l_audit_event_id` — position of the decision in SBO3L's
///   hash-chained audit log; lets the auditor pull the chain prefix and
///   re-derive the per-event hash.
/// - `sbo3l_passport_capsule_hash` — *target*, optional today. Once a
///   Passport capsule lives at a published URI, its content hash goes
///   here so the auditor can pin the snapshot they verified against.
///
/// Field order is fixed by the JSON Schema sketch in
/// `docs/keeperhub-integration-paths.md` §IP-3 — the
/// `envelope_serialises_with_documented_field_order` test pins it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sbo3lEnvelope {
    pub sbo3l_request_hash: String,
    pub sbo3l_policy_hash: String,
    pub sbo3l_receipt_signature: String,
    pub sbo3l_audit_event_id: String,
    /// Optional. Omitted from the wire form when `None` so that
    /// pre-Passport KeeperHub deployments don't need to special-case
    /// it. Set via [`Sbo3lEnvelope::with_passport_capsule`] once the
    /// Passport surface ships (P5.1 → P7.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbo3l_passport_capsule_hash: Option<String>,
}

impl Sbo3lEnvelope {
    /// Build an envelope from a freshly-signed `PolicyReceipt` plus the
    /// audit chain's just-appended event id (the value
    /// [`PaymentRequestResponse::audit_event_id`](
    /// https://docs.rs/sbo3l-server) carries back over HTTP).
    ///
    /// Notably, this does **not** copy `receipt.audit_event_id`
    /// blindly: callers pass the `audit_event_id` they actually saw on
    /// the response so a future receipt-shape change can't silently
    /// desync the envelope from the wire. The two values must agree;
    /// any verifier that re-derives the receipt's signature will catch
    /// a mismatch immediately.
    pub fn from_receipt(receipt: &PolicyReceipt, audit_event_id: &str) -> Self {
        Self {
            sbo3l_request_hash: receipt.request_hash.clone(),
            sbo3l_policy_hash: receipt.policy_hash.clone(),
            sbo3l_receipt_signature: receipt.signature.signature_hex.clone(),
            sbo3l_audit_event_id: audit_event_id.to_string(),
            sbo3l_passport_capsule_hash: None,
        }
    }

    /// Attach a Passport capsule content hash. The capsule is the
    /// portable proof artefact (`sbo3l.passport_capsule.v1`) that
    /// `sbo3l passport run` writes; once a Passport URI exists the
    /// envelope can pin its hash so an auditor can detect tampering of
    /// the published file. Today the field stays `None` for
    /// pre-Passport deployments.
    pub fn with_passport_capsule(mut self, capsule_hash: String) -> Self {
        self.sbo3l_passport_capsule_hash = Some(capsule_hash);
        self
    }

    /// Returns the canonical wire-format String. Field order is honoured
    /// via `derive(Serialize)` on the struct; this method bypasses
    /// `serde_json::Value`, which would otherwise alphabetically reorder
    /// keys under the workspace's no-`preserve_order` `serde_json` setup.
    ///
    /// The five-field declaration order matches the JSON Schema sketch in
    /// `docs/keeperhub-integration-paths.md` §IP-3 (request_hash →
    /// policy_hash → receipt_signature → audit_event_id →
    /// passport_capsule_hash) so KeeperHub-side auditors can byte-diff
    /// envelopes from different SBO3L instances and not see spurious
    /// reorderings.
    pub fn to_json_payload(&self) -> String {
        serde_json::to_string(self)
            .expect("Sbo3lEnvelope's #[derive(Serialize)] is infallible for owned fields")
    }
}

#[cfg(test)]
mod envelope_tests {
    use super::*;
    use crate::receipt::{EmbeddedSignature, ReceiptType, SignatureAlgorithm};

    /// Reuse the same fixture-shape `sbo3l-execution/src/keeperhub.rs`
    /// uses — same agent_id, same hash widths, same audit_event_id
    /// shape — so the envelope tests pin the exact wire shape the live
    /// adapter will produce when it stops returning `BackendOffline`.
    fn fixture_receipt() -> PolicyReceipt {
        PolicyReceipt {
            receipt_type: ReceiptType::PolicyReceiptV1,
            version: 1,
            agent_id: "research-agent-01".to_string(),
            decision: Decision::Allow,
            deny_code: None,
            request_hash: "a".repeat(64),
            policy_hash: "b".repeat(64),
            policy_version: Some(1),
            audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            execution_ref: None,
            issued_at: chrono::Utc::now(),
            expires_at: None,
            signature: EmbeddedSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: "decision-signer-v1".to_string(),
                signature_hex: "f".repeat(128),
            },
        }
    }

    #[test]
    fn envelope_constructed_from_real_receipt() {
        let r = fixture_receipt();
        let env = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id);
        assert_eq!(env.sbo3l_request_hash, r.request_hash);
        assert_eq!(env.sbo3l_policy_hash, r.policy_hash);
        assert_eq!(env.sbo3l_receipt_signature, r.signature.signature_hex);
        assert_eq!(env.sbo3l_audit_event_id, r.audit_event_id);
        assert_eq!(env.sbo3l_passport_capsule_hash, None);
    }

    #[test]
    fn envelope_serialises_with_documented_field_order() {
        // Field order pinned: request_hash → policy_hash → receipt_signature
        // → audit_event_id → passport_capsule_hash. `serde_json::to_string`
        // honours struct-declaration order for the `Serialize` derive,
        // so a mismatched key ordering is a real regression that this
        // test catches.
        let r = fixture_receipt();
        let env = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id)
            .with_passport_capsule("c".repeat(64));
        let s = serde_json::to_string(&env).expect("serialise");
        let r_idx = s.find("sbo3l_request_hash").expect("request_hash key");
        let p_idx = s.find("sbo3l_policy_hash").expect("policy_hash key");
        let sig_idx = s.find("sbo3l_receipt_signature").expect("signature key");
        let ev_idx = s.find("sbo3l_audit_event_id").expect("audit_event_id key");
        let cap_idx = s
            .find("sbo3l_passport_capsule_hash")
            .expect("capsule_hash key");
        assert!(
            r_idx < p_idx && p_idx < sig_idx && sig_idx < ev_idx && ev_idx < cap_idx,
            "field order violated; got serialised body: {s}"
        );
    }

    #[test]
    fn envelope_audit_event_id_matches_crockford_pattern() {
        // The IP-3 sister-tool docs pin `^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$`
        // — Crockford base32 ULID, no I/L/O/U. Our fixture uses a real
        // ULID so this check is meaningful: a future refactor that
        // accidentally lowercases the id (or strips the `evt-` prefix)
        // breaks here.
        let r = fixture_receipt();
        let env = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id);
        let id = &env.sbo3l_audit_event_id;
        assert!(id.starts_with("evt-"), "got: {id}");
        let body = &id["evt-".len()..];
        assert_eq!(body.len(), 26, "ULID body must be 26 chars; got: {body}");
        let mut cs = body.chars();
        let first = cs.next().expect("non-empty");
        assert!(
            ('0'..='7').contains(&first),
            "first char must be 0-7 (ULID timestamp upper bits); got: {first}"
        );
        const CROCKFORD_TAIL: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        for c in cs {
            assert!(
                CROCKFORD_TAIL.contains(c),
                "tail char {c:?} not in Crockford alphabet (no I/L/O/U)"
            );
        }
    }

    #[test]
    fn envelope_capsule_hash_omitted_when_none_via_skip_serializing() {
        let r = fixture_receipt();
        let env = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id);
        let payload = env.to_json_payload();
        // Parse the canonical wire-form String into a Value purely for
        // structural inspection. The Map sorts keys alphabetically here,
        // but that's fine — this test asserts on the *field set*, not
        // the order (`to_json_payload_preserves_documented_field_order`
        // covers ordering).
        let v: serde_json::Value = serde_json::from_str(&payload).expect("parse payload");
        let obj = v.as_object().expect("object");
        assert!(
            !obj.contains_key("sbo3l_passport_capsule_hash"),
            "skip_serializing_if didn't omit the absent capsule hash; obj: {obj:?}"
        );
        // Only the four mandatory fields appear on the wire when the
        // capsule isn't yet known.
        assert_eq!(obj.len(), 4, "expected 4 fields, got: {obj:?}");
    }

    #[test]
    fn envelope_capsule_hash_present_when_with_passport_capsule_called() {
        let r = fixture_receipt();
        let capsule_hash = "d".repeat(64);
        let env = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id)
            .with_passport_capsule(capsule_hash.clone());
        assert_eq!(env.sbo3l_passport_capsule_hash, Some(capsule_hash.clone()));
        // Round-trip via to_json_payload to be sure the field is on
        // the wire when populated.
        let payload = env.to_json_payload();
        let v: serde_json::Value = serde_json::from_str(&payload).expect("parse payload");
        assert_eq!(
            v.get("sbo3l_passport_capsule_hash")
                .and_then(|v| v.as_str()),
            Some(capsule_hash.as_str())
        );
    }

    #[test]
    fn envelope_to_json_payload_round_trips_via_serde() {
        // `to_json_payload` is documented as the canonical wire-form
        // helper. A consumer should be able to deserialise it back
        // into a Sbo3lEnvelope without losing fields — including the
        // optional capsule hash. This guards against a silent
        // drift between `Serialize` and `Deserialize`.
        let r = fixture_receipt();
        let original = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id)
            .with_passport_capsule("9".repeat(64));
        let payload = original.to_json_payload();
        let round_tripped: Sbo3lEnvelope =
            serde_json::from_str(&payload).expect("round-trip deserialise");
        assert_eq!(round_tripped, original);
    }

    /// Codex P1 on PR #51 — pins the `to_json_payload` byte order
    /// directly. The previous `envelope_serialises_with_documented_field_order`
    /// test serialises the *struct* via `to_string(&env)` (which honours
    /// declaration order via `derive(Serialize)`) and so missed a
    /// regression in `to_json_payload` itself. Asserting against the
    /// payload String now catches a future implementation that round-
    /// trips through `serde_json::Value` (which would alphabetically
    /// reorder keys under the workspace's no-`preserve_order` setup).
    #[test]
    fn to_json_payload_preserves_documented_field_order() {
        let r = fixture_receipt();
        let env = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id)
            .with_passport_capsule("c".repeat(64));
        let payload = env.to_json_payload();
        // Verify byte order of the field keys.
        let order = [
            "sbo3l_request_hash",
            "sbo3l_policy_hash",
            "sbo3l_receipt_signature",
            "sbo3l_audit_event_id",
            "sbo3l_passport_capsule_hash",
        ];
        let mut cursor = 0usize;
        for key in order {
            let needle = format!(r#""{key}":"#);
            let pos = payload
                .find(&needle)
                .unwrap_or_else(|| panic!("missing key {key} in payload: {payload}"));
            assert!(
                pos >= cursor,
                "field {key} appears out of documented order in payload: {payload}"
            );
            cursor = pos;
        }
    }
}
