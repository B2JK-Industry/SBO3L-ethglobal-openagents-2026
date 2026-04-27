//! KeeperHub guarded execution adapter.
//!
//! `Mandate decides, KeeperHub executes.` This adapter gates execution on a
//! signed `PolicyReceipt` from Mandate. If the receipt's decision is not
//! `allow`, execution is refused before any sponsor backend is contacted.
//!
//! Two modes:
//!
//! * `Live` — would call KeeperHub's real MCP/API endpoint. The hackathon
//!   build leaves this stubbed because public KeeperHub credentials are not
//!   available; switching to live is a single function body.
//! * `LocalMock` — returns a deterministic execution receipt with a fresh
//!   ULID `execution_ref` and `mock: true`. The demo discloses this clearly.

use mandate_core::aprp::PaymentRequest;
use mandate_core::receipt::{Decision, PolicyReceipt};

use crate::{ExecutionError, ExecutionReceipt, GuardedExecutor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeeperHubMode {
    Live,
    LocalMock,
}

#[derive(Debug, Clone)]
pub struct KeeperHubExecutor {
    pub mode: KeeperHubMode,
}

impl KeeperHubExecutor {
    pub fn local_mock() -> Self {
        Self {
            mode: KeeperHubMode::LocalMock,
        }
    }

    pub fn live() -> Self {
        Self {
            mode: KeeperHubMode::Live,
        }
    }
}

impl GuardedExecutor for KeeperHubExecutor {
    fn sponsor_id(&self) -> &'static str {
        "keeperhub"
    }

    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError> {
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
            KeeperHubMode::Live => Err(ExecutionError::BackendOffline(
                "live KeeperHub backend not configured for this hackathon build; \
                 switch to KeeperHubMode::LocalMock or wire credentials"
                    .to_string(),
            )),
        }
    }
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
}
