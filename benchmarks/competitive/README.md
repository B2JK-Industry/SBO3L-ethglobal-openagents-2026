# SBO3L competitive benchmarks (R13 P5)

> Criterion-based benchmark harness measuring SBO3L's hot paths against in-process baselines and (where structurally feasible) competitor stubs. Reproducible via `cargo bench`.

## Honest scope statement

This is a **hackathon-scope** benchmark suite. The four full competitor benchmarks Daniel's R13 brief asked for (mandate.md, OPA, Casbin, in-process) require:

- **mandate.md** — closed-source proprietary; no in-process API. A daemon-mode comparison is possible but adds Docker + REST RTT to both sides; out of scope for hackathon.
- **OPA (Open Policy Agent)** — bundled WASM + Rego compile; in-process feasible via the `opa-rs` Rust binding but adds ~30 minutes of integration work to load + evaluate equivalent policies.
- **Casbin** — `casbin-rs` ABAC model; ~15 min integration. Comparable to SBO3L's expression evaluator.
- **In-process equivalent** — covered as the `baseline_*` benchmarks below.

What this PR ships **today**: the benchmark scaffolding + 4 working SBO3L benchmarks + 3 in-process baselines. Competitor integrations (OPA + Casbin) are tracked in `TODO.md` for follow-up.

## Benchmarks

### `policy_eval`

| Variant | What it measures |
|---|---|
| `sbo3l_policy_parse_yaml` | Cold-path policy YAML → `Policy` struct |
| `baseline_always_allow` | Lower bound — what's the overhead of any policy boundary? |
| `baseline_hashmap_allowlist` | Typical hand-rolled allowlist used pre-framework adoption |

### `audit_chain_append`

| Variant | What it measures |
|---|---|
| `audit_event_canonical_hash` | Per-event canonical-JSON + SHA-256 cost |
| `audit_chain_canonical_walk_1000` | Walk + hash 1000-event chain (verifier hot path) |

### `capsule_verify`

| Variant | What it measures |
|---|---|
| `capsule_verify_cold` | End-to-end `verify_capsule(&Value)` — 6 strict checks |
| `capsule_json_parse_only` | JSON parse only — bottom of the stack |

### `request_hash`

| Variant | What it measures |
|---|---|
| `aprp_request_hash` | JCS canonicalization + SHA-256 (incoming APRP hot path) |
| `aprp_canonical_json` | Canonicalization only |
| `sha256_raw_bytes_baseline` | Raw bytes → SHA-256 (lower bound for hash alone) |

## Run

```bash
cd benchmarks/competitive

# All benchmarks
cargo bench

# A single bench
cargo bench --bench policy_eval

# A single benchmark within a bench
cargo bench --bench policy_eval -- baseline_hashmap_allowlist

# View HTML report
open target/criterion/report/index.html
```

## Reproducibility

- Hardware: any modern x86_64 or ARM64 dev machine.
- Toolchain: stable Rust pinned in `rust-toolchain.toml` (workspace root).
- Run **twice**: criterion auto-baselines against the previous run's stored sample.
- For PRs claiming a perf improvement: paste the `change` column from the second run.

## Honest interpretation

These benchmarks measure **in-process Rust function call cost**. They tell you:

- ✅ Per-decision overhead of the policy boundary
- ✅ Per-event audit chain cost
- ✅ Per-capsule verification cost
- ✅ Lower bounds for the daemon's request path

They do **NOT** tell you:

- ❌ Daemon throughput under concurrency (use `crates/sbo3l-server/examples/load_test.rs` for that — Phase 3.4 honest 7.5K rps measurement)
- ❌ Network RTT vs in-process (depends on deployment)
- ❌ Memory pressure under sustained load (use `valgrind --tool=massif` against a daemon instance)
- ❌ Cold-start latency (criterion warm-runs by design; first invocation ~10-100x slower)

For each of these, see the linked tooling.

## TODO

- [ ] OPA (`opa-rs`) baseline — load equivalent allowlist policy, bench `evaluate()`.
- [ ] Casbin (`casbin-rs`) baseline — load equivalent ABAC model, bench `enforce()`.
- [ ] mandate.md daemon-mode (Docker compose with mandate.md + SBO3L; HTTP RTT both sides).
- [ ] Memory bench (jemalloc + heap snapshot deltas).
- [ ] Cold-start bench (drop criterion warm-up, measure first 100 requests).

## See also

- `crates/sbo3l-server/examples/load_test.rs` — daemon-mode throughput (Phase 3.4)
- `docs/proof/chaos-suite-results-v1.2.0.md` — qualitative correctness under failure
- `crates/sbo3l-core/tests/proptest_invariants.rs` — property invariants
