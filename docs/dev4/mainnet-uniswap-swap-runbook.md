# Mainnet Uniswap swap — Daniel runbook

> **Audience:** Daniel.
> **Outcome:** one live mainnet ETH→USDC swap via the SBO3L
> policy-guarded `sbo3l uniswap swap --broadcast` flow. Captured
> tx hash becomes the live evidence row in the Uniswap bounty
> submission.
> **Cost ceiling:** 0.005 ETH swap input + ~$8-15 gas at 50 gwei
> = **$25 total budget**. Swap output: ~$15 USDC at current rate.
> **Time:** <10 min including pre-flight + verification.
> **Reversibility:** swap is irreversible (mainnet tx). Choose the
> `--amount-in` carefully.

---

## STEP 0 — Pre-flight (read-only, no gas)

```bash
export MAINNET_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/n9pLYLbfcNRkZXVs7Togt
export DANIEL_WALLET=0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231

# Set the mainnet gate FIRST. The CLI rejects every `--network mainnet`
# invocation (including --dry-run) without this — STEPS 0.2 and 1
# below would otherwise fail at the gate check before any quoting
# happens. The gate is the same disclosure pattern audit-anchor /
# verify-ens use; setting it acknowledges "yes I'm intentionally
# touching mainnet."
export SBO3L_ALLOW_MAINNET_TX=1

# 0.1 — Verify Daniel wallet has > 0.013 ETH (0.005 swap + 0.008 gas headroom)
cast balance "$DANIEL_WALLET" --rpc-url "$MAINNET_RPC_URL"
# expect: > 13000000000000000

# 0.2 — Check current ETH/USDC pool depth (sanity, no commit)
# (Optional — Quoter is read-only; --dry-run never broadcasts.)
sbo3l uniswap swap \
  --network mainnet \
  --amount-in 0.005ETH \
  --token-out USDC \
  --recipient "$DANIEL_WALLET" \
  --rpc-url "$MAINNET_RPC_URL" \
  --dry-run
# expect: human-readable envelope with `expected_amount_out: <N>`
#         where N ≈ 12-18 USDC (depends on ETH/USDC rate)
```

If `expected_amount_out` shows < 8 USDC for 0.005 ETH, ETH
crashed or RPC is misconfigured — STOP and investigate.

---

## STEP 1 — Build + inspect the dry-run envelope (JSON)

`sbo3l uniswap swap` prints a human-readable summary to stdout.
To get the **JSON envelope** for `jq` queries you must pass
`--out <path>`. The flag writes the canonical JSON to disk in
addition to the stdout summary.

```bash
sbo3l uniswap swap \
  --network mainnet \
  --amount-in 0.005ETH \
  --token-out USDC \
  --recipient "$DANIEL_WALLET" \
  --rpc-url "$MAINNET_RPC_URL" \
  --out /tmp/sbo3l-swap-envelope.json \
  --dry-run

# Inspect the envelope. The JSON is FLAT — top-level keys, no
# nested `quote` or `receipt` objects (the swap envelope is
# pure swap calldata + quote metadata; signing happens at
# --broadcast time, not in the dry-run envelope).
jq '{ amount_in_wei, expected_amount_out, amount_out_minimum, slippage_bps, quote_source, deadline_unix }' \
  /tmp/sbo3l-swap-envelope.json
```

What you're looking for in the JSON:

- `amount_in_wei == "5000000000000000"` (= 0.005 ETH; matches `--amount-in`)
- `expected_amount_out` is a non-empty decimal string ≥ 8 (≈ 12-18
  USDC for 0.005 ETH at current rates; treat anything below 8 as a
  red flag)
- `amount_out_minimum` is `expected_amount_out × (1 - slippage_bps/10000)`,
  default slippage 50 bps = 0.5%
- `quote_source` starts with `uniswap-v3-quoter-mainnet-` (proves
  the quote came from a real on-chain Quoter call, not a stale
  stub)
- `deadline_unix` is a future timestamp ~30 min out

If `expected_amount_out == "0"` and `quote_source` says
`no-quote`, the CLI couldn't reach the live Quoter — confirm
`--rpc-url` is set and the RPC is reachable, then retry.

The dry-run envelope does NOT include a signed receipt or a
`decision` field — those live at the policy-engine layer (a
separate flow). The envelope is pure swap calldata + quote
metadata; Daniel's policy decision happens by inspecting this
output and choosing whether to invoke `--broadcast`.

---

## STEP 2 — Broadcast the swap

⚠️ **This sends mainnet ETH.** Triple-check:
- `--amount-in 0.005ETH` (NOT `0.5ETH`).
- `--recipient "$DANIEL_WALLET"` (the swap output goes back to you).
- `--network mainnet` (NOT `sepolia`).

```bash
# Set Daniel's primary wallet PK. Don't paste it into a script
# checked into the repo; gitleaks will flag and (rightly) fail.
export SBO3L_SIGNER_KEY=<paste-Daniel-primary-wallet-PK>

# Mainnet gate.
export SBO3L_ALLOW_MAINNET_TX=1

sbo3l uniswap swap \
  --network mainnet \
  --amount-in 0.005ETH \
  --token-out USDC \
  --recipient "$DANIEL_WALLET" \
  --rpc-url "$MAINNET_RPC_URL" \
  --broadcast
```

Output:

- `signed envelope: ...`
- `tx_hash: 0x<64-hex>`
- `explorer: https://etherscan.io/tx/0x...`
- `confirmed: block <N> gas_used=<G>`
- `swap status: USDC delivered to <recipient>`

Save the tx hash:

```bash
export SWAP_TX_HASH=0x...   # from output
```

---

## STEP 3 — Verify

### 3a — Etherscan

Open `https://etherscan.io/tx/$SWAP_TX_HASH` in a browser. Expected:

- Status: Success
- Method: `multicall` or `exactInputSingle` (depending on which
  router path the executor took)
- Token Transferred: USDC ≥ envelope's `amount_out_minimum`
- To: `0xdc7E…D231` (Daniel's wallet)

### 3b — Cast verify (no browser)

```bash
cast tx "$SWAP_TX_HASH" --rpc-url "$MAINNET_RPC_URL" | head -20
# expect: status: 1 (success), gasUsed reasonable

# Check Daniel's USDC balance went up
cast call 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48 \
  "balanceOf(address)(uint256)" "$DANIEL_WALLET" \
  --rpc-url "$MAINNET_RPC_URL"
# expect: increased by ≥ envelope's `amount_out_minimum` (USDC, 6 decimals)
```

---

## STEP 4 — After verify (Daniel-side done; my-side starts)

Paste the tx hash into our chat. I'll run the 5-min follow-up PR:

1. Add a row to `docs/proof/etherscan-link-pack.md` under
   "Mainnet" → "Live SBO3L mainnet activity": Uniswap swap tx
   hash + linked Etherscan URL.
2. Update `docs/submission/bounty-uniswap.md` (or equivalent) to
   flip "demo on Sepolia" → "live mainnet activity at <tx-hash>".
3. Update `docs/dev4/closeout-status.md` if it has a Uniswap row.
4. Save memory note `mainnet_uniswap_swap_<date>.md` with the tx
   hash + USDC amount + recipient + executor evidence digest, so
   future conversations can cite the live tx without re-fetching.

---

## STEP 5 — Rollback

There IS no rollback for a confirmed mainnet swap. ETH → USDC at
mainnet rates is final.

The closest mitigation is **don't broadcast if you're not sure**:
the dry-run path (`--dry-run`) costs nothing and produces the
same envelope minus the on-chain commit — use it to inspect
the quote (`expected_amount_out`, `amount_out_minimum`,
`quote_source`) before STEP 2.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Decision: deny` in dry-run | Policy denied the swap | Inspect `deny_code`; common cases: counterparty not in allowlist, slippage too tight, recipient blocklist |
| `eth_chainId mismatch` | RPC chain id doesn't match `--network mainnet` | Verify `MAINNET_RPC_URL` actually returns `1` (`cast chain-id`) |
| `insufficient funds for gas * price + value` | Wallet ETH < 0.005 + gas | Top up before retry; pre-flight 0.1 should have caught this |
| Tx pending > 60s | Gas price too low for current congestion | Wait or bump gas via Etherscan's "Speed Up" UI |
| Swap output < quote minimum | Slippage on confirm > slippage tolerance | Tx will revert atomically; no funds lost (only gas). Retry with higher `--slippage-bps` (default 50 = 0.5%; bump to 100 for 1%) |

---

## Why this matters (judge-grade impact)

- **Uniswap track**: from "we built a thing on Sepolia" → "live
  mainnet activity, tx hash <X>." 2nd → 1st probability +30% per
  the prompt's grade-impact mapping.
- **Cross-track**: the live mainnet swap evidence pairs with the
  upstream [Universal Router PR](https://github.com/Uniswap/universal-router/pull/477)
  — "we proposed the per-command pattern + we shipped a live
  swap through SBO3L's implementation."
- **Truthfulness**: every "production-shaped Uniswap integration"
  claim becomes resolvable from a public RPC + Etherscan link.
