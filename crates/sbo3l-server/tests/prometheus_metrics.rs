//! R13 P7 — end-to-end test for `GET /v1/metrics` (Prometheus text)
//! and the JSON dashboard's switch from placeholder zeros to live data.
//!
//! Two flows:
//!
//! 1. **Fresh daemon: counters are zero, histogram is parseable.**
//! 2. **After a real POST: counters increment, latency bucket
//!    advances, JSON dashboard reads the same numbers.**

use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::Value;
use std::time::Duration;
use tokio::net::TcpListener;

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

async fn spawn_server() -> String {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    let state = AppState::new(reference_policy(), storage);
    let app = router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind 0");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("127.0.0.1:{}", addr.port())
}

fn body_with_nonce(nonce: &str) -> Value {
    let mut v: Value = serde_json::from_str(APRP_GOLDEN).unwrap();
    v["nonce"] = Value::String(nonce.to_string());
    v
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fresh_daemon_metrics_endpoint_emits_zeroed_prometheus_text() {
    let addr = spawn_server().await;
    let url = format!("http://{addr}/v1/metrics");
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("GET /v1/metrics");
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .map(|h| h.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert!(
        ct.starts_with("text/plain"),
        "content-type must be text/plain; got {ct}"
    );
    let body = resp.text().await.expect("body");

    // Counters present + zero.
    assert!(body.contains("# TYPE sbo3l_requests_total counter"));
    assert!(body.contains("sbo3l_requests_total 0"));
    assert!(body.contains("sbo3l_decisions_total{outcome=\"allow\"} 0"));
    assert!(body.contains("sbo3l_decisions_total{outcome=\"deny\"} 0"));

    // Histogram skeleton present even with zero observations.
    assert!(body.contains("# TYPE sbo3l_request_duration_seconds histogram"));
    assert!(body.contains("sbo3l_request_duration_seconds_bucket{le=\"+Inf\"} 0"));
    assert!(body.contains("sbo3l_request_duration_seconds_count 0"));
    assert!(body.contains("sbo3l_request_duration_seconds_sum 0"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_request_increments_counters_and_dashboard_reflects_it() {
    let addr = spawn_server().await;
    let client = reqwest::Client::new();

    // POST one allowed request (golden APRP allows under reference policy).
    let resp = client
        .post(format!("http://{addr}/v1/payment-requests"))
        .json(&body_with_nonce("01HMETRCSP70000000000000A0"))
        .send()
        .await
        .expect("POST");
    assert_eq!(resp.status(), 200);

    // Prometheus endpoint reflects the request.
    let prom = client
        .get(format!("http://{addr}/v1/metrics"))
        .send()
        .await
        .expect("GET /v1/metrics")
        .text()
        .await
        .unwrap();
    assert!(
        prom.contains("sbo3l_requests_total 1"),
        "expected requests_total = 1; got:\n{prom}"
    );
    assert!(
        prom.contains("sbo3l_decisions_total{outcome=\"allow\"} 1"),
        "expected allow = 1; got:\n{prom}"
    );
    // The +Inf bucket equals the count.
    assert!(prom.contains("sbo3l_request_duration_seconds_bucket{le=\"+Inf\"} 1"));
    assert!(prom.contains("sbo3l_request_duration_seconds_count 1"));

    // JSON dashboard reads the same numbers (no longer placeholder zero).
    let json: Value = client
        .get(format!("http://{addr}/v1/admin/metrics"))
        .send()
        .await
        .expect("GET /v1/admin/metrics")
        .json()
        .await
        .unwrap();
    let bucket = &json["buckets"][0];
    assert_eq!(
        bucket["requests"].as_u64().unwrap(),
        1,
        "JSON dashboard requests must match Prometheus; full payload: {json}"
    );
    assert_eq!(bucket["allows"].as_u64().unwrap(), 1);
    assert_eq!(bucket["denies"].as_u64().unwrap(), 0);
    // Latency in JSON is reported in milliseconds; not strictly
    // assertable but must be non-negative.
    let p99 = bucket["latency_ms"]["p99"].as_f64().unwrap();
    assert!(p99 >= 0.0, "p99 must be non-negative: {p99}");
}
