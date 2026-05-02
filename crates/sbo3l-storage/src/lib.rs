//! SBO3L storage: SQLite persistence and audit hash chain.
//!
//! Default backend is SQLite (rusqlite). The `postgres` Cargo feature
//! adds a parallel sqlx-postgres backend used by the multi-tenant
//! production deployment — see `crate::pg::PgPool` and
//! `migrations/V020__postgres_init.sql`. The two backends do not share
//! a Storage trait yet; the Postgres rollout (per
//! docs/dev3/production/01-postgres-rls-migration.md) ships dual-write
//! one store at a time.

#[cfg(feature = "postgres")]
pub mod pg;

pub mod audit_checkpoint_store;
pub mod audit_store;
pub mod budget_store;
pub mod db;
pub mod error;
pub mod idempotency_store;
pub mod mock_kms_store;
pub mod nonce_store;
pub mod policy_store;
pub mod tenant;
// Remote upload backends for `sbo3l audit export` (Task C).
pub mod zerog_backend;

pub use audit_checkpoint_store::AuditCheckpointRecord;
pub use audit_store::NewAuditEvent;
pub use budget_store::{usd_str_to_cents, BudgetIncrement, BudgetStateRow};
pub use db::Storage;
pub use error::{StorageError, StorageResult};
pub use policy_store::ActivePolicyRecord;
pub use tenant::{TenantId, TenantMode, DEFAULT_TENANT_ID};
