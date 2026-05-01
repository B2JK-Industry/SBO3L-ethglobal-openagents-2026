//! SBO3L HTTP API.
//!
//! Exposes `POST /v1/payment-requests` and `GET /v1/health`. The handler runs
//! the full pipeline: **idempotency-key check** → schema validate →
//! request_hash → **persistent nonce replay claim** → policy decide →
//! budget check → audit append → policy receipt sign. On a successful
//! 200 response with `Idempotency-Key` set, the whole response envelope
//! is cached for safe retry; subsequent retries with the same key + body
//! return the cached envelope without re-running any side-effecting step.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::audit::SignedAuditEvent;
use sbo3l_core::hashing::request_hash;
use sbo3l_core::receipt::{Decision as ReceiptDecision, PolicyReceipt, UnsignedReceipt};
use sbo3l_core::schema;
use sbo3l_core::signer::DevSigner;
use sbo3l_policy::engine::Decision as EngineDecision;
use sbo3l_policy::{decide, BudgetTracker, Policy};
use sbo3l_storage::audit_store::NewAuditEvent;
use sbo3l_storage::idempotency_store::{ClaimOutcome, IdempotencyEntry, IdempotencyState};
use sbo3l_storage::Storage;

pub mod auth;
pub use auth::AuthConfig;

/// `Idempotency-Key` header constraints from `docs/api/openapi.json`.
const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
const IDEMPOTENCY_KEY_MIN_LEN: usize = 16;
const IDEMPOTENCY_KEY_MAX_LEN: usize = 64;

/// F-3: a `failed` idempotency row is held for this many seconds before
/// a same-key retry can reclaim it. Within the window, retries return
/// HTTP 409 `protocol.idempotency_in_flight`.
const IDEMPOTENCY_FAILED_GRACE_SECS: i64 = 60;

#[derive(Clone)]
pub struct AppState(pub Arc<AppInner>);

pub struct AppInner {
    pub policy: Policy,
    pub storage: Mutex<Storage>,
    pub audit_signer: DevSigner,
    pub receipt_signer: DevSigner,
    pub auth: AuthConfig,
}

impl AppState {
    /// Build a server state with **deterministic, public dev signing seeds**
    /// and auth disabled.
    ///
    /// ⚠ DEV ONLY ⚠ — the seeds below are constants in this public repo, so
    /// anyone can forge audit events and policy receipts that pass `verify()`.
    /// Acceptable for the hackathon demo and inline tests; **production
    /// deployments must inject real signers** via `AppState::with_signers`
    /// (or load them from a TEE/HSM-backed signing backend per
    /// `docs/spec/17_interface_contracts.md` §1) **and** must construct via
    /// [`AppState::with_auth_config`] passing an [`AuthConfig`] from
    /// [`AuthConfig::from_env`].
    pub fn new(policy: Policy, storage: Storage) -> Self {
        Self::full(
            policy,
            storage,
            DevSigner::from_seed("audit-signer-v1", [11u8; 32]),
            DevSigner::from_seed("decision-signer-v1", [7u8; 32]),
            AuthConfig::disabled(),
        )
    }

    /// Build a server state with caller-supplied signers and auth disabled.
    pub fn with_signers(
        policy: Policy,
        storage: Storage,
        audit_signer: DevSigner,
        receipt_signer: DevSigner,
    ) -> Self {
        Self::full(
            policy,
            storage,
            audit_signer,
            receipt_signer,
            AuthConfig::disabled(),
        )
    }

    /// Build a server state with the dev signing seeds and a caller-supplied
    /// [`AuthConfig`]. The binary uses this with [`AuthConfig::from_env`].
    pub fn with_auth_config(policy: Policy, storage: Storage, auth: AuthConfig) -> Self {
        Self::full(
            policy,
            storage,
            DevSigner::from_seed("audit-signer-v1", [11u8; 32]),
            DevSigner::from_seed("decision-signer-v1", [7u8; 32]),
            auth,
        )
    }

    /// Build a server state with caller-supplied signers and auth config.
    pub fn full(
        policy: Policy,
        storage: Storage,
        audit_signer: DevSigner,
        receipt_signer: DevSigner,
        auth: AuthConfig,
    ) -> Self {
        Self(Arc::new(AppInner {
            policy,
            storage: Mutex::new(storage),
            audit_signer,
            receipt_signer,
            auth,
        }))
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/payment-requests", post(create_payment_request))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentRequestResponse {
    pub status: PaymentStatus,
    pub decision: ReceiptDecision,
    pub deny_code: Option<String>,
    pub matched_rule_id: Option<String>,
    pub request_hash: String,
    pub policy_hash: String,
    pub audit_event_id: String,
    pub receipt: PolicyReceipt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    AutoApproved,
    Rejected,
    RequiresHuman,
}

#[derive(Debug, Serialize)]
pub struct Problem {
    pub r#type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: String,
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}

pub(crate) fn problem(code: &str, status: u16, title: &str, detail: impl Into<String>) -> Problem {
    Problem {
        r#type: format!("https://schemas.sbo3l.dev/errors/{code}"),
        title: title.to_string(),
        status,
        detail: detail.into(),
        code: code.to_string(),
    }
}

/// Read and validate the optional `Idempotency-Key` header. Empty header
/// → `Ok(None)`. Header present but malformed (non-ASCII, too short, too
/// long per OpenAPI spec) → `Err(Problem)` carrying a 400.
fn extract_idempotency_key(headers: &HeaderMap) -> Result<Option<String>, Problem> {
    let raw = match headers.get(IDEMPOTENCY_KEY_HEADER) {
        Some(v) => v,
        None => return Ok(None),
    };
    let s = raw.to_str().map_err(|_| {
        problem(
            "protocol.idempotency_key_invalid",
            400,
            "Idempotency-Key must be ASCII",
            "non-ASCII bytes in header",
        )
    })?;
    // PR #23 P2 review: the doc-comment above promises an empty header
    // value is treated as absent (`Ok(None)`), but the length check below
    // would otherwise reject `Idempotency-Key:` (empty) as
    // `protocol.idempotency_key_invalid` 400. RFC 7230 §3.2.4 allows
    // empty field-values; HTTP libraries that auto-emit headers from
    // None values can produce them. Treat empty as absent so callers
    // that pass `""` get the no-idempotency path instead of a 400.
    if s.is_empty() {
        return Ok(None);
    }
    if s.len() < IDEMPOTENCY_KEY_MIN_LEN || s.len() > IDEMPOTENCY_KEY_MAX_LEN {
        return Err(problem(
            "protocol.idempotency_key_invalid",
            400,
            "Idempotency-Key length out of range",
            format!(
                "expected {IDEMPOTENCY_KEY_MIN_LEN}..={IDEMPOTENCY_KEY_MAX_LEN} chars, got {}",
                s.len()
            ),
        ));
    }
    Ok(Some(s.to_string()))
}

/// Build a `Response` from a cached idempotency entry. The cached
/// `response_body` is replayed verbatim (string-identical to the original
/// 200 OK body), with `Content-Type: application/json` and the original
/// HTTP status. By design we only ever cache 200 responses, but the
/// builder honours whatever status was stored.
fn cached_response(entry: &IdempotencyEntry) -> Response {
    let status =
        StatusCode::from_u16(entry.response_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        entry.response_body.clone(),
    )
        .into_response()
}

async fn create_payment_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let inner = state.0.clone();

    // F-1: authorize before any side-effecting work, before idempotency
    // lookup, before nonce claim, before policy/audit/signing. A rejected
    // request must produce zero state changes — same property the nonce
    // and idempotency layers depend on.
    if let Err(p) = auth::authorize(&inner.auth, &headers, &body) {
        return p.into_response();
    }

    // Step 0a: extract the optional `Idempotency-Key` header.
    let idempotency_key = match extract_idempotency_key(&headers) {
        Ok(k) => k,
        Err(p) => return p.into_response(),
    };
    // Hash up front: needed for atomic claim (race-safe insert), for
    // cache-replay body matching, and for conflict detection. Defaulting
    // to empty on hash failure keeps the pipeline reachable so schema
    // validation can surface the real problem.
    let body_canonical_hash = request_hash(&body).unwrap_or_default();

    // Step 0b: F-3 atomic claim. Replaces the pre-F-3 lookup-then-post-
    // success-INSERT race. The CLAIM is a single INSERT keyed on the
    // PRIMARY KEY: exactly one concurrent same-key writer wins, every
    // other concurrent same-key request observes the existing row and
    // is rejected here without ever entering the pipeline. The pipeline
    // therefore runs **at most once per Idempotency-Key**, irrespective
    // of how many concurrent retries arrive.
    if let Some(ref key) = idempotency_key {
        let now = Utc::now();
        let mut storage = inner.storage.lock().expect("storage lock");
        match storage.idempotency_try_claim(key, &body_canonical_hash, now) {
            Ok(ClaimOutcome::Claimed) => {
                // We won the race; fall through to the pipeline. The
                // finalize step at the bottom of the handler promotes
                // the row to `succeeded` (200) or `failed` (non-200).
            }
            Ok(ClaimOutcome::Existing(entry)) => {
                drop(storage);
                if let Some(resp) =
                    handle_existing_claim(key, &entry, &body_canonical_hash, now, &inner)
                {
                    return resp;
                }
                // None means "we successfully reclaimed past-grace
                // failed row; proceed with the pipeline".
            }
            Err(e) => {
                drop(storage);
                return problem(
                    "audit.write_failed",
                    500,
                    "idempotency store error",
                    e.to_string(),
                )
                .into_response();
            }
        }
    }

    let pipeline_result = run_pipeline(&inner, body).await;

    // Serialize the response envelope (we need the bytes both for the HTTP
    // response and, on success with idempotency, for the cache row).
    let (status, response_body) = match &pipeline_result {
        Ok(resp) => match serde_json::to_string(resp) {
            Ok(s) => (StatusCode::OK, s),
            Err(e) => {
                let p = problem(
                    "audit.write_failed",
                    500,
                    "response serialisation failed",
                    e.to_string(),
                );
                let body = serde_json::to_string(&p).unwrap_or_default();
                (
                    StatusCode::from_u16(p.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    body,
                )
            }
        },
        Err(p) => {
            let body = serde_json::to_string(p).unwrap_or_default();
            let s = StatusCode::from_u16(p.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (s, body)
        }
    };

    // F-3 finalize: promote the claimed row to `succeeded` (cache the
    // 200 envelope for byte-identical replay) or `failed` (block retries
    // for the grace window so a buggy client can't double-spend by
    // hammering retries). We log but ignore Err(_): the response is
    // correct; a failure to update the row just means the row stays in
    // `processing` until the grace-window reclaim path mops it up.
    if let Some(key) = idempotency_key {
        let mut storage = inner.storage.lock().expect("storage lock");
        if status == StatusCode::OK {
            let _ = storage.idempotency_succeed(&key, status.as_u16(), &response_body);
        } else {
            let _ = storage.idempotency_fail(&key, status.as_u16(), Utc::now());
        }
    }

    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        response_body,
    )
        .into_response()
}

/// Decide what to surface to the client when the F-3 claim observed an
/// existing row for the same `key`. Returns `Some(resp)` on a definitive
/// outcome (cached replay, conflict, or in_flight); returns `None` when
/// the caller successfully reclaimed a stale `failed` row and should
/// proceed with the pipeline.
fn handle_existing_claim(
    key: &str,
    entry: &IdempotencyEntry,
    body_canonical_hash: &str,
    now: chrono::DateTime<Utc>,
    inner: &Arc<AppInner>,
) -> Option<Response> {
    match entry.state {
        IdempotencyState::Succeeded => {
            if entry.request_hash == body_canonical_hash {
                Some(cached_response(entry))
            } else {
                Some(
                    problem(
                        "protocol.idempotency_conflict",
                        409,
                        "Idempotency-Key conflict",
                        format!(
                            "key={key} was used previously with a different canonical request body"
                        ),
                    )
                    .into_response(),
                )
            }
        }
        IdempotencyState::Processing => Some(
            problem(
                "protocol.idempotency_in_flight",
                409,
                "Idempotency-Key in flight",
                format!("key={key} is currently being processed by another request"),
            )
            .into_response(),
        ),
        IdempotencyState::Failed => {
            // Within the grace window every retry gets in_flight. Past
            // the window we attempt an atomic reclaim — the UPDATE is
            // race-safe; only one concurrent reclaimer wins.
            let age = now - entry.created_at;
            if age < chrono::Duration::seconds(IDEMPOTENCY_FAILED_GRACE_SECS) {
                return Some(
                    problem(
                        "protocol.idempotency_in_flight",
                        409,
                        "Idempotency-Key in flight (failed within grace window)",
                        format!(
                            "key={key} failed in last {IDEMPOTENCY_FAILED_GRACE_SECS}s; \
                             retries are blocked during the grace window"
                        ),
                    )
                    .into_response(),
                );
            }
            let mut storage = inner.storage.lock().expect("storage lock");
            match storage.idempotency_try_reclaim_failed(
                key,
                body_canonical_hash,
                now,
                IDEMPOTENCY_FAILED_GRACE_SECS,
            ) {
                Ok(true) => {
                    // Reclaimed; signal "proceed with pipeline".
                    None
                }
                Ok(false) => Some(
                    problem(
                        "protocol.idempotency_in_flight",
                        409,
                        "Idempotency-Key in flight",
                        format!("key={key} reclaim race: another request reclaimed the row first"),
                    )
                    .into_response(),
                ),
                Err(e) => Some(
                    problem(
                        "audit.write_failed",
                        500,
                        "idempotency reclaim failed",
                        e.to_string(),
                    )
                    .into_response(),
                ),
            }
        }
    }
}

async fn run_pipeline(
    inner: &Arc<AppInner>,
    body: Value,
) -> Result<PaymentRequestResponse, Problem> {
    if let Err(e) = schema::validate_aprp(&body) {
        return Err(problem(
            e.code(),
            400,
            "Invalid APRP request",
            e.to_string(),
        ));
    }

    let aprp: PaymentRequest = serde_json::from_value(body.clone()).map_err(|e| {
        problem(
            "schema.wrong_type",
            400,
            "APRP type round-trip failed",
            e.to_string(),
        )
    })?;

    // Replay protection — see `docs/spec/17_interface_contracts.md` §3.1
    // (`protocol.nonce_replay` → HTTP 409). The nonce is claimed against
    // the persistent `nonce_replay` SQLite table (migration V002) *before*
    // any policy / budget / audit / signing work, so:
    //
    // 1. A duplicate nonce is rejected without producing audit or receipt
    //    side effects (the gate short-circuits with the 409 response).
    // 2. Two concurrent requests with the same nonce both attempt the
    //    INSERT; SQLite's PRIMARY KEY constraint serialises them, exactly
    //    one wins, the loser surfaces `Ok(false)` and is rejected.
    // 3. The dedup outlives a daemon restart when persistent storage is
    //    used. The hackathon demo uses `Storage::open_in_memory()`, which
    //    is dropped when the daemon process exits — see "Known limitations"
    //    in `SUBMISSION_NOTES.md`.
    //
    // Fail closed on any other SQLite error: we never silently allow a
    // request when we cannot verify whether its nonce was already seen.
    //
    // Tradeoff: this is rejection-only, not safe-retry. If a downstream
    // step (request_hash, policy decide, audit_append, receipt sign) fails
    // *after* the nonce is claimed, the nonce is permanently consumed —
    // a client retry with the same body will see 409, not the original
    // 5xx. RFC 8470-style `Idempotency-Key` semantics for safe-retry are
    // tracked separately as backlog item PS-P1-02.
    {
        let mut storage = inner.storage.lock().expect("storage lock");
        match storage.nonce_try_claim(&aprp.nonce, &aprp.agent_id, Utc::now()) {
            Ok(true) => {} // fresh — proceed
            Ok(false) => {
                return Err(problem(
                    "protocol.nonce_replay",
                    409,
                    "Nonce has already been used",
                    format!(
                        "agent_id={}, nonce={} — replay rejected",
                        aprp.agent_id, aprp.nonce
                    ),
                ));
            }
            Err(e) => {
                return Err(problem(
                    "audit.write_failed",
                    500,
                    "nonce store error",
                    e.to_string(),
                ));
            }
        }
    }

    let req_hash = request_hash(&body).map_err(|e| {
        problem(
            "transport.tls_handshake",
            500,
            "request hash error",
            e.to_string(),
        )
    })?;

    let outcome = decide(&inner.policy, &aprp).map_err(|e| {
        problem(
            "policy.escalation_required",
            500,
            "policy engine error",
            e.to_string(),
        )
    })?;

    let now = Utc::now();
    let mut final_decision = outcome.decision.clone();
    let mut final_deny_code = outcome.deny_code.clone();

    // F-2: storage-backed budget check. Reads current per-bucket spend
    // from `budget_state` (V008). On a deny we still emit a signed audit
    // row downstream — the budget table itself is left untouched (no
    // persisted side effect).
    if matches!(outcome.decision, EngineDecision::Allow) {
        let storage = inner.storage.lock().expect("storage lock");
        match BudgetTracker::check(&storage, &inner.policy, &aprp, now) {
            Ok(Some(deny)) => {
                final_decision = EngineDecision::Deny;
                final_deny_code = Some(deny.deny_code.to_string());
            }
            Ok(None) => {
                // The actual increment + audit append happens below in a
                // single transaction (`BudgetTracker::commit`), satisfying
                // the F-2 acceptance criterion that "policy + budget +
                // audit wrap in single transaction".
            }
            Err(e) => {
                drop(storage);
                return Err(problem(
                    "policy.config_error",
                    500,
                    "policy config error",
                    e.to_string(),
                ));
            }
        }
    }

    let receipt_decision = match final_decision {
        EngineDecision::Allow => ReceiptDecision::Allow,
        EngineDecision::Deny => ReceiptDecision::Deny,
        EngineDecision::RequiresHuman => ReceiptDecision::RequiresHuman,
    };
    let payment_status = match receipt_decision {
        ReceiptDecision::Allow => PaymentStatus::AutoApproved,
        ReceiptDecision::Deny => PaymentStatus::Rejected,
        ReceiptDecision::RequiresHuman => PaymentStatus::RequiresHuman,
    };

    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "decision".to_string(),
        Value::String(format!("{receipt_decision:?}").to_lowercase()),
    );
    if let Some(c) = &final_deny_code {
        metadata.insert("deny_code".to_string(), Value::String(c.clone()));
    }
    metadata.insert(
        "matched_rule_id".to_string(),
        match &outcome.matched_rule_id {
            Some(id) => Value::String(id.clone()),
            None => Value::Null,
        },
    );
    let audit_event = NewAuditEvent {
        event_type: "policy_decided".to_string(),
        actor: "policy_engine".to_string(),
        subject_id: format!("pr-{}", ulid::Ulid::new()),
        payload_hash: req_hash.clone(),
        metadata,
        policy_version: Some(inner.policy.version),
        policy_hash: Some(outcome.policy_hash.clone()),
        attestation_ref: None,
        ts: now,
    };

    // F-2: on allow, delegate to `BudgetTracker::commit` which wraps
    // budget upserts AND the audit append in a single transaction. On
    // deny / requires_human there is no budget to charge, so we go
    // straight to the same atomic seam (`Storage::finalize_decision`)
    // with an empty increments slice — the two writes always either
    // both land or both roll back, regardless of decision.
    let signed_event: SignedAuditEvent = {
        let mut storage = inner.storage.lock().expect("storage lock");
        if matches!(final_decision, EngineDecision::Allow) {
            BudgetTracker::commit(
                &mut storage,
                &inner.policy,
                &aprp,
                now,
                audit_event,
                &inner.audit_signer,
            )
            .map_err(|e| {
                problem(
                    "audit.write_failed",
                    500,
                    "audit append failed",
                    e.to_string(),
                )
            })?
        } else {
            storage
                .finalize_decision(&[], audit_event, &inner.audit_signer)
                .map_err(|e| {
                    problem(
                        "audit.write_failed",
                        500,
                        "audit append failed",
                        e.to_string(),
                    )
                })?
        }
    };

    let receipt = UnsignedReceipt {
        agent_id: aprp.agent_id.clone(),
        decision: receipt_decision.clone(),
        deny_code: final_deny_code.clone(),
        request_hash: req_hash.clone(),
        policy_hash: outcome.policy_hash.clone(),
        policy_version: Some(inner.policy.version),
        audit_event_id: signed_event.event.id.clone(),
        execution_ref: None,
        issued_at: now,
        expires_at: None,
    }
    .sign(&inner.receipt_signer)
    .map_err(|e| {
        problem(
            "audit.signer_unavailable",
            500,
            "receipt signing failed",
            e.to_string(),
        )
    })?;

    Ok(PaymentRequestResponse {
        status: payment_status,
        decision: receipt_decision,
        deny_code: final_deny_code,
        matched_rule_id: outcome.matched_rule_id.clone(),
        request_hash: req_hash,
        policy_hash: outcome.policy_hash.clone(),
        audit_event_id: signed_event.event.id.clone(),
        receipt,
    })
}

/// Embedded reference policy for development/demo. Production callers should
/// load from `/etc/sbo3l/policies/...`.
pub fn reference_policy() -> Policy {
    Policy::parse_json(include_str!("../policies/reference_low_risk.json"))
        .expect("invariant: bundled reference policy parses")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn build_app() -> Router {
        let storage = Storage::open_in_memory().unwrap();
        let policy = reference_policy();
        let state = AppState::new(policy, storage);
        router(state)
    }

    async fn post_json(app: Router, path: &str, body: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        (status, v)
    }

    fn aprp_value(path: &str) -> Value {
        let raw = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[tokio::test]
    async fn legit_x402_request_is_auto_approved() {
        let app = build_app();
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let (status, v) = post_json(app, "/v1/payment-requests", body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["status"], "auto_approved");
        assert_eq!(v["decision"], "allow");
        assert!(v["deny_code"].is_null());
        assert_eq!(
            v["matched_rule_id"], "allow-small-x402-api-call",
            "got {:?}",
            v["matched_rule_id"]
        );
        assert!(v["receipt"]["signature"]["signature_hex"].is_string());
    }

    #[tokio::test]
    async fn prompt_injection_request_is_rejected() {
        let app = build_app();
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/deny_prompt_injection_request.json"
        ));
        let (status, v) = post_json(app, "/v1/payment-requests", body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["status"], "rejected");
        assert_eq!(v["decision"], "deny");
        let code = v["deny_code"].as_str().unwrap();
        assert!(
            code == "policy.deny_unknown_provider"
                || code == "policy.deny_recipient_not_allowlisted",
            "unexpected deny_code {code}"
        );
    }

    #[tokio::test]
    async fn adversarial_unknown_field_is_rejected_at_400() {
        let app = build_app();
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/adversarial_unknown_field.json"
        ));
        let (status, v) = post_json(app, "/v1/payment-requests", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(v["code"], "schema.unknown_field");
    }

    #[tokio::test]
    async fn replayed_nonce_returns_409_protocol_nonce_replay() {
        // Spec §3.1: a reused APRP nonce must surface as
        // `protocol.nonce_replay` with HTTP 409. Build the app once so both
        // requests share the same `seen_nonces` set, then submit the same
        // body twice. The first request goes through the usual pipeline
        // (auto_approved); the second must be rejected before any policy
        // decision happens.
        let app = build_app();
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));

        let (status1, v1) = post_json(app.clone(), "/v1/payment-requests", body.clone()).await;
        assert_eq!(status1, StatusCode::OK);
        assert_eq!(v1["status"], "auto_approved");

        let (status2, v2) = post_json(app, "/v1/payment-requests", body).await;
        assert_eq!(status2, StatusCode::CONFLICT);
        assert_eq!(v2["code"], "protocol.nonce_replay");
    }

    #[tokio::test]
    async fn distinct_nonces_are_independently_processed() {
        let app = build_app();
        let mut body1 = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let mut body2 = body1.clone();
        body1["nonce"] = Value::String("01HTAWX5K3R8YV9NQB7C6P2DG1".to_string());
        body2["nonce"] = Value::String("01HTAWX5K3R8YV9NQB7C6P2DG2".to_string());
        // distinct task_ids keep request_hash from colliding too
        body1["task_id"] = Value::String("demo-task-A".to_string());
        body2["task_id"] = Value::String("demo-task-B".to_string());

        let (status1, _) = post_json(app.clone(), "/v1/payment-requests", body1).await;
        let (status2, _) = post_json(app, "/v1/payment-requests", body2).await;
        assert_eq!(status1, StatusCode::OK);
        assert_eq!(status2, StatusCode::OK);
    }

    #[tokio::test]
    async fn replay_with_same_nonce_but_mutated_body_is_still_rejected() {
        // Pin the security property: replay protection keys on `nonce`
        // alone, so an attacker cannot bypass the gate by perturbing
        // non-security fields (task_id, amount, etc.) while keeping the
        // captured nonce. The dedup happens before request_hash, policy
        // decide, budget, audit, and signing — so the second response is
        // 409 `protocol.nonce_replay` with no audit/receipt side effects,
        // even though the body differs from the first request.
        let app = build_app();
        let body1 = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let mut body2 = body1.clone();
        // Same nonce as body1; mutate non-security fields.
        body2["task_id"] = Value::String("demo-task-mutated".to_string());
        body2["amount"]["value"] = Value::String("0.04".to_string());
        assert_eq!(
            body1["nonce"], body2["nonce"],
            "test setup: nonce must match"
        );
        assert_ne!(body1, body2, "test setup: bodies must differ");

        let (status1, v1) = post_json(app.clone(), "/v1/payment-requests", body1).await;
        assert_eq!(status1, StatusCode::OK);
        assert_eq!(v1["status"], "auto_approved");

        let (status2, v2) = post_json(app, "/v1/payment-requests", body2).await;
        assert_eq!(status2, StatusCode::CONFLICT);
        assert_eq!(v2["code"], "protocol.nonce_replay");
        // Replay rejection must not produce a receipt or audit_event_id —
        // the response is the Problem object, not PaymentRequestResponse.
        assert!(v2.get("receipt").is_none());
        assert!(v2.get("audit_event_id").is_none());
    }

    #[tokio::test]
    async fn nonce_replay_rejection_persists_across_storage_reopen() {
        // The point of PS-P1-01: replay protection survives a daemon
        // restart against the same SQLite database. Open a tempfile-backed
        // storage, post a request inside one AppState, drop that AppState
        // (and the Storage handle it owns), reopen the same db file in a
        // fresh AppState, and post the same body again. The second post
        // must be rejected with HTTP 409 `protocol.nonce_replay` even
        // though every in-memory cache has been thrown away.
        //
        // This test exercises the storage-layer `nonce_try_claim` end of
        // the gate; the `sbo3l-storage::nonce_store` unit tests cover
        // the SQLite primitives directly.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db_path = tmp.path().to_path_buf();
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));

        // First daemon instance — claims the nonce.
        {
            let storage = Storage::open(&db_path).unwrap();
            let app = router(AppState::new(reference_policy(), storage));
            let (status1, v1) = post_json(app, "/v1/payment-requests", body.clone()).await;
            assert_eq!(status1, StatusCode::OK);
            assert_eq!(v1["status"], "auto_approved");
        }

        // Second daemon instance against the same db — must reject the
        // replay with 409 even though every in-memory state was dropped.
        {
            let storage = Storage::open(&db_path).unwrap();
            let app = router(AppState::new(reference_policy(), storage));
            let (status2, v2) = post_json(app, "/v1/payment-requests", body).await;
            assert_eq!(status2, StatusCode::CONFLICT);
            assert_eq!(v2["code"], "protocol.nonce_replay");
            assert!(v2.get("receipt").is_none());
            assert!(v2.get("audit_event_id").is_none());
        }
    }

    // --------------------------- Idempotency-Key (PSM-A2) ---------------------------
    //
    // Behaviour matrix the tests below pin:
    //
    //  | Request 1                          | Request 2                                  | Outcome                          |
    //  |------------------------------------|--------------------------------------------|----------------------------------|
    //  | K=K1, body=B1, success             | K=K1, body=B1                              | byte-identical cached response   |
    //  | K=K1, body=B1, success             | K=K1, body=B2                              | 409 protocol.idempotency_conflict|
    //  | K=K1, body=B1, success (file db)   | (drop daemon) K=K1, body=B1                | byte-identical cached response   |
    //  | K=K1, body=B1, success             | K=K2, body=B1 (same nonce)                 | 409 protocol.nonce_replay        |
    //  | no K, body=B1, success             | no K, body=B1                              | 409 protocol.nonce_replay (legacy)|
    //  | malformed K (too short / too long) | -                                          | 400 protocol.idempotency_key_invalid |
    //
    // The "K=K1, body=B1, success" / "K=K1, body=B1" pair must NOT add a
    // new audit event (cached path skips the pipeline entirely).

    /// Helper that submits an APRP body with an `Idempotency-Key` header
    /// and returns (status, parsed body, raw bytes). The raw bytes let the
    /// cached-replay test assert byte-identical replay.
    async fn post_json_with_idempotency_key(
        app: Router,
        path: &str,
        body: Value,
        idempotency_key: &str,
    ) -> (StatusCode, Value, Vec<u8>) {
        let req = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .header(IDEMPOTENCY_KEY_HEADER, idempotency_key)
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        (status, v, bytes)
    }

    #[tokio::test]
    async fn idempotency_returns_cached_response_for_same_key_and_body() {
        // First POST consumes the nonce, runs the pipeline, signs a
        // receipt, appends an audit row. Second POST with the same key
        // and body must return the byte-identical response WITHOUT
        // running any side-effecting step. We assert that by:
        //   1. comparing raw response bytes,
        //   2. checking audit_count is unchanged after the retry.
        let storage = Storage::open_in_memory().unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let key = "idem-test-fixed-aaaaaaaaaaaaaaaa"; // 32 chars, in spec range

        let (s1, v1, bytes1) =
            post_json_with_idempotency_key(app.clone(), "/v1/payment-requests", body.clone(), key)
                .await;
        assert_eq!(s1, StatusCode::OK);
        assert_eq!(v1["status"], "auto_approved");
        let receipt_id_1 = v1["audit_event_id"].as_str().unwrap().to_string();

        // Second POST: same key, same body → cached response replayed.
        let (s2, v2, bytes2) =
            post_json_with_idempotency_key(app, "/v1/payment-requests", body, key).await;
        assert_eq!(s2, StatusCode::OK);
        assert_eq!(
            bytes1, bytes2,
            "cached retry must return byte-identical body"
        );
        // Same audit_event_id → confirms we did NOT append a new event.
        assert_eq!(v2["audit_event_id"], receipt_id_1);
    }

    #[tokio::test]
    async fn idempotency_returns_409_conflict_for_same_key_different_body() {
        let storage = Storage::open_in_memory().unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let body1 = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let mut body2 = body1.clone();
        // Mutate to a clearly different canonical body. We also have to
        // change the nonce so that — if conflict didn't fire — the
        // second request would NOT be rejected on nonce-replay grounds
        // (we want to isolate the conflict path for this assertion).
        body2["nonce"] = Value::String("01HFAKEALTERNATENONCEAAAAAA".to_string());
        body2["task_id"] = Value::String("idempotency-conflict-test".to_string());
        let key = "idem-conflict-key-aaaaaaaaaaaaaa"; // 32 chars

        let (s1, _, _) =
            post_json_with_idempotency_key(app.clone(), "/v1/payment-requests", body1, key).await;
        assert_eq!(s1, StatusCode::OK);

        let (s2, v2, _) =
            post_json_with_idempotency_key(app, "/v1/payment-requests", body2, key).await;
        assert_eq!(s2, StatusCode::CONFLICT);
        assert_eq!(v2["code"], "protocol.idempotency_conflict");
        assert!(v2.get("receipt").is_none());
        assert!(v2.get("audit_event_id").is_none());
    }

    #[tokio::test]
    async fn idempotency_persists_across_storage_reopen() {
        // Same key + same body across daemon restart → cached response.
        // This is the production-shape claim: a 5xx-flailing client can
        // safely retry past a daemon bounce without nonce_replay-ing.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db_path = tmp.path().to_path_buf();
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let key = "idem-restart-key-aaaaaaaaaaaaaaaa"; // 32 chars

        let bytes_first;
        {
            let storage = Storage::open(&db_path).unwrap();
            let app = router(AppState::new(reference_policy(), storage));
            let (s, v, raw) =
                post_json_with_idempotency_key(app, "/v1/payment-requests", body.clone(), key)
                    .await;
            assert_eq!(s, StatusCode::OK);
            assert_eq!(v["status"], "auto_approved");
            bytes_first = raw;
        }

        // Drop the AppState and Storage; reopen against the same file.
        let storage = Storage::open(&db_path).unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let (s2, _, raw2) =
            post_json_with_idempotency_key(app, "/v1/payment-requests", body, key).await;
        assert_eq!(s2, StatusCode::OK);
        assert_eq!(
            bytes_first, raw2,
            "cached envelope must replay byte-identically across daemon restart"
        );
    }

    #[tokio::test]
    async fn idempotency_key_too_short_returns_400() {
        let storage = Storage::open_in_memory().unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let too_short = "shortkey";
        let (s, v, _) =
            post_json_with_idempotency_key(app, "/v1/payment-requests", body, too_short).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v["code"], "protocol.idempotency_key_invalid");
    }

    #[tokio::test]
    async fn idempotency_key_too_long_returns_400() {
        let storage = Storage::open_in_memory().unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        // 65 chars — one past the OpenAPI maxLength.
        let too_long = "a".repeat(65);
        let (s, v, _) =
            post_json_with_idempotency_key(app, "/v1/payment-requests", body, &too_long).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v["code"], "protocol.idempotency_key_invalid");
    }

    #[tokio::test]
    async fn nonce_replay_still_protects_when_no_idempotency_key() {
        // Sanity: behaviour without `Idempotency-Key` is unchanged. This
        // is structurally the same scenario as
        // `replayed_nonce_returns_409_protocol_nonce_replay` but kept as
        // a separate, named test because the idempotency layer changes
        // the handler shape; we want a regression test that pins
        // "no header → legacy nonce semantics" specifically.
        let storage = Storage::open_in_memory().unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let (s1, v1) = post_json(app.clone(), "/v1/payment-requests", body.clone()).await;
        assert_eq!(s1, StatusCode::OK);
        assert_eq!(v1["status"], "auto_approved");
        let (s2, v2) = post_json(app, "/v1/payment-requests", body).await;
        assert_eq!(s2, StatusCode::CONFLICT);
        assert_eq!(v2["code"], "protocol.nonce_replay");
    }

    #[tokio::test]
    async fn defense_in_depth_different_idempotency_key_same_nonce_returns_nonce_replay() {
        // Defence in depth. An attacker who captures a successful
        // request body cannot bypass the nonce gate by attaching a fresh
        // Idempotency-Key — the nonce is still consumed, so the second
        // request gets 409 protocol.nonce_replay (not a fresh allow,
        // and not idempotency_conflict either since K2 is unseen).
        let storage = Storage::open_in_memory().unwrap();
        let app = router(AppState::new(reference_policy(), storage));
        let body = aprp_value(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        ));
        let key1 = "idem-defense-key-1-aaaaaaaaaaaa"; // 32 chars
        let key2 = "idem-defense-key-2-bbbbbbbbbbbb"; // 32 chars

        let (s1, _, _) =
            post_json_with_idempotency_key(app.clone(), "/v1/payment-requests", body.clone(), key1)
                .await;
        assert_eq!(s1, StatusCode::OK);

        let (s2, v2, _) =
            post_json_with_idempotency_key(app, "/v1/payment-requests", body, key2).await;
        // The cache miss falls through to the nonce gate → 409.
        assert_eq!(s2, StatusCode::CONFLICT);
        assert_eq!(v2["code"], "protocol.nonce_replay");
    }

    // PR #23 P2 review: the doc-comment on `extract_idempotency_key`
    // promises an empty header value is treated as absent (`Ok(None)`).
    // These three tests pin that contract: absent → None, empty → None,
    // too-short non-empty → 400. Without the empty-string guard the
    // function used to reject `Idempotency-Key:` (empty) as a 400.
    #[test]
    fn extract_idempotency_key_absent_header_is_none() {
        let headers = HeaderMap::new();
        assert!(matches!(extract_idempotency_key(&headers), Ok(None)));
    }

    #[test]
    fn extract_idempotency_key_empty_header_is_treated_as_absent() {
        let mut headers = HeaderMap::new();
        headers.insert(IDEMPOTENCY_KEY_HEADER, "".parse().unwrap());
        assert!(matches!(extract_idempotency_key(&headers), Ok(None)));
    }

    #[test]
    fn extract_idempotency_key_short_non_empty_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(IDEMPOTENCY_KEY_HEADER, "tooshort".parse().unwrap());
        let err = extract_idempotency_key(&headers).unwrap_err();
        // Problem carries a 400 with code protocol.idempotency_key_invalid.
        assert_eq!(err.status, 400);
        assert_eq!(err.code, "protocol.idempotency_key_invalid");
    }
}
