//! Side-by-side BEFORE / AFTER demo of the IP-1 envelope on a KeeperHub
//! workflow-webhook submission.
//!
//! Run with: `cargo run --example before_after_envelope -p sbo3l-keeperhub-adapter`
//!
//! Output is fully deterministic given a fixed `PolicyReceipt`: same input
//! produces byte-identical stdout. The shell wrapper
//! `demo-scripts/sponsors/keeperhub-before-after.sh` calls this example
//! and pipes the output through `sed` for terminal styling, but the
//! evidence (the JSON blocks themselves) is produced here.
//!
//! This file imports ONLY `sbo3l_keeperhub_adapter` and `sbo3l_core` —
//! by design, so a third-party agent framework can use the same
//! `build_envelope` to attach IP-1 fields to *their own* KeeperHub
//! webhook submissions without pulling the rest of the SBO3L workspace.

use sbo3l_core::receipt::{
    Decision, EmbeddedSignature, PolicyReceipt, ReceiptType, SignatureAlgorithm,
};
use sbo3l_keeperhub_adapter::build_envelope;

fn main() {
    // 1. Construct a deterministic signed PolicyReceipt — the same shape
    //    `mandate_server::router` returns after the policy + budget +
    //    nonce-replay + audit pipeline has run. Hard-coded hex pads
    //    (`a..a`, `b..b`, `f..f`) keep the BEFORE/AFTER output
    //    byte-identical across runs so the demo video doesn't drift.
    let receipt = PolicyReceipt {
        receipt_type: ReceiptType::PolicyReceiptV1,
        version: 1,
        agent_id: "research-agent.team.eth".into(),
        decision: Decision::Allow,
        deny_code: None,
        request_hash: "a".repeat(64),
        policy_hash: "b".repeat(64),
        policy_version: Some(1),
        audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".into(),
        execution_ref: None,
        // Fixed timestamp so the demo transcript is reproducible.
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:00Z")
            .expect("fixed RFC3339")
            .with_timezone(&chrono::Utc),
        expires_at: None,
        signature: EmbeddedSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            key_id: "decision-signer-v1".into(),
            signature_hex: "f".repeat(128),
        },
    };

    // 2. Synthesise the raw KeeperHub workflow-webhook submission body
    //    that an agent WITHOUT SBO3L would post. This is documented
    //    target shape per the KeeperHub team's hackathon office hours
    //    — there is no public schema yet, so this is illustrative.
    let raw_kh_body = serde_json::json!({
        "workflow_id":        "wf-x402-api-call",
        "agent_id":           "research-agent.team.eth",
        "intent":             "purchase_api_call",
        "amount":             { "value": "0.05", "currency": "USD" },
        "destination": {
            "type":   "x402_endpoint",
            "url":    "https://api.example.com/v1/inference",
            "method": "POST",
        },
    });

    // 3. Build the IP-1 envelope using the shared helper. SBO3L-side
    //    auditors (and KeeperHub if they choose to echo the fields back)
    //    can verify every `sbo3l_*` value against the same APRP body
    //    and signed receipt the agent posted, *without trusting either
    //    side*.
    let envelope = build_envelope(&receipt);

    // 4. Compose the AFTER body: the original KH submit body with the
    //    `sbo3l_*` fields appended. Note: the AFTER body intentionally
    //    keeps the original fields verbatim — KeeperHub's existing
    //    parser stays unchanged; the envelope is additive.
    let mut after_body = raw_kh_body.as_object().expect("object").clone();
    let env_obj: serde_json::Value =
        serde_json::from_str(&envelope.to_json_payload()).expect("envelope round-trip");
    if let Some(env_map) = env_obj.as_object() {
        for (k, v) in env_map {
            after_body.insert(k.clone(), v.clone());
        }
    }

    // Format both blocks as pretty JSON for demo readability. The
    // wire form sent by `KeeperHubExecutor::live()` would be the same
    // bytes in compact form (see `Sbo3lEnvelope::to_json_payload`).
    let before_json =
        serde_json::to_string_pretty(&raw_kh_body).expect("BEFORE pretty");
    let after_json = serde_json::to_string_pretty(&serde_json::Value::Object(after_body))
        .expect("AFTER pretty");

    println!("== BEFORE SBO3L — raw KeeperHub workflow-webhook submission ==");
    println!("{before_json}");
    println!();
    println!("== AFTER SBO3L — same workflow with IP-1 envelope attached ==");
    println!("{after_json}");
    println!();
    println!("== Why this matters ==");
    println!(
        "An auditor reading the AFTER body can re-derive sbo3l_request_hash from the\n\
         original APRP, re-derive sbo3l_policy_hash from the active policy snapshot,\n\
         re-verify sbo3l_receipt_signature against the agent's published Ed25519\n\
         pubkey, and walk the audit chain from sbo3l_audit_event_id back to genesis\n\
         — without trusting KeeperHub or the agent. The BEFORE body offers no such\n\
         offline-verifiable path: the auditor must trust the agent's prose claim\n\
         that this submission was authorised.\n\
         \n\
         IP-1 (envelope), IP-3 (sbo3l.audit_lookup MCP tool — symmetric to\n\
         keeperhub.lookup_execution), and IP-4 (this crate, `cargo add\n\
         sbo3l-keeperhub-adapter`) are catalogued in\n\
         docs/keeperhub-integration-paths.md."
    );
}
