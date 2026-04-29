//! Persistent storage for HTTP `Idempotency-Key` safe-retry envelopes.
//!
//! Backs `Storage::idempotency_lookup` / `Storage::idempotency_store`. The
//! HTTP daemon hits this table BEFORE the schema validation / nonce gate /
//! policy / budget / audit / signing pipeline on `POST /v1/payment-requests`.
//!
//! Behaviour matrix (server-side):
//! - Same `Idempotency-Key` + same canonical `request_hash` → return the
//!   cached response, never re-run the pipeline (no duplicate audit
//!   events, no duplicate budget commits, no duplicate signed receipts).
//! - Same `Idempotency-Key` + different canonical `request_hash` → 409
//!   `protocol.idempotency_conflict`.
//! - Cached responses are 200-only — failure responses (4xx / 5xx) are
//!   intentionally not cached so a client can retry past a transient
//!   failure through the full pipeline.
//!
//! TTL eviction is deliberately not implemented here; APRP requests carry
//! their own `expires_at` and a future migration can sweep stale rows
//! without affecting safe-retry semantics.

use chrono::{DateTime, Utc};
use rusqlite::{params, ErrorCode};

use crate::error::{StorageError, StorageResult};
use crate::Storage;

/// One row of the `idempotency_keys` table. The HTTP daemon constructs an
/// `IdempotencyEntry` after a successful 200 response and stores it; on
/// retry it reads the same struct back and replays the cached envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyEntry {
    pub key: String,
    pub request_hash: String,
    pub response_status: u16,
    pub response_body: String,
    pub created_at: DateTime<Utc>,
}

impl Storage {
    /// Look up a previously-stored idempotency envelope by client-supplied
    /// `Idempotency-Key`. Returns `Ok(None)` for an unseen key.
    pub fn idempotency_lookup(&self, key: &str) -> StorageResult<Option<IdempotencyEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, request_hash, response_status, response_body, created_at
             FROM idempotency_keys WHERE key = ?1",
        )?;
        match stmt.query_row(params![key], |r| {
            let ts: String = r.get(4)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&ts)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&Utc);
            Ok(IdempotencyEntry {
                key: r.get(0)?,
                request_hash: r.get(1)?,
                response_status: r.get::<_, i64>(2)? as u16,
                response_body: r.get(3)?,
                created_at,
            })
        }) {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomically register an idempotency envelope. Returns:
    /// - `Ok(true)` — the row was new and has been stored.
    /// - `Ok(false)` — a row with the same `key` already exists. The caller
    ///   MUST NOT silently overwrite it; the existing row is the
    ///   authoritative cached response. Most callers will treat this as a
    ///   no-op (the previous winner already stored the response).
    /// - `Err(...)` — any other SQLite error (caller should fail closed).
    pub fn idempotency_try_store(&mut self, entry: &IdempotencyEntry) -> StorageResult<bool> {
        match self.conn.execute(
            "INSERT INTO idempotency_keys
                (key, request_hash, response_status, response_body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entry.key,
                entry.request_hash,
                entry.response_status as i64,
                entry.response_body,
                entry.created_at.to_rfc3339(),
            ],
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == ErrorCode::ConstraintViolation =>
            {
                Ok(false)
            }
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// Diagnostic only — number of cached envelopes currently in the table.
    pub fn idempotency_count(&self) -> StorageResult<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM idempotency_keys", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, hash: &str, body: &str) -> IdempotencyEntry {
        IdempotencyEntry {
            key: key.to_string(),
            request_hash: hash.to_string(),
            response_status: 200,
            response_body: body.to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-04-28T12:00:00Z")
                .unwrap()
                .into(),
        }
    }

    #[test]
    fn lookup_unknown_key_returns_none() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.idempotency_lookup("idem-unknown-XXX").unwrap().is_none());
    }

    #[test]
    fn store_then_lookup_round_trip() {
        let mut s = Storage::open_in_memory().unwrap();
        let e = entry("idem-key-001", "deadbeef", r#"{"status":"auto_approved"}"#);
        assert!(s.idempotency_try_store(&e).unwrap());
        let got = s
            .idempotency_lookup(&e.key)
            .unwrap()
            .expect("row must exist");
        assert_eq!(got, e);
    }

    #[test]
    fn store_with_duplicate_key_returns_false() {
        // PRIMARY KEY on `key` gives atomic insert-or-fail. Two writers
        // racing with the same key both attempt INSERT; exactly one
        // succeeds, the loser surfaces ConstraintViolation which we
        // translate to Ok(false). The first row stays authoritative.
        let mut s = Storage::open_in_memory().unwrap();
        let first = entry("idem-key-002", "aaaa", r#"{"a":1}"#);
        let second = entry("idem-key-002", "bbbb", r#"{"b":2}"#);
        assert!(s.idempotency_try_store(&first).unwrap());
        assert!(!s.idempotency_try_store(&second).unwrap());
        // The first envelope is still what's there.
        let got = s.idempotency_lookup("idem-key-002").unwrap().unwrap();
        assert_eq!(got.request_hash, "aaaa");
        assert_eq!(got.response_body, r#"{"a":1}"#);
    }

    #[test]
    fn idempotency_persists_across_storage_reopen() {
        // The whole point of safe-retry: a cached envelope must survive a
        // daemon restart against the same SQLite file.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let e = entry("idem-key-003", "cafebabe", r#"{"status":"auto_approved"}"#);
        {
            let mut s = Storage::open(&path).unwrap();
            assert!(s.idempotency_try_store(&e).unwrap());
        }
        let s = Storage::open(&path).unwrap();
        let got = s
            .idempotency_lookup(&e.key)
            .unwrap()
            .expect("survives reopen");
        assert_eq!(got, e);
    }

    #[test]
    fn count_reflects_inserts() {
        let mut s = Storage::open_in_memory().unwrap();
        assert_eq!(s.idempotency_count().unwrap(), 0);
        s.idempotency_try_store(&entry("idem-key-A", "h1", r#"{"v":1}"#))
            .unwrap();
        s.idempotency_try_store(&entry("idem-key-B", "h2", r#"{"v":2}"#))
            .unwrap();
        assert_eq!(s.idempotency_count().unwrap(), 2);
    }
}
