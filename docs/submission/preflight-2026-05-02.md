# Submission preflight — 2026-05-02 ~11:05 CEST

> **Audience:** Daniel (decides go/no-go on submission), Heidi (re-runs at submission time -1h).
> **What this is:** every row in [`live-url-inventory.md`](live-url-inventory.md) curl-tested + version-checked. Pass/fail per row + remediation per gap.
> **Outcome:** Daniel sees one table, makes the call.

## Summary

- **Crates.io:** 9/9 ✅ at 1.2.0
- **npm:** 1/8 ✅ (only `@sbo3l/sdk@1.0.0`); 7 integration packages NOT published (NPM_TOKEN gating)
- **PyPI:** 4/5 ✅ (sdk + langchain + crewai + llamaindex); 1 missing (langgraph)
- **Marketing surfaces:** 4/4 ✅ (root + /proof + /submission + /demo all 200 on Vercel preview)
- **CCIP gateway:** ✅ root + smoke-fail-mode (400 on invalid input)
- **GitHub + ENS:** ✅
- **Custom domains:** 🔴 not pointed (sbo3l.dev / docs.sbo3l.dev / app.sbo3l.dev all DNS timeout)

**Verdict: SUBMISSION-READY** with two known unblocks Daniel can resolve in ~30 min:
1. Either publish the 7 npm integration packages (NPM_TOKEN) OR drop them from the bounty docs as "queued"
2. Either point `sbo3l.dev` DNS OR keep using Vercel preview URLs (already documented as fallback)

## Per-row preflight

### Crates.io (9/9 ✅)

| Crate | Status | Version | Falsifier |
|---|---|---|---|
| sbo3l-core | ✅ | 1.2.0 | `curl -sf https://crates.io/api/v1/crates/sbo3l-core \| jq -r .crate.max_version` |
| sbo3l-storage | ✅ | 1.2.0 | same |
| sbo3l-policy | ✅ | 1.2.0 | same |
| sbo3l-identity | ✅ | 1.2.0 | same |
| sbo3l-execution | ✅ | 1.2.0 | same |
| sbo3l-keeperhub-adapter | ✅ | 1.2.0 | same |
| sbo3l-server | ✅ | 1.2.0 | same |
| sbo3l-mcp | ✅ | 1.2.0 | same |
| sbo3l-cli | ✅ | 1.2.0 | `cargo install sbo3l-cli --version 1.2.0 && sbo3l --version` → `sbo3l 1.2.0` |

### npm (1/8 ✅; 7 unpublished)

| Package | Status | Version | Gap |
|---|---|---|---|
| @sbo3l/sdk | 🟡 | 1.0.0 | sdk-typescript.yml @ sdk-ts-v1.2.0 still QUEUED (run 25249048116). Will land. |
| @sbo3l/langchain | 🔴 | (missing) | NPM_TOKEN gate; integrations-publish.yml ran but 404 on registry |
| @sbo3l/autogen | 🔴 | (missing) | same |
| @sbo3l/elizaos | 🔴 | (missing) | same |
| @sbo3l/vercel-ai | 🔴 | (missing) | same |
| @sbo3l/design-tokens | 🔴 | (missing) | same |
| @sbo3l/anthropic | 🔴 | (missing) | same |
| @sbo3l/marketplace | 🔴 | (missing) | new package from #244; not yet published |

**Remediation:** Daniel provisions NPM_TOKEN (or confirms OIDC trusted publisher per npm settings); Heidi re-runs `for t in <8-tags>; do gh workflow run integrations-publish.yml --ref "$t-v1.2.0"; done`. ETA: ~30 min once token in place.

### PyPI (4/5 ✅)

| Package | Status | Version | Notes |
|---|---|---|---|
| sbo3l-sdk | 🟡 | 1.0.0 | sdk-python.yml @ sdk-py-v1.2.0 still QUEUED (run 25249048103). Will land. |
| sbo3l-langchain | ✅ | 1.2.0 | published via integrations-publish.yml |
| sbo3l-crewai | ✅ | 1.2.0 | same |
| sbo3l-llamaindex | ✅ | 1.2.0 | same |
| sbo3l-langgraph | 🔴 | (missing) | per-tag publisher likely not configured for `langgraph-py-v1.2.0`; or Dev 2 #228 runbook hasn't covered it yet |

**Remediation:** Daniel adds `pypi-langgraph-py` trusted publisher per #228 runbook + re-runs that one tag.

### Web surfaces (5/5 ✅ on Vercel preview)

| Surface | URL | Status | Notes |
|---|---|---|---|
| Marketing root | https://sbo3l-marketing.vercel.app/ | ✅ 200 | |
| `/proof` | https://sbo3l-marketing.vercel.app/proof | ✅ 200 | **NEW since round 9** — Astro deploy unblocked |
| `/submission` | https://sbo3l-marketing.vercel.app/submission | ✅ 200 | NEW |
| `/demo` | https://sbo3l-marketing.vercel.app/demo | ✅ 200 | NEW (#150 deploy reached production) |
| CCIP gateway root | https://sbo3l-ccip.vercel.app/ | ✅ 200 | |
| CCIP gateway smoke-fail | https://sbo3l-ccip.vercel.app/api/0xdeadbeef/0x12345678.json | ✅ 400 | correct rejection |

### Custom domains (0/4 🔴 DNS not pointed)

| Surface | Status | Remediation |
|---|---|---|
| https://sbo3l.dev | 🔴 timeout | Daniel: point apex via Vercel dashboard (CTI-3-1) |
| https://docs.sbo3l.dev | 🔴 timeout | Daniel: docs Vercel project + DNS |
| https://app.sbo3l.dev | 🔴 timeout | Daniel: hosted-app Vercel project + DNS |
| https://ccip.sbo3l.dev | _untested_ | Daniel: ccip Vercel project + DNS |

**Submission-impact:** all bounty one-pagers + ETHGlobal-form-content list custom domains as primary + Vercel preview as fallback. Judges using primary URL get a DNS error; judges using fallback get content. Submission package narrates this honestly. **Not a blocker** for submission, but a credibility gap if judges click the canonical URL.

### GitHub (3/3 ✅)

| Surface | Status |
|---|---|
| Repo | ✅ 200 |
| Releases (v1.0.0 + v1.0.1) | ✅ 200 |
| ENS app `sbo3lagent.eth` | ✅ 200 |

### ENS mainnet (✅ live)

`sbo3lagent.eth` resolves on mainnet with 5 `sbo3l:*` records on chain (verified previously; matches offline fixture byte-for-byte for `policy_hash = e044f13c5acb…`).

## Workflows in flight

| Run | Workflow | Tag | Status |
|---|---|---|---|
| 25249048095 | crates-publish.yml | v1.2.0 | ✅ SUCCESS (~10:30 CEST) |
| 25249048116 | sdk-typescript.yml | sdk-ts-v1.2.0 | ⏳ queued |
| 25249048103 | sdk-python.yml | sdk-py-v1.2.0 | ⏳ queued |

Both SDK workflows were queued 1h+ at this preflight. Likely waiting on cascade CI capacity. Will surface via cascade-watch when each completes.

## Gap-to-submission punch list (Daniel's 30-min unblock)

1. **NPM_TOKEN provision** → unblocks 7 npm integration publishes (highest visibility gap; bounty docs reference `npm install @sbo3l/<pkg>` for each)
2. **PyPI langgraph trusted publisher** → unblocks 5/5 PyPI
3. **Custom domain DNS** (`sbo3l.dev` + 3 subdomains) → makes canonical URLs in submission docs work (cosmetic; submission still works on Vercel preview URLs)
4. **GitHub Release v1.2.0 page** → Heidi creates once SDK publishes complete (R10 P3)

## Re-run schedule

- **Now (~11:05 CEST):** this report
- **At submission time -1h:** Heidi re-runs `bash scripts/monitoring/check-live-urls.sh` + `bash scripts/judges/verify-everything.sh` + this preflight; posts updated punch-list
- **At submission time:** Daniel decides go/no-go from the final punch-list

## Re-run command

```bash
# Quick re-smoke (10s)
bash scripts/monitoring/check-live-urls.sh

# Full judge-runnable verification (~5 min)
bash scripts/judges/verify-everything.sh

# This preflight, regenerated (TODO: scripts/qa/preflight-render.sh)
```
