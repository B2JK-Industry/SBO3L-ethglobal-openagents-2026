//! Audit log persistence + chain verification.

use chrono::{DateTime, Utc};
use rusqlite::params;

use mandate_core::audit::{verify_chain, AuditEvent, SignedAuditEvent, ZERO_HASH};
use mandate_core::receipt::{EmbeddedSignature, SignatureAlgorithm};
use mandate_core::signer::DevSigner;

use crate::error::{StorageError, StorageResult};
use crate::Storage;

const SELECT_AUDIT_BY_SEQ: &str = "SELECT seq, id, ts, type, actor, subject_id, payload_hash, \
                                   metadata_json, policy_version, policy_hash, attestation_ref, \
                                   prev_event_hash, event_hash, signature_alg, signature_key_id, \
                                   signature_hex FROM audit_events WHERE seq = ?1";

const SELECT_AUDIT_ALL_ORDERED: &str =
    "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, policy_version, \
     policy_hash, attestation_ref, prev_event_hash, event_hash, signature_alg, \
     signature_key_id, signature_hex FROM audit_events ORDER BY seq ASC";

const SELECT_AUDIT_LAST: &str =
    "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, policy_version, \
     policy_hash, attestation_ref, prev_event_hash, event_hash, signature_alg, \
     signature_key_id, signature_hex FROM audit_events ORDER BY seq DESC LIMIT 1";

fn row_to_signed_audit_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<SignedAuditEvent> {
    let metadata_json: String = r.get(7)?;
    let metadata: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&metadata_json)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
        })?;
    let ts: String = r.get(2)?;
    let ts_parsed = chrono::DateTime::parse_from_rfc3339(&ts)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
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
}

#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub event_type: String,
    pub actor: String,
    pub subject_id: String,
    pub payload_hash: String,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub policy_version: Option<u32>,
    pub policy_hash: Option<String>,
    pub attestation_ref: Option<String>,
    pub ts: DateTime<Utc>,
}

impl NewAuditEvent {
    pub fn now(
        event_type: impl Into<String>,
        actor: impl Into<String>,
        subject_id: impl Into<String>,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            actor: actor.into(),
            subject_id: subject_id.into(),
            payload_hash: ZERO_HASH.to_string(),
            metadata: serde_json::Map::new(),
            policy_version: None,
            policy_hash: None,
            attestation_ref: None,
            ts: Utc::now(),
        }
    }
}

impl Storage {
    pub fn audit_count(&self) -> StorageResult<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Fetch the most recent audit event (highest `seq`) with a single query.
    ///
    /// Previously this was a `SELECT seq` followed by `audit_get(seq)`; the
    /// extra round-trip is unnecessary because the same row mapper accepts
    /// every column we need. `audit_list` was migrated to single-query in
    /// `8809f48`; this brings `audit_last` in line with that change.
    pub fn audit_last(&self) -> StorageResult<Option<SignedAuditEvent>> {
        let mut stmt = self.conn.prepare(SELECT_AUDIT_LAST)?;
        match stmt.query_row([], row_to_signed_audit_event) {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn audit_get(&self, seq: u64) -> StorageResult<SignedAuditEvent> {
        let mut stmt = self.conn.prepare(SELECT_AUDIT_BY_SEQ)?;
        let row = stmt.query_row(params![seq as i64], row_to_signed_audit_event)?;
        Ok(row)
    }

    /// Fetch the entire audit log in seq order with a single query.
    pub fn audit_list(&self) -> StorageResult<Vec<SignedAuditEvent>> {
        let mut stmt = self.conn.prepare(SELECT_AUDIT_ALL_ORDERED)?;
        let rows = stmt
            .query_map([], row_to_signed_audit_event)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Fetch the chain prefix from genesis (seq=1) up to and including the
    /// event with the given `event_id`, in seq order. Returns
    /// `StorageError::AuditEventNotFound` if no such event exists in the log.
    ///
    /// The slice this returns is exactly what an `audit_bundle::AuditBundle`
    /// needs as its `audit_chain_segment` for a receipt that points at
    /// `event_id` — it includes every prev_event_hash predecessor needed for
    /// chain verification, and stops at the receipt's referenced event so
    /// the bundle is no larger than the proof requires.
    pub fn audit_chain_prefix_through(
        &self,
        event_id: &str,
    ) -> StorageResult<Vec<SignedAuditEvent>> {
        let chain = self.audit_list()?;
        let position = chain
            .iter()
            .position(|e| e.event.id == event_id)
            .ok_or_else(|| StorageError::AuditEventNotFound {
                id: event_id.to_string(),
            })?;
        let mut prefix = chain;
        prefix.truncate(position + 1);
        Ok(prefix)
    }

    pub fn audit_append(
        &mut self,
        new_event: NewAuditEvent,
        signer: &DevSigner,
    ) -> StorageResult<SignedAuditEvent> {
        let last = self.audit_last()?;
        let next_seq = last.as_ref().map(|e| e.event.seq + 1).unwrap_or(1);
        let prev_hash = last
            .map(|e| e.event_hash)
            .unwrap_or_else(|| ZERO_HASH.to_string());
        let event = AuditEvent {
            version: 1,
            seq: next_seq,
            id: format!("evt-{}", ulid::Ulid::new()),
            ts: new_event.ts,
            event_type: new_event.event_type,
            actor: new_event.actor,
            subject_id: new_event.subject_id,
            payload_hash: new_event.payload_hash,
            metadata: new_event.metadata,
            policy_version: new_event.policy_version,
            policy_hash: new_event.policy_hash,
            attestation_ref: new_event.attestation_ref,
            prev_event_hash: prev_hash,
        };
        let signed = SignedAuditEvent::sign(event, signer)?;
        self.conn.execute(
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
        Ok(signed)
    }

    pub fn audit_verify(&self, verifying_key_hex: Option<&str>) -> StorageResult<()> {
        let events = self.audit_list()?;
        verify_chain(&events, true, verifying_key_hex).map_err(StorageError::Chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_two_events_and_verify() {
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);

        let _e1 = s
            .audit_append(
                NewAuditEvent::now("runtime_started", "mandate-server", "runtime"),
                &signer,
            )
            .unwrap();
        let _e2 = s
            .audit_append(
                NewAuditEvent::now("config_loaded", "mandate-server", "config"),
                &signer,
            )
            .unwrap();

        assert_eq!(s.audit_count().unwrap(), 2);
        s.audit_verify(Some(&signer.verifying_key_hex())).unwrap();
    }

    #[test]
    fn audit_last_returns_none_for_empty_chain() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.audit_last().unwrap().is_none());
    }

    #[test]
    fn audit_last_returns_highest_seq_after_appends() {
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        let _e1 = s
            .audit_append(
                NewAuditEvent::now("runtime_started", "mandate-server", "runtime"),
                &signer,
            )
            .unwrap();
        let e2 = s
            .audit_append(
                NewAuditEvent::now("config_loaded", "mandate-server", "config"),
                &signer,
            )
            .unwrap();
        let last = s.audit_last().unwrap().expect("last event present");
        assert_eq!(last.event.seq, 2);
        assert_eq!(last.event_hash, e2.event_hash);
        assert_eq!(last.event.event_type, "config_loaded");
    }

    #[test]
    fn migrations_are_idempotent() {
        let s = Storage::open_in_memory().unwrap();
        // Re-running migrate via reopening reuses the in-memory DB, so reopen
        // signals via the migrate path only on a fresh db. Instead, assert
        // schema_migrations is populated.
        let n: i64 = s
            .conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert!(n >= 1, "at least one migration applied");
    }

    #[test]
    fn audit_chain_prefix_through_returns_correct_slice() {
        // The audit-bundle DB-backed export needs everything from genesis
        // through the receipt's referenced event — no more, no less. Pin
        // the slice contents by id and length on a 3-event chain.
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        let e1 = s
            .audit_append(
                NewAuditEvent::now("runtime_started", "mandate-server", "runtime"),
                &signer,
            )
            .unwrap();
        let e2 = s
            .audit_append(
                NewAuditEvent::now("policy_decided", "policy_engine", "pr-001"),
                &signer,
            )
            .unwrap();
        let _e3 = s
            .audit_append(
                NewAuditEvent::now("policy_decided", "policy_engine", "pr-002"),
                &signer,
            )
            .unwrap();

        // Slice through the middle event must be exactly [genesis, middle].
        let prefix = s.audit_chain_prefix_through(&e2.event.id).unwrap();
        assert_eq!(prefix.len(), 2);
        assert_eq!(prefix[0].event.id, e1.event.id);
        assert_eq!(prefix[1].event.id, e2.event.id);

        // Slicing through the last event returns the entire chain.
        let full = s.audit_chain_prefix_through(&_e3.event.id).unwrap();
        assert_eq!(full.len(), 3);
    }

    #[test]
    fn audit_chain_prefix_through_returns_not_found_for_unknown_id() {
        // The DB-backed export must fail clearly when a receipt points at
        // an event id the daemon never wrote. Carries the bad id in the
        // error so the CLI can echo it back to the user verbatim.
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        s.audit_append(
            NewAuditEvent::now("runtime_started", "mandate-server", "runtime"),
            &signer,
        )
        .unwrap();
        let err = s
            .audit_chain_prefix_through("evt-DOES-NOT-EXIST")
            .expect_err("must fail when id is missing");
        match err {
            StorageError::AuditEventNotFound { id } => assert_eq!(id, "evt-DOES-NOT-EXIST"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
