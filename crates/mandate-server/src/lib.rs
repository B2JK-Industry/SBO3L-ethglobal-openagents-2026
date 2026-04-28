//! Mandate HTTP API.
//!
//! Exposes `POST /v1/payment-requests` and `GET /v1/health`. The handler runs
//! the full pipeline: schema validate → request_hash → **persistent nonce
//! replay claim** → policy decide → budget check → audit append → policy
//! receipt sign.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use mandate_core::aprp::PaymentRequest;
use mandate_core::audit::SignedAuditEvent;
use mandate_core::hashing::request_hash;
use mandate_core::receipt::{Decision as ReceiptDecision, PolicyReceipt, UnsignedReceipt};
use mandate_core::schema;
use mandate_core::signer::DevSigner;
use mandate_policy::engine::Decision as EngineDecision;
use mandate_policy::{decide, BudgetTracker, Policy};
use mandate_storage::audit_store::NewAuditEvent;
use mandate_storage::Storage;

#[derive(Clone)]
pub struct AppState(pub Arc<AppInner>);

pub struct AppInner {
    pub policy: Policy,
    pub storage: Mutex<Storage>,
    pub budgets: Mutex<BudgetTracker>,
    pub audit_signer: DevSigner,
    pub receipt_signer: DevSigner,
}

impl AppState {
    /// Build a server state with **deterministic, public dev signing seeds**.
    ///
    /// ⚠ DEV ONLY ⚠ — the seeds below are constants in this public repo, so
    /// anyone can forge audit events and policy receipts that pass `verify()`.
    /// Acceptable for the hackathon demo and CI; **production deployments
    /// must inject real signers** via `AppState::with_signers` (or load them
    /// from a TEE/HSM-backed signing backend per
    /// `docs/spec/17_interface_contracts.md` §1).
    pub fn new(policy: Policy, storage: Storage) -> Self {
        Self::with_signers(
            policy,
            storage,
            DevSigner::from_seed("audit-signer-v1", [11u8; 32]),
            DevSigner::from_seed("decision-signer-v1", [7u8; 32]),
        )
    }

    /// Build a server state with caller-supplied signers. Use this in any
    /// non-demo deployment.
    pub fn with_signers(
        policy: Policy,
        storage: Storage,
        audit_signer: DevSigner,
        receipt_signer: DevSigner,
    ) -> Self {
        Self(Arc::new(AppInner {
            policy,
            storage: Mutex::new(storage),
            budgets: Mutex::new(BudgetTracker::new()),
            audit_signer,
            receipt_signer,
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

fn problem(code: &str, status: u16, title: &str, detail: impl Into<String>) -> Problem {
    Problem {
        r#type: format!("https://schemas.mandate.dev/errors/{code}"),
        title: title.to_string(),
        status,
        detail: detail.into(),
        code: code.to_string(),
    }
}

async fn create_payment_request(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<PaymentRequestResponse>, Problem> {
    let inner = state.0.clone();

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

    if matches!(outcome.decision, EngineDecision::Allow) {
        let mut budgets = inner.budgets.lock().expect("budget lock");
        match budgets.check(&inner.policy, &aprp, now) {
            Ok(Some(deny)) => {
                final_decision = EngineDecision::Deny;
                final_deny_code = Some(deny.deny_code.to_string());
            }
            Ok(None) => {
                // commit() can fail only with `BudgetError::BadValue` — i.e. a
                // malformed `cap_usd` decimal in the loaded policy. That is a
                // server-side configuration error, not a business denial; use
                // a distinct code so callers don't confuse it with a real cap
                // breach (which surfaces via `Ok(Some(deny))` above with code
                // `budget.hard_cap_exceeded`).
                budgets.commit(&inner.policy, &aprp, now).map_err(|e| {
                    problem(
                        "policy.config_error",
                        500,
                        "policy config error",
                        e.to_string(),
                    )
                })?;
            }
            Err(e) => {
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

    let signed_event: SignedAuditEvent = {
        let mut storage = inner.storage.lock().expect("storage lock");
        storage
            .audit_append(audit_event, &inner.audit_signer)
            .map_err(|e| {
                problem(
                    "audit.write_failed",
                    500,
                    "audit append failed",
                    e.to_string(),
                )
            })?
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

    Ok(Json(PaymentRequestResponse {
        status: payment_status,
        decision: receipt_decision,
        deny_code: final_deny_code,
        matched_rule_id: outcome.matched_rule_id.clone(),
        request_hash: req_hash,
        policy_hash: outcome.policy_hash.clone(),
        audit_event_id: signed_event.event.id.clone(),
        receipt,
    }))
}

/// Embedded reference policy for development/demo. Production callers should
/// load from `/etc/mandate/policies/...`.
pub fn reference_policy() -> Policy {
    Policy::parse_json(include_str!(
        "../../../test-corpus/policy/reference_low_risk.json"
    ))
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
        // the gate; the `mandate-storage::nonce_store` unit tests cover
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
}
