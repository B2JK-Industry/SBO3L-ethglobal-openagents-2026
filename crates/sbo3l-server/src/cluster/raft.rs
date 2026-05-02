//! `RaftNode` — the assembled `openraft::Raft` instance + its handles
//! to log store, state machine, and network factory.
//!
//! Each running cluster member holds exactly one `RaftNode`. Test code
//! and the HTTP handlers in `cluster::http` call into this through
//! `node.raft()` (the openraft-typed handle).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use openraft::{BasicNode, Config, Raft};
use tokio::sync::Mutex;

use sbo3l_core::signer::DevSigner;
use sbo3l_storage::Storage;

use super::network::{HttpNetworkFactory, InProcessNetworkFactory};
use super::state_machine::AuditStateMachine;
use super::storage::InMemoryRaftLogStore;
use super::types::{ClusterTypeConfig, NodeId};

/// Either an in-process network factory (test cluster) or the HTTP one
/// (real cluster). Held as an enum so a single `RaftNode` shape can
/// carry either. We don't expose the variant publicly — the e2e test
/// constructs through `RaftNode::in_process_for_test`, the binary
/// constructs through `RaftNode::with_http_network`.
///
/// `dead_code` suppression: Rust sees the variants' `0` field as unread
/// because we only match in `register_self_in_factory`, which is
/// `cfg(test)`. Without the allow, a `--release` build of cluster mode
/// would emit a warning. The factory is genuinely held for the
/// in-process test path; removing it breaks the ability to register
/// late-joining peers.
#[allow(dead_code)]
enum NetworkBackend {
    InProcess(InProcessNetworkFactory),
    Http(HttpNetworkFactory),
}

/// A single Raft cluster member.
pub struct RaftNode {
    node_id: NodeId,
    /// Type-erased openraft handle. The two underlying networks are
    /// behind the same `Raft<C>` because we instantiate at construction
    /// time and never swap.
    raft: Raft<ClusterTypeConfig>,
    /// Held for the test surface (`state_machine()` accessor) and so
    /// the strong ref keeps the `Arc<Mutex<Storage>>` alive while the
    /// engine is using it. `dead_code` allow because in `--release`
    /// without `cfg(test)` only the storage clone-through path reads
    /// it through openraft.
    #[allow(dead_code)]
    state_machine: AuditStateMachine,
    log_store: InMemoryRaftLogStore,
    /// Held so we can re-register peers in tests + so the in-process
    /// factory's Arc-shared registry stays alive.
    #[allow(dead_code)]
    network: NetworkBackend,
}

impl RaftNode {
    /// Default openraft `Config` tuned for a 3-node test cluster:
    /// short tick + heartbeat so leader election fires within ~1s.
    /// Production deployments should bump these (see
    /// `docs/cluster-mode.md`).
    fn default_test_config(cluster_name: &str) -> Config {
        Config {
            cluster_name: cluster_name.to_string(),
            heartbeat_interval: 100,
            election_timeout_min: 300,
            election_timeout_max: 600,
            // Keep replication frequent so tests don't time out.
            max_payload_entries: 64,
            ..Default::default()
        }
    }

    /// Construct a node wired to the in-process test network. The
    /// caller is responsible for registering this node into the
    /// shared factory afterwards via `factory.register(...)`.
    pub async fn in_process_for_test(
        node_id: NodeId,
        cluster_name: &str,
        storage: Arc<Mutex<Storage>>,
        audit_signer: Arc<DevSigner>,
        factory: InProcessNetworkFactory,
    ) -> anyhow::Result<Arc<Self>> {
        let config = Arc::new(Self::default_test_config(cluster_name).validate()?);
        let log_store = InMemoryRaftLogStore::new();
        let state_machine = AuditStateMachine::new(storage, audit_signer);
        let raft = Raft::new(
            node_id,
            config,
            factory.clone(),
            log_store.clone(),
            state_machine.clone(),
        )
        .await?;
        Ok(Arc::new(Self {
            node_id,
            raft,
            state_machine,
            log_store,
            network: NetworkBackend::InProcess(factory),
        }))
    }

    /// Construct a node wired to the HTTP network factory. Used by the
    /// docker-compose deployment; not exercised in tests yet.
    pub async fn with_http_network(
        node_id: NodeId,
        cluster_name: &str,
        storage: Arc<Mutex<Storage>>,
        audit_signer: Arc<DevSigner>,
    ) -> anyhow::Result<Arc<Self>> {
        let config = Arc::new(Self::default_test_config(cluster_name).validate()?);
        let log_store = InMemoryRaftLogStore::new();
        let state_machine = AuditStateMachine::new(storage, audit_signer);
        let factory = HttpNetworkFactory::new();
        let raft = Raft::new(
            node_id,
            config,
            factory.clone(),
            log_store.clone(),
            state_machine.clone(),
        )
        .await?;
        Ok(Arc::new(Self {
            node_id,
            raft,
            state_machine,
            log_store,
            network: NetworkBackend::Http(factory),
        }))
    }

    /// Initialise the cluster on this node. Called exactly once on the
    /// chosen bootstrap leader; the other nodes are added via
    /// `add_learner` / `change_membership`.
    ///
    /// `peers` is `(node_id, BasicNode)` for every initial cluster
    /// member, including this one.
    pub async fn initialize(&self, peers: BTreeMap<NodeId, BasicNode>) -> anyhow::Result<()> {
        self.raft.initialize(peers).await?;
        Ok(())
    }

    /// Add a learner. Idempotent — if the target is already a member
    /// this is a no-op as far as the cluster is concerned.
    pub async fn add_learner(&self, node_id: NodeId, node: BasicNode) -> anyhow::Result<()> {
        self.raft.add_learner(node_id, node, true).await?;
        Ok(())
    }

    /// Promote learners to voting members. EXPERIMENTAL: takes the
    /// non-joint path which is unsafe in flight; only call on a healthy
    /// cluster with one membership change at a time. See
    /// `docs/cluster-mode.md` for the joint-consensus follow-up.
    pub async fn change_membership(&self, voters: BTreeSet<NodeId>) -> anyhow::Result<()> {
        self.raft.change_membership(voters, false).await?;
        Ok(())
    }

    /// Submit an `AuditAppend` through Raft — leader replicates,
    /// followers apply. Returns the leader's local seq + event_hash.
    pub async fn client_write(
        &self,
        req: super::types::AuditAppend,
    ) -> anyhow::Result<super::types::AuditAppendResponse> {
        let r = self.raft.client_write(req).await?;
        Ok(r.data)
    }

    /// The openraft handle. Use this when you need to issue an RPC the
    /// `RaftNode` wrapper doesn't proxy.
    pub fn raft(&self) -> &Raft<ClusterTypeConfig> {
        &self.raft
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Test-only handle: lets the e2e test snapshot the SQLite row count
    /// after replication. Not exposed in production code.
    #[cfg(test)]
    pub fn state_machine(&self) -> &AuditStateMachine {
        &self.state_machine
    }

    /// Test-only handle: register this node with the in-process factory
    /// so peers can RPC into it. Returns Err if not constructed in
    /// in-process mode.
    #[cfg(test)]
    pub async fn register_self_in_factory(self: &Arc<Self>) -> anyhow::Result<()> {
        match &self.network {
            NetworkBackend::InProcess(f) => {
                f.register(self.node_id, self.clone()).await;
                Ok(())
            }
            NetworkBackend::Http(_) => Err(anyhow::anyhow!(
                "register_self_in_factory only valid for in-process test nodes"
            )),
        }
    }

    /// Drop reference to silence unused warnings on the log store
    /// handle (it's held so tests / future work can poke it directly).
    #[allow(dead_code)]
    pub(crate) fn log_store(&self) -> &InMemoryRaftLogStore {
        &self.log_store
    }
}
