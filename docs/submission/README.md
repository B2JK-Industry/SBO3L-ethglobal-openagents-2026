# SBO3L — ETHGlobal Open Agents 2026 submission package

> **Audience:** ETHGlobal judges + sponsor reviewers (KeeperHub, ENS, Uniswap, 0G, Gensyn).
> **Outcome:** in 5 minutes, a judge has a working SBO3L install, has verified one signed Passport capsule offline, and has links to every live surface the project ships.

This folder is the **single source of truth** for SBO3L's submission. Every external claim made in the ETHGlobal track forms or the demo video maps to a code reference, a live URL, or a runnable command listed here.

## What SBO3L is, in one sentence

**SBO3L is the cryptographically verifiable trust layer for autonomous AI agents.** Every action your agent takes — pay, swap, store, compute, coordinate — passes through SBO3L's policy boundary first; the output is a self-contained Passport capsule that anyone can verify offline.

Tagline (preserved through the rebrand from Mandate): **Don't give your agent a wallet. Give it a mandate.**

## 5-minute judge walkthrough

```bash
# 1. Install the CLI from crates.io
cargo install sbo3l-cli --version 1.2.0

# 2. Run the SBO3L server (any APRP request gets policy-decided + audited)
sbo3l serve --db /tmp/sbo3l-judge.db &
sleep 2

# 3. Submit one APRP, get back a signed receipt
curl -s :8730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" \
  -d '{"schema":"sbo3l.aprp.v1","agent_id":"judge","intent":"transfer","amount":{"value":"0.01","currency":"USDC"},"chain":"sepolia","expiry":"2026-12-31T23:59:59Z","risk_class":"low","nonce":"01HJUDGE00000000000000001"}' \
  | jq

# 4. Capture a self-contained Passport capsule + verify it offline
sbo3l passport run /path/to/aprp.json \
  --executor keeperhub --mode mock \
  --out /tmp/capsule.json
sbo3l passport verify --strict --path /tmp/capsule.json
# expect: PASSED, ZERO SKIPPED checks
```

If the verifier prints `PASSED` with **zero `SKIPPED` checks**, you have just re-derived the policy decision and audit chain from the capsule alone — no daemon, no network, no RPC.

That single line is the load-bearing identity sub-claim:

> **Every agent action leaves a portable, offline-verifiable proof of authorisation.**

Browser version of the same check at https://sbo3l.dev/proof — drop a capsule JSON in, the WASM verifier runs the same byte-for-byte checks in the page.

## What's in this folder

| File | What it is | Audience |
|---|---|---|
| [`README.md`](README.md) | This file — overview + 5-minute walkthrough | judges |
| [`live-url-inventory.md`](live-url-inventory.md) | Every live URL SBO3L ships (cargo, npm, PyPI, marketing, docs, hosted, CCIP, releases) | judges + sponsor reviewers |
| [`demo-video-script.md`](demo-video-script.md) | 3-minute walkthrough script (storyboard + voiceover) | Daniel before recording |
| [`ETHGlobal-form-content.md`](ETHGlobal-form-content.md) | Ready-to-paste form fields per track | Daniel at submission time |
| [`partner-onepagers/keeperhub.md`](partner-onepagers/keeperhub.md) | KeeperHub × SBO3L 1-pager + v1.2.0 install | KH team |
| [`partner-onepagers/ens.md`](partner-onepagers/ens.md) | ENS × SBO3L 1-pager + v1.2.0 install | ENS team |
| [`partner-onepagers/uniswap.md`](partner-onepagers/uniswap.md) | Uniswap × SBO3L 1-pager + v1.2.0 install | Uniswap team |

The detailed integration docs at [`docs/partner-onepagers/`](../partner-onepagers/) remain authoritative for engineering audiences; the versions in this folder are submission-shaped (shorter, install-first, refreshed with v1.2.0 commands).

## What sponsor judges should look at

| Sponsor | Track | What to look at first |
|---|---|---|
| **KeeperHub** | Best Use of KH | [`docs/keeperhub-integration-paths.md`](../keeperhub-integration-paths.md) IP-1..IP-5 + `crates/sbo3l-keeperhub-adapter/` (standalone crate published on crates.io) |
| **KeeperHub** | Builder Feedback | [`FEEDBACK.md`](../../FEEDBACK.md) — concrete pain points + 5+ KH GitHub issues filed |
| **ENS** | Most Creative | https://app.sbo3l.dev/trust-dns (live trust-DNS visualization) + `sbo3lagent.eth` mainnet apex |
| **ENS** | AI Agents | ENSIP-25 CCIP-Read gateway at https://ccip.sbo3l.dev + ERC-8004 Identity Registry integration |
| **Uniswap** | Best API | `examples/uniswap-agent/` + real Sepolia swap with tx hash captured into a capsule |
| **0G** | Track A/B | Capsule storage on 0G + DA + Compute |
| **Gensyn** | AXL | Multi-node SBO3L with Gensyn AXL coordination |

## Verification ground rules

If a claim in any submission form does not appear here with a code reference or live URL, **the claim is wrong** and should be cut. SBO3L's voice is honest-over-slick: every public statement is falsifiable.
