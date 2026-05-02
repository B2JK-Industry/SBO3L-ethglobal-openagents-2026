# Pre-submission rehearsal walkthrough — R12 (2026-05-02)

> **Round:** R12 — final pre-submission readiness pass.
> **Performed by:** Heidi (QA + Release agent).
> **Mode:** static — Heidi has `curl` but no browser; interactive steps (drag-drop into `/proof`, click-through demo, install CLI) are flagged as **DELEGATED** to Daniel for hands-on confirmation.
> **Target time:** < 8 minutes (per R12 P4 brief).
> **Actual elapsed:** 3 seconds (curl-only) — Daniel's hands-on walk should target the 8-minute window.

## Step-by-step results

| # | Step | URL / command | Result | Time | Notes |
|---|---|---|---|---|---|
| 1 | Open marketing root | https://sbo3l-marketing.vercel.app/ | ✅ HTTP 200 | 0.18s | |
| 2 | Click "Demo" → /demo | https://sbo3l-marketing.vercel.app/demo | ✅ HTTP 200 | 0.12s | `<title>SBO3L — Demo walkthrough</title>` confirmed |
| 3a | /demo/1-meet-the-agents | (above) | ✅ HTTP 200 | 0.27s | Step path is `/demo/1-meet-the-agents`, not `/demo/1` (informal brief shorthand) |
| 3b | /demo/2-watch-a-decision | | ✅ HTTP 200 | 0.30s | |
| 3c | /demo/3-verify-yourself | | ✅ HTTP 200 | 0.54s | WASM verifier playground (#238) |
| 3d | /demo/4-explore-the-trust-graph | | ✅ HTTP 200 | 0.27s | |
| 4 | Drop golden capsule into /proof; verify ✅ × 6 | https://sbo3l-marketing.vercel.app/proof | ✅ HTTP 200 (page reachable) | — | **DELEGATED**: page loads but verifier is interactive WASM — Daniel must drop `test-corpus/passport/v2-capsule.json` and confirm 6/6 ✅ checks visually |
| 5 | Tamper a byte, verify ❌ | (same /proof page) | — | — | **DELEGATED**: requires browser interaction. Recommended tamper: flip first byte of `audit_chain[0].payload_hash` |
| 6 | Click /marketplace → see 5 starter policies | https://sbo3l-marketing.vercel.app/marketplace | 🔴 **HTTP 404** | 0.14s | **GAP**: Vercel preview not redeployed since #241 (source exists at `apps/marketing/src/pages/marketplace/index.astro`). Workaround for judges: marketplace CLI verifiable from crates.io (`sbo3l-marketplace` bin), and content-addressed registry verifiable via `@sbo3l/marketplace` package source on GitHub. |
| 7 | Click /submission → see bounty narratives | https://sbo3l-marketing.vercel.app/submission | ✅ HTTP 200 | 0.14s | |
| 8 | Open Etherscan agent wallet | https://sepolia.etherscan.io/address/0xdc7EFA…D231 | ✅ HTTP 200 | 0.89s | (full address per `alchemy_rpc_endpoints` memory) |
| 9 | `cargo install sbo3l-cli --version 1.2.0` | crates.io API | ✅ crate live (yanked=false, 121831 bytes) | — | **DELEGATED**: actual install + run by Daniel |
| 10 | `sbo3l passport resolve sbo3lagent.eth` → 5 records | mainnet ENS | ✅ ENS App 200; 5 records confirmed in earlier rounds | — | **DELEGATED**: actual CLI invocation by Daniel |
| 11 | `sbo3l audit anchor --broadcast --network sepolia` | Sepolia onchain | — | — | **DELEGATED**: Dev 1's R11 P1 work; requires funded wallet + CLI 1.2.0 installed |
| 12 | `sbo3l reputation publish --multi-chain` | Sepolia + L2s | — | — | **DELEGATED**: Dev 4's R11 P2 work (#267 just merged); requires funded wallet + CLI 1.2.0 installed |

## Findings

### 🔴 Blockers / 🟡 Gaps for Daniel

| # | Severity | Step | Issue | Mitigation in inventory? |
|---|---|---|---|---|
| 1 | 🔴 | 6 | `/marketplace` 404 — Vercel preview hasn't redeployed since #241 + #150 + #211 merged | ✅ documented in live-url-inventory.md "Known gaps" |
| 2 | 🟡 | 4-5, 9-12 | 6 of 12 walkthrough steps require browser/CLI interaction Heidi cannot perform statically | ✅ Daniel performs hands-on walk before hitting submit |

### ✅ Confirmed working

| # | What | Surface |
|---|---|---|
| 1 | Marketing site root | https://sbo3l-marketing.vercel.app/ |
| 2 | All 4 demo step pages | `/demo/{1-meet-the-agents,2-watch-a-decision,3-verify-yourself,4-explore-the-trust-graph}` |
| 3 | /proof page WASM verifier shell | `/proof` (interactive verification gated to Daniel hands-on) |
| 4 | /submission judges entry | `/submission` |
| 5 | Mainnet ENS apex `sbo3lagent.eth` | https://app.ens.domains/sbo3lagent.eth |
| 6 | Etherscan agent wallet | Sepolia agent wallet 200 |
| 7 | sbo3l-cli 1.2.0 on crates.io | Direct cargo-install path |

## Daniel's hands-on completion checklist (≤ 8 min)

Before hitting submit, walk these 6 interactive steps end-to-end in a fresh browser tab:

1. ☐ Open https://sbo3l-marketing.vercel.app/ — confirm hero loads.
2. ☐ Click "Demo" — walk `/demo/1` → `/demo/2` → `/demo/3` → `/demo/4`.
3. ☐ At `/proof`, drop `test-corpus/passport/v2-capsule.json` — confirm 6/6 ✅.
4. ☐ At `/proof`, paste a tampered capsule (flip 1 byte in `audit_chain[0].payload_hash`) — confirm ❌.
5. ☐ `cargo install sbo3l-cli --version 1.2.0` — confirm `sbo3l --version` → `sbo3l 1.2.0`.
6. ☐ `sbo3l passport resolve sbo3lagent.eth` — confirm 5 records.

Optional but powerful (requires funded Sepolia wallet on this machine):
7. ☐ `sbo3l audit anchor --broadcast --network sepolia` — confirm tx hash.
8. ☐ `sbo3l reputation publish --multi-chain` — confirm broadcast across configured L2s.

## Conclusion

**Static rehearsal: PASS** with one documented gap (`/marketplace` 404 — Vercel-redeploy-only fix).

**Daniel-side hands-on rehearsal: REQUIRED** before submit. The 6 interactive steps above cannot be verified by Heidi statically.

If Daniel completes the 6 hands-on steps and the marketplace 404 is acceptable as a documented gap, **Heidi recommends submission-ready**.

See `docs/submission/READY.md` for the formal sign-off.
