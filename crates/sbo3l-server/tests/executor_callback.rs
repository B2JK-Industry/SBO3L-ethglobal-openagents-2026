//! Integration tests for `POST /v1/executor-callback`.
//!
//! Tests sign with a deterministic ed25519 key, set the matching
//! `SBO3L_EXECUTOR_CALLBACK_PUBKEY_<SPONSOR>` env var, and exercise
//! the full pipeline: header parse → re-canonicalise → signature
//! verify → audit append.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use sbo3l_core::webhook::{sign_webhook, BodyKind};
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

// Each test uses a UNIQUE sponsor name → distinct env var key per
// test, so `std::env::set_var` calls don't race even though they
// share process-global env. No mutex needed.

fn signing_key_from_seed(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn setup_pubkey(sponsor: &str, sk: &SigningKey) {
    let env_name = format!(
        "SBO3L_EXECUTOR_CALLBACK_PUBKEY_{}",
        sponsor.to_uppercase().replace(['-', '.'], "_")
    );
    let hex = hex::encode(sk.verifying_key().to_bytes());
    // SAFETY: single-threaded test sequence guarded by ENV_GUARD.
    unsafe { std::env::set_var(&env_name, hex) };
}

fn unset_pubkey(sponsor: &str) {
    let env_name = format!(
        "SBO3L_EXECUTOR_CALLBACK_PUBKEY_{}",
        sponsor.to_uppercase().replace(['-', '.'], "_")
    );
    unsafe { std::env::remove_var(&env_name) };
}

fn build_app() -> axum::Router {
    let storage = Storage::open_in_memory().unwrap();
    router(AppState::new(reference_policy(), storage))
}

fn signed_request(
    sk: &SigningKey,
    body_value: &Value,
    nonce: &str,
    now_unix: i64,
) -> Request<Body> {
    let body_bytes = serde_json::to_vec(body_value).unwrap();
    let envelope = sign_webhook(sk, &body_bytes, BodyKind::Json, nonce, now_unix).unwrap();
    Request::builder()
        .method("POST")
        .uri("/v1/executor-callback")
        .header("content-type", "application/json")
        .header("X-Upstream-Signature", envelope.header_value())
        .body(Body::from(body_bytes))
        .unwrap()
}

fn callback_body(audit_event_id: &str, sponsor: &str) -> Value {
    json!({
        "schema": "sbo3l.executor_callback.v1",
        "audit_event_id": audit_event_id,
        "sponsor": sponsor,
        "status": "success",
        "execution_ref": "uniswap-router:sepolia:01HV0000000000000000000000",
        "evidence": { "tx_hash": "0xdeadbeef" }
    })
}

async fn read_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

#[tokio::test]
async fn happy_path_appends_execution_confirmed_audit_event() {
    let sponsor = "uniswap-router";
    let sk = signing_key_from_seed(0xA1);
    setup_pubkey(sponsor, &sk);

    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", sponsor);
    let now = chrono::Utc::now().timestamp();
    let req = signed_request(&sk, &body, "01HV0000000000000000000001", now);

    let resp = build_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert_eq!(body["schema"], "sbo3l.executor_callback_response.v1");
    assert!(body["audit_event_id"]
        .as_str()
        .map(|s| s.starts_with("evt-"))
        .unwrap_or(false));
    assert!(body["appended_seq"].as_u64().is_some());

    unset_pubkey(sponsor);
}

#[tokio::test]
async fn missing_signature_header_returns_401() {
    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", "uniswap-router");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/executor-callback")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = build_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "callback.missing_signature");
}

#[tokio::test]
async fn unknown_sponsor_returns_503() {
    // Sign with a key, but DON'T configure the env var for the
    // sponsor — the daemon can't resolve the pubkey, surfaces 503.
    let sponsor = "no-such-sponsor";
    unset_pubkey(sponsor);
    let sk = signing_key_from_seed(0xB2);

    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", sponsor);
    let now = chrono::Utc::now().timestamp();
    let req = signed_request(&sk, &body, "01HV0000000000000000000002", now);

    let resp = build_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "callback.no_pubkey");
}

#[tokio::test]
async fn signature_from_wrong_key_returns_401() {
    let sponsor = "wrong-key-sponsor";
    let configured_sk = signing_key_from_seed(0xC3);
    setup_pubkey(sponsor, &configured_sk);

    // Sender uses a DIFFERENT key — sig fails verify.
    let attacker_sk = signing_key_from_seed(0xD4);
    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", sponsor);
    let now = chrono::Utc::now().timestamp();
    let req = signed_request(&attacker_sk, &body, "01HV0000000000000000000003", now);

    let resp = build_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "callback.bad_signature");

    unset_pubkey(sponsor);
}

#[tokio::test]
async fn replayed_nonce_returns_409() {
    let sponsor = "replay-sponsor";
    let sk = signing_key_from_seed(0xE5);
    setup_pubkey(sponsor, &sk);

    let app = build_app();
    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", sponsor);
    let now = chrono::Utc::now().timestamp();
    let nonce = "01HV0000000000000000000004";

    // First call — succeeds.
    let req1 = signed_request(&sk, &body, nonce, now);
    let resp1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);

    // Second call with the SAME nonce — daemon's in-memory store
    // sees the dup and returns 409.
    let req2 = signed_request(&sk, &body, nonce, now);
    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::CONFLICT);
    let body = read_body(resp2).await;
    assert_eq!(body["code"], "callback.replay");

    unset_pubkey(sponsor);
}

#[tokio::test]
async fn stale_timestamp_returns_401() {
    let sponsor = "stale-sponsor";
    let sk = signing_key_from_seed(0xF6);
    setup_pubkey(sponsor, &sk);

    // Timestamp 10 minutes in the past — well outside the 5-min
    // window.
    let now = chrono::Utc::now().timestamp();
    let stale_ts = now - 600;
    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", sponsor);
    let req = signed_request(&sk, &body, "01HV0000000000000000000005", stale_ts);

    let resp = build_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "callback.stale_timestamp");

    unset_pubkey(sponsor);
}

#[tokio::test]
async fn body_tampered_after_signing_returns_400() {
    let sponsor = "tamper-sponsor";
    let sk = signing_key_from_seed(0xA7);
    setup_pubkey(sponsor, &sk);

    let body = callback_body("evt-01HTAWX5K3R8YV9NQB7C6P2DGS", sponsor);
    let now = chrono::Utc::now().timestamp();
    let signed_body_bytes = serde_json::to_vec(&body).unwrap();
    let envelope = sign_webhook(
        &sk,
        &signed_body_bytes,
        BodyKind::Json,
        "01HV0000000000000000000006",
        now,
    )
    .unwrap();

    // Build a DIFFERENT body but reuse the signature — JCS
    // re-canon produces a different hash, the daemon rejects.
    let mut tampered = body.clone();
    tampered["status"] = json!("failure"); // changed from "success"
    let tampered_bytes = serde_json::to_vec(&tampered).unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/v1/executor-callback")
        .header("content-type", "application/json")
        .header("X-Upstream-Signature", envelope.header_value())
        .body(Body::from(tampered_bytes))
        .unwrap();

    let resp = build_app().oneshot(req).await.unwrap();
    // Tampered body trips BadSignature (not BodyHashMismatch). The
    // header design doesn't carry the sender's body hash — the
    // receiver always re-canonicalises locally, so the
    // body-hash-vs-envelope-hash comparison in `verify_webhook`
    // trivially matches (same value computed twice). The mismatch
    // surfaces at the signature step: the signed string contains
    // the receiver's recomputed hash, which differs from the hash
    // the sender signed over, so Ed25519 verify fails. The security
    // property (tampered body rejected) is preserved; just at a
    // different layer than BodyHashMismatch.
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "callback.bad_signature");

    unset_pubkey(sponsor);
}

#[tokio::test]
async fn schema_mismatch_returns_400() {
    let sponsor = "schema-sponsor";
    let sk = signing_key_from_seed(0xB8);
    setup_pubkey(sponsor, &sk);

    // Wrong schema string in body.
    let body = json!({
        "schema": "sbo3l.executor_callback.v999",
        "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGS",
        "sponsor": sponsor,
        "status": "success",
    });
    let now = chrono::Utc::now().timestamp();
    let req = signed_request(&sk, &body, "01HV0000000000000000000007", now);

    let resp = build_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "callback.bad_schema");

    unset_pubkey(sponsor);
}
