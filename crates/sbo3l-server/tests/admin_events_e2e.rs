//! R13 P6 — end-to-end test for `GET /v1/admin/events`.
//!
//! Three flows:
//!
//! 1. **Live forward.** WS connect → 3 POST requests → 3 `decision`
//!    frames arrive with monotonic ids.
//! 2. **Cursor replay from ring.** 3 POSTs FIRST, then WS connect
//!    with `?since_id=0` → all 3 prior events replay before any
//!    live frame.
//! 3. **Filter excludes non-matching events.** WS connect with
//!    `?tenant=other` → live POST doesn't deliver any frame
//!    (filter applied before send). Use a short timeout to detect
//!    "no frames came through" without hanging the test.

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

async fn post(client: &reqwest::Client, http_url: &str, nonce: &str) {
    let resp = client
        .post(http_url)
        .json(&body_with_nonce(nonce))
        .send()
        .await
        .expect("POST /v1/payment-requests");
    assert_eq!(
        resp.status(),
        200,
        "request with nonce {nonce} must succeed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_forward_three_decisions_in_monotonic_order() {
    let addr = spawn_server().await;
    let ws_url = format!("ws://{addr}/v1/admin/events");
    let http_url = format!("http://{addr}/v1/payment-requests");

    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade");

    let client = reqwest::Client::new();
    let nonces = [
        "01HADM0000000000000000000A",
        "01HADM0000000000000000000B",
        "01HADM0000000000000000000C",
    ];
    for nonce in nonces {
        post(&client, &http_url, nonce).await;
    }

    let collect = async {
        let mut events: Vec<Value> = Vec::new();
        while events.len() < 3 {
            let frame = ws.next().await.expect("frame").expect("frame ok");
            let payload = match frame {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };
            let v: Value = serde_json::from_str(&payload).expect("JSON frame");
            if v["kind"] == "decision" {
                events.push(v);
            }
        }
        events
    };
    let events = tokio::time::timeout(Duration::from_secs(10), collect)
        .await
        .expect("3 decision events must arrive within 10s");

    assert_eq!(events.len(), 3);
    let ids: Vec<u64> = events
        .iter()
        .map(|e| e["id"].as_u64().expect("id is u64"))
        .collect();
    assert_eq!(ids[0] + 1, ids[1], "ids must be contiguous");
    assert_eq!(ids[1] + 1, ids[2], "ids must be contiguous");
    for e in &events {
        assert_eq!(e["tenant_id"], "default");
        assert_eq!(e["decision"], "allow");
        assert_eq!(e["severity"], "info");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cursor_replay_returns_buffered_events_before_live() {
    let addr = spawn_server().await;
    let http_url = format!("http://{addr}/v1/payment-requests");
    let client = reqwest::Client::new();

    // Burn 3 requests BEFORE any subscriber connects. They land in
    // the bus ring buffer.
    for nonce in [
        "01HADM00CRSR0000000000000A",
        "01HADM00CRSR0000000000000B",
        "01HADM00CRSR0000000000000C",
    ] {
        post(&client, &http_url, nonce).await;
    }

    // Now subscribe with since_id=0 — should replay all 3 from ring.
    let ws_url = format!("ws://{addr}/v1/admin/events?since_id=0");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade");

    let collect = async {
        let mut events: Vec<Value> = Vec::new();
        while events.len() < 3 {
            let frame = ws.next().await.expect("frame").expect("frame ok");
            let payload = match frame {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };
            let v: Value = serde_json::from_str(&payload).expect("JSON frame");
            if v["kind"] == "decision" {
                events.push(v);
            }
        }
        events
    };
    let events = tokio::time::timeout(Duration::from_secs(10), collect)
        .await
        .expect("3 replay events must arrive within 10s");

    assert_eq!(events.len(), 3);
    let ids: Vec<u64> = events
        .iter()
        .map(|e| e["id"].as_u64().expect("id is u64"))
        .collect();
    // Replay should be in monotonic order, oldest first.
    assert!(ids[0] < ids[1] && ids[1] < ids[2]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn filter_tenant_excludes_non_matching_events() {
    let addr = spawn_server().await;
    let http_url = format!("http://{addr}/v1/payment-requests");
    // Pipeline always tags events with tenant_id = "default" (single
    // tenant for now). Subscribe with `?tenant=other-tenant` — no
    // event should match the filter, so the WS receives no frames in
    // a 1.5s window after we POST 3 requests.
    let ws_url = format!("ws://{addr}/v1/admin/events?tenant=other-tenant");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade");

    let client = reqwest::Client::new();
    for nonce in [
        "01HADM00FTR000000000000001",
        "01HADM00FTR000000000000002",
        "01HADM00FTR000000000000003",
    ] {
        post(&client, &http_url, nonce).await;
    }

    // Should NOT see any frame within 1.5s — the filter dropped them
    // server-side. If a frame arrives, that's a regression where the
    // filter isn't being applied to live broadcast.
    let frame = tokio::time::timeout(Duration::from_millis(1500), ws.next()).await;
    match frame {
        Err(_elapsed) => {
            // Timeout — no frame arrived. Expected.
        }
        Ok(None) => {
            // Stream closed cleanly without sending — also acceptable.
        }
        Ok(Some(Ok(Message::Text(t)))) => {
            panic!("filter leaked a frame past the server-side filter: {t}");
        }
        Ok(Some(Ok(_other))) => {
            // Pings/closes are fine; only Text frames signal a leak.
        }
        Ok(Some(Err(e))) => {
            panic!("ws error during filter test: {e}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn filter_severity_info_matches_allow_decisions() {
    let addr = spawn_server().await;
    // The golden APRP allows → severity=info. Subscribe with
    // `?severity=info` and verify the live frame still arrives —
    // proves the filter applies a positive match correctly, not
    // just a negative one.
    let ws_url = format!("ws://{addr}/v1/admin/events?severity=info");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade");

    let client = reqwest::Client::new();
    let http_url = format!("http://{addr}/v1/payment-requests");
    post(&client, &http_url, "01HADM00SEV0NF0000000000F1").await;

    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("frame within 5s")
        .expect("frame")
        .expect("frame ok");
    if let Message::Text(t) = frame {
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["kind"], "decision");
        assert_eq!(v["severity"], "info");
    } else {
        panic!("expected Text frame, got {frame:?}");
    }
}
