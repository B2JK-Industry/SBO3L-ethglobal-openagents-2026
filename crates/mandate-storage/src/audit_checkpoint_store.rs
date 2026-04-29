//! Persistent **mock-anchored** audit checkpoints (PSM-A4).
//!
//! Backs `mandate audit checkpoint {create,verify}`. A checkpoint
//! captures the audit chain's tip at a moment in time — sequence,
//! latest event id + hash, plus an aggregated `chain_digest` over
//! every event_hash in the prefix — and stamps it with a
//! deterministic `mock_anchor_ref` that simulates the *shape* of an
//! on-chain anchor without ever leaving the process.
//!
//! Truthfulness rules:
//!
//! - This is **mock** anchoring, NOT real on-chain anchoring. The
//!   `mock_anchor_ref` is a 64-bit content-derived id rendered as
//!   `local-mock-anchor-<16 hex>` (8 bytes hex-encoded). A real anchor
//!   would be e.g. a Merkle root committed to an L2 contract or an
//!   Ethereum tx hash broadcast to a public chain.
//!   `migrations/V007__audit_checkpoints.sql` carries a comment that
//!   says `<8 hex>` — that string is misleading (the actual ref is
//!   16 hex chars), but the comment is part of the SQL bytes hashed
//!   into `schema_migrations.sha256`. Editing it post-merge would
//!   trip the migration-drift detector against any DB that has
//!   already applied V007, so we keep the SQL comment as-is and
//!   rely on this module +
//!   `crates/mandate-cli/src/audit_checkpoint.rs` +
//!   `docs/cli/audit-checkpoint.md` as the source of truth on the
//!   ref's actual length.
//! - `chain_digest` is `SHA-256(event_hash[0] || event_hash[1] ||
//!   … || event_hash[N-1])` over the chain prefix through `sequence`.
//!   That makes the whole prefix verifiable from a single 32-byte
//!   commitment without depending on the audit-event hash linkage.
//!   A consumer that already trusts the audit-event hashes can
//!   re-derive `chain_digest` by reading the same prefix.
//! - Checkpoints are append-only at the storage layer; there is no
//!   `audit_checkpoint_delete` API. The CLI surface enforces the
//!   `mock-anchor:` prefix on every line for loud disclosure.

use chrono::{DateTime, Utc};
use rusqlite::params;
use sha2::{Digest, Sha256};

use crate::error::{StorageError, StorageResult};
use crate::Storage;

/// One row of the `audit_checkpoints` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditCheckpointRecord {
    pub id: i64,
    pub sequence: u64,
    pub latest_event_id: String,
    pub latest_event_hash: String,
    pub chain_digest: String,
    pub mock_anchor_ref: String,
    pub created_at: DateTime<Utc>,
}

/// Compute the chain digest = SHA-256 over the concatenation of
/// `event_hash` bytes from the chain prefix, in seq order. Public so
/// the CLI can re-derive it on `verify --db <path>` and compare
/// against the persisted/exported value.
pub fn compute_chain_digest(event_hashes_hex: &[String]) -> StorageResult<String> {
    let mut hasher = Sha256::new();
    for h in event_hashes_hex {
        // Each `event_hash` is 64 hex chars (32 bytes). Hash the
        // raw bytes, not the hex string, so the digest is over the
        // semantic content, not its encoding. Defensive parse —
        // surface a clear error if the chain has a malformed hash.
        let bytes = hex::decode(h).map_err(|e| {
            StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "audit_checkpoint: malformed event_hash {h:?}: {e}"
            )))
        })?;
        if bytes.len() != 32 {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!(
                    "audit_checkpoint: event_hash must be 32 bytes; got {} for {h:?}",
                    bytes.len()
                ),
            )));
        }
        hasher.update(&bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Derive a deterministic mock-anchor reference from the checkpoint
/// content. The leading `local-mock-anchor-` prefix is mandatory for
/// disclosure; the 16-hex tail is `SHA-256(chain_digest || sequence
/// || created_at)[0..8]`. Same prefix → same ref, so re-creating the
/// same checkpoint twice would collide on the UNIQUE constraint
/// (intentional — the storage layer refuses duplicates).
pub fn mock_anchor_ref(chain_digest: &str, sequence: u64, created_at: &DateTime<Utc>) -> String {
    let mut h = Sha256::new();
    h.update(chain_digest.as_bytes());
    h.update(sequence.to_be_bytes());
    h.update(created_at.to_rfc3339().as_bytes());
    let digest = h.finalize();
    let tail: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("local-mock-anchor-{tail}")
}

impl Storage {
    /// Create a new checkpoint covering the chain prefix through the
    /// current chain tip. Caller supplies the pre-computed
    /// `chain_digest` and `created_at`; storage records both verbatim
    /// so a verifier sees exactly what was committed.
    ///
    /// Refuses to create against an empty audit chain (no chain tip
    /// to commit to) — this surfaces the operationally-honest
    /// "nothing to checkpoint yet" state. Returns `StorageError::Sqlite`
    /// wrapping `QueryReturnedNoRows`.
    pub fn audit_checkpoint_create(
        &mut self,
        chain_digest: &str,
        created_at: DateTime<Utc>,
    ) -> StorageResult<AuditCheckpointRecord> {
        let last = self
            .audit_last()?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))?;
        let sequence = last.event.seq;
        let mock_anchor_ref = mock_anchor_ref(chain_digest, sequence, &created_at);

        self.conn.execute(
            "INSERT INTO audit_checkpoints
             (sequence, latest_event_id, latest_event_hash, chain_digest,
              mock_anchor_ref, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                sequence as i64,
                last.event.id,
                last.event_hash,
                chain_digest,
                mock_anchor_ref,
                created_at.to_rfc3339(),
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(AuditCheckpointRecord {
            id,
            sequence,
            latest_event_id: last.event.id,
            latest_event_hash: last.event_hash,
            chain_digest: chain_digest.to_string(),
            mock_anchor_ref,
            created_at,
        })
    }

    /// Total number of checkpoints currently stored. Used by `mandate
    /// doctor` to decide between `skip` (table empty) and `ok`
    /// (rows present).
    pub fn audit_checkpoint_count(&self) -> StorageResult<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM audit_checkpoints", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Most recent checkpoint, or `None` if none has been created.
    pub fn audit_checkpoint_latest(&self) -> StorageResult<Option<AuditCheckpointRecord>> {
        let result = self.conn.query_row(
            "SELECT id, sequence, latest_event_id, latest_event_hash,
                    chain_digest, mock_anchor_ref, created_at
             FROM audit_checkpoints ORDER BY id DESC LIMIT 1",
            [],
            row_to_record,
        );
        match result {
            Ok(rec) => Ok(Some(rec)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// All checkpoints in ascending `id` order.
    pub fn audit_checkpoint_list(&self) -> StorageResult<Vec<AuditCheckpointRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, sequence, latest_event_id, latest_event_hash,
                    chain_digest, mock_anchor_ref, created_at
             FROM audit_checkpoints ORDER BY id ASC",
        )?;
        let iter = stmt.query_map([], row_to_record)?;
        let mut rows = Vec::new();
        for r in iter {
            rows.push(r?);
        }
        Ok(rows)
    }

    /// Look up by mock-anchor reference. Returns `None` if no row
    /// matches — the verifier uses this to confirm a checkpoint
    /// artifact was actually issued by *this* DB (not just a JSON
    /// file someone forged with a syntactically-valid ref).
    pub fn audit_checkpoint_by_anchor_ref(
        &self,
        anchor_ref: &str,
    ) -> StorageResult<Option<AuditCheckpointRecord>> {
        let result = self.conn.query_row(
            "SELECT id, sequence, latest_event_id, latest_event_hash,
                    chain_digest, mock_anchor_ref, created_at
             FROM audit_checkpoints WHERE mock_anchor_ref = ?1",
            params![anchor_ref],
            row_to_record,
        );
        match result {
            Ok(rec) => Ok(Some(rec)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// Helper: read every event_hash from the chain in seq order so
    /// callers can re-derive a chain digest without owning the
    /// `SignedAuditEvent` decoder. Used by both
    /// `audit_checkpoint_create` (via the CLI which supplies the
    /// digest itself) and `mandate audit checkpoint verify --db`.
    pub fn audit_event_hashes_in_order(&self) -> StorageResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT event_hash FROM audit_events ORDER BY seq ASC")?;
        let iter = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for h in iter {
            out.push(h?);
        }
        Ok(out)
    }
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<AuditCheckpointRecord> {
    let ts: String = r.get(6)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&ts)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Utc);
    Ok(AuditCheckpointRecord {
        id: r.get(0)?,
        sequence: r.get::<_, i64>(1)? as u64,
        latest_event_id: r.get(2)?,
        latest_event_hash: r.get(3)?,
        chain_digest: r.get(4)?,
        mock_anchor_ref: r.get(5)?,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_store::NewAuditEvent;
    use mandate_core::signer::DevSigner;

    fn fresh_storage_with_events(n: u64) -> Storage {
        let mut s = Storage::open_in_memory().unwrap();
        let signer = DevSigner::from_seed("checkpoint-test-signer", [9u8; 32]);
        for i in 0..n {
            s.audit_append(
                NewAuditEvent::now("policy_decided", "checkpoint-test", format!("subj-{i}")),
                &signer,
            )
            .unwrap();
        }
        s
    }

    #[test]
    fn create_against_empty_chain_is_refused() {
        let mut s = Storage::open_in_memory().unwrap();
        let err = s
            .audit_checkpoint_create("0".repeat(64).as_str(), Utc::now())
            .expect_err("must refuse empty-chain checkpoint");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("rows") || msg.contains("query") || msg.contains("returned"),
            "expected QueryReturnedNoRows-shaped error; got: {msg}"
        );
    }

    #[test]
    fn create_after_appending_events_succeeds_and_records_tip() {
        let mut s = fresh_storage_with_events(3);
        let hashes = s.audit_event_hashes_in_order().unwrap();
        let digest = compute_chain_digest(&hashes).unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-28T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let rec = s.audit_checkpoint_create(&digest, now).unwrap();
        assert_eq!(rec.sequence, 3);
        assert_eq!(rec.chain_digest, digest);
        assert!(rec.mock_anchor_ref.starts_with("local-mock-anchor-"));
        assert_eq!(rec.mock_anchor_ref.len(), "local-mock-anchor-".len() + 16);
        assert_eq!(s.audit_checkpoint_count().unwrap(), 1);
    }

    #[test]
    fn chain_digest_is_deterministic_and_changes_when_chain_grows() {
        let mut s = fresh_storage_with_events(2);
        let d1 = compute_chain_digest(&s.audit_event_hashes_in_order().unwrap()).unwrap();
        let signer = DevSigner::from_seed("checkpoint-test-signer", [9u8; 32]);
        s.audit_append(
            NewAuditEvent::now("policy_decided", "checkpoint-test", "subj-x"),
            &signer,
        )
        .unwrap();
        let d2 = compute_chain_digest(&s.audit_event_hashes_in_order().unwrap()).unwrap();
        assert_ne!(d1, d2, "appending an event must change the chain digest");
        // Idempotency: recomputing without changing the chain yields the same digest.
        let d2_again = compute_chain_digest(&s.audit_event_hashes_in_order().unwrap()).unwrap();
        assert_eq!(d2, d2_again);
    }

    #[test]
    fn duplicate_anchor_ref_is_rejected() {
        // Same content + same created_at → same mock_anchor_ref → UNIQUE
        // constraint fires. Tests the truthfulness invariant: you
        // cannot accidentally checkpoint the same prefix twice and
        // pretend they're separate anchors.
        let mut s = fresh_storage_with_events(2);
        let hashes = s.audit_event_hashes_in_order().unwrap();
        let digest = compute_chain_digest(&hashes).unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-28T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        s.audit_checkpoint_create(&digest, now).unwrap();
        let err = s
            .audit_checkpoint_create(&digest, now)
            .expect_err("duplicate anchor_ref must be rejected");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("unique") || msg.contains("constraint"),
            "expected UNIQUE constraint failure; got: {msg}"
        );
    }

    #[test]
    fn list_round_trip_and_lookup_by_anchor_ref() {
        let mut s = fresh_storage_with_events(2);
        let hashes = s.audit_event_hashes_in_order().unwrap();
        let digest = compute_chain_digest(&hashes).unwrap();
        let t1 = chrono::DateTime::parse_from_rfc3339("2026-04-28T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let t2 = chrono::DateTime::parse_from_rfc3339("2026-04-28T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let r1 = s.audit_checkpoint_create(&digest, t1).unwrap();
        let r2 = s.audit_checkpoint_create(&digest, t2).unwrap();
        let all = s.audit_checkpoint_list().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, r1.id);
        assert_eq!(all[1].id, r2.id);
        let latest = s.audit_checkpoint_latest().unwrap().unwrap();
        assert_eq!(latest.id, r2.id);
        let by_ref = s
            .audit_checkpoint_by_anchor_ref(&r1.mock_anchor_ref)
            .unwrap()
            .unwrap();
        assert_eq!(by_ref, r1);
        let missing = s
            .audit_checkpoint_by_anchor_ref("local-mock-anchor-deadbeefdeadbeef")
            .unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn checkpoint_persists_across_storage_reopen() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let digest;
        let id;
        {
            let mut s = Storage::open(&path).unwrap();
            let signer = DevSigner::from_seed("checkpoint-persist-signer", [4u8; 32]);
            s.audit_append(
                NewAuditEvent::now("policy_decided", "p-test", "subj-1"),
                &signer,
            )
            .unwrap();
            let hashes = s.audit_event_hashes_in_order().unwrap();
            digest = compute_chain_digest(&hashes).unwrap();
            let rec = s.audit_checkpoint_create(&digest, Utc::now()).unwrap();
            id = rec.id;
        }
        let s = Storage::open(&path).unwrap();
        let got = s.audit_checkpoint_latest().unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.chain_digest, digest);
    }
}
