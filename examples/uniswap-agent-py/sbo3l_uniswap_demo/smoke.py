"""Smoke runner — exercises the swap calldata + Etherscan URL path with NO
daemon required. Verifies the SDK helpers wire end-to-end and produce the
right shape for live broadcast.

For the full agent-loop with daemon + (optionally) live broadcast, use:
    python -m sbo3l_uniswap_demo.agent
"""

from __future__ import annotations

import sys

from sbo3l_sdk import uniswap


def main() -> int:
    recipient = "0x0000000000000000000000000000000000000000"

    print("▶ smoke: building Sepolia WETH → USDC swap (mock mode)")
    result = uniswap.swap(
        uniswap.SwapParams(
            token_in=uniswap.SEPOLIA_WETH,
            token_out=uniswap.SEPOLIA_USDC,
            fee=3_000,
            recipient=recipient,
            amount_in=10_000_000_000_000_000,  # 0.01 WETH
            amount_out_minimum=1_000_000,  # 1 USDC floor
        )
    )

    print(f"  mode:            {result.mode}")
    print(f"  router:          {result.to}")
    print(f"  tx_hash:         {result.tx_hash}")
    print(f"  etherscan:       {result.etherscan_url}")
    print(f"  calldata length: {len(result.calldata)} chars")

    if result.mode != "mock":
        print("expected mock mode — unset SBO3L_LIVE_ETH for the smoke", file=sys.stderr)
        return 1
    print("\n✓ smoke ok")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
