# Submission Notes — ETHGlobal Open Agents 2026

## Project

- **Public brand:** SBO3L
- **Tagline:** Spending mandates for autonomous agents.
- **Pitch line:** Don't give your agent a wallet. Give it a mandate.
- **Technical namespace:** `sbo3l` (crates, daemon, CLI, schema host).
- **Event:** ETHGlobal Open Agents 2026.

## What was built during the hackathon

All implementation code in this repository:

- Rust workspace (`crates/sbo3l-*`).
- `sbo3l` CLI: `aprp validate|hash|run-corpus`, `schema`, `verify-audit`, `audit export`, `audit verify-bundle`.
- Strict APRP schema validation, decision token signing, policy receipts.
- Policy engine (hackathon-grade custom expression evaluator over the locked rule grammar in `sbo3l-policy/src/expr.rs`; Rego via `regorus` is the production target per `docs/spec/17_interface_contracts.md`) + multi-scope budget checks + fail-closed agent gate (unknown / paused / revoked / `emergency.paused_agents`).
- SQLite-backed storage with hash-chained audit log + persistent SQLite-backed APRP nonce-replay table (`nonce_replay`, migration V002).
- Verifiable audit-bundle export and offline verifier (`sbo3l.audit_bundle.v1`), including a DB-backed export path that pre-flights chain integrity and signatures before writing the bundle.
- Real research-agent harness with `legit-x402` and `prompt-injection` scenarios.
- ENS identity proof adapter (resolves agent → SBO3L endpoint + policy hash + audit root).
- KeeperHub guarded-execution adapter (`sbo3l-execution::keeperhub`) — `local_mock()` + `live()` constructor pair.
- Uniswap guarded-swap adapter (`sbo3l-execution::uniswap`): swap-policy guard (token allowlist, max notional, max slippage, quote freshness, treasury recipient) + `UniswapExecutor::local_mock()`.
- Sponsor demo scripts: `ens-agent-identity.sh`, `keeperhub-guarded-execution.sh`, `uniswap-guarded-swap.sh`.
- Standalone red-team gate: `demo-scripts/red-team/prompt-injection.sh`.
- `demo-scripts/run-openagents-final.sh` — single-command demo runner with audit-chain tamper detection, agent no-key boundary proof, and a deterministic transcript artifact.
- Static, offline trust-badge proof viewer (`trust-badge/build.py`) + stdlib regression test (`trust-badge/test_build.py`).
- Static, offline operator console (`operator-console/build.py`) — sister surface to the trust-badge with **five real-evidence panels** (PSM-A2 four-case Idempotency-Key matrix / PSM-A5 `sbo3l doctor --json` grouped report / PSM-A1.9 mock-KMS keyring table / PSM-A3 active-policy lifecycle / PSM-A4 audit checkpoint with explicit `mock anchoring, NOT onchain` pill) rendered straight from the production-shaped runner's `sbo3l-operator-evidence-v1` transcript. Zero pending pills, zero blocked pills.
- HTTP `Idempotency-Key` safe-retry (PSM-A2): persistent SQLite-backed dedup with the four-case behaviour matrix exercised by `demo-scripts/run-production-shaped-mock.sh` step 7 against a real `sbo3l-server` daemon.
- `sbo3l doctor` (PSM-A5): operator readiness summary; refuses to open a missing DB (no write-on-typo); per-feature `ok`/`skip`/`warn`/`fail`; stable JSON envelope.
- Mock KMS CLI surface + persistence (PSM-A1.9): `sbo3l key {init,list,rotate} --mock` with `mock_kms_keys` SQLite table (V005). Mock — not production-grade.
- Production-shaped mock fixtures (`demo-fixtures/mock-*.json`) + per-fixture transition guides (`demo-fixtures/mock-*.md`) + single `docs/production-transition-checklist.md`.
- `demo-scripts/demo-video-script.md` — 3:30 video script with recording checklist.
- Active-policy lifecycle: `sbo3l policy {validate, current, activate, diff}` (PSM-A3) backed by SQLite migration V006 (`active_policy` table with a non-NULL-keyed partial UNIQUE singleton index — the singleton invariant is enforced by the database itself, not just the CLI). This is **local** lifecycle, not remote governance — there is no on-chain anchor, no consensus, no signing on activation. Documented in `docs/cli/policy.md`.
- Audit checkpoints + mock anchoring: `sbo3l audit checkpoint {create, verify}` (PSM-A4) backed by SQLite migration V007 (`audit_checkpoints` table). **Mock anchoring**, NOT real onchain anchoring — `mock_anchor_ref` is a deterministic local id, never broadcast. Every CLI line carries a `mock-anchor:` prefix and every JSON artifact carries `mock_anchor: true`. Documented in `docs/cli/audit-checkpoint.md`.
- **SBO3L Passport capsule schema + verifier + CLI** (`schemas/sbo3l.passport_capsule.v1.json`, `crates/sbo3l-cli/src/passport.rs`, Passport P1.1 + P2.1) — `sbo3l passport run` drives the existing `POST /v1/payment-requests` pipeline in-process, builds a checkpoint, and self-verifies before atomic write; `sbo3l passport verify` runs the structural verifier against any capsule.
- **MCP-callable SBO3L gateway** (`crates/sbo3l-mcp/`, Passport P3.1) — stdio JSON-RPC 2.0 server with `sbo3l.validate_aprp` / `sbo3l.decide` / `sbo3l.run_guarded_execution` / `sbo3l.verify_capsule` / `sbo3l.audit_lookup` (the IP-3 sister tool to KeeperHub's proposed `keeperhub.lookup_execution`). Sponsor demo: `bash demo-scripts/sponsors/mcp-passport.sh`. Judge-facing walk-through: `docs/mcp-integration-guide.md` (Passport P3.2).
- **KeeperHub `sbo3l_*` envelope helper** (`sbo3l_keeperhub_adapter::build_envelope`, Passport P5.1, IP-1) — composes `sbo3l_request_hash` / `sbo3l_policy_hash` / `sbo3l_receipt_signature` / `sbo3l_audit_event_id` directly from an existing `PolicyReceipt`. Helper shipped; live KeeperHub HTTP wiring still gated on KeeperHub publishing a stable submission/result schema (open questions in `docs/keeperhub-live-spike.md`).
- **Standalone `sbo3l-keeperhub-adapter` workspace crate** (IP-4) — `KeeperHubExecutor`, `GuardedExecutor` re-exports, `build_envelope`, README, changelog and runnable example live under `crates/sbo3l-keeperhub-adapter/`; `sbo3l-execution` re-exports it for back-compat. Crates.io publication remains a target, not a shipped claim.
- **GitHub Pages public proof site** (`.github/workflows/pages.yml` + `site/index.html`, Passport P7.1) — deploys from `main` to `https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/`. Plain HTML, no JS, no client-side network calls; same offline-only ground rules as trust-badge / operator-console.
- CI: fmt, clippy, tests (310 passing), schema validation, OpenAPI validation, trust-badge regression test, operator-console regression test, demo-fixtures validator.
- Production-shaped mock runner: `bash demo-scripts/run-production-shaped-mock.sh` walks the operator surface end-to-end (doctor, mock KMS CLI, full PSM-A3 policy lifecycle, **PSM-A4 audit checkpoint create/verify with mock anchoring**, persistent SQLite allow + deny, audit-bundle export, Passport P2.1 capsule emit/verify, plus the `sbo3l-operator-evidence-v1` transcript consumed by the operator console). Tally: **26 real / 0 mock / 1 skipped** — every A-side backlog row has merged; only the optional `--include-final-demo` flag remains on the SKIPPED list.
- Self-audit artefact: [`SECURITY_NOTES.md`](SECURITY_NOTES.md) documents the known scope limitations a production deployment would need to address (daemon authentication, production signer wiring, budget tracker persistence, idempotency in-flight semantics, Passport verifier scope). Honest disclosure, not a roadmap promise.

## What was reused as planning material

The pre-hackathon planning repository (`agent-vault-os`) is included as planning material in [`docs/spec/`](docs/spec/) and clearly labelled. It contains:

- Strategic vision, threat model, architecture and policy model docs.
- JSON Schemas, OpenAPI draft, golden/adversarial test corpus, demo-agent harness contract.

These are documentation/specifications, not prior product code. See [`AI_USAGE.md`](AI_USAGE.md).

## Targeted partner prizes (max 3)

1. **KeeperHub — Best Use of KeeperHub.** *KeeperHub executes. SBO3L proves the execution was authorised.* SBO3L is the pre-execution policy / signing / audit layer; KeeperHub is the execution substrate. Approved actions are routed to KeeperHub via `KeeperHubExecutor::execute()`; denied actions never reach it. Five concrete integration paths the KeeperHub team could merge or build on are catalogued in [`docs/keeperhub-integration-paths.md`](docs/keeperhub-integration-paths.md) (IP-1 envelope fields, IP-2 submission JSON Schema, IP-3 MCP lookup tool, IP-4 standalone adapter crate, IP-5 Passport capsule URI on the execution row). Demo uses `KeeperHubExecutor::local_mock()`; the live `KeeperHubExecutor::live()` shape is documented end-to-end in [`docs/keeperhub-live-spike.md`](docs/keeperhub-live-spike.md) and is a single-constructor-body change once a stable submission schema and credentials are available.
2. **ENS — Best Integration for AI Agents.** ENS records resolve `sbo3l:agent_id`, `sbo3l:endpoint`, `sbo3l:policy_hash`, `sbo3l:audit_root`. ENS gates discovery and verifies the active policy hash matches. Demo uses an offline resolver fixture; live testnet resolution is a single trait-backed adapter swap.
3. **Uniswap — Best Uniswap API Integration (stretch).** SBO3L is not a trading bot. The guarded-swap demo enforces token allowlists, slippage caps, max notional, treasury recipient and quote freshness before any agent-initiated swap is signed. Demo uses `UniswapExecutor::local_mock()`; the swap-policy guard and the deny path are real.

## What is live vs mocked

- APRP / policy / receipts / audit chain / persistent nonce-replay / audit-bundle export & verify / no-key proof / trust-badge render — **live, end-to-end, deterministic**.
- ENS resolution — offline fixture (`OfflineEnsResolver` reads `demo-fixtures/ens-records.json`). The `EnsResolver` trait abstraction is real; live resolver is a future swap.
- KeeperHub adapter — local mock (`KeeperHubExecutor::local_mock()`). The adapter boundary is real; the live constructor exists but is not exercised in the demo.
- Uniswap adapter — local mock (`UniswapExecutor::local_mock()`). The swap-policy guard runs before any executor call. `UniswapExecutor::live()` is intentionally stubbed and returns `BackendOffline`.
- Signing seeds — deterministic dev seeds in `AppState::new` are public and demo-only. Production deployments inject real signers via `AppState::with_signers`. We do not claim production readiness for TEE/HSM in this build.
- There is **no** `MANDATE_*_LIVE` environment variable feature flag in this build.

## Known limitations (hackathon scope)

- `Budget.soft_cap_usd` is parsed and validated against the schema, but not yet enforced by `BudgetTracker`. A production deployment (per `docs/spec/17_interface_contracts.md`) emits a soft-cap warning into the receipt; this hackathon build only enforces the hard cap (`cap_usd`). See comment in `crates/sbo3l-policy/src/model.rs`.
- Signing seeds in `AppState::new` are public constants in this open repo. Acceptable for the demo and CI; production callers must inject real signers via `AppState::with_signers`.
- APRP nonce replay protection is enforced (`POST /v1/payment-requests` returns HTTP 409 `protocol.nonce_replay` on a reused nonce, before any policy/budget/audit/signing side effects). The dedup is backed by a persistent SQLite table (`nonce_replay`, migration V002), so the protection survives a daemon restart whenever persistent storage is configured (`Storage::open(path)`). The hackathon demo defaults to `Storage::open_in_memory()`, which is also a real SQLite database — nonces persist for the full lifetime of the daemon process and are dropped only when the in-memory database is destroyed (i.e. when the demo process exits).
- HTTP `Idempotency-Key` safe-retry is implemented for `POST /v1/payment-requests` (PSM-A2 / migration V004 `idempotency_keys`). When the header is present, SBO3L caches the 200 response envelope and replays it byte-identically on retry — no duplicate policy / budget / audit / signing side effects. Same key + different canonical body returns 409 `protocol.idempotency_conflict`. The cache survives daemon restart against the same SQLite file. Only 200 responses are cached; 4xx/5xx flow through fresh on retry. The nonce gate runs after the idempotency lookup, so an attacker cannot bypass it by attaching a fresh key to a captured nonce — different key + same nonce still returns 409 `protocol.nonce_replay`. TTL eviction is not yet implemented. See `docs/cli/idempotency.md` for the full behaviour matrix.
- The audit-bundle v1 carries the full chain prefix from genesis and the canonical `request_hash` only (not the original APRP body). Pruned / Merkle-proof variants and embedded APRP are tracked as future work — see `docs/cli/audit-bundle.md`.

## Demo

Three commands a judge needs:

```bash
bash demo-scripts/run-openagents-final.sh   # 13-gate vertical demo
python3 trust-badge/build.py                # render the one-screen proof viewer
python3 trust-badge/test_build.py           # stdlib regression test for the viewer
```

The demo proves, in order:

1. Strict APRP schema gate (golden + adversarial).
2. Locked golden APRP `request_hash`.
3. Audit chain — structural verify + strict-hash verify of the seed fixture.
4. Policy + budget + storage + server unit tests pass live.
5. Real research-agent harness — legit-x402 → Allow + signed receipt; prompt-injection → Deny.
6. ENS sponsor identity proof — published policy_hash matches active.
7. KeeperHub sponsor — approved request routes to mock executor; denied request never reaches the sponsor.
8. Uniswap sponsor — bounded swap allowed; rug-token attacker quote denied at swap-policy guard AND SBO3L boundary.
9. Red-team prompt-injection standalone gate.
10. Audit-chain tamper detection — flip a byte and confirm strict-hash verify rejects.
11. Agent no-key boundary proof — zero `SigningKey` / `signing_key` references in the agent crate, zero key-material fixtures, no signing-related cargo deps.
12. Demo transcript artifact — deterministic JSON written to `demo-scripts/artifacts/`, consumed by `trust-badge/build.py`.

For a verifiable single-decision proof package, see `docs/cli/audit-bundle.md`: `sbo3l audit export` packages a signed receipt + audit chain prefix + signer keys into one JSON file; `sbo3l audit verify-bundle` re-derives every claim from that file alone.

## Demo video

Target length 3:30, hard stop 3:50. Real human voice narration. Video script in [`demo-scripts/demo-video-script.md`](demo-scripts/demo-video-script.md).

## Judging criteria mapping

| Criterion | SBO3L angle |
|---|---|
| Technicality | Policy engine + signed receipts + persistent nonce-replay store + hash-chained audit + verifiable audit bundle + sponsor adapters + agent harness + offline trust-badge. |
| Originality | Spending mandate replaces agent wallet — agent never holds key. |
| Practicality | Local daemon + CLI + runnable demo + offline proof bundle; useful for agent builders today. |
| Usability | One-command final demo, one-command proof viewer, readable receipts, clear deny codes. |
| WOW factor | Prompt-injection visibly tries to spend, SBO3L denies pre-execution and proves why on a single screen. |
