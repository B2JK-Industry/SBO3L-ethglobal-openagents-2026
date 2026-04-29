//! Persistent APRP nonce replay protection.
//!
//! Backed by the `nonce_replay` table (migration V002). The table's PRIMARY
//! KEY on `nonce` gives us atomic insert-or-fail semantics for concurrent
//! requests: two writers with the same nonce both attempt INSERT, exactly
//! one wins, the loser surfaces `SQLITE_CONSTRAINT_PRIMARYKEY` which we
//! translate to `Ok(false)`.
//!
//! Why not use `SELECT ... INSERT` (read-then-write)? It would race: two
//! concurrent reads see no row, both then INSERT, one fails. Letting the
//! INSERT *be* the read avoids the race entirely.

use chrono::{DateTime, Utc};
use rusqlite::ErrorCode;

use crate::error::{StorageError, StorageResult};
use crate::Storage;

impl Storage {
    /// Atomically register an APRP nonce as "seen". Returns:
    ///
    /// - `Ok(true)`  — the nonce was previously unseen and has now been
    ///   claimed. The caller should proceed with the request.
    /// - `Ok(false)` — the nonce was already in the table; the caller MUST
    ///   reject the request with HTTP 409 `protocol.nonce_replay` and MUST
    ///   NOT produce audit / receipt side effects.
    /// - `Err(...)`  — any other SQLite error. The caller MUST fail closed
    ///   (treat as a server error) — we never silently allow a request when
    ///   we can't verify whether its nonce was already seen.
    ///
    /// `seen_at` is recorded for diagnostics and future TTL eviction; it
    /// does not affect rejection semantics.
    pub fn nonce_try_claim(
        &mut self,
        nonce: &str,
        agent_id: &str,
        seen_at: DateTime<Utc>,
    ) -> StorageResult<bool> {
        let ts = seen_at.to_rfc3339();
        match self.conn.execute(
            "INSERT INTO nonce_replay(nonce, agent_id, seen_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![nonce, agent_id, ts],
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

    /// Number of nonces currently registered. Diagnostic only — the gate
    /// uses `nonce_try_claim` directly and does not consult this.
    pub fn nonce_count(&self) -> StorageResult<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM nonce_replay", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_claim_succeeds() {
        let mut s = Storage::open_in_memory().unwrap();
        assert!(s
            .nonce_try_claim("01HFAKE001", "research-agent-01", Utc::now())
            .unwrap());
        assert_eq!(s.nonce_count().unwrap(), 1);
    }

    #[test]
    fn second_claim_with_same_nonce_returns_false() {
        let mut s = Storage::open_in_memory().unwrap();
        let now = Utc::now();
        assert!(s
            .nonce_try_claim("01HFAKE002", "research-agent-01", now)
            .unwrap());
        // Replay (same nonce, even from same agent at the same instant).
        assert!(!s
            .nonce_try_claim("01HFAKE002", "research-agent-01", now)
            .unwrap());
        // Replay must NOT create a second row.
        assert_eq!(s.nonce_count().unwrap(), 1);
    }

    #[test]
    fn replay_with_different_agent_id_is_still_rejected() {
        // The replay key is the nonce alone — APRP nonces are globally
        // unique by spec, so the same nonce from a different agent is
        // also a replay (or a malicious cross-agent reuse).
        let mut s = Storage::open_in_memory().unwrap();
        assert!(s
            .nonce_try_claim("01HFAKE003", "research-agent-01", Utc::now())
            .unwrap());
        assert!(!s
            .nonce_try_claim("01HFAKE003", "OTHER-AGENT", Utc::now())
            .unwrap());
    }

    #[test]
    fn distinct_nonces_can_each_be_claimed() {
        let mut s = Storage::open_in_memory().unwrap();
        assert!(s
            .nonce_try_claim("01HFAKE004A", "research-agent-01", Utc::now())
            .unwrap());
        assert!(s
            .nonce_try_claim("01HFAKE004B", "research-agent-01", Utc::now())
            .unwrap());
        assert_eq!(s.nonce_count().unwrap(), 2);
    }

    #[test]
    fn nonce_persists_across_storage_reopen() {
        // The whole point of this PR: the nonce table outlives a daemon
        // restart. Open a tempfile-backed db, claim a nonce, drop the
        // Storage handle, reopen the same path — the second claim attempt
        // for that nonce must fail.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let mut s = Storage::open(&path).unwrap();
            assert!(s
                .nonce_try_claim("01HFAKE005", "research-agent-01", Utc::now())
                .unwrap());
        }
        // Re-open and confirm the row survived. This is what makes the
        // protection robust against `sbo3l-server` crash / restart with
        // a persistent SQLite database.
        let mut s = Storage::open(&path).unwrap();
        assert_eq!(s.nonce_count().unwrap(), 1);
        assert!(!s
            .nonce_try_claim("01HFAKE005", "research-agent-01", Utc::now())
            .unwrap());
    }
}
