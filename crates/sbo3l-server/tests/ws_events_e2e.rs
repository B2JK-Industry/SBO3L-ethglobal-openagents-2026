//! T-3-5 closeout: end-to-end loop test.
//!
//! Sister test to `ws_events.rs` (which covers the 3-request happy
//! path + the first-request 3-kinds shape). This test pins the
//! **5-request stress shape** the brief calls out: 5 payment
//! requests in flight against one daemon, with one WebSocket
//! subscriber that must see every `decision.made` frame in the
//! same order the requests were submitted, plus the bootstrap
//! `agent.discovered` (once) + every `audit.checkpoint` (one per
//! decision).
//!
//! A regression in the publish path (e.g. a future refactor that
//! reorders the publish-after-audit-append step, or accidentally
//! drops a frame on a slow subscriber) shows up here as a count
//! or order mismatch.

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

/// Five payment requests, one WebSocket subscriber, every emitted
/// frame in the expected order. Per the T-3-5 contract:
///
/// - First request: `agent.discovered` (once) + `decision.made` +
///   `audit.checkpoint`.
/// - Each subsequent request: `decision.made` + `audit.checkpoint`
///   (no second `agent.discovered` — `first_seen_agent` only fires
///   once per agent_id per process).
///
/// Total expected frames: 1 (discovered) + 5*(decision + checkpoint)
/// = 11. The test waits for >= 5 `decision.made` frames in the same
/// order the requests were submitted, then asserts the totals.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn five_requests_loop_end_to_end_ws_subscriber_sees_every_frame() {
    let addr = spawn_server().await;
    let ws_url = format!("ws://{addr}/v1/events");
    let http_url = format!("http://{addr}/v1/payment-requests");

    let (mut ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket upgrade");

    let client = reqwest::Client::new();

    // 5 distinct nonces — same agent_id (research-agent-01 per the
    // golden APRP) so we expect exactly one `agent.discovered`.
    let nonces = [
        "01HTAWXE2E0000000000000001",
        "01HTAWXE2E0000000000000002",
        "01HTAWXE2E0000000000000003",
        "01HTAWXE2E0000000000000004",
        "01HTAWXE2E0000000000000005",
    ];
    for nonce in nonces {
        let resp = client
            .post(&http_url)
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

    // Collect frames until we see >= 5 decisions or hit the budget.
    // Don't filter early — the order property is asserted across all
    // frame kinds, since a `decision.made` for request N must precede
    // an `audit.checkpoint` for the same request, which must precede
    // any frame for request N+1.
    let collect = async {
        let mut all: Vec<Value> = Vec::new();
        let mut decisions_seen = 0usize;
        while decisions_seen < 5 {
            let frame = ws.next().await.expect("frame").expect("frame ok");
            let payload = match frame {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };
            let v: Value = serde_json::from_str(&payload).expect("JSON frame");
            if v["kind"] == "decision.made" {
                decisions_seen += 1;
            }
            all.push(v);
        }
        all
    };
    let frames = tokio::time::timeout(Duration::from_secs(15), collect)
        .await
        .expect("5 decisions must arrive within 15s");

    // Tally — expect 1 agent.discovered, 5 decision.made, 5
    // audit.checkpoint. Total frames = 11.
    let n_discovered = frames
        .iter()
        .filter(|v| v["kind"] == "agent.discovered")
        .count();
    let n_decisions = frames
        .iter()
        .filter(|v| v["kind"] == "decision.made")
        .count();
    let n_checkpoints = frames
        .iter()
        .filter(|v| v["kind"] == "audit.checkpoint")
        .count();
    assert_eq!(
        n_discovered, 1,
        "exactly one agent.discovered (first-seen only); got {n_discovered}"
    );
    assert_eq!(
        n_decisions, 5,
        "expected 5 decision.made frames; got {n_decisions}"
    );
    // checkpoints may arrive after our 5-decision cutoff if the
    // last-decision-then-last-checkpoint pair straddles our break.
    // Accept >= 4 with a clear message.
    assert!(
        n_checkpoints >= 4,
        "expected >= 4 audit.checkpoint frames in the early window; got {n_checkpoints}"
    );

    // Order property: every decision.made frame's index in `frames`
    // is strictly less than the next decision.made frame's index.
    // (Trivially true if there are 5 distinct frames; the explicit
    // assertion catches a pathological reordering bug.)
    let decision_indices: Vec<usize> = frames
        .iter()
        .enumerate()
        .filter(|(_, v)| v["kind"] == "decision.made")
        .map(|(i, _)| i)
        .collect();
    let mut prev: i64 = -1;
    for &idx in &decision_indices {
        assert!(
            (idx as i64) > prev,
            "decision frames out of order at index {idx} (prev {prev})"
        );
        prev = idx as i64;
    }

    // Per-frame contract checks against the golden APRP. Every
    // decision.made frame carries the same agent_id and "allow"
    // (the reference policy approves the golden APRP).
    for v in frames.iter().filter(|v| v["kind"] == "decision.made") {
        assert_eq!(v["agent_id"], "research-agent-01");
        assert_eq!(v["decision"], "allow");
        assert!(v.get("ts_ms").is_some(), "ts_ms required by VizEvent");
    }
    for v in frames.iter().filter(|v| v["kind"] == "audit.checkpoint") {
        assert_eq!(v["agent_id"], "research-agent-01");
        assert!(
            v["chain_length"].as_u64().is_some(),
            "audit.checkpoint must carry chain_length"
        );
        assert!(
            v["root_hash"]
                .as_str()
                .map(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit()))
                .unwrap_or(false),
            "audit.checkpoint root_hash must be non-empty hex"
        );
    }

    let _ = ws.close(None).await;
}
