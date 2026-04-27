//! Mandate storage: SQLite persistence and audit hash chain.

pub mod audit_store;
pub mod db;
pub mod error;

pub use audit_store::NewAuditEvent;
pub use db::Storage;
pub use error::{StorageError, StorageResult};
