//! Mandate Passport capsule structural verification (P1.1).
//!
//! A *passport capsule* (`mandate.passport_capsule.v1`) is the portable,
//! offline-verifiable proof artifact wrapping one Mandate decision plus
//! its surrounding identity, request, policy, execution, audit, and
//! verification context. The capsule is composed from existing Mandate
//! primitives (APRP, PolicyReceipt, SignedAuditEvent, AuditCheckpoint,
//! ENS records) — this module DOES NOT redefine them, it only checks
//! how they appear together inside one capsule.
//!
//! `verify_capsule` is **structural only** in P1.1:
//!
//! 1. Schema validation via [`crate::schema::validate_passport_capsule`].
//! 2. Internal-consistency invariants from
//!    `docs/product/MANDATE_PASSPORT_SOURCE_OF_TRUTH.md`:
//!    - `decision.result == "deny"` ⇒ `execution.status == "not_called"`
//!      and `execution.execution_ref` is null.
//!    - `execution.mode == "live"` ⇒ `execution.live_evidence` contains
//!      at least one concrete reference (`transport`, `response_ref`, or
//!      `block_ref`).
//!    - `request.request_hash == decision.receipt.request_hash`.
//!    - `policy.policy_hash == decision.receipt.policy_hash`.
//!    - `decision.result == decision.receipt.decision`.
//!    - `agent.agent_id == decision.receipt.agent_id`.
//!    - `audit.audit_event_id == decision.receipt.audit_event_id`.
//!    - When `audit.checkpoint` is present:
//!      `audit.checkpoint.latest_event_hash == audit.event_hash` and the
//!      schema-level `mock_anchor: true` invariant has already fired.
//!
//! Crypto-level checks (Ed25519 signature verification, audit-chain
//! prev-hash linkage, full APRP re-hashing to confirm `request_hash`)
//! are intentionally **out of scope** for P1.1. They belong in P2.1
//! (`mandate passport run/verify`), where the verifier wraps the
//! existing `audit_bundle` codec rather than reimplementing it.

use serde_json::Value;

use crate::error::SchemaError;

/// Reasons a capsule fails structural verification.
#[derive(Debug, thiserror::Error)]
pub enum CapsuleVerifyError {
    #[error("capsule.schema_invalid: {0}")]
    SchemaInvalid(#[from] SchemaError),

    /// `decision.result == "deny"` but the capsule still records an
    /// execution call. This is the strongest truthfulness rule for the
    /// capsule: a denied action must never have reached an executor.
    #[error(
        "capsule.deny_with_execution: deny capsule must have execution.status=\"not_called\" \
         and execution.execution_ref=null; got status={status:?} execution_ref={execution_ref:?}"
    )]
    DenyWithExecution {
        status: String,
        execution_ref: Option<String>,
    },

    /// `execution.mode == "live"` requires concrete `live_evidence`.
    /// Live without evidence is the prototypical "fake live" claim.
    #[error(
        "capsule.live_without_evidence: execution.mode=\"live\" requires non-null \
         execution.live_evidence with at least one of transport/response_ref/block_ref"
    )]
    LiveWithoutEvidence,

    /// `execution.mode == "mock"` must NOT carry live evidence.
    #[error(
        "capsule.mock_with_live_evidence: execution.mode=\"mock\" must have null \
         execution.live_evidence; live_evidence on a mock execution is a mislabel"
    )]
    MockWithLiveEvidence,

    /// `request.request_hash` and the embedded receipt's `request_hash`
    /// disagree. The capsule is internally inconsistent.
    #[error(
        "capsule.request_hash_mismatch: request.request_hash={outer} but \
         decision.receipt.request_hash={receipt}"
    )]
    RequestHashMismatch { outer: String, receipt: String },

    /// `policy.policy_hash` and the embedded receipt's `policy_hash`
    /// disagree.
    #[error(
        "capsule.policy_hash_mismatch: policy.policy_hash={outer} but \
         decision.receipt.policy_hash={receipt}"
    )]
    PolicyHashMismatch { outer: String, receipt: String },

    /// `decision.result` and `decision.receipt.decision` disagree.
    #[error(
        "capsule.decision_result_mismatch: decision.result={outer} but \
         decision.receipt.decision={receipt}"
    )]
    DecisionResultMismatch { outer: String, receipt: String },

    /// `agent.agent_id` and `decision.receipt.agent_id` disagree.
    #[error(
        "capsule.agent_id_mismatch: agent.agent_id={outer} but \
         decision.receipt.agent_id={receipt}"
    )]
    AgentIdMismatch { outer: String, receipt: String },

    /// `audit.audit_event_id` and `decision.receipt.audit_event_id`
    /// disagree.
    #[error(
        "capsule.audit_event_id_mismatch: audit.audit_event_id={outer} but \
         decision.receipt.audit_event_id={receipt}"
    )]
    AuditEventIdMismatch { outer: String, receipt: String },

    /// Embedded checkpoint's `latest_event_hash` doesn't match the
    /// outer `audit.event_hash`. The capsule is internally inconsistent.
    #[error(
        "capsule.checkpoint_event_hash_mismatch: audit.event_hash={outer} but \
         audit.checkpoint.latest_event_hash={checkpoint}"
    )]
    CheckpointEventHashMismatch { outer: String, checkpoint: String },

    /// Catch-all for malformed but technically schema-valid capsules
    /// where a required nested string is the wrong shape after schema
    /// validation passed (e.g. an enum value snuck through). Should be
    /// rare; helps surface internal logic bugs.
    #[error("capsule.malformed: {detail}")]
    Malformed { detail: String },
}

impl CapsuleVerifyError {
    /// Stable machine-readable error code for CLI/JSON consumers.
    pub fn code(&self) -> &'static str {
        match self {
            Self::SchemaInvalid(_) => "capsule.schema_invalid",
            Self::DenyWithExecution { .. } => "capsule.deny_with_execution",
            Self::LiveWithoutEvidence => "capsule.live_without_evidence",
            Self::MockWithLiveEvidence => "capsule.mock_with_live_evidence",
            Self::RequestHashMismatch { .. } => "capsule.request_hash_mismatch",
            Self::PolicyHashMismatch { .. } => "capsule.policy_hash_mismatch",
            Self::DecisionResultMismatch { .. } => "capsule.decision_result_mismatch",
            Self::AgentIdMismatch { .. } => "capsule.agent_id_mismatch",
            Self::AuditEventIdMismatch { .. } => "capsule.audit_event_id_mismatch",
            Self::CheckpointEventHashMismatch { .. } => "capsule.checkpoint_event_hash_mismatch",
            Self::Malformed { .. } => "capsule.malformed",
        }
    }
}

/// Run schema validation **and** the cross-field truthfulness invariants
/// against `value`. Returns `Ok(())` only if every check passes. The
/// first violation surfaces — caller can re-check after fixing the cause.
pub fn verify_capsule(value: &Value) -> std::result::Result<(), CapsuleVerifyError> {
    crate::schema::validate_passport_capsule(value)?;

    // Schema guarantees `decision`, `execution`, `request`, `policy`,
    // `agent`, `audit` exist and have the right shapes. We unwrap with
    // `Malformed` fallback purely as defense-in-depth — a passing
    // schema validation should make these impossible.
    let decision = value
        .get("decision")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "decision missing after schema-pass".into(),
        })?;
    let execution = value
        .get("execution")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "execution missing after schema-pass".into(),
        })?;
    let request = value
        .get("request")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "request missing after schema-pass".into(),
        })?;
    let policy = value
        .get("policy")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "policy missing after schema-pass".into(),
        })?;
    let agent = value
        .get("agent")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "agent missing after schema-pass".into(),
        })?;
    let audit = value
        .get("audit")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "audit missing after schema-pass".into(),
        })?;
    let receipt = decision
        .get("receipt")
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: "decision.receipt missing after schema-pass".into(),
        })?;

    let decision_result = string_field(decision, "result")?;

    // Invariant 1: deny ⇒ no executor call.
    if decision_result == "deny" {
        let status = string_field(execution, "status")?;
        let execution_ref = execution
            .get("execution_ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if status != "not_called" || execution_ref.is_some() {
            return Err(CapsuleVerifyError::DenyWithExecution {
                status,
                execution_ref,
            });
        }
    }

    // Invariant 2: live mode ⇒ concrete live evidence; mock mode ⇒ no live evidence.
    let mode = string_field(execution, "mode")?;
    let live_evidence = execution.get("live_evidence");
    let evidence_present = live_evidence.map(|v| !v.is_null()).unwrap_or(false);
    let concrete_evidence_present = live_evidence_has_concrete_ref(live_evidence);
    match (mode.as_str(), evidence_present, concrete_evidence_present) {
        ("live", _, false) => return Err(CapsuleVerifyError::LiveWithoutEvidence),
        ("mock", true, _) => return Err(CapsuleVerifyError::MockWithLiveEvidence),
        _ => {}
    }

    // Invariant 3: request_hash agreement.
    let outer_request_hash = string_field(request, "request_hash")?;
    let receipt_request_hash = string_field(receipt, "request_hash")?;
    if outer_request_hash != receipt_request_hash {
        return Err(CapsuleVerifyError::RequestHashMismatch {
            outer: outer_request_hash,
            receipt: receipt_request_hash,
        });
    }

    // Invariant 4: policy_hash agreement.
    let outer_policy_hash = string_field(policy, "policy_hash")?;
    let receipt_policy_hash = string_field(receipt, "policy_hash")?;
    if outer_policy_hash != receipt_policy_hash {
        return Err(CapsuleVerifyError::PolicyHashMismatch {
            outer: outer_policy_hash,
            receipt: receipt_policy_hash,
        });
    }

    // Invariant 5: decision result agreement.
    let receipt_decision = string_field(receipt, "decision")?;
    if decision_result != receipt_decision {
        return Err(CapsuleVerifyError::DecisionResultMismatch {
            outer: decision_result,
            receipt: receipt_decision,
        });
    }

    // Invariant 6: agent_id agreement.
    let outer_agent_id = string_field(agent, "agent_id")?;
    let receipt_agent_id = string_field(receipt, "agent_id")?;
    if outer_agent_id != receipt_agent_id {
        return Err(CapsuleVerifyError::AgentIdMismatch {
            outer: outer_agent_id,
            receipt: receipt_agent_id,
        });
    }

    // Invariant 7: audit_event_id agreement.
    let outer_audit_event_id = string_field(audit, "audit_event_id")?;
    let receipt_audit_event_id = string_field(receipt, "audit_event_id")?;
    if outer_audit_event_id != receipt_audit_event_id {
        return Err(CapsuleVerifyError::AuditEventIdMismatch {
            outer: outer_audit_event_id,
            receipt: receipt_audit_event_id,
        });
    }

    // Invariant 8: when checkpoint is present, its latest_event_hash
    // must match the outer audit.event_hash.
    if let Some(checkpoint) = audit.get("checkpoint") {
        if !checkpoint.is_null() {
            let outer_event_hash = string_field(audit, "event_hash")?;
            let cp_latest = string_field(checkpoint, "latest_event_hash")?;
            if outer_event_hash != cp_latest {
                return Err(CapsuleVerifyError::CheckpointEventHashMismatch {
                    outer: outer_event_hash,
                    checkpoint: cp_latest,
                });
            }
        }
    }

    Ok(())
}

fn string_field(parent: &Value, key: &str) -> std::result::Result<String, CapsuleVerifyError> {
    parent
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| CapsuleVerifyError::Malformed {
            detail: format!("expected string field {key:?} after schema-pass"),
        })
}

fn live_evidence_has_concrete_ref(value: Option<&Value>) -> bool {
    let Some(object) = value.and_then(|v| v.as_object()) else {
        return false;
    };
    ["transport", "response_ref", "block_ref"]
        .iter()
        .any(|key| {
            object
                .get(*key)
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(path: &str) -> Value {
        let raw = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn corpus(name: &str) -> Value {
        let path =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-corpus/passport/").to_string() + name;
        load(&path)
    }

    #[test]
    fn golden_allow_capsule_verifies() {
        let v = corpus("golden_001_allow_keeperhub_mock.json");
        verify_capsule(&v).expect("golden capsule must verify");
    }

    #[test]
    fn tampered_deny_with_execution_ref_is_rejected() {
        let v = corpus("tampered_001_deny_with_execution_ref.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.deny_with_execution", "{err}");
    }

    #[test]
    fn tampered_mock_anchor_marked_live_is_rejected_by_schema() {
        let v = corpus("tampered_002_mock_anchor_marked_live.json");
        let err = verify_capsule(&v).expect_err("must fail");
        // Schema enforces `mock_anchor: const true`; verifier surfaces the
        // schema failure path, not a custom invariant code.
        assert_eq!(err.code(), "capsule.schema_invalid", "{err}");
    }

    #[test]
    fn tampered_live_mode_without_evidence_is_rejected() {
        let v = corpus("tampered_003_live_mode_without_evidence.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.live_without_evidence", "{err}");
    }

    #[test]
    fn tampered_live_mode_empty_evidence_is_rejected() {
        let v = corpus("tampered_008_live_mode_empty_evidence.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.schema_invalid", "{err}");
    }

    #[test]
    fn live_mode_with_concrete_evidence_verifies() {
        let mut v = corpus("golden_001_allow_keeperhub_mock.json");
        let execution = v["execution"].as_object_mut().unwrap();
        execution.insert("mode".into(), Value::String("live".into()));
        execution.insert(
            "live_evidence".into(),
            serde_json::json!({
                "transport": "https",
                "response_ref": "keeperhub-execution-01HTAWX5K3R8YV9NQB7C6P2DGS"
            }),
        );
        verify_capsule(&v).expect("live capsule with concrete evidence must verify");
    }

    #[test]
    fn mock_mode_with_concrete_live_evidence_is_rejected() {
        let mut v = corpus("golden_001_allow_keeperhub_mock.json");
        let execution = v["execution"].as_object_mut().unwrap();
        execution.insert(
            "live_evidence".into(),
            serde_json::json!({
                "response_ref": "keeperhub-execution-01HTAWX5K3R8YV9NQB7C6P2DGS"
            }),
        );
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.mock_with_live_evidence", "{err}");
    }

    #[test]
    fn tampered_request_hash_mismatch_is_rejected() {
        let v = corpus("tampered_004_request_hash_mismatch.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.request_hash_mismatch", "{err}");
    }

    #[test]
    fn tampered_policy_hash_mismatch_is_rejected() {
        let v = corpus("tampered_005_policy_hash_mismatch.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.policy_hash_mismatch", "{err}");
    }

    #[test]
    fn tampered_malformed_checkpoint_is_rejected_by_schema() {
        // tampered_006 has mock_anchor_ref="remote-onchain-eth-..." which
        // does NOT match the `^local-mock-anchor-[0-9a-f]{16}$` pattern.
        let v = corpus("tampered_006_malformed_checkpoint.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.schema_invalid", "{err}");
    }

    #[test]
    fn tampered_unknown_field_is_rejected_by_schema() {
        let v = corpus("tampered_007_unknown_field.json");
        let err = verify_capsule(&v).expect_err("must fail");
        assert_eq!(err.code(), "capsule.schema_invalid", "{err}");
    }

    #[test]
    fn schema_compiles() {
        // Pin: the embedded schema must compile at startup. Caught by
        // build_with_refs's expect-panic but worth a sentinel test.
        let _ = crate::schema::PASSPORT_CAPSULE_SCHEMA_JSON;
        let v: serde_json::Value =
            serde_json::from_str(crate::schema::PASSPORT_CAPSULE_SCHEMA_JSON).unwrap();
        assert_eq!(
            v["$id"].as_str().unwrap(),
            crate::schema::PASSPORT_CAPSULE_SCHEMA_ID
        );
    }
}
