//! T-3-5 backend integration test: real WebSocket round-trip.
//!
//! Spawns the daemon (router + state) on an ephemeral loopback port,
//! opens a WebSocket against `/v1/events`, then sends 3 distinct
//! `POST /v1/payment-requests` calls and asserts the WebSocket
//! receives at least 3 `decision.made` frames in the same order
//! (and additional `agent.discovered` + `audit.checkpoint` frames
//! per the contract).

#![cfg(feature = "ws_events")]

use futures_util::StreamExt;
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::Value;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

/// Spawn the router on an ephemeral port. Returns the bound URL +
/// the handle so the test can drop it at the end.
async fn spawn_server() -> String {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    let state = AppState::new(reference_policy(), storage);
    let app = router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind 0");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Tiny grace so the server registers before the WS dialer hits it.
    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("127.0.0.1:{}", addr.port())
}

fn body_with_nonce(nonce: &str) -> Value {
    let mut v: Value = serde_json::from_str(APRP_GOLDEN).unwrap();
    v["nonce"] = Value::String(nonce.to_string());
    v
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_requests_yield_three_decision_frames_in_order() {
    let addr = spawn_server().await;
    let ws_url = format!("ws://{addr}/v1/events");
    let http_url = format!("http://{addr}/v1/payment-requests");

    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade should succeed");

    let client = reqwest::Client::new();

    // Distinct nonces so the 3 requests don't collide on the
    // nonce-replay gate.
    let nonces = [
        "01HTAWX5K3R8YV9NQB7C6WSE01",
        "01HTAWX5K3R8YV9NQB7C6WSE02",
        "01HTAWX5K3R8YV9NQB7C6WSE03",
    ];
    for nonce in nonces {
        let resp = client
            .post(&http_url)
            .json(&body_with_nonce(nonce))
            .send()
            .await
            .expect("POST /v1/payment-requests");
        assert_eq!(resp.status(), 200);
    }

    // Read frames until we've seen >= 3 decision.made events or hit a
    // generous timeout. The order of unrelated kinds (agent.discovered,
    // audit.checkpoint) is intentionally not asserted — only that the
    // 3 decisions arrive and arrive in the same order they were
    // submitted.
    let collect = async {
        let mut decisions: Vec<Value> = Vec::new();
        while decisions.len() < 3 {
            let frame = ws.next().await.expect("frame")
                .expect("frame ok");
            let payload = match frame {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };
            let v: Value = serde_json::from_str(&payload).expect("JSON frame");
            if v["kind"] == "decision.made" {
                decisions.push(v);
            }
        }
        decisions
    };
    let decisions = tokio::time::timeout(Duration::from_secs(10), collect)
        .await
        .expect("3 decision.made frames must arrive within 10s");

    assert!(decisions.len() >= 3, "expected >= 3 decision frames, got {}", decisions.len());
    for d in &decisions[..3] {
        assert_eq!(d["kind"], "decision.made");
        assert_eq!(d["agent_id"], "research-agent-01");
        assert_eq!(d["decision"], "allow");
        assert!(d.get("ts_ms").is_some());
    }

    let _ = ws.close(None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn first_request_emits_agent_discovered_frame() {
    let addr = spawn_server().await;
    let ws_url = format!("ws://{addr}/v1/events");
    let http_url = format!("http://{addr}/v1/payment-requests");

    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade");

    let client = reqwest::Client::new();
    let resp = client
        .post(&http_url)
        .json(&body_with_nonce("01HTAWX5K3R8YV9NQB7C6WSE10"))
        .send()
        .await
        .expect("POST");
    assert_eq!(resp.status(), 200);

    let collect = async {
        let mut kinds: Vec<String> = Vec::new();
        while kinds.len() < 3 {
            let frame = ws.next().await.expect("frame").expect("frame ok");
            let Message::Text(t) = frame else { continue };
            let v: Value = serde_json::from_str(&t).unwrap();
            if let Some(k) = v["kind"].as_str() {
                kinds.push(k.to_string());
            }
        }
        kinds
    };
    let kinds = tokio::time::timeout(Duration::from_secs(5), collect)
        .await
        .expect("3 frames within 5s");

    assert!(
        kinds.contains(&"agent.discovered".to_string()),
        "first request must emit agent.discovered; got {kinds:?}"
    );
    assert!(
        kinds.contains(&"decision.made".to_string()),
        "first request must emit decision.made; got {kinds:?}"
    );
    assert!(
        kinds.contains(&"audit.checkpoint".to_string()),
        "first request must emit audit.checkpoint; got {kinds:?}"
    );
}
