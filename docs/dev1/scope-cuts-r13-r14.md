# Dev 1 — honest scope cuts, R13 + R14

**Authored:** 2026-05-02T16:28Z
**Purpose:** Every R13/R14 brief item that I did NOT fully ship, with the spec, what actually shipped, why I scoped down, and what's needed to finish. Per the project's no-overclaim rule (memory note: `user_role_and_workflow.md`) — and reinforced by Daniel's R12 P3 feedback that "Honest is better than fake" — this is the truth surface.

> **Reading guide:** items grouped by round. Each entry has four lines: **Spec / Shipped / Why / To finish.**

---

## R13 P7 — OpenTelemetry instrumentation

**Spec:** opentelemetry-rust spans on every request + Prometheus exporter on `/v1/metrics` + Tempo/Jaeger trace export + 12-panel Grafana `dashboard.json` + Helm chart for full stack.
**Shipped (PR #303):** Real Prometheus exporter at `/v1/metrics` (counters + 13-bucket histogram), backfilled the existing `/v1/admin/metrics` JSON dashboard so it stops returning placeholder zeros, 9 unit + 2 e2e tests.
**Why scoped down:** OTEL spans, Tempo/Jaeger, dashboard.json, and Helm chart were each ~2h efforts. Bundling all five into one "P7 (~2h)" was unrealistic; I shipped the demonstrably-observable Prometheus surface that judges can `curl` today.
**To finish:** OTEL spans landed in **R14 #330** (separate PR). Grafana dashboard.json landed in **R14 #330**. Helm chart landed in **R14 #329**. Tempo/Jaeger forwarding still **NOT shipped** — operator brings their own collector (documented in `docs/observability.md`).

## R14 P1 — gRPC API alongside REST

**Spec:** tonic-based gRPC server, generated TS + Python clients, criterion benchmark vs REST, bidirectional streaming, examples for both languages, 20+ unit + 10 integration tests.
**Shipped (PR #322):** tonic 0.12 + 11 unit + 3 e2e tests, Decide + Health + AuditChainStream RPCs (server-streaming on the last), TS client + quickstart example, `docs/api/grpc.md`. ~1.5K LOC.
**Why scoped down:**
- **No Python client** — packaging overhead (`pyproject.toml`, package layout, separate publish workflow) was a multi-hour cost for a second client whose value over the TS one is duplicative for hackathon judges.
- **No criterion benchmark** — gRPC-vs-REST throughput comparison is a separate perf-bench effort with its own test harness; the load test we have (`scripts/perf/load-test.sh`) measures REST alone.
- **No bidi streaming** — server streaming on `AuditChainStream` covers the realistic operator use case (subscribe to chain growth); client streaming would be a different RPC with no current consumer.
**To finish:** Add `sdks/grpc-py/` mirroring `sdks/grpc-ts/` if a Python consumer materialises. Add `benches/grpc_vs_rest.rs` if performance becomes a sales angle.

## R14 P2 — Backup + restore + S3 + Parquet export

**Spec:** `sbo3l admin backup --to <local-or-s3-uri>` + age encryption + `--format parquet`, daily cron in `.github/workflows/audit-backup.yml`, point-in-time recovery, 25+ tests.
**Shipped (PR #327):** `sbo3l admin {backup,restore,export,verify}` behind `--features admin_backup`. tar.zst + age (full round-trip), JSONL export, 5 unit + 7 integration tests.
**Why scoped down:**
- **`s3://` URIs are PARSED but rejected** with a clear "needs aws-sdk-s3 + creds" message. S3 is gated on the same R15 cred-flip that KMS expected; with Daniel confirming no cloud creds, this stays as the deliberate exclusion.
- **`--format parquet` errors** with "arrow-rs adds ~50 transitive crates; deferred to a separate PR with explicit dep-tree review". The dep-tree growth wasn't worth the format choice for a hackathon repo.
- **No daily cron in `.github/workflows/audit-backup.yml`** — the cron file would target a CI-hosted DB (which doesn't exist) and upload to S3 (which we can't). Operator's own ops team owns this in production.
- **No point-in-time recovery to arbitrary seq** — restore is whole-DB. PITR via audit-event replay is a separate primitive that needs a "redo log" abstraction the codebase doesn't have.
**To finish:** Implement `s3://` upload behind a new `admin_backup_s3` feature when cloud creds are available. Implement Parquet export (probably as a separate `admin_backup_parquet` feature). PITR is a distinct piece of work with its own design review.

## R14 P3 — AWS + GCP KMS, real end-to-end

**Spec:** `aws-sdk-kms` + `google-cloud-kms` wired, `EthSigner` impl for both, EIP-55 derivation from KMS public-key, integration tests gated on `AWS_KMS_TEST_ENABLED` / `GCP_KMS_TEST_ENABLED`, runbooks.
**Shipped (PR #324):** Both AWS + GCP backends fully wired behind `eth_kms_aws` / `eth_kms_gcp` features, mock-client trait wrappers, 53 unit tests covering DER signature decode + SPKI pubkey decode + address derivation + recovery byte derivation, gated integration tests, runbooks at `docs/kms-aws-setup.md` + `docs/kms-gcp-setup.md`.
**Why scoped down:** Daniel confirmed no AWS/GCP creds available now or in R15. The "real end-to-end" claim is hollow without creds — every signature path runs against the mock.
**To finish (when Daniel decides to provision):**
1. Run `MOCK_KMS_TEST_ENABLED=1 cargo test -p sbo3l-core --features eth_kms_aws --test aws_kms_live` against the mock (deterministic; doesn't need real KMS).
2. If/when real creds appear, set `SBO3L_AWS_KMS_KEY_ARN=arn:aws:kms:...` + `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` + a separate live-test env, and run a parallel `*_live_real.rs` suite.
3. The R15 brief's `MOCK_KMS_TEST_ENABLED` rename is the cleaner path — it makes the no-cred-needed mock test the canonical CI gate, and live-real tests become an opt-in operator concern. See `closeout-status.md` §KMS-shift for the rename plan.

## R14 P4 — 3-node Raft cluster, production-ready

**Spec:** openraft + replicated audit chain + leader election + auto-failover + HTTP `/v1/admin/cluster/{status,join,leave}` + docker-compose-cluster.yml + toxiproxy partition tests + 30+ unit + 10 e2e tests + production review.
**Shipped (PR #323):** openraft 0.9.24 scaffold under `--features cluster`, RaftLogStorage + RaftStateMachine traits backed by the existing SQLite Storage, in-process 3-node e2e (leader election + 3-entry replication), 13 tests total. **Marked EXPERIMENTAL** at the top of `docs/cluster-mode.md` and the PR description.
**Why scoped down:** Production-grade Raft with replicated audit chain + partition tolerance is genuinely a multi-day effort, not 6h. The brief acknowledged this with the "experimental tag" framing.
**Explicit gaps documented in `docs/cluster-mode.md`:**
- Snapshot install / log compaction (openraft will OOM on long-running nodes without periodic snapshots).
- Partition recovery testing — toxiproxy harness is a separate piece of work.
- Joint consensus for safe membership changes (current `add/remove` is naïve).
- Durability tuning — openraft's Storage trait has subtle `fsync` requirements at specific boundaries.
- Raft-fronting `POST /v1/payment-requests` — the API hot path still writes locally; cross-node idempotency through Raft is P5+ work.
- mTLS on inter-node RPCs (deferred to service-mesh integration).
**To finish:** A multi-day production-hardening pass with explicit production review, partition test harness, and the durability fsync audit. Treat as a separate epic, not a follow-up PR.

## R14 P5 — OpenTelemetry full integration

**Spec:** opentelemetry-rust spans + tracing-opentelemetry layer + stdout + Jaeger + Tempo exporters + 12-panel Grafana dashboard.json + docker-compose adds Jaeger + Tempo + Loki services (profile: telemetry) + setup docs.
**Shipped (PR #330):** OTEL 0.31 stack (`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `opentelemetry-stdout`, `tracing-opentelemetry 0.32`) behind `--features otel`, env-var driven exporter selection (`SBO3L_OTEL_EXPORTER=none|stdout|otlp`), per-request span middleware, graceful-shutdown drain, 10 unit + 2 e2e tests, 12-panel `apps/observability/grafana/dashboard.json`, `docs/observability.md` runbook.
**Why scoped down:**
- **No docker-compose Tempo/Jaeger/Loki bring-up** — the daemon is the OTEL EMITTER; the collector is the operator's choice. Bundling a collector stack we don't run/test would be theater. The runbook documents the one-line `otel/opentelemetry-collector-contrib` invocation operators can use.
- **No live integration test against a real collector** — stdout exporter is enough to prove the wiring; the integration test pipes real OTEL output through `stdout` and asserts the span shape.
**To finish:** Add a `docker-compose.observability.yml` with Tempo + Loki + Grafana when an operator actually wants the full stack as part of the project's distribution. Today every reasonable production deployment already runs its own observability tier.

## R14 P6 — Helm chart full

**Spec:** `deploy/helm/sbo3l/` with Chart.yaml + values.yaml (3-node default) + templates (deployment + service + ingress + configmap + secret) + CRDs (`SBO3LPolicy` + `SBO3LCluster`) + helm test runbook + Chart.lock + helm lint clean.
**Shipped (PR #329):** Full chart skeleton with 14 files, helm lint clean, helm template clean across default + full-feature override, EXPERIMENTAL header on README + values.yaml. Includes guarded ingress / configmap / secret / serviceaccount / servicemonitor / poddisruptionbudget templates.
**Why scoped down:**
- **No CRDs** (`SBO3LPolicy`, `SBO3LCluster`) — CRDs without a controller pod are inert; deferred to a separate `sbo3l-operator` chart.
- **No 3-node default** — the audit chain is single-writer; multi-replica without Raft means data loss. Defaults to `replicaCount: 1`. Operators wanting Raft use the experimental `docker-compose-cluster.yml` (or wait for the operator chart that fronts #323).
- **No live cluster apply attempt** — `helm lint` + `helm template` + offline `python yaml.safe_load_all` validated; no minikube/kind run was attempted.
**To finish:** Build a separate `sbo3l-operator` chart that owns the CRDs + reconciler. Validate this chart in CI against a live `kind` cluster (single-node smoke).

---

## Items intentionally left out of *every* round

These were in various briefs but I never claimed to ship them — flagging them here so they don't show up as surprise gaps in a future audit.

- **Phala TEE backend.** Listed alongside aws_kms / gcp_kms in F-5; never wired. Same scope-deferral as KMS but with no realistic R15 path.
- **Per-tenant labels on Prometheus metrics** (PR #303). Single-tenant default keeps cardinality bounded; per-tenant lands when multi-tenant request headers do.
- **WS admin events `Operational` variant emission** (PR #301). The variant exists in the type so dashboards don't break when emitted; no code path emits one yet.
- **gRPC interceptors for auth + idempotency-key** (PR #322). The REST handler enforces both; gRPC parallel surface relies on operator-level network controls today.

---

## How to use this doc

If you're picking up Dev 1 work in R15+: start here, find the surface you care about, read the **To finish** line. Match it against memory notes (`worktree_pattern_for_shared_repo.md`, `shared_worktree_4plus1_friction.md`) and Daniel's brief constraints. Don't re-derive what was deliberately cut — extend it.
