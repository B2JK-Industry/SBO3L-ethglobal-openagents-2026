"""Uniswap demo agent (Python). Mirror of examples/uniswap-agent-ts/src/agent.ts.

Two-step flow:

  1. Submit a Uniswap-shaped APRP via sbo3l_sdk.SBO3LClientSync.submit() so
     SBO3L's policy boundary decides whether the swap is permitted.
  2. On `decision == "allow"`, call sbo3l_sdk.uniswap.swap() which (in live
     mode) builds SwapRouter02 calldata, signs with SBO3L_ETH_PRIVATE_KEY,
     and broadcasts via SBO3L_ETH_RPC_URL. Mock mode returns a deterministic
     pseudo-tx-hash.

The no-key boundary stays intact: SBO3L's daemon never sees the private key.
Signing happens client-side here.
"""

from __future__ import annotations

import os
import sys

from sbo3l_sdk import SBO3LClientSync, bearer, uniswap


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    bearer_token = os.environ.get("SBO3L_BEARER_TOKEN")
    auth = bearer(bearer_token) if bearer_token else None

    # ── Step 1: build + submit Uniswap APRP for policy decision ──
    aprp = uniswap.aprp_for_swap(
        uniswap.SwapAprpParams(
            agent_id="research-agent-01",
            task_id="uniswap-py-demo-1",
            amount_usd="1.50",  # ~0.01 WETH at recent prices
            nonce="01HTAWX5K3R8YV9NQB7C6P2DGM",
            expiry="2026-05-01T10:31:00Z",
            risk_class="medium",
        )
    )

    print("▶ submitting APRP for Uniswap swap (Sepolia WETH → USDC)")
    with SBO3LClientSync(endpoint, auth=auth) as client:
        decision = client.submit(aprp)

    print(f"  decision:        {decision.decision}")
    print(f"  audit_event_id:  {decision.audit_event_id}")

    if decision.decision != "allow":
        print(f"  ✗ denied — {decision.deny_code or '(no code)'}")
        return 2

    # ── Step 2: build + (live mode) broadcast the swap ──
    recipient = os.environ.get(
        "SBO3L_ETH_RECIPIENT", "0x0000000000000000000000000000000000000000"
    )

    result = uniswap.swap(
        uniswap.SwapParams(
            token_in=uniswap.SEPOLIA_WETH,
            token_out=uniswap.SEPOLIA_USDC,
            fee=3_000,
            recipient=recipient,
            amount_in=10_000_000_000_000_000,  # 0.01 WETH (10^16 wei)
            amount_out_minimum=1_000_000,  # 1 USDC slippage floor
        )
    )

    print(f"\n▶ swap ({result.mode} mode):")
    print(f"  router:          {result.to}")
    print(f"  tx_hash:         {result.tx_hash}")
    print(f"  etherscan:       {result.etherscan_url}")
    print(f"  audit_event_id:  {decision.audit_event_id}")

    if result.mode == "mock":
        print(
            "\n  ℹ  mock mode — set SBO3L_LIVE_ETH=1 + SBO3L_ETH_RPC_URL "
            "+ SBO3L_ETH_PRIVATE_KEY for live broadcast."
        )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
