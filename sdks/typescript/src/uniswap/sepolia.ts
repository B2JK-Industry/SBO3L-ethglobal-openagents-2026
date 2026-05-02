/**
 * Sepolia constants — kept in sync with Rust's `sbo3l_execution::uniswap_trading`.
 * If anything here drifts from the Rust constants, the cross-language
 * demo (multi-framework + Rust executor) breaks loudly at the next test run.
 */

/** EIP-155 Sepolia chain id. */
export const SEPOLIA_CHAIN_ID = 11_155_111 as const;

/** Sepolia SwapRouter02 deployment. */
export const SEPOLIA_SWAP_ROUTER_02 =
  "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E" as const;

/** Sepolia USDC (Circle's official testnet USDC). 6 decimals. */
export const SEPOLIA_USDC = "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238" as const;

/** Sepolia WETH9. 18 decimals. */
export const SEPOLIA_WETH = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14" as const;

/**
 * Selector for `exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))`.
 * Pinned in tests against `keccak256(canonical_type_string).slice(0, 4)`.
 */
export const EXACT_INPUT_SINGLE_SELECTOR = "0x04e45aaf" as const;

/**
 * Build the canonical Sepolia Etherscan URL for a transaction hash.
 * Mirrors Rust's `sepolia_etherscan_tx_url`.
 */
export function sepoliaEtherscanTxUrl(txHash: string): string {
  const h = txHash.startsWith("0x") ? txHash.slice(2) : txHash;
  return `https://sepolia.etherscan.io/tx/0x${h}`;
}
