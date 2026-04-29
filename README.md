# Mandate

> Don't give your agent a wallet. Give it a mandate.

**Mandate** is a local policy, budget, receipt and audit firewall that decides whether an autonomous AI agent may execute an onchain or payment action. The agent never holds a private key. Mandate decides, signs and audits.

This repository was implemented during **ETHGlobal Open Agents 2026**. Planning and specification artifacts under [`docs/spec/`](docs/spec/) were copied from a pre-hackathon planning repository (`agent-vault-os`) and are clearly labelled — they are not prior product code.

---

## Status

**Implemented and reproducible from a fresh clone.** `cargo test --workspace --all-targets` runs **292/292 green**. `bash demo-scripts/run-openagents-final.sh` runs all **13 demo gates** end-to-end clean in ~5 seconds. `bash demo-scripts/run-production-shaped-mock.sh` exercises the production-shaped surface (HTTP `Idempotency-Key` four-case matrix + `mandate doctor` + mock-KMS CLI lifecycle + active-policy lifecycle + **audit checkpoint create/verify with mock anchoring** + audit-bundle round-trip + the operator-evidence transcript consumed by the operator console + the Passport capsule emit/verify pair) end-to-end with **Tally: 26 real, 0 mock, 1 skipped** (only the optional `--include-final-demo` flag remains on the SKIPPED list — every A-side backlog item has merged). See [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) for the current snapshot.

## Three commands a judge needs

```bash
# 1. Run the full vertical demo (legit allow, prompt-injection deny, audit tamper detection, no-key proof, signed transcript).
bash demo-scripts/run-openagents-final.sh

# 2. Render the one-screen, judge-readable proof viewer (static HTML, no JS, no network).
python3 trust-badge/build.py
# then open trust-badge/index.html

# 3. Re-verify the proof viewer's regression test (stdlib-only).
python3 trust-badge/test_build.py
```

For a verifiable, offline-portable proof of a single decision, see [`docs/cli/audit-bundle.md`](docs/cli/audit-bundle.md): `mandate audit export` packages a signed receipt + audit chain prefix + signer keys into one JSON file; `mandate audit verify-bundle` re-derives every claim from that file alone.

## What this is

- A Rust workspace implementing the **Mandate** spending-mandate firewall for AI agents.
- A real research-agent demo harness that proves legitimate vs prompt-injection scenarios across the same boundary.
- Sponsor-facing adapters for **KeeperHub**, **ENS** and **Uniswap** with explicit `local_mock()` / `live()` constructors.
- Signed Ed25519 policy receipts and a hash-chained, tamper-evident audit log persisted in SQLite.
- A verifiable audit-bundle export and a static, offline trust-badge proof viewer.

## How Mandate plugs into KeeperHub

> *KeeperHub executes. Mandate proves the execution was authorised.*

Mandate sits **in front of** KeeperHub as the policy / budget / signing / audit boundary. Allow receipts flow into `KeeperHubExecutor::execute()`; Deny receipts are refused before any sponsor call. Five concrete integration paths the KeeperHub team could merge or build on — `mandate_*` upstream-proof envelope fields (IP-1), submission JSON Schema (IP-2), `keeperhub.lookup_execution` MCP tool (IP-3), standalone `mandate-keeperhub-adapter` crate (IP-4), Passport capsule URI on the execution row (IP-5) — are catalogued in [`docs/keeperhub-integration-paths.md`](docs/keeperhub-integration-paths.md). Each is independently small, independently reviewable, and pointed at the place in this repo where the corresponding work lives.

The demo today always constructs `KeeperHubExecutor::local_mock()` (clearly disclosed); the live shape is documented end-to-end in [`docs/keeperhub-live-spike.md`](docs/keeperhub-live-spike.md) including the eight open questions for the KeeperHub team, the offline-CI test strategy, and the file-by-file shopping list for the live PR (~250 lines of Rust). On the MCP front the IP-3 **Mandate side is implemented on `main`** — `mandate-mcp` (PR #46) exposes a stdio JSON-RPC `mandate.audit_lookup` tool symmetric to KeeperHub's proposed `keeperhub.lookup_execution`, so an MCP-aware auditor can cross-verify a KeeperHub `executionId` against a Mandate audit bundle in two tool calls; judge-facing walk-through in [`docs/mcp-integration-guide.md`](docs/mcp-integration-guide.md). The KeeperHub side of the IP-3 pair remains target.

## What is real vs mocked in this build

End-to-end real: APRP wire format, JCS canonical request hashing, JSON Schema validation, policy engine, multi-scope budget tracker, persistent SQLite-backed APRP nonce-replay protection, signed receipts / decision tokens / audit events, hash-chained audit log with structural and strict-hash verifiers, audit-bundle export and verify, no-key proof, trust-badge render.

Mocked / offline (clearly labelled in demo output and in [`SUBMISSION_FORM_DRAFT.md`](SUBMISSION_FORM_DRAFT.md)): ENS resolution uses `OfflineEnsResolver` with a fixture file; `KeeperHubExecutor::local_mock()` and `UniswapExecutor::local_mock()` are constructed by the demo. There is no `MANDATE_*_LIVE` env-var feature flag in this build — switching to a live sponsor backend is a single constructor-call change. The dev signing seeds in `AppState::new` are deterministic public constants (clearly marked `⚠ DEV ONLY ⚠`); production deployments inject real signers via `AppState::with_signers`.

## How to run from a fresh clone

You need a Rust toolchain (workspace MSRV `1.80`) and Python 3 for the schema validators and the trust-badge build.

```bash
git clone https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026.git
cd mandate-ethglobal-openagents-2026
bash demo-scripts/run-openagents-final.sh
python3 trust-badge/build.py
```

The 13 demo gates cover: schema strictness, locked golden APRP `request_hash`, audit-chain structural and strict-hash verify, policy/budget/storage/server tests, the research-agent harness (legit + prompt-injection), the ENS identity proof, the KeeperHub guarded-execution path, the Uniswap guarded-swap path, the standalone red-team prompt-injection gate, the audit-chain tamper-detection gate, the agent no-key boundary proof, and a deterministic transcript artifact written to [`demo-scripts/artifacts/latest-demo-summary.json`](demo-scripts/artifacts/) which is the input the trust-badge consumes.

## Submission documents

- [`SUBMISSION_FORM_DRAFT.md`](SUBMISSION_FORM_DRAFT.md) — copy-paste-ready ETHGlobal form text.
- [`SUBMISSION_NOTES.md`](SUBMISSION_NOTES.md) — judge narrative and partner-prize positioning.
- [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) — current implementation snapshot.
- [`FEEDBACK.md`](FEEDBACK.md) — partner-sponsor feedback (KeeperHub, ENS, Uniswap).
- [`AI_USAGE.md`](AI_USAGE.md) — AI-tooling disclosure.
- [`demo-scripts/demo-video-script.md`](demo-scripts/demo-video-script.md) — 3:30 demo-video script.
- [`docs/cli/audit-bundle.md`](docs/cli/audit-bundle.md) — audit-bundle export / verify reference.
- [`trust-badge/README.md`](trust-badge/README.md) — trust-badge proof-viewer reference.
- [`FINAL_REVIEW.md`](FINAL_REVIEW.md) — historical readiness audit (kept for the audit trail).

## Repository layout

```
crates/         Rust workspace crates (implementation)
demo-agents/    Real agent harness (research-agent)
demo-scripts/   Demo runners (final + per-sponsor + red-team)
trust-badge/    Static, offline proof-viewer (build.py + test_build.py)
schemas/        JSON Schema 2020-12 contracts (live)
test-corpus/    Golden + adversarial fixtures
migrations/     SQLite schema migrations
docs/api/       OpenAPI 3.1 contract
docs/cli/       CLI reference (audit-bundle export / verify)
docs/spec/      Pre-hackathon planning artifacts (reference only)
.github/        CI workflows
```

## License

[MIT](LICENSE)
