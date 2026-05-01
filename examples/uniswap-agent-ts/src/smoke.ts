/**
 * Smoke runner — exercises the swap calldata + Etherscan URL path with
 * NO daemon required. Verifies the SDK helpers wire end-to-end and
 * produce the right shape for live broadcast.
 *
 * For the full agent-loop with daemon + (optionally) live broadcast,
 * use `npm run agent`.
 */

import { uniswap } from "@sbo3l/sdk";

async function main(): Promise<void> {
  const recipient = "0x0000000000000000000000000000000000000000";

  console.log("▶ smoke: building Sepolia WETH → USDC swap (mock mode)");
  const result = await uniswap.swap({
    tokenIn: uniswap.SEPOLIA_WETH,
    tokenOut: uniswap.SEPOLIA_USDC,
    fee: 3_000,
    recipient,
    amountIn: 10_000_000_000_000_000n,  // 0.01 WETH
    amountOutMinimum: 1_000_000n,        // 1 USDC floor
  });

  console.log(`  mode:            ${result.mode}`);
  console.log(`  router:          ${result.to}`);
  console.log(`  tx_hash:         ${result.txHash}`);
  console.log(`  etherscan:       ${result.etherscanUrl}`);
  console.log(`  calldata length: ${result.calldata.length} chars`);

  if (result.mode !== "mock") {
    console.error("expected mock mode — unset SBO3L_LIVE_ETH for the smoke");
    process.exit(1);
  }
  console.log("\n✓ smoke ok");
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
