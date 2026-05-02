# `examples/uniswap-agent-ts`

Uniswap demo agent (TypeScript). Submits a Uniswap-shaped APRP to SBO3L for a policy decision; on `allow`, constructs SwapRouter02 `exactInputSingle` calldata for Sepolia and (in live mode) signs + broadcasts via the operator's RPC + key.

## 3-line setup (mock mode — no daemon, no key)

```bash
cd examples/uniswap-agent-ts && npm install
npm run smoke
```

Output (mock mode):

```
▶ smoke: building Sepolia WETH → USDC swap (mock mode)
  mode:            mock
  router:          0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E
  tx_hash:         0x...                        # deterministic pseudo-hash
  etherscan:       https://sepolia.etherscan.io/tx/0x...
  calldata length: 458 chars

✓ smoke ok
```

## Full agent flow (daemon required, live broadcast optional)

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
npm run agent
```

In **mock mode** (no eth env): submits APRP → SBO3L allow → builds calldata → returns deterministic pseudo-tx-hash. No on-chain action.

## Live mode (real Sepolia broadcast)

Requires:

- `SBO3L_LIVE_ETH=1`
- `SBO3L_ETH_RPC_URL=<Alchemy or other Sepolia endpoint>`
- `SBO3L_ETH_PRIVATE_KEY=0x<32-byte hex>`  (the swap signer; **client-side only — daemon never sees it**)
- `SBO3L_ETH_RECIPIENT=0x<EOA>`             (where `tokenOut` lands)

```bash
export SBO3L_LIVE_ETH=1
export SBO3L_ETH_RPC_URL="https://eth-sepolia.g.alchemy.com/v2/..."
export SBO3L_ETH_PRIVATE_KEY="0x..."
export SBO3L_ETH_RECIPIENT="0xdc7EFA..."   # the funded wallet
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
npm run agent
```

Output (live mode, after a successful Sepolia broadcast):

```
▶ submitting APRP for Uniswap swap (Sepolia WETH → USDC)
  decision:        allow
  audit_event_id:  evt-01HTAWX5K3R8YV9NQB7C6P2DGR

▶ swap (live mode):
  router:          0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E
  tx_hash:         0x<actual tx hash>
  etherscan:       https://sepolia.etherscan.io/tx/0x<actual tx hash>
  audit_event_id:  evt-01HTAWX5K3R8YV9NQB7C6P2DGR
```

> **First successful Sepolia broadcast (PASTE HERE):** `0x...` — to be filled in by Heidi after Daniel pushes the funded wallet's key into the env. The daemon's audit_event_id pairs cryptographically with the on-chain tx.

## Pre-swap requirement

Before the FIRST swap with a given (token_in, recipient) pair, the recipient EOA must approve SwapRouter02 to spend `tokenIn`:

```bash
# One-time, separate from this demo. Use cast / foundry:
cast send 0xfff9976782d46cc05630d1f6ebab18b2324d6b14 \
  "approve(address,uint256)" \
  0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E \
  $(cast --to-uint256 1ether) \
  --rpc-url $SBO3L_ETH_RPC_URL --private-key $SBO3L_ETH_PRIVATE_KEY
```

The demo doesn't auto-approve — it's a separate gas + UX cost.

## Files

- `src/agent.ts` — full APRP-submit + swap flow.
- `src/smoke.ts` — calldata + Etherscan URL only; no daemon needed.

## License

MIT
