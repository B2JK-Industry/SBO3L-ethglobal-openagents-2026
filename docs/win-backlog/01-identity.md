# Product Identity (LOCKED)

> Identity is locked. Do not propose pivots. Every ticket reinforces this identity.
> If you find drift in any doc/code, file a bug; do not silently re-frame.

## Brand

**Name:** SBO3L
**Pronunciation:** "ess-bee-oh-three-ell" (or "essbothree-ell" colloquially)
**Origin:** rebranded from Mandate on 2026-04-29 (PR #58) — `mandate.md` is a live commercial SaaS in the same space, collision was unsalvageable
**Domain (acquire Phase 1):** `sbo3l.dev`
**GitHub:** `B2JK-Industry/SBO3L-ethglobal-openagents-2026` (post-hackathon rename to `B2JK-Industry/sbo3l`)

## Brand promise

> **Don't give your agent a wallet. Give it a mandate.**

The wordplay is preserved: lowercase "mandate" is the *thing* SBO3L issues (a generic noun); SBO3L is the brand. Do not capitalise "Mandate" in this tagline; that would confuse with the unrelated `mandate.md` company.

## One-line positioning

**SBO3L is the cryptographically verifiable trust layer for autonomous AI agents.**

## Two-line elevator

> Every action your agent takes — pay, swap, store, compute, coordinate — passes through SBO3L's policy boundary first: schema validation, deterministic policy decision, multi-scope budgeting, Ed25519-signed receipts, hash-chained audit log, sponsor adapter routing.
>
> Output: a self-contained Passport capsule anyone can verify offline against the agent's published Ed25519 pubkey alone — no daemon, no network, no RPC.

## Per-audience value prop (use the right one for your audience)

| Audience | Value prop |
|---|---|
| **Agent developer** | One HTTP endpoint. Stop holding keys, stop hand-rolling policy. Get a signed audit trail for free. |
| **Compliance / security team** | Cryptographically verifiable proof every agent action was authorised, with offline re-derivation. |
| **Sponsor team (KH/Uniswap/ENS)** | Receive a signed APRP envelope upstream of execution. Replay-safe, policy-checked, audit-linked. |
| **Auditor / judge** | Download one JSON file, run one verifier command, reconstruct what was authorised, who authorised it, which policy applied — without trusting any single party. |

## Architecture pillars (do not break)

1. **APRP wire format** — `serde(deny_unknown_fields)` end-to-end; JCS-canonical SHA-256 request hash. Payment-shaped: `intent`, `amount`, `chain`, `expiry`, `risk_class`, `nonce`.
2. **Hash-chained Ed25519 audit log** — `prev_event_hash` linkage; structural verifier (linkage only) and strict-hash verifier (linkage + signatures + content hashes).
3. **Self-contained Passport capsule v2** (Phase 1 deliverable) — embeds `policy_snapshot` + `audit_segment` so `--strict` re-derives without aux inputs.
4. **Sponsor adapter trait** — `GuardedExecutor` + `local_mock()` / `live_from_env()` ctors per sponsor. Mock and live are first-class peers; mock is CI-safe default.
5. **ENS as the agent trust DNS** (Phase 2 deliverable) — `sbo3l:*` text records + cross-agent verification protocol.
6. **No-key agent boundary** — agent crate has zero `SigningKey` references; signing happens only inside SBO3L. Demo gate 12 grep-asserts this.

## Brand surface (cohesion check, must match across all docs/code/sites)

| Surface | URL / identifier |
|---|---|
| Domain | `sbo3l.dev` |
| Marketing site | `https://sbo3l.dev` |
| Docs site | `https://docs.sbo3l.dev` |
| Hosted preview | `https://app.sbo3l.dev` |
| Public proof page | `https://sbo3l.dev/proof` (migrate from github.io URL gradually) |
| GitHub | `B2JK-Industry/SBO3L-ethglobal-openagents-2026` |
| npm scope | `@sbo3l` |
| PyPI prefix | `sbo3l-*` |
| crates.io prefix | `sbo3l-*` |
| Mainnet ENS | `sbo3lagent.eth` (5 records correct as of 2026-04-30) |
| Sepolia ENS apex | `sbo3l.eth` (Phase 2: register, then issue per-agent subnames via Durin) |

## Identity sub-claims (each is testable)

These are the load-bearing claims SBO3L makes to the world. If a ticket weakens one, escalate to Daniel.

1. **"Self-contained capsule"** — `passport verify --strict --path <v2-capsule>` succeeds with rc=0 and zero SKIPPED checks. Test: `cargo test --test passport_v2_self_contained`.
2. **"No-key agent boundary"** — `grep -rn "SigningKey\|signing_key" demo-agents/research-agent/` returns 0 lines. Test: demo gate 12.
3. **"Hash-chained tamper-evident audit"** — flip one byte in `audit_events.payload_hash`, strict verifier rejects. Test: `bash demo-scripts/run-openagents-final.sh` step 11.
4. **"Cross-agent verification"** (Phase 2) — agent A delegates to agent B via signed attestation; tampered attestation rejected. Test: `cargo test --test cross_agent_verify`.
5. **"Live everywhere"** — KH, ENS mainnet, Uniswap Sepolia all have working `live_from_env()` smoke. Test: 3 example smoke binaries pass with operator env vars set.
6. **"5-minute first success"** — clone → daemon up → curl → signed receipt in < 5 minutes. Test: QUICKSTART.md walked end-to-end by Heidi against fresh clone.

## Anti-claims (things SBO3L explicitly is NOT)

1. **Not a wallet** — never holds private keys for the agent. The agent never holds them either. Signing happens inside SBO3L only.
2. **Not a relayer** — does not broadcast transactions on behalf of users. Sponsor adapters do that (KH, Uniswap).
3. **Not a trading bot** — has no opinion about what to trade. The Uniswap adapter is a guarded executor, not a strategy.
4. **Not on-chain** — the audit chain is local SQLite. The "anchor" today is `mock_anchor_ref` clearly labelled `mock anchoring, NOT onchain`. Optional onchain anchor (EAS, ENS text record) is Phase 3.
5. **Not a TEE/HSM** — production deployments inject real signers via `Signer` trait; SBO3L itself doesn't ship secret enclave hardware.
6. **Not a generic policy engine** — APRP is payment-shaped. SBO3L is purpose-built for autonomous-agent payment intent, not arbitrary tool-call wrapping.

## Voice + tone (for docs, marketing, demo video)

- **Honest over slick.** No silent claims, no marketing fluff. Every claim has a code reference or test.
- **Specific over general.** "881/881 tests, 13 demo gates, 8/8 adversarial fail-closed" beats "robust testing".
- **Code-first.** Show the curl, show the response, then explain.
- **Tagline survives intact.** Don't reword "Don't give your agent a wallet. Give it a mandate."
- **No emoji in code or commits.** Emoji allowed in marketing site / demo video / per-agent persona docs.
- **No exclamation marks** in docs except where literally appropriate (CLI output `!`).
