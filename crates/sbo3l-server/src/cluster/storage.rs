//! In-memory [`RaftLogStorage`] implementation for the cluster scaffold.
//!
//! **EXPERIMENTAL — not durable.** Logs and vote live in `Mutex<…>` and
//! evaporate on process restart. A production implementation needs to:
//!
//! 1. Persist `vote` synchronously (sled / rocksdb / a sqlite WAL with
//!    explicit `SYNCHRONOUS=FULL`). openraft requires `save_vote` to
//!    `fsync` before returning — without that, two leaders can be
//!    elected concurrently after a crash + restart.
//! 2. Persist log entries with the same fsync boundary. openraft's
//!    `append` callback signals "logs are persisted on disk"; the
//!    in-memory backend signals success immediately, which is fine for
//!    leader-election demos but wrong for crash safety.
//! 3. Implement `truncate` / `purge` against on-disk segments without
//!    leaving holes (openraft enforces "no hole in logs" — a partial
//!    truncate that leaves index N+1 alive while N is gone violates the
//!    invariant and triggers a Fatal).
//!
//! See `docs/cluster-mode.md` "What's NOT yet" for the full hardening
//! list; this scaffold is sufficient for in-process leader election +
//! log replication tests but should not back any deployment that
//! survives a kill -9.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;

use openraft::storage::{LogFlushed, LogState, RaftLogStorage};
use openraft::{Entry, LogId, OptionalSend, RaftLogReader, RaftTypeConfig, StorageError, Vote};
use tokio::sync::Mutex;

use super::types::ClusterTypeConfig;

/// Inner mutable state shared between the `RaftLogStorage` impl and any
/// log readers. Wrapped in a `Mutex` because openraft serialises writes
/// at the engine level but reads can be concurrent — an `RwLock` would
/// also work; we picked Mutex for simplicity since the test e2e is
/// dominated by leader-election traffic, not log fanout.
#[derive(Default)]
struct Inner {
    /// Persisted vote — needed for crash recovery (we don't crash-recover
    /// in the in-memory scaffold but the trait demands the column).
    vote: Option<Vote<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
    /// Log entries indexed by `LogId.index`. BTreeMap so range queries
    /// are O(log n) lookups + linear scan over the requested range.
    log: BTreeMap<u64, Entry<ClusterTypeConfig>>,
    /// `last_purged_log_id` — set by `purge`. The "no holes" invariant
    /// means everything in `log` has index > this.
    last_purged: Option<LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
    /// `committed` checkpoint — set by `save_committed`. Optional in
    /// the trait; we plumb it through because the openraft engine reads
    /// it back on restart for state-machine sync.
    committed: Option<LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
}

/// In-memory `RaftLogStorage` backed by `Arc<Mutex<Inner>>` so the
/// log-reader handle (`Self`) can be cheaply cloned and handed to
/// replication tasks per openraft's design.
#[derive(Clone, Default)]
pub struct InMemoryRaftLogStore {
    inner: Arc<Mutex<Inner>>,
}

impl InMemoryRaftLogStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RaftLogReader<ClusterTypeConfig> for InMemoryRaftLogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<
        Vec<Entry<ClusterTypeConfig>>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        let inner = self.inner.lock().await;
        // BTreeMap::range honours the same RangeBounds; clone entries
        // because the engine consumes them by value.
        let entries = inner
            .log
            .range(range)
            .map(|(_, e)| Entry {
                log_id: e.log_id,
                payload: e.payload.clone(),
            })
            .collect();
        Ok(entries)
    }
}

impl RaftLogStorage<ClusterTypeConfig> for InMemoryRaftLogStore {
    type LogReader = Self;

    async fn get_log_state(
        &mut self,
    ) -> Result<
        LogState<ClusterTypeConfig>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        let inner = self.inner.lock().await;
        let last_log_id = inner
            .log
            .values()
            .next_back()
            .map(|e| e.log_id)
            .or(inner.last_purged);
        Ok(LogState {
            last_purged_log_id: inner.last_purged,
            last_log_id,
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        // Cheap Arc-clone — both handles share the same Inner.
        self.clone()
    }

    async fn save_vote(
        &mut self,
        vote: &Vote<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    ) -> Result<(), StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>> {
        // EXPERIMENTAL: in-memory only — see module docstring.
        let mut inner = self.inner.lock().await;
        inner.vote = Some(*vote);
        Ok(())
    }

    async fn read_vote(
        &mut self,
    ) -> Result<
        Option<Vote<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        Ok(self.inner.lock().await.vote)
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
    ) -> Result<(), StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>> {
        self.inner.lock().await.committed = committed;
        Ok(())
    }

    async fn read_committed(
        &mut self,
    ) -> Result<
        Option<LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>,
        StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    > {
        Ok(self.inner.lock().await.committed)
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<ClusterTypeConfig>,
    ) -> Result<(), StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>>
    where
        I: IntoIterator<Item = Entry<ClusterTypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        {
            let mut inner = self.inner.lock().await;
            for e in entries {
                inner.log.insert(e.log_id.index, e);
            }
        }
        // EXPERIMENTAL: signal "flushed to disk" immediately. A durable
        // backend would only call this after the WAL fsync returns.
        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(
        &mut self,
        log_id: LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    ) -> Result<(), StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>> {
        let mut inner = self.inner.lock().await;
        // `truncate` is "delete logs [index, +oo)" — split off the
        // range and drop the tail.
        inner.log.split_off(&log_id.index);
        Ok(())
    }

    async fn purge(
        &mut self,
        log_id: LogId<<ClusterTypeConfig as RaftTypeConfig>::NodeId>,
    ) -> Result<(), StorageError<<ClusterTypeConfig as RaftTypeConfig>::NodeId>> {
        let mut inner = self.inner.lock().await;
        // `purge` is "delete logs (-oo, index]" — keep only entries
        // strictly after `log_id.index`.
        let kept = inner.log.split_off(&(log_id.index + 1));
        inner.log = kept;
        inner.last_purged = Some(log_id);
        Ok(())
    }
}
