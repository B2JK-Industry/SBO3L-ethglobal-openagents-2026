/**
 * ElizaOS-shape agent driver — exercises the @sbo3l/elizaos plugin
 * end-to-end. ElizaOS's runtime is heavyweight; this demo simulates a
 * minimal Eliza-shaped runtime that:
 *
 *   1. Calls `fetchUrl` (the agent's "data_fetch" Action equivalent)
 *   2. Builds a synthetic Eliza message containing the APRP
 *   3. Validates + dispatches the plugin's SBO3L_PAYMENT_REQUEST Action
 *
 * Real ElizaOS character configs reference the plugin via
 * `plugins: ["@sbo3l/elizaos"]` and let the framework pick when to fire
 * its Actions. This demo bypasses ElizaOS bootstrap (which is preview /
 * heavy) so the same plugin code path runs against a tiny harness.
 */

import { buildSbo3lPlugin, defaultClient, fetchUrl, KH_WORKFLOW_ID } from "./tools.js";

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-elizaos-1",
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
  console.log(`▶ KH workflow target = ${KH_WORKFLOW_ID}\n`);

  // Step 1: research the provider
  console.log("▶ Action equivalent: data_fetch (provider metadata)");
  const fetched = await fetchUrl(APRP.provider_url + "/.well-known/health");
  if (fetched.error !== undefined) {
    console.log(`  fetch warning: ${fetched.error} (continuing)`);
  } else {
    console.log(`  ✓ HTTP ${fetched.status}`);
  }

  // Step 2: dispatch SBO3L_PAYMENT_REQUEST via the plugin
  console.log("\n▶ Action: SBO3L_PAYMENT_REQUEST (real plugin path)");
  const client = defaultClient();
  const plugin = buildSbo3lPlugin(client);
  const action = plugin.actions[0]!;

  const message = { content: { aprp: APRP } };
  const isValid = await action.validate({}, message);
  if (!isValid) {
    console.error("error: action.validate returned false — bad APRP shape?");
    process.exit(1);
  }
  console.log("  ✓ validate passed");

  let envelope: string | undefined;
  await action.handler({}, message, undefined, undefined, ({ text }) => {
    envelope = text;
  });

  if (envelope === undefined) {
    console.error("error: action.handler did not invoke callback");
    process.exit(1);
  }

  const decision = JSON.parse(envelope) as Record<string, unknown>;
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
    console.log(`\n✗ ${decision["error"]}`);
    process.exit(2);
  }
  console.log(`\n✗ ${String(decision["decision"] ?? "?")} — deny_code ${String(decision["deny_code"] ?? "?")}`);
  process.exit(2);
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
