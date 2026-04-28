# Implementation Status

Current snapshot for the ETHGlobal Open Agents 2026 submission of **Mandate**.

**Last updated:** 2026-04-29
**Branch:** `main` (HEAD `f789db8` — post-B2.v2)
**Phase:** submission. `main` is implemented, reproducible, and ready to submit.
**Open implementation PRs:** **1** — A-side [#38](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/38) `fix: self-review truthfulness pass` (DIRTY against current `main`; A-side author rebases). The B5 PR for this snapshot is open separately and does not block submission.
**CI on `main`:** ✅ green (`Rust check` + `Validate JSON schemas / OpenAPI` + trust-badge regression test).
**Blockers:** none.

For the **B5 final audit (current snapshot)** see [`FINAL_REVIEW_B5.md`](FINAL_REVIEW_B5.md).
For the **historical PR-by-PR audit trail** (frozen at `f52596c`, pre PR #11+) see [`FINAL_REVIEW.md`](FINAL_REVIEW.md).

## Verification summary

| Command | Result |
|---|---|
| `cargo fmt --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ no warnings |
| `cargo test --workspace --all-targets` | ✅ **218 / 218 pass** (0 fail, 0 ignored) |
| `python3 scripts/validate_schemas.py` | ✅ (6 schemas + 4 corpus fixtures) |
| `python3 scripts/validate_openapi.py` | ✅ (`docs/api/openapi.json` valid) |
| `bash demo-scripts/run-openagents-final.sh` | ✅ all **13 gates** green incl. audit-chain tamper detection and agent no-key proof (~5 seconds end-to-end) |
| `bash demo-scripts/run-production-shaped-mock.sh` | ✅ **Tally: 24 real, 0 mock, 1 skipped** — PSM-A1.9 mock-KMS lifecycle + PSM-A2 four-case Idempotency-Key matrix + PSM-A3 active-policy lifecycle + **PSM-A4 audit checkpoint create/verify with mock anchoring** + PSM-A5 doctor all walked end-to-end, plus step 12 emits the `mandate-operator-evidence-v1` transcript consumed by the operator console; only the optional `--include-final-demo` flag remains on the SKIPPED list |
| `python3 trust-badge/build.py` | ✅ writes `trust-badge/index.html` (self-contained, no JS, no fetch) |
| `python3 trust-badge/test_build.py` | ✅ 31 stdlib assertions on the rendered HTML |
| `python3 operator-console/build.py` | ✅ writes `operator-console/index.html` (self-contained, no JS, no fetch) — renders the `mandate-demo-summary-v1` transcript plus the `mandate-operator-evidence-v1` evidence transcript, with one real panel per merged A-side surface |
| `python3 operator-console/test_build.py` | ✅ 83 stdlib assertions (B2.v2 real-evidence panels — PSM-A2 + PSM-A5 + PSM-A1.9 + PSM-A3 + PSM-A4 — render values pulled directly from the evidence fixture; explicit negative assertions verify none of the five PSM-A* rows ever appear inside a `pill blocked` or `pill pending`) |
| `python3 demo-fixtures/test_fixtures.py` | ✅ 4 mock fixtures clean + url-allowlist self-test |

## What is implemented

Full Open Agents vertical:

- Rust workspace (8 crates + research-agent demo binary).
- `mandate` CLI: `aprp validate|hash|run-corpus`, `schema`, `verify-audit`, `audit export`, `audit verify-bundle`.
- APRP v1 wire format with `serde(deny_unknown_fields)` end-to-end + JCS canonical request hashing (golden hash `c0bd2fab…` locked in test).
- Strict JSON Schema validation (embedded, local refs, no network).
- Ed25519 dev signer (deterministic seed, public, demo-only; production path via `AppState::with_signers`).
- Policy receipt v1, decision token v1, audit event v1 — all sign + verify + schema-validated.
- Hash-chained audit log with `prev_event_hash` linkage, SQLite-backed storage, structural and strict-hash verifiers.
- Verifiable audit bundle (`mandate.audit_bundle.v1`) with both JSONL-chain and DB-backed export paths. The DB-backed exporter pre-flights signature and chain integrity before writing the bundle file.
- Tiny Rego-compatible expression evaluator + `decide()` + canonical policy hash.
- Multi-scope budget tracker (`per_tx`, `daily`, `monthly`, `per_provider`).
- HTTP API: `POST /v1/payment-requests` (full pipeline: schema → request_hash → **persistent SQLite-backed nonce replay gate** → policy → budget → audit → signed receipt) + `GET /v1/health`.
- Persistent APRP nonce-replay store backed by SQLite (`nonce_replay` table, migration V002) accessed via `Storage::nonce_try_claim`. Atomic INSERT-or-fail dedup; survives daemon restart when `Storage::open(path)` is used.
- Real research-agent harness (`legit-x402`, `prompt-injection`) using an in-memory daemon.
- ENS identity adapter (offline fixture resolver + policy_hash verification, trait-backed).
- KeeperHub guarded-execution adapter (`local_mock` + `live` constructor pair; demo uses `local_mock`).
- Uniswap guarded-swap adapter (token allowlist, max notional, max slippage, quote freshness, treasury recipient) + `UniswapExecutor::local_mock()`.
- Sponsor demo scripts: `ens-agent-identity.sh`, `keeperhub-guarded-execution.sh`, `uniswap-guarded-swap.sh`.
- Standalone red-team gate: `demo-scripts/red-team/prompt-injection.sh` (`D-RT-PI-01..03`).
- Reset hook: `demo-scripts/reset.sh`.
- Final demo runner: `bash demo-scripts/run-openagents-final.sh` — single command, **13 gates**, ~5 seconds. Includes: schema gate, locked golden hash, audit-chain structural + strict verify, live `cargo test` of policy/budget/storage/server, real research-agent harness, ENS identity proof, KeeperHub guarded execution, Uniswap guarded swap, red-team prompt-injection gate, audit-chain tamper detection, agent no-key boundary proof, deterministic transcript artifact.
- Production-shaped mock runner: `bash demo-scripts/run-production-shaped-mock.sh` — exercises the operator surface (doctor, mock KMS CLI, active-policy lifecycle, persistent-SQLite allow + deny, audit-bundle export, **audit checkpoint create + verify with mock anchoring**) end-to-end and emits a `mandate-operator-evidence-v1` transcript for the operator console. Tally **24 real / 0 mock / 1 skipped** post-B2.v2; the only SKIPPED item is the optional `--include-final-demo` flag — every A-side backlog row has merged.
- Audit checkpoints + mock anchoring: `mandate audit checkpoint {create, verify}` (PSM-A4) backed by SQLite migration V007 (`audit_checkpoints` table). This is **mock anchoring**, NOT real onchain anchoring — the `mock_anchor_ref` is a deterministic local id, never broadcast and never attested by any chain. Every CLI line carries a `mock-anchor:` prefix; `mock_anchor: true` is in every JSON artifact; the verifier refuses any artifact with `mock_anchor: false`. Documented in `docs/cli/audit-checkpoint.md`.
- Active-policy lifecycle: `mandate policy {validate, current, activate, diff}` (PSM-A3) backed by SQLite migration V006 (`active_policy` table with partial UNIQUE singleton index). Local lifecycle, not remote governance — `docs/cli/policy.md` documents the scope honestly.
- Static, offline trust-badge proof viewer (`trust-badge/build.py`, stdlib Python) + stdlib regression test (`trust-badge/test_build.py`). No JS, no fetch, works from `file://`.

## Surfaces a judge can verify

- Signed Ed25519 policy receipts (`receipt.signature`).
- Canonical `request_hash` over the JCS-canonicalised APRP body.
- `policy_hash` over the canonicalised active policy.
- `audit_event_id` linking each receipt to a specific position in the audit chain.
- Hash-chained audit log with structural verify (skip-hash) and strict-hash verify modes.
- Audit-chain tamper detection — flip one byte and strict-hash verify rejects.
- Persistent SQLite nonce-replay table — replay returns HTTP 409 `protocol.nonce_replay` before any side effects, persists across daemon restart.
- Verifiable audit bundle (`mandate audit export` + `mandate audit verify-bundle`), DB-backed export path included.
- Agent no-key proof gate — asserts zero signing references, zero key-material fixtures, no signing-related cargo deps in the agent crate.
- Deterministic transcript artifact written to `demo-scripts/artifacts/latest-demo-summary.json` (consumed by the trust-badge).
- Static, offline trust-badge / proof viewer rendered straight from that transcript.
- **HTTP `Idempotency-Key` safe-retry** (PSM-A2) — persistent SQLite-backed dedup; same-key/same-body → byte-identical cached response, no second audit row; same-key/different-body → 409 `protocol.idempotency_conflict`; different-key + same-nonce → 409 `protocol.nonce_replay` (defense in depth). Migration V004.
- **`mandate doctor`** (PSM-A5) — operator readiness summary. Reports per-feature `ok`/`skip`/`warn`/`fail`; refuses to open a missing DB (no write-on-typo); falls through to real `storage_open` errors on permission/IO failures (not "does not exist"). Stable `mandate.doctor.v1` JSON envelope.
- **Mock KMS CLI surface + persistence** (PSM-A1.9) — `mandate key {init,list,rotate} --mock`; persistent `mock_kms_keys` SQLite table (V005). Every CLI line `mock-kms:`-prefixed; `--mock` mandatory; rotate refuses on mismatched root-seed; current-version lookup propagates real DB errors. **Mock — not production-grade.**
- **Production-shaped mock runner** (`demo-scripts/run-production-shaped-mock.sh`) — exercises the full PSM-A2 four-case matrix, PSM-A5 doctor, PSM-A1.9 init/list/rotate lifecycle, PSM-A3 active-policy lifecycle, PSM-A4 audit-checkpoint create/verify with mock anchoring, and the operator-evidence transcript emission (step 12) end-to-end against real binaries; `Tally: 24 real, 0 mock, 1 skipped`.
- **Static, offline operator console** (`operator-console/build.py`) — sister surface to the trust-badge: vertical timeline + multi-panel grid + **five real-evidence panels** (PSM-A2 four-case Idempotency-Key matrix, PSM-A5 `mandate doctor --json` grouped report, PSM-A1.9 mock KMS keyring table, PSM-A3 active-policy lifecycle, PSM-A4 audit checkpoint with explicit `mock anchoring, NOT onchain` pill) rendered straight from the production-shaped runner's `mandate-operator-evidence-v1` transcript. Zero pending pills, zero blocked pills. When the evidence transcript is missing/malformed/wrong-schema each panel renders an honest "evidence not gathered" placeholder — never a fake-OK.
- **Production-shaped mock fixtures** (`demo-fixtures/mock-*.json`) — ENS multi-agent registry, KeeperHub workflow envelopes (success/conflict/refused/lookup), Uniswap quote catalogue (happy/multi-violation rug/recipient-allowlist), mock-KMS public keyring metadata. Plus stdlib validator (`test_fixtures.py`) with `urlparse`-based safe-host allowlist + URL-bypass self-test.
- **Per-fixture production-transition guides** (`demo-fixtures/mock-*.md`) and a single **`docs/production-transition-checklist.md`** — every surface (ENS / KeeperHub / Uniswap / Signer-KMS-HSM) has env vars / endpoints / credentials / code-change steps / verification / truthfulness invariants spelled out.

## Pending / stretch (not blocking submission)

- Live KeeperHub backend (one-constructor switch via `KeeperHubExecutor::live()`; demo uses `local_mock()`). Wire-format design notes in `docs/keeperhub-live-spike.md`.
- Live ENS testnet resolver (offline fixture today; trait already abstracts the backend).
- Live Uniswap quote backend (`UniswapExecutor::live()` is intentionally stubbed; demo uses `local_mock()`).
- Production KMS / HSM signer (`MANDATE_SIGNER_BACKEND` selector + per-role `MANDATE_*_SIGNER_KEY_ID` env vars). The dev `DevSigner` and the persistent mock `MockKmsSigner` are both `⚠ DEV ONLY ⚠`; production injects real signers via `AppState::with_signers`.
- Recorded demo video (3:30 cut). Script committed in `demo-scripts/demo-video-script.md`.
- Pruned / Merkle-proof variants of the audit bundle, and optional embedded original APRP. Tracked in `docs/cli/audit-bundle.md`.
- Soft-cap warning emission in receipts (`Budget.soft_cap_usd` parsed but not enforced).
- B5 — final submission package wiring (transcripts/, recorded video URL, etc).

## Blockers

**None.**
