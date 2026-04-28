# ETHGlobal Open Agents 2026 — Submission Form Draft (Mandate)

Copy-paste-ready text for the ETHGlobal Open Agents 2026 project submission form. Every field below maps to an ETHGlobal form field (or to a video / repo link). Fields with a `<TODO: …>` placeholder are the only ones that need to be filled in on submission day.

> One-sentence hook (use anywhere a hook is required):
>
> **Don't give your agent a wallet. Give it a mandate.**

---

## Project name

```
Mandate
```

## Category

```
Infrastructure
```

## Emoji

```
🛂
```

(Passport-control emoji — visualises Mandate as the customs gate every agent payment has to clear before it ever reaches a wallet or sponsor.)

## Short description (under 100 characters)

```
Spending mandates for AI agents. The agent never holds the key; Mandate decides, signs and audits.
```

(98 characters including spaces — ETHGlobal's short-description field caps at 100.)

## Long description (judge-facing)

```
Mandate is a local policy, budget, receipt and audit firewall that decides whether an autonomous AI agent may execute an onchain or payment action — so the agent never has to hold a private key.

A research-agent in our demo emits a payment request (an APRP — "Agent Payment Request Protocol") across the Mandate boundary. Mandate validates the request, evaluates a deterministic policy, enforces multi-scope budgets, rejects replayed nonces with HTTP 409, signs an Ed25519 policy receipt, appends a hash-chained audit event, and only then routes the action to a sponsor executor (KeeperHub or Uniswap in this demo). When the same agent is prompt-injected and forwards a hostile request, Mandate denies before any signer or executor runs and the audit log captures the rejection. Tampering with one byte of an audit event is rejected by the strict-hash verifier.

The whole flow is deterministic, runs offline, and reproduces from a fresh clone in ~5 seconds with `bash demo-scripts/run-openagents-final.sh`. 96/96 tests pass, schemas validate, the demo's 11 gates are green end-to-end including the audit-chain tamper-detection step.

Mandate is not a wallet, not a relayer, and not a trading bot. It is the pre-execution policy and signing boundary that lets autonomous agents transact without ever being trusted with a key.
```

## How it is made

```
Mandate is a Rust workspace built during ETHGlobal Open Agents 2026 around four hard contracts: a strict APRP wire format with `serde(deny_unknown_fields)` end-to-end and a JCS-canonical request hash locked at `c0bd2fab…`; a deterministic policy engine evaluating a small Rego-compatible expression grammar over a hash-locked policy file; a multi-scope budget tracker (`per_tx`, `daily`, `monthly`, `per_provider`); and an Ed25519-signed, hash-chained audit log persisted in SQLite with a separate JSONL verifier offering both structural and strict-hash modes.

The HTTP boundary is `POST /v1/payment-requests`, served by axum. Each request runs through the same fail-closed pipeline: schema validation → canonical request hash → APRP nonce-replay gate (HTTP 409 + `protocol.nonce_replay`, before any state mutation) → policy decision → budget commit (only on Allow) → audit append → signed policy receipt. Receipts and decision tokens are JCS-canonical JSON signed with Ed25519; audit events are linked by `prev_event_hash` and verifiable end-to-end with the `mandate verify-audit` CLI.

A research-agent harness drives the boundary across two scenarios — a legitimate x402 purchase and a prompt-injection attack — by posting real APRP fixtures across the API and printing the daemon's signed response. The agent crate intentionally has zero signing dependencies; you can verify it by grepping for `SigningKey` / `signing_key` in `demo-agents/research-agent/`. ENS, KeeperHub and Uniswap each show up as guarded executors behind a thin adapter trait so they can be swapped for live backends without touching the policy/signing core.
```

## Tech stack

```
Rust workspace (8 crates + research-agent demo binary):
  - mandate-core        APRP types, JCS canonical hashing, Ed25519 signer, receipts, decision tokens, audit events.
  - mandate-policy      Policy model + Rego-compatible expression evaluator, decide(), multi-scope budget tracker.
  - mandate-storage     SQLite-backed audit log with hash-chain verifier (rusqlite + migrations).
  - mandate-server      axum HTTP server, POST /v1/payment-requests pipeline, SQLite-backed APRP nonce-replay gate (atomic INSERT into the `nonce_replay` table from migration V002).
  - mandate-execution   Guarded-executor adapters (KeeperHub, Uniswap) with explicit local_mock / live constructors.
  - mandate-identity    ENS resolver trait + offline fixture resolver + policy_hash drift check.
  - mandate-mcp         MCP tool surface skeleton.
  - mandate-cli         `mandate` CLI: aprp validate|hash|run-corpus, schema, verify-audit.

Cryptography & wire format:
  - ed25519-dalek                 Ed25519 signatures over canonical JSON (receipts, decision tokens, audit events).
  - serde_json_canonicalizer      JCS (RFC 8785) for request and policy canonical hashing.
  - sha2                          SHA-256 for request_hash, policy_hash, audit event_hash.
  - JSON Schema 2020-12           6 schemas (aprp, policy, x402, policy_receipt, decision_token, audit_event).
  - OpenAPI 3.1                   docs/api/openapi.json validated in CI.
  - ULID                          Stable, sortable identifiers for audit and execution refs.

Other: axum, tokio, tower, rusqlite, anyhow, thiserror, tracing, clap, chrono.

Tooling: cargo fmt, cargo clippy -D warnings, GitHub Actions CI (Rust check + JSON Schema/OpenAPI validators), Codex (Claude Code) PR review workflow.
```

## What is real vs mocked (truthfulness)

The demo runs offline and deterministically. The submission narrative deliberately separates the parts that are end-to-end real from the parts that are local mocks.

```
REAL (end-to-end, exercised by the test suite + the final demo):
  - APRP wire format and `serde(deny_unknown_fields)` strictness — adversarial fixture rejected with `schema.unknown_field`.
  - JCS canonical request hash — golden hash `c0bd2fab…` locked in test.
  - JSON Schema validation — 6 schemas, 4-fixture corpus, no network.
  - Policy engine + agent gate (unknown / paused / revoked / `emergency.paused_agents` / `emergency.freeze_all`).
  - Multi-scope budget tracker (per_tx non-accumulating; daily / monthly / per_provider accumulating; commit only on Allow).
  - APRP nonce replay rejection — HTTP 409 + `protocol.nonce_replay`, fires before request_hash / policy / budget / audit / signing so a replay produces no side effects. Dedup is backed by the persistent `nonce_replay` SQLite table (migration V002) via `Storage::nonce_try_claim`, so a daemon configured with `Storage::open(path)` rejects replays across process restart; the demo defaults to `Storage::open_in_memory()`, where the same SQLite-backed dedup persists for the lifetime of the demo process.
  - Ed25519-signed policy receipts, decision tokens and audit events.
  - Hash-chained audit log (SQLite + JSONL verifier with structural and strict-hash modes).
  - Audit-chain tamper detection — flip one byte and strict-hash verify rejects.
  - Real research-agent harness posting real APRP fixtures (legit + prompt-injection) across the boundary.

MOCKED / OFFLINE in this hackathon build (clearly labelled in demo output):
  - ENS resolution — the demo uses an offline `OfflineEnsResolver` fixture loaded from `demo-fixtures/ens-records.json`; the `EnsResolver` trait abstracts a future live testnet resolver but no live resolver is shipped in this build.
  - KeeperHub backend — the demo always constructs `KeeperHubExecutor::local_mock()`. A `KeeperHubExecutor::live()` constructor exists for the production path but is not exercised; the demo's KeeperHub mock receipt prints `mock: true` and a sponsor note.
  - Uniswap backend — the demo always constructs `UniswapExecutor::local_mock()`. `UniswapExecutor::live()` is intentionally stubbed and returns `BackendOffline`; the swap-policy guard (token allowlist, max notional, max slippage, treasury recipient, quote freshness) is real and runs before any executor call.
  - Signing seeds — `AppState::new` uses deterministic dev seeds in `mandate-server::lib.rs` (clearly labelled `⚠ DEV ONLY ⚠`); production deployments inject real signers via `AppState::with_signers` (TEE/HSM-backed).

Not present in this build (intentional):
  - No `MANDATE_*_LIVE` environment variable feature flag — switching any sponsor adapter from mock to live is a single-constructor-call change, not a runtime toggle.
  - No RFC 8470-style `Idempotency-Key` semantics for safe-retry on 5xx — a 5xx after the nonce is consumed will surface as a 409 `protocol.nonce_replay` on retry rather than replaying the original response.
  - No real secrets, API keys, private keys or wallet keys committed anywhere.
```

## Partner prize notes

### ENS — Best Integration for AI Agents

```
Mandate uses ENS as the public identity layer for autonomous agents. The demo agent `research-agent.team.eth` resolves text records:

  mandate:agent_id        research-agent-01
  mandate:endpoint        https://example.com/agents/research-agent-01
  mandate:policy_hash     <canonical SHA-256 of the active Mandate policy>
  mandate:audit_root      <root of the agent's hash-chained audit log>
  mandate:receipt_schema  <link to the policy_receipt_v1 schema>

The demo verifies that the published `mandate:policy_hash` matches the canonical hash of the daemon's currently-loaded policy. If they ever drift, the agent is treated as un-trustable. This is a one-line check that gives sponsor reviewers immediate, cryptographic confidence that the on-chain identity and the off-chain enforcement are bound together.

In this hackathon build the resolver is offline (`OfflineEnsResolver` reads `demo-fixtures/ens-records.json`). The `EnsResolver` trait abstraction is real and live testnet resolution is a single adapter swap.
```

### KeeperHub — Best Use of KeeperHub

```
Mandate decides, KeeperHub executes. After the policy engine returns Allow, the signed `PolicyReceipt` and the underlying APRP are handed to `KeeperHubExecutor::execute()`. Only Allow receipts ever reach the sponsor; Deny receipts are refused before any sponsor call (`policy receipt rejected: decision=Deny`). This separation maps directly onto KeeperHub's "execution layer for AI agents onchain" framing: Mandate is the pre-execution policy and risk layer, KeeperHub is the execution layer.

In this hackathon build the demo constructs `KeeperHubExecutor::local_mock()` and prints `mock: true` plus a deterministic `kh-<ULID>` execution_ref. A `KeeperHubExecutor::live()` constructor exists; switching to a live KeeperHub MCP/API call is a single-function-body change once a stable action-submission schema is published.
```

### Uniswap — Best Uniswap API Integration (stretch)

```
Mandate is not a trading bot. The Uniswap adapter exists to prove that an agent which wants to trade through Uniswap can still be bounded by Mandate's policy and the swap-policy guard. The flow is:

  1. The agent emits an APRP `smart_account_session` swap intent (input/output token, max slippage bps, max notional USD, recipient).
  2. The swap-policy guard (`mandate_execution::uniswap::evaluate_swap`) enforces input/output token allowlists, max notional, max slippage, quote freshness window and treasury-recipient guard.
  3. The APRP is posted to Mandate's policy engine under a swap-aware variant of the reference policy.
  4. Approved receipts go to `UniswapExecutor::local_mock()` which returns a deterministic `uni-<ULID>` execution_ref. Denied receipts never reach the executor.

Demo allow path: USDC → ETH within all caps. Demo deny path: USDC → rug-token at 1500 bps slippage to a non-allowlisted recipient — both the swap-policy guard and Mandate deny independently. The static fixture's quote uses a `(relaxed)` freshness flag that is explicitly visible in demo output; live mode would use the strict freshness check.

In this hackathon build the demo always constructs `UniswapExecutor::local_mock()`; `UniswapExecutor::live()` is intentionally stubbed and returns `BackendOffline`. Wiring the Uniswap Trading API is a single-function-body change.
```

## Demo link

```
<TODO: paste recorded demo URL (YouTube unlisted or Loom) before submission>

Backup live-demo command for any judge who can build from source:
    git clone https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026.git
    cd mandate-ethglobal-openagents-2026
    bash demo-scripts/run-openagents-final.sh
```

## GitHub repo link

```
https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026
```

## Suggested 4-minute video structure

Mirrors `demo-scripts/demo-video-script.md` (target 3:30, hard stop 3:50). Real human voiceover, no AI TTS, no music-only segments.

| t | Beat | Visual | Notes |
|---|---|---|---|
| 0:00–0:20 | Hook + one-sentence "what is Mandate" + tagline. | Title card. | Land "Don't give your agent a wallet. Give it a mandate." in the first 10 seconds. |
| 0:20–0:40 | ENS agent identity. Show `mandate:policy_hash` matching the active Mandate policy hash. | `bash demo-scripts/sponsors/ens-agent-identity.sh` | Highlight the `ens.verify: ok (matches active policy …)` line. Disclose offline resolver in one line. |
| 0:40–1:15 | Legitimate x402 purchase. Allow → signed receipt → audit event. | `./demo-agents/research-agent/run --scenario legit-x402` | Pause on `decision: Allow`, `request_hash`, `policy_hash`, `audit_event`, `receipt_sig`. |
| 1:15–1:45 | KeeperHub guarded execution. Approved receipt routed; `kh-<ULID>` returned. | `bash demo-scripts/sponsors/keeperhub-guarded-execution.sh` (allow path only) | Disclose `mock: true` line in passing. |
| 1:45–2:25 | Prompt-injection attack. Show `attack_prompt` text on screen. Mandate denies before any signer/executor runs. KeeperHub refuses the denied receipt. | `./demo-agents/research-agent/run --scenario prompt-injection --execute-keeperhub` | Linger on `decision: Deny` + `keeperhub.refused`. Make the malicious string visible. |
| 2:25–3:00 | Uniswap guarded swap. Bounded USDC → ETH allowed. Rug-token attacker quote denied by both swap-policy guard and Mandate. | `bash demo-scripts/sponsors/uniswap-guarded-swap.sh` | Show the `FAIL` lines on the deny path + `uniswap.refused`. |
| 3:00–3:25 | Audit chain end-to-end. Three events linked, signed. Tamper one byte → strict-hash verify rejects. | `run-openagents-final.sh` step 11 output | Close on `strict-hash verify rejected the tampered audit event`. |
| 3:25–3:50 | Sign-off. | Title card + commit hash. | "Don't give your agent a wallet. Give it a mandate." Title card carries the commit hash so judges can reproduce. |

Recording checklist:
- 720p+ (1080p preferred), real screen recorder, no phone capture.
- Real human voiceover. No AI TTS, no music-only segments.
- Run `bash demo-scripts/reset.sh` before recording so any persistent state starts fresh.
- Record commit hash on the title card.
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

- `README.md` — current submission build status and demo command.
- `SUBMISSION_NOTES.md` — partner-prize framing, "what is live vs mocked", judging-criteria mapping.
- `FEEDBACK.md` — per-partner narrative for ENS, KeeperHub, Uniswap including hackathon limitations.
- `IMPLEMENTATION_STATUS.md` — what is implemented and which hardening PRs landed.
- `FINAL_REVIEW.md` — independent submission-readiness audit, mock/live truthfulness verdict.
- `demo-scripts/run-openagents-final.sh` — the 11-step demo flow; ground truth for all "what runs" claims.
- `demo-scripts/demo-video-script.md` — existing storyboard; the 4-minute structure above stays compatible.
- `demo-agents/research-agent/README.md` — agent-boundary description (no signing keys in the agent crate).

Update this file if any of the above documents change.
