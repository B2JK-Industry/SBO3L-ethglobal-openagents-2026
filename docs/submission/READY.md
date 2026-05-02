# READY — pre-submission sign-off

> **Filed by:** Heidi (QA + Release agent), R12.
> **Date/time:** 2026-05-02 ~14:50 CEST.
> **Repo state:** main HEAD `8da68d5` — `feat(cli): --multi-chain reputation broadcast (R11 P2) + Cargo.toml fix (#267)`.
> **Status:** ⚠️ **READY WITH DOCUMENTED GAPS** — Daniel can submit after the 6-step hands-on rehearsal (see `rehearsal-walkthrough-r12-2026-05-02.md` § "Daniel's hands-on completion checklist").

---

## Pre-flight summary

| Priority | Item | Status |
|---|---|---|
| P1 | GitHub Release v1.2.0 page | ✅ live (Latest) at https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.2.0 |
| P2 | npm publishes (5 integrations + sdk @ 1.2.0) | 🔴 **Daniel-gated**: `NPM_TOKEN` not provisioned. 8 integration tags + sdk-ts-v1.2.0 already pushed; publish fires automatically when secret lands. |
| P3 | `sbo3l-langgraph` PyPI 1.2.0 | 🔴 **Daniel-gated**: PyPI trusted-publisher provisioning needed for this single package (other 4 PyPI integrations work). |
| P4 | Pre-submission rehearsal walkthrough | ✅ static walk PASS (3s); 6 interactive steps **DELEGATED** to Daniel hands-on. |
| P5 | Live URL inventory final pass | ✅ rewritten with honest state — see `live-url-inventory.md` (also documents the gaps Daniel needs to close or accept). |
| P6 | (this doc) | ✅ filed. |

## All confirmed working

### Code + crates
- ✅ **9 Rust crates** at 1.2.0 on crates.io (sbo3l-{core,storage,policy,identity,execution,keeperhub-adapter,server,mcp,cli})
- ✅ **4 Python packages** at 1.2.0 on PyPI (sbo3l-{sdk,langchain,crewai,llamaindex})
- ✅ **318+ tests on main** (per checkpoint memory; runs continue post-cascade)
- ✅ **5/5 chaos scenarios** PASS (proof in `docs/proof/chaos-suite-results-v1.2.0.md`)
- ✅ **`v1.2.0` GitHub Release** as Latest

### Web surfaces (Vercel previews)
- ✅ **Marketing root** https://sbo3l-marketing.vercel.app/
- ✅ **/demo + 4 step pages** `/demo/{1-meet-the-agents,2-watch-a-decision,3-verify-yourself,4-explore-the-trust-graph}`
- ✅ **/proof** (WASM verifier; interactive verification delegated to Daniel hands-on)
- ✅ **/submission** judges entry page
- ✅ **/features** product page
- ✅ **CCIP-Read gateway** https://sbo3l-ccip.vercel.app

### Onchain + ENS
- ✅ **Mainnet ENS apex** `sbo3lagent.eth` — 5 records on chain
- ✅ **Sepolia OffchainResolver** `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` (CCIP-Read flow E2E verified)
- ✅ **Sepolia QuoterV2** (Uniswap path) verified
- ✅ **Etherscan agent wallet** reachable

### Submission package (`docs/submission/`)
- ✅ `README.md` index
- ✅ `judges-walkthrough.md` (5/30/90-min reading paths)
- ✅ `live-url-inventory.md` (R12 final pass)
- ✅ `preflight-2026-05-02.md` (R11 P3)
- ✅ `rehearsal-runbook.md` + `rehearsal-walkthrough-r12-2026-05-02.md`
- ✅ Per-bounty docs: `bounty-{ens-ai-agents,ens-most-creative,keeperhub,uniswap}.md`
- ✅ `ETHGlobal-form-content.md`
- ✅ `demo-video-script.md`

## Known gaps at submission time

Each gap below has a documented workaround judges can use without breaking the bounty narrative.

| Gap | Severity | Daniel action to close | Submission impact if NOT closed |
|---|---|---|---|
| `NPM_TOKEN` not provisioned → 5 integration npm packages 404 + `@sbo3l/sdk@1.0.0` not 1.2.0 | 🟡 Medium | Add `NPM_TOKEN` to repo secrets; publishes fire automatically | TS install path one minor behind; **mitigation**: Python SDK at 1.2.0 + crates.io CLI cover the install story |
| `sbo3l-langgraph` PyPI publisher not provisioned → 404 | 🟢 Low | Create PyPI trusted-publisher for `sbo3l-langgraph` (5 min) | LangGraph integration install-blocked; **mitigation**: 4 other Python integrations work + framework story holds via langchain/crewai/llamaindex |
| `/marketplace` 404 on Vercel preview (source merged, deploy lagging) | 🟡 Medium | Trigger fresh Vercel deploy of `sbo3l-marketing` | Cannot click-through marketplace UI; **mitigation**: `@sbo3l/marketplace` registry + `sbo3l-marketplace` CLI both verifiable from package source |
| `sbo3l-trust-dns-viz.vercel.app` 404 (T-3-5 viz not deployed) | 🟢 Low | Deploy `apps/trust-dns-viz/` to Vercel | Cannot click-through visualization; **mitigation**: source verifiable; canvas renderer (#164) works locally |
| Custom domains (`sbo3l.dev` etc.) DNS not pointed | 🟢 Low | (Optional, post-submission) point CTI-3-1 DNS | Vercel preview URLs are canonical; submission package uses them throughout |
| 6 interactive walkthrough steps not statically verifiable | 🟢 Low | Daniel walks 6 hands-on steps before hitting submit | Heidi cannot drop files into WASM verifier or run installed CLI; **mitigation**: 6-step checklist in rehearsal walkthrough doc |

## Daniel's go/no-go decision

**Heidi recommends GO** if:
1. Daniel completes the 6-step hands-on rehearsal (≤ 8 min — see `rehearsal-walkthrough-r12-2026-05-02.md`).
2. Daniel acknowledges the 5 documented gaps above (or closes any subset of them in the time remaining).

**Heidi recommends NO-GO** only if:
- The 6 hands-on steps reveal a regression Heidi missed statically (e.g. `/proof` WASM verifier broken; CLI install fails).
- A new 🔴 surface goes down between now and submit (Heidi's cascade-watch will fire if so).

## What "Daniel can submit" looks like operationally

After Daniel hits submit:
1. The submission form references **stable** URLs (Vercel previews + crates.io + PyPI + GitHub Release tag — none of those are mutable in the next 48h).
2. The 5 gaps above are honestly documented in `live-url-inventory.md` so judges encountering them have the workaround inline.
3. The `regression-on-main.yml` workflow keeps verifying main health post-submission.
4. Heidi's cascade-watch keeps polling for any regression on documented surfaces.

---

**Daniel can submit.**
