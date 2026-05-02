# R14 final rehearsal walkthrough — 2026-05-02 ~17:25 CEST

> **Round:** R14 — final pre-submission walk after the R13 cascade fully drained.
> **Performed by:** Heidi (QA + Release agent).
> **Mode:** static curl (interactive steps remain Daniel-hands-on).
> **Repo state:** main HEAD `cd0fcfb` (R13 fully landed; 50 PRs in last 24h; cascade clean: 0 open non-draft PRs).
> **Static walk elapsed:** 31s.

## Step-by-step results

### Web surfaces

| # | URL | HTTP | Time | Notes |
|---|---|---|---|---|
| 1 | https://sbo3l-marketing.vercel.app | ✅ 200 | 0.55s | hero loads |
| 2 | /demo | ✅ 200 | 0.41s | 4-step walkthrough hub |
| 3 | /demo/1-meet-the-agents | ✅ 200 | 0.34s | step 1 |
| 4 | /demo/2-watch-a-decision | ✅ 200 | 0.38s | step 2 |
| 5 | /demo/3-verify-yourself | ✅ 200 | 0.49s | step 3 (WASM verifier playground) |
| 6 | /demo/4-explore-the-trust-graph | ✅ 200 | 0.52s | step 4 |
| 7 | /proof | ✅ 200 | 0.52s | drag-drop verifier (Daniel hands-on) |
| 8 | /features | ✅ 200 | 0.52s | product page |
| 9 | /submission | ✅ 200 | 0.45s | judges-tailored entry |
| 10 | /marketplace | 🔴 **404** | 0.28s | **GAP** — Vercel deploy lag (source merged in #241; needs fresh deploy) |
| 11 | sbo3l-ccip.vercel.app | ✅ 200 | 0.21s | CCIP-Read gateway |
| 12 | sbo3l-trust-dns-viz.vercel.app | 🔴 **404** | 0.17s | **GAP** — viz Vercel project not deployed (source ready since #164) |

### Package registries

**Rust crates (crates.io):** 9/9 ✅ at 1.2.0.

| Package | Version |
|---|---|
| sbo3l-core | 1.2.0 ✅ |
| sbo3l-storage | 1.2.0 ✅ |
| sbo3l-policy | 1.2.0 ✅ |
| sbo3l-identity | 1.2.0 ✅ |
| sbo3l-execution | 1.2.0 ✅ |
| sbo3l-keeperhub-adapter | 1.2.0 ✅ |
| sbo3l-server | 1.2.0 ✅ |
| sbo3l-mcp | 1.2.0 ✅ |
| sbo3l-cli | 1.2.0 ✅ |

**npm (@sbo3l scope):** 1/6 ⚠️ — `NPM_TOKEN` not provisioned.

| Package | Version |
|---|---|
| @sbo3l/sdk | 1.0.0 (1.2.0 tag pushed; pending NPM_TOKEN) |
| @sbo3l/langchain | 🔴 404 |
| @sbo3l/autogen | 🔴 404 |
| @sbo3l/elizaos | 🔴 404 |
| @sbo3l/vercel-ai | 🔴 404 |
| @sbo3l/design-tokens | 🔴 404 |

**PyPI (sbo3l_*):** 4/5 ✅ — `sbo3l-langgraph` PyPI publisher not provisioned.

| Package | Version |
|---|---|
| sbo3l-sdk | 1.2.0 ✅ |
| sbo3l-langchain | 1.2.0 ✅ |
| sbo3l-crewai | 1.2.0 ✅ |
| sbo3l-llamaindex | 1.2.0 ✅ |
| sbo3l-langgraph | 🔴 404 |

### ENS + onchain

| Surface | Status |
|---|---|
| Mainnet ENS apex `sbo3lagent.eth` | ✅ 200 (5 records on chain) |
| Mainnet PublicResolver | ✅ 200 |
| Sepolia OffchainResolver `0x7c6913…A8c3` | ✅ 200 |
| Sepolia QuoterV2 (Uniswap path) | ✅ 200 |

### GitHub

| Surface | Status |
|---|---|
| Repo | ✅ 200 |
| v1.2.0 release page (Latest) | ✅ 200 |
| GitHub Pages (capsule mirror) | ✅ 200 |

## Delta from R12 rehearsal

**Things that changed since R12 (2026-05-02 ~14:35 CEST):**

- ✅ All R13 work landed (50 PRs in 24h):
  - SECURITY.md + bug bounty (#281)
  - Compliance posture (#287, 7 docs)
  - Proptest invariants (#289, fixed by hotfix #300)
  - Fuzz harnesses (#293, 5 targets)
  - Mutation testing (#295)
  - Competitive benchmarks (#298, criterion)
  - Plus: 4 new framework adapters (letta+autogpt+babyagi+superagi), Reputation Bond contract (#285), Subname Auction (#283), 21 i18n locales, NameWrapper (#292), Monaco editor (#290), recharts viz (#288), Prometheus exporter (#303), per-tenant billing UI (#297), production-rollout design (#294).
- ✅ Test count bumped: 377 → 777.
- ✅ Cascade fully drained (1 draft PR open: #132).

**Things that remain unchanged (still gaps):**

- 🔴 `/marketplace` 404 on Vercel preview — Daniel needs to trigger fresh deploy.
- 🔴 `sbo3l-trust-dns-viz.vercel.app` 404 — Daniel needs to deploy this Vercel project.
- 🔴 `NPM_TOKEN` not provisioned — 5 npm packages 404, @sbo3l/sdk stale at 1.0.0.
- 🔴 `sbo3l-langgraph` PyPI publisher not provisioned — package 404.

## Daniel's 6-step hands-on completion checklist (≤ 8 min)

(Same as R12 P4 walkthrough; unchanged because the interactive paths remain stable across the R13 cascade.)

1. ☐ Open https://sbo3l-marketing.vercel.app/ — confirm hero loads.
2. ☐ Click "Demo" — walk `/demo/1-meet-the-agents` → 2 → 3 → 4.
3. ☐ At `/proof`, drop `test-corpus/passport/v2_golden_001_minimal.json` — confirm 6/6 ✅.
4. ☐ At `/proof`, paste a tampered capsule (flip 1 byte in `audit_chain[0].payload_hash`) — confirm ❌.
5. ☐ `cargo install sbo3l-cli --version 1.2.0` — confirm `sbo3l --version` → `sbo3l 1.2.0`.
6. ☐ `sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com` — confirm 5 records.

Optional but powerful (requires funded Sepolia wallet):

7. ☐ `sbo3l audit anchor --broadcast --network sepolia` — confirm tx hash.
8. ☐ `sbo3l reputation publish --multi-chain` — confirm broadcast across configured L2s.

## Conclusion

**Static rehearsal: PASS** with 4 documented gaps (marketplace 404, trust-dns-viz 404, NPM_TOKEN, sbo3l-langgraph PyPI). All 4 gaps have judge-facing workarounds in `live-url-inventory.md`.

**Daniel-side hands-on rehearsal: REQUIRED** before submit. The 6 interactive steps above cannot be verified by Heidi statically.

**R14 verdict: SUBMISSION-READY** — main is at 777/777 tests, all R13 work landed cleanly, cascade drained, no regressions in static surfaces from R12.

Heidi recommends GO subject to Daniel's hands-on rehearsal.

See [`READY.md`](READY.md) for the formal sign-off (refreshed for R14).
