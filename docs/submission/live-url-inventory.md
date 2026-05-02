# Live URL inventory

> Every public surface SBO3L ships, with smoke-tested status. **If it isn't on this page, it isn't claimed.**
>
> Status legend: ✅ live (HTTP 200 verified) · 🟢 API-verified (web is SPA/bot-blocked but install/registry API confirms) · 🟡 reachable but content not yet final · 🔴 not yet live · ❌ broken
>
> Smoke timestamp: **2026-05-02 ~14:35 CEST** (R12 final pre-submission pass). Re-run via the script at the bottom.

## Package registries

Web pages on `crates.io` and `npmjs.com` are JS-rendered SPAs that return 404/403 to plain `curl` — they're **live for browser users + machine consumers**. The `Verify` column is the canonical falsifiable check.

### Rust crates — crates.io (9/9 ✅ at 1.2.0)

| Surface | URL | Status | Verify (machine API) |
|---|---|---|---|
| crates.io — sbo3l-core | https://crates.io/crates/sbo3l-core | 🟢 1.2.0 | `curl -sf https://crates.io/api/v1/crates/sbo3l-core \| jq -r .crate.max_version` |
| crates.io — sbo3l-storage | https://crates.io/crates/sbo3l-storage | 🟢 1.2.0 | same pattern |
| crates.io — sbo3l-policy | https://crates.io/crates/sbo3l-policy | 🟢 1.2.0 | |
| crates.io — sbo3l-identity | https://crates.io/crates/sbo3l-identity | 🟢 1.2.0 | |
| crates.io — sbo3l-execution | https://crates.io/crates/sbo3l-execution | 🟢 1.2.0 | |
| crates.io — sbo3l-keeperhub-adapter | https://crates.io/crates/sbo3l-keeperhub-adapter | 🟢 1.2.0 | |
| crates.io — sbo3l-server | https://crates.io/crates/sbo3l-server | 🟢 1.2.0 | |
| crates.io — sbo3l-mcp | https://crates.io/crates/sbo3l-mcp | 🟢 1.2.0 | |
| crates.io — sbo3l-cli | https://crates.io/crates/sbo3l-cli | 🟢 1.2.0 | `cargo install sbo3l-cli --version 1.2.0 && sbo3l --version` → `sbo3l 1.2.0` |

### npm — @sbo3l/* scope (1/6 published; **NPM_TOKEN not provisioned**)

| Surface | URL | Status | Verify (machine API) |
|---|---|---|---|
| npm — @sbo3l/sdk | https://www.npmjs.com/package/@sbo3l/sdk | 🟡 1.0.0 (1.2.0 tag pushed; publish queued — pending NPM_TOKEN) | `npm view @sbo3l/sdk version` → 1.0.0 |
| npm — @sbo3l/langchain | https://www.npmjs.com/package/@sbo3l/langchain | 🔴 404 (never published — pending NPM_TOKEN) | `curl -sf https://registry.npmjs.org/@sbo3l/langchain` → 404 |
| npm — @sbo3l/autogen | https://www.npmjs.com/package/@sbo3l/autogen | 🔴 404 (pending NPM_TOKEN) | same |
| npm — @sbo3l/elizaos | https://www.npmjs.com/package/@sbo3l/elizaos | 🔴 404 (pending NPM_TOKEN) | |
| npm — @sbo3l/vercel-ai | https://www.npmjs.com/package/@sbo3l/vercel-ai | 🔴 404 (pending NPM_TOKEN) | |
| npm — @sbo3l/design-tokens | https://www.npmjs.com/package/@sbo3l/design-tokens | 🔴 404 (pending NPM_TOKEN) | |

> **Daniel action item:** add `NPM_TOKEN` to repo secrets. The 8 integration tags + sdk-ts-v1.2.0 tag are already pushed; publish workflow will fire automatically once the secret lands.

### PyPI — sbo3l_* (4/5 ✅ at 1.2.0)

| Surface | URL | Status | Verify (machine API) |
|---|---|---|---|
| PyPI — sbo3l-sdk | https://pypi.org/project/sbo3l-sdk/ | ✅ 200 (1.2.0) | `curl -sf https://pypi.org/pypi/sbo3l-sdk/json \| jq -r .info.version` |
| PyPI — sbo3l-langchain | https://pypi.org/project/sbo3l-langchain/ | ✅ 200 (1.2.0) | same |
| PyPI — sbo3l-crewai | https://pypi.org/project/sbo3l-crewai/ | ✅ 200 (1.2.0) | |
| PyPI — sbo3l-llamaindex | https://pypi.org/project/sbo3l-llamaindex/ | ✅ 200 (1.2.0) | |
| PyPI — sbo3l-langgraph | https://pypi.org/project/sbo3l-langgraph/ | 🔴 404 (never published — pending PyPI publisher provisioning for `sbo3l-langgraph`) | |

> **Daniel action item:** provision PyPI trusted publisher for `sbo3l-langgraph` (other 4 PyPI packages already work via OIDC). The `langgraph-py-v1.2.0` tag is already pushed; publish fires automatically once provisioning is done.

## Web surfaces

| Surface | Canonical (custom) | Vercel preview | Status |
|---|---|---|---|
| Marketing site / | `https://sbo3l.dev` 🔴 (DNS not pointed) | https://sbo3l-marketing.vercel.app | ✅ 200 |
| `/proof` page (WASM verifier) | `https://sbo3l.dev/proof` 🔴 | https://sbo3l-marketing.vercel.app/proof | ✅ 200 |
| `/features` page | `https://sbo3l.dev/features` 🔴 | https://sbo3l-marketing.vercel.app/features | ✅ 200 |
| `/submission` page (judges entry) | `https://sbo3l.dev/submission` 🔴 | https://sbo3l-marketing.vercel.app/submission | ✅ 200 |
| `/demo` page | `https://sbo3l.dev/demo` 🔴 | https://sbo3l-marketing.vercel.app/demo | ✅ 200 |
| `/marketplace` page | `https://sbo3l.dev/marketplace` 🔴 | https://sbo3l-marketing.vercel.app/marketplace | 🔴 404 (Vercel not redeployed since #241 — source exists at `apps/marketing/src/pages/marketplace/index.astro`) |
| Documentation | `https://docs.sbo3l.dev` 🔴 | _no Vercel preview URL discovered_ | 🔴 |
| Hosted preview | `https://app.sbo3l.dev` 🔴 | _no Vercel project deployed_ | 🔴 (deploy workflow shipped in #229; awaits Daniel's Vercel project + secrets) |
| Trust-DNS visualization | `https://app.sbo3l.dev/trust-dns` 🔴 | https://sbo3l-trust-dns-viz.vercel.app | 🔴 404 — viz package not yet deployed (T-3-5 source merged in #164 + #181) |
| CCIP-Read gateway | `https://ccip.sbo3l.dev` 🔴 | https://sbo3l-ccip.vercel.app | ✅ 200 |

> **Daniel action items (web surfaces):**
> 1. Trigger fresh Vercel deploy of `sbo3l-marketing` to pick up `/marketplace` + Phase 3 routes (#150, #211, #241).
> 2. Deploy `apps/trust-dns-viz` to `sbo3l-trust-dns-viz.vercel.app`.
> 3. (Optional, post-submission) point CTI-3-1 (`sbo3l.dev` and subdomains) to the Vercel projects so judge-facing URLs are stable. Until then, judge submission **uses the working Vercel preview URLs above** (or the machine APIs for the registries).

## ENS

| Record | Value | Status | Verify |
|---|---|---|---|
| Mainnet apex | `sbo3lagent.eth` | ✅ live (5 records on chain) | https://app.ens.domains/sbo3lagent.eth (200) or `sbo3l passport resolve sbo3lagent.eth` |
| Mainnet `policy_hash` | `e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf` | ✅ matches offline fixture byte-for-byte | re-derive: `sbo3l policy current --hash` |
| Sepolia parent | `sbo3lagent.eth` (per ENS-parent-decision 2026-05-01 — re-using mainnet parent for Sepolia subnames) | 🟡 fleet-of-5 in flight | |
| Subname pattern | `<name>.sbo3lagent.eth` | 🟡 issued via direct ENS Registry `setSubnodeRecord` (Durin dropped 2026-05-01) | |
| ENS Registry constant | `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` | ✅ deterministic on mainnet + Sepolia | https://etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e |
| Mainnet PublicResolver | `0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63` | ✅ | https://etherscan.io/address/0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63 |
| Sepolia OffchainResolver (CCIP-Read) | `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` | ✅ | https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3 |

## GitHub

| Surface | URL | Status |
|---|---|---|
| Repo | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 | ✅ 200 |
| Releases page | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases | ✅ 200 |
| `v1.2.0` release page (Phase 2 closeout — **Latest**) | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.2.0 | ✅ 200 |
| `v1.0.1` release page (Phase 2 ENS integration patch) | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.1 | ✅ 200 |
| `v1.0.0` release page (Phase 1 closeout) | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.0 | ✅ 200 |
| GitHub Pages (capsule mirror) | https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/ | ✅ 200 |
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
| Chaos suite proof | `docs/proof/chaos-suite-results-v1.2.0.md` |
| Demo video | _Daniel records; URL added at submission time_ |

## Onchain references

| Tx / contract | Network | Reference |
|---|---|---|
| Sepolia QuoterV2 (Uniswap) | Sepolia | `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3` ([Etherscan](https://sepolia.etherscan.io/address/0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3)) |
| Sepolia USDC | Sepolia | `0x1c7D4B19…` (route used in submission-day verification) |
| Sepolia OffchainResolver | Sepolia | `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` ([Etherscan](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3)) |
| Real Sepolia swap (capsule's `tx_hash`) | Sepolia | _populate from `demo-scripts/artifacts/uniswap-real-swap-capsule.json` after T-5-5 lands_ |
| KeeperHub workflow execution | KH (off-chain) | last verified executionId: `kh-172o77rxov7mhwvpssc3x` |
| ENS subname issuance txs | Sepolia | _populate from `demo-fixtures/sepolia-agent-fleet.json` after fleet-of-5 broadcasts_ |

## Re-run the smoke (paste-ready)

```bash
# crates.io machine API (returns 1.2.0 for each)
for c in sbo3l-core sbo3l-storage sbo3l-policy sbo3l-identity sbo3l-execution \
         sbo3l-keeperhub-adapter sbo3l-server sbo3l-mcp sbo3l-cli; do
  printf "%-30s %s\n" "$c" "$(curl -sf https://crates.io/api/v1/crates/$c | jq -r .crate.max_version)"
done

# npm registry API (only @sbo3l/sdk currently exists at 1.0.0; rest 404 until NPM_TOKEN provisioned)
for p in @sbo3l/sdk @sbo3l/langchain @sbo3l/autogen @sbo3l/elizaos @sbo3l/vercel-ai @sbo3l/design-tokens; do
  v=$(curl -sf "https://registry.npmjs.org/$p" 2>/dev/null | jq -r '.["dist-tags"].latest // "404"')
  printf "%-30s %s\n" "$p" "$v"
done

# PyPI JSON API (4 of 5 at 1.2.0; sbo3l-langgraph 404 until publisher provisioned)
for p in sbo3l-sdk sbo3l-langchain sbo3l-crewai sbo3l-llamaindex sbo3l-langgraph; do
  v=$(curl -sf "https://pypi.org/pypi/$p/json" 2>/dev/null | jq -r '.info.version // "404"')
  printf "%-30s %s\n" "$p" "$v"
done

# Web pages
for u in \
  https://sbo3l-marketing.vercel.app \
  https://sbo3l-marketing.vercel.app/demo \
  https://sbo3l-marketing.vercel.app/proof \
  https://sbo3l-marketing.vercel.app/features \
  https://sbo3l-marketing.vercel.app/submission \
  https://sbo3l-marketing.vercel.app/marketplace \
  https://sbo3l-ccip.vercel.app \
  https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 \
  https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.2.0 \
  https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/ \
  https://app.ens.domains/sbo3lagent.eth ; do
  printf "%-90s %s\n" "$u" "$(curl -sk -o /dev/null -w '%{http_code}' -m 10 -L "$u")"
done
```

Expected output (2026-05-02 R12 reference):
- 9 × `1.2.0` (crates.io)
- npm: `@sbo3l/sdk` → 1.0.0; rest → 404 (gated on `NPM_TOKEN`)
- PyPI: 4 × `1.2.0`; `sbo3l-langgraph` → 404 (gated on publisher provisioning)
- Web pages: all `200` **except** `/marketplace` 404 (Vercel needs redeploy)

## Refresh cadence

This page is updated whenever a new surface goes live or a custom domain points. The `regression-on-main.yml` workflow does not currently link-check this file; `scripts/check_live_urls.py` (TODO) will add 200/API-version verification per row and post a delta to coordination if any drops.

## Known gaps at submission time (2026-05-02 final closeout)

> **Major gap closures since R12 inventory** (re-verified by Heidi 2026-05-02 ~17:30 CEST):
> - ✅ `/marketplace` now **HTTP 200** (was 404 — Vercel redeployed)
> - ✅ `sbo3l-langgraph` 1.2.0 **LIVE on PyPI** (publisher provisioned)
> - ✅ `@sbo3l/{langchain,autogen,elizaos}` 1.2.0 **LIVE on npm** (NPM_TOKEN provisioned)
> - ✅ 4 new Sepolia contracts deployed (AnchorRegistry, SubnameAuction, ReputationBond, ReputationRegistry — all bytecode verified onchain)

**Remaining gaps** — each has a judge-facing workaround:

| Gap | Impact | Workaround for judges |
|---|---|---|
| `@sbo3l/sdk` still at 1.0.0 (not 1.2.0) | TS SDK base package one minor behind | Framework integrations all at 1.2.0; Python SDK + CLI at 1.2.0 cover full install path |
| `@sbo3l/{vercel-ai,design-tokens,marketplace,anthropic,openai-assistants,...}` 404 | Newer/peripheral npm packages not yet published | 3 core integrations live (langchain, autogen, elizaos) |
| `sbo3l-{agno,pydantic-ai,...}` (newer PyPI integrations beyond top-5) | Newer integrations install-blocked | Top-5 Python integrations all live (sdk + langchain + crewai + llamaindex + langgraph) |
| `sbo3l-trust-dns-viz.vercel.app` 404 | Cannot click-through visualization | Source live at `apps/trust-dns-viz/`; canvas renderer verifiable from local build |
| Custom domains (`sbo3l.dev` etc.) DNS not pointed | Vercel preview URLs are the canonical live URLs | Submission uses Vercel preview URLs throughout |
| `/admin` 404 on marketing site | Admin lives at hosted-app, not marketing | hosted-app has `/admin/*`; deploy to its own Vercel project (gated on Daniel signing up) |

## Sepolia smart contracts deployed 2026-05-02 (R14)

| Contract | Address | Etherscan | Bytes |
|---|---|---|---|
| AnchorRegistry | `0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac` | [link](https://sepolia.etherscan.io/address/0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac) | 1653 |
| SBO3LSubnameAuction | `0x5dE75E64739A95701367F3Ad592e0b674b22114B` | [link](https://sepolia.etherscan.io/address/0x5dE75E64739A95701367F3Ad592e0b674b22114B) | 4466 |
| SBO3LReputationBond | `0x75072217B43960414047c362198A428f0E9793dA` | [link](https://sepolia.etherscan.io/address/0x75072217B43960414047c362198A428f0E9793dA) | 2683 |
| SBO3LReputationRegistry | `0x6aA95d8126B6221607245c068483fa5008F36dc2` | [link](https://sepolia.etherscan.io/address/0x6aA95d8126B6221607245c068483fa5008F36dc2) | 3011 |

Deployer wallet: `0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c`. Total Sepolia gas spent: ~0.000003 ETH.
ReputationBond: slasher = deployer; insuranceBeneficiary = Daniel's wallet `0xdc7EFA6b…D231`.

PyPI `sbo3l-langgraph` @ 1.2.0 LIVE (2026-05-02). NPM_TOKEN provisioned 2026-05-02 — npm publishes queued.
