//! SBO3L adapter for KeeperHub workflow execution.
//!
//! `SBO3L decides, KeeperHub executes.` This crate wraps a single
//! `KeeperHubExecutor` that gates execution on a SBO3L-signed
//! `PolicyReceipt` and (when live) carries the IP-1 `sbo3l_*`
//! upstream-proof envelope to KeeperHub's workflow webhook. The whole
//! crate has **one** workspace-internal dependency
//! ([`sbo3l_core`]) — by design, so a third-party agent framework
//! can depend on `sbo3l-keeperhub-adapter` and pull in only the
//! SBO3L types they need, not the policy engine, server, storage,
//! or CLI.
//!
//! ## Quickstart
//!
//! ```no_run
//! use sbo3l_keeperhub_adapter::{KeeperHubExecutor, GuardedExecutor};
//! # use sbo3l_core::receipt::PolicyReceipt;
//! # use sbo3l_core::aprp::PaymentRequest;
//! # let request: PaymentRequest = unimplemented!();
//! # let receipt: PolicyReceipt = unimplemented!();
//! let executor = KeeperHubExecutor::local_mock();
//! let result = executor.execute(&request, &receipt);
//! ```
//!
//! Live mode (`KeeperHubExecutor::live()`) posts the IP-1 envelope
//! to the webhook URL in `SBO3L_KEEPERHUB_WEBHOOK_URL` via
//! `reqwest::blocking` (5-s timeout). When the env var is unset the
//! executor returns `ExecutionError::BackendOffline`; any network /
//! non-2xx / parse failure surfaces as
//! `ExecutionError::ProtocolError`. See [`submit_live`] for the
//! full contract.
//!
//! ## What this crate is *not*
//!
//! - **Not a live KeeperHub client.** Live submission is gated; see
//!   [`docs/keeperhub-live-spike.md`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/keeperhub-live-spike.md).
//! - **Not a policy engine.** Policy decisions happen upstream
//!   (`sbo3l-policy`); this crate consumes the signed `PolicyReceipt`
//!   and refuses to execute anything that isn't `Decision::Allow`.
//! - **Not a daemon / server.** No HTTP server, no SQLite, no MCP.
//!   For those, take the corresponding workspace crate
//!   (`sbo3l-server`, `sbo3l-mcp`).
//!
//! See [`docs/keeperhub-integration-paths.md`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md)
//! for the full IP-1..IP-5 catalogue this crate is the IP-4 realisation
//! of.

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::execution::GuardedExecutor as CoreGuardedExecutor;
use sbo3l_core::receipt::{Decision, PolicyReceipt};

// ---------------------------------------------------------------------------
// Re-exports — third-party callers should use these, not direct
// `sbo3l_core::execution::*` paths, so the adapter crate can evolve
// the shape (e.g. wrap with deprecation shims) without breaking them.
// ---------------------------------------------------------------------------

pub use sbo3l_core::execution::{
    ExecutionError, ExecutionError as Error, ExecutionReceipt, ExecutionReceipt as Receipt,
    GuardedExecutor, Sbo3lEnvelope,
};

// ---------------------------------------------------------------------------
// KeeperHub executor
// ---------------------------------------------------------------------------

/// Two execution modes:
///
/// - [`KeeperHubMode::Live`] — POSTs the IP-1 envelope to the URL in
///   `SBO3L_KEEPERHUB_WEBHOOK_URL` via `reqwest::blocking` (5-s
///   timeout), parses `executionId` (or `id` fallback) from the 2xx
///   response, and returns an `ExecutionReceipt` with `mock: false`
///   plus the envelope as `evidence`. Unset env var →
///   [`ExecutionError::BackendOffline`]; network / non-2xx /
///   parse failure → [`ExecutionError::ProtocolError`].
/// - [`KeeperHubMode::LocalMock`] — returns a deterministic
///   `ExecutionReceipt` with a fresh ULID `execution_ref` and
///   `mock: true`. Demos disclose this clearly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeeperHubMode {
    Live,
    LocalMock,
}

/// The adapter itself. Construct via [`KeeperHubExecutor::local_mock`]
/// or [`KeeperHubExecutor::live`]; the runtime mode is read from
/// `self.mode` inside [`GuardedExecutor::execute`].
#[derive(Debug, Clone)]
pub struct KeeperHubExecutor {
    pub mode: KeeperHubMode,
}

impl KeeperHubExecutor {
    /// Construct a deterministic local mock executor. Returns a fresh
    /// `kh-<ULID>` `execution_ref` on every successful execute, with
    /// `mock: true` so demo output can disclose the mock state.
    pub fn local_mock() -> Self {
        Self {
            mode: KeeperHubMode::LocalMock,
        }
    }

    /// Construct a live-mode executor. `execute()` POSTs the IP-1
    /// envelope to the URL in `SBO3L_KEEPERHUB_WEBHOOK_URL`; see
    /// [`submit_live`] for the full contract.
    pub fn live() -> Self {
        Self {
            mode: KeeperHubMode::Live,
        }
    }
}

impl CoreGuardedExecutor for KeeperHubExecutor {
    fn sponsor_id(&self) -> &'static str {
        "keeperhub"
    }

    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        // Hard truthfulness rule: a denied (or requires_human) receipt
        // never reaches the sponsor backend. The check happens BEFORE
        // any I/O so a future addition (HTTP submission, file write,
        // metrics emit) can't accidentally fire on a non-allow path.
        if !matches!(receipt.decision, Decision::Allow) {
            return Err(ExecutionError::NotApproved(receipt.decision.clone()));
        }
        match self.mode {
            KeeperHubMode::LocalMock => Ok(ExecutionReceipt {
                sponsor: "keeperhub",
                execution_ref: format!("kh-{}", ulid::Ulid::new()),
                mock: true,
                note: format!(
                    "local mock: would route {agent}/{intent} via KeeperHub MCP",
                    agent = request.agent_id,
                    intent = serde_json::to_string(&request.intent).unwrap_or_default(),
                ),
                // KeeperHub mock doesn't capture sponsor-specific evidence
                // today. When KH IP-1 lands as live wire form, that
                // envelope flows into `ExecutionReceipt.evidence` and
                // surfaces in the capsule's NEW
                // `execution.executor_evidence` slot — the same slot
                // Uniswap populates today with `UniswapQuoteEvidence`
                // (P6.1). It is NOT the `live_evidence` slot: that slot
                // is transport-level (HTTP transport, response ref,
                // block ref) and the verifier's bidirectional invariant
                // keeps it strictly live-only. See
                // `docs/keeperhub-live-spike.md`.
                evidence: None,
            }),
            KeeperHubMode::Live => submit_live(request, receipt),
        }
    }
}

/// Build the IP-1 envelope (see
/// `docs/keeperhub-integration-paths.md` §IP-1) that a live KeeperHub
/// submission carries alongside the APRP body and signed
/// `PolicyReceipt`.
///
/// Exposed at module level so tests can pin the wire shape without
/// poking through the executor's submission path.
pub fn build_envelope(receipt: &PolicyReceipt) -> Sbo3lEnvelope {
    Sbo3lEnvelope::from_receipt(receipt, &receipt.audit_event_id)
}

/// Env var that holds the live KeeperHub workflow-webhook URL. Per
/// Daniel's office-hours intel, KeeperHub's webhook is per-workflow:
/// `https://app.keeperhub.com/api/workflows/{workflowId}/webhook`.
/// The operator supplies the full URL with the `{workflowId}` baked
/// in; `submit_live` does not assemble it.
pub const LIVE_WEBHOOK_ENV: &str = "SBO3L_KEEPERHUB_WEBHOOK_URL";

/// Env var holding the KeeperHub workflow-webhook bearer token.
/// MUST start with `wfb_` (workflow-webhook prefix per KeeperHub's
/// token-naming convention). The `kh_` prefix is for the platform
/// REST API / MCP — submitting to the workflow webhook with a `kh_`
/// token will return a sponsor-side auth error, so we reject up
/// front with a clear `ProtocolError` rather than burning a
/// round-trip on a known-bad shape.
pub const LIVE_TOKEN_ENV: &str = "SBO3L_KEEPERHUB_TOKEN";

/// A8 — execute one live submission against KeeperHub's workflow webhook.
///
/// Wire shape: POSTs `{ agent_id, intent, sbo3l_*: ... }`
/// (a thin APRP echo + the IP-1 envelope from [`build_envelope`])
/// as JSON, with `Authorization: Bearer ${SBO3L_KEEPERHUB_TOKEN}`.
/// Reads the URL + token from [`LIVE_WEBHOOK_ENV`] / [`LIVE_TOKEN_ENV`].
///
/// Returns:
/// - `Ok(ExecutionReceipt { mock: false, … })` on 2xx + JSON body
///   carrying `executionId` (KeeperHub's documented field) or `id`
///   (common fallback). `execution_ref` carries `kh-<id>`.
/// - `Err(ExecutionError::BackendOffline(…))` when
///   `SBO3L_KEEPERHUB_WEBHOOK_URL` is unset — operator hasn't wired
///   live mode at all. Distinguishable from `ProtocolError` ("wired
///   but the round-trip itself failed").
/// - `Err(ExecutionError::ProtocolError(…))` for every other failure:
///   token unset, token wrong-prefix (`kh_` instead of `wfb_`),
///   network error, timeout, non-2xx, body-parse failure, missing
///   `executionId`/`id`. Error message carries diagnostic context
///   without leaking secrets (token never logged).
fn submit_live(
    request: &PaymentRequest,
    receipt: &PolicyReceipt,
) -> Result<ExecutionReceipt, ExecutionError> {
    let webhook_url = std::env::var(LIVE_WEBHOOK_ENV).map_err(|_| {
        ExecutionError::BackendOffline(format!(
            "live KeeperHub backend not configured: set {LIVE_WEBHOOK_ENV} or \
             switch to KeeperHubExecutor::local_mock()"
        ))
    })?;
    let token = std::env::var(LIVE_TOKEN_ENV).map_err(|_| {
        ExecutionError::ProtocolError(format!(
            "live KeeperHub bearer token not set: supply {LIVE_TOKEN_ENV} (must \
             start with `wfb_`)"
        ))
    })?;
    submit_live_to(request, receipt, &webhook_url, &token)
}

/// Inner submission helper exposed for tests so a `mockito::Server`'s
/// URL + a deterministic token can be passed in without mutating the
/// process-global env vars. Production callers go through
/// [`submit_live`] which reads `SBO3L_KEEPERHUB_WEBHOOK_URL` +
/// `SBO3L_KEEPERHUB_TOKEN`.
///
/// Token-prefix rule: KeeperHub uses `wfb_` for workflow-webhook
/// tokens and `kh_` for the platform REST API / MCP. Submitting to a
/// workflow webhook with a `kh_` token is a known-bad shape — this
/// helper rejects up front with a `ProtocolError` rather than burning
/// a round-trip on a request the sponsor will refuse.
pub(crate) fn submit_live_to(
    request: &PaymentRequest,
    receipt: &PolicyReceipt,
    webhook_url: &str,
    token: &str,
) -> Result<ExecutionReceipt, ExecutionError> {
    if token.starts_with("kh_") {
        return Err(ExecutionError::ProtocolError(
            "wrong-token-prefix; webhook submissions require wfb_ tokens (got kh_)".to_string(),
        ));
    }
    if !token.starts_with("wfb_") {
        return Err(ExecutionError::ProtocolError(format!(
            "wrong-token-prefix; webhook submissions require wfb_ tokens (got {} prefix)",
            token.chars().take(4).collect::<String>()
        )));
    }
    let envelope = build_envelope(receipt);
    // Compose the wire body: thin APRP echo (agent_id + intent) plus
    // the IP-1 envelope. KeeperHub's documented IP-1 shape adds the
    // sbo3l_* fields alongside the workflow body the agent already
    // posted; we assemble a minimal-but-shaped body here so tests pin
    // the exact JSON the live submission produces.
    let env_value: serde_json::Value =
        serde_json::from_str(&envelope.to_json_payload()).map_err(|e| {
            ExecutionError::ProtocolError(format!("could not re-parse own envelope: {e}"))
        })?;
    // `to_value` is `T: Serialize` over an owned value. `Intent` is `Copy`,
    // so `request.intent` (without `&` or `.clone()`) just copies the field
    // out of the borrowed `&PaymentRequest`. Clippy on Rust 1.95 rejects
    // both `&request.intent` (`needless_borrows_for_generic_args`) and
    // `request.intent.clone()` (`clone_on_copy`); the bare field access
    // is the only form that passes `-D warnings`.
    let intent_value = serde_json::to_value(request.intent).unwrap_or(serde_json::Value::Null);
    let mut body = serde_json::Map::new();
    body.insert(
        "agent_id".into(),
        serde_json::Value::String(request.agent_id.clone()),
    );
    body.insert("intent".into(), intent_value);
    if let Some(env_obj) = env_value.as_object() {
        for (k, v) in env_obj {
            body.insert(k.clone(), v.clone());
        }
    }
    let body = serde_json::Value::Object(body);

    // 5-second hard timeout — long enough for transient slowness, short
    // enough that an unresponsive backend doesn't block the executor.
    // Operators who need a longer ceiling can wrap `submit_live` with
    // their own retry/timeout policy upstream.
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| ExecutionError::ProtocolError(format!("HTTP client init failed: {e}")))?;
    let resp = client
        .post(webhook_url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .map_err(|e| ExecutionError::ProtocolError(format!("HTTP send failed: {e}")))?;

    let status = resp.status();
    let resp_body = resp.text().map_err(|e| {
        ExecutionError::ProtocolError(format!("HTTP {status}: read body failed: {e}"))
    })?;
    if !status.is_success() {
        // Truncate body in error message — sponsor responses can be
        // large or contain sensitive data; first 200 bytes is enough
        // for diagnosis.
        let snippet: String = resp_body.chars().take(200).collect();
        return Err(ExecutionError::ProtocolError(format!(
            "HTTP {status}: {snippet}"
        )));
    }
    let parsed: serde_json::Value = serde_json::from_str(&resp_body).map_err(|e| {
        ExecutionError::ProtocolError(format!(
            "2xx body did not parse as JSON: {e} (body prefix: {})",
            resp_body.chars().take(100).collect::<String>()
        ))
    })?;
    let execution_id = parsed
        .get("executionId")
        .or_else(|| parsed.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ExecutionError::ProtocolError(format!(
                "2xx body missing `executionId` or `id` field: {}",
                resp_body.chars().take(200).collect::<String>()
            ))
        })?;

    Ok(ExecutionReceipt {
        sponsor: "keeperhub",
        execution_ref: format!("kh-{execution_id}"),
        mock: false,
        note: format!(
            "live: submitted to {host} via IP-1 envelope; received executionId={execution_id}",
            host = url_host_for_note(webhook_url),
        ),
        // The IP-1 envelope is the executor-side evidence today.
        // Surfaced into the capsule's `execution.executor_evidence`
        // slot via `ExecutionReceipt.evidence`.
        evidence: Some(env_value),
    })
}

/// Extract a host string from a webhook URL for the `ExecutionReceipt.note`
/// field. Returns the URL verbatim if parsing fails — better to leak the
/// raw URL into a note (which is operator-side) than to swallow it.
fn url_host_for_note(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sbo3l_core::receipt::{EmbeddedSignature, ReceiptType, SignatureAlgorithm};

    fn aprp() -> PaymentRequest {
        let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        serde_json::from_value(v).unwrap()
    }

    fn receipt(decision: Decision) -> PolicyReceipt {
        PolicyReceipt {
            receipt_type: ReceiptType::PolicyReceiptV1,
            version: 1,
            agent_id: "research-agent-01".to_string(),
            decision,
            deny_code: None,
            request_hash: "1".repeat(64),
            policy_hash: "2".repeat(64),
            policy_version: Some(1),
            audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            execution_ref: None,
            issued_at: chrono::Utc::now(),
            expires_at: None,
            signature: EmbeddedSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: "test".to_string(),
                signature_hex: "0".repeat(128),
            },
        }
    }

    #[test]
    fn approved_receipt_routes_to_keeperhub_mock() {
        let exec = KeeperHubExecutor::local_mock();
        let r = exec.execute(&aprp(), &receipt(Decision::Allow)).unwrap();
        assert_eq!(r.sponsor, "keeperhub");
        assert!(r.mock);
        assert!(r.execution_ref.starts_with("kh-"));
    }

    #[test]
    fn denied_receipt_never_reaches_keeperhub() {
        let exec = KeeperHubExecutor::local_mock();
        let err = exec.execute(&aprp(), &receipt(Decision::Deny)).unwrap_err();
        assert!(matches!(err, ExecutionError::NotApproved(_)));
    }

    /// A8 — `submit_live_to` against an unroutable URL surfaces as
    /// `ProtocolError`. This pins the network-failure path without
    /// mutating the process-global `SBO3L_KEEPERHUB_WEBHOOK_URL` env
    /// var (which would race with parallel tests and re-trip the
    /// Codex P2 flagged on PR #55). The env-var-unset path itself
    /// is one obvious line in `submit_live`; mockito covers the
    /// reachable paths.
    #[test]
    fn live_returns_protocol_error_for_unreachable_url() {
        // Port 1 / TCP is reserved (RFC 5735) and never listens — fast
        // connection-refused on every loopback-capable platform.
        let unreachable = "http://127.0.0.1:1/sbo3l-test-no-listener";
        let err = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            unreachable,
            "wfb_test_token",
        )
        .unwrap_err();
        assert!(
            matches!(err, ExecutionError::ProtocolError(_)),
            "expected ProtocolError, got: {err:?}"
        );
    }

    /// A8 — happy path. Mockito stands up a local HTTP server, returns
    /// 200 + a JSON body with `executionId`, and we assert the live
    /// receipt carries `mock: false` and the parsed id prefixed with
    /// `kh-`.
    #[test]
    #[allow(non_snake_case)]
    fn live_happy_path_parses_executionId_field() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"executionId":"wf-abc123","status":"submitted"}"#)
            .create();

        let r = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            &server.url(),
            "wfb_test_token",
        )
        .expect("happy path must succeed");
        assert_eq!(r.sponsor, "keeperhub");
        assert!(!r.mock, "live receipt must NOT carry mock=true");
        assert_eq!(r.execution_ref, "kh-wf-abc123");
        // Evidence slot carries the IP-1 envelope so an auditor reading
        // the capsule can re-verify the wire body offline.
        let env = r.evidence.as_ref().expect("live evidence present");
        assert!(env.get("sbo3l_request_hash").is_some());
        assert!(env.get("sbo3l_audit_event_id").is_some());
    }

    /// A8 — fallback `id` field. Some sponsor backends return `id` not
    /// `executionId`; the brief said to accept either. Pin that.
    #[test]
    fn live_happy_path_parses_id_field_fallback() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"wf-fallback-7","status":"submitted"}"#)
            .create();

        let r = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            &server.url(),
            "wfb_test_token",
        )
        .unwrap();
        assert_eq!(r.execution_ref, "kh-wf-fallback-7");
    }

    /// A8 — non-2xx response. KeeperHub-side rejection (4xx) or
    /// internal error (5xx) surfaces as `ProtocolError` with the
    /// status code in the message.
    #[test]
    fn live_returns_protocol_error_on_non_2xx() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("POST", "/")
            .with_status(503)
            .with_body("KeeperHub: workflow runner unavailable")
            .create();

        let err = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            &server.url(),
            "wfb_test_token",
        )
        .unwrap_err();
        match err {
            ExecutionError::ProtocolError(msg) => {
                assert!(
                    msg.contains("503"),
                    "ProtocolError message must include status: {msg}"
                );
            }
            other => panic!("expected ProtocolError, got: {other:?}"),
        }
    }

    /// A8 — 200 status but unparseable / id-less body. Surfaces as
    /// `ProtocolError` so the operator sees the contract violation
    /// rather than a silent success that downstream auditors can't
    /// re-verify.
    #[test]
    fn live_returns_protocol_error_on_unparseable_body() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"submitted","other":"field"}"#)
            .create();

        let err = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            &server.url(),
            "wfb_test_token",
        )
        .unwrap_err();
        match err {
            ExecutionError::ProtocolError(msg) => {
                assert!(
                    msg.contains("executionId") || msg.contains("id"),
                    "diagnostic must mention the missing id field: {msg}"
                );
            }
            other => panic!("expected ProtocolError, got: {other:?}"),
        }
    }

    /// A8 — wrong-prefix `kh_` token (platform REST API / MCP token,
    /// not workflow-webhook) must be rejected up front. KeeperHub's
    /// webhook will refuse it anyway; we surface the local diagnostic
    /// rather than burn a round-trip on a known-bad shape.
    #[test]
    fn live_rejects_kh_prefix_token_without_network_call() {
        // Use an unreachable URL — we expect the prefix check to fire
        // BEFORE any network attempt; if it didn't, the test would also
        // fail (timeout / connection-refused) but with a different
        // ProtocolError message. Asserting on the message text
        // distinguishes "rejected by prefix check" from "network
        // failed".
        let unreachable = "http://127.0.0.1:1/sbo3l-test-no-listener";
        let err = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            unreachable,
            "kh_platform_token_abc",
        )
        .unwrap_err();
        match err {
            ExecutionError::ProtocolError(msg) => {
                assert!(
                    msg.contains("wrong-token-prefix"),
                    "must surface wrong-token-prefix diagnostic, got: {msg}"
                );
                assert!(
                    msg.contains("wfb_"),
                    "must mention required wfb_ prefix, got: {msg}"
                );
            }
            other => panic!("expected ProtocolError, got: {other:?}"),
        }
    }

    /// A8 — token with neither `wfb_` nor `kh_` prefix is also
    /// rejected up front. Catches typos / stale tokens / placeholder
    /// values like "TODO" before they hit the wire.
    #[test]
    fn live_rejects_bare_token_without_known_prefix() {
        let unreachable = "http://127.0.0.1:1/sbo3l-test-no-listener";
        let err = submit_live_to(
            &aprp(),
            &receipt(Decision::Allow),
            unreachable,
            "TODO_set_real_token",
        )
        .unwrap_err();
        match err {
            ExecutionError::ProtocolError(msg) => {
                assert!(
                    msg.contains("wrong-token-prefix"),
                    "must surface wrong-token-prefix diagnostic, got: {msg}"
                );
            }
            other => panic!("expected ProtocolError, got: {other:?}"),
        }
    }

    #[test]
    fn keeperhub_live_constructs_envelope_via_from_receipt() {
        let r = receipt(Decision::Allow);
        let env = build_envelope(&r);
        assert_eq!(env.sbo3l_request_hash, r.request_hash);
        assert_eq!(env.sbo3l_policy_hash, r.policy_hash);
        assert_eq!(env.sbo3l_receipt_signature, r.signature.signature_hex);
        assert_eq!(env.sbo3l_audit_event_id, r.audit_event_id);
        assert!(env.sbo3l_passport_capsule_hash.is_none());
    }

    /// Surface coverage: the public re-exports remain wired to
    /// `sbo3l_core::execution::*`. A future bump of sbo3l-core that
    /// renames or relocates these types breaks here, before any
    /// downstream consumer sees the breakage.
    #[test]
    fn public_reexports_are_wired() {
        // The aliased re-exports.
        let _: Error = ExecutionError::Integration("smoke".into());
        let _: Receipt = ExecutionReceipt {
            sponsor: "smoke",
            execution_ref: "kh-smoke".into(),
            mock: true,
            note: "smoke".into(),
            evidence: None,
        };
        let r = receipt(Decision::Allow);
        let _: Sbo3lEnvelope = Sbo3lEnvelope::from_receipt(&r, &r.audit_event_id);
        // The trait re-export — implemented for our own executor.
        let exec: Box<dyn GuardedExecutor> = Box::new(KeeperHubExecutor::local_mock());
        assert_eq!(exec.sponsor_id(), "keeperhub");
    }

    /// The crate's surface must NOT pull `sbo3l_execution` /
    /// `sbo3l_server` / `sbo3l_storage` into a third-party caller's
    /// dependency closure. This test compiles against
    /// `sbo3l_core::execution::*` only — if a future change accidentally
    /// adds a workspace-internal type to a public signature, the test
    /// won't compile because the upstream crate isn't listed in
    /// `[dependencies]`.
    #[test]
    fn surface_is_sbo3l_core_only() {
        fn _accepts_only_core_types(env: &Sbo3lEnvelope, exec: &KeeperHubExecutor) {
            let _ = env.sbo3l_request_hash.len();
            let _ = exec.sponsor_id();
        }
    }
}
