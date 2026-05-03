# Scene 5 — Sponsor wins + SDK reach (static slide content)

> **Format:** paste this into one Keynote / Google Slides slide at 1920×1080.
> **Hold time:** 15 s on camera.
> **Layout suggestion:** two columns. Left column = 4 ETHGlobal sponsor
> tracks (KH, ENS, Uniswap, 0G) + 1 SDK adapter row (Anthropic) clearly
> visually separated from the sponsor block. Right column = the four-
> number outro footer (881 / 13 / 25 / 9). Use the SBO3L palette: bg
> `#0a0a0f`, accent `#4ad6a7`, fg `#e6e6ec`.
>
> **Anthropic note for the recording:** Anthropic is NOT an ETHGlobal
> Open Agents 2026 sponsor (no bounty / no prize). The Claude tool-use
> adapter is real and shipped on npm, but it belongs in the "SDK reach"
> bucket, not the sponsor bucket. The slide MUST visually separate them
> (different sub-heading + a divider) so a viewer can't read it as
> "Anthropic gives a prize."
>
> **Voice match:** these bullets mirror the per-bounty docs at
> [`bounty-keeperhub.md`](../bounty-keeperhub.md), [`bounty-ens-most-creative-final.md`](../bounty-ens-most-creative-final.md),
> [`bounty-uniswap.md`](../bounty-uniswap.md), [`bounty-anthropic.md`](../bounty-anthropic.md).
> Numerical claims verified against `main` on 2026-05-03.

---

## Title

> **What SBO3L ships per sponsor**

---

## KeeperHub — execution layer for AI agents

- Real workflow `m4t4cnpmhv8qquce3bv3c` POSTed live during submission window;
  KH-issued `executionId` of the form `kh-172o77rxov7mhwvpssc3x` captured
  into the Passport capsule
- 5 integration paths (IP-1 through IP-5) catalogued in
  [`docs/keeperhub-integration-paths.md`](../../keeperhub-integration-paths.md);
  each independently small + reviewable
- `sbo3l-keeperhub-adapter` standalone crate published at
  [`crates.io/crates/sbo3l-keeperhub-adapter`](https://crates.io/crates/sbo3l-keeperhub-adapter) v1.2.0
- 5 KH improvement issues filed (Builder Feedback bounty)
- MCP `sbo3l.audit_lookup(execution_id)` mirror of the proposed
  `keeperhub.lookup_execution` tool — implemented today

---

## ENS — trust DNS for autonomous agents

- `sbo3lagent.eth` on **Ethereum mainnet** — 7 canonical `sbo3l:*` text
  records (`agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`,
  `capability`, `reputation`)
- 5 named-role agents on Sepolia: `research-agent`, `trading-agent`,
  `swap-agent`, `audit-agent`, `coordinator-agent` — each issued via
  direct ENS Registry `setSubnodeRecord`
- **OffchainResolver** deployed on Sepolia at
  [`0x87e9…b1f6`](https://sepolia.etherscan.io/address/0x87e99508C222c6E419734CACbb6781b8d282b1F6)
  + CCIP-Read gateway live at `sbo3l-ccip.vercel.app`
- ENSIP draft for `reputation_score` text-record convention
- 14 framework adapters consume ENS-resolved agent identity through this stack

---

## Uniswap — Best API track

- Live `quoteExactInputSingle` against Sepolia QuoterV2
  (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`) every swap intent;
  `sqrt_price_x96_after` + freshness timestamp captured into the capsule
- `UniswapExecutor::live_from_env()` in `sbo3l-execution`; offline-mock
  default, live behind two env vars
- Universal Router with **per-step policy gates** (T-5-2, PR #171 merged)
- Smart Wallet integration with **per-call policy gates** (T-5-3, PR #183 merged)
- MEV protection expressed as policy rules (`max_slippage`, `max_quote_age`,
  `allowed_recipients`)

---

## SDK adapter (non-sponsor): Anthropic Claude tool-use

> **Important visual cue when slidedesigning:** put this section
> under its own sub-heading or divider so it's clearly separate
> from the 4 sponsor blocks above. Anthropic does NOT sponsor Open
> Agents 2026 — listing them under the sponsor block would be a
> truthfulness defect.

- `@sbo3l/anthropic` npm package — `sbo3lTool` Claude `Tool` definition +
  `runSbo3lToolUseLoop()` driver for multi-turn tool dispatch
- Every `tool_use` block routes through SBO3L's policy boundary; the
  signed `PolicyReceipt` is returned as the matching `tool_result`
- No prompt re-engineering, no model swap, no SDK fork — adapter is a
  thin Tool definition + dispatcher
- Examples in [`examples/anthropic-research-agent/`](../../../examples/anthropic-research-agent/)
- Counted in the **8 framework integrations** number below

---

## 0G — Storage / DA / Compute (Phase 3)

- Capsule storage on 0G Storage (Track A) — capsule URI written to ENS
  text record `sbo3l:proof_uri` resolves to a 0G Storage chunk
- Audit-chain checkpoints anchored via 0G DA (Track B); 0G Compute used
  for batch capsule re-verification
- **Honesty note:** 0G integration is Phase 3 (post-submission window) —
  scoped, in-flight, NOT live at hackathon close. See
  [`docs/submission/ETHGlobal-form-content.md`](../ETHGlobal-form-content.md)
  for the current 0G track field. Faucet rate-limit workaround documented;
  testnet wallet provisioned.

---

## Outro footer (right column)

> **881** tests · **13** demo gates · **25** npm packages · **9** crates

(Numbers verified against `main` 2026-05-03. `cargo test --workspace --tests`
= 881/881 green. `bash demo-scripts/run-openagents-final.sh` = 13/13 gates.
`@sbo3l/*` npm scope = 25 packages. crates.io `sbo3l-*` = 9 crates at v1.2.0.)

---

## Tagline (bottom of slide, 32pt, accent mint)

> **Don't give your agent a wallet. Give it a mandate.**

---

## Optional: 5-second cross-fade text (alternate slide)

If Daniel wants a second slide for visual rhythm, use this short version:

```
KeeperHub  →  real workflow live · 5 integration paths · adapter on crates.io
ENS        →  sbo3lagent.eth mainnet · OffchainResolver on Sepolia · 14 adapters
Uniswap    →  Sepolia QuoterV2 live · Universal Router gated · Smart Wallet gated
Anthropic  →  @sbo3l/anthropic on npm · Claude tool-use loop with signed receipts
0G         →  Phase 3 (storage + DA + compute) — scoped, in-flight, not live yet
```

Hold for 7 s, cross-fade into the main slide for the remaining 8 s.
