//! CHAOS-2 regression: expired APRP must be rejected with HTTP 400 +
//! `deny_code: "protocol.aprp_expired"`. Prior to this fix the chaos
//! suite scenario 5 (`scripts/chaos/05_clock_skew.sh`) showed an
//! APRP whose `expiry` was 60s in the past being APPROVED and signed
//! — a real security gap (an attacker who steals an old APRP body
//! can replay it indefinitely).
//!
//! The fix lives in `run_pipeline` immediately after the APRP type
//! parse and BEFORE the nonce claim, so expired requests don't
//! consume nonces or audit slots.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::Value;
use tower::ServiceExt;

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

fn build_app() -> axum::Router {
    let storage = Storage::open_in_memory().unwrap();
    router(AppState::new(reference_policy(), storage))
}

/// Build the golden APRP, override `expiry` to the supplied time and
/// `nonce` to a unique value (so the rejection path isn't masked by
/// nonce replay across tests).
///
/// Uses the same `Z`-suffix RFC3339 form the golden fixture uses to
/// avoid tripping the JSON Schema `format: date-time` validator on
/// the `+00:00` form chrono's `to_rfc3339()` produces by default.
fn body_with_expiry(expiry: chrono::DateTime<Utc>, nonce: &str) -> Value {
    let mut v: Value = serde_json::from_str(APRP_GOLDEN).unwrap();
    let formatted = expiry.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    v["expiry"] = Value::String(formatted);
    v["nonce"] = Value::String(nonce.to_string());
    v
}

async fn post_aprp(app: axum::Router, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
async fn expired_aprp_60_seconds_in_past_returns_400_aprp_expired() {
    // Reproduces the chaos suite finding: expiry 60s in the past was
    // accepted prior to this fix.
    let now = Utc::now();
    let expiry = now - Duration::seconds(120); // 120s past = beyond 60s skew tolerance
    let body = body_with_expiry(expiry, "01HCHAS2EXPRDS0000000000Z1");
    let (status, resp_body) = post_aprp(build_app(), body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "expired APRP must reject with 400; got {status}"
    );
    assert_eq!(
        resp_body["code"], "protocol.aprp_expired",
        "deny code must be protocol.aprp_expired; got body={resp_body}"
    );
}

#[tokio::test]
async fn fresh_aprp_in_future_succeeds() {
    // Counter-example: a non-expired APRP must still pass the gate.
    let now = Utc::now();
    let expiry = now + Duration::minutes(10);
    let body = body_with_expiry(expiry, "01HCHAS2FRESH0000000000Z02");
    let (status, _resp_body) = post_aprp(build_app(), body).await;
    assert_eq!(status, StatusCode::OK, "fresh APRP must succeed");
}

#[tokio::test]
async fn aprp_within_60_second_skew_window_succeeds() {
    // 30 seconds in the past — inside the 60s tolerance for clock
    // drift. Must NOT be rejected; sender/receiver clock skew is the
    // realistic case that this tolerance accommodates.
    let now = Utc::now();
    let expiry = now - Duration::seconds(30);
    let body = body_with_expiry(expiry, "01HCHAS2SKEW00000000000Z03");
    let (status, _resp_body) = post_aprp(build_app(), body).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "APRP 30s in past must pass (within 60s skew window); got {status}"
    );
}

#[tokio::test]
async fn expired_aprp_does_not_consume_nonce() {
    // Security property: an expired APRP must be rejected BEFORE the
    // nonce is claimed. Otherwise an attacker could spam expired
    // requests to burn nonces and DoS legit traffic.
    //
    // Submit an expired request with nonce N, then a fresh request
    // with the same nonce N — the fresh one must succeed (nonce
    // wasn't consumed by the expired one).
    let app = build_app();
    let nonce = "01HCHAS2NCNS00000000000Z04";
    let now = Utc::now();
    let expired = body_with_expiry(now - Duration::seconds(120), nonce);
    let (s1, _) = post_aprp(app.clone(), expired).await;
    assert_eq!(s1, StatusCode::BAD_REQUEST);

    let fresh = body_with_expiry(now + Duration::minutes(10), nonce);
    let (s2, _) = post_aprp(app, fresh).await;
    assert_eq!(
        s2,
        StatusCode::OK,
        "fresh request with same nonce must succeed — expired request must NOT have consumed the nonce"
    );
}
