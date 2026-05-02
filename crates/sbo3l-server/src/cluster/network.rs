//! `RaftNetwork` + `RaftNetworkFactory` implementations.
//!
//! Two flavours, both gated behind `--features cluster`:
//!
//! * [`InProcessNetworkFactory`] — used by the e2e test in
//!   `cluster::tests::e2e`. Holds an `Arc<RaftNode>` for every peer in
//!   a single process and dispatches RPCs directly to the in-memory
//!   `Raft` instance. No serialisation, no transport — purely a way to
//!   test leader election + log replication without a real network.
//!
//! * [`HttpNetworkFactory`] — used by `docker-compose-cluster.yml`. Each
//!   node runs the axum router with the cluster routes mounted, and the
//!   network sends openraft RPCs as JSON POSTs to peers' `/v1/admin/cluster/raft/*`
//!   endpoints. Keep-it-simple: one `reqwest::Client` per node, JSON
//!   bodies, no streaming snapshots (the scaffold doesn't compact).

use std::collections::HashMap;
use std::sync::Arc;

use openraft::error::{InstallSnapshotError, RPCError, RaftError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::BasicNode;
use tokio::sync::RwLock;

use super::raft::RaftNode;
use super::types::{ClusterTypeConfig, NodeId};

// =============================================================================
// In-process network (test only)
// =============================================================================

/// In-process registry of `RaftNode` handles, shared between every node
/// in a test cluster. The factory looks up the target by `NodeId` and
/// dispatches the RPC straight against `Raft::append_entries` /
/// `Raft::vote` / `Raft::install_snapshot` on the peer.
#[derive(Clone, Default)]
pub struct InProcessNetworkFactory {
    nodes: Arc<RwLock<HashMap<NodeId, Arc<RaftNode>>>>,
}

impl InProcessNetworkFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a peer's `RaftNode` handle so subsequent
    /// `new_client(target)` calls can dispatch to it.
    pub async fn register(&self, node_id: NodeId, node: Arc<RaftNode>) {
        self.nodes.write().await.insert(node_id, node);
    }
}

impl RaftNetworkFactory<ClusterTypeConfig> for InProcessNetworkFactory {
    type Network = InProcessNetwork;

    async fn new_client(&mut self, target: NodeId, _node: &BasicNode) -> Self::Network {
        InProcessNetwork {
            target,
            registry: self.nodes.clone(),
        }
    }
}

/// One per (factory, target) pair. The factory registry is `Arc`-shared
/// so the network handle is itself cheap to clone.
pub struct InProcessNetwork {
    target: NodeId,
    registry: Arc<RwLock<HashMap<NodeId, Arc<RaftNode>>>>,
}

impl RaftNetwork<ClusterTypeConfig> for InProcessNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<ClusterTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let peer = self
            .registry
            .read()
            .await
            .get(&self.target)
            .cloned()
            .ok_or_else(|| {
                RPCError::Unreachable(Unreachable::new(&unreachable_io(format!(
                    "no in-process peer registered for node-id={}",
                    self.target
                ))))
            })?;
        peer.raft()
            .append_entries(rpc)
            .await
            .map_err(|e| RPCError::RemoteError(openraft::error::RemoteError::new(self.target, e)))
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<ClusterTypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        let peer = self
            .registry
            .read()
            .await
            .get(&self.target)
            .cloned()
            .ok_or_else(|| {
                RPCError::Unreachable(Unreachable::new(&unreachable_io(format!(
                    "no in-process peer registered for node-id={}",
                    self.target
                ))))
            })?;
        peer.raft()
            .install_snapshot(rpc)
            .await
            .map_err(|e| RPCError::RemoteError(openraft::error::RemoteError::new(self.target, e)))
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let peer = self
            .registry
            .read()
            .await
            .get(&self.target)
            .cloned()
            .ok_or_else(|| {
                RPCError::Unreachable(Unreachable::new(&unreachable_io(format!(
                    "no in-process peer registered for node-id={}",
                    self.target
                ))))
            })?;
        peer.raft()
            .vote(rpc)
            .await
            .map_err(|e| RPCError::RemoteError(openraft::error::RemoteError::new(self.target, e)))
    }
}

// =============================================================================
// HTTP network (used by docker-compose-cluster.yml — not yet exercised in tests)
// =============================================================================

/// Factory that builds [`HttpNetwork`] clients addressed by
/// `BasicNode.addr`. Each `new_client` allocates a fresh `reqwest::Client`
/// — fine for the scaffold; a production deployment would pool them.
#[derive(Clone, Default)]
pub struct HttpNetworkFactory;

impl HttpNetworkFactory {
    pub fn new() -> Self {
        Self
    }
}

impl RaftNetworkFactory<ClusterTypeConfig> for HttpNetworkFactory {
    type Network = HttpNetwork;

    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        HttpNetwork {
            target,
            target_addr: node.addr.clone(),
            client: reqwest::Client::new(),
        }
    }
}

/// HTTP-backed `RaftNetwork`. The wire encoding is JSON; no
/// authentication on the inter-node RPCs in the scaffold (a production
/// deployment would terminate these over mTLS or a shared HMAC).
pub struct HttpNetwork {
    /// Held for tracing / future structured-log work; the `target_addr`
    /// is what's actually used to build the URL. See
    /// `_silence_unused_target` for the diagnostic hook.
    #[allow(dead_code)]
    target: NodeId,
    target_addr: String,
    client: reqwest::Client,
}

impl HttpNetwork {
    fn url(&self, path: &str) -> String {
        // `target_addr` is "host:port" per BasicNode usage; we prefix
        // http:// because mTLS isn't wired (see docs/cluster-mode.md).
        format!("http://{}{}", self.target_addr, path)
    }

    async fn post<Req, Resp>(&self, path: &str, body: &Req) -> Result<Resp, Unreachable>
    where
        Req: serde::Serialize,
        Resp: for<'de> serde::Deserialize<'de>,
    {
        let url = self.url(path);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| Unreachable::new(&unreachable_io(format!("POST {url}: {e}"))))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Unreachable::new(&unreachable_io(format!(
                "POST {url}: HTTP {}",
                status.as_u16()
            ))));
        }
        resp.json()
            .await
            .map_err(|e| Unreachable::new(&unreachable_io(format!("decode {url}: {e}"))))
    }
}

impl RaftNetwork<ClusterTypeConfig> for HttpNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<ClusterTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.post("/v1/admin/cluster/raft/append-entries", &rpc)
            .await
            .map_err(RPCError::Unreachable)
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<ClusterTypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        self.post("/v1/admin/cluster/raft/install-snapshot", &rpc)
            .await
            .map_err(RPCError::Unreachable)
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.post("/v1/admin/cluster/raft/vote", &rpc)
            .await
            .map_err(RPCError::Unreachable)
    }
}

/// Build a `std::io::Error` from a message string. openraft's
/// `Unreachable::new` requires `&E: Error + 'static`, but `String`
/// doesn't impl `Error` so we wrap it in `io::Error::other`.
fn unreachable_io(msg: String) -> std::io::Error {
    std::io::Error::other(msg)
}

#[allow(dead_code)]
fn _silence_unused_target(target: NodeId) -> NodeId {
    // The `target` field on HttpNetwork is referenced for diagnostic
    // purposes only — keep this stub so a future commit that wires up
    // structured tracing has a hook.
    target
}
