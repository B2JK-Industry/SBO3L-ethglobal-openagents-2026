# `sbo3l uniswap swap` — Uniswap V3 swap envelope

> **Task D — Daniel-broadcast demo.** This CLI builds a signed-tx
> envelope for a Uniswap V3 `exactInputSingle` swap on either
> Sepolia or mainnet. Default `--dry-run` emits a JSON envelope you
> can inspect, hand off to a separate signer, or feed into `cast
> send`. Daniel runs the actual mainnet broadcast on his own primary
> key — this CLI prepares the bytes; signing-and-sending is a
> separate step.

## TL;DR — the demo invocation

```bash
sbo3l uniswap swap \
    --network mainnet \
    --amount-in 0.005ETH \
    --token-out USDC \
    --recipient 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231 \
    --rpc-url "$ALCHEMY_MAINNET_RPC_URL" \
    --dry-run \
    --out /tmp/sbo3l-swap-envelope.json
```

> `--dry-run` is the default; passing it explicitly is
> defense-in-depth for automation. Mutually exclusive with
> `--broadcast`.

The CLI prints a human-readable envelope summary AND writes the same
content as JSON to `--out`. The JSON shape is pinned by the
`sbo3l.uniswap_swap_envelope.v1` schema id (the first field in every
envelope).

## What the envelope contains

| Field | Meaning |
| --- | --- |
| `schema` | `sbo3l.uniswap_swap_envelope.v1` (stable) |
| `network`, `chain_id` | `mainnet` / `1` or `sepolia` / `11155111` |
| `router`, `quoter` | Uniswap V3 SwapRouter02 + QuoterV2 addresses for the network |
| `token_in`, `token_out` | Symbol + address + decimals |
| `recipient` | EIP-55 hex of who receives `tokenOut` |
| `fee_tier` | Pinned `3000` (0.3% pool — deepest WETH/USDC liquidity) |
| `amount_in_raw` | Exactly what the operator typed (`0.005ETH`) |
| `amount_in_wei` | The integer base-unit form that hits the wire (`5000000000000000`) |
| `expected_amount_out` | Live QuoterV2 result (`0` if no `--rpc-url` was supplied) |
| `amount_out_minimum` | `expected × (10000 − slippage_bps) / 10000` |
| `slippage_bps` | Default `50` (0.5%); accepts `1..=10000` |
| `deadline_unix` / `deadline_seconds` | Soft window (envelope-side; SwapRouter02's tuple has no on-chain deadline) |
| `to`, `data`, `value` | Drop-in for `eth_sendTransaction` |
| `quote_source` | `uniswap-v3-quoter-{network}-{quoter_addr}` or `no-quote (...)` |
| `computed_at` | RFC3339 timestamp |
| `broadcasted` | `false` for dry-run; `true` after `--broadcast` |
| `tx_hash`, `explorer_url` | Populated only after a successful `--broadcast` |

## Two-step flow — prepare locally, broadcast separately

Daniel's pattern for the mainnet demo:

### Step 1 — Prepare the envelope

```bash
# On Daniel's machine, with funded wallet selected via the
# Alchemy / PublicNode RPC URL.
SBO3L_ALLOW_MAINNET_TX=1 \
sbo3l uniswap swap \
    --network mainnet \
    --amount-in 0.005ETH \
    --token-out USDC \
    --recipient 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231 \
    --rpc-url "$ALCHEMY_MAINNET_RPC_URL" \
    --slippage-bps 50 \
    --out /tmp/sbo3l-swap-envelope.json
```

Inspect the printed envelope. Confirm:

- `expected_amount_out` looks reasonable (e.g. ~13.5 USDC for
  0.005 ETH at $2700/ETH).
- `amount_out_minimum` is exactly `expected × 0.995` (50 bps).
- `to` is `0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45` (mainnet
  SwapRouter02).
- `recipient` matches Daniel's wallet.

### Step 2a — Broadcast via this CLI (`--features eth_broadcast`)

```bash
# Build with the broadcast feature once.
cargo build -p sbo3l-cli --features eth_broadcast --release

# Set the gate + signer + RPC, then re-run with --broadcast (NOT --dry-run).
export SBO3L_ALLOW_MAINNET_TX=1
export SBO3L_SIGNER_KEY=0x<your-32-byte-hex-key>
export SBO3L_RPC_URL=$ALCHEMY_MAINNET_RPC_URL

./target/release/sbo3l uniswap swap \
    --network mainnet \
    --amount-in 0.005ETH \
    --token-out USDC \
    --recipient 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231 \
    --slippage-bps 50 \
    --broadcast
```

The CLI:

1. Refuses if `SBO3L_ALLOW_MAINNET_TX=1` is unset.
2. Refuses if the key env var is unset / not 32 bytes hex.
3. Refuses if the RPC URL's `eth_chainId` doesn't match the
   `--network` argument.
4. Refuses if no live quote was performed (would broadcast with
   `amount_out_minimum=0` — unsafe MEV exposure).
5. Calls QuoterV2, applies slippage, builds calldata, signs, sends,
   waits for 1 confirmation, prints the tx hash + Etherscan URL.

### Step 2b — Broadcast via `cast send` (alt path)

If you'd rather not recompile with `--features eth_broadcast`:

```bash
ENV=$(cat /tmp/sbo3l-swap-envelope.json)
TO=$(jq -r .to <<<"$ENV")
DATA=$(jq -r .data <<<"$ENV")

cast send \
    --rpc-url "$ALCHEMY_MAINNET_RPC_URL" \
    --private-key "$SBO3L_SIGNER_KEY" \
    "$TO" \
    "$DATA"
```

## Env-var matrix

| Variable | Required for | Notes |
| --- | --- | --- |
| `SBO3L_ALLOW_MAINNET_TX=1` | `--network mainnet` (always) | Same gate as `audit anchor` + `agent register`. Without it the CLI refuses to even build the envelope. |
| `SBO3L_SIGNER_KEY` | `--broadcast` only | Override with `--private-key-env-var <NAME>`. The CLI never accepts the key on a flag. |
| `SBO3L_RPC_URL` | `--broadcast` always; dry-run with quote | Override with `--rpc-url <url>`. http/https only. |
| `MAINNET_RPC_URL` + `SBO3L_ALLOW_MAINNET_LIVE_TEST=1` | The opt-in live mainnet quoter integration test | Cumulative gates so CI never accidentally hits mainnet. |

## Pinned addresses (for verification)

| Network | Role | Address |
| --- | --- | --- |
| mainnet | SwapRouter02 | `0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45` |
| mainnet | QuoterV2 | `0x61fFE014bA17989E743c5F6cB21bF9697530B21e` |
| mainnet | WETH9 | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` |
| mainnet | USDC | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` |
| sepolia | SwapRouter02 | `0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E` |
| sepolia | QuoterV2 | `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3` |
| sepolia | WETH9 | `0xfff9976782d46cc05630d1f6ebab18b2324d6b14` |
| sepolia | USDC (Circle testnet) | `0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238` |

Sources: `developers.uniswap.org/contracts/v3/reference/deployments`.

## Default recipient

`0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231` — Daniel's wallet
(provisioned 2026-05-01, funded with 0.1 SEP-ETH via Alchemy
faucet). Set as the demo's swap-output recipient so the on-chain
trail is recoverable from one address.

## Pre-broadcast checklist (for the live mainnet run)

1. Approve WETH spend by SwapRouter02 if input is WETH:
   ```bash
   cast send "$WETH" "approve(address,uint256)" \
       0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45 \
       0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff \
       --rpc-url "$RPC" --private-key "$KEY"
   ```
   (One-time; idempotent.) ETH-in flows go through a multicall
   wrapper outside this CLI's scope — for the demo, pre-wrap a
   small amount of ETH into WETH separately.
2. Verify chain ID with `cast chain-id --rpc-url "$RPC"` → expect `1`.
3. Run `--dry-run` first; eyeball the envelope.
4. Run `--broadcast`. Wait for confirmation. Save the tx hash.
5. Verify on Etherscan: input matches, output ≥ `amount_out_minimum`.

## What this CLI deliberately doesn't do

- **No private key on flags.** Only env vars. Never log the key
  itself; the broadcast prints `signer: 0x<address>` and a redacted
  RPC URL.
- **No automatic ERC-20 approve.** WETH-in needs a one-time approve;
  Daniel runs it once out-of-band. (Permit2 + Universal Router is a
  follow-up.)
- **No v4 hooks.** V3 single-pool exact-input only.
- **No on-chain deadline.** SwapRouter02's `exactInputSingle` tuple
  has no `deadline` field; the envelope's `deadline_unix` is for
  human-side "build, then broadcast within N minutes" workflow.
- **No MEV protection.** Submit through Flashbots Protect / MEV-Share
  if the size warrants it; this CLI only constructs the bytes.
