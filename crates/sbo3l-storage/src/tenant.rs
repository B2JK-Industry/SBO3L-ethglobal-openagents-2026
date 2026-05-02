//! Per-tenant audit chain isolation.
//!
//! Single-instance SBO3L deployments serve one tenant; the
//! pre-existing `Storage::audit_*` methods continue to work
//! unchanged on those, with every event tagged `tenant_id='default'`
//! by the V010 migration's column default.
//!
//! Multi-tenant deployments (`SBO3L_MULTI_TENANT=1`) route every
//! request to a per-tenant chain. Daemon middleware extracts the
//! tenant id from the JWT (`tid` claim — see Dev 3's hosted-app PR
//! #190 for the auth contract) and the handler calls the
//! `*_for_tenant` methods on `Storage`.
//!
//! # Cryptographic isolation
//!
//! Each tenant has their own logical audit chain: `prev_event_hash`
//! links only events with the same `tenant_id`. A tenant cannot
//! forge an event linking into another tenant's chain because the
//! per-tenant `audit_last_for_tenant` query returns only their own
//! tail. Verification of tenant X's chain reads only WHERE
//! `tenant_id=X`, so tampering with tenant Y's events doesn't
//! poison X's verification.
//!
//! # Default tenant
//!
//! [`DEFAULT_TENANT_ID`] is the sentinel value backfilled into
//! every existing row by V010, and the implicit tenant_id used by
//! the legacy `audit_*` methods. Single-tenant deployments never
//! see this string in practice — it's only relevant when an
//! operator flips on multi-tenant mode against a database that
//! previously held single-tenant rows. Those rows remain accessible
//! via `audit_*_for_tenant("default")`.

use serde::{Deserialize, Serialize};

/// Sentinel tenant id for single-tenant deployments + the V010
/// migration's `DEFAULT 'default'` column constraint. Production
/// multi-tenant code never constructs this directly — it always
/// receives a real tenant id from the JWT middleware.
pub const DEFAULT_TENANT_ID: &str = "default";

/// Newtype wrapper around the tenant identifier string. Constructed
/// at the daemon's auth middleware boundary; consumers receive
/// `&TenantId` and can't accidentally pass an arbitrary `&str`
/// (e.g., a user-supplied agent id) where a tenant id is expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(String);

impl TenantId {
    /// Construct from a trusted source. The auth middleware should
    /// be the only call site in production; tests + the
    /// single-tenant-default path use it freely.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The implicit tenant for single-tenant deployments. Backed by
    /// [`DEFAULT_TENANT_ID`].
    pub fn default_tenant() -> Self {
        Self(DEFAULT_TENANT_ID.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TenantId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Mode flag the daemon sets at startup. `Single` is the legacy
/// path — `audit_append` writes everything under `'default'`.
/// `Multi` requires the auth middleware to populate a `TenantId`
/// per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantMode {
    Single,
    Multi,
}

impl TenantMode {
    /// Read `SBO3L_MULTI_TENANT` from the environment.
    /// `SBO3L_MULTI_TENANT=1` → [`TenantMode::Multi`]; anything else
    /// (including unset) → [`TenantMode::Single`].
    pub fn from_env() -> Self {
        match std::env::var("SBO3L_MULTI_TENANT").as_deref() {
            Ok("1") => Self::Multi,
            _ => Self::Single,
        }
    }
}
