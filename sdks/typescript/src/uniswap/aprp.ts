/**
 * APRP-builder for Uniswap swaps. Constructs an APRP body whose
 * `intent` + `destination` tell SBO3L this is a Uniswap swap intent —
 * any operator policy that gates per-DEX or per-token can match on it
 * without parsing free-form fields.
 *
 * The SBO3L policy gate runs BEFORE any on-chain action — agent must
 * `await client.submit(aprp)` first; only on `decision === "allow"` does
 * `swap()` actually broadcast.
 */

import { SEPOLIA_USDC, SEPOLIA_WETH } from "./sepolia.js";

export interface SwapAprpParams {
  /** Stable agent slug (matches APRP `agent_id` regex). */
  agentId: string;
  /** Caller-chosen task identifier. */
  taskId: string;
  /**
   * Source token address — Sepolia WETH (default) or USDC. Pass any
   * EIP-55-cased ERC-20 address for arbitrary pairs.
   */
  tokenIn?: string;
  /** Destination token address (default: USDC if `tokenIn` is WETH, else WETH). */
  tokenOut?: string;
  /**
   * Amount to spend, as a decimal string in the **token's smallest unit's
   * fiat value**. The wire-format APRP carries USD value; the actual
   * on-chain `amountIn` (wei or micros) is derived by the caller from
   * a recent quote.
   */
  amountUsd: string;
  /** ULID nonce for replay protection. */
  nonce: string;
  /** RFC 3339 expiry. */
  expiry: string;
  /** Risk class for this swap. Default `medium` for trading. */
  riskClass?: "low" | "medium" | "high" | "critical";
}

/**
 * Build an APRP body for a Uniswap swap intent. Caller submits this
 * via `SBO3LClient.submit()` BEFORE invoking the actual swap.
 */
export function aprpForSwap(params: SwapAprpParams): Record<string, unknown> {
  const tokenIn = params.tokenIn ?? SEPOLIA_WETH;
  const tokenOut =
    params.tokenOut ??
    (tokenIn.toLowerCase() === SEPOLIA_WETH.toLowerCase() ? SEPOLIA_USDC : SEPOLIA_WETH);

  return {
    agent_id: params.agentId,
    task_id: params.taskId,
    intent: "purchase_api_call",
    amount: { value: params.amountUsd, currency: "USD" },
    token: "USDC",
    destination: {
      type: "erc20_transfer",
      token_address: tokenIn,
      recipient: tokenOut,
    },
    payment_protocol: "erc20_transfer",
    chain: "sepolia",
    provider_url: "https://sepolia.etherscan.io",
    expiry: params.expiry,
    nonce: params.nonce,
    risk_class: params.riskClass ?? "medium",
  };
}
