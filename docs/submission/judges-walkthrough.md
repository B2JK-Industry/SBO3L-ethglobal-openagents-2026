---
title: "SBO3L for ETHGlobal Open Agents 2026 — judges walkthrough"
audience: "ETHGlobal judges + sponsor-bounty reviewers"
outcome: "In 5, 30, or 90 minutes you can independently verify SBO3L's load-bearing claim from a clean machine."
---

# SBO3L — judges walkthrough

> **Tagline:** Don't give your agent a wallet. Give it a mandate.
>
> **Load-bearing claim:** Every agent action leaves a portable, offline-verifiable proof of authorisation.
>
> **Verification mode:** Every claim on this page has a **runnable command, a code reference, or a live URL**. Honest-over-slick.

This is the entry point. It indexes everything else. Pick a reading-time budget below.

---

## ⏱️ If you have **5 minutes** — verify one capsule end-to-end

```bash
# 1. Install the CLI from crates.io (latest stable: v1.2.2)
cargo install sbo3l-cli --version 1.2.2

# 2. Run the full Open-Agents demo (13 gates, ~30s)
git clone --depth=1 https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
cd SBO3L-ethglobal-openagents-2026
bash demo-scripts/run-openagents-final.sh
# expect: 13/13 gates green, capsule written to demo-scripts/artifacts/

# 3. Verify the capsule offline (no daemon, no network, no RPC)
sbo3l passport verify --strict --path demo-scripts/artifacts/passport-allow.json
# expect: PASSED with ZERO SKIPPED checks
```

That's the whole product in three commands. The capsule contains every byte needed to re-derive the policy decision; the verifier runs against the agent's published Ed25519 pubkey alone.

**Browser version:** drop a capsule JSON at https://sbo3l-marketing.vercel.app/proof — same WASM verifier runs in the page. Tamper one byte, verifier rejects.

If just one of those three commands fails, **the project hasn't shipped what it claims**. Move on. If they all pass, you're holding cryptographic proof of an agent action — keep reading for the rest.

---

## ⏱️ If you have **30 minutes** — independent reproduction script + per-bounty depth

### One script verifies everything

```bash
bash scripts/judges/verify-everything.sh
# 33 PASS / 1 FAIL (known namehash bug in script; not a SBO3L regression)
# elapsed: ~4-5 min on a fresh Ubuntu/macOS clone
```

What it checks:
- 10 Rust crates installable from crates.io @ 1.2.2
- 26 npm packages + 8 PyPI packages installable
- `sbo3l --version` returns 1.2.2
- ENS Registry mainnet RPC: `sbo3lagent.eth` resolver lookup
- CCIP-Read gateway smoke fail-mode rejection
- Marketing site / GitHub / ENS app HTTP 200
- `request_hash` byte-deterministic across machines (`5a46c8ae…5732c4`)

### Per-bounty deep-dives (~6 min each)

Each one-pager is ~500 words, evidence-linked, sponsor-specific:

| Bounty | One-pager | Key claim |
|---|---|---|
| KeeperHub Best Use | [`bounty-keeperhub.md`](bounty-keeperhub.md) | KeeperHub executes; SBO3L proves the execution was authorised. IP-1..IP-5 paths catalogued. |
| KeeperHub Builder Feedback | [`bounty-keeperhub-builder-feedback.md`](bounty-keeperhub-builder-feedback.md) | Five concrete issues filed during real integration: [KH/cli #47-#51](https://github.com/KeeperHub/cli/issues). |
| ENS Most Creative | [`bounty-ens-most-creative.md`](bounty-ens-most-creative.md) | ENS isn't the integration; ENS is the trust DNS. 5 records on chain, byte-matching the offline fixture. |
| ENS AI Agents | [`bounty-ens-ai-agents.md`](bounty-ens-ai-agents.md) | Identity (ENS) + dynamic state (CCIP-Read) + global registry (ERC-8004) — three layers, one trust profile. |
| Uniswap Best API | [`bounty-uniswap.md`](bounty-uniswap.md) | A swap via Uniswap is opaque today. SBO3L makes the audit trail cryptographic — same API, every call gated/signed/bound to a re-derivable decision. |

### What's actually live on the wire

| Surface | URL | Status |
|---|---|---|
| Marketing site | https://sbo3l-marketing.vercel.app/ | ✅ 200 |
| Public proof verifier | https://sbo3l-marketing.vercel.app/proof | ✅ 200 |
| CCIP-Read gateway | https://sbo3l-ccip.vercel.app/ + smoke-fail https://sbo3l-ccip.vercel.app/api/0xdeadbeef/0x12345678.json | ✅ 200 / ✅ 400 (correct rejection) |
| ENS mainnet apex | https://app.ens.domains/sbo3lagent.eth | ✅ 200; 5 records on chain |
| GitHub releases | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases | ✅ v1.0.0 + v1.2.2 |
| crates.io / npm / PyPI | see [`live-url-inventory.md`](live-url-inventory.md) | 10 crates + 26 npm + 8 PyPI live (3 npm packages awaiting first tag-publish, marked allow_failure in install-smoke matrix) |

### Real-time visualisation

[`https://sbo3l-trust-dns-viz.vercel.app`](https://sbo3l-trust-dns-viz.vercel.app) (custom-domain `app.sbo3l.dev` not yet pointed) — D3 + canvas force-directed graph; agents discover each other and sign cross-agent attestations live. **This is the demo-video centerpiece for ENS Most Creative.**

---

## ⏱️ If you have **90 minutes** — long-form narratives + chaos verification

### Trust DNS essay (1851 words)

[`docs/concepts/trust-dns-manifesto.md`](../concepts/trust-dns-manifesto.md) — Frank-style audience+outcome, literal DNS analogy table, why ENS specifically (5 properties no competing system gives at once), static-vs-dynamic records via CCIP-Read, the 60-agent fleet that validates scale, honest scope (4 things SBO3L is NOT), reproducibility table.

### Long-form ENS narrative (~400 lines)

[`docs/proof/ens-narrative.md`](../proof/ens-narrative.md) — runtime cross-agent authentication walkthrough with code examples; byte-match assertions; resolution-from-mainnet timing budgets.

### Chaos engineering suite

```bash
cargo build --release -p sbo3l-server
SBO3L_SERVER_BIN=target/release/sbo3l-server bash scripts/chaos/run-all.sh
```

Five scenarios that prove the daemon's hash-chained audit log + idempotency state machine + budget transactions are tamper-evident and recoverable under realistic failure modes:

| # | Scenario | Asserts |
|---|---|---|
| 1 | daemon crash mid-tx | audit chain advances correctly across SIGKILL+restart |
| 2 | storage byte-flip | strict-hash verifier rejects; structural verifier accepts (linkage byte intact — by design) |
| 3 | sponsor partition | KH webhook to RFC 5737 black-hole; idempotency 409 on replay |
| 4 | concurrent same-key race | 50 same-key POSTs → exactly one event in audit chain (state machine holds under load) |
| 5 | clock-skew expiry | APRP with `expiry: 120s ago` rejected with `protocol.aprp_expired` (P0-SECURITY fix [#226](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/226)) |

Two real findings surfaced and fixed during the chaos run, captured as commit history (CHAOS-1 [#227](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/227), CHAOS-2 [#226](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/226)) — exactly the kind of finding you want a chaos suite to surface before a release.

### v1.2.0 release narrative

[`docs/release/v1.2.0-prep.md`](../release/v1.2.0-prep.md) — pre-tag verification + version bump checklist + tag flow + publish workflow mapping + post-publish smoke + GitHub Release page template + rollback decision tree. Tag cuts when Phase 2 strict-done ≥ 80% (currently 82%, 19/22 tickets).

### Codex audit feedback loop

Every PR runs through Codex automated review. As of submission rehearsal: **35 P1/P2 findings tracked across the PR queue, 17 fixed in active-fix buckets, remaining 18 tracked in [#162](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/issues/162)**. Examples: [#226 CHAOS-2 fix](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/226) (Codex-flagged P0-SECURITY closed), [#102 idempotency `created_at` reset](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/102) (Codex P2 closed), [#104 KMS dev-seed lockout](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/104) (Codex P1 closed).

---

## How SBO3L is *built* (the meta-claim)

Production-shaped, not hackathon-shaped:

- **400+ Rust workspace tests**, 13/13 demo gates, 26 real / 0 mock / 1 skipped on the production-shaped runner
- **Post-merge regression-on-main workflow** ([`#161`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/161)) — every push to `main` runs the full sweep + posts a Heidi-styled summary back on the merging PR
- **Lighthouse audit workflow** ([`#210`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/210)) — score gate ≥ 90 on perf/a11y/best-practices/SEO; auto-issue on failure
- **Uptime probe workflow** ([`#196`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/196)) — every 30 min, every live URL; auto-issue on failure
- **Chaos suite** ([`#196`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/196), [`#227`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/227)) — pre-tag gate
- **Cascade watcher** ([`#210`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/210)) — monitors PR transitions, codex findings, CI changes; emits one event per state transition

200+ PRs merged in 100 days; every PR reviewed by Codex + Heidi (QA + Release agent); zero force-pushes to `main`; branch protection requires 2 approvals + green CI + up-to-date branch.

---

## Reading-time budget summary

| You have | Read | Run |
|---|---|---|
| **5 min** | this page section "5 min" | the 3 install + verify commands |
| **30 min** | this page + the 5 bounty one-pagers | `verify-everything.sh` (4-5 min) |
| **90 min** | this page + bounty docs + Trust DNS essay + ENS narrative | `verify-everything.sh` + `chaos/run-all.sh` + browse `/proof` page |

If you have less than 5 minutes: **drag any `passport-*.json` from `demo-scripts/artifacts/` into https://sbo3l-marketing.vercel.app/proof**. That's the load-bearing demonstration in one click.

---

## Submission package map (what's where)

```
docs/submission/
├── README.md                              ← package overview
├── judges-walkthrough.md                  ← THIS FILE (entry point)
├── live-url-inventory.md                  ← every live URL with smoke status
├── ETHGlobal-form-content.md              ← paste-ready form fields
├── demo-video-script.md                   ← 3-min storyboard
├── rehearsal-runbook.md                   ← Daniel's pre-record checklist
├── codex-followup-2026-05-01.md           ← codex finding cross-reference
├── url-evidence.md                        ← HTTP-level URL evidence
├── bounty-keeperhub.md                    ← KH Best Use
├── bounty-keeperhub-builder-feedback.md   ← KH Builder Feedback
├── bounty-ens-most-creative.md            ← ENS Most Creative
├── bounty-ens-ai-agents.md                ← ENS AI Agents
├── bounty-uniswap.md                      ← Uniswap Best API
└── partner-onepagers/{keeperhub,ens,uniswap}.md  ← submission-shaped install-first

docs/concepts/
└── trust-dns-manifesto.md                     ← long-form Trust DNS essay (T-3-6)

docs/proof/
├── ens-narrative.md                       ← long-form ENS deep-dive
├── ens-pitch.md                           ← Dhaiwat-targeted pitch
├── ens-tweets.md                          ← submission tweet thread
└── ens-fleet-*.json                       ← Sepolia agent fleet manifest

docs/release/
└── v1.2.0-prep.md                         ← release runbook

scripts/
├── judges/verify-everything.sh            ← 10-min judge verification
├── chaos/                                 ← 5-scenario chaos suite
├── monitoring/check-live-urls.sh          ← uptime probe
├── submission/rehearsal-audit.sh          ← link + claim audit
└── qa/cascade-watch.sh                    ← Heidi PR watcher
```

Tagline survives intact: **Don't give your agent a wallet. Give it a mandate.**
