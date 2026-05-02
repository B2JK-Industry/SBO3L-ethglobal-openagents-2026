//! `RaftStateMachine` impl that applies `AuditAppend` log entries to
//! the local SQLite-backed audit chain.
//!
//! The state machine wraps an `Arc<Mutex<Storage>>` shared with the rest
//! of the server. This makes the "state machine" really just a thin
//! adapter onto the existing `Storage::audit_append_for_tenant` write
//! path — Raft only sequences the calls.
//!
//! See module docstring of `cluster/mod.rs` for the snapshot caveats:
//! `build_snapshot` / `install_snapshot` here are placeholders that
//! satisfy the trait but do not actually compact log entries. A
//! long-running cluster will accumulate log entries indefinitely.

use std::io::Cursor;
use std::sync::Arc;

use openraft::storage::{RaftSnapshotBuilder, RaftStateMachine};
use openraft::{
    Entry, EntryPayload, LogId, OptionalSend, RaftTypeConfig, Snapshot, SnapshotMeta, StorageError,
    StoredMembership,
};
use tokio::sync::Mutex;

use sbo3l_core::signer::DevSigner;
use sbo3l_storage::audit_store::NewAuditEvent;
use sbo3l_storage::Storage;

use super::types::{AuditAppend, AuditAppendResponse, ClusterTypeConfig};

/// Persisted state-machine pointer set. Tracks the highest applied log
/// id + the membership the engine should resume with on restart.
#[derive(Default, Clone)]
struct SmState {
    last_applied: Option<LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
    last_membership: StoredMembership<
        <ClusterTypeConfig as RaftTypeConfig>::NodeId,
        <ClusterTypeConfig as RaftTypeConfig>::Node,
    >,
    /// Monotonically-increasing snapshot id, bumped each time
    /// `build_snapshot` is invoked. Mostly for debug — the snapshot
    /// payload itself is empty in this scaffold.
    snapshot_idx: u64,
    /// The most recent snapshot returned by `build_snapshot` /
    /// installed by `install_snapshot`. Held so `get_current_snapshot`
    /// can return it again if the engine asks.
    current_snapshot: Option<StoredSnapshot>,
}

#[derive(Clone)]
struct StoredSnapshot {
    meta: SnapshotMeta<
        <ClusterTypeConfig as RaftTypeConfig>::NodeId,
        <ClusterTypeConfig as RaftTypeConfig>::Node,
    >,
    /// Empty bytes for the scaffold — see module docstring.
    data: Vec<u8>,
}

/// Audit-chain state machine. The Raft engine drives `apply()` for
/// every committed log entry; each entry triggers a local SQLite append
/// via the existing `Storage::audit_append_for_tenant` API.
#[derive(Clone)]
pub struct AuditStateMachine {
    storage: Arc<Mutex<Storage>>,
    audit_signer: Arc<DevSigner>,
    state: Arc<Mutex<SmState>>,
}

impl AuditStateMachine {
    pub fn new(storage: Arc<Mutex<Storage>>, audit_signer: Arc<DevSigner>) -> Self {
        Self {
            storage,
            audit_signer,
            state: Arc::new(Mutex::new(SmState::default())),
        }
    }

    /// Test helper: read the current applied count by counting audit
    /// rows in the underlying SQLite. Used by the e2e test to verify
    /// each follower applied the leader's appends.
    #[cfg(test)]
    pub async fn audit_count(&self) -> u64 {
        let s = self.storage.lock().await;
        s.audit_count().unwrap_or(0)
    }
}

impl RaftSnapshotBuilder<ClusterTypeConfig> for AuditStateMachine {
    async fn build_snapshot(
        &mut self,
    ) -> Result<
        Snapshot<ClusterTypeConfig>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        // EXPERIMENTAL: snapshots are placeholders. A real
        // implementation would serialise the audit chain (or a digest
        // of it) into `data` so a freshly-joining follower can catch
        // up without replaying the full log. See `docs/cluster-mode.md`.
        let mut state = self.state.lock().await;
        state.snapshot_idx = state.snapshot_idx.saturating_add(1);
        let meta = SnapshotMeta {
            last_log_id: state.last_applied,
            last_membership: state.last_membership.clone(),
            snapshot_id: format!("sbo3l-snap-{}", state.snapshot_idx),
        };
        let stored = StoredSnapshot {
            meta: meta.clone(),
            data: Vec::new(),
        };
        state.current_snapshot = Some(stored.clone());
        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(stored.data)),
        })
    }
}

impl RaftStateMachine<ClusterTypeConfig> for AuditStateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
            StoredMembership<
                <ClusterTypeConfig as RaftTypeConfig>::NodeId,
                <ClusterTypeConfig as RaftTypeConfig>::Node,
            >,
        ),
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        let s = self.state.lock().await;
        Ok((s.last_applied, s.last_membership.clone()))
    }

    async fn apply<I>(
        &mut self,
        entries: I,
    ) -> Result<Vec<AuditAppendResponse>, StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>
    where
        I: IntoIterator<Item = Entry<ClusterTypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let mut responses = Vec::new();
        let mut state = self.state.lock().await;
        for entry in entries {
            state.last_applied = Some(entry.log_id);
            match entry.payload {
                EntryPayload::Blank => {
                    // No-op blank entry openraft inserts on leader
                    // election. Push a placeholder response so the
                    // input/output cardinalities match.
                    responses.push(AuditAppendResponse {
                        seq: 0,
                        event_hash: String::new(),
                    });
                }
                EntryPayload::Membership(m) => {
                    state.last_membership = StoredMembership::new(Some(entry.log_id), m);
                    responses.push(AuditAppendResponse {
                        seq: 0,
                        event_hash: String::new(),
                    });
                }
                EntryPayload::Normal(req) => {
                    // The actual replicated audit append. Drop the
                    // engine lock briefly so the SQLite `INSERT` doesn't
                    // hold the SmState mutex for any longer than needed.
                    drop(state);
                    let resp =
                        apply_audit_append(self.storage.clone(), self.audit_signer.clone(), req)
                            .await;
                    state = self.state.lock().await;
                    responses.push(resp);
                }
            }
        }
        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<
        Box<<ClusterTypeConfig as RaftTypeConfig>::SnapshotData>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<
            <ClusterTypeConfig as RaftTypeConfig>::NodeId,
            <ClusterTypeConfig as RaftTypeConfig>::Node,
        >,
        snapshot: Box<<ClusterTypeConfig as RaftTypeConfig>::SnapshotData>,
    ) -> Result<(), StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>> {
        // EXPERIMENTAL: install_snapshot does not actually rebuild the
        // SQLite chain from snapshot bytes (the bytes are empty, see
        // `build_snapshot`). What it does is advance the in-memory
        // pointer set so `applied_state` reports the post-snapshot
        // log id + membership; the chain itself stays at whatever
        // `apply()` has produced. Joining a node mid-cluster requires
        // either replaying the full log (works while logs aren't
        // purged) or carrying real chain bytes through `data` (TODO).
        let mut state = self.state.lock().await;
        state.last_applied = meta.last_log_id;
        state.last_membership = meta.last_membership.clone();
        let stored = StoredSnapshot {
            meta: meta.clone(),
            data: snapshot.into_inner(),
        };
        state.current_snapshot = Some(stored);
        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<
        Option<Snapshot<ClusterTypeConfig>>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        let state = self.state.lock().await;
        Ok(state.current_snapshot.as_ref().map(|s| Snapshot {
            meta: s.meta.clone(),
            snapshot: Box::new(Cursor::new(s.data.clone())),
        }))
    }
}

/// Apply one `AuditAppend` to the underlying SQLite-backed `Storage`.
///
/// Pulled out as a free function so it can be unit-tested without
/// running an openraft engine (see `tests::state_machine_apply_*`).
pub async fn apply_audit_append(
    storage: Arc<Mutex<Storage>>,
    audit_signer: Arc<DevSigner>,
    req: AuditAppend,
) -> AuditAppendResponse {
    let new_event = NewAuditEvent {
        event_type: req.event_type,
        actor: req.actor,
        subject_id: req.subject_id,
        payload_hash: req.payload_hash,
        metadata: serde_json::Map::new(),
        policy_version: None,
        policy_hash: None,
        attestation_ref: None,
        ts: chrono::Utc::now(),
    };
    let mut s = storage.lock().await;
    match s.audit_append_for_tenant(&req.tenant_id, new_event, audit_signer.as_ref()) {
        Ok(signed) => AuditAppendResponse {
            seq: signed.event.seq,
            event_hash: signed.event_hash,
        },
        Err(e) => {
            tracing::error!(error = ?e, "audit_append_for_tenant failed in raft state machine");
            // We deliberately don't propagate the error as a
            // `StorageError` because `RaftStateMachine::apply` errors
            // are Fatal-only in openraft's contract — a SQLite write
            // failure here would tear down the whole cluster.
            // Returning a sentinel response with empty event_hash lets
            // the cluster keep replicating; tests check the SQLite
            // count to verify the row landed.
            AuditAppendResponse {
                seq: 0,
                event_hash: String::new(),
            }
        }
    }
}
