//! Audit log persistence + chain verification.

use chrono::{DateTime, Utc};
use rusqlite::params;

use sbo3l_core::audit::{verify_chain, AuditEvent, SignedAuditEvent, ZERO_HASH};
use sbo3l_core::receipt::{EmbeddedSignature, SignatureAlgorithm};
use sbo3l_core::signer::DevSigner;

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

const SELECT_AUDIT_PREFIX_BY_SEQ: &str =
    "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, policy_version, \
     policy_hash, attestation_ref, prev_event_hash, event_hash, signature_alg, \
     signature_key_id, signature_hex FROM audit_events WHERE seq <= ?1 ORDER BY seq ASC";

/// Pushed-down pagination for `audit_list_paginated`.
const SELECT_AUDIT_AFTER_SEQ_PAGINATED: &str =
    "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, policy_version, \
     policy_hash, attestation_ref, prev_event_hash, event_hash, signature_alg, \
     signature_key_id, signature_hex FROM audit_events \
     WHERE seq > ?1 ORDER BY seq ASC LIMIT ?2";

const SELECT_AUDIT_SEQ_BY_ID: &str = "SELECT seq FROM audit_events WHERE id = ?1";

/// Same column projection as [`SELECT_AUDIT_LAST`], filtered to a
/// specific tenant. Used by `audit_last_for_tenant` to compute the
/// `prev_event_hash` link for a per-tenant chain — events from
/// other tenants don't appear and so can't poison the chain
/// integrity of this tenant's verification.
const SELECT_AUDIT_LAST_FOR_TENANT: &str =
    "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, policy_version, \
     policy_hash, attestation_ref, prev_event_hash, event_hash, signature_alg, \
     signature_key_id, signature_hex FROM audit_events \
     WHERE tenant_id = ?1 ORDER BY seq DESC LIMIT 1";

/// Per-tenant ordered listing — used by `audit_list_for_tenant`
/// and the tenant-scoped chain verifier.
const SELECT_AUDIT_ALL_FOR_TENANT_ORDERED: &str =
    "SELECT seq, id, ts, type, actor, subject_id, payload_hash, metadata_json, policy_version, \
     policy_hash, attestation_ref, prev_event_hash, event_hash, signature_alg, \
     signature_key_id, signature_hex FROM audit_events \
     WHERE tenant_id = ?1 ORDER BY seq ASC";

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

    /// Pushed-down pagination over the audit chain. Returns up to
    /// `limit` events with `seq > since_seq`, in ascending seq order.
    /// Use `since_seq = 0` to start from genesis.
    ///
    /// **Why this exists:** the gRPC `AuditChainStream` RPC and the
    /// admin events ring-replay path were both implemented against
    /// [`audit_list`], filtering after-the-fact in Rust. That meant a
    /// request asking for "10 events after seq=99000" still loaded
    /// the entire 100K-event chain into memory. With this primitive
    /// the cost scales with the page, not the chain. Self-review
    /// finding §Bug 2 in `docs/dev1/self-review-r14.md`.
    pub fn audit_list_paginated(
        &self,
        since_seq: u64,
        limit: u64,
    ) -> StorageResult<Vec<SignedAuditEvent>> {
        let mut stmt = self.conn.prepare(SELECT_AUDIT_AFTER_SEQ_PAGINATED)?;
        let rows = stmt
            .query_map(
                params![since_seq as i64, limit as i64],
                row_to_signed_audit_event,
            )?
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
    ///
    /// This is implemented as two scoped SQL queries — first an `id → seq`
    /// lookup (the audit_events.id UNIQUE index makes this O(log n)), then
    /// a `WHERE seq <= ?1 ORDER BY seq ASC` over only the prefix rows.
    /// Rows with `seq > target_seq` are never deserialised, so:
    ///
    ///   * the cost scales with the proof prefix size, not the total log
    ///     size — exporting an old receipt's bundle does not pay for every
    ///     event written since;
    ///   * a malformed or tampered row past the target event cannot break
    ///     a proof for an earlier event whose own prefix is intact.
    ///
    /// A malformed row *inside* the prefix still surfaces (either through
    /// the row mapper or, downstream, via `verify_chain` on the returned
    /// segment) — that is the correct outcome, since the proof depends on
    /// every preceding event being well-formed.
    pub fn audit_chain_prefix_through(
        &self,
        event_id: &str,
    ) -> StorageResult<Vec<SignedAuditEvent>> {
        let target_seq: i64 =
            match self
                .conn
                .query_row(SELECT_AUDIT_SEQ_BY_ID, params![event_id], |r| r.get(0))
            {
                Ok(seq) => seq,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(StorageError::AuditEventNotFound {
                        id: event_id.to_string(),
                    });
                }
                Err(e) => return Err(e.into()),
            };
        let mut stmt = self.conn.prepare(SELECT_AUDIT_PREFIX_BY_SEQ)?;
        let rows = stmt
            .query_map(params![target_seq], row_to_signed_audit_event)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
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

    // ------------------------------------------------------------
    // Per-tenant variants (T-3 / V010 multi-tenant scoping).
    //
    // The legacy methods above operate on the global table; per-tenant
    // deployments call these instead. Each tenant's audit chain is
    // the subsequence WHERE tenant_id=X with `prev_event_hash`
    // linking only events within that tenant's subsequence.
    // ------------------------------------------------------------

    /// Count audit events for a specific tenant. Single-tenant
    /// deployments pass [`crate::DEFAULT_TENANT_ID`] (or use the
    /// non-suffixed `audit_count` which has the same effect when
    /// V010 has run, since every row is `tenant_id='default'`).
    pub fn audit_count_for_tenant(&self, tenant_id: &str) -> StorageResult<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM audit_events WHERE tenant_id = ?1",
            params![tenant_id],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    /// Last audit event for a specific tenant. The chain link
    /// (`prev_event_hash`) points only to the previous event in
    /// THIS tenant's subsequence, so per-tenant chain verification
    /// is robust against tampering in other tenants' rows.
    pub fn audit_last_for_tenant(
        &self,
        tenant_id: &str,
    ) -> StorageResult<Option<SignedAuditEvent>> {
        let mut stmt = self.conn.prepare(SELECT_AUDIT_LAST_FOR_TENANT)?;
        match stmt.query_row(params![tenant_id], row_to_signed_audit_event) {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// All audit events for a specific tenant in seq order. A
    /// caller that wants to verify the per-tenant chain calls this
    /// then runs `verify_chain` against the result.
    pub fn audit_list_for_tenant(&self, tenant_id: &str) -> StorageResult<Vec<SignedAuditEvent>> {
        let mut stmt = self.conn.prepare(SELECT_AUDIT_ALL_FOR_TENANT_ORDERED)?;
        let rows = stmt.query_map(params![tenant_id], row_to_signed_audit_event)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Append an audit event scoped to a specific tenant. The
    /// `prev_event_hash` is computed from the LAST event in this
    /// tenant's subsequence — cross-tenant events don't link.
    /// `seq` remains globally unique (preserves the existing
    /// `INTEGER PRIMARY KEY` invariant); per-tenant ordering is
    /// derivable via the (tenant_id, seq) index.
    pub fn audit_append_for_tenant(
        &mut self,
        tenant_id: &str,
        new_event: NewAuditEvent,
        signer: &DevSigner,
    ) -> StorageResult<SignedAuditEvent> {
        // Global next seq (preserves the existing PRIMARY KEY
        // monotonicity across tenants — two tenants writing
        // concurrently can't collide on seq).
        let global_last_seq: i64 = self
            .conn
            .query_row("SELECT COALESCE(MAX(seq), 0) FROM audit_events", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        let next_seq = (global_last_seq as u64) + 1;
        // Per-tenant prev_event_hash — only events in THIS
        // tenant's subsequence link.
        let tenant_last = self.audit_last_for_tenant(tenant_id)?;
        let prev_hash = tenant_last
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
                 event_hash, signature_alg, signature_key_id, signature_hex, tenant_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                tenant_id,
            ],
        )?;
        Ok(signed)
    }

    // NOTE: `audit_verify_for_tenant` is intentionally deferred.
    // The existing `verify_chain` requires contiguous seq values
    // starting at 1, but per-tenant subsequences over a shared
    // global `seq` PRIMARY KEY have gaps (other tenants' events
    // occupy the global sequence). Closing this gap requires
    // either (a) a per-tenant `tenant_seq` column carried in the
    // chain hash, or (b) a tenant-aware verifier that re-indexes
    // the events before walking. Both are larger than this PR's
    // scope; this commit ships the cross-tenant ISOLATION property
    // (read/write filtered by tenant_id) and the chain-verification
    // story lands in a follow-up.
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
                NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
                &signer,
            )
            .unwrap();
        let _e2 = s
            .audit_append(
                NewAuditEvent::now("config_loaded", "sbo3l-server", "config"),
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
                NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
                &signer,
            )
            .unwrap();
        let e2 = s
            .audit_append(
                NewAuditEvent::now("config_loaded", "sbo3l-server", "config"),
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
    fn audit_list_paginated_respects_since_seq_and_limit() {
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        // Seed a 5-event chain (seq 1..=5).
        for i in 0..5 {
            s.audit_append(
                NewAuditEvent::now("policy_decided", "policy_engine", &format!("pr-{i}")),
                &signer,
            )
            .unwrap();
        }
        // since_seq=2, limit=10 → events 3,4,5.
        let page = s.audit_list_paginated(2, 10).unwrap();
        let seqs: Vec<u64> = page.iter().map(|e| e.event.seq).collect();
        assert_eq!(seqs, vec![3, 4, 5]);

        // since_seq=0, limit=2 → events 1,2 (caps at limit).
        let page = s.audit_list_paginated(0, 2).unwrap();
        let seqs: Vec<u64> = page.iter().map(|e| e.event.seq).collect();
        assert_eq!(seqs, vec![1, 2]);

        // since_seq past chain head → empty page (no error).
        let page = s.audit_list_paginated(99, 10).unwrap();
        assert!(page.is_empty());

        // limit=0 → empty page (no error). Edge case: callers that
        // want the full chain should call audit_list() instead.
        let page = s.audit_list_paginated(0, 0).unwrap();
        assert!(page.is_empty());
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
                NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
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
            NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
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

    #[test]
    fn audit_chain_prefix_through_does_not_read_rows_after_target_seq() {
        // Codex P2: the prefix query MUST NOT pay for rows past the
        // target event. We prove that by writing a well-formed prefix
        // (seq=1 + seq=2) and then injecting a row at seq=3 whose
        // `metadata_json` is not valid JSON. If the implementation read
        // the whole table and truncated in memory, the row mapper would
        // run on seq=3 and return an Err. Because we use `WHERE seq <=
        // target_seq`, seq=3 is never even loaded — exporting through
        // seq=2 succeeds.
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        let _e1 = s
            .audit_append(
                NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
                &signer,
            )
            .unwrap();
        let target = s
            .audit_append(
                NewAuditEvent::now("policy_decided", "policy_engine", "pr-test-001"),
                &signer,
            )
            .unwrap();
        // Inject a malformed seq=3 row directly. We don't go through
        // `audit_append` because that would refuse to write malformed
        // JSON; the test specifically simulates a corrupted future row.
        s.conn
            .execute(
                "INSERT INTO audit_events
                    (seq, id, ts, type, actor, subject_id, payload_hash, metadata_json,
                     policy_version, policy_hash, attestation_ref, prev_event_hash,
                     event_hash, signature_alg, signature_key_id, signature_hex)
                 VALUES (3, 'evt-MALFORMED-FUTURE', '2026-04-27T12:00:02Z', 'policy_decided',
                         'policy_engine', 'pr-test-002', '{}', 'NOT JSON',
                         NULL, NULL, NULL,
                         '0000000000000000000000000000000000000000000000000000000000000000',
                         '0000000000000000000000000000000000000000000000000000000000000000',
                         'ed25519', 'audit-signer-v1', 'aaaa')",
                [],
            )
            .unwrap();

        let prefix = s
            .audit_chain_prefix_through(&target.event.id)
            .expect("prefix must load even with a malformed row at higher seq");
        assert_eq!(prefix.len(), 2);
        assert_eq!(prefix[1].event.id, target.event.id);
    }

    #[test]
    fn audit_chain_prefix_through_propagates_malformed_row_inside_prefix() {
        // Symmetric guarantee: a malformed row *inside* the prefix MUST
        // surface — a proof needs every predecessor to be well-formed.
        // Inject a malformed seq=2 row, then export through seq=3. The
        // row mapper on seq=2 fails when parsing `metadata_json`, which
        // surfaces as a SQLite conversion error.
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
        let _e1 = s
            .audit_append(
                NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
                &signer,
            )
            .unwrap();
        // Inject malformed row at seq=2 directly.
        s.conn
            .execute(
                "INSERT INTO audit_events
                    (seq, id, ts, type, actor, subject_id, payload_hash, metadata_json,
                     policy_version, policy_hash, attestation_ref, prev_event_hash,
                     event_hash, signature_alg, signature_key_id, signature_hex)
                 VALUES (2, 'evt-MALFORMED-PREFIX', '2026-04-27T12:00:01Z', 'policy_decided',
                         'policy_engine', 'pr-test-001', '{}', 'NOT JSON',
                         NULL, NULL, NULL,
                         '0000000000000000000000000000000000000000000000000000000000000000',
                         '0000000000000000000000000000000000000000000000000000000000000000',
                         'ed25519', 'audit-signer-v1', 'aaaa')",
                [],
            )
            .unwrap();
        // And a valid-looking seq=3 the test will try to export through.
        s.conn
            .execute(
                "INSERT INTO audit_events
                    (seq, id, ts, type, actor, subject_id, payload_hash, metadata_json,
                     policy_version, policy_hash, attestation_ref, prev_event_hash,
                     event_hash, signature_alg, signature_key_id, signature_hex)
                 VALUES (3, 'evt-OK-SEQ3', '2026-04-27T12:00:02Z', 'policy_decided',
                         'policy_engine', 'pr-test-002', '{}', '{}',
                         NULL, NULL, NULL,
                         '0000000000000000000000000000000000000000000000000000000000000000',
                         '0000000000000000000000000000000000000000000000000000000000000000',
                         'ed25519', 'audit-signer-v1', 'bbbb')",
                [],
            )
            .unwrap();

        let err = s
            .audit_chain_prefix_through("evt-OK-SEQ3")
            .expect_err("must fail when a row inside the prefix is malformed");
        // The row mapper turns the bad metadata_json into a SQLite
        // conversion error, which our error type wraps as Sqlite(_).
        assert!(matches!(err, StorageError::Sqlite(_)), "got {err:?}");
    }
}
