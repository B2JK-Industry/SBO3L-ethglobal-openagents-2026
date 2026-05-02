/**
 * Smoke runner — proves the agent's tool path end-to-end against a running
 * SBO3L daemon WITHOUT an OpenAI key. Hardcodes the tool calls the LLM
 * would make so CI / Heidi can verify the integration without burning LLM
 * tokens.
 *
 * Usage:
 *   SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
 *   npm install
 *   npm run smoke
 *
 * AC verification:
 *   - install verified (`npm install` resolves)
 *   - runs end-to-end in <30s (no LLM round-trips)
 *   - signed PolicyReceipt visible in console
 *   - audit_event_id appended (printed)
 */

import { dataFetchTool, buildSbo3lPayTool, defaultClient, KH_WORKFLOW_ID } from "./tools.js";

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-langchain-smoke-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: "2026-05-01T10:31:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  risk_class: "low",
};

async function main(): Promise<void> {
  console.log(`▶ smoke: KH workflow target = ${KH_WORKFLOW_ID}\n`);

  // Step 1: simulate the agent's `data_fetch` tool call.
  console.log("▶ tool: data_fetch (GitHub status — public, low-noise)");
  const fetchOut = await dataFetchTool.func(JSON.stringify({ url: "https://www.githubstatus.com/api/v2/status.json" }));
  const parsed = JSON.parse(fetchOut) as { status?: number; error?: string };
  if (parsed.error !== undefined) {
    console.log(`  fetch warning: ${parsed.error} (continuing — SBO3L gate is the test)`);
  } else {
    console.log(`  ✓ HTTP ${parsed.status}`);
  }

  // Step 2: simulate the agent's `sbo3l_payment_request` tool call.
  console.log("\n▶ tool: sbo3l_payment_request (APRP → SBO3L → KH adapter)");
  const client = defaultClient();
  const payTool = buildSbo3lPayTool(client);
  const decisionRaw = await payTool.func(JSON.stringify(APRP));
  const decision = JSON.parse(decisionRaw) as Record<string, unknown>;

  console.log("  envelope:");
  for (const [k, v] of Object.entries(decision)) {
    console.log(`    ${k}: ${JSON.stringify(v)}`);
  }

  if (decision["decision"] === "allow") {
    console.log(`\n✓ allow — execution_ref ${decision["execution_ref"] ?? "(none)"}`);
    console.log(`  audit_event_id: ${decision["audit_event_id"] ?? "(unknown)"}`);
    process.exit(0);
  }

  if (typeof decision["error"] === "string") {
    console.log(`\n✗ transport error — ${decision["error"]}`);
    process.exit(2);
  }

  console.log(`\n✗ ${String(decision["decision"] ?? "?")} — deny_code ${String(decision["deny_code"] ?? "?")}`);
  process.exit(2);
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
