//! `RaftTypeConfig` and the application-level request / response types.
//!
//! Kept in a dedicated submodule because `declare_raft_types!` produces
//! the canonical `ClusterTypeConfig` struct and openraft is verbose about
//! the GAT-bound shape; pulling everything else through this one config
//! is the cleanest seam.

use std::io::Cursor;

use serde::{Deserialize, Serialize};

/// 64-bit node identifier. We use `u64` because openraft's defaults
/// already key on `u64` and the operator-supplied env var
/// `SBO3L_NODE_ID` parses to `u64` cleanly.
pub type NodeId = u64;

/// The single application-level request shape we replicate through Raft.
///
/// In this scaffold the only thing the cluster log replicates is "a new
/// audit event was appended on the leader". The state machine on each
/// node consumes this entry by calling
/// `Storage::audit_append_for_tenant` against its local SQLite — the
/// hash-link / signature is recomputed locally so each node's chain is
/// internally consistent. The follower's chain `event_hash` will differ
/// from the leader's because `id` (a ULID) and `ts` are sampled
/// per-append; that is acceptable for the scaffold and is documented as
/// future work in `docs/cluster-mode.md` (deterministic-replay would
/// require carrying the leader-stamped `id` + `ts` through the log
/// entry, which is straightforward but out of scope for this PR).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditAppend {
    /// Tenant id the append is scoped to. Single-tenant deployments
    /// pass [`sbo3l_storage::DEFAULT_TENANT_ID`].
    pub tenant_id: String,
    /// Event type (e.g. `"runtime_started"`, `"policy_decided"`).
    pub event_type: String,
    /// Actor (e.g. `"sbo3l-server"`).
    pub actor: String,
    /// Subject id (e.g. payment-request id).
    pub subject_id: String,
    /// Hex-encoded payload hash (already computed on the leader).
    pub payload_hash: String,
}

/// The state machine's response to applying one [`AuditAppend`] entry.
///
/// Currently just the local seq the row landed at + the per-node
/// `event_hash`. Returned to the caller of `Raft::client_write` on the
/// leader; followers compute their own values when they apply.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditAppendResponse {
    pub seq: u64,
    pub event_hash: String,
}

openraft::declare_raft_types!(
    /// Concrete openraft type config for the SBO3L cluster scaffold.
    ///
    /// Defaults left intact: `NodeId = u64`, `Node = BasicNode`,
    /// `Entry = openraft::Entry<Self>`, `SnapshotData = Cursor<Vec<u8>>`,
    /// `AsyncRuntime = TokioRuntime`. Only `D` (request) and `R`
    /// (response) are application-specific.
    pub ClusterTypeConfig:
        D = AuditAppend,
        R = AuditAppendResponse,
        SnapshotData = Cursor<Vec<u8>>,
);
