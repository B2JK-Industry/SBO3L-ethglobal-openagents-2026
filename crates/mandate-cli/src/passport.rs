//! `mandate passport verify --path <capsule>` (PSM Passport P1.1).
//!
//! Structural verification of a `mandate.passport_capsule.v1` JSON
//! artifact. Wraps the `mandate-core::passport::verify_capsule` invariant
//! suite — schema validation plus the cross-field truthfulness rules.
//!
//! P1.1 is **structural only**. The full `passport run` / `passport
//! explain` surfaces (and full cryptographic verification) land in P2.1.

use std::path::Path;
use std::process::ExitCode;

use mandate_core::passport::{verify_capsule, CapsuleVerifyError};

/// `mandate passport verify --path <capsule>`
///
/// Exit codes (per `docs/product/MANDATE_PASSPORT_BACKLOG.md` P1.1):
/// - 0 — capsule verifies (schema + every cross-field invariant).
/// - 1 — IO / parse failure (file missing, not JSON).
/// - 2 — capsule is malformed, tampered, or internally inconsistent.
pub fn cmd_verify(path: &Path) -> ExitCode {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "mandate passport verify: read {} failed: {e}",
                path.display()
            );
            return ExitCode::from(1);
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "mandate passport verify: parse {} failed: {e}",
                path.display()
            );
            return ExitCode::from(1);
        }
    };

    match verify_capsule(&value) {
        Ok(()) => {
            // Surface the high-signal fields a reviewer wants to see at
            // a glance. These all came through schema validation so
            // unwrap-style access via `as_str()` falls back gracefully.
            let schema = value.get("schema").and_then(|v| v.as_str()).unwrap_or("?");
            let result = value
                .get("decision")
                .and_then(|d| d.get("result"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let executor = value
                .get("execution")
                .and_then(|e| e.get("executor"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let mode = value
                .get("execution")
                .and_then(|e| e.get("mode"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let exec_status = value
                .get("execution")
                .and_then(|e| e.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let policy_hash = value
                .get("policy")
                .and_then(|p| p.get("policy_hash"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let request_hash = value
                .get("request")
                .and_then(|r| r.get("request_hash"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let policy_prefix: String = policy_hash.chars().take(12).collect();
            let request_prefix: String = request_hash.chars().take(12).collect();

            println!("passport: schema:        {schema}");
            println!("passport: decision:      {result}");
            println!("passport: executor:      {executor} (mode={mode}, status={exec_status})");
            println!("passport: policy_hash:   {policy_prefix}…");
            println!("passport: request_hash:  {request_prefix}…");
            println!("passport: structural verify: ok");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("mandate passport verify: {} ({})", e, e.code());
            // Distinguish "schema invalid" from cross-field mismatches
            // for downstream tooling — both map to exit 2 (the spec
            // groups them under "malformed/tampered/inconsistent"), but
            // the eprintln above carries the explicit code.
            match e {
                CapsuleVerifyError::SchemaInvalid(_)
                | CapsuleVerifyError::DenyWithExecution { .. }
                | CapsuleVerifyError::LiveWithoutEvidence
                | CapsuleVerifyError::MockWithLiveEvidence
                | CapsuleVerifyError::RequestHashMismatch { .. }
                | CapsuleVerifyError::PolicyHashMismatch { .. }
                | CapsuleVerifyError::DecisionResultMismatch { .. }
                | CapsuleVerifyError::AgentIdMismatch { .. }
                | CapsuleVerifyError::AuditEventIdMismatch { .. }
                | CapsuleVerifyError::CheckpointEventHashMismatch { .. }
                | CapsuleVerifyError::Malformed { .. } => ExitCode::from(2),
            }
        }
    }
}
