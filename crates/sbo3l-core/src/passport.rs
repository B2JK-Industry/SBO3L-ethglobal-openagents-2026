//! SBO3L Passport capsule structural verification (P1.1).
//!
//! A *passport capsule* (`sbo3l.passport_capsule.v1`) is the portable,
//! offline-verifiable proof artifact wrapping one SBO3L decision plus
//! its surrounding identity, request, policy, execution, audit, and
//! verification context. The capsule is composed from existing SBO3L
//! primitives (APRP, PolicyReceipt, SignedAuditEvent, AuditCheckpoint,
//! ENS records) — this module DOES NOT redefine them, it only checks
//! how they appear together inside one capsule.
//!
//! `verify_capsule` is **structural only** in P1.1:
//!
//! 1. Schema validation via [`crate::schema::validate_passport_capsule`].
//! 2. Internal-consistency invariants from
//!    `docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`:
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
//! (`sbo3l passport run/verify`), where the verifier wraps the
//! existing `audit_bundle` codec rather than reimplementing it.

use serde_json::Value;

use crate::audit_bundle::{self, AuditBundle, BundleError};
use crate::error::SchemaError;
use crate::hashing;
use crate::receipt::PolicyReceipt;
use crate::signer::VerifyError;

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

// =====================================================================
// Strict (cryptographic) capsule verification.
// =====================================================================
//
// `verify_capsule` (above) is intentionally structural-only — see the
// module doc-comment + `SECURITY_NOTES.md` §"Passport verifier scope".
// `verify_capsule_strict` extends it with the cryptographic checks that
// belong off the structural fast path: re-hash the APRP, verify the
// receipt's Ed25519 signature, walk the audit chain, and check the
// audit-event-id linkage.
//
// Some checks are conditional on auxiliary inputs the capsule does NOT
// itself carry (a receipt-signer pubkey, an audit bundle, a policy
// snapshot). When the input is absent the corresponding check is
// reported as `Skipped` rather than `Failed` — so a caller that only
// wants the `aprp → request_hash` recompute can pass `Default::default()`
// and still get a useful pass/fail report. This is the honest disclosure
// pattern: never a fake-OK; always either a real PASS, an explicit
// SKIP-with-reason, or a FAIL-with-reason.

/// Auxiliary inputs for the cryptographic strict verifier. Each input is
/// independent — passing all three runs every check; passing none runs
/// only the structural pass + the `request_hash` recompute (the only
/// crypto check the capsule alone is enough for).
#[derive(Default, Debug)]
pub struct StrictVerifyOpts<'a> {
    /// Hex-encoded Ed25519 public key for the receipt signer. Required
    /// to run the `receipt_signature` check. When `None` that check is
    /// reported as `Skipped(missing_input)`.
    pub receipt_pubkey_hex: Option<&'a str>,

    /// Audit bundle (`sbo3l.audit_bundle.v1`) whose chain segment must
    /// contain the capsule's `audit.audit_event_id`. Required to run
    /// the `audit_chain` and `audit_event_link` checks. The bundle is
    /// fully verified via [`crate::audit_bundle::verify`] (signatures +
    /// chain linkage + summary consistency); if that returns `Ok`, the
    /// link check additionally pins that `bundle.summary.audit_event_id
    /// == capsule.audit.audit_event_id` so a capsule cannot point at a
    /// chain prefix that doesn't include its own decision event.
    pub audit_bundle: Option<&'a AuditBundle>,

    /// Canonical policy JSON snapshot whose JCS+SHA-256 hash should
    /// match `capsule.policy.policy_hash`. Required to run the
    /// `policy_hash_recompute` check.
    pub policy_json: Option<&'a Value>,
}

/// Outcome of one strict-verify check. `Skipped` carries a one-line
/// reason for the operator (typically: missing aux input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    Passed,
    Skipped(String),
    Failed(String),
}

impl CheckOutcome {
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped(_))
    }
}

/// Per-check report produced by [`verify_capsule_strict`]. Stable shape
/// for CLI/JSON consumers; field order matches the documented check
/// order in the strict-mode CLI output.
#[derive(Debug, Clone)]
pub struct StrictVerifyReport {
    /// Structural verify (the same checks as [`verify_capsule`]).
    /// If this fails, every other check is `Skipped(structural_failed)`
    /// because running crypto on a structurally-invalid capsule
    /// produces noise instead of signal.
    pub structural: CheckOutcome,

    /// Recompute `request_hash` from `capsule.request.aprp` via JCS +
    /// SHA-256, then assert it matches BOTH `capsule.request.request_hash`
    /// AND `capsule.decision.receipt.request_hash`. The capsule alone
    /// is enough — no aux input required.
    pub request_hash_recompute: CheckOutcome,

    /// Recompute `policy_hash` from the supplied policy JSON via JCS +
    /// SHA-256, then assert it matches `capsule.policy.policy_hash`.
    /// `Skipped` when `opts.policy_json` is absent.
    pub policy_hash_recompute: CheckOutcome,

    /// Verify the Ed25519 signature on `capsule.decision.receipt`
    /// against the supplied pubkey. `Skipped` when
    /// `opts.receipt_pubkey_hex` is absent.
    pub receipt_signature: CheckOutcome,

    /// Run [`crate::audit_bundle::verify`] over the supplied bundle —
    /// this catches every chain-level tampering (mutated event hash,
    /// broken `prev_event_hash` linkage, signature bytes mutated, etc.).
    /// `Skipped` when `opts.audit_bundle` is absent.
    pub audit_chain: CheckOutcome,

    /// Pin that the supplied bundle's audit event id (and the bundle's
    /// summary) match the capsule's `audit.audit_event_id`. Catches the
    /// "wrong bundle for this capsule" attack. `Skipped` when
    /// `opts.audit_bundle` is absent.
    pub audit_event_link: CheckOutcome,
}

impl StrictVerifyReport {
    /// True iff every check that ran (i.e. was not `Skipped`) passed.
    /// A report with all skips trivially returns `true` — callers who
    /// want full coverage should use [`Self::is_fully_ok`].
    pub fn is_ok(&self) -> bool {
        self.iter().all(|c| !c.is_failed())
    }

    /// True iff every check passed (none skipped, none failed). The
    /// strongest possible verification result.
    pub fn is_fully_ok(&self) -> bool {
        self.iter().all(|c| c.is_passed())
    }

    /// All six check outcomes, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &CheckOutcome> {
        [
            &self.structural,
            &self.request_hash_recompute,
            &self.policy_hash_recompute,
            &self.receipt_signature,
            &self.audit_chain,
            &self.audit_event_link,
        ]
        .into_iter()
    }

    /// Stable label for each check; pairs 1:1 with `iter()`.
    pub fn labels() -> [&'static str; 6] {
        [
            "structural",
            "request_hash_recompute",
            "policy_hash_recompute",
            "receipt_signature",
            "audit_chain",
            "audit_event_link",
        ]
    }
}

/// Run [`verify_capsule`] plus the cryptographic checks supported by
/// the supplied auxiliary inputs. Returns a structured report — see
/// [`StrictVerifyReport`] — never a single boolean. The caller decides
/// what counts as a passing run via `is_ok()` (no failures) or
/// `is_fully_ok()` (no failures + no skips).
pub fn verify_capsule_strict(value: &Value, opts: &StrictVerifyOpts) -> StrictVerifyReport {
    // Step 1: structural verify. If this fails, the capsule is
    // self-inconsistent — running crypto on it produces misleading
    // results, so every downstream check is reported as Skipped with
    // a structural-failed reason.
    let structural = match verify_capsule(value) {
        Ok(()) => CheckOutcome::Passed,
        Err(e) => CheckOutcome::Failed(format!("{} ({})", e, e.code())),
    };

    if structural.is_failed() {
        let skip = CheckOutcome::Skipped(
            "skipped: structural verify failed; crypto checks not meaningful".into(),
        );
        return StrictVerifyReport {
            structural,
            request_hash_recompute: skip.clone(),
            policy_hash_recompute: skip.clone(),
            receipt_signature: skip.clone(),
            audit_chain: skip.clone(),
            audit_event_link: skip,
        };
    }

    // Schema has passed; the unwraps below are safe and labelled if not.
    let request_hash_recompute = check_request_hash_recompute(value);
    let policy_hash_recompute = check_policy_hash_recompute(value, opts.policy_json);
    let receipt_signature = check_receipt_signature(value, opts.receipt_pubkey_hex);
    let audit_chain = check_audit_chain(opts.audit_bundle);
    let audit_event_link = check_audit_event_link(value, opts.audit_bundle);

    StrictVerifyReport {
        structural,
        request_hash_recompute,
        policy_hash_recompute,
        receipt_signature,
        audit_chain,
        audit_event_link,
    }
}

fn check_request_hash_recompute(capsule: &Value) -> CheckOutcome {
    let Some(aprp) = capsule.pointer("/request/aprp") else {
        return CheckOutcome::Failed("capsule.request.aprp missing".into());
    };
    let recomputed = match hashing::request_hash(aprp) {
        Ok(h) => h,
        Err(e) => return CheckOutcome::Failed(format!("JCS canonicalization failed: {e}")),
    };
    let outer = capsule
        .pointer("/request/request_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let receipt = capsule
        .pointer("/decision/receipt/request_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if outer != recomputed {
        return CheckOutcome::Failed(format!(
            "capsule.request.request_hash={outer} but recomputed JCS+SHA-256 of \
             capsule.request.aprp = {recomputed}"
        ));
    }
    if receipt != recomputed {
        return CheckOutcome::Failed(format!(
            "capsule.decision.receipt.request_hash={receipt} but recomputed JCS+SHA-256 of \
             capsule.request.aprp = {recomputed}"
        ));
    }
    CheckOutcome::Passed
}

fn check_policy_hash_recompute(capsule: &Value, policy_json: Option<&Value>) -> CheckOutcome {
    let Some(policy) = policy_json else {
        return CheckOutcome::Skipped(
            "skipped: --policy <path> not supplied; policy_hash recompute requires the canonical \
             policy JSON snapshot"
                .into(),
        );
    };
    let bytes = match hashing::canonical_json(policy) {
        Ok(b) => b,
        Err(e) => return CheckOutcome::Failed(format!("policy JCS canonicalization failed: {e}")),
    };
    let recomputed = hashing::sha256_hex(&bytes);
    let claimed = capsule
        .pointer("/policy/policy_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if claimed != recomputed {
        return CheckOutcome::Failed(format!(
            "capsule.policy.policy_hash={claimed} but recomputed JCS+SHA-256 of supplied \
             policy snapshot = {recomputed}"
        ));
    }
    CheckOutcome::Passed
}

fn check_receipt_signature(capsule: &Value, pubkey_hex: Option<&str>) -> CheckOutcome {
    let Some(pubkey) = pubkey_hex else {
        return CheckOutcome::Skipped(
            "skipped: --receipt-pubkey <hex> not supplied; Ed25519 signature verification \
             requires the receipt signer's public key"
                .into(),
        );
    };
    let Some(receipt_value) = capsule.pointer("/decision/receipt") else {
        return CheckOutcome::Failed("capsule.decision.receipt missing".into());
    };
    let receipt: PolicyReceipt = match serde_json::from_value(receipt_value.clone()) {
        Ok(r) => r,
        Err(e) => {
            return CheckOutcome::Failed(format!(
                "capsule.decision.receipt could not be deserialized as PolicyReceipt: {e}"
            ))
        }
    };
    match receipt.verify(pubkey) {
        Ok(()) => CheckOutcome::Passed,
        Err(VerifyError::BadPublicKey) => {
            CheckOutcome::Failed("supplied receipt-pubkey is not a valid Ed25519 public key".into())
        }
        Err(VerifyError::BadSignature) => CheckOutcome::Failed(
            "capsule.decision.receipt.signature.signature_hex is not a valid Ed25519 signature \
             (wrong length or non-hex)"
                .into(),
        ),
        Err(VerifyError::Invalid) => CheckOutcome::Failed(
            "Ed25519 signature did not verify against supplied receipt-pubkey over the \
             canonical receipt body"
                .into(),
        ),
    }
}

fn check_audit_chain(bundle: Option<&AuditBundle>) -> CheckOutcome {
    let Some(b) = bundle else {
        return CheckOutcome::Skipped(
            "skipped: --audit-bundle <path> not supplied; chain walk requires the \
             sbo3l.audit_bundle.v1 artefact for the capsule's audit event"
                .into(),
        );
    };
    match audit_bundle::verify(b) {
        Ok(_) => CheckOutcome::Passed,
        Err(BundleError::ReceiptSignatureInvalid) => CheckOutcome::Failed(
            "audit_bundle::verify: receipt signature does not verify under the bundle's \
             receipt-signer pubkey"
                .into(),
        ),
        Err(BundleError::AuditEventSignatureInvalid) => CheckOutcome::Failed(
            "audit_bundle::verify: audit event signature does not verify under the bundle's \
             audit-signer pubkey"
                .into(),
        ),
        Err(BundleError::Chain(e)) => {
            CheckOutcome::Failed(format!("audit chain verify failed: {e}"))
        }
        Err(e) => CheckOutcome::Failed(format!("audit_bundle::verify: {e}")),
    }
}

fn check_audit_event_link(capsule: &Value, bundle: Option<&AuditBundle>) -> CheckOutcome {
    let Some(b) = bundle else {
        return CheckOutcome::Skipped(
            "skipped: --audit-bundle <path> not supplied; audit-event-id linkage requires \
             the bundle"
                .into(),
        );
    };
    let capsule_id = capsule
        .pointer("/audit/audit_event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let bundle_id = b.summary.audit_event_id.as_str();
    if capsule_id != bundle_id {
        return CheckOutcome::Failed(format!(
            "capsule.audit.audit_event_id={capsule_id} but \
             bundle.summary.audit_event_id={bundle_id} — wrong bundle for this capsule"
        ));
    }
    // Defence in depth: also check the chain segment actually contains the event id.
    let in_chain = b
        .audit_chain_segment
        .iter()
        .any(|e| e.event.id == capsule_id);
    if !in_chain {
        return CheckOutcome::Failed(format!(
            "capsule.audit.audit_event_id={capsule_id} not present in bundle.audit_chain_segment"
        ));
    }
    CheckOutcome::Passed
}


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

    // -----------------------------------------------------------------
    // P6.1 — `execution.executor_evidence` (mode-agnostic sponsor slot)
    // -----------------------------------------------------------------
    //
    // The verifier adds NO new cross-field invariant for
    // `executor_evidence`: the schema is the single source of truth for
    // the slot's shape (`oneOf null / object minProperties:1`,
    // `additionalProperties: true`). The two tests below pin the two
    // behaviours the verifier MUST exhibit:
    //
    // 1. A capsule with `executor_evidence: null` (or omitted) verifies.
    // 2. A capsule with arbitrary, freeform `executor_evidence` content
    //    verifies — the schema validates the slot's shape; the
    //    bidirectional `live_evidence` invariant continues to hold
    //    because `executor_evidence` is a separate slot.

    #[test]
    fn executor_evidence_null_accepted() {
        // The golden allow capsule omits `executor_evidence` entirely
        // (the schema's `oneOf null / object` accepts a missing field
        // when the property has no required entry). Adding `null`
        // explicitly should also pass — both forms are equivalent on
        // the wire and the verifier must treat them identically.
        let v_missing = corpus("golden_001_allow_keeperhub_mock.json");
        verify_capsule(&v_missing).expect("golden (executor_evidence missing) must verify");

        let mut v_null = corpus("golden_001_allow_keeperhub_mock.json");
        v_null["execution"]
            .as_object_mut()
            .unwrap()
            .insert("executor_evidence".into(), Value::Null);
        verify_capsule(&v_null).expect("explicit executor_evidence: null must verify");
    }

    #[test]
    fn executor_evidence_arbitrary_object_accepted() {
        // The schema is `additionalProperties: true` for the
        // executor_evidence slot, so any non-empty object passes
        // schema-level validation. The verifier (this module) adds no
        // shape rules of its own — sponsor adapters carry their own
        // structured payload here. We pin both a single-key shape
        // (KeeperHub IP-1 envelope progenitor) and a Uniswap-flavoured
        // multi-key shape so the test fails closed if a future change
        // accidentally tightens the slot.
        let mut v_min = corpus("golden_001_allow_keeperhub_mock.json");
        v_min["execution"].as_object_mut().unwrap().insert(
            "executor_evidence".into(),
            serde_json::json!({ "quote_id": "x" }),
        );
        verify_capsule(&v_min).expect("single-key executor_evidence must verify");

        let mut v_uni = corpus("golden_001_allow_keeperhub_mock.json");
        v_uni["execution"].as_object_mut().unwrap().insert(
            "executor_evidence".into(),
            serde_json::json!({
                "quote_id": "mock-uniswap-quote-X",
                "quote_source": "mock-uniswap-v3-router",
                "input_token": { "symbol": "USDC", "address": "0x0" },
                "output_token": { "symbol": "ETH", "address": "0x1" },
                "route_tokens": [],
                "notional_in": "0.05",
                "slippage_cap_bps": 50,
                "quote_timestamp_unix": 1_700_000_000,
                "quote_freshness_seconds": 30,
                "recipient_address": "0x1111111111111111111111111111111111111111"
            }),
        );
        verify_capsule(&v_uni).expect("uniswap-shaped executor_evidence must verify");
    }

    #[test]
    fn tampered_executor_evidence_empty_object_is_rejected_by_schema() {
        // tampered_009 sets `executor_evidence: {}` — schema's
        // `oneOf null / object minProperties:1` rejects this; the
        // verifier surfaces it as `capsule.schema_invalid`.
        let v = corpus("tampered_009_executor_evidence_empty_object.json");
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

    // =================================================================
    // Strict-verifier coverage (B1)
    // =================================================================
    //
    // The structural-only `verify_capsule` already has full coverage in
    // the tests above. These tests pin the cryptographic strict mode:
    // every check in `StrictVerifyReport` must pass on a freshly-built
    // capsule + matching aux inputs, and each documented tampering
    // class must produce a `Failed` result on the right check while
    // every other check stays `Passed`.

    use crate::audit::{AuditEvent, SignedAuditEvent, ZERO_HASH};
    use crate::audit_bundle;
    use crate::receipt::{Decision, UnsignedReceipt};
    use crate::signer::DevSigner;

    /// Build a real, cryptographically-valid capsule + the matching
    /// auxiliary inputs (receipt pubkey + audit bundle + policy
    /// snapshot). All inputs derived from the same `DevSigner` seeds so
    /// a happy-path strict verify with all aux inputs returns
    /// `is_fully_ok()`.
    fn strict_fixture() -> (
        Value,
        DevSigner, // receipt signer
        DevSigner, // audit signer
        AuditBundle,
        Value, // canonical policy snapshot
    ) {
        let receipt_signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);
        let audit_signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);

        // Canonical policy snapshot — any deterministic JSON works as
        // long as JCS+SHA-256 over its bytes equals the capsule's
        // policy.policy_hash. We keep it tiny.
        let policy_json: Value = serde_json::json!({
            "policy_id": "reference_low_risk_v1",
            "version": 1,
            "rules": [
                { "id": "allow-low-risk-x402", "decision": "allow" }
            ]
        });
        let policy_bytes = hashing::canonical_json(&policy_json).unwrap();
        let policy_hash = hashing::sha256_hex(&policy_bytes);

        // Real APRP body. The capsule's request_hash + the receipt's
        // request_hash must both equal sha256(JCS(this body)).
        let aprp: Value = serde_json::json!({
            "agent_id": "research-agent-01",
            "task_id": "demo-task-1",
            "intent": "purchase_api_call",
            "amount": { "value": "0.05", "currency": "USD" },
            "token": "USDC",
            "destination": {
                "type": "x402_endpoint",
                "url": "https://api.example.com/v1/inference",
                "method": "POST",
                "expected_recipient": "0x1111111111111111111111111111111111111111"
            },
            "payment_protocol": "x402",
            "chain": "base",
            "provider_url": "https://api.example.com",
            "x402_payload": null,
            "expiry": "2026-05-01T10:31:00Z",
            "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
            "expected_result": null,
            "risk_class": "low"
        });
        let request_hash_hex = hashing::request_hash(&aprp).unwrap();

        // 3-event chain: runtime_started → policy_decided (the one the
        // capsule references) → policy_decided (filler).
        let e1_event = AuditEvent {
            version: 1,
            seq: 1,
            id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGQ".into(),
            ts: chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:00Z")
                .unwrap()
                .into(),
            event_type: "runtime_started".into(),
            actor: "sbo3l-server".into(),
            subject_id: "runtime".into(),
            payload_hash: ZERO_HASH.into(),
            metadata: serde_json::Map::new(),
            policy_version: None,
            policy_hash: None,
            attestation_ref: None,
            prev_event_hash: ZERO_HASH.into(),
        };
        let e1 = SignedAuditEvent::sign(e1_event, &audit_signer).unwrap();

        let e2_event = AuditEvent {
            version: 1,
            seq: 2,
            id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR".into(),
            ts: chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:01Z")
                .unwrap()
                .into(),
            event_type: "policy_decided".into(),
            actor: "policy_engine".into(),
            subject_id: "pr-strict-001".into(),
            payload_hash: request_hash_hex.clone(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some(policy_hash.clone()),
            attestation_ref: None,
            prev_event_hash: e1.event_hash.clone(),
        };
        let e2 = SignedAuditEvent::sign(e2_event, &audit_signer).unwrap();

        let e3_event = AuditEvent {
            version: 1,
            seq: 3,
            id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".into(),
            ts: chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:02Z")
                .unwrap()
                .into(),
            event_type: "policy_decided".into(),
            actor: "policy_engine".into(),
            subject_id: "pr-strict-002".into(),
            payload_hash: ZERO_HASH.into(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some(policy_hash.clone()),
            attestation_ref: None,
            prev_event_hash: e2.event_hash.clone(),
        };
        let e3 = SignedAuditEvent::sign(e3_event, &audit_signer).unwrap();

        // Real signed receipt over (request_hash, policy_hash,
        // audit_event_id = e2.id).
        let unsigned = UnsignedReceipt {
            agent_id: "research-agent-01".into(),
            decision: Decision::Allow,
            deny_code: None,
            request_hash: request_hash_hex.clone(),
            policy_hash: policy_hash.clone(),
            policy_version: Some(1),
            audit_event_id: e2.event.id.clone(),
            execution_ref: None,
            issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-29T12:00:01.500Z")
                .unwrap()
                .into(),
            expires_at: None,
        };
        let receipt = unsigned.sign(&receipt_signer).unwrap();

        // Audit bundle covering the chain prefix through e2.
        let bundle = audit_bundle::build(
            receipt.clone(),
            vec![e1, e2.clone(), e3],
            receipt_signer.verifying_key_hex(),
            audit_signer.verifying_key_hex(),
            chrono::DateTime::parse_from_rfc3339("2026-04-29T13:00:00Z")
                .unwrap()
                .into(),
        )
        .unwrap();

        // Build a structurally-valid capsule wrapping the receipt.
        let capsule = serde_json::json!({
            "schema": "sbo3l.passport_capsule.v1",
            "generated_at": "2026-04-29T12:30:00Z",
            "agent": {
                "agent_id": "research-agent-01",
                "ens_name": "research-agent.team.eth",
                "resolver": "offline-fixture",
                "records": {
                    "sbo3l:policy_hash": policy_hash,
                    "sbo3l:audit_root": "local-mock-anchor-strict-fixture-1",
                    "sbo3l:passport_schema": "sbo3l.passport_capsule.v1"
                }
            },
            "request": {
                "aprp": aprp,
                "request_hash": request_hash_hex,
                "idempotency_key": "strict-fixture-1",
                "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM"
            },
            "policy": {
                "policy_hash": policy_hash,
                "policy_version": 1,
                "activated_at": "2026-04-28T10:00:00Z",
                "source": "operator-cli"
            },
            "decision": {
                "result": "allow",
                "matched_rule": "allow-low-risk-x402",
                "deny_code": null,
                "receipt": serde_json::to_value(&receipt).unwrap(),
                "receipt_signature": receipt.signature.signature_hex.clone()
            },
            "execution": {
                "executor": "keeperhub",
                "mode": "mock",
                "execution_ref": "kh-strict-001",
                "status": "submitted",
                "sponsor_payload_hash": ZERO_HASH,
                "live_evidence": null
            },
            "audit": {
                "audit_event_id": e2.event.id,
                "prev_event_hash": e2.event.prev_event_hash,
                "event_hash": e2.event_hash,
                "bundle_ref": "sbo3l.audit_bundle.v1",
                "checkpoint": {
                    "schema": "sbo3l.audit_checkpoint.v1",
                    "sequence": 1,
                    "latest_event_id": e2.event.id,
                    "latest_event_hash": e2.event_hash,
                    "chain_digest": ZERO_HASH,
                    "mock_anchor": true,
                    "mock_anchor_ref": "local-mock-anchor-strict-fixture-1",
                    "created_at": "2026-04-29T12:00:30Z"
                }
            },
            "verification": {
                "doctor_status": "ok",
                "offline_verifiable": true,
                "live_claims": []
            }
        });

        (capsule, receipt_signer, audit_signer, bundle, policy_json)
    }

    /// B1 test 1 — happy path. Every aux input present + valid → every
    /// check passes, including no skips.
    #[test]
    fn strict_verify_happy_path_passes_every_check() {
        let (capsule, receipt_signer, _audit_signer, bundle, policy) = strict_fixture();
        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(report.is_fully_ok(), "expected fully-ok; report = {report:?}");
        assert!(report.structural.is_passed());
        assert!(report.request_hash_recompute.is_passed());
        assert!(report.policy_hash_recompute.is_passed());
        assert!(report.receipt_signature.is_passed());
        assert!(report.audit_chain.is_passed());
        assert!(report.audit_event_link.is_passed());
    }

    /// B1 test 2 — tampered request body. Mutating `capsule.request.aprp`
    /// must surface a `request_hash_recompute` Failed result; structural
    /// pass remains green because the schema is satisfied + the *claimed*
    /// request_hash still equals the receipt's claimed request_hash.
    #[test]
    fn strict_verify_tampered_request_body_fails_request_hash_recompute() {
        let (mut capsule, receipt_signer, _audit_signer, bundle, policy) = strict_fixture();
        // Mutate the APRP body without updating any hashes — the
        // recomputed JCS+SHA-256 will diverge from the claimed
        // request_hash.
        capsule["request"]["aprp"]["amount"]["value"] = serde_json::Value::String("999.00".into());
        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(report.structural.is_passed(), "structural should still pass");
        assert!(
            report.request_hash_recompute.is_failed(),
            "request_hash_recompute should fail on mutated APRP body"
        );
    }

    /// B1 test 3 — tampered policy snapshot. Supplying a different
    /// policy JSON than the one that produced `capsule.policy.policy_hash`
    /// must surface a `policy_hash_recompute` Failed result.
    #[test]
    fn strict_verify_tampered_policy_snapshot_fails_policy_hash_recompute() {
        let (capsule, receipt_signer, _audit_signer, bundle, _policy) = strict_fixture();
        let bad_policy = serde_json::json!({
            "policy_id": "different-policy",
            "rules": []
        });
        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&bad_policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(
            report.policy_hash_recompute.is_failed(),
            "policy_hash_recompute should fail when supplied policy ≠ capsule.policy.policy_hash"
        );
    }

    /// B1 test 4 — tampered receipt signature. Flipping a hex byte of
    /// the signature must surface a `receipt_signature` Failed result.
    #[test]
    fn strict_verify_tampered_receipt_signature_fails_receipt_signature() {
        let (mut capsule, receipt_signer, _audit_signer, bundle, policy) = strict_fixture();
        // Flip a hex character in the embedded receipt's signature_hex.
        // The receipt deserializes (still 128 hex chars) but the
        // signature won't verify under the real pubkey.
        let sig = capsule["decision"]["receipt"]["signature"]["signature_hex"]
            .as_str()
            .unwrap()
            .to_string();
        let mut chars: Vec<char> = sig.chars().collect();
        // Flip the first hex char between '0' ↔ '1' so the result stays
        // valid hex.
        chars[0] = if chars[0] == '0' { '1' } else { '0' };
        let mutated: String = chars.into_iter().collect();
        capsule["decision"]["receipt"]["signature"]["signature_hex"] =
            serde_json::Value::String(mutated);

        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(
            report.receipt_signature.is_failed(),
            "receipt_signature must fail on a flipped signature byte"
        );
    }

    /// B1 test 5 — tampered audit prev_event_hash inside the bundle.
    /// The chain walk in `audit_bundle::verify` re-hashes each event
    /// and checks linkage; mutating one event's prev_event_hash must
    /// surface an `audit_chain` Failed result.
    #[test]
    fn strict_verify_tampered_audit_prev_hash_fails_audit_chain() {
        let (capsule, receipt_signer, _audit_signer, mut bundle, policy) = strict_fixture();
        // Flip a hex byte of the second event's prev_event_hash. The
        // event's signature still verifies (it's signed over original
        // canonical bytes? actually the signature is over canonical
        // bytes including prev_event_hash, so this also breaks the
        // event signature) — either way audit_chain must fail.
        let original = bundle.audit_chain_segment[1]
            .event
            .prev_event_hash
            .clone();
        let mut chars: Vec<char> = original.chars().collect();
        chars[0] = if chars[0] == '0' { '1' } else { '0' };
        bundle.audit_chain_segment[1].event.prev_event_hash = chars.into_iter().collect();

        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(
            report.audit_chain.is_failed(),
            "audit_chain must fail when prev_event_hash linkage is broken"
        );
    }

    /// B1 test 6 — wrong audit bundle (capsule's audit_event_id is
    /// not present in the supplied bundle). `audit_event_link` must
    /// surface a Failed result.
    #[test]
    fn strict_verify_wrong_audit_bundle_fails_audit_event_link() {
        let (mut capsule, receipt_signer, _audit_signer, bundle, policy) = strict_fixture();
        // Mutate the capsule's claimed audit_event_id to a value the
        // bundle doesn't contain. We have to update *both* outer and
        // receipt-embedded ids to keep the structural check green so
        // we can isolate the link failure on the strict side.
        let bogus = "evt-01ZZZZZZZZZZZZZZZZZZZZZZZZ";
        capsule["audit"]["audit_event_id"] = serde_json::Value::String(bogus.into());
        capsule["decision"]["receipt"]["audit_event_id"] = serde_json::Value::String(bogus.into());

        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(
            report.audit_event_link.is_failed(),
            "audit_event_link must fail when capsule.audit.audit_event_id is not in the bundle's chain"
        );
    }

    /// Bonus — minimal (no aux inputs). Runs only the structural pass
    /// + the request_hash recompute (the only crypto check the capsule
    /// alone supports). Every other check is `Skipped` with a reason.
    #[test]
    fn strict_verify_no_aux_inputs_skips_aux_dependent_checks() {
        let (capsule, _receipt_signer, _audit_signer, _bundle, _policy) = strict_fixture();
        let report = verify_capsule_strict(&capsule, &StrictVerifyOpts::default());
        assert!(report.structural.is_passed());
        assert!(report.request_hash_recompute.is_passed());
        assert!(report.policy_hash_recompute.is_skipped());
        assert!(report.receipt_signature.is_skipped());
        assert!(report.audit_chain.is_skipped());
        assert!(report.audit_event_link.is_skipped());
        assert!(report.is_ok(), "no failures means is_ok() = true");
        assert!(
            !report.is_fully_ok(),
            "skips mean is_fully_ok() = false"
        );
    }

    /// Bonus — structural failure short-circuits crypto. A capsule that
    /// fails the structural pass must report every downstream check as
    /// Skipped (not Failed) so the operator knows the structural cause
    /// is what to fix first.
    #[test]
    fn strict_verify_structural_failure_short_circuits_crypto_checks() {
        let (mut capsule, receipt_signer, _audit_signer, bundle, policy) = strict_fixture();
        // Break a structural invariant: force capsule.request.request_hash
        // to mismatch the receipt's request_hash. The structural verifier
        // catches this as RequestHashMismatch.
        capsule["request"]["request_hash"] =
            serde_json::Value::String("0000000000000000000000000000000000000000000000000000000000000000".into());
        let pk = receipt_signer.verifying_key_hex();
        let opts = StrictVerifyOpts {
            receipt_pubkey_hex: Some(&pk),
            audit_bundle: Some(&bundle),
            policy_json: Some(&policy),
        };
        let report = verify_capsule_strict(&capsule, &opts);
        assert!(report.structural.is_failed());
        assert!(report.request_hash_recompute.is_skipped());
        assert!(report.policy_hash_recompute.is_skipped());
        assert!(report.receipt_signature.is_skipped());
        assert!(report.audit_chain.is_skipped());
        assert!(report.audit_event_link.is_skipped());
    }
}
