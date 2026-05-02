/**
 * Uniswap demo agent (TS). Two-step flow:
 *
 *   1. Submit a Uniswap-shaped APRP via @sbo3l/sdk's SBO3LClient.submit()
 *      so SBO3L's policy boundary decides whether the swap is permitted.
 *   2. On `decision === "allow"`, call uniswap.swap() which (in live mode)
 *      builds SwapRouter02 calldata, signs with SBO3L_ETH_PRIVATE_KEY, and
 *      broadcasts via SBO3L_ETH_RPC_URL. Mock mode returns a deterministic
 *      pseudo-tx-hash so the demo path runs without secrets.
 *
 * The no-key boundary stays intact: SBO3L's daemon never sees the
 * private key. Signing happens client-side here.
 */

import { SBO3LClient, uniswap } from "@sbo3l/sdk";

async function main(): Promise<void> {
  const endpoint = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
  const bearer = process.env["SBO3L_BEARER_TOKEN"];
  const client = new SBO3LClient({
    endpoint,
    ...(bearer !== undefined ? { auth: { kind: "bearer" as const, token: bearer } } : {}),
  });

  // ── Step 1: build + submit Uniswap APRP for policy decision ──
  const aprp = uniswap.aprpForSwap({
    agentId: "research-agent-01",
    taskId: "uniswap-ts-demo-1",
    amountUsd: "1.50",       // ~0.01 WETH at recent prices
    nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
    expiry: "2026-05-01T10:31:00Z",
    riskClass: "medium",
  });

  console.log(`▶ submitting APRP for Uniswap swap (Sepolia WETH → USDC)`);
  const decision = await client.submit(aprp as Parameters<typeof client.submit>[0]);
  console.log(`  decision:        ${decision.decision}`);
  console.log(`  audit_event_id:  ${decision.audit_event_id}`);

  if (decision.decision !== "allow") {
    console.log(`  ✗ denied — ${decision.deny_code ?? "(no code)"}`);
    process.exit(2);
  }

  // ── Step 2: build + (live mode) broadcast the swap ──
  const recipient =
    process.env["SBO3L_ETH_RECIPIENT"] ?? "0x0000000000000000000000000000000000000000";

  const result = await uniswap.swap({
    tokenIn: uniswap.SEPOLIA_WETH,
    tokenOut: uniswap.SEPOLIA_USDC,
    fee: 3_000,
    recipient,
    amountIn: 10_000_000_000_000_000n,  // 0.01 WETH (10^16 wei)
    amountOutMinimum: 1_000_000n,        // 1 USDC slippage floor
  });

  console.log(`\n▶ swap (${result.mode} mode):`);
  console.log(`  router:          ${result.to}`);
  console.log(`  tx_hash:         ${result.txHash}`);
  console.log(`  etherscan:       ${result.etherscanUrl}`);
  console.log(`  audit_event_id:  ${decision.audit_event_id}`);

  if (result.mode === "mock") {
    console.log(
      "\n  ℹ  mock mode — set SBO3L_LIVE_ETH=1 + SBO3L_ETH_RPC_URL + SBO3L_ETH_PRIVATE_KEY for live broadcast.",
    );
  }
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
