//! SQLite connection + migrations.

use std::path::Path;

use rusqlite::Connection;
use sha2::Digest;

use crate::error::{StorageError, StorageResult};

pub const V001_SQL: &str = include_str!("../../../migrations/V001__init.sql");

const MIGRATIONS: &[(i64, &str, &str)] = &[(1, "init", V001_SQL)];

pub struct Storage {
    pub(crate) conn: Connection,
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
