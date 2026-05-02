# Codex audit follow-up — 2026-05-01 23:30 CEST

> **Purpose:** cross-reference every codex P1/P2 finding still on an open PR against what owner agents are actively fixing this turn. Anything not in a named active-fix bucket gets a Heidi inline comment so it doesn't fall through.
>
> **Snapshot scope:** 22 open PRs with inline review comments (codex bot + a couple of human review notes). Excludes already-merged PRs (#180 ✅, #161 ✅, etc.).

## Owner active-fix buckets (this turn)

| Owner | Active-fix PRs | Note |
|---|---|---|
| **Dev 1** | #134 (wasm verifier slice), #176 (T-3-5 e2e closeout) | |
| **Dev 2** | #179 (MEV guard) + **7-PR nonce sweep** + #165 (T-5-1 swap CI) | Nonce sweep covers #144, #148, #151, #152, #153, #154, #155 demos |
| **Dev 3** | #136 (hosted-app slice 2), #150 (/demo polish), #164 (canvas renderer) | |
| **Dev 4** | #167, #177 (T-3-4 / T-3-7 rebase) + #138, #173, #163 (T-3-3, manifest, T-3-2) | |

## 🟢 Findings already in an active-fix bucket (no action from Heidi)

| PR | Owner | Severity | Finding (one-liner) | Bucket |
|---|---|---|---|---|
| #163 | Dev 4 | _CI failure-2_ | T-3-2 verify-ens CLI — verify-ens slice + live mainnet test | Dev 4 active |
| #138 | Dev 4 | _CI green_ | T-3-3 fleet-of-5 infra | Dev 4 active |
| #173 | Dev 4 | _CI green_ | commit-fleet-manifest.sh follow-up | Dev 4 active |
| #167 | Dev 4 | _CONFLICTING_ | T-3-4 cross-agent verification protocol | Dev 4 rebase |
| #177 | Dev 4 | _CONFLICTING_ | T-3-7 ENS narrative live verification + cli | Dev 4 rebase |
| #134 | Dev 1 | _CI green_ | wasm verifier slice — sbo3l-core builds for wasm | Dev 1 active |
| #136 | Dev 3 | _CI green_ | hosted-app real SBO3L wiring with mock fallback | Dev 3 active |
| #150 | Dev 3 | _CI green_ | /demo judge walkthrough | Dev 3 active |
| #164 | Dev 3 | _CI green_ | trust-dns-viz canvas renderer for ≥100 agents | Dev 3 active |
| #179 | Dev 2 | _CI mixed_ | T-5-4 MEV guard — slippage + recipient allowlist | Dev 2 active |
| #165 | Dev 2 | _CI failure-2_ | T-5-1 full swap construction (Rust + TS + Py) | Dev 2 active |
| #144 | Dev 2 | P2 | demo vercel-ai — fixed nonce | Dev 2 nonce sweep |
| #148 | Dev 2 | P1 | demo langchain-py — stale APRP expiry + fixed nonce | Dev 2 nonce sweep |
| #151 | Dev 2 | P1 | demo autogen — fixed nonce | Dev 2 nonce sweep |
| #153 | Dev 2 | P2 | demo llamaindex — fixed nonce | Dev 2 nonce sweep |
| #154 | Dev 2 | P1 | demo langgraph — fixed nonce | Dev 2 nonce sweep |
| #152 | Dev 2 | P2 | demo elizaos — fixed nonce | Dev 2 nonce sweep |

## 🔴 Findings NOT in any active-fix bucket — Heidi flags

These need a comment ping so they don't fall through. Posting inline on each PR.

### PR #141 — Dev 4 — ENS-AGENT-A1 amplifier (60-agent fleet config)

| Severity | File | Issue |
|---|---|---|
| **P1** | `scripts/register-fleet.sh:251` | Resolver fallback for legacy dry-run envelopes not implemented (only `DURIN_REGISTRAR_ADDR` has fallback; `PUBLIC_RESOLVER` hard-fails the entire fleet run) |
| P2 | `scripts/register-fleet.sh:122` | Manifest filename `ens-fleet-<date>.json` doesn't include fleet size — running 5-agent + 60-agent on the same day overwrites; expected `ens-fleet-60-<date>.json` per docs |

### PR #145 — Dev 2 — per-package release workflow

| Severity | File | Issue |
|---|---|---|
| **P1** | `.github/workflows/integrations-publish.yml:103` | `pypi-build-test` matrix mixes `python:` array with `include:` list incorrectly — does not produce 4×3 integration-by-Python matrix as intended |
| P2 | `.github/workflows/integrations-publish.yml:163` | `npm-publish` `needs: npm-build-test` blocks per-package release on unrelated matrix failures — defeats the per-package release flow |

### PR #152 — Dev 2 — demo elizaos (additional finding beyond nonce)

| Severity | File | Issue |
|---|---|---|
| **P1** | `examples/elizaos-research-agent/package.json:14` | `@sbo3l/elizaos` referenced via `file:` path but `dist/` not built/committed — fresh checkout gets `ERR_MODULE_NOT_FOUND` running `npm install && npm run smoke` |

### PR #155 — Dev 2 — multi-framework killer demo

| Severity | File | Issue |
|---|---|---|
| **P1** | `examples/multi-framework-agent/services/plan/app.py:46` | APRP intent is `pay_compute_job`; `reference_low_risk.json` only allows `input.intent == "purchase_api_call"` — first `/plan` decision is denied, never reaches advertised 3-step chain |
| **P1** | `…/plan/app.py:52` | x402 APRPs missing `destination.expected_recipient` — recipient allowlist resolves null → `policy.deny_recipient_*` |

### PR #182 — Dev 4 — cross-agent verification TS port

| Severity | File | Issue |
|---|---|---|
| **P1** | `crates/sbo3l-identity/src/cross_agent.rs:182` | Resolver adapter converts every `resolve_raw_text` failure into `CrossAgentError::EnsResolve`, including `UnknownName` — should map to a `CrossAgentTrust` rejection (not a hard error) so unknown peers are denied cleanly |
| P2 | `sdks/typescript/src/cross-agent.ts:158` | `signChallenge` returns the original `challenge` object by reference — caller mutation after signing causes silent signature/payload drift |

## Severity totals (across all 22 open PRs with inline comments)

- **P1 inside active-fix buckets:** 5 (will land with the named PRs)
- **P1 outside any active-fix bucket:** 7 → flagged below as inline Heidi comments
- **P2 outside any active-fix bucket:** 4 → flagged inline (lower priority but bundled with the P1 ping)

## Process note

The `regression-on-main.yml` workflow shipped in #161 runs the full sweep post-merge but does **not** check for unaddressed codex findings. Suggestion: a follow-up PR adds `scripts/check-codex-followup.py` that fails CI if a merging PR has open codex inline comments authored by `chatgpt-codex-connector[bot]` and not closed by a "fixed in <commit>" reply. Defer to Phase 3.
