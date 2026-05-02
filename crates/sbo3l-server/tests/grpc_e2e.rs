//! R14 P1 — end-to-end integration tests for the gRPC service.
//!
//! These tests spin up a real `tonic::transport::Server` on an
//! ephemeral port, dial it back via the generated `Sbo3lClient`, and
//! exercise the wire path through HTTP/2. They complement the unit
//! tests in `src/grpc.rs::tests` (which call the service trait
//! methods directly) by covering the actual transport layer —
//! prost serialisation, tonic interceptors, status code propagation.

#![cfg(feature = "grpc")]

use sbo3l_server::grpc::{pb, GrpcService, Sbo3lClient};
use sbo3l_server::{reference_policy, AppState};
use sbo3l_storage::Storage;
use serde_json::Value;
use std::time::Duration;
use tokio::net::TcpListener;
use tonic::transport::{Endpoint, Server};

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

/// Spawn an in-process gRPC server on a random ephemeral port and
/// return the bound address. The server runs on a background task and
/// is dropped when the test exits — fine for short-lived integration
/// tests, no graceful-shutdown plumbing needed.
async fn spawn_grpc_server() -> std::net::SocketAddr {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    let state = AppState::new(reference_policy(), storage);
    let svc = GrpcService::new(state.0).into_server();

    // Bind 127.0.0.1:0 to discover an unused port, then hand the
    // resulting `std::net::TcpListener` to tonic via
    // `serve_with_incoming`. We can't use the address directly with
    // `serve()` because that races with port allocation in CI.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    tokio::spawn(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_incoming(incoming)
            .await
            .expect("tonic server fail");
    });
    // Give tonic a beat to mark itself ready before clients dial.
    // 50ms is enough on local hardware; tests that flake on slower
    // CI can grow this.
    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

async fn client(addr: std::net::SocketAddr) -> Sbo3lClient<tonic::transport::Channel> {
    let endpoint = Endpoint::try_from(format!("http://{addr}"))
        .expect("endpoint")
        .connect_timeout(Duration::from_secs(2));
    let channel = endpoint.connect().await.expect("connect");
    Sbo3lClient::new(channel)
}

fn aprp_with_nonce(nonce: &str) -> String {
    let mut v: Value = serde_json::from_str(APRP_GOLDEN).unwrap();
    v["nonce"] = Value::String(nonce.to_string());
    serde_json::to_string(&v).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_decide_round_trips_over_http2() {
    let addr = spawn_grpc_server().await;
    let mut c = client(addr).await;
    let resp = c
        .decide(pb::DecideRequest {
            aprp_json: aprp_with_nonce("01HMG0CTPCH4VVRG0TVCHE2EA0"),
        })
        .await
        .expect("decide ok")
        .into_inner();
    assert_eq!(resp.status, pb::PaymentStatus::AutoApproved as i32);
    assert_eq!(resp.decision, pb::Decision::Allow as i32);
    assert!(!resp.audit_event_id.is_empty());
    // Verify the receipt JSON parses + carries the same agent_id we
    // sent. The signature byte-stream survived prost encoding, HTTP/2
    // framing, and prost decoding.
    let receipt: sbo3l_core::receipt::PolicyReceipt =
        serde_json::from_str(&resp.receipt_json).expect("receipt json");
    assert_eq!(receipt.agent_id, "research-agent-01");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_health_returns_version_and_chain_state() {
    let addr = spawn_grpc_server().await;
    let mut c = client(addr).await;

    // Fresh daemon: chain is empty.
    let h0 = c
        .health(pb::HealthRequest {})
        .await
        .expect("health ok")
        .into_inner();
    assert_eq!(h0.audit_chain_length, 0);
    assert_eq!(h0.version, env!("CARGO_PKG_VERSION"));

    // Submit one decision.
    c.decide(pb::DecideRequest {
        aprp_json: aprp_with_nonce("01HMG0CTPCH4VVRG0TVCHE2EB0"),
    })
    .await
    .expect("decide ok");

    // Chain advanced.
    let h1 = c
        .health(pb::HealthRequest {})
        .await
        .expect("health ok")
        .into_inner();
    assert_eq!(h1.audit_chain_length, 1);
    assert!(!h1.audit_chain_head.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_audit_chain_stream_yields_each_decision() {
    use tokio_stream::StreamExt;

    let addr = spawn_grpc_server().await;
    let mut c = client(addr).await;

    for nonce in ["01HMG0CTPCH4VVRG0TVCHE2EC0", "01HMG0CTPCH4VVRG0TVCHE2ED0"] {
        c.decide(pb::DecideRequest {
            aprp_json: aprp_with_nonce(nonce),
        })
        .await
        .expect("decide ok");
    }

    let mut stream = c
        .audit_chain_stream(pb::AuditChainRequest {
            since_seq: 0,
            limit: 0,
        })
        .await
        .expect("stream open ok")
        .into_inner();

    let mut count = 0u64;
    while let Some(msg) = stream.next().await {
        let ev = msg.expect("stream event ok");
        count += 1;
        assert_eq!(ev.seq, count, "seqs are 1-indexed and contiguous");
        assert!(!ev.event_hash.is_empty());
    }
    assert_eq!(count, 2, "stream emitted one event per decision");
}
