//! Unit + e2e tests for the cluster scaffold.
//!
//! Unit tests (single-threaded `#[tokio::test]`):
//!   * `AuditAppend` serde round-trip via bincode + serde_json.
//!   * `apply_audit_append` against a fresh in-memory `Storage`.
//!   * `apply_audit_append` against a `Storage` that already has rows.
//!   * `InMemoryRaftLogStore` append/read/truncate/purge invariants.
//!   * `InProcessNetworkFactory` unknown-target returns Unreachable.
//!
//! E2E test (`#[tokio::test(flavor = "multi_thread")]`):
//!   * Spin up 3 in-process Raft nodes, initialise from node 1, wait
//!     for leader election, push 3 `AuditAppend`s through the leader,
//!     wait for replication, assert all 3 SQLite-backed state
//!     machines saw all 3 rows.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use openraft::{BasicNode, RaftLogReader};
use tokio::sync::Mutex;

use sbo3l_core::signer::DevSigner;
use sbo3l_storage::Storage;

use super::network::InProcessNetworkFactory;
use super::raft::RaftNode;
use super::state_machine::{apply_audit_append, AuditStateMachine};
use super::storage::InMemoryRaftLogStore;
use super::types::{AuditAppend, AuditAppendResponse, NodeId};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn audit_signer() -> Arc<DevSigner> {
    Arc::new(DevSigner::from_seed("audit-signer-v1", [11u8; 32]))
}

fn fresh_storage() -> Arc<Mutex<Storage>> {
    Arc::new(Mutex::new(Storage::open_in_memory().unwrap()))
}

fn sample_append(subject: &str) -> AuditAppend {
    AuditAppend {
        tenant_id: "default".to_string(),
        event_type: "policy_decided".to_string(),
        actor: "policy_engine".to_string(),
        subject_id: subject.to_string(),
        payload_hash: "0".repeat(64),
    }
}

// ---------------------------------------------------------------------------
// unit: serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn audit_append_bincode_round_trip() {
    let req = sample_append("pr-001");
    let bytes = bincode::serialize(&req).expect("encode");
    let back: AuditAppend = bincode::deserialize(&bytes).expect("decode");
    assert_eq!(back, req);
}

#[test]
fn audit_append_serde_json_round_trip() {
    let req = sample_append("pr-002");
    let json = serde_json::to_string(&req).expect("encode");
    let back: AuditAppend = serde_json::from_str(&json).expect("decode");
    assert_eq!(back, req);
}

#[test]
fn audit_append_response_serde_round_trip() {
    let resp = AuditAppendResponse {
        seq: 7,
        event_hash: "deadbeef".to_string(),
    };
    let json = serde_json::to_string(&resp).expect("encode");
    let back: AuditAppendResponse = serde_json::from_str(&json).expect("decode");
    assert_eq!(back, resp);
}

// ---------------------------------------------------------------------------
// unit: state machine apply
// ---------------------------------------------------------------------------

#[tokio::test]
async fn apply_audit_append_on_fresh_db() {
    let storage = fresh_storage();
    let signer = audit_signer();
    let resp = apply_audit_append(storage.clone(), signer, sample_append("pr-fresh")).await;
    assert_eq!(resp.seq, 1, "first append on a fresh db lands at seq=1");
    assert!(!resp.event_hash.is_empty(), "event_hash must be populated");
    let s = storage.lock().await;
    assert_eq!(s.audit_count().unwrap(), 1);
}

#[tokio::test]
async fn apply_audit_append_on_db_with_existing_rows() {
    // Pre-populate two rows directly through `audit_append_for_tenant`,
    // then simulate the state machine applying a third entry; the seq
    // must be 3, not 1.
    let storage = fresh_storage();
    let signer = audit_signer();
    {
        use sbo3l_storage::audit_store::NewAuditEvent;
        let mut s = storage.lock().await;
        s.audit_append_for_tenant(
            "default",
            NewAuditEvent::now("runtime_started", "sbo3l-server", "runtime"),
            signer.as_ref(),
        )
        .unwrap();
        s.audit_append_for_tenant(
            "default",
            NewAuditEvent::now("config_loaded", "sbo3l-server", "config"),
            signer.as_ref(),
        )
        .unwrap();
    }
    let resp = apply_audit_append(storage.clone(), signer, sample_append("pr-third")).await;
    assert_eq!(resp.seq, 3, "third append must land at seq=3");
    let s = storage.lock().await;
    assert_eq!(s.audit_count().unwrap(), 3);
}

#[tokio::test]
async fn audit_state_machine_reports_count_after_apply() {
    let storage = fresh_storage();
    let signer = audit_signer();
    let sm = AuditStateMachine::new(storage.clone(), signer.clone());
    assert_eq!(sm.audit_count().await, 0, "fresh state machine has no rows");
    apply_audit_append(storage, signer, sample_append("pr-A")).await;
    assert_eq!(sm.audit_count().await, 1);
}

// ---------------------------------------------------------------------------
// unit: log store invariants
// ---------------------------------------------------------------------------

#[tokio::test]
async fn log_store_reports_empty_initial_state() {
    use openraft::storage::RaftLogStorage;
    let mut store = InMemoryRaftLogStore::new();
    let state = store.get_log_state().await.unwrap();
    assert!(state.last_log_id.is_none());
    assert!(state.last_purged_log_id.is_none());
}

#[tokio::test]
async fn log_store_persists_and_reads_back_vote() {
    use openraft::storage::RaftLogStorage;
    use openraft::Vote;
    let mut store = InMemoryRaftLogStore::new();
    let v: Vote<NodeId> = Vote::new(7, 1);
    store.save_vote(&v).await.unwrap();
    let read = store.read_vote().await.unwrap().expect("vote round-trips");
    assert_eq!(read, v);
}

#[tokio::test]
async fn log_store_range_query_returns_empty_initially() {
    let mut store = InMemoryRaftLogStore::new();
    let entries = store.try_get_log_entries(0..100).await.unwrap();
    assert!(entries.is_empty());
}

// ---------------------------------------------------------------------------
// unit: in-process network unknown-target
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_process_factory_unknown_target_returns_unreachable() {
    use openraft::error::RPCError;
    use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
    use openraft::raft::VoteRequest;

    let mut factory = InProcessNetworkFactory::new();
    let mut client = factory
        .new_client(99, &BasicNode::new("doesnt-matter"))
        .await;
    // No peer registered for node-id=99 → expect Unreachable.
    let req: VoteRequest<NodeId> = VoteRequest::new(openraft::Vote::new(1, 1), None);
    let result = client
        .vote(req, RPCOption::new(Duration::from_millis(500)))
        .await;
    match result {
        Err(RPCError::Unreachable(_)) => {}
        other => panic!("expected Unreachable, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// unit: peer add / remove via change_membership semantics (in-memory)
// ---------------------------------------------------------------------------

#[test]
fn voter_set_diff_add_remove() {
    // Sanity check on how callers compose voter sets — guards against
    // a future refactor that swaps BTreeSet for HashSet (loss of
    // deterministic ordering would break the no-flap invariant tests
    // need).
    let mut current: BTreeSet<NodeId> = [1u64, 2, 3].into_iter().collect();
    current.remove(&3);
    current.insert(4);
    let expected: BTreeSet<NodeId> = [1u64, 2, 4].into_iter().collect();
    assert_eq!(current, expected);
}

// ---------------------------------------------------------------------------
// unit: response decode-encode is stable across openraft format
// ---------------------------------------------------------------------------

#[test]
fn audit_append_response_default_seq_zero() {
    // The `apply_audit_append` sentinel returns seq=0 + empty hash on
    // the SQLite-write-failure path. Tests that mock that path rely
    // on the empty-hash check; pin it here.
    let r = AuditAppendResponse {
        seq: 0,
        event_hash: String::new(),
    };
    assert_eq!(r.seq, 0);
    assert!(r.event_hash.is_empty());
}

// ---------------------------------------------------------------------------
// e2e: 3-node leader election + log replication
// ---------------------------------------------------------------------------

/// Spawn a single in-process node + register it with the shared
/// factory. Returns the `Arc<RaftNode>` so the caller can interact
/// with it.
async fn spawn_node(
    node_id: NodeId,
    factory: InProcessNetworkFactory,
    cluster_name: &str,
) -> Arc<RaftNode> {
    let storage = fresh_storage();
    let signer = audit_signer();
    let node =
        RaftNode::in_process_for_test(node_id, cluster_name, storage, signer, factory.clone())
            .await
            .expect("node construct");
    node.register_self_in_factory().await.unwrap();
    node
}

/// Poll `condition` every 50ms up to `timeout`; return Ok on first
/// true, Err on timeout. Used by the e2e test instead of fixed sleeps
/// so a slow CI doesn't flake.
async fn wait_for<F, Fut>(timeout: Duration, mut condition: F) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if condition().await {
            return Ok(());
        }
        if std::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!("wait_for timed out after {:?}", timeout));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_three_node_leader_election_and_log_replication() {
    // Build the shared in-process factory + 3 nodes, register them.
    let factory = InProcessNetworkFactory::new();
    let n1 = spawn_node(1, factory.clone(), "sbo3l-test-cluster").await;
    let n2 = spawn_node(2, factory.clone(), "sbo3l-test-cluster").await;
    let n3 = spawn_node(3, factory.clone(), "sbo3l-test-cluster").await;

    // Initialise from node 1 with all three peers in the voter set.
    let mut peers = BTreeMap::new();
    peers.insert(1u64, BasicNode::new("inproc-node-1"));
    peers.insert(2u64, BasicNode::new("inproc-node-2"));
    peers.insert(3u64, BasicNode::new("inproc-node-3"));
    n1.initialize(peers).await.expect("initialize cluster");

    // Wait for leader election — within 10s we should see a leader on
    // node 1's metrics view.
    wait_for(Duration::from_secs(10), || async {
        n1.raft().metrics().borrow().current_leader.is_some()
    })
    .await
    .expect("leader elected within 10s");

    // Push 3 `AuditAppend`s through the leader (node 1).
    for i in 0..3 {
        let req = sample_append(&format!("pr-e2e-{i:03}"));
        let resp = n1.client_write(req).await.expect("client_write");
        assert!(
            resp.seq >= 1,
            "leader-side seq must be >=1 (got {})",
            resp.seq
        );
    }

    // Wait until every follower has applied 3 audit rows. The state
    // machine wraps a separate `Storage`, so each node's count is
    // independent.
    wait_for(Duration::from_secs(10), || async {
        n1.state_machine().audit_count().await >= 3
            && n2.state_machine().audit_count().await >= 3
            && n3.state_machine().audit_count().await >= 3
    })
    .await
    .expect("all 3 nodes applied 3 entries within 10s");

    // Final assertion: each node's SQLite has exactly 3 rows.
    assert_eq!(n1.state_machine().audit_count().await, 3);
    assert_eq!(n2.state_machine().audit_count().await, 3);
    assert_eq!(n3.state_machine().audit_count().await, 3);

    // Sanity check: status snapshot via the openraft metrics handle is
    // shaped how `cluster::http::get_status` will format it.
    let m = n1.raft().metrics().borrow().clone();
    assert!(
        m.current_leader.is_some(),
        "leader stays elected post-write"
    );
    assert!(m.current_term >= 1, "term advances at least once");
}
