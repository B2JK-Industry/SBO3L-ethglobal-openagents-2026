use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration drift at v{version}: applied={applied_sha}, expected={expected_sha}")]
    MigrationDrift {
        version: i64,
        applied_sha: String,
        expected_sha: String,
    },
    #[error("audit chain: {0}")]
    Chain(#[from] sbo3l_core::audit::ChainError),
    #[error("audit core: {0}")]
    Core(#[from] sbo3l_core::CoreError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("audit_event id '{id}' not found in chain")]
    AuditEventNotFound { id: String },
    /// Configuration error — bad DATABASE_URL, missing env var, etc.
    /// Used by the Postgres backend (feature `postgres`).
    #[error("configuration: {0}")]
    Configuration(String),
    /// Migration runtime error (Postgres backend). Sqlx propagates the
    /// underlying SQL error into the message.
    #[error("migration: {0}")]
    Migration(String),
}

pub type StorageResult<T> = Result<T, StorageError>;
