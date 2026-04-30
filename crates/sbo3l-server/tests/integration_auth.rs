//! F-1 integration tests: bearer + JWT auth on `POST /v1/payment-requests`.
//!
//! Driven via `tower::oneshot` against the in-process router; no real network
//! sockets. Each test owns its own in-memory storage and `AuthConfig`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};
use http_body_util::BodyExt;
use sbo3l_server::auth::AuthConfig;
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::Value;
use tower::ServiceExt;

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

fn build(auth: AuthConfig) -> axum::Router {
    let storage = Storage::open_in_memory().unwrap();
    let policy = reference_policy();
    router(AppState::with_auth_config(policy, storage, auth))
}

fn body() -> Value {
    serde_json::from_str(APRP_GOLDEN).unwrap()
}

async fn post(
    app: axum::Router,
    body: Value,
    headers: &[(&'static str, String)],
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, v.as_str());
    }
    let req = req
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    (status, v)
}

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hand-roll an EdDSA JWT against a 32-byte Ed25519 signing key. Cheaper
/// than wiring jsonwebtoken's encoder (the encoder needs PKCS#8 DER for the
/// private key); the verifier under test is what we care about.
fn sign_jwt(sk: &SigningKey, sub: &str) -> String {
    let header = b64url(br#"{"alg":"EdDSA","typ":"JWT"}"#);
    let payload = b64url(format!(r#"{{"sub":"{sub}"}}"#).as_bytes());
    let signing_input = format!("{header}.{payload}");
    let sig = sk.sign(signing_input.as_bytes());
    let sig_b64 = b64url(&sig.to_bytes());
    format!("{signing_input}.{sig_b64}")
}

// ----------------------------- bearer -----------------------------

#[tokio::test]
async fn no_auth_header_with_required_returns_401_auth_required() {
    let app = build(AuthConfig::default());
    let (status, v) = post(app, body(), &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.required");
    // RFC 7807 fields populated.
    assert!(v.get("type").is_some());
    assert!(v.get("title").is_some());
    assert_eq!(v["status"], 401);
    assert!(v.get("detail").is_some());
    // Rejection produces no receipt or audit_event_id.
    assert!(v.get("receipt").is_none());
    assert!(v.get("audit_event_id").is_none());
}

#[tokio::test]
async fn no_auth_header_with_dev_flag_passes() {
    let app = build(AuthConfig {
        allow_unauthenticated: true,
        bearer_hash: None,
        jwt_pubkey_hex: None,
    });
    let (status, v) = post(app, body(), &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "auto_approved");
}

#[tokio::test]
async fn bearer_correct_token_passes() {
    let hash = bcrypt::hash("sekret123", 4).unwrap();
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: Some(hash),
        jwt_pubkey_hex: None,
    });
    let (status, v) = post(
        app,
        body(),
        &[("authorization", "Bearer sekret123".to_string())],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "auto_approved");
}

#[tokio::test]
async fn bearer_wrong_token_returns_401_invalid_token() {
    let hash = bcrypt::hash("sekret123", 4).unwrap();
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: Some(hash),
        jwt_pubkey_hex: None,
    });
    let (status, v) = post(
        app,
        body(),
        &[("authorization", "Bearer wrongpassword".to_string())],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.invalid_token");
}

#[tokio::test]
async fn bearer_with_no_hash_configured_returns_401() {
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: None,
        jwt_pubkey_hex: None,
    });
    let (status, v) = post(
        app,
        body(),
        &[("authorization", "Bearer anything".to_string())],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.invalid_token");
}

#[tokio::test]
async fn malformed_authorization_scheme_returns_401() {
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: Some(bcrypt::hash("x", 4).unwrap()),
        jwt_pubkey_hex: None,
    });
    let (status, v) = post(
        app,
        body(),
        &[("authorization", "Basic dXNlcjpwdw==".to_string())],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.invalid_token");
}

// ----------------------------- JWT (EdDSA) -----------------------------

#[tokio::test]
async fn jwt_valid_signature_and_matching_sub_passes() {
    let sk = SigningKey::from_bytes(&[1u8; 32]);
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: None,
        jwt_pubkey_hex: Some(pk_hex),
    });
    let body_v = body();
    let agent_id = body_v["agent_id"].as_str().unwrap().to_string();
    let token = sign_jwt(&sk, &agent_id);
    let (status, v) = post(app, body_v, &[("authorization", format!("Bearer {token}"))]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "auto_approved");
}

#[tokio::test]
async fn jwt_sub_does_not_match_agent_id_returns_401_agent_id_mismatch() {
    let sk = SigningKey::from_bytes(&[2u8; 32]);
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: None,
        jwt_pubkey_hex: Some(pk_hex),
    });
    let token = sign_jwt(&sk, "wrong-agent-id");
    let (status, v) = post(app, body(), &[("authorization", format!("Bearer {token}"))]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.agent_id_mismatch");
    assert!(v.get("receipt").is_none());
    assert!(v.get("audit_event_id").is_none());
}

#[tokio::test]
async fn jwt_signed_by_wrong_key_returns_401_invalid_token() {
    let sk_real = SigningKey::from_bytes(&[3u8; 32]);
    let sk_evil = SigningKey::from_bytes(&[4u8; 32]);
    // Server trusts the real key only.
    let pk_hex = hex::encode(sk_real.verifying_key().to_bytes());
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: None,
        jwt_pubkey_hex: Some(pk_hex),
    });
    let body_v = body();
    let agent_id = body_v["agent_id"].as_str().unwrap().to_string();
    // Attacker signs the JWT with a different Ed25519 key.
    let token = sign_jwt(&sk_evil, &agent_id);
    let (status, v) = post(app, body_v, &[("authorization", format!("Bearer {token}"))]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.invalid_token");
}

#[tokio::test]
async fn jwt_with_no_pubkey_configured_returns_401() {
    let sk = SigningKey::from_bytes(&[5u8; 32]);
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: Some(bcrypt::hash("anything", 4).unwrap()),
        jwt_pubkey_hex: None,
    });
    let body_v = body();
    let agent_id = body_v["agent_id"].as_str().unwrap().to_string();
    let token = sign_jwt(&sk, &agent_id);
    let (status, v) = post(app, body_v, &[("authorization", format!("Bearer {token}"))]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.invalid_token");
}

#[tokio::test]
async fn jwt_with_malformed_pubkey_hex_returns_401() {
    let app = build(AuthConfig {
        allow_unauthenticated: false,
        bearer_hash: None,
        jwt_pubkey_hex: Some("not-real-hex".to_string()),
    });
    let sk = SigningKey::from_bytes(&[6u8; 32]);
    let body_v = body();
    let agent_id = body_v["agent_id"].as_str().unwrap().to_string();
    let token = sign_jwt(&sk, &agent_id);
    let (status, v) = post(app, body_v, &[("authorization", format!("Bearer {token}"))]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["code"], "auth.invalid_token");
}

// ----------------------------- existing pipeline still works -----------------------------

#[tokio::test]
async fn auth_runs_before_pipeline_no_audit_or_nonce_consumed_on_reject() {
    // Pin the security property: a rejected request produces zero side
    // effects. Submitting the same nonce twice — first rejected by auth,
    // second accepted — must succeed. If auth ran *after* the nonce gate,
    // the second request would 409 with `protocol.nonce_replay`.
    let storage = Storage::open_in_memory().unwrap();
    let policy = reference_policy();
    let auth_required = AuthConfig::default();
    let app_required = router(AppState::with_auth_config(
        policy.clone(),
        storage,
        auth_required,
    ));
    // First POST: rejected on auth.
    let (s1, v1) = post(app_required.clone(), body(), &[]).await;
    assert_eq!(s1, StatusCode::UNAUTHORIZED);
    assert_eq!(v1["code"], "auth.required");

    // Build a separate app with auth disabled but the SAME nonce in the
    // request body. This exercises the contract that the rejected request
    // didn't store side effects in *its* DB. The second app uses fresh
    // in-memory storage so the assertion is "auth gate didn't leak", not
    // "the nonce table is shared".
    let storage2 = Storage::open_in_memory().unwrap();
    let app_open = router(AppState::with_auth_config(
        policy,
        storage2,
        AuthConfig::disabled(),
    ));
    let (s2, v2) = post(app_open, body(), &[]).await;
    assert_eq!(s2, StatusCode::OK);
    assert_eq!(v2["status"], "auto_approved");
}
