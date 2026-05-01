# SBO3L × Uniswap — submission one-pager (v1.0.1)

> **Bounded swap intent. Cryptographic proof of authorisation. Real Sepolia tx.**
> **Audience:** Uniswap team + ETHGlobal judges (Best API).
> Engineering deep-dive at [`docs/partner-onepagers/uniswap.md`](../../partner-onepagers/uniswap.md).

## Try it now (90 seconds)

```bash
cargo install sbo3l-cli --version 1.0.1

# Sepolia QuoterV2 quote — live, no swap yet
SBO3L_UNISWAP_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
sbo3l passport run /path/to/swap-aprp.json \
  --executor uniswap --mode live --quote-only \
  --out /tmp/capsule-uniswap.json
sbo3l passport verify --strict --path /tmp/capsule-uniswap.json
# expect: PASSED, capsule contains the quote

# Real Sepolia swap (after T-5-5)
SBO3L_UNISWAP_TRADING_API_KEY="$UNI_TRADING_API_KEY" \
SBO3L_UNISWAP_PRIVATE_KEY="$SEPOLIA_PRIVATE_KEY" \
sbo3l passport run /path/to/swap-aprp.json \
  --executor uniswap --mode live \
  --out /tmp/capsule-real-swap.json
# capsule.execution.live_evidence.tx_hash → real Sepolia Etherscan link
```

## What goes through SBO3L

```
agent  ──APRP{intent:"swap",input,output,slippage_bps,…}──▶
          [SBO3L policy boundary]
            ├─ token_allowlist gate
            ├─ value_cap gate
            ├─ slippage_bps ≤ policy.max_slippage gate
            ├─ MEV: priority_fee bound + quote freshness gate
            └─ multi-scope budget (per_tx, daily, monthly, per_provider)
          ──signed PolicyReceipt──▶  [UniswapExecutor::live_from_env]
            ├─ QuoterV2.quoteExactInputSingle (Sepolia)
            ├─ SwapRouter02.exactInputSingle (Sepolia)
            └─ tx_hash → captured into Passport capsule
```

Every byte of the swap decision — quote evidence, slippage outcome, tx hash — lives in the capsule. Verify offline tomorrow, an auditor can reconstruct the bounded swap path without trusting our daemon.

## What's in v1.0.1

- `sbo3l-execution` v1.0.1 on crates.io (UniswapExecutor lives here)
- `local_mock()` for CI; `live_from_env()` for real Sepolia
- Sepolia QuoterV2 quote path live since pre-rebrand B7
- Universal Router pattern with per-step policy guards (T-5-2)
- Smart Wallet integration (agent as Smart Account owner — T-5-3)
- MEV protection rules (T-5-4)
- Real Sepolia swap with tx hash in capsule (T-5-5)
- `examples/uniswap-agent/` TS + Py (T-5-6)

## Why "Best API"

A swap executed via the Uniswap API is, today, an opaque tx — the audit trail is whatever the agent code happened to write. SBO3L's Uniswap path makes the audit trail *cryptographic*: same Uniswap API, but every call is gated, signed, and bound to a re-derivable policy decision. The capsule format means the audit travels with the action — across teams, across days, across systems.

## Crates / packages

| Surface | Install | Verify |
|---|---|---|
| Execution crate | `cargo add sbo3l-execution@1.0.1` | https://crates.io/crates/sbo3l-execution |
| CLI | `cargo install sbo3l-cli --version 1.0.1` | `sbo3l --version` |
| Example agents | `cd examples/uniswap-agent && npm install && node main.ts` | |

## Live evidence we'll show judges

- Sepolia tx hash on Etherscan (captured in `demo-scripts/artifacts/uniswap-real-swap-capsule.json`)
- Same capsule re-verified in browser at https://sbo3l.dev/proof
- Tampered capsule → `capsule.live_evidence_mismatch` (visible in red on the proof page)
