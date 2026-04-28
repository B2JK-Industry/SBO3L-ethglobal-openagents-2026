//! Persistence for the **mock** KMS keyring (PSM-A1.9).
//!
//! Holds *only* public-key material — no seeds, no private keys. The
//! `mandate key {init,list,rotate} --mock` CLI supplies the
//! deterministic `--root-seed` on every operation; this module records
//! the resulting per-version public key plus stable metadata
//! (`role`, `version`, `key_id`, `public_hex`, `created_at`).
//!
//! Truthfulness rules:
//! - Every row is mock. The CLI surface enforces a `--mock` flag for
//!   loud disclosure.
//! - A real KMS keyring would store an opaque KMS handle/ARN per
//!   version, NOT a deterministic public key. This is mock-shape, not
//!   production-shape.
//! - Rotation never advances backwards: the unique `(role, version)`
//!   primary key prevents rollback / collision.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::error::{StorageError, StorageResult};
use crate::Storage;

/// One row of the `mock_kms_keys` table. Mirrors what
/// `mandate_core::mock_kms::MockKmsKeyMeta` carries, minus the
/// always-true `mock: bool` field (the table itself implies mock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockKmsKeyRecord {
    pub role: String,
    pub version: u32,
    pub key_id: String,
    pub public_hex: String,
    pub created_at: DateTime<Utc>,
}

impl Storage {
    /// Insert a keyring entry. Returns:
    /// - `Ok(true)`  — row was new and stored;
    /// - `Ok(false)` — row already existed for this (role, version) or
    ///   (key_id) — caller may treat as a no-op;
    /// - `Err(...)` — any other SQLite error.
    pub fn mock_kms_insert(&mut self, record: &MockKmsKeyRecord) -> StorageResult<bool> {
        match self.conn.execute(
            "INSERT INTO mock_kms_keys (role, version, key_id, public_hex, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.role,
                record.version as i64,
                record.key_id,
                record.public_hex,
                record.created_at.to_rfc3339(),
            ],
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Ok(false)
            }
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// List keyring rows in ascending `(role, version)` order. If
    /// `role` is supplied, restrict to that role.
    pub fn mock_kms_list(&self, role: Option<&str>) -> StorageResult<Vec<MockKmsKeyRecord>> {
        let mut rows = Vec::new();
        match role {
            Some(r) => {
                let mut stmt = self.conn.prepare(
                    "SELECT role, version, key_id, public_hex, created_at
                     FROM mock_kms_keys WHERE role = ?1
                     ORDER BY role ASC, version ASC",
                )?;
                let iter = stmt.query_map(params![r], row_to_record)?;
                for row in iter {
                    rows.push(row?);
                }
            }
            None => {
                let mut stmt = self.conn.prepare(
                    "SELECT role, version, key_id, public_hex, created_at
                     FROM mock_kms_keys ORDER BY role ASC, version ASC",
                )?;
                let iter = stmt.query_map([], row_to_record)?;
                for row in iter {
                    rows.push(row?);
                }
            }
        }
        Ok(rows)
    }

    /// Highest existing version for `role`, or `None` if no keyring
    /// has been initialised.
    ///
    /// `SELECT MAX(...)` always returns exactly one row (a value or
    /// SQL NULL), so `QueryReturnedNoRows` cannot happen here — `?`
    /// propagates only real errors (schema drift, IO, corruption).
    /// Codex P2 on PR #28 caught that the previous `.ok()` swallowed
    /// every error as `None`, which would have made `cmd_rotate`
    /// silently behave as if the keyring did not exist when in fact
    /// the table was missing or unreadable.
    pub fn mock_kms_current_version(&self, role: &str) -> StorageResult<Option<u32>> {
        let n: Option<i64> = self.conn.query_row(
            "SELECT MAX(version) FROM mock_kms_keys WHERE role = ?1",
            params![role],
            |r| r.get(0),
        )?;
        match n {
            None => Ok(None),
            // version=0 should never exist (we start at 1) — but defend
            // against a tampered DB by treating it as no initialised
            // keyring rather than overflowing into u32::MAX.
            Some(0) => Ok(None),
            Some(v) => Ok(Some(v as u32)),
        }
    }
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<MockKmsKeyRecord> {
    let ts: String = r.get(4)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&ts)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Utc);
    Ok(MockKmsKeyRecord {
        role: r.get(0)?,
        version: r.get::<_, i64>(1)? as u32,
        key_id: r.get(2)?,
        public_hex: r.get(3)?,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(role: &str, version: u32, suffix: &str) -> MockKmsKeyRecord {
        MockKmsKeyRecord {
            role: role.to_string(),
            version,
            key_id: format!("{role}-v{version}"),
            public_hex: format!("aaaa{suffix}"),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-04-28T00:00:00Z")
                .unwrap()
                .into(),
        }
    }

    #[test]
    fn insert_then_list_round_trip() {
        let mut s = Storage::open_in_memory().unwrap();
        let r = rec("audit-mock", 1, "01");
        assert!(s.mock_kms_insert(&r).unwrap());
        let got = s.mock_kms_list(None).unwrap();
        assert_eq!(got, vec![r]);
    }

    #[test]
    fn duplicate_role_version_returns_false() {
        let mut s = Storage::open_in_memory().unwrap();
        let r1 = rec("audit-mock", 1, "01");
        let r2 = MockKmsKeyRecord {
            public_hex: "different-pubkey".to_string(),
            ..rec("audit-mock", 1, "02")
        };
        // PRIMARY KEY is (role, version) AND key_id is UNIQUE — two
        // different attempts to insert at the same coordinate must
        // surface as Ok(false), not silent overwrite.
        assert!(s.mock_kms_insert(&r1).unwrap());
        assert!(!s.mock_kms_insert(&r2).unwrap());
        // The first record wins.
        let got = s.mock_kms_list(None).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].public_hex, "aaaa01");
    }

    #[test]
    fn duplicate_key_id_across_roles_is_rejected() {
        // key_id has a UNIQUE constraint regardless of role — keeps the
        // mapping `key_id -> public_hex` injective across the keyring.
        let mut s = Storage::open_in_memory().unwrap();
        let mut r1 = rec("audit-mock", 1, "01");
        let mut r2 = rec("decision-mock", 1, "02");
        // Force the key_ids to collide (would only happen via a misuse
        // of the CLI; the test pins the storage-level guarantee).
        r1.key_id = "shared-id".to_string();
        r2.key_id = "shared-id".to_string();
        assert!(s.mock_kms_insert(&r1).unwrap());
        assert!(!s.mock_kms_insert(&r2).unwrap());
    }

    #[test]
    fn current_version_returns_max_per_role() {
        let mut s = Storage::open_in_memory().unwrap();
        s.mock_kms_insert(&rec("audit-mock", 1, "01")).unwrap();
        s.mock_kms_insert(&rec("audit-mock", 2, "02")).unwrap();
        s.mock_kms_insert(&rec("audit-mock", 3, "03")).unwrap();
        s.mock_kms_insert(&rec("decision-mock", 1, "11")).unwrap();
        assert_eq!(s.mock_kms_current_version("audit-mock").unwrap(), Some(3));
        assert_eq!(
            s.mock_kms_current_version("decision-mock").unwrap(),
            Some(1)
        );
        assert_eq!(s.mock_kms_current_version("never-seen").unwrap(), None);
    }

    #[test]
    fn list_can_filter_by_role() {
        let mut s = Storage::open_in_memory().unwrap();
        s.mock_kms_insert(&rec("audit-mock", 1, "01")).unwrap();
        s.mock_kms_insert(&rec("decision-mock", 1, "11")).unwrap();
        s.mock_kms_insert(&rec("audit-mock", 2, "02")).unwrap();
        let only_audit = s.mock_kms_list(Some("audit-mock")).unwrap();
        assert_eq!(only_audit.len(), 2);
        assert!(only_audit.iter().all(|r| r.role == "audit-mock"));
        let all = s.mock_kms_list(None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn current_version_propagates_query_errors() {
        // Codex P2 on PR #28: `mock_kms_current_version` previously
        // converted every rusqlite error into `Ok(None)` via `.ok()`,
        // which made schema drift / IO failure indistinguishable from
        // "no keyring initialised yet". After the fix, propagation via
        // `?` means a missing/broken table surfaces as `Err(Sqlite)`
        // and callers (e.g. `mandate key rotate`) refuse to advance.
        //
        // We construct the failure shape by opening Storage normally
        // (so migrations apply), then dropping the table out from
        // under it via raw rusqlite. Re-opening Storage skips the
        // already-recorded V005 migration, so the table stays gone.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let _ = Storage::open(&path).unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute("DROP TABLE mock_kms_keys", []).unwrap();
        }
        let s = Storage::open(&path).unwrap();
        let err = s
            .mock_kms_current_version("audit-mock")
            .expect_err("missing table must propagate as Err, not silently report None");
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("mock_kms_keys") || msg.to_lowercase().contains("no such"),
            "expected the error to mention the missing table; got: {msg}"
        );
    }

    #[test]
    fn keyring_persists_across_storage_reopen() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let mut s = Storage::open(&path).unwrap();
            s.mock_kms_insert(&rec("audit-mock", 1, "01")).unwrap();
            s.mock_kms_insert(&rec("audit-mock", 2, "02")).unwrap();
        }
        let s = Storage::open(&path).unwrap();
        let got = s.mock_kms_list(Some("audit-mock")).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(s.mock_kms_current_version("audit-mock").unwrap(), Some(2));
    }
}
