# Live URL inventory

> Every public surface SBO3L ships, with smoke-tested status. **If it isn't on this page, it isn't claimed.**
>
> Status legend: ✅ live (HTTP 200 verified) · 🟢 API-verified (web is SPA/bot-blocked but install/registry API confirms) · 🟡 reachable but content not yet final · 🔴 not yet live · ❌ broken
>
> Smoke timestamp: **2026-05-01 ~23:05 CEST** (re-verify before submission). Re-run via the script at the bottom.

## Package registries

Web pages on `crates.io` and `npmjs.com` are JS-rendered SPAs that return 404/403 to plain `curl` — they're **live for browser users + machine consumers**. The `Verify` column is the canonical falsifiable check.

| Surface | URL | Status | Verify (machine API) |
|---|---|---|---|
| crates.io — sbo3l-core | https://crates.io/crates/sbo3l-core | 🟢 1.0.1 | `curl -sf https://crates.io/api/v1/crates/sbo3l-core \| jq -r .crate.max_version` |
| crates.io — sbo3l-storage | https://crates.io/crates/sbo3l-storage | 🟢 1.0.1 | same pattern |
| crates.io — sbo3l-policy | https://crates.io/crates/sbo3l-policy | 🟢 1.0.1 | |
| crates.io — sbo3l-identity | https://crates.io/crates/sbo3l-identity | 🟢 1.0.1 | |
| crates.io — sbo3l-execution | https://crates.io/crates/sbo3l-execution | 🟢 1.0.1 | |
| crates.io — sbo3l-keeperhub-adapter | https://crates.io/crates/sbo3l-keeperhub-adapter | 🟢 1.0.1 | |
| crates.io — sbo3l-server | https://crates.io/crates/sbo3l-server | 🟢 1.0.1 | |
| crates.io — sbo3l-mcp | https://crates.io/crates/sbo3l-mcp | 🟢 1.0.1 | |
| crates.io — sbo3l-cli | https://crates.io/crates/sbo3l-cli | 🟢 1.0.1 | `cargo install sbo3l-cli --version 1.0.1 && sbo3l --version` → `sbo3l 1.0.1` |
| npm — @sbo3l/sdk | https://www.npmjs.com/package/@sbo3l/sdk | 🟢 1.0.0 | `npm view @sbo3l/sdk version` |
| npm — @sbo3l/langchain | https://www.npmjs.com/package/@sbo3l/langchain | 🟢 | `npm view @sbo3l/langchain version` |
| npm — @sbo3l/autogen | https://www.npmjs.com/package/@sbo3l/autogen | 🟢 | `npm view @sbo3l/autogen version` |
| npm — @sbo3l/elizaos | https://www.npmjs.com/package/@sbo3l/elizaos | 🟢 | `npm view @sbo3l/elizaos version` |
| npm — @sbo3l/vercel-ai | https://www.npmjs.com/package/@sbo3l/vercel-ai | 🟢 | `npm view @sbo3l/vercel-ai version` |
| npm — @sbo3l/design-tokens | https://www.npmjs.com/package/@sbo3l/design-tokens | 🟢 | `npm view @sbo3l/design-tokens version` |
| PyPI — sbo3l-sdk | https://pypi.org/project/sbo3l-sdk/ | ✅ 200 (1.0.0) | `pip index versions sbo3l-sdk` |
| PyPI — sbo3l-langchain | https://pypi.org/project/sbo3l-langchain/ | ✅ 200 | same |
| PyPI — sbo3l-crewai | https://pypi.org/project/sbo3l-crewai/ | ✅ 200 | |
| PyPI — sbo3l-llamaindex | https://pypi.org/project/sbo3l-llamaindex/ | ✅ 200 | |
| PyPI — sbo3l-langgraph | https://pypi.org/project/sbo3l-langgraph/ | ✅ 200 | |

## Web surfaces

| Surface | Canonical (custom) | Vercel preview | Status |
|---|---|---|---|
| Marketing site | `https://sbo3l.dev` 🔴 (DNS not pointed) | https://sbo3l-marketing.vercel.app | ✅ 200 |
| `/proof` page (WASM verifier) | `https://sbo3l.dev/proof` 🔴 | `/proof` on the Vercel preview | 🟡 routes 404 on the preview deploy — need confirmation Astro slice with `/proof` + `/submission` is the deployed bundle |
| `/features` page | `https://sbo3l.dev/features` 🔴 | same Vercel preview | 🟡 same as above |
| `/submission` page | `https://sbo3l.dev/submission` 🔴 | same | 🟡 |
| Documentation | `https://docs.sbo3l.dev` 🔴 | _no Vercel preview URL discovered_ | 🔴 |
| Hosted preview | `https://app.sbo3l.dev` 🔴 | https://sbo3l-hosted-app.vercel.app | 🟡 deploy workflow shipped (`.github/workflows/hosted-app.yml`); waits on Daniel's Vercel project + secrets setup (see `apps/hosted-app/DEPLOY.md`) |
| Trust-DNS visualization | `https://app.sbo3l.dev/trust-dns` 🔴 | `https://sbo3l-trust-dns-viz.vercel.app` 🔴 (404) | 🔴 — viz package main not yet deployed; T-3-5 in flight |
| CCIP-Read gateway | `https://ccip.sbo3l.dev` 🔴 | https://sbo3l-ccip.vercel.app ✅ 200; long preview https://sbo3l-ccip-i05tmr4jc-babjak-daniel-5461s-projects.vercel.app ✅ 200 | ✅ via Vercel preview |

**Action item for Daniel:** point CTI-3-1 (`sbo3l.dev` and subdomains) to the Vercel projects so judge-facing URLs are stable. Until then, judge submission should use the working Vercel preview URLs above (or the machine APIs for the registries).

## ENS

| Record | Value | Status | Verify |
|---|---|---|---|
| Mainnet apex | `sbo3lagent.eth` | ✅ live (5 records on chain) | https://app.ens.domains/sbo3lagent.eth (200) or `sbo3l passport resolve sbo3lagent.eth` |
| Mainnet `policy_hash` | `e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf` | ✅ matches offline fixture byte-for-byte | re-derive: `sbo3l policy current --hash` |
| Sepolia parent | `sbo3lagent.eth` (per ENS-parent-decision 2026-05-01 — re-using mainnet parent for Sepolia subnames; new `sbo3l.eth` apex was rejected) | 🟡 fleet PR #138 in flight | |
| Subname pattern | `<name>.sbo3lagent.eth` | 🟡 issued via direct ENS Registry `setSubnodeRecord` (Durin dropped 2026-05-01) | |
| ENS Registry constant | `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` | ✅ deterministic on mainnet + Sepolia | https://etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e |
| Mainnet PublicResolver | `0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63` | ✅ | https://etherscan.io/address/0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63 |

## GitHub

| Surface | URL | Status |
|---|---|---|
| Repo | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 | ✅ 200 |
| Releases (v1.0.0 + v1.0.1) | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases | ✅ 200 |
| `v1.0.1` release page (Phase 1 closeout green table) | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.1 | ✅ 200 |
| `v1.0.0` release page (CHANGELOG + closeout table appended) | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.0 | ✅ 200 |
| GitHub Pages (capsule mirror) | https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/ | _untested in last sweep_ |
| FEEDBACK to KeeperHub | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/FEEDBACK.md | ✅ 200 |
| Phase 2 AC tracker | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/issues/162 | ✅ 200 |

## KeeperHub Builder Feedback issues (T-2-1)

| Issue | Subject |
|---|---|
| https://github.com/KeeperHub/cli/issues/47 | token-prefix naming (`kh_*` vs `wfb_*`) |
| https://github.com/KeeperHub/cli/issues/48 | undocumented submission/result envelope schema |
| https://github.com/KeeperHub/cli/issues/49 | `executionId` lookup undocumented |
| https://github.com/KeeperHub/cli/issues/50 | first-class upstream policy/audit fields on submission |
| https://github.com/KeeperHub/cli/issues/51 | `Idempotency-Key` retry semantics on workflow webhooks |

## Demo / proof artifacts

| Artifact | Location |
|---|---|
| Golden Passport capsule | `test-corpus/passport/v2-capsule.json` |
| Live demo transcript | `demo-scripts/artifacts/latest-demo-summary.json` |
| Operator-console evidence | `demo-scripts/artifacts/latest-operator-evidence.json` |
| Demo video | _Daniel records; URL added at submission time_ |

## Onchain references (Sepolia)

| Tx / contract | Network | Reference |
|---|---|---|
| Sepolia QuoterV2 | Sepolia | `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3` |
| Sepolia USDC | Sepolia | `0x1c7D4B19…` (route used in submission-day verification) |
| Real Sepolia swap (capsule's `tx_hash`) | Sepolia | _populate from `demo-scripts/artifacts/uniswap-real-swap-capsule.json` after T-5-5 lands_ |
| KeeperHub workflow execution | KH (off-chain) | last verified executionId: `kh-172o77rxov7mhwvpssc3x` |
| ENS subname issuance txs | Sepolia | _populate from `demo-fixtures/sepolia-agent-fleet.json` after fleet-of-5 #138 broadcasts_ |

## Re-run the smoke (paste-ready)

```bash
# crates.io machine API (returns 1.0.1 for each)
for c in sbo3l-core sbo3l-storage sbo3l-policy sbo3l-identity sbo3l-execution \
         sbo3l-keeperhub-adapter sbo3l-server sbo3l-mcp sbo3l-cli; do
  printf "%-30s %s\n" "$c" "$(curl -sf https://crates.io/api/v1/crates/$c | jq -r .crate.max_version)"
done

# npm registry API
for p in @sbo3l/sdk @sbo3l/langchain @sbo3l/autogen @sbo3l/elizaos @sbo3l/vercel-ai @sbo3l/design-tokens; do
  printf "%-30s %s\n" "$p" "$(curl -sf https://registry.npmjs.org/$p | jq -r '.["dist-tags"].latest')"
done

# PyPI JSON API
for p in sbo3l-sdk sbo3l-langchain sbo3l-crewai sbo3l-llamaindex sbo3l-langgraph; do
  printf "%-30s %s\n" "$p" "$(curl -sf https://pypi.org/pypi/$p/json | jq -r .info.version)"
done

# Web pages
for u in https://sbo3l-marketing.vercel.app https://sbo3l-ccip.vercel.app \
         https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 \
         https://app.ens.domains/sbo3lagent.eth ; do
  printf "%-90s %s\n" "$u" "$(curl -sk -o /dev/null -w '%{http_code}' -m 10 -L "$u")"
done
```

Expected output (2026-05-01 reference): 9 × `1.0.1`, 6 × npm version (≥ 1.0.0), 5 × PyPI version, all four web URLs `200`.

## Refresh cadence

This page is updated whenever a new surface goes live or a custom domain points. The `regression-on-main.yml` workflow does not currently link-check this file; `scripts/check_live_urls.py` (TODO) will add 200/API-version verification per row and post a delta to coordination if any drops.
