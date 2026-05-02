# Competitive benchmarks — SBO3L vs OPA vs Casbin vs in-process baseline

> **Reproducibility-first proof doc.** Every claim below is anchored to a runnable benchmark and a rig fingerprint. Numbers without a fingerprint are placeholders.

## How to reproduce

```bash
# Full run (~5 min on a modern dev machine)
./scripts/run-competitive-benchmarks.sh

# Quick smoke (~30s; not for publication)
./scripts/run-competitive-benchmarks.sh quick

# Filter to a specific bench
./scripts/run-competitive-benchmarks.sh policy
```

The runner emits:
1. `benchmarks/competitive/target/criterion/report/index.html` — interactive HTML report.
2. `benchmarks/competitive/results-<host>-<date>.json` — machine-readable rollup with rig fingerprint.

## The benchmark

All four enforcers evaluate the **same logical policy**: deny if the recipient address is not in a 100-element allowlist. This is the most-frequently-deployed real-world boundary, so it's the apples-to-apples comparison.

| Enforcer | Library | Language |
|---|---|---|
| **SBO3L** | `sbo3l-policy` (HashSet-backed allowlist primitive) | Rust |
| **OPA** | `regorus 0.2` (pure-Rust Rego interpreter from Microsoft) | Rust |
| **Casbin** | `casbin 2.x` (pure-Rust ABAC enforcer) | Rust |
| **Baseline** | `std::collections::HashSet` direct lookup | Rust |

Source: [`benchmarks/competitive/benches/competitor_comparison.rs`](../../benchmarks/competitive/benches/competitor_comparison.rs).

## Hardware rig (placeholder — populate from `results-<host>-<date>.json`)

| Field | Value |
|---|---|
| Host | _populate from rollup_ |
| Date | _populate from rollup_ |
| CPU model | _populate from rollup_ |
| CPU cores | _populate from rollup_ |
| Memory GB | _populate from rollup_ |
| Rust toolchain | _populate from rollup_ |
| Git HEAD | _populate from rollup_ |
| Cargo.toml SHA-256 | _populate from rollup_ |

## Reference rig — GitHub Actions ubuntu-latest (CI baseline)

This is the **reference rig** numbers Daniel can compare any local run against. Populated automatically by the `competitive-benchmarks` CI workflow. The CI runner is intentionally a noisy environment (shared with other workloads) — local-machine numbers will typically be **2-5× faster** for in-memory benchmarks.

| Bench function | Mean (ns) | ops/sec | Notes |
|---|---|---|---|
| `competitor_comparison/sbo3l_allowlist_check` | _CI populates_ | _CI populates_ | Direct HashSet lookup primitive |
| `competitor_comparison/opa_regorus_evaluate` | _CI populates_ | _CI populates_ | Rego query evaluation against pre-loaded data |
| `competitor_comparison/casbin_enforce` | _CI populates_ | _CI populates_ | ABAC `enforce()` call with Tokio runtime block_on |
| `competitor_comparison/baseline_hashmap_allowlist` | _CI populates_ | _CI populates_ | Lower bound — what's a HashSet `contains` worth? |

## Honest interpretation

### What this benchmark proves

✅ **In-process per-decision overhead** for the boundary-check portion.
✅ **Throughput ceiling** at infinite concurrency (single-threaded measurement scaled by core count).
✅ **Apples-to-apples comparison** at the boundary-check level.

### What this benchmark does NOT prove

❌ **End-to-end daemon throughput.** SBO3L produces a signed PolicyReceipt + audit row + chain hash on each decision; OPA and Casbin do not (out-of-the-box). For the full daemon comparison, see [`crates/sbo3l-server/examples/load_test.rs`](../../crates/sbo3l-server/examples/load_test.rs) (Phase 3.4 honest 7.5K rps measurement).

❌ **Cold-start latency.** Criterion warm-runs by design; first 100 invocations are 10–100× slower across all four enforcers.

❌ **Memory under load.** Use `valgrind --tool=massif` against a daemon instance for memory.

❌ **Network RTT.** All four are in-process; daemon-mode adds RTT to all sides.

❌ **Different feature sets.** SBO3L's policy is a **superset** — budget enforcement, MEV guard, multi-tenant isolation, signed receipts, audit chain. OPA and Casbin would need additional code to match these. The boundary-check portion measured here is the common subset.

### Why use SBO3L instead of OPA or Casbin?

The benchmark comparison is **not** "which is faster" — it's "what do you get for the cost." Per-decision overhead (the in-process number) tells you the **floor**. The ceiling is determined by the rest of the boundary:

| Feature | SBO3L | OPA | Casbin |
|---|---|---|---|
| Allowlist enforcement | ✅ | ✅ | ✅ |
| Budget enforcement (per_tx / daily / monthly) | ✅ | ❌ (custom Rego required) | ❌ (out of model scope) |
| Tamper-evident audit chain | ✅ (Ed25519 + chain_hash_v2) | ❌ (logs only) | ❌ (no audit) |
| Signed PolicyReceipt | ✅ | ❌ | ❌ |
| Capsule export + WASM verify | ✅ (built-in `/proof` page) | ❌ | ❌ |
| Compliance audit-control mapping | ✅ (4 frameworks) | ❌ | ❌ |
| Multi-tenant isolation | ✅ (V010 SQL-level) | ❌ (caller responsibility) | ✅ (model-level) |
| ENS-anchored agent identity | ✅ | ❌ | ❌ |

If you only need allowlist enforcement, OPA or Casbin are simpler. If you need the SBO3L feature set, you'd write equivalent code on top of OPA/Casbin and pay the **sum** of all those costs — at which point SBO3L's per-decision overhead is no longer a meaningful comparison.

## Status

🟡 **Reference numbers pending.** The benchmark suite + run script + harness are committed. CI workflow runs the benchmarks but the result aggregation step is wired to a follow-up to publish numbers into this doc.

Daniel can run locally any time:

```bash
./scripts/run-competitive-benchmarks.sh
# Paste results-<host>-<date>.json into the rig table above
```

## See also

- [`benchmarks/competitive/README.md`](../../benchmarks/competitive/README.md) — full bench harness docs
- [`scripts/run-competitive-benchmarks.sh`](../../scripts/run-competitive-benchmarks.sh) — runner
- [`crates/sbo3l-server/examples/load_test.rs`](../../crates/sbo3l-server/examples/load_test.rs) — daemon-mode throughput
- [`docs/compliance/audit-log-as-evidence.md`](../compliance/audit-log-as-evidence.md) — what the SBO3L superset features get you
