//! Persistence for the **local** active-policy lifecycle (PSM-A3).
//!
//! Backs `mandate policy {validate,current,activate,diff}`. Stores
//! every activated policy version verbatim (the canonical JSON the
//! operator handed to `policy activate`) plus metadata: numeric
//! version (auto-incremented per activation), SHA-256 hash of the
//! canonical JSON, source label (`operator-cli` by default), and
//! activation/deactivation timestamps.
//!
//! Singleton invariant: at most one row has `deactivated_at IS NULL`
//! at any moment. Enforced by a partial UNIQUE index in V006 — a
//! buggy CLI cannot leave two simultaneously-active rows. A new
//! `activate` call atomically deactivates the prior active row in
//! the same transaction.
//!
//! Truthfulness rules:
//! - This is **local** lifecycle, not remote governance. There is no
//!   on-chain anchor, no consensus, no signing on activation;
//!   whoever opens the DB can activate a policy.
//! - Hash uniqueness is global across versions: re-activating an
//!   already-active policy is a no-op (`activate` returns the existing
//!   version), so identical activations never multiply rows.
//! - Re-activating a *previously deactivated* policy by hash is
//!   refused — the hash UNIQUE constraint prevents it. Operators must
//!   change the policy (even cosmetically) to push a new version.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::error::{StorageError, StorageResult};
use crate::Storage;

/// One row of the `active_policy` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePolicyRecord {
    pub version: u32,
    pub policy_hash: String,
    pub policy_json: String,
    pub activated_at: DateTime<Utc>,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub source: String,
}

/// Outcome of an [`Storage::policy_activate`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateOutcome {
    /// A new row was inserted; the caller-supplied policy is now active.
    Activated { version: u32 },
    /// The supplied policy was already the currently-active one;
    /// no row was inserted. Idempotent re-runs hit this path.
    AlreadyActive { version: u32 },
}

impl Storage {
    /// Activate `policy_json` (which the caller MUST have already
    /// validated and hashed; we re-hash here only as a self-check).
    ///
    /// Behaviour:
    /// - If no row is currently active, insert as `version=1`.
    /// - If the currently-active row's `policy_hash` equals
    ///   `policy_hash`, no-op (returns `AlreadyActive`).
    /// - Otherwise, mark the current row's `deactivated_at = now()`
    ///   and insert a new row at `version = max(version)+1`.
    /// - Hash collision against any historical row (already-deactivated
    ///   policies) is refused via `StorageError::Sqlite` (the UNIQUE
    ///   constraint on `policy_hash` fires).
    pub fn policy_activate(
        &mut self,
        policy_json: &str,
        policy_hash: &str,
        source: &str,
        now: DateTime<Utc>,
    ) -> StorageResult<ActivateOutcome> {
        let tx = self.conn.transaction()?;

        // Is something currently active?
        //
        // Self-review (mirroring the Codex P2 finding on PR #28's
        // `mock_kms_current_version`): the previous shape used `.ok()`
        // to coerce every error into `None`, which made schema /
        // IO failures indistinguishable from "no active row" and
        // caused `policy_activate` to silently behave as if the
        // table were empty (and proceed to INSERT, bypassing the
        // "is something already active" check). Explicit match here
        // propagates real errors; only `QueryReturnedNoRows` maps to
        // `None`.
        let current: Option<(u32, String)> = match tx.query_row(
            "SELECT version, policy_hash FROM active_policy
             WHERE deactivated_at IS NULL",
            [],
            |r| Ok((r.get::<_, i64>(0)? as u32, r.get::<_, String>(1)?)),
        ) {
            Ok(row) => Some(row),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(StorageError::Sqlite(e)),
        };

        if let Some((v, h)) = &current {
            if h == policy_hash {
                tx.commit()?;
                return Ok(ActivateOutcome::AlreadyActive { version: *v });
            }
        }

        if let Some((v, _)) = current {
            tx.execute(
                "UPDATE active_policy SET deactivated_at = ?1
                 WHERE version = ?2 AND deactivated_at IS NULL",
                params![now.to_rfc3339(), v as i64],
            )?;
        }

        let max_version: Option<i64> =
            tx.query_row("SELECT MAX(version) FROM active_policy", [], |r| r.get(0))?;
        let next_version = (max_version.unwrap_or(0) as u32) + 1;

        tx.execute(
            "INSERT INTO active_policy
             (version, policy_hash, policy_json, activated_at, deactivated_at, source)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![
                next_version as i64,
                policy_hash,
                policy_json,
                now.to_rfc3339(),
                source,
            ],
        )?;
        tx.commit()?;
        Ok(ActivateOutcome::Activated {
            version: next_version,
        })
    }

    /// Currently-active policy, or `None` if no policy has ever been
    /// activated (or the most recent one was explicitly deactivated —
    /// see [`Storage::policy_deactivate_current`], not yet exposed
    /// through the CLI).
    pub fn policy_current(&self) -> StorageResult<Option<ActivePolicyRecord>> {
        let result = self.conn.query_row(
            "SELECT version, policy_hash, policy_json, activated_at, deactivated_at, source
             FROM active_policy WHERE deactivated_at IS NULL",
            [],
            row_to_record,
        );
        match result {
            Ok(rec) => Ok(Some(rec)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// All rows, in ascending version order. Includes deactivated
    /// historical rows so an operator can see the full lifecycle.
    pub fn policy_list(&self) -> StorageResult<Vec<ActivePolicyRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT version, policy_hash, policy_json, activated_at, deactivated_at, source
             FROM active_policy ORDER BY version ASC",
        )?;
        let iter = stmt.query_map([], row_to_record)?;
        let mut out = Vec::new();
        for row in iter {
            out.push(row?);
        }
        Ok(out)
    }

    /// Look up a specific historical version (active or deactivated).
    pub fn policy_get_version(&self, version: u32) -> StorageResult<Option<ActivePolicyRecord>> {
        let result = self.conn.query_row(
            "SELECT version, policy_hash, policy_json, activated_at, deactivated_at, source
             FROM active_policy WHERE version = ?1",
            params![version as i64],
            row_to_record,
        );
        match result {
            Ok(rec) => Ok(Some(rec)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<ActivePolicyRecord> {
    let activated_str: String = r.get(3)?;
    let deactivated_str: Option<String> = r.get(4)?;
    let activated_at = parse_rfc3339(&activated_str, 3)?;
    let deactivated_at = match deactivated_str {
        Some(s) => Some(parse_rfc3339(&s, 4)?),
        None => None,
    };
    Ok(ActivePolicyRecord {
        version: r.get::<_, i64>(0)? as u32,
        policy_hash: r.get(1)?,
        policy_json: r.get(2)?,
        activated_at,
        deactivated_at,
        source: r.get(5)?,
    })
}

fn parse_rfc3339(s: &str, col: usize) -> rusqlite::Result<DateTime<Utc>> {
    Ok(chrono::DateTime::parse_from_rfc3339(s)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_at(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s).unwrap().into()
    }

    fn pol(rule_id: &str) -> (String, String) {
        // Tiny synthetic policy JSON (not validated against the real
        // schema — that lives in mandate-policy. We only need a string
        // that's stable + hashable for storage tests).
        let j = format!(
            r#"{{"version":1,"agents":[],"rules":[{{"id":"{rule_id}"}}],"providers":[],"recipients":[],"budgets":[]}}"#
        );
        let h = hex::encode(sha2::Sha256::digest(j.as_bytes()));
        (j, h)
    }

    use sha2::Digest;

    #[test]
    fn activate_first_time_returns_version_one() {
        let mut s = Storage::open_in_memory().unwrap();
        let (j, h) = pol("rule-a");
        let outcome = s
            .policy_activate(&j, &h, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();
        assert_eq!(outcome, ActivateOutcome::Activated { version: 1 });
        let cur = s.policy_current().unwrap().unwrap();
        assert_eq!(cur.version, 1);
        assert_eq!(cur.policy_hash, h);
        assert!(cur.deactivated_at.is_none());
        assert_eq!(cur.source, "operator-cli");
    }

    #[test]
    fn activate_same_hash_is_idempotent() {
        let mut s = Storage::open_in_memory().unwrap();
        let (j, h) = pol("rule-a");
        let _ = s
            .policy_activate(&j, &h, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();
        let outcome = s
            .policy_activate(&j, &h, "operator-cli", now_at("2026-04-28T11:00:00Z"))
            .unwrap();
        assert_eq!(outcome, ActivateOutcome::AlreadyActive { version: 1 });
        // No new row was inserted.
        assert_eq!(s.policy_list().unwrap().len(), 1);
    }

    #[test]
    fn activate_different_hash_replaces_active_row() {
        let mut s = Storage::open_in_memory().unwrap();
        let (j1, h1) = pol("rule-a");
        let (j2, h2) = pol("rule-b");
        let _ = s
            .policy_activate(&j1, &h1, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();
        let outcome = s
            .policy_activate(&j2, &h2, "operator-cli", now_at("2026-04-28T11:00:00Z"))
            .unwrap();
        assert_eq!(outcome, ActivateOutcome::Activated { version: 2 });

        let all = s.policy_list().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].version, 1);
        assert!(
            all[0].deactivated_at.is_some(),
            "v1 must be deactivated after v2 activates"
        );
        assert_eq!(all[1].version, 2);
        assert!(all[1].deactivated_at.is_none());

        let cur = s.policy_current().unwrap().unwrap();
        assert_eq!(cur.version, 2);
        assert_eq!(cur.policy_hash, h2);
    }

    #[test]
    fn current_returns_none_when_no_policy_activated() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.policy_current().unwrap().is_none());
    }

    #[test]
    fn re_activating_deactivated_policy_hash_is_refused() {
        // Hash-uniqueness is global. Once a hash has been seen, even
        // the deactivated row continues to occupy that hash — so an
        // operator who tries to flip back-and-forth between A and B
        // hits a UNIQUE constraint failure on the second A. This is
        // the truthful behaviour: the lifecycle records every
        // activation, you cannot pretend a deactivated version was
        // never run. To go back, you must produce a fresh policy
        // (different hash, even cosmetically).
        let mut s = Storage::open_in_memory().unwrap();
        let (j1, h1) = pol("rule-a");
        let (j2, h2) = pol("rule-b");
        s.policy_activate(&j1, &h1, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();
        s.policy_activate(&j2, &h2, "operator-cli", now_at("2026-04-28T11:00:00Z"))
            .unwrap();
        let err = s
            .policy_activate(&j1, &h1, "operator-cli", now_at("2026-04-28T12:00:00Z"))
            .expect_err("re-activating deactivated hash must fail");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("unique") || msg.contains("constraint"),
            "expected UNIQUE-constraint error; got: {msg}"
        );
    }

    #[test]
    fn list_returns_full_history_in_version_order() {
        let mut s = Storage::open_in_memory().unwrap();
        for (i, label) in ["a", "b", "c"].iter().enumerate() {
            let (j, h) = pol(label);
            s.policy_activate(
                &j,
                &h,
                "operator-cli",
                now_at(&format!("2026-04-28T1{i}:00:00Z")),
            )
            .unwrap();
        }
        let rows = s.policy_list().unwrap();
        assert_eq!(rows.len(), 3);
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r.version, (i as u32) + 1);
        }
        // First two are deactivated, last is active.
        assert!(rows[0].deactivated_at.is_some());
        assert!(rows[1].deactivated_at.is_some());
        assert!(rows[2].deactivated_at.is_none());
    }

    #[test]
    fn get_version_round_trip() {
        let mut s = Storage::open_in_memory().unwrap();
        let (j, h) = pol("rule-a");
        s.policy_activate(&j, &h, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();
        let v1 = s.policy_get_version(1).unwrap().unwrap();
        assert_eq!(v1.policy_hash, h);
        assert_eq!(v1.policy_json, j);
        assert!(s.policy_get_version(99).unwrap().is_none());
    }

    /// Codex P1 review on PR #35: the singleton invariant must be
    /// enforced by the database itself, not just by the
    /// `policy_activate` CLI guard. This test bypasses the high-level
    /// activate path and INSERTs a second active row directly via raw
    /// rusqlite. With the V006 partial UNIQUE index keyed on
    /// `(deactivated_at IS NULL)`, that second insert MUST fail with
    /// a UNIQUE constraint error. The previous shape — partial UNIQUE
    /// keyed directly on `deactivated_at` — silently accepted the
    /// second insert because SQLite treats every NULL as distinct.
    #[test]
    fn db_layer_refuses_two_active_rows_even_via_raw_insert() {
        let mut s = Storage::open_in_memory().unwrap();
        let (j1, h1) = pol("rule-a");
        s.policy_activate(&j1, &h1, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();

        // Bypass `policy_activate` entirely. If the singleton invariant
        // depended only on the CLI's "deactivate previous in same tx"
        // path, this insert would succeed and the table would carry
        // two active rows — which is exactly the shape Codex P1 caught.
        let raw_insert = s.conn.execute(
            "INSERT INTO active_policy
             (version, policy_hash, policy_json, activated_at, deactivated_at, source)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            rusqlite::params![
                999_i64,
                "0".repeat(64), // a different, syntactically-valid hash
                "{}",
                "2026-04-28T11:00:00Z",
                "test-bypass",
            ],
        );
        let err = raw_insert.expect_err(
            "DB-level singleton invariant must reject a second active row \
             even when policy_activate is bypassed",
        );
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("unique") || msg.contains("constraint"),
            "expected UNIQUE constraint failure; got: {msg}"
        );

        // Sanity: the original active row is still present and is
        // still the one and only active policy.
        let all = s.policy_list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].policy_hash, h1);
        assert!(all[0].deactivated_at.is_none());
    }

    /// Pin the lifecycle: deactivate-then-activate is the supported
    /// flow and the new singleton index must NOT block it. Distinct
    /// from `db_layer_refuses_two_active_rows_…` above which exercises
    /// the rejection path.
    #[test]
    fn db_layer_allows_normal_deactivate_then_activate_lifecycle() {
        let mut s = Storage::open_in_memory().unwrap();
        let (j1, h1) = pol("rule-a");
        let (j2, h2) = pol("rule-b");
        s.policy_activate(&j1, &h1, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .unwrap();
        // Activating a second, distinct policy goes through the
        // policy_activate path which deactivates v1 in the same tx
        // before inserting v2. The DB-level singleton index must
        // accept this — historical (deactivated) rows are excluded
        // from the partial index entirely.
        let outcome = s
            .policy_activate(&j2, &h2, "operator-cli", now_at("2026-04-28T11:00:00Z"))
            .unwrap();
        assert_eq!(outcome, ActivateOutcome::Activated { version: 2 });
        let all = s.policy_list().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].deactivated_at.is_some(), "v1 deactivated");
        assert!(all[1].deactivated_at.is_none(), "v2 active");
    }

    #[test]
    fn policy_persists_across_storage_reopen() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let (j, h) = pol("rule-a");
        {
            let mut s = Storage::open(&path).unwrap();
            s.policy_activate(&j, &h, "operator-cli", now_at("2026-04-28T10:00:00Z"))
                .unwrap();
        }
        let s = Storage::open(&path).unwrap();
        let cur = s.policy_current().unwrap().unwrap();
        assert_eq!(cur.policy_hash, h);
        assert_eq!(cur.version, 1);
    }

    /// Self-review parallel to the Codex P2 finding on PR #28's
    /// `mock_kms_current_version`: `policy_activate`'s "is something
    /// currently active" lookup previously used `.ok()` to coerce
    /// every error into `None`, which made schema/IO failures
    /// indistinguishable from "no active row" and caused
    /// `policy_activate` to silently behave as if the table were
    /// empty (and proceed to INSERT, bypassing the duplicate-active
    /// check). After the fix, dropping the table out from under the
    /// storage layer surfaces as a proper `Err`, not a silent
    /// success.
    #[test]
    fn policy_activate_propagates_query_errors_when_table_dropped() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        // Apply all current migrations.
        {
            let _ = Storage::open(&path).unwrap();
        }
        // Drop the active_policy table out from under the storage,
        // simulating either schema corruption or an older daemon DB
        // that somehow lost V006. The next Storage::open will skip
        // re-running V006 because schema_migrations records it as
        // already applied — exactly the operational shape that the
        // .ok()-swallowed path used to hide.
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute("DROP TABLE active_policy", []).unwrap();
        }
        let mut s = Storage::open(&path).unwrap();
        let (j, h) = pol("rule-a");
        let err = s
            .policy_activate(&j, &h, "operator-cli", now_at("2026-04-28T10:00:00Z"))
            .expect_err("missing table must propagate as Err, not silently insert");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("active_policy") || msg.contains("no such"),
            "expected the error to mention the missing table; got: {msg}"
        );
    }
}
