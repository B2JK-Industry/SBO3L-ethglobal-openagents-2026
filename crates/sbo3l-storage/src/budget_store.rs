//! Persistent budget state (F-2).
//!
//! Backs the SQLite-backed `BudgetTracker` in `sbo3l-policy`. Rows persist
//! the committed spend per (agent_id, scope, scope_key) bucket so the cap
//! enforcement survives daemon restart. See migration V008 in
//! `migrations/V008__budget_state.sql` for the schema and column semantics.
//!
//! Two API surfaces:
//!
//! 1. [`Storage::budget_state_get`] / [`Storage::budget_spent_cents`] — pure
//!    reads used by `BudgetTracker::check`. They take `&Storage` (not `&mut`)
//!    so multiple read paths can hold the connection lock concurrently if
//!    that ever becomes the design.
//!
//! 2. [`Storage::finalize_decision`] — the ACID combo used by
//!    `BudgetTracker::commit` on the request path. It opens one rusqlite
//!    transaction, upserts every applicable budget row, appends the audit
//!    event, and commits. A failure at any step rolls back: no partial
//!    writes, no audit row without its budget commit, no budget commit
//!    without its audit row.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use sbo3l_core::audit::{AuditEvent, SignedAuditEvent, ZERO_HASH};
use sbo3l_core::receipt::{EmbeddedSignature, SignatureAlgorithm};
use sbo3l_core::signer::DevSigner;

use crate::audit_store::NewAuditEvent;
use crate::error::{StorageError, StorageResult};
use crate::Storage;

/// One row of `budget_state`. Mirrors the columns 1:1; the `scope` field is
/// the textual repr (`'per_tx' | 'daily' | 'monthly' | 'per_provider'`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetStateRow {
    pub agent_id: String,
    pub scope: String,
    pub scope_key: String,
    pub spent_cents: i64,
    pub cap_cents: i64,
    pub reset_at_unix: Option<i64>,
}

/// One budget bucket to add `delta_cents` to under a single transaction.
/// `cap_cents` and `reset_at_unix` are upserted alongside so a row that
/// already existed with a stale cap is brought forward to the current
/// policy's cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetIncrement {
    pub agent_id: String,
    pub scope: String,
    pub scope_key: String,
    pub delta_cents: i64,
    pub cap_cents: i64,
    pub reset_at_unix: Option<i64>,
}

impl Storage {
    /// Read the row for one bucket. Returns `Ok(None)` if the bucket has
    /// never received a commit (the convention is "missing row = 0 spent").
    pub fn budget_state_get(
        &self,
        agent_id: &str,
        scope: &str,
        scope_key: &str,
    ) -> StorageResult<Option<BudgetStateRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT agent_id, scope, scope_key, spent_cents, cap_cents, reset_at_unix
             FROM budget_state
             WHERE agent_id = ?1 AND scope = ?2 AND scope_key = ?3",
        )?;
        let row = stmt
            .query_row(params![agent_id, scope, scope_key], |r| {
                Ok(BudgetStateRow {
                    agent_id: r.get(0)?,
                    scope: r.get(1)?,
                    scope_key: r.get(2)?,
                    spent_cents: r.get(3)?,
                    cap_cents: r.get(4)?,
                    reset_at_unix: r.get(5)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    /// Convenience read used by `BudgetTracker::check`. Treats a missing row
    /// as 0 spent.
    pub fn budget_spent_cents(
        &self,
        agent_id: &str,
        scope: &str,
        scope_key: &str,
    ) -> StorageResult<i64> {
        Ok(self
            .budget_state_get(agent_id, scope, scope_key)?
            .map(|r| r.spent_cents)
            .unwrap_or(0))
    }

    /// Diagnostic: total number of budget rows currently in the table.
    pub fn budget_state_count(&self) -> StorageResult<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM budget_state", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Atomic combo: append the audit event AND apply every budget
    /// increment under a single transaction. On any error the whole tx is
    /// rolled back.
    ///
    /// This is the production-shape commit path used by
    /// `sbo3l-policy::BudgetTracker::commit` — wrapping policy + budget +
    /// audit in one transaction (F-2 acceptance criterion).
    pub fn finalize_decision(
        &mut self,
        increments: &[BudgetIncrement],
        audit_event: NewAuditEvent,
        audit_signer: &DevSigner,
    ) -> StorageResult<SignedAuditEvent> {
        let tx = self.conn.transaction()?;

        // 1. Upsert every applicable budget row.
        for inc in increments {
            tx.execute(
                "INSERT INTO budget_state
                    (agent_id, scope, scope_key, spent_cents, cap_cents, reset_at_unix)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(agent_id, scope, scope_key) DO UPDATE
                   SET spent_cents   = budget_state.spent_cents + excluded.spent_cents,
                       cap_cents     = excluded.cap_cents,
                       reset_at_unix = excluded.reset_at_unix",
                params![
                    inc.agent_id,
                    inc.scope,
                    inc.scope_key,
                    inc.delta_cents,
                    inc.cap_cents,
                    inc.reset_at_unix,
                ],
            )?;
        }

        // 2. Append the audit event using the same transaction. Inlined
        // from `Storage::audit_append` so the read of `audit_last` and the
        // INSERT both happen under the tx — a parallel writer is blocked
        // by SQLite's database-level write lock until we COMMIT.
        let last = audit_last_via_tx(&tx)?;
        let next_seq = last.as_ref().map(|e| e.event.seq + 1).unwrap_or(1);
        let prev_hash = last
            .map(|e| e.event_hash)
            .unwrap_or_else(|| ZERO_HASH.to_string());
        let event = AuditEvent {
            version: 1,
            seq: next_seq,
            id: format!("evt-{}", ulid::Ulid::new()),
            ts: audit_event.ts,
            event_type: audit_event.event_type,
            actor: audit_event.actor,
            subject_id: audit_event.subject_id,
            payload_hash: audit_event.payload_hash,
            metadata: audit_event.metadata,
            policy_version: audit_event.policy_version,
            policy_hash: audit_event.policy_hash,
            attestation_ref: audit_event.attestation_ref,
            prev_event_hash: prev_hash,
        };
        let signed = SignedAuditEvent::sign(event, audit_signer)?;
        tx.execute(
            "INSERT INTO audit_events
                (seq, id, ts, type, actor, subject_id, payload_hash, metadata_json,
                 policy_version, policy_hash, attestation_ref, prev_event_hash,
                 event_hash, signature_alg, signature_key_id, signature_hex)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                signed.event.seq as i64,
                signed.event.id,
                signed.event.ts.to_rfc3339(),
                signed.event.event_type,
                signed.event.actor,
                signed.event.subject_id,
                signed.event.payload_hash,
                serde_json::Value::Object(signed.event.metadata.clone()).to_string(),
                signed.event.policy_version.map(|v| v as i64),
                signed.event.policy_hash,
                signed.event.attestation_ref,
                signed.event.prev_event_hash,
                signed.event_hash,
                "ed25519",
                signed.signature.key_id,
                signed.signature.signature_hex,
            ],
        )?;

        // 3. Commit. Drop on error rolls back the whole tx automatically.
        tx.commit()?;
        Ok(signed)
    }
}

/// Read the highest-seq audit event using the supplied connection-like (so
/// the read happens inside the caller's transaction). Mirrors
/// `Storage::audit_last` but parameterised on a `Connection` reference.
fn audit_last_via_tx(conn: &rusqlite::Connection) -> StorageResult<Option<SignedAuditEvent>> {
    let mut stmt = conn.prepare(
        "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, \
         policy_version, policy_hash, attestation_ref, prev_event_hash, event_hash, \
         signature_alg, signature_key_id, signature_hex \
         FROM audit_events ORDER BY seq DESC LIMIT 1",
    )?;
    match stmt.query_row([], |r| {
        let metadata_json: String = r.get(7)?;
        let metadata: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&metadata_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let ts: String = r.get(2)?;
        let ts_parsed = chrono::DateTime::parse_from_rfc3339(&ts)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let event = AuditEvent {
            version: 1,
            seq: r.get::<_, i64>(0)? as u64,
            id: r.get(1)?,
            ts: ts_parsed,
            event_type: r.get(3)?,
            actor: r.get(4)?,
            subject_id: r.get(5)?,
            payload_hash: r.get(6)?,
            metadata,
            policy_version: r.get::<_, Option<i64>>(8)?.map(|v| v as u32),
            policy_hash: r.get(9)?,
            attestation_ref: r.get(10)?,
            prev_event_hash: r.get(11)?,
        };
        let signature = EmbeddedSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            key_id: r.get(14)?,
            signature_hex: r.get(15)?,
        };
        let event_hash: String = r.get(12)?;
        let _alg: String = r.get(13)?;
        Ok(SignedAuditEvent {
            event,
            event_hash,
            signature,
        })
    }) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StorageError::Sqlite(e)),
    }
}

/// Best-effort: parse a USD decimal string (e.g. `"0.05"`) to integer cents.
/// Rejects values with > 2 fractional digits and any value that overflows
/// `i64` after scaling. Used by `BudgetTracker` to bridge the policy's
/// `cap_usd: String` field and this table's `cap_cents` column.
pub fn usd_str_to_cents(s: &str) -> Option<i64> {
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    let dec = Decimal::from_str(s).ok()?;
    if dec < Decimal::ZERO {
        return None;
    }
    let scaled = dec * Decimal::new(100, 0);
    if scaled.fract() != Decimal::ZERO {
        // sub-cent precision rejected; F-2 budgets are in whole cents.
        return None;
    }
    scaled.trunc().to_i64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-30T12:00:00Z")
            .unwrap()
            .into()
    }

    fn signer() -> DevSigner {
        DevSigner::from_seed("audit-signer-v1", [11u8; 32])
    }

    fn audit_event() -> NewAuditEvent {
        NewAuditEvent {
            event_type: "policy_decided".to_string(),
            actor: "policy_engine".to_string(),
            subject_id: "pr-test".to_string(),
            payload_hash: "deadbeef".to_string(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some("00".repeat(32)),
            attestation_ref: None,
            ts: ts(),
        }
    }

    #[test]
    fn usd_to_cents_basic() {
        assert_eq!(usd_str_to_cents("0.05"), Some(5));
        assert_eq!(usd_str_to_cents("0.10"), Some(10));
        assert_eq!(usd_str_to_cents("10.00"), Some(1000));
        assert_eq!(usd_str_to_cents("0"), Some(0));
    }

    #[test]
    fn usd_to_cents_rejects_sub_cent_and_negatives() {
        assert_eq!(usd_str_to_cents("0.001"), None);
        assert_eq!(usd_str_to_cents("-1.00"), None);
        assert_eq!(usd_str_to_cents("not-decimal"), None);
    }

    #[test]
    fn budget_state_get_returns_none_for_missing_bucket() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s
            .budget_state_get("research-agent-01", "daily", "2026-04-30")
            .unwrap()
            .is_none());
        assert_eq!(
            s.budget_spent_cents("research-agent-01", "daily", "2026-04-30")
                .unwrap(),
            0
        );
    }

    #[test]
    fn finalize_decision_inserts_budget_row_and_audit_event() {
        let mut s = Storage::open_in_memory().unwrap();
        let inc = BudgetIncrement {
            agent_id: "research-agent-01".to_string(),
            scope: "daily".to_string(),
            scope_key: "2026-04-30".to_string(),
            delta_cents: 5,
            cap_cents: 10,
            reset_at_unix: Some(1714521600),
        };
        let signed = s
            .finalize_decision(&[inc], audit_event(), &signer())
            .unwrap();
        assert_eq!(signed.event.seq, 1);
        assert_eq!(s.audit_count().unwrap(), 1);
        let row = s
            .budget_state_get("research-agent-01", "daily", "2026-04-30")
            .unwrap()
            .unwrap();
        assert_eq!(row.spent_cents, 5);
        assert_eq!(row.cap_cents, 10);
        assert_eq!(row.reset_at_unix, Some(1714521600));
    }

    #[test]
    fn finalize_decision_accumulates_on_repeat_commits() {
        let mut s = Storage::open_in_memory().unwrap();
        let mk = |delta: i64| BudgetIncrement {
            agent_id: "research-agent-01".to_string(),
            scope: "daily".to_string(),
            scope_key: "2026-04-30".to_string(),
            delta_cents: delta,
            cap_cents: 10,
            reset_at_unix: Some(1714521600),
        };
        s.finalize_decision(&[mk(5)], audit_event(), &signer())
            .unwrap();
        s.finalize_decision(&[mk(3)], audit_event(), &signer())
            .unwrap();
        let row = s
            .budget_state_get("research-agent-01", "daily", "2026-04-30")
            .unwrap()
            .unwrap();
        assert_eq!(row.spent_cents, 8);
        assert_eq!(s.audit_count().unwrap(), 2);
    }

    #[test]
    fn finalize_decision_persists_across_storage_reopen() {
        // The reason F-2 exists: a budget commit must survive daemon
        // restart against the same SQLite file.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let mut s = Storage::open(&path).unwrap();
            let inc = BudgetIncrement {
                agent_id: "research-agent-01".to_string(),
                scope: "daily".to_string(),
                scope_key: "2026-04-30".to_string(),
                delta_cents: 7,
                cap_cents: 10,
                reset_at_unix: None,
            };
            s.finalize_decision(&[inc], audit_event(), &signer())
                .unwrap();
        }
        let s = Storage::open(&path).unwrap();
        assert_eq!(
            s.budget_spent_cents("research-agent-01", "daily", "2026-04-30")
                .unwrap(),
            7
        );
    }

    #[test]
    fn finalize_decision_with_no_increments_still_appends_audit_event() {
        // A pure deny has zero increments but still needs a hash-chained
        // audit row. The combo path must accept that.
        let mut s = Storage::open_in_memory().unwrap();
        let signed = s.finalize_decision(&[], audit_event(), &signer()).unwrap();
        assert_eq!(signed.event.seq, 1);
        assert_eq!(s.audit_count().unwrap(), 1);
        assert_eq!(s.budget_state_count().unwrap(), 0);
    }
}
