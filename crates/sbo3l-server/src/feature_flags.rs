//! Hot-reloadable feature flags + admin endpoints.
//!
//! Operators flip flags at runtime to gate experimental code paths
//! (`flag.experimental_v2_executor`, `flag.dry_run_uniswap`,
//! `flag.relax_replay_window`) without restarting the daemon. Every
//! flag mutation appends a `flag_change` audit event so a future
//! incident review can answer "when did flag X flip?" against the
//! cryptographically-chained log.
//!
//! # Surface
//!
//! - `GET /v1/admin/flags` — returns the current flag set as JSON.
//! - `POST /v1/admin/flags` — body `{ "key": "...", "enabled": true|false }`
//!   sets the flag, appends an audit event, and returns the new value.
//!
//! Both routes require admin auth via `Authorization: Bearer <token>`
//! where `<token>` matches the bcrypt hash in
//! `SBO3L_ADMIN_BEARER_HASH`. This is a SEPARATE credential from the
//! regular per-agent JWT — admin actions are operator-grade, not
//! agent-grade. If the env var is unset the admin endpoints refuse
//! with 503 (operator misconfiguration).
//!
//! # Initial flag state
//!
//! On daemon startup `FlagStore::from_env()` walks the environment
//! for `SBO3L_FLAG_<KEY>=true|false` pairs and seeds the in-memory
//! map. After startup the env is no longer consulted — POST is the
//! only update path. This keeps the audit trail authoritative; an
//! operator who flips an env var mid-run + restarts gets a fresh
//! seed but the audit chain shows the explicit POST history that
//! preceded the restart.
//!
//! # Concurrency
//!
//! The store is `Arc<RwLock<HashMap<String, FeatureFlag>>>` —
//! reads (the hot path: every request that gates on a flag) take a
//! shared read lock, writes (admin POSTs) take an exclusive write
//! lock. Tokio's request handlers grab + drop the guard inside a
//! single sync block to avoid holding it across an await point.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use sbo3l_storage::audit_store::NewAuditEvent;

use crate::AppState;

/// Env-var prefix for seed flags. `SBO3L_FLAG_DRY_RUN_UNISWAP=true`
/// becomes `flag.dry_run_uniswap = true` at startup. Lowercase + the
/// `flag.` namespace mirror the convention agent_id / deny_code use,
/// so an audit-log reader sees a familiar shape.
pub const FLAG_ENV_PREFIX: &str = "SBO3L_FLAG_";

/// Audit event_type emitted on every successful flag mutation.
/// Stable string — auditors filter the chain on this.
pub const FLAG_CHANGE_EVENT_TYPE: &str = "flag_change";

/// Admin bearer-token env var. Holds the bcrypt hash of the admin
/// secret (NOT the secret itself); bcrypt comparison is constant-
/// time-ish and slow enough to defang offline brute-force on a
/// leaked hash. Mirrors the F-1 pattern for `SBO3L_BEARER_HASH`.
pub const ADMIN_BEARER_HASH_ENV: &str = "SBO3L_ADMIN_BEARER_HASH";

/// One flag's wire shape — emitted by GET, accepted on POST.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureFlag {
    pub key: String,
    pub enabled: bool,
    /// RFC3339 timestamp of the last mutation (or daemon startup
    /// for env-seeded flags).
    pub updated_at: DateTime<Utc>,
    /// Subject_id-style actor that last changed the flag.
    /// `"env"` for startup-seeded, `"admin"` for POST mutations.
    pub last_actor: String,
}

#[derive(Default)]
struct StoreInner {
    flags: HashMap<String, FeatureFlag>,
}

/// Hot-reloadable flag store. Cheap to clone (Arc) — handlers take
/// a clone, drop the guard before any await, and re-take on the
/// next operation. The store's contents live as long as the
/// daemon process; restart re-seeds from env.
#[derive(Clone, Default)]
pub struct FlagStore {
    inner: Arc<RwLock<StoreInner>>,
}

impl FlagStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed from env vars matching `SBO3L_FLAG_<KEY>=true|false`.
    /// Anything else (including `1`/`0`, `yes`/`no`) is ignored —
    /// stricter parsing avoids ambiguity at the boundary.
    pub fn from_env() -> Self {
        let store = Self::new();
        for (k, v) in std::env::vars() {
            let Some(suffix) = k.strip_prefix(FLAG_ENV_PREFIX) else {
                continue;
            };
            if suffix.is_empty() {
                continue;
            }
            let enabled = match v.as_str() {
                "true" => true,
                "false" => false,
                _ => continue,
            };
            let key = format!("flag.{}", suffix.to_lowercase());
            let mut inner = store.inner.write().expect("flag store poisoned");
            inner.flags.insert(
                key.clone(),
                FeatureFlag {
                    key,
                    enabled,
                    updated_at: Utc::now(),
                    last_actor: "env".to_string(),
                },
            );
        }
        store
    }

    /// Read a flag. Unknown keys default to `false` — feature gates
    /// are opt-in by construction so a typo or missing-on-restart
    /// flag fails closed.
    pub fn is_enabled(&self, key: &str) -> bool {
        let inner = self.inner.read().expect("flag store poisoned");
        inner.flags.get(key).map(|f| f.enabled).unwrap_or(false)
    }

    /// Snapshot every flag, sorted by key. Returned as `Vec` so the
    /// JSON wire shape is stable across calls (HashMap iteration
    /// order isn't).
    pub fn list(&self) -> Vec<FeatureFlag> {
        let inner = self.inner.read().expect("flag store poisoned");
        let mut out: Vec<FeatureFlag> = inner.flags.values().cloned().collect();
        out.sort_by(|a, b| a.key.cmp(&b.key));
        out
    }

    /// Set a flag's value. Returns the resulting [`FeatureFlag`] (a
    /// fresh `updated_at` + the supplied `actor`). Caller is
    /// responsible for the audit-event append — handler code does
    /// it after the lock drops.
    pub fn set(&self, key: &str, enabled: bool, actor: &str) -> FeatureFlag {
        let now = Utc::now();
        let flag = FeatureFlag {
            key: key.to_string(),
            enabled,
            updated_at: now,
            last_actor: actor.to_string(),
        };
        let mut inner = self.inner.write().expect("flag store poisoned");
        inner.flags.insert(key.to_string(), flag.clone());
        flag
    }
}

/// Wire shape for `POST /v1/admin/flags`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetFlagRequest {
    pub key: String,
    pub enabled: bool,
}

/// Wire shape for `GET /v1/admin/flags`.
#[derive(Debug, Serialize)]
pub struct ListFlagsResponse {
    pub schema: &'static str,
    pub flags: Vec<FeatureFlag>,
}

const LIST_SCHEMA: &str = "sbo3l.feature_flags_list.v1";

/// Wire shape for `POST /v1/admin/flags` success.
#[derive(Debug, Serialize)]
pub struct SetFlagResponse {
    pub schema: &'static str,
    pub flag: FeatureFlag,
    pub audit_event_id: String,
}

const SET_SCHEMA: &str = "sbo3l.feature_flag_set.v1";

#[derive(Debug, Serialize)]
struct Problem {
    code: &'static str,
    detail: String,
}

fn problem(status: StatusCode, code: &'static str, detail: impl Into<String>) -> Response {
    (
        status,
        Json(Problem {
            code,
            detail: detail.into(),
        }),
    )
        .into_response()
}

/// Admin auth check. Returns Ok on a valid bearer; problem-shape
/// 401/503 on rejection. Distinct from the per-agent JWT path —
/// admin tokens are operator-grade and there's no per-agent claim
/// to compare against.
///
/// `Response` is large but always boxed by the caller pattern
/// (handler returns `Response` directly), so the size lint here is
/// noise for our usage.
#[allow(clippy::result_large_err)]
fn admin_authorized(headers: &HeaderMap) -> Result<(), Response> {
    let hash = std::env::var(ADMIN_BEARER_HASH_ENV).map_err(|_| {
        problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "admin.no_credential_configured",
            format!("admin endpoints require {ADMIN_BEARER_HASH_ENV} to be set"),
        )
    })?;
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.strip_prefix("Bearer ")
                .or_else(|| s.strip_prefix("bearer "))
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            problem(
                StatusCode::UNAUTHORIZED,
                "admin.missing_token",
                "Authorization: Bearer <token> required",
            )
        })?;
    if bcrypt::verify(token.as_bytes(), &hash).unwrap_or(false) {
        Ok(())
    } else {
        Err(problem(
            StatusCode::UNAUTHORIZED,
            "admin.invalid_token",
            "admin token rejected",
        ))
    }
}

pub async fn list_flags_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(resp) = admin_authorized(&headers) {
        return resp;
    }
    let flags = state.0.feature_flags.list();
    Json(ListFlagsResponse {
        schema: LIST_SCHEMA,
        flags,
    })
    .into_response()
}

pub async fn set_flag_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetFlagRequest>,
) -> Response {
    if let Err(resp) = admin_authorized(&headers) {
        return resp;
    }
    if req.key.is_empty() {
        return problem(
            StatusCode::BAD_REQUEST,
            "admin.bad_key",
            "key must be non-empty",
        );
    }
    let inner = &state.0;
    let flag = inner.feature_flags.set(&req.key, req.enabled, "admin");

    // Append audit event so the chain captures the change. Lock is
    // taken briefly + dropped before we build the JSON response.
    let mut metadata = serde_json::Map::new();
    metadata.insert("flag_key".into(), Value::String(req.key.clone()));
    metadata.insert("enabled".into(), Value::Bool(req.enabled));
    metadata.insert("actor".into(), Value::String("admin".to_string()));
    let new_event = NewAuditEvent {
        event_type: FLAG_CHANGE_EVENT_TYPE.to_string(),
        actor: "admin".to_string(),
        subject_id: req.key.clone(),
        payload_hash: format!("{:0>64}", "0"),
        metadata,
        policy_version: None,
        policy_hash: None,
        attestation_ref: None,
        ts: Utc::now(),
    };
    let signed = match inner.storage.lock() {
        Ok(mut s) => match s.audit_append(new_event, &inner.audit_signer) {
            Ok(ev) => ev,
            Err(e) => {
                return problem(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "admin.audit_write_failed",
                    e.to_string(),
                );
            }
        },
        Err(_) => {
            return problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "admin.storage_unreachable",
                "storage mutex poisoned",
            );
        }
    };
    Json(SetFlagResponse {
        schema: SET_SCHEMA,
        flag,
        audit_event_id: signed.event.id,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_key_defaults_to_false() {
        let store = FlagStore::new();
        assert!(!store.is_enabled("flag.never_set"));
    }

    #[test]
    fn set_then_is_enabled_round_trip() {
        let store = FlagStore::new();
        store.set("flag.experimental", true, "test");
        assert!(store.is_enabled("flag.experimental"));
        store.set("flag.experimental", false, "test");
        assert!(!store.is_enabled("flag.experimental"));
    }

    #[test]
    fn list_returns_all_flags_sorted_by_key() {
        let store = FlagStore::new();
        store.set("flag.zebra", true, "test");
        store.set("flag.alpha", false, "test");
        store.set("flag.mango", true, "test");
        let flags = store.list();
        let keys: Vec<&str> = flags.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["flag.alpha", "flag.mango", "flag.zebra"]);
    }

    #[test]
    fn set_records_actor_and_updates_timestamp() {
        let store = FlagStore::new();
        let f = store.set("flag.x", true, "admin");
        assert_eq!(f.last_actor, "admin");
        // Sanity: timestamp is recent (within last second).
        let drift = (Utc::now() - f.updated_at).num_milliseconds().abs();
        assert!(drift < 1000, "drift {drift}ms exceeds 1s budget");
    }

    #[test]
    fn from_env_seeds_flags_with_documented_prefix_and_lowercases_key() {
        // SAFETY: serial test that mutates process env.
        unsafe {
            std::env::set_var("SBO3L_FLAG_DRY_RUN_UNISWAP", "true");
            std::env::set_var("SBO3L_FLAG_RELAX_REPLAY_WINDOW", "false");
            // Garbage value — must be ignored, not coerced.
            std::env::set_var("SBO3L_FLAG_AMBIGUOUS", "yes");
        }
        let store = FlagStore::from_env();
        assert!(store.is_enabled("flag.dry_run_uniswap"));
        assert!(!store.is_enabled("flag.relax_replay_window"));
        assert!(!store.is_enabled("flag.ambiguous"));
        let listed_keys: Vec<String> = store.list().into_iter().map(|f| f.key).collect();
        assert!(listed_keys.contains(&"flag.dry_run_uniswap".to_string()));
        assert!(listed_keys.contains(&"flag.relax_replay_window".to_string()));
        assert!(!listed_keys.contains(&"flag.ambiguous".to_string()));
        unsafe {
            std::env::remove_var("SBO3L_FLAG_DRY_RUN_UNISWAP");
            std::env::remove_var("SBO3L_FLAG_RELAX_REPLAY_WINDOW");
            std::env::remove_var("SBO3L_FLAG_AMBIGUOUS");
        }
    }

    #[test]
    fn from_env_marks_seeded_flags_with_env_actor() {
        unsafe {
            std::env::set_var("SBO3L_FLAG_FROM_ENV_ACTOR_TEST", "true");
        }
        let store = FlagStore::from_env();
        let f = store
            .list()
            .into_iter()
            .find(|f| f.key == "flag.from_env_actor_test")
            .expect("seeded flag");
        assert_eq!(f.last_actor, "env");
        unsafe {
            std::env::remove_var("SBO3L_FLAG_FROM_ENV_ACTOR_TEST");
        }
    }

    #[test]
    fn empty_suffix_after_prefix_does_not_seed_flag() {
        unsafe {
            std::env::set_var("SBO3L_FLAG_", "true");
        }
        let store = FlagStore::from_env();
        assert_eq!(
            store.list().iter().filter(|f| f.key == "flag.").count(),
            0,
            "empty key must not be seeded"
        );
        unsafe {
            std::env::remove_var("SBO3L_FLAG_");
        }
    }

    #[test]
    fn store_clone_shares_state() {
        // FlagStore is Arc-backed — cloning shares the same map, so
        // a write on one handle is visible on the other. The handler
        // wires this assumption into the AppInner pattern.
        let store_a = FlagStore::new();
        let store_b = store_a.clone();
        store_a.set("flag.shared", true, "a");
        assert!(store_b.is_enabled("flag.shared"));
        store_b.set("flag.shared", false, "b");
        assert!(!store_a.is_enabled("flag.shared"));
    }
}
