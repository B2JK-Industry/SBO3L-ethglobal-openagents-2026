//! Mandate adapter for KeeperHub workflow execution.
//!
//! `Mandate decides, KeeperHub executes.` This crate wraps a single
//! `KeeperHubExecutor` that gates execution on a Mandate-signed
//! `PolicyReceipt` and (when live) carries the IP-1 `mandate_*`
//! upstream-proof envelope to KeeperHub's workflow webhook. The whole
//! crate has **one** workspace-internal dependency
//! ([`mandate_core`]) — by design, so a third-party agent framework
//! can depend on `mandate-keeperhub-adapter` and pull in only the
//! Mandate types they need, not the policy engine, server, storage,
//! or CLI.
//!
//! ## Quickstart
//!
//! ```no_run
//! use mandate_keeperhub_adapter::{KeeperHubExecutor, GuardedExecutor};
//! # use mandate_core::receipt::PolicyReceipt;
//! # use mandate_core::aprp::PaymentRequest;
//! # let request: PaymentRequest = unimplemented!();
//! # let receipt: PolicyReceipt = unimplemented!();
//! let executor = KeeperHubExecutor::local_mock();
//! let result = executor.execute(&request, &receipt);
//! ```
//!
//! Live mode (`KeeperHubExecutor::live()`) currently returns
//! `ExecutionError::BackendOffline` — KeeperHub credentials and
//! workflow-webhook submission land in a follow-up release. The IP-1
//! envelope IS still constructed inside the `Live` arm so the
//! wire-format invariant is exercised in CI before the live HTTP path
//! turns on.
//!
//! ## What this crate is *not*
//!
//! - **Not a live KeeperHub client.** Live submission is gated; see
//!   [`docs/keeperhub-live-spike.md`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-live-spike.md).
//! - **Not a policy engine.** Policy decisions happen upstream
//!   (`mandate-policy`); this crate consumes the signed `PolicyReceipt`
//!   and refuses to execute anything that isn't `Decision::Allow`.
//! - **Not a daemon / server.** No HTTP server, no SQLite, no MCP.
//!   For those, take the corresponding workspace crate
//!   (`mandate-server`, `mandate-mcp`).
//!
//! See [`docs/keeperhub-integration-paths.md`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md)
//! for the full IP-1..IP-5 catalogue this crate is the IP-4 realisation
//! of.

use mandate_core::aprp::PaymentRequest;
use mandate_core::execution::GuardedExecutor as CoreGuardedExecutor;
use mandate_core::receipt::{Decision, PolicyReceipt};

// ---------------------------------------------------------------------------
// Re-exports — third-party callers should use these, not direct
// `mandate_core::execution::*` paths, so the adapter crate can evolve
// the shape (e.g. wrap with deprecation shims) without breaking them.
// ---------------------------------------------------------------------------

pub use mandate_core::execution::{
    ExecutionError, ExecutionError as Error, ExecutionReceipt, ExecutionReceipt as Receipt,
    GuardedExecutor, MandateEnvelope,
};

// ---------------------------------------------------------------------------
// KeeperHub executor
// ---------------------------------------------------------------------------

/// Two execution modes:
///
/// - [`KeeperHubMode::Live`] — would call KeeperHub's workflow-webhook
///   endpoint. Today this returns
///   [`ExecutionError::BackendOffline`] because public KeeperHub
///   credentials are not wired into the hackathon build; the IP-1
///   envelope IS built and serialised in this arm so the wire-format
///   invariant has CI coverage before the live HTTP send turns on.
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

    /// Construct a live-mode executor. Today this returns
    /// `BackendOffline` from `execute()`; live submission lands with
    /// concrete credentials + `live_evidence` in a follow-up release.
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
            }),
            KeeperHubMode::Live => {
                // P5.1 contract: build AND serialise the envelope before
                // returning `BackendOffline`, so a future receipt-shape
                // change can't silently desync the wire format. The
                // payload is intentionally dropped via `let _ = …` —
                // explicit disclosure that we proved we *could* send it
                // without actually sending.
                let _envelope = build_envelope(receipt);
                let _payload_str = _envelope.to_json_payload();
                Err(ExecutionError::BackendOffline(
                    "live KeeperHub backend not configured for this hackathon build; \
                     switch to KeeperHubMode::LocalMock or wire credentials"
                        .to_string(),
                ))
            }
        }
    }
}

/// Build the IP-1 envelope (see
/// `docs/keeperhub-integration-paths.md` §IP-1) that a live KeeperHub
/// submission carries alongside the APRP body and signed
/// `PolicyReceipt`. Today the envelope is constructed but never sent
/// — the live arm of [`GuardedExecutor::execute`] always returns
/// `BackendOffline`.
///
/// Exposed at module level so tests can pin the wire shape without
/// poking through the executor's error path.
pub fn build_envelope(receipt: &PolicyReceipt) -> MandateEnvelope {
    MandateEnvelope::from_receipt(receipt, &receipt.audit_event_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mandate_core::receipt::{EmbeddedSignature, ReceiptType, SignatureAlgorithm};

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

    #[test]
    fn live_mode_fails_loudly_without_credentials() {
        let exec = KeeperHubExecutor::live();
        let err = exec
            .execute(&aprp(), &receipt(Decision::Allow))
            .unwrap_err();
        assert!(matches!(err, ExecutionError::BackendOffline(_)));
    }

    #[test]
    fn keeperhub_live_constructs_envelope_via_from_receipt() {
        let r = receipt(Decision::Allow);
        let env = build_envelope(&r);
        assert_eq!(env.mandate_request_hash, r.request_hash);
        assert_eq!(env.mandate_policy_hash, r.policy_hash);
        assert_eq!(env.mandate_receipt_signature, r.signature.signature_hex);
        assert_eq!(env.mandate_audit_event_id, r.audit_event_id);
        assert!(env.mandate_passport_capsule_hash.is_none());
    }

    /// Surface coverage: the public re-exports remain wired to
    /// `mandate_core::execution::*`. A future bump of mandate-core that
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
        };
        let r = receipt(Decision::Allow);
        let _: MandateEnvelope = MandateEnvelope::from_receipt(&r, &r.audit_event_id);
        // The trait re-export — implemented for our own executor.
        let exec: Box<dyn GuardedExecutor> = Box::new(KeeperHubExecutor::local_mock());
        assert_eq!(exec.sponsor_id(), "keeperhub");
    }

    /// The crate's surface must NOT pull `mandate_execution` /
    /// `mandate_server` / `mandate_storage` into a third-party caller's
    /// dependency closure. This test compiles against
    /// `mandate_core::execution::*` only — if a future change accidentally
    /// adds a workspace-internal type to a public signature, the test
    /// won't compile because the upstream crate isn't listed in
    /// `[dependencies]`.
    #[test]
    fn surface_is_mandate_core_only() {
        fn _accepts_only_core_types(env: &MandateEnvelope, exec: &KeeperHubExecutor) {
            let _ = env.mandate_request_hash.len();
            let _ = exec.sponsor_id();
        }
    }
}
