/**
 * `@sbo3l/sdk/uniswap` — agent-side Uniswap swap helper.
 *
 * Flow:
 *
 *   1. Caller builds an APRP describing the intended swap (this module's
 *      `aprpForSwap()` does it for the common Sepolia WETH/USDC pair).
 *   2. Caller submits the APRP via `SBO3LClient.submit()` for the
 *      cryptographic policy decision (handled outside this module).
 *   3. On `decision === "allow"`, caller invokes `swap()` from this
 *      module which, in **live mode** (`SBO3L_LIVE_ETH=1` + RPC URL +
 *      private key in env), constructs the SwapRouter02
 *      `exactInputSingle` calldata, signs the transaction, broadcasts
 *      via `eth_sendRawTransaction`, and returns the tx hash + Etherscan
 *      URL. In **mock mode** (default), returns a deterministic
 *      pseudo-tx-hash so the demo path runs in CI without secrets.
 *
 * The no-key boundary is preserved: signing happens **client-side** in
 * the agent process. SBO3L's daemon never sees the private key.
 *
 * The Sepolia constants and ABI selector mirror Rust's `sbo3l_execution::
 * uniswap_trading` module — single source of truth for the wire shape.
 */

export { aprpForSwap } from "./aprp.js";
export type { SwapAprpParams } from "./aprp.js";
export { swap, encodeExactInputSingle } from "./swap.js";
export type { SwapParams, SwapResult, SwapEnv } from "./swap.js";
export {
  SEPOLIA_CHAIN_ID,
  SEPOLIA_SWAP_ROUTER_02,
  SEPOLIA_USDC,
  SEPOLIA_WETH,
  EXACT_INPUT_SINGLE_SELECTOR,
  sepoliaEtherscanTxUrl,
} from "./sepolia.js";
