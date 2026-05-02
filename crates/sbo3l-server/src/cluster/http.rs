//! axum HTTP handlers for the `/v1/admin/cluster/*` surface.
//!
//! Two flavours of endpoint live here:
//!
//! * Operator-facing — `GET /v1/admin/cluster/status`, `POST
//!   /v1/admin/cluster/join`, `POST /v1/admin/cluster/leave`. Used by
//!   humans / orchestrators to inspect the cluster and add/remove
//!   members. Currently unauthenticated; a deployment behind the
//!   existing F-1 auth middleware would gate them via
//!   `AuthConfig::admin_required`.
//!
//! * Inter-node openraft RPCs — `POST /v1/admin/cluster/raft/append-entries`,
//!   `/raft/install-snapshot`, `/raft/vote`. The `HttpNetworkFactory`
//!   in `cluster::network` POSTs to these to drive replication. JSON
//!   bodies, no streaming, no auth (out-of-scope for the scaffold).
//!
//! All handlers take an `Arc<RaftNode>` from the axum `State<…>`. The
//! cluster mode is opt-in: the binary mounts these routes only when
//! `--features cluster` is enabled AND a `RaftNode` is constructed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest, VoteRequest};
use openraft::BasicNode;
use serde::{Deserialize, Serialize};

use super::raft::RaftNode;
use super::types::{ClusterTypeConfig, NodeId};

/// Mount the `/v1/admin/cluster/*` routes onto a fresh router scoped to
/// the cluster `State`. The caller (`lib.rs::router`) merges this into
/// the main app router when `--features cluster` is on.
pub fn cluster_router(node: Arc<RaftNode>) -> Router {
    Router::new()
        .route("/v1/admin/cluster/status", get(get_status))
        .route("/v1/admin/cluster/join", post(post_join))
        .route("/v1/admin/cluster/leave", post(post_leave))
        .route(
            "/v1/admin/cluster/raft/append-entries",
            post(raft_append_entries),
        )
        .route(
            "/v1/admin/cluster/raft/install-snapshot",
            post(raft_install_snapshot),
        )
        .route("/v1/admin/cluster/raft/vote", post(raft_vote))
        .with_state(node)
}

// ---------------------------------------------------------------------------
// Operator-facing
// ---------------------------------------------------------------------------

/// Public JSON wire shape for `GET /v1/admin/cluster/status`. Stable
/// enough for an operator dashboard to depend on; cluster mode is
/// experimental so the field set may grow.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClusterStatusResponse {
    pub node_id: NodeId,
    pub leader_id: Option<NodeId>,
    pub current_term: u64,
    pub last_log_index: Option<u64>,
    pub last_applied: Option<u64>,
    pub voters: Vec<NodeId>,
    pub learners: Vec<NodeId>,
}

async fn get_status(State(node): State<Arc<RaftNode>>) -> Response {
    let m = node.raft().metrics().borrow().clone();
    let last_log_index = m.last_log_index;
    let last_applied = m.last_applied.as_ref().map(|lid| lid.index);
    let voters: Vec<NodeId> = m.membership_config.membership().voter_ids().collect();
    let learners: Vec<NodeId> = m.membership_config.membership().learner_ids().collect();
    let resp = ClusterStatusResponse {
        node_id: node.node_id(),
        leader_id: m.current_leader,
        current_term: m.current_term,
        last_log_index,
        last_applied,
        voters,
        learners,
    };
    ok_json(resp)
}

#[derive(Deserialize, Debug)]
pub struct JoinRequest {
    pub node_id: NodeId,
    pub addr: String,
}

async fn post_join(State(node): State<Arc<RaftNode>>, Json(req): Json<JoinRequest>) -> Response {
    match node
        .add_learner(req.node_id, BasicNode::new(req.addr.clone()))
        .await
    {
        Ok(_) => ok_json(serde_json::json!({"joined": req.node_id})),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Deserialize, Debug)]
pub struct LeaveRequest {
    /// The new voter set after the leave. We accept the full set
    /// rather than "remove node X" because change_membership is
    /// non-idempotent on the diff path; passing the full set lets the
    /// caller declare intent unambiguously.
    pub voters: Vec<NodeId>,
}

async fn post_leave(State(node): State<Arc<RaftNode>>, Json(req): Json<LeaveRequest>) -> Response {
    let voters: BTreeSet<NodeId> = req.voters.into_iter().collect();
    match node.change_membership(voters).await {
        Ok(_) => ok_json(serde_json::json!({"changed": true})),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Inter-node openraft RPCs
// ---------------------------------------------------------------------------

async fn raft_append_entries(
    State(node): State<Arc<RaftNode>>,
    Json(req): Json<AppendEntriesRequest<ClusterTypeConfig>>,
) -> Response {
    match node.raft().append_entries(req).await {
        Ok(resp) => ok_json(resp),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn raft_install_snapshot(
    State(node): State<Arc<RaftNode>>,
    Json(req): Json<InstallSnapshotRequest<ClusterTypeConfig>>,
) -> Response {
    match node.raft().install_snapshot(req).await {
        Ok(resp) => ok_json(resp),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn raft_vote(
    State(node): State<Arc<RaftNode>>,
    Json(req): Json<VoteRequest<NodeId>>,
) -> Response {
    match node.raft().vote(req).await {
        Ok(resp) => ok_json(resp),
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn ok_json<T: Serialize>(value: T) -> Response {
    (StatusCode::OK, Json(value)).into_response()
}

fn err_json(status: StatusCode, msg: String) -> Response {
    (status, Json(serde_json::json!({"error": msg}))).into_response()
}

/// Initialise the cluster bootstrap. Used by ops + tests as a helper
/// rather than an HTTP route — the bootstrap leader calls this from
/// the startup sequence in `main.rs` (cluster mode) once it has every
/// peer's `BasicNode` resolved.
pub async fn cluster_init(
    node: &Arc<RaftNode>,
    peers: BTreeMap<NodeId, BasicNode>,
) -> anyhow::Result<()> {
    node.initialize(peers).await
}
