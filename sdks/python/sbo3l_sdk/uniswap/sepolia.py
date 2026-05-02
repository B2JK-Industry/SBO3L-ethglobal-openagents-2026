"""Sepolia constants — kept in sync with Rust's `sbo3l_execution::uniswap_trading`
and TS's `@sbo3l/sdk/uniswap/sepolia`."""

from __future__ import annotations

#: EIP-155 Sepolia chain id.
SEPOLIA_CHAIN_ID: int = 11_155_111

#: Sepolia SwapRouter02 deployment.
SEPOLIA_SWAP_ROUTER_02: str = "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E"

#: Sepolia USDC (Circle's official testnet USDC). 6 decimals.
SEPOLIA_USDC: str = "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238"

#: Sepolia WETH9. 18 decimals.
SEPOLIA_WETH: str = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14"

#: Selector for `exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))`.
#: Pinned in tests against `keccak256(canonical_type_string)[:4]`.
EXACT_INPUT_SINGLE_SELECTOR: bytes = bytes.fromhex("04e45aaf")


def sepolia_etherscan_tx_url(tx_hash: str) -> str:
    """Build the canonical Sepolia Etherscan URL for a transaction hash."""

    h = tx_hash[2:] if tx_hash.startswith(("0x", "0X")) else tx_hash
    return f"https://sepolia.etherscan.io/tx/0x{h}"
