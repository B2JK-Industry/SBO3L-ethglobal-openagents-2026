//! R14 P4 — 3-node Raft cluster scaffold (**EXPERIMENTAL**).
//!
//! This module is gated on `--features cluster` and is **NOT** built into
//! the default `cargo build -p sbo3l-server`. Production deployments
//! continue to run as a single-node daemon writing the audit chain
//! directly to local SQLite; the cluster layer is a parallel surface that
//! demonstrates leader election + log replication of new audit-event
//! appends through openraft.
//!
//! # What works (scaffold deliverables)
//!
//! * 3-node leader election via `openraft::Raft` with the `single-term-leader`
//!   feature enabled — exercised by the in-process e2e test in
//!   `cluster::tests::e2e`.
//! * Replication of [`AuditAppend`] log entries: the leader accepts a
//!   client write (via `Raft::client_write`), Raft replicates the entry
//!   to the followers, and each node's [`state_machine::AuditStateMachine`]
//!   applies it to its local SQLite via the existing
//!   `Storage::audit_append_for_tenant` API.
//! * HTTP surface mounted under `/v1/admin/cluster/*` for cluster status
//!   / membership management plus the openraft inter-node RPCs.
//!
//! # What's NOT yet (read this before deploying)
//!
//! See `docs/cluster-mode.md` — copied in summary form here:
//!
//! 1. **No durable log compaction / snapshot install.** A long-running
//!    leader will OOM as the log grows. Snapshot building / install paths
//!    are stubbed out and return `()` / `Default`.
//! 2. **No partition-recovery testing.** toxiproxy partition tests are
//!    explicitly out of scope of this PR; a follow-up is needed before
//!    treating a partitioned 3-node cluster as a tolerated failure mode.
//! 3. **No joint-consensus membership change.** `add_learner` →
//!    `change_membership` calls are issued sequentially with no joint
//!    config; safe only when one node is added/removed at a time on a
//!    healthy cluster.
//! 4. **Durability is "best-effort", not Raft-correct.** openraft's
//!    `RaftLogStorage` trait has subtle fsync requirements (vote must be
//!    persisted before the response, log writes must survive crash). The
//!    SQLite backend in [`storage::SqliteRaftLogStore`] commits each
//!    append in its own transaction but does not pin the WAL `fsync`
//!    boundary that Raft correctness requires; auditing this is a
//!    separate review pass.
//! 5. **The HTTP request hot path (`POST /v1/payment-requests`) does NOT
//!    write through Raft.** It still writes locally on whichever node
//!    received the request. Consumers wanting cluster-replicated audit
//!    chain currently call the cluster API directly. Raft-fronting the
//!    request handler changes its semantics (write quorum, leader
//!    redirect, idempotency-across-nodes) and is deferred to P5+.

pub mod http;
pub mod network;
pub mod raft;
pub mod state_machine;
pub mod storage;
pub mod types;

pub use raft::RaftNode;
pub use types::{AuditAppend, AuditAppendResponse, ClusterTypeConfig, NodeId};

#[cfg(test)]
mod tests;
