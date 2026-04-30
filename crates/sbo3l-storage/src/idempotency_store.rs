//! Persistent storage for HTTP `Idempotency-Key` safe-retry with the F-3
//! atomic state machine.
//!
//! The HTTP daemon hits this table on `POST /v1/payment-requests` BEFORE the
//! schema / nonce / policy / budget / audit / signing pipeline. Pre-F-3
//! the daemon read first then INSERTed post-success, which under
//! concurrency let two same-key requests both observe a cache miss, both
//! run the full pipeline, and only the loser's INSERT collide on the
//! PRIMARY KEY constraint — by then the winner had already double-spent
//! against nonce + budget + audit. F-3 closes that gap with a single
//! atomic CLAIM (`INSERT … state='processing'`) that races safely on the
//! PRIMARY KEY: exactly one writer wins, every concurrent same-key
//! request is rejected with HTTP 409 `protocol.idempotency_in_flight`.
//!
//! State semantics:
//!
//! * `processing` — claimed by a request currently inside the pipeline.
//!   Concurrent same-key requests get `idempotency_in_flight`.
//! * `succeeded`  — pipeline returned 200; `(response_status,
//!   response_body)` is authoritative for byte-identical replay on
//!   same-key + same-body retries.
//! * `failed`     — pipeline returned non-200. The row is held for a
//!   60-second grace window during which retries get
//!   `idempotency_in_flight`; past the grace window it's reclaimable
//!   (atomic UPDATE WHERE state='failed' AND created_at < cutoff).
//!
//! Cached replay only fires for `succeeded` rows — failure responses are
//! never replayed (the spec calls for retries past failure to re-run the
//! pipeline, not see a stale failure body).

use chrono::{DateTime, Utc};
use rusqlite::{params, ErrorCode};

use crate::error::{StorageError, StorageResult};
use crate::Storage;

/// Three-state machine over an idempotency claim. See module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyState {
    Processing,
    Succeeded,
    Failed,
}

impl IdempotencyState {
    /// Database textual representation. Must match the V009 CHECK
    /// constraint exactly.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Processing => "processing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "processing" => Some(Self::Processing),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// One row of the `idempotency_keys` table. `response_status` and
/// `response_body` are placeholders (0, "") while `state == Processing`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyEntry {
    pub key: String,
    pub request_hash: String,
    pub response_status: u16,
    pub response_body: String,
    pub created_at: DateTime<Utc>,
    pub state: IdempotencyState,
}

/// Outcome of an atomic claim attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// We won the race; the row now exists with `state = Processing`. The
    /// caller MUST follow up with `idempotency_succeed` or
    /// `idempotency_fail` once its pipeline completes.
    Claimed,
    /// A row already exists for this key. The caller decides what to
    /// surface to the client based on the existing row's state and body
    /// hash (see module doc).
    Existing(IdempotencyEntry),
}

impl Storage {
    /// Look up a previously-stored idempotency envelope by client-supplied
    /// `Idempotency-Key`. Returns `Ok(None)` for an unseen key.
    pub fn idempotency_lookup(&self, key: &str) -> StorageResult<Option<IdempotencyEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, request_hash, response_status, response_body, created_at, state
             FROM idempotency_keys WHERE key = ?1",
        )?;
        match stmt.query_row(params![key], row_to_entry) {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomically CLAIM the key for an in-flight request. On success the
    /// row is inserted with `state = 'processing'` and placeholder
    /// `(response_status = 0, response_body = '')`. On the constraint
    /// collision the existing row is returned for the caller to inspect.
    pub fn idempotency_try_claim(
        &mut self,
        key: &str,
        request_hash: &str,
        now: DateTime<Utc>,
    ) -> StorageResult<ClaimOutcome> {
        match self.conn.execute(
            "INSERT INTO idempotency_keys
                (key, request_hash, response_status, response_body, created_at, state)
             VALUES (?1, ?2, 0, '', ?3, 'processing')",
            params![key, request_hash, now.to_rfc3339()],
        ) {
            Ok(_) => Ok(ClaimOutcome::Claimed),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == ErrorCode::ConstraintViolation =>
            {
                let existing = self
                    .idempotency_lookup(key)?
                    .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))?;
                Ok(ClaimOutcome::Existing(existing))
            }
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// Move a `Processing` row to `Succeeded` and cache the response.
    /// Returns `Ok(true)` if exactly one row was updated. `Ok(false)`
    /// means the row was already in another state (e.g. a daemon-crash
    /// recovery sweep marked it `Failed`); the caller should not retry
    /// this update.
    pub fn idempotency_succeed(
        &mut self,
        key: &str,
        response_status: u16,
        response_body: &str,
    ) -> StorageResult<bool> {
        let rows = self.conn.execute(
            "UPDATE idempotency_keys
                SET state = 'succeeded',
                    response_status = ?1,
                    response_body = ?2
              WHERE key = ?3 AND state = 'processing'",
            params![response_status as i64, response_body, key],
        )?;
        Ok(rows == 1)
    }

    /// Move a `Processing` row to `Failed`. Cached body stays empty —
    /// failure responses are never replayed; the row only exists so a
    /// retry within the grace window gets `idempotency_in_flight`
    /// instead of double-running the pipeline.
    ///
    /// Resets `created_at` to the failure moment so the grace window
    /// (`idempotency_try_reclaim_failed`) starts counting from FAIL,
    /// not from the original `try_claim`. Without this, a slow pipeline
    /// (>60s before failing) would have an immediately reclaimable row
    /// — defeating the back-off intent of the grace window.
    pub fn idempotency_fail(
        &mut self,
        key: &str,
        response_status: u16,
        now: DateTime<Utc>,
    ) -> StorageResult<bool> {
        let rows = self.conn.execute(
            "UPDATE idempotency_keys
                SET state = 'failed',
                    response_status = ?1,
                    response_body = '',
                    created_at = ?2
              WHERE key = ?3 AND state = 'processing'",
            params![response_status as i64, now.to_rfc3339(), key],
        )?;
        Ok(rows == 1)
    }

    /// Past-the-grace-window reclaim: try to atomically promote a
    /// `Failed` row back to `Processing` for a fresh attempt with the
    /// new `request_hash`. The race-safe guard is the WHERE clause:
    /// only updates if state is still `Failed` AND `created_at` is
    /// older than `now - grace_secs`. Concurrent reclaimers see
    /// rows = 0; only one wins.
    pub fn idempotency_try_reclaim_failed(
        &mut self,
        key: &str,
        request_hash: &str,
        now: DateTime<Utc>,
        grace_secs: i64,
    ) -> StorageResult<bool> {
        let cutoff = now - chrono::Duration::seconds(grace_secs);
        let rows = self.conn.execute(
            "UPDATE idempotency_keys
                SET state = 'processing',
                    request_hash = ?1,
                    response_status = 0,
                    response_body = '',
                    created_at = ?2
              WHERE key = ?3 AND state = 'failed' AND created_at < ?4",
            params![request_hash, now.to_rfc3339(), key, cutoff.to_rfc3339(),],
        )?;
        Ok(rows == 1)
    }

    /// Diagnostic only — number of cached envelopes currently in the table.
    pub fn idempotency_count(&self) -> StorageResult<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM idempotency_keys", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}

fn row_to_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<IdempotencyEntry> {
    let ts: String = r.get(4)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&ts)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Utc);
    let state_str: String = r.get(5)?;
    let state = IdempotencyState::from_db_str(&state_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown idempotency state {state_str:?}"),
            )),
        )
    })?;
    Ok(IdempotencyEntry {
        key: r.get(0)?,
        request_hash: r.get(1)?,
        response_status: r.get::<_, i64>(2)? as u16,
        response_body: r.get(3)?,
        created_at,
        state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(off_secs: i64) -> DateTime<Utc> {
        let base: DateTime<Utc> = chrono::DateTime::parse_from_rfc3339("2026-05-01T12:00:00Z")
            .unwrap()
            .into();
        base + chrono::Duration::seconds(off_secs)
    }

    #[test]
    fn lookup_unknown_key_returns_none() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.idempotency_lookup("idem-unknown-XXX").unwrap().is_none());
    }

    #[test]
    fn try_claim_first_writer_wins() {
        let mut s = Storage::open_in_memory().unwrap();
        let outcome = s
            .idempotency_try_claim("idem-key-001", "req-hash-aaa", ts(0))
            .unwrap();
        assert_eq!(outcome, ClaimOutcome::Claimed);
        let entry = s.idempotency_lookup("idem-key-001").unwrap().unwrap();
        assert_eq!(entry.state, IdempotencyState::Processing);
        assert_eq!(entry.request_hash, "req-hash-aaa");
        assert_eq!(entry.response_status, 0);
        assert_eq!(entry.response_body, "");
    }

    #[test]
    fn try_claim_returns_existing_on_conflict() {
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-002", "req-hash-aaa", ts(0))
            .unwrap();
        let outcome = s
            .idempotency_try_claim("idem-key-002", "req-hash-bbb", ts(1))
            .unwrap();
        match outcome {
            ClaimOutcome::Existing(e) => {
                assert_eq!(e.state, IdempotencyState::Processing);
                assert_eq!(
                    e.request_hash, "req-hash-aaa",
                    "first writer's body hash is authoritative"
                );
            }
            other => panic!("expected Existing, got {other:?}"),
        }
    }

    #[test]
    fn succeed_promotes_processing_and_caches_body() {
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-003", "req-hash", ts(0))
            .unwrap();
        let updated = s
            .idempotency_succeed("idem-key-003", 200, r#"{"decision":"allow"}"#)
            .unwrap();
        assert!(updated);
        let entry = s.idempotency_lookup("idem-key-003").unwrap().unwrap();
        assert_eq!(entry.state, IdempotencyState::Succeeded);
        assert_eq!(entry.response_status, 200);
        assert_eq!(entry.response_body, r#"{"decision":"allow"}"#);
    }

    #[test]
    fn succeed_is_no_op_when_state_already_succeeded() {
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-004", "req-hash", ts(0))
            .unwrap();
        s.idempotency_succeed("idem-key-004", 200, "first").unwrap();
        let updated = s
            .idempotency_succeed("idem-key-004", 200, "second")
            .unwrap();
        assert!(
            !updated,
            "succeed must only fire on the processing -> succeeded edge"
        );
        let entry = s.idempotency_lookup("idem-key-004").unwrap().unwrap();
        assert_eq!(entry.response_body, "first");
    }

    #[test]
    fn fail_marks_row_failed_with_status() {
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-005", "req-hash", ts(0))
            .unwrap();
        let updated = s.idempotency_fail("idem-key-005", 409, ts(5)).unwrap();
        assert!(updated);
        let entry = s.idempotency_lookup("idem-key-005").unwrap().unwrap();
        assert_eq!(entry.state, IdempotencyState::Failed);
        assert_eq!(entry.response_status, 409);
        assert_eq!(entry.response_body, "");
    }

    #[test]
    fn try_reclaim_failed_blocked_within_grace() {
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-006", "old-hash", ts(0))
            .unwrap();
        // Pipeline failed at ts(5); grace counts from FAIL, not original claim.
        s.idempotency_fail("idem-key-006", 500, ts(5)).unwrap();
        // 30s after the failure (ts(35)), with a 60s grace, reclaim must fail.
        let reclaimed = s
            .idempotency_try_reclaim_failed("idem-key-006", "new-hash", ts(35), 60)
            .unwrap();
        assert!(!reclaimed);
        let entry = s.idempotency_lookup("idem-key-006").unwrap().unwrap();
        assert_eq!(entry.state, IdempotencyState::Failed);
    }

    #[test]
    fn try_reclaim_failed_succeeds_past_grace() {
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-007", "old-hash", ts(0))
            .unwrap();
        s.idempotency_fail("idem-key-007", 500, ts(5)).unwrap();
        // 61s after the failure (ts(66)), with a 60s grace, reclaim succeeds.
        let reclaimed = s
            .idempotency_try_reclaim_failed("idem-key-007", "new-hash", ts(66), 60)
            .unwrap();
        assert!(reclaimed);
        let entry = s.idempotency_lookup("idem-key-007").unwrap().unwrap();
        assert_eq!(entry.state, IdempotencyState::Processing);
        assert_eq!(
            entry.request_hash, "new-hash",
            "reclaim adopts the new request body's hash"
        );
        assert_eq!(entry.response_status, 0);
    }

    #[test]
    fn slow_pipeline_failure_does_not_immediately_unlock_reclaim() {
        // The grace window counts from FAIL, not from try_claim. A pipeline
        // that takes 90s before failing must NOT be reclaimable at fail-time —
        // the row needs `grace_secs` after the failure mark to be reclaimable.
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-slow-fail", "old-hash", ts(0))
            .unwrap();
        // Pipeline ran 90s, exceeding the 60s grace window measured from claim.
        s.idempotency_fail("idem-key-slow-fail", 500, ts(90))
            .unwrap();
        // Immediately after failure: must still be blocked from reclaim
        // (grace counts from ts(90), not ts(0)).
        let reclaimed = s
            .idempotency_try_reclaim_failed("idem-key-slow-fail", "new-hash", ts(90), 60)
            .unwrap();
        assert!(
            !reclaimed,
            "grace window must count from the FAIL timestamp, not the original claim"
        );
        // 30s after fail still blocked.
        let reclaimed_mid = s
            .idempotency_try_reclaim_failed("idem-key-slow-fail", "new-hash", ts(120), 60)
            .unwrap();
        assert!(!reclaimed_mid);
        // 61s after fail: now reclaimable.
        let reclaimed_late = s
            .idempotency_try_reclaim_failed("idem-key-slow-fail", "new-hash", ts(151), 60)
            .unwrap();
        assert!(reclaimed_late);
    }

    #[test]
    fn try_reclaim_only_one_concurrent_winner() {
        // Pin the race-safe property of the reclaim UPDATE: only one
        // concurrent reclaimer sees rows=1, others see rows=0.
        let mut s = Storage::open_in_memory().unwrap();
        s.idempotency_try_claim("idem-key-008", "old", ts(0))
            .unwrap();
        s.idempotency_fail("idem-key-008", 500, ts(5)).unwrap();

        let first = s
            .idempotency_try_reclaim_failed("idem-key-008", "winner", ts(120), 60)
            .unwrap();
        let second = s
            .idempotency_try_reclaim_failed("idem-key-008", "loser", ts(120), 60)
            .unwrap();
        assert!(first);
        assert!(
            !second,
            "second reclaim must lose because state is now 'processing'"
        );
    }

    #[test]
    fn round_trip_persists_across_storage_reopen() {
        // Pre-V009 was 'cached envelope survives daemon restart'. F-3
        // extends that to: claim → succeed → reopen → row still
        // succeeded with same body.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let mut s = Storage::open(&path).unwrap();
            s.idempotency_try_claim("idem-key-009", "h", ts(0)).unwrap();
            s.idempotency_succeed("idem-key-009", 200, r#"{"x":1}"#)
                .unwrap();
        }
        let s = Storage::open(&path).unwrap();
        let entry = s.idempotency_lookup("idem-key-009").unwrap().unwrap();
        assert_eq!(entry.state, IdempotencyState::Succeeded);
        assert_eq!(entry.response_body, r#"{"x":1}"#);
    }

    #[test]
    fn count_reflects_inserts() {
        let mut s = Storage::open_in_memory().unwrap();
        assert_eq!(s.idempotency_count().unwrap(), 0);
        s.idempotency_try_claim("idem-key-A", "h1", ts(0)).unwrap();
        s.idempotency_try_claim("idem-key-B", "h2", ts(0)).unwrap();
        assert_eq!(s.idempotency_count().unwrap(), 2);
    }
}
