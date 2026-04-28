//! SQLite connection + migrations.

use std::path::Path;

use rusqlite::Connection;
use sha2::Digest;

use crate::error::{StorageError, StorageResult};

pub const V001_SQL: &str = include_str!("../../../migrations/V001__init.sql");
pub const V002_SQL: &str = include_str!("../../../migrations/V002__nonce_replay.sql");
pub const V004_SQL: &str = include_str!("../../../migrations/V004__idempotency_keys.sql");

// V003 was reserved for a separate experiment that did not land; numbering
// stays sparse intentionally so future migrations don't have to renumber.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "init", V001_SQL),
    (2, "nonce_replay", V002_SQL),
    (4, "idempotency_keys", V004_SQL),
];

pub struct Storage {
    pub(crate) conn: Connection,
}

impl Storage {
    /// True if a table with the given name exists in the open database.
    /// Used by `mandate doctor` to detect optional tables (e.g.
    /// `idempotency_keys` from a future migration) without forcing a
    /// migration order.
    pub fn table_exists(&self, name: &str) -> StorageResult<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            rusqlite::params![name],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// List of `(version, description)` for every migration that has been
    /// applied against this database. `mandate doctor` prints this to
    /// reassure the operator that storage is current.
    pub fn applied_migrations(&self) -> StorageResult<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT version, description FROM schema_migrations ORDER BY version ASC")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Count rows in a table that may not exist. Returns `Ok(None)` when
    /// the table is missing — handy for the doctor's optional-feature
    /// reporting where a missing table is "skip", not "fail".
    pub fn optional_count(&self, table: &str) -> StorageResult<Option<u64>> {
        if !self.table_exists(table)? {
            return Ok(None);
        }
        // We control `table` (called only with hard-coded names from the
        // CLI), so direct interpolation is fine — SQLite parameter
        // bindings can't substitute table identifiers anyway.
        let sql = format!("SELECT COUNT(*) FROM \"{table}\"");
        let n: i64 = self.conn.query_row(&sql, [], |r| r.get(0))?;
        Ok(Some(n as u64))
    }
}

impl Storage {
    pub fn open_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory().map_err(StorageError::Sqlite)?;
        Self::configure(&conn)?;
        let me = Self { conn };
        me.migrate()?;
        Ok(me)
    }

    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let conn = Connection::open(path).map_err(StorageError::Sqlite)?;
        Self::configure(&conn)?;
        let me = Self { conn };
        me.migrate()?;
        Ok(me)
    }

    fn configure(conn: &Connection) -> StorageResult<()> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(StorageError::Sqlite)?;
        Ok(())
    }

    fn migrate(&self) -> StorageResult<()> {
        // Ensure the migration tracking table exists before we try to read it.
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version     INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at  TEXT NOT NULL,
                    sha256      TEXT NOT NULL
                 )",
            )
            .map_err(StorageError::Sqlite)?;

        for (version, description, sql) in MIGRATIONS {
            let already_applied: Option<String> = self
                .conn
                .query_row(
                    "SELECT sha256 FROM schema_migrations WHERE version = ?1",
                    [version],
                    |r| r.get(0),
                )
                .ok();
            let expected_sha = sha256_hex(sql.as_bytes());
            if let Some(applied_sha) = already_applied {
                if applied_sha != expected_sha {
                    return Err(StorageError::MigrationDrift {
                        version: *version,
                        applied_sha,
                        expected_sha,
                    });
                }
                continue;
            }
            self.conn.execute_batch(sql).map_err(StorageError::Sqlite)?;
            let now = chrono::Utc::now().to_rfc3339();
            self.conn
                .execute(
                    "INSERT INTO schema_migrations(version, description, applied_at, sha256) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![version, description, now, expected_sha],
                )
                .map_err(StorageError::Sqlite)?;
        }
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(bytes))
}
