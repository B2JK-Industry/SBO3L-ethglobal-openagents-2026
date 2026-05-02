# SBO3L fuzz targets (R13 P3)

> 5 cargo-fuzz harnesses covering the parsers + verifiers reachable from untrusted input. Default nightly run: 1h per target. OSS-Fuzz integration: see `oss-fuzz/`.

## Targets

| Target | What it fuzzes | Reachable from |
|---|---|---|
| `aprp_parser` | `serde_json::from_*` → `PaymentRequest` | HTTP request body |
| `capsule_deserialize` | `verify_capsule(&Value)` | `/proof` page WASM, `sbo3l verify-capsule` CLI |
| `policy_yaml` | `Policy::parse_yaml` + `Policy::parse_json` | Daemon startup; admin policy upload |
| `audit_event` | `AuditEvent` + `SignedAuditEvent` parse + `canonical_hash` | Audit chain export/import |
| `canonical_json` | JCS canonicalizer + `request_hash` | Every sign/verify operation |

## Setup

```bash
# Install cargo-fuzz (one-time)
cargo install cargo-fuzz

# List available targets
cargo fuzz list

# Run a target for 10 minutes
cargo fuzz run aprp_parser -- -max_total_time=600

# Run for 10M iterations
cargo fuzz run capsule_deserialize -- -runs=10000000

# Continuous run (Ctrl-C to stop)
cargo fuzz run policy_yaml
```

## Corpus

cargo-fuzz manages corpus files under `fuzz/corpus/<target>/` (gitignored). Seed corpus from existing test fixtures:

```bash
# Seed APRP corpus from golden fixtures
mkdir -p fuzz/corpus/aprp_parser
cp test-corpus/aprp/*.json fuzz/corpus/aprp_parser/

# Seed capsule corpus
mkdir -p fuzz/corpus/capsule_deserialize
cp test-corpus/passport/*.json fuzz/corpus/capsule_deserialize/

# Seed policy corpus
mkdir -p fuzz/corpus/policy_yaml
cp test-corpus/policy/*.yaml fuzz/corpus/policy_yaml/
cp test-corpus/policy/*.json fuzz/corpus/policy_yaml/
```

## CI integration

`.github/workflows/fuzz.yml` runs each target for 10 minutes nightly (50 min total). On crash:
- The crash artifact is uploaded.
- A GitHub issue is opened with the reproducer + target name.
- The CI job fails (red).

## OSS-Fuzz

> **Status:** scaffolding shipped at `fuzz/oss-fuzz/`. Submission to `google/oss-fuzz` is a Daniel-side action (requires Google email + project owner approval). Estimate: 1 day from "go" to first builds.

## Reporting findings

If a fuzz target crashes locally, the crash file is at `fuzz/artifacts/<target>/crash-<hash>`. Report via:
1. `SECURITY.md` channels (private; do NOT commit the crash file to a public branch).
2. Include: target name, libfuzzer reproducer command, crash file SHA-256, severity self-assessment.

## See also

- [`SECURITY.md`](../SECURITY.md) — disclosure policy.
- `crates/sbo3l-core/tests/proptest_invariants.rs` — property-based tests (correctness invariants; complementary to fuzzing's "no panic" focus).
