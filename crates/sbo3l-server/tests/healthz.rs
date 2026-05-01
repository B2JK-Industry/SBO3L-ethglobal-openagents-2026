//! `GET /v1/healthz` integration tests.
//!
//! Dev 3's hosted-app polls this endpoint for daemon-down detection.
//! The wire format is contract-stable; if these tests need updating,
//! the hosted-app probe also needs updating.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::Value;
use tower::ServiceExt;

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

fn build() -> axum::Router {
    let storage = Storage::open_in_memory().unwrap();
    router(AppState::new(reference_policy(), storage))
}

async fn get_healthz(app: axum::Router) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri("/v1/healthz")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    (status, json)
}

#[tokio::test]
async fn healthz_on_fresh_daemon_returns_ok_with_null_chain_head() {
    let app = build();
    let (status, body) = get_healthz(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(
        body["version"].as_str().is_some(),
        "version must be present"
    );
    // Fresh storage — no audit events yet — head is null, length 0.
    assert!(
        body["audit_chain_head"].is_null(),
        "fresh chain has null head, got {body}"
    );
    assert_eq!(body["audit_chain_length"], 0);
    assert!(
        body["uptime_seconds"].as_u64().is_some(),
        "uptime_seconds must be a u64"
    );
}

#[tokio::test]
async fn healthz_after_request_reports_chain_advanced() {
    let app = build();

    // Fire one payment request so the audit chain has a tip.
    let req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json")
        .body(Body::from(APRP_GOLDEN.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let (status, body) = get_healthz(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["audit_chain_length"], 1);
    let head = body["audit_chain_head"].as_str().unwrap_or_default();
    assert!(
        !head.is_empty() && head.chars().all(|c| c.is_ascii_hexdigit()),
        "audit_chain_head must be a non-empty hex string, got {head:?}"
    );
}

#[tokio::test]
async fn healthz_response_shape_is_stable() {
    // Locks the wire contract — Dev 3's hosted-app probe parses these
    // exact keys. Adding a key is fine (additive); removing or
    // renaming any of these is a breaking change.
    let app = build();
    let (_status, body) = get_healthz(app).await;
    for key in [
        "status",
        "version",
        "audit_chain_head",
        "audit_chain_length",
        "uptime_seconds",
    ] {
        assert!(
            body.get(key).is_some(),
            "/v1/healthz response missing key {key}, got {body}"
        );
    }
}

#[tokio::test]
async fn legacy_health_endpoint_still_returns_ok() {
    // /v1/health (the original liveness probe) must keep returning the
    // literal "ok" — callers shipped against this before /v1/healthz
    // existed, and we don't want to break them.
    let app = build();
    let req = Request::builder()
        .method("GET")
        .uri("/v1/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}
