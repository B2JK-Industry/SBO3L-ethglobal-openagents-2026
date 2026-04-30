//! Standalone example: build a signed `PolicyReceipt`, hand it to the
//! KeeperHub executor, observe the IP-1 envelope and the
//! `BackendOffline` live-mode response.
//!
//! Run with: `cargo run --example submit_signed_receipt -p sbo3l-keeperhub-adapter`
//!
//! This example imports ONLY `sbo3l_keeperhub_adapter` and
//! `sbo3l_core` — it doesn't touch `sbo3l-server`, `sbo3l-policy`,
//! `sbo3l-storage`, `sbo3l-cli`, or `sbo3l-mcp`. That's the
//! IP-4 promise: a third-party agent framework can do the same.

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::receipt::{
    Decision, EmbeddedSignature, PolicyReceipt, ReceiptType, SignatureAlgorithm,
};
use sbo3l_keeperhub_adapter::{build_envelope, GuardedExecutor, KeeperHubExecutor};

fn main() {
    // 1. Build a request body. In a real agent, this is the APRP the
    //    agent sent through SBO3L's HTTP / MCP surface; here we
    //    construct a placeholder that satisfies the schema enough to
    //    flow through the executor.
    let aprp_json = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
    let request: PaymentRequest =
        serde_json::from_str(aprp_json).expect("APRP fixture is schema-valid JSON");

    // 2. Construct a signed PolicyReceipt. In production, this comes
    //    back from `sbo3l_server::router` after the policy +
    //    budget + nonce + audit pipeline has run. For an offline
    //    example we hand-build one that the executor will accept.
    let receipt = PolicyReceipt {
        receipt_type: ReceiptType::PolicyReceiptV1,
        version: 1,
        agent_id: "research-agent-01".into(),
        decision: Decision::Allow,
        deny_code: None,
        request_hash: "a".repeat(64),
        policy_hash: "b".repeat(64),
        policy_version: Some(1),
        audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".into(),
        execution_ref: None,
        issued_at: chrono::Utc::now(),
        expires_at: None,
        signature: EmbeddedSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            key_id: "decision-signer-v1".into(),
            signature_hex: "f".repeat(128),
        },
    };

    // 3. Inspect the IP-1 envelope that the live arm would carry. This
    //    is the wire payload future KeeperHub workflow webhooks
    //    receive alongside the APRP body and signed receipt.
    let envelope = build_envelope(&receipt);
    println!("== IP-1 envelope (canonical wire form) ==");
    println!("{}", envelope.to_json_payload());

    // 4. Mock execution — succeeds with a fresh kh-<ULID> ref.
    let mock = KeeperHubExecutor::local_mock();
    let mock_receipt = mock
        .execute(&request, &receipt)
        .expect("local_mock always succeeds on Decision::Allow");
    println!("\n== mock execute result ==");
    println!(
        "sponsor={} mock={} execution_ref={}",
        mock_receipt.sponsor, mock_receipt.mock, mock_receipt.execution_ref
    );

    // 5. Live execution — POSTs the IP-1 envelope to a real KeeperHub
    //    workflow webhook. Activated when both env vars are set:
    //
    //        SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/<id>/webhook
    //        SBO3L_KEEPERHUB_TOKEN=wfb_<token>
    //
    //    With both set, this prints the captured `executionId`. With either
    //    unset, prints a `Configuration` error explaining the missing env var.
    //    See `crates/sbo3l-keeperhub-adapter/src/lib.rs::submit_live_to` for
    //    the wire-format details. Verified end-to-end against a real
    //    KeeperHub workflow during the ETHGlobal Open Agents 2026 submission.
    let live = KeeperHubExecutor::live();
    match live.execute(&request, &receipt) {
        Ok(r) => println!(
            "\n== live execute ==\nsponsor={} mock={} execution_ref={}",
            r.sponsor, r.mock, r.execution_ref
        ),
        Err(e) => println!("\n== live execute (gated, env vars not set) ==\n{e}"),
    }
}
