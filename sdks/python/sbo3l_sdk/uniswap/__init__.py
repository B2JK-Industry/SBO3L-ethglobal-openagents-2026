"""sbo3l_sdk.uniswap — agent-side Uniswap swap helper.

Mirror of `@sbo3l/sdk/uniswap` (TypeScript). Same `swap()` interface,
same SwapRouter02 calldata layout, same Sepolia constants.

Flow:

    1. Caller builds an APRP describing the intended swap (`aprp_for_swap()`).
    2. Caller submits via `SBO3LClientSync.submit()` for the policy decision.
    3. On `decision == "allow"`, caller invokes `swap()` from this module.
       In live mode (`SBO3L_LIVE_ETH=1` env + RPC URL + private key), it
       constructs SwapRouter02 calldata, signs the tx via the caller's
       provided web3 client, broadcasts. Returns tx hash + Etherscan URL.
       In mock mode (default), returns a deterministic pseudo-tx-hash.

The no-key boundary stays intact: signing happens client-side. SBO3L's
daemon never sees the private key.
"""

from __future__ import annotations

from .aprp import SwapAprpParams, aprp_for_swap
from .sepolia import (
    EXACT_INPUT_SINGLE_SELECTOR,
    SEPOLIA_CHAIN_ID,
    SEPOLIA_SWAP_ROUTER_02,
    SEPOLIA_USDC,
    SEPOLIA_WETH,
    sepolia_etherscan_tx_url,
)
from .swap import SwapEnv, SwapParams, SwapResult, encode_exact_input_single, swap

__all__ = [
    "EXACT_INPUT_SINGLE_SELECTOR",
    "SEPOLIA_CHAIN_ID",
    "SEPOLIA_SWAP_ROUTER_02",
    "SEPOLIA_USDC",
    "SEPOLIA_WETH",
    "SwapAprpParams",
    "SwapEnv",
    "SwapParams",
    "SwapResult",
    "aprp_for_swap",
    "encode_exact_input_single",
    "sepolia_etherscan_tx_url",
    "swap",
]
