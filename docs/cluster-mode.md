# Cluster mode (Raft) — EXPERIMENTAL

> **EXPERIMENTAL — not production-ready. Needs hardening before deployment.**
>
> This document covers R14 P4: a 3-node Raft cluster scaffold using
> `openraft` 0.9.x. It is **OFF by default** and gated on
> `--features cluster` for `sbo3l-server`. The single-node daemon — the
> existing `docker compose up sbo3l` flow — is not affected by anything in
> this doc; the cluster code lives behind a feature flag and does not
> ship in the default build.

## Why

The single-node daemon writes the audit chain to a local SQLite. That's
fine for the hackathon demo and for any deployment where one box is
acceptable. Production deployments wanting (a) tolerance to a single-node
crash, (b) read scalability across more than one machine, or (c) a
basis for cross-region replication eventually need replicated state. The
Raft scaffold is the smallest credible step in that direction.

## What works

- **3-node leader election.** `openraft::Raft` with the
  `single-term-leader` feature elects a leader within ~300-600ms (test
  config) of cluster bootstrap. The in-process e2e test
  (`cluster::tests::e2e_three_node_leader_election_and_log_replication`)
  exercises this.
- **Log replication of new audit appends.** A client write sent to the
  leader (`Raft::client_write` with an `AuditAppend` payload) is replicated
  to followers via `RaftNetwork::append_entries`. Each follower's state
  machine then applies the entry to its local SQLite via the existing
  `Storage::audit_append_for_tenant` API.
- **Basic peer membership.** `POST /v1/admin/cluster/join` and `POST
  /v1/admin/cluster/leave` add/remove voters. Membership changes go
  through Raft itself — they're committed log entries.
- **HTTP RPC surface for inter-node traffic.** Plain JSON over POST at
  `/v1/admin/cluster/raft/{append-entries,install-snapshot,vote}`.
  Sufficient for the docker-compose deployment.

## What's NOT yet (read this before deploying)

1. **No durable log compaction / snapshot install.**
   `RaftSnapshotBuilder::build_snapshot` returns an empty `Vec<u8>` and
   `install_snapshot` does not rebuild the SQLite chain from snapshot
   bytes. A long-running cluster will accumulate log entries indefinitely
   in the in-memory log store and OOM. Closing this gap requires
   serialising the audit-chain state into snapshot bytes and rebuilding
   on `install_snapshot`.

2. **No partition-recovery testing.** toxiproxy partition tests are
   explicitly out of scope of this PR. The in-process e2e test
   demonstrates the happy path (3 nodes up, 1 elected, replicate, all
   apply); a partitioned-network test harness needs to live alongside
   the cluster code so we can verify quorum behaviour on the way back
   together.

3. **No joint-consensus membership change.** `change_membership` is
   called with `retain=false`, taking the non-joint path. Safe only when
   one node is added/removed at a time on a healthy cluster. A correct
   implementation uses joint consensus (C_old + C_new committed before
   transitioning to C_new alone) to tolerate failures during the
   transition.

4. **Durability is "best-effort", not Raft-correct.** The
   `InMemoryRaftLogStore` impl signals `log_io_completed(Ok(()))`
   immediately after the in-memory write returns — there's no fsync
   boundary because there's no disk. openraft requires `save_vote` to
   `fsync` before returning (otherwise two leaders can be elected
   concurrently after a crash + restart). Implementing this on top of
   sled or rocksdb is straightforward but takes a careful audit pass to
   make sure every place the trait demands persistence actually pins
   the data to disk.

5. **The HTTP request hot path (`POST /v1/payment-requests`) does NOT
   write through Raft.** It still writes locally on whichever node
   received the request. Consumers wanting cluster-replicated audit
   chain currently call the cluster API directly. Raft-fronting the
   request handler changes its semantics (write quorum, leader redirect,
   idempotency-across-nodes) and is deferred to P5+.

6. **Per-event `id` and `ts` differ across nodes.** Each follower's
   state machine generates a fresh ULID + timestamp when it applies the
   `AuditAppend`. The leader's local `event_hash` will not match any
   follower's. This is acceptable for replication-of-history but means
   per-node chains can't be byte-compared. A deterministic-replay variant
   would carry the leader-stamped `id` + `ts` through the log entry.

7. **No auth on inter-node RPCs.** The `/v1/admin/cluster/raft/*`
   endpoints are unauthenticated in the scaffold. A production
   deployment needs mTLS or a shared HMAC.

## Architecture

```
+---------+        +---------+        +---------+
|  node-1 |<------>|  node-2 |<------>|  node-3 |
|  (leader|        |(follower|        |(follower|
|         |        |         |        |         |
| RaftNet |        | RaftNet |        | RaftNet |
| Storage |        | Storage |        | Storage |
| StateMch|        | StateMch|        | StateMch|
|    |    |        |    |    |        |    |    |
|  SQLite |        |  SQLite |        |  SQLite |
+---------+        +---------+        +---------+
```

- **`RaftNetwork`** — the openraft trait that sends `AppendEntries` /
  `Vote` / `InstallSnapshot` RPCs to peers. We have two impls:
  `InProcessNetworkFactory` (test-only, dispatches directly to peer
  `Raft` handles in the same process) and `HttpNetworkFactory` (POSTs
  JSON to peers' `/v1/admin/cluster/raft/*`).
- **`RaftLogStorage`** — `InMemoryRaftLogStore`. Holds vote + log
  entries in `Arc<Mutex<…>>`. Not durable.
- **`RaftStateMachine`** — `AuditStateMachine`. Applies `AuditAppend`
  entries to the existing `Storage` (SQLite-backed audit chain).

## Quickstart

```bash
# 1. Build + spin up 3 nodes:
docker compose -f docker-compose-cluster.yml up

# 2. Initialise the cluster (TODO: this is currently a manual step;
#    a `cluster init` HTTP route is on the follow-up list).

# 3. Inspect cluster state:
curl http://localhost:8731/v1/admin/cluster/status | jq
# → {"node_id": 1, "leader_id": 1, "current_term": 2, ...}

# 4. From any node, ask which node is the leader; the cluster API
#    surface lives at /v1/admin/cluster/* on every node.
```

## Testing

```bash
# Build with the cluster feature:
cargo build -p sbo3l-server --features cluster

# Run cluster unit + e2e tests:
cargo test -p sbo3l-server --features cluster cluster::

# The e2e test spawns 3 in-process Raft nodes via the
# InProcessNetworkFactory, demonstrates leader election within 10s,
# pushes 3 AuditAppends through the leader, and verifies all 3
# follower SQLite-backed state machines applied 3 rows.
```

## File map

| Path | Purpose |
| --- | --- |
| `crates/sbo3l-server/src/cluster/mod.rs` | module root + EXPERIMENTAL caveats summary |
| `crates/sbo3l-server/src/cluster/types.rs` | `RaftTypeConfig` + `AuditAppend` / `AuditAppendResponse` |
| `crates/sbo3l-server/src/cluster/storage.rs` | `InMemoryRaftLogStore` (RaftLogStorage impl) |
| `crates/sbo3l-server/src/cluster/state_machine.rs` | `AuditStateMachine` (RaftStateMachine impl, applies to SQLite) |
| `crates/sbo3l-server/src/cluster/network.rs` | `InProcessNetworkFactory` + `HttpNetworkFactory` |
| `crates/sbo3l-server/src/cluster/raft.rs` | `RaftNode` — assembled openraft handle |
| `crates/sbo3l-server/src/cluster/http.rs` | axum handlers for `/v1/admin/cluster/*` |
| `crates/sbo3l-server/src/cluster/tests.rs` | unit + e2e tests |
| `docker-compose-cluster.yml` | 3-node deployment (root) |
| `docs/cluster-mode.md` | this file |

## Follow-up work

The following are explicitly **NOT** shipped in R14 P4 and are tracked
for future hardening rounds:

- **toxiproxy partition tests.** Build a test harness that injects
  network partitions between cluster members and asserts on quorum
  behaviour during the partition + recovery after.
- **Durable log store.** Replace `InMemoryRaftLogStore` with a
  sled/rocksdb-backed impl that fsync-commits vote + log writes. Run
  the openraft `Suite::test_all` test battery against it.
- **Snapshot serialisation.** Define an audit-chain snapshot format
  (probably a CBOR-encoded list of `SignedAuditEvent` rows) and wire
  `build_snapshot` / `install_snapshot` to use it.
- **Joint-consensus membership.** Replace single-step
  `change_membership` with the joint protocol so failures during the
  transition don't lose quorum.
- **Raft-fronting the request hot path.** Replace the local
  `audit_append_for_tenant` call inside `POST /v1/payment-requests`
  with `Raft::client_write`. Requires leader-redirect, idempotency
  across nodes, and a careful look at the request lifecycle so the
  HTTP response only fires after replication.
- **mTLS on inter-node RPCs.** Terminate `/v1/admin/cluster/raft/*`
  on a separate listener bound only to a cluster-private interface,
  with mTLS for peer auth.
