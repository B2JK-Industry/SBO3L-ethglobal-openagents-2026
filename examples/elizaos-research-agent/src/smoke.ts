/**
 * Smoke runner — same path as agent.ts but with no provider-fetch round-trip
 * (uses a known-good public URL). Proves the plugin's Action triggers, the
 * APRP routes through SBO3L, and the daemon's KH adapter is hit.
 */

import { buildSbo3lPlugin, defaultClient, fetchUrl, KH_WORKFLOW_ID } from "./tools.js";

// Fresh nonce + expiry per run — daemon's protocol.nonce_replay rejects
// duplicate (nonce, agent_id) tuples.
function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-elizaos-smoke-1",
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
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: freshNonce(),
  risk_class: "low",
};

async function main(): Promise<void> {
  console.log(`▶ smoke: KH workflow target = ${KH_WORKFLOW_ID}\n`);

  console.log("▶ data_fetch (GitHub status — public, low-noise)");
  const fetched = await fetchUrl("https://www.githubstatus.com/api/v2/status.json");
  if (fetched.error !== undefined) {
    console.log(`  fetch warning: ${fetched.error} (continuing)`);
  } else {
    console.log(`  ✓ HTTP ${fetched.status}`);
  }

  console.log("\n▶ Action: SBO3L_PAYMENT_REQUEST (plugin path)");
  const plugin = buildSbo3lPlugin(defaultClient());
  const action = plugin.actions[0]!;
  const message = { content: { aprp: APRP } };

  let envelope: string | undefined;
  await action.handler({}, message, undefined, undefined, ({ text }) => {
    envelope = text;
  });
  const decision = JSON.parse(envelope!) as Record<string, unknown>;

  for (const [k, v] of Object.entries(decision)) {
    console.log(`  ${k}: ${JSON.stringify(v)}`);
  }
  if (decision["decision"] === "allow") {
    console.log(`\n✓ allow — execution_ref ${decision["execution_ref"] ?? "(none)"}`);
    process.exit(0);
  }
  if (typeof decision["error"] === "string") {
    console.log(`\n✗ ${decision["error"]}`);
    process.exit(2);
  }
  console.log(`\n✗ ${String(decision["decision"] ?? "?")}`);
  process.exit(2);
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
