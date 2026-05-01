# Live URL inventory

> Every public surface SBO3L ships. **If it isn't on this page, it isn't claimed.** Updated whenever a new surface goes live; refreshed for v1.0.1.

## Package registries

| Surface | URL | Verify |
|---|---|---|
| crates.io — sbo3l-core | https://crates.io/crates/sbo3l-core | `cargo search sbo3l-core \| grep "^sbo3l-core ="` |
| crates.io — sbo3l-storage | https://crates.io/crates/sbo3l-storage | same |
| crates.io — sbo3l-policy | https://crates.io/crates/sbo3l-policy | same |
| crates.io — sbo3l-identity | https://crates.io/crates/sbo3l-identity | same |
| crates.io — sbo3l-execution | https://crates.io/crates/sbo3l-execution | same |
| crates.io — sbo3l-keeperhub-adapter | https://crates.io/crates/sbo3l-keeperhub-adapter | same |
| crates.io — sbo3l-server | https://crates.io/crates/sbo3l-server | same |
| crates.io — sbo3l-mcp | https://crates.io/crates/sbo3l-mcp | same |
| crates.io — sbo3l-cli | https://crates.io/crates/sbo3l-cli | `cargo install sbo3l-cli --version 1.0.1 && sbo3l --version` |
| npm — @sbo3l/sdk | https://www.npmjs.com/package/@sbo3l/sdk | `npm view @sbo3l/sdk version` |
| npm — @sbo3l/langchain | https://www.npmjs.com/package/@sbo3l/langchain | `npm view @sbo3l/langchain version` |
| npm — @sbo3l/autogen | https://www.npmjs.com/package/@sbo3l/autogen | `npm view @sbo3l/autogen version` |
| npm — @sbo3l/elizaos | https://www.npmjs.com/package/@sbo3l/elizaos | `npm view @sbo3l/elizaos version` |
| npm — @sbo3l/vercel-ai | https://www.npmjs.com/package/@sbo3l/vercel-ai | `npm view @sbo3l/vercel-ai version` |
| npm — @sbo3l/design-tokens | https://www.npmjs.com/package/@sbo3l/design-tokens | `npm view @sbo3l/design-tokens version` |
| PyPI — sbo3l-sdk | https://pypi.org/project/sbo3l-sdk/ | `pip index versions sbo3l-sdk` |
| PyPI — sbo3l-langchain | https://pypi.org/project/sbo3l-langchain/ | same |
| PyPI — sbo3l-crewai | https://pypi.org/project/sbo3l-crewai/ | same |
| PyPI — sbo3l-llamaindex | https://pypi.org/project/sbo3l-llamaindex/ | same |
| PyPI — sbo3l-langgraph | https://pypi.org/project/sbo3l-langgraph/ | same |

## Web surfaces

| Surface | URL | What it is |
|---|---|---|
| Marketing site | https://sbo3l.dev | Pitch + numbers + 5-minute walkthrough |
| Public proof page | https://sbo3l.dev/proof | Drop a capsule JSON → WASM verifier runs offline checks in browser |
| `/features` reproducibility | https://sbo3l.dev/features | Feature pillars with `file:line` evidence references |
| Trust-DNS story | https://sbo3l.dev/trust-dns-story | The narrative for ENS Most Creative |
| Documentation | https://docs.sbo3l.dev | Astro Starlight site; `/concepts/`, `/sdks/`, `/reference/` |
| Hosted preview | https://app.sbo3l.dev | Next.js + NextAuth dashboard skeleton |
| Trust-DNS visualization | https://app.sbo3l.dev/trust-dns | Live D3 force-directed graph of agent fleet |
| Login | https://app.sbo3l.dev/login | GitHub OAuth |
| CCIP-Read gateway | https://ccip.sbo3l.dev | ENSIP-25 off-chain text-record resolver |

## ENS

| Record | Value | Verify |
|---|---|---|
| Mainnet apex | `sbo3lagent.eth` | `dig +short sbo3lagent.eth via ENS gateway` or use any ENS resolver |
| Sepolia parent | `sbo3l.eth` (per Phase 2 plan; `sbo3lagent.eth` if pivoted) | `sbo3l passport resolve sbo3lagent.eth` |
| Subname pattern | `<name>.sbo3lagent.eth` | issued via Durin contract (T-3-1) |

## GitHub

| Surface | URL |
|---|---|
| Repo | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 |
| Releases | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases |
| GitHub Pages (capsule mirror) | https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/ |
| FEEDBACK to KeeperHub | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/FEEDBACK.md |
| KH GitHub issues filed | _populate from FEEDBACK.md after T-2-1 lands_ |

## Demo / proof artifacts

| Artifact | Location |
|---|---|
| Golden Passport capsule | `test-corpus/passport/v2-capsule.json` |
| Live demo transcript | `demo-scripts/artifacts/latest-demo-summary.json` |
| Operator-console evidence | `demo-scripts/artifacts/latest-operator-evidence.json` |
| Demo video | _Daniel records; URL added at submission time_ |

## Onchain references

| Tx / contract | Network | Etherscan |
|---|---|---|
| Real Sepolia swap (capsule's `tx_hash`) | Sepolia | _populate from `demo-scripts/artifacts/uniswap-real-swap-capsule.json`_ |
| KeeperHub workflow execution | KH | `kh-<ULID>` from latest demo run |
| ENS subname issuance txs | Sepolia | _populate from `demo-fixtures/sepolia-agent-fleet.json`_ |

## Refresh cadence

This page is updated after every cascade merge that adds or moves a public surface. The `regression-on-main.yml` workflow does not currently link-check this file; `scripts/check_live_urls.py` (TODO) will add curl+200 verification per row.
