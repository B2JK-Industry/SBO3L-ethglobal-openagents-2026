# ETHGlobal Open Agents 2026 — Submission Form Draft (SBO3L)

Copy-paste-ready text for the ETHGlobal Open Agents 2026 project submission form. Every field below maps to an ETHGlobal form field (or to a video / repo link). Fields with a `<TODO: …>` placeholder are the only ones that need to be filled in on submission day.

> One-sentence hook (use anywhere a hook is required):
>
> **Don't give your agent a wallet. Give it a mandate.**

---

## Project name

```
SBO3L
```

## Category

```
Infrastructure
```

## Emoji

```
🛂
```

(Passport-control emoji — visualises SBO3L as the customs gate every agent payment has to clear before it ever reaches a wallet or sponsor.)

## Short description (under 100 characters)

```
Spending mandates for AI agents. The agent never holds the key; SBO3L decides, signs and audits.
```

(98 characters including spaces — ETHGlobal's short-description field caps at 100.)

## Long description (judge-facing)

```
SBO3L is a local policy, budget, receipt and audit firewall that decides whether an autonomous AI agent may execute an onchain or payment action — so the agent never has to hold a private key.

A research-agent in our demo emits a payment request (an APRP — "Agent Payment Request Protocol") across the SBO3L boundary. SBO3L validates the request, evaluates a deterministic policy, enforces multi-scope budgets, rejects replayed nonces with HTTP 409 (backed by a persistent SQLite table so dedup survives restart), signs an Ed25519 policy receipt, appends a hash-chained audit event, and only then routes the action to a sponsor executor (KeeperHub or Uniswap in this demo). When the same agent is prompt-injected and forwards a hostile request, SBO3L denies before any signer or executor runs and the audit log captures the rejection. Tampering with one byte of an audit event is rejected by the strict-hash verifier.

Every decision can be packaged as a verifiable audit bundle: `sbo3l audit export` produces a single JSON file containing the signed receipt, the audit-chain prefix, and the signer keys; `sbo3l audit verify-bundle` re-derives every claim from that file alone. A static, offline proof viewer (`python3 trust-badge/build.py`) renders the most recent demo run into a single self-contained HTML page — no JS, no fetch, works directly from `file://`.

The whole flow is deterministic, runs offline, and reproduces from a fresh clone in ~10 seconds end-to-end with `bash demo-scripts/run-openagents-final.sh`. 377/377 tests pass, schemas validate, the demo's 13 gates are green end-to-end including audit-chain tamper-detection and the agent no-key boundary proof. A second runner — `bash demo-scripts/run-production-shaped-mock.sh` — exercises the production-shaped surface (HTTP `Idempotency-Key` four-case matrix + `sbo3l doctor` + mock-KMS CLI lifecycle + active-policy lifecycle + **audit checkpoint create/verify with mock anchoring** + audit-bundle round-trip + the operator-evidence transcript consumed by the operator console + the Passport capsule emit/verify pair) end-to-end with `Tally: 26 real, 0 mock, 1 skipped` — every A-side backlog row has merged; only the optional `--include-final-demo` flag remains on the SKIPPED list. An MCP-callable SBO3L gateway (`crates/sbo3l-mcp`, Passport P3.1) and a static GitHub Pages public proof URL (Passport P7.1) are both on `main`.

SBO3L is not a wallet, not a relayer, and not a trading bot. It is the pre-execution policy and signing boundary that lets autonomous agents transact without ever being trusted with a key.
```

## How it is made

```
SBO3L is a Rust workspace built during ETHGlobal Open Agents 2026 around four hard contracts: a strict APRP wire format with `serde(deny_unknown_fields)` end-to-end and a JCS-canonical request hash locked at `c0bd2fab…`; a deterministic policy engine evaluating a small Rego-compatible expression grammar over a hash-locked policy file; a multi-scope budget tracker (`per_tx`, `daily`, `monthly`, `per_provider`); and an Ed25519-signed, hash-chained audit log persisted in SQLite with a separate JSONL verifier offering both structural and strict-hash modes.

The HTTP boundary is `POST /v1/payment-requests`, served by axum. Each request runs through the same fail-closed pipeline: schema validation → canonical request hash → APRP nonce-replay gate (HTTP 409 + `protocol.nonce_replay`, before any state mutation, backed by an atomic INSERT into the persistent `nonce_replay` SQLite table from migration V002) → policy decision → budget commit (only on Allow) → audit append → signed policy receipt. Receipts and decision tokens are JCS-canonical JSON signed with Ed25519; audit events are linked by `prev_event_hash` and verifiable end-to-end with the `sbo3l verify-audit` CLI.

Beyond the per-request flow, the CLI exposes an audit-bundle path: `sbo3l audit export --db <sqlite> --receipt <json> --receipt-pubkey <hex> --audit-pubkey <hex> --out <bundle>` reads the chain prefix straight out of SQLite, pre-flights signature + chain integrity, and writes a self-contained `sbo3l.audit_bundle.v1` JSON file. `sbo3l audit verify-bundle --path <bundle>` re-derives every claim — receipt signature, audit-event signature, full-chain hash linkage, and a re-derived summary block — from the bundle alone.

A research-agent harness drives the boundary across two scenarios — a legitimate x402 purchase and a prompt-injection attack — by posting real APRP fixtures across the API and printing the daemon's signed response. The agent crate intentionally has zero signing dependencies; demo gate 12 verifies this by grepping for `SigningKey` / `signing_key` in `demo-agents/research-agent/` and asserting the count is zero. ENS, KeeperHub and Uniswap each show up as guarded executors behind a thin adapter trait so they can be swapped for live backends without touching the policy/signing core.

A small static proof viewer (`trust-badge/build.py`, stdlib-only Python) reads the demo runner's deterministic transcript artifact and renders a one-screen HTML summary — no JS, no fetch, no external resources — so a judge can see allow + deny side-by-side, no-key proof status, and the audit-tamper-detection result on a single page.
```

## Tech stack

```
Rust workspace (9 crates + research-agent demo binary):
  - sbo3l-core               APRP types, JCS canonical hashing, Ed25519 signer, receipts, decision tokens, audit events, audit-bundle codec.
  - sbo3l-policy             Policy model + Rego-compatible expression evaluator, decide(), multi-scope budget tracker.
  - sbo3l-storage            SQLite-backed audit log with hash-chain verifier (rusqlite + migrations); persistent nonce-replay table (V002); chain prefix slicing for audit-bundle export.
  - sbo3l-server             axum HTTP server, POST /v1/payment-requests pipeline, persistent SQLite-backed APRP nonce-replay gate.
  - sbo3l-execution          Guarded-executor adapters (KeeperHub, Uniswap) with explicit local_mock / live constructors.
  - sbo3l-identity           ENS resolver trait + offline fixture resolver + policy_hash drift check.
  - sbo3l-mcp                Functional MCP stdio JSON-RPC server (PR #46): sbo3l.validate_aprp / sbo3l.decide / sbo3l.run_guarded_execution / sbo3l.verify_capsule / sbo3l.audit_lookup / sbo3l.explain_denial.
  - sbo3l-keeperhub-adapter  Standalone publishable IP-4 adapter crate (PR #56): re-exports KeeperHubExecutor + GuardedExecutor + Sbo3lEnvelope + build_envelope; depends only on sbo3l-core.
  - sbo3l-cli                `sbo3l` CLI: aprp {validate, hash, run-corpus}, schema, verify-audit, audit {export, verify-bundle, checkpoint {create, verify}}, passport {verify, run, explain}, policy {validate, current, activate, diff}, key {init, list, rotate} --mock, doctor.

Cryptography & wire format:
  - ed25519-dalek                 Ed25519 signatures over canonical JSON (receipts, decision tokens, audit events).
  - serde_json_canonicalizer      JCS (RFC 8785) for request and policy canonical hashing.
  - sha2                          SHA-256 for request_hash, policy_hash, audit event_hash.
  - JSON Schema 2020-12           7 schemas (aprp, policy, x402, policy_receipt, decision_token, audit_event, sbo3l.passport_capsule.v1).
  - OpenAPI 3.1                   docs/api/openapi.json validated in CI.
  - ULID                          Stable, sortable identifiers for audit and execution refs.

Trust-badge proof viewer: stdlib Python (json, html, argparse, pathlib) — no external dependencies, no JS, no fetch. Stdlib regression test (`trust-badge/test_build.py`) runs in CI.

Other: axum, tokio, tower, rusqlite, anyhow, thiserror, tracing, clap, chrono.

Tooling: cargo fmt, cargo clippy -D warnings, GitHub Actions CI (Rust check + JSON Schema/OpenAPI validators + trust-badge regression test), Codex (Claude Code) PR review workflow.
```

## What is real vs mocked (truthfulness)

The demo runs offline and deterministically. The submission narrative deliberately separates the parts that are end-to-end real from the parts that are local mocks.

```
REAL (end-to-end, exercised by the test suite + the final demo):
  - APRP wire format and `serde(deny_unknown_fields)` strictness — adversarial fixture rejected with `schema.unknown_field`.
  - JCS canonical request hash — golden hash `c0bd2fab…` locked in test.
  - JSON Schema validation — 7 schemas, 14-fixture corpus, no network.
  - Policy engine + agent gate (unknown / paused / revoked / `emergency.paused_agents` / `emergency.freeze_all`).
  - Multi-scope budget tracker (per_tx non-accumulating; daily / monthly / per_provider accumulating; commit only on Allow).
  - APRP nonce replay rejection — HTTP 409 + `protocol.nonce_replay`, fires before request_hash / policy / budget / audit / signing so a replay produces no side effects. Dedup is backed by the persistent `nonce_replay` SQLite table (migration V002) via `Storage::nonce_try_claim`, so a daemon configured with `Storage::open(path)` rejects replays across process restart; the demo defaults to `Storage::open_in_memory()`, where the same SQLite-backed dedup persists for the lifetime of the demo process.
  - Ed25519-signed policy receipts, decision tokens and audit events.
  - Hash-chained audit log (SQLite + JSONL verifier with structural and strict-hash modes).
  - Audit-chain tamper detection — flip one byte and strict-hash verify rejects.
  - Verifiable audit bundle (`sbo3l audit export` + `sbo3l audit verify-bundle`), including a DB-backed export path that pre-flights chain integrity and signature checks before writing the bundle.
  - Real research-agent harness posting real APRP fixtures (legit + prompt-injection) across the boundary.
  - Agent no-key boundary proof — demo gate asserts zero `SigningKey` / `signing_key` references in the agent crate, zero key-material fixtures, no signing-related cargo deps.
  - Static, offline trust-badge proof viewer with stdlib-only regression test.

MOCKED / OFFLINE in this hackathon build (clearly labelled in demo output):
  - ENS resolution — the demo default is the offline `OfflineEnsResolver` fixture loaded from `demo-fixtures/ens-records.json`. A `LiveEnsResolver` (in `crates/sbo3l-identity/src/ens_live.rs`) is shipped and reads the five `sbo3l:*` text records from a real Ethereum JSON-RPC endpoint; it is env-gated on `SBO3L_ENS_RPC_URL` and is not the demo default. `cargo run -p sbo3l-identity --example ens_live_smoke` validates the live path against `sbo3lagent.eth` on mainnet (5 `sbo3l:*` records, owned by the team during the submission window).
  - KeeperHub backend — the demo default constructs `KeeperHubExecutor::local_mock()`. The live path (`KeeperHubExecutor::execute` with `submit_live_to`, in `crates/sbo3l-keeperhub-adapter/src/lib.rs`) is shipped and was exercised end-to-end against a real KeeperHub workflow during the submission window — env-gated on `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN`, returns a real `executionId`. Mock mode prints `mock: true` and a sponsor note; live mode prints `mock: false` with the real `executionId`.
  - Uniswap backend — the demo default constructs `UniswapExecutor::local_mock()`. The shipped live path is `UniswapExecutor::live_from_env()` (`crates/sbo3l-execution/src/uniswap.rs`), which hits Sepolia QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`) when `SBO3L_UNISWAP_RPC_URL` and `SBO3L_UNISWAP_TOKEN_OUT` are set — read-side quote evidence only (real swap broadcast still scope-cut). The bare back-compat `UniswapExecutor::live()` ctor returns `BackendOffline` at runtime. The swap-policy guard (token allowlist, max notional, max slippage, treasury recipient, quote freshness) is real and runs before any executor call.
  - Signing seeds — `AppState::new` uses deterministic dev seeds in `sbo3l-server::lib.rs` (clearly labelled `⚠ DEV ONLY ⚠`); these seeds are public and demo-only. Production deployments inject real signers via `AppState::with_signers` (TEE/HSM-backed). We do not claim production readiness for TEE/HSM in this build.

Not present in this build (intentional):
  - No `SBO3L_*_LIVE` environment variable feature flag — switching any sponsor adapter from mock to live is a single-constructor-call change, not a runtime toggle.
  - No RFC 8470-style `Idempotency-Key` semantics for safe-retry on 5xx — a 5xx after the nonce is consumed will surface as a 409 `protocol.nonce_replay` on retry rather than replaying the original response.
  - No real secrets, API keys, private keys or wallet keys committed anywhere.

Self-audit artefact: SECURITY_NOTES.md at the repo root documents the known scope limitations a production deployment would need to address (daemon authentication, production signer wiring, budget tracker persistence, idempotency in-flight semantics, Passport verifier scope). Honest disclosure, not a roadmap promise.
```

## Partner prize notes

### ENS — Best Integration for AI Agents

```
SBO3L uses ENS as the public identity layer for autonomous agents. The demo agent `research-agent.team.eth` resolves text records:

  sbo3l:agent_id        research-agent-01
  sbo3l:endpoint        https://example.com/agents/research-agent-01
  sbo3l:policy_hash     <canonical SHA-256 of the active SBO3L policy>
  sbo3l:audit_root      <root of the agent's hash-chained audit log>
  sbo3l:proof_uri       <link to the agent's public proof site / passport capsule>

The demo verifies that the published `sbo3l:policy_hash` matches the canonical hash of the daemon's currently-loaded policy. If they ever drift, the agent is treated as un-trustable. This is a one-line check that gives sponsor reviewers immediate, cryptographic confidence that the on-chain identity and the off-chain enforcement are bound together.

In this hackathon build the resolver is offline (`OfflineEnsResolver` reads `demo-fixtures/ens-records.json`). The `EnsResolver` trait abstraction is real; live testnet resolution is a single adapter swap.
```

### KeeperHub — Best Use of KeeperHub

```
KeeperHub executes. SBO3L proves the execution was authorised. The two layers are designed from the start to compose without rewriting either side: SBO3L sits in front of KeeperHub as the policy / budget / signing / audit boundary; KeeperHub stays the execution substrate SBO3L routes Allow receipts into. Only Allow receipts ever reach the sponsor — Deny receipts are refused before any sponsor call (`policy receipt rejected: decision=Deny`).

What is intentionally adoption-ready on the KeeperHub side: five concrete integration paths (IP-1 … IP-5) catalogued in `docs/keeperhub-integration-paths.md`, each independently small and independently reviewable. Status on `main` at submission time:

  IP-1  sbo3l_* upstream-proof envelope fields on the workflow webhook (4-5 optional string fields)
        → SBO3L-side helper SHIPPED (`sbo3l_keeperhub_adapter::build_envelope`, P5.1); KH side adopts when ready.
  IP-2  Public submission/result envelope JSON Schema (one schema file under your docs)
        → target on KH side (SBO3L validates against KH's published schema once available).
  IP-3  keeperhub.lookup_execution(execution_id) MCP tool (one tool definition + thin handler)
        → SBO3L-side symmetric tool SHIPPED (`sbo3l.audit_lookup` in `sbo3l-mcp`, P3.1, PR #46); judge-facing walk-through `docs/mcp-integration-guide.md`. KH side adopts when ready.
  IP-4  Standalone sbo3l-keeperhub-adapter Rust crate (integrations-page listing / crates.io target)
        → Publishable workspace crate SHIPPED under `crates/sbo3l-keeperhub-adapter/`, re-exported by `sbo3l-execution` for back-compat; crates.io publication itself remains target.
  IP-5  SBO3L Passport capsule URI on the execution row (one optional string column)
        → Capsule schema + verifier SHIPPED (PR #42); `sbo3l passport run` CLI MVP SHIPPED (PR #44). KH side adopts the URI column when ready.

Stacking them gives end-to-end offline auditability of every KeeperHub execution that flowed through SBO3L: an auditor with the right keys can reconstruct what was authorised, who authorised it, which policy applied, and where the audit chain says it sits — without trusting any single party.

In this hackathon build, the demo constructs `KeeperHubExecutor::local_mock()` (clearly disclosed as `mock: true` in every demo output line, with a deterministic `kh-<ULID>` execution_ref). A `KeeperHubExecutor::live()` constructor exists; the live wiring is documented end-to-end in `docs/keeperhub-live-spike.md` (target shape, eight open questions for the KeeperHub team, the test strategy that keeps CI offline, and the file-by-file shopping list — about 250 lines of Rust). Switching to live is a single-constructor-body change once a stable submission schema and credentials are available; there is no env-var feature flag in this build, no silent fallback from mock to live, and no KeeperHub credentials anywhere in the repo (verifiable by `git grep`).
```

### Uniswap — Best Uniswap API Integration (stretch)

```
SBO3L is not a trading bot. The Uniswap adapter exists to prove that an agent which wants to trade through Uniswap can still be bounded by SBO3L's policy and the swap-policy guard. The flow is:

  1. The agent emits an APRP `smart_account_session` swap intent (input/output token, max slippage bps, max notional USD, recipient).
  2. The swap-policy guard (`sbo3l_execution::uniswap::evaluate_swap`) enforces input/output token allowlists, max notional, max slippage, quote freshness window and treasury-recipient guard.
  3. The APRP is posted to SBO3L's policy engine under a swap-aware variant of the reference policy.
  4. Approved receipts go to `UniswapExecutor::local_mock()` which returns a deterministic `uni-<ULID>` execution_ref. Denied receipts never reach the executor.

Demo allow path: USDC → ETH within all caps. Demo deny path: USDC → rug-token at 1500 bps slippage to a non-allowlisted recipient — both the swap-policy guard and SBO3L deny independently. The static fixture's quote uses a `(relaxed)` freshness flag that is explicitly visible in demo output; live mode would use the strict freshness check.

Demo default constructs `UniswapExecutor::local_mock()`. `UniswapExecutor::live_from_env()` (in `crates/sbo3l-execution/src/uniswap.rs`) is the shipped live path — it hits Sepolia QuoterV2 (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`) when `SBO3L_UNISWAP_RPC_URL` and `SBO3L_UNISWAP_TOKEN_OUT` are set; missing env vars surface as `LiveConfigError::MissingEnvVar` at construction time (the executor is never built). The bare back-compat `UniswapExecutor::live()` ctor is the runtime-`BackendOffline` path. The guard and the deny path are real; the executor is mock by default; live mode emits a real read-side quote evidence object (the four QuoterV2 return values: `amountOut`, `sqrtPriceX96After`, `initializedTicksCrossed`, `gasEstimate`). Real swap broadcast is still scope-cut — only the read-side quote is wired.
```

## Demo link

```
Public proof URL (static, no JS, offline-verifiable):
    https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/

Recorded video:
    [VIDEO_URL_PENDING_RECORDING]
    (single in-place replace before submission, portable across BSD/macOS and GNU sed:
        perl -pi -e 's|\[VIDEO_URL_PENDING_RECORDING\]|<actual URL>|' SUBMISSION_FORM_DRAFT.md)

Backup live-demo command for any judge who can build from source:
    git clone https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026.git
    cd SBO3L-ethglobal-openagents-2026
    bash demo-scripts/run-openagents-final.sh
    python3 trust-badge/build.py
    # then open trust-badge/index.html
```

The public proof URL is deployed from `main` by `.github/workflows/pages.yml` (Passport P7.1) and links to: the trust-badge proof viewer, the operator-console evidence panels, and a downloadable `sbo3l.passport_capsule.v1` Passport capsule a judge can verify offline with `sbo3l passport verify --path capsule.json`. The deployed surface is plain static HTML — no JavaScript, no client-side network calls, no external CDN/font/script — and is rendered from the same deterministic regression fixtures the trust-badge / operator-console test suites validate on every CI run, so the URL shows the same shape on every visit.

## GitHub repo link

```
https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
```

## Suggested 4-minute video structure

Mirrors `demo-scripts/demo-video-script.md` (target 3:30, hard stop 3:50). Real human voiceover, no AI TTS, no music-only segments.

| t | Beat | Visual | Notes |
|---|---|---|---|
| 0:00–0:15 | Hook + tagline. | Title card. | "Autonomous agents can be wrong. SBO3L keeps the money safe anyway." Land tagline in the first 10 seconds. |
| 0:15–0:45 | Legit x402 request. Allow → signed receipt → audit event. | `bash demo-scripts/run-openagents-final.sh` (gates 6 + 8) | Pause on `decision: Allow`, `request_hash`, `policy_hash`, `audit_event`, `receipt_sig`. |
| 0:45–1:25 | Prompt-injection attack. SBO3L denies before any signer or executor runs. | Same demo run, gates 6 (prompt-injection scenario) + 10 | Make the malicious string visible. Linger on `decision: Deny`. |
| 1:25–2:00 | Sponsor adapters: KeeperHub and Uniswap. Approved → sponsor mock executes. Denied → sponsor refuses. | Gates 8 + 9 | Disclose `mock: true` and `via … mock executor` qualifiers in passing. |
| 2:00–2:35 | Proof: request hash, policy hash, audit event, signed receipt, tamper detection. | Gate 11 | Close on `strict-hash verify rejected the tampered audit event`. |
| 2:35–3:10 | Agent no-key proof + trust badge one-screen summary. | Gate 12 + `python3 trust-badge/build.py` and open `trust-badge/index.html` | Show the trust-badge after the CLI demo. Static HTML, no network. |
| 3:10–3:40 | Closing. | Title card + commit hash. | "Don't give your agent a wallet. Give it a mandate." Title card carries the commit hash so judges can reproduce. |

Recording checklist:
- 720p+ (1080p preferred), real screen recorder, no phone capture.
- Real human voiceover. No AI TTS, no music-only segments.
- Run `bash demo-scripts/reset.sh` before recording so any persistent state starts fresh.
- Record commit hash on the title card.
- Show the trust badge (`trust-badge/index.html`) after the CLI demo.
- Do not speed up terminal output. If pacing is tight, edit out long waits, never compress them.

---

## Field-by-field copy-paste cheat sheet

| ETHGlobal field | Source section above |
|---|---|
| Project name | Project name |
| Tagline / one-liner | hook block at the top |
| Short description (≤ 100 chars) | Short description |
| Long description | Long description |
| How it is made | How it is made |
| Tech stack | Tech stack |
| Emoji | Emoji |
| Category | Category |
| Demo link | Demo link |
| Source code | GitHub repo link |
| Notes for ENS / KeeperHub / Uniswap | Partner prize notes |

## Sources used to draft this file

- `README.md` — current submission build status and three judge commands.
- `SUBMISSION_NOTES.md` — partner-prize framing, "what is live vs mocked", judging-criteria mapping.
- `FEEDBACK.md` — per-partner narrative for ENS, KeeperHub, Uniswap including hackathon limitations.
- `IMPLEMENTATION_STATUS.md` — current implementation snapshot.
- `demo-scripts/run-openagents-final.sh` — the 13-gate demo flow; ground truth for all "what runs" claims.
- `demo-scripts/demo-video-script.md` — current 3:30 video script; the 4-minute structure above mirrors it.
- `docs/cli/audit-bundle.md` — audit-bundle export / verify reference (linked from README).
- `trust-badge/README.md` — trust-badge proof viewer reference.
- `demo-agents/research-agent/README.md` — agent-boundary description (no signing keys in the agent crate).

Update this file if any of the above documents change.
