//! Postgres backend (feature `postgres`).
//!
//! SQLite remains the default backend for development. Postgres is the
//! production target — see docs/dev3/production/01-postgres-rls-migration.md
//! for the migration plan.
//!
//! Connection-per-request: every per-tenant transaction must call
//! [`PgPool::tenant_tx`] which sets `app.tenant_uuid` GUC inside the
//! transaction so the RLS policies in V020 fire. The GUC is scoped to
//! the transaction (`SET LOCAL`) so commit/rollback resets it.

#![cfg(feature = "postgres")]

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::{StorageError, StorageResult};

/// Migration SQL, baked at compile time. The sqlx migrator wants its
/// files in a directory; we prefer `include_str!` for hermeticity since
/// the file is only consumed by [`PgPool::run_migrations`] below.
pub const V020_PG_INIT: &str = include_str!("../migrations/V020__postgres_init.sql");

/// Wrapper over `sqlx::PgPool` that exposes the SBO3L-specific transaction
/// helpers. Hand the daemon a single instance; clone it freely (PgPool is
/// cheap to clone — refs the same inner connection pool).
#[derive(Clone)]
pub struct PgPool {
    inner: sqlx::PgPool,
}

#[derive(Clone, Debug)]
pub struct PgConfig {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout_secs: u64,
    pub idle_timeout_secs: u64,
}

impl PgConfig {
    pub fn from_env() -> StorageResult<Self> {
        let url = std::env::var("DATABASE_URL").map_err(|_| {
            StorageError::Configuration("DATABASE_URL env var not set".into())
        })?;
        Ok(Self {
            url,
            max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
            acquire_timeout_secs: 8,
            idle_timeout_secs: 600,
        })
    }
}

impl PgPool {
    pub async fn connect(config: PgConfig) -> StorageResult<Self> {
        let opts: PgConnectOptions = config
            .url
            .parse()
            .map_err(|e: sqlx::Error| StorageError::Configuration(e.to_string()))?;
        let inner = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
            .idle_timeout(Some(Duration::from_secs(config.idle_timeout_secs)))
            .connect_with(opts)
            .await
            .map_err(|e| StorageError::Configuration(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Apply V020. Idempotent on re-run thanks to the
    /// CREATE … IF NOT EXISTS clauses in the migration. Future versions
    /// will switch to `sqlx migrate` proper once we have multiple
    /// PG migrations to manage.
    pub async fn run_migrations(&self) -> StorageResult<()> {
        sqlx::raw_sql(V020_PG_INIT)
            .execute(&self.inner)
            .await
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        Ok(())
    }

    /// Begin a transaction with the tenant-isolation GUC set. **Always**
    /// use this for per-tenant queries — never .begin() directly, or the
    /// RLS policies in V020 will return zero rows (which is the safe
    /// default but probably not what you wanted).
    pub async fn tenant_tx(
        &self,
        tenant_uuid: Uuid,
    ) -> StorageResult<Transaction<'_, Postgres>> {
        let mut tx = self
            .inner
            .begin()
            .await
            .map_err(|e| StorageError::Configuration(e.to_string()))?;
        let stmt = format!("SET LOCAL app.tenant_uuid = '{}'", tenant_uuid);
        sqlx::raw_sql(&stmt)
            .execute(&mut *tx)
            .await
            .map_err(|e| StorageError::Configuration(e.to_string()))?;
        Ok(tx)
    }

    /// Admin-scope transaction without the tenant GUC. Use only for
    /// global operations (tenant CRUD, memberships, stripe_events).
    /// Never for per-tenant data.
    pub async fn admin_tx(&self) -> StorageResult<Transaction<'_, Postgres>> {
        self.inner
            .begin()
            .await
            .map_err(|e| StorageError::Configuration(e.to_string()))
    }

    /// Raw pool ref for code that already takes `&PgPool` (e.g. sqlx
    /// extension traits). Prefer [`tenant_tx`] for new query sites.
    pub fn raw(&self) -> &sqlx::PgPool {
        &self.inner
    }
}
