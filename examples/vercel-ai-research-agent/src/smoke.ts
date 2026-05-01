/**
 * Smoke runner — proves the demo's tool path end-to-end against a running
 * SBO3L daemon WITHOUT an OpenAI key. Hardcodes the tool execute() calls
 * a reasonable agent would make so CI / Heidi can verify the integration
 * without burning LLM tokens.
 *
 * Usage:
 *   SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
 *   npm install
 *   npm run smoke
 *
 * AC verification:
 *   - install verified (`npm install` resolves)
 *   - runs end-to-end in <30s
 *   - signed PolicyReceipt visible in console
 *   - audit_event_id printed
 */

import { dataFetchTool, buildSbo3lTool, defaultClient, KH_WORKFLOW_ID } from "./tools.js";
import { PolicyDenyError, SBO3LError } from "@sbo3l/vercel-ai";

// Fresh nonce + expiry per run. The daemon's protocol.nonce_replay guard
// rejects exact-duplicate (nonce, agent_id) tuples, so a static nonce
// only succeeds the first time the smoke runs against a given daemon.
// randomUUID is available on globalThis since Node 19; the fallback covers
// older Node 18 builds and exotic runtimes.
function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-vercel-ai-smoke-1",
  intent: "purchase_api_call" as const,
  amount: { value: "0.05", currency: "USD" as const },
  token: "USDC",
  destination: {
    type: "x402_endpoint" as const,
    url: "https://api.example.com/v1/inference",
    method: "POST" as const,
  },
  payment_protocol: "x402" as const,
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: freshNonce(),
  risk_class: "low" as const,
};

async function main(): Promise<void> {
  console.log(`▶ smoke: KH workflow target = ${KH_WORKFLOW_ID}\n`);

  // Step 1: data_fetch
  console.log("▶ tool: data_fetch (GitHub status — public, low-noise)");
  const fetchOut = await dataFetchTool.execute!(
    { url: "https://www.githubstatus.com/api/v2/status.json" },
    {},
  );
  if ("error" in fetchOut) {
    console.log(`  fetch warning: ${fetchOut.error} (continuing — SBO3L gate is the test)`);
  } else {
    console.log(`  ✓ HTTP ${fetchOut.status}`);
  }

  // Step 2: sbo3l_pay
  console.log("\n▶ tool: sbo3l_pay (APRP → SBO3L → KH adapter)");
  const client = defaultClient();
  const payTool = buildSbo3lTool(client);

  try {
    const receipt = await payTool.execute!(APRP, {});
    console.log(`✓ allow — execution_ref ${receipt.execution_ref ?? "(none)"}`);
    console.log(`  audit_event_id: ${receipt.audit_event_id}`);
    console.log(`  signature.algorithm: ${receipt.signature.algorithm}`);
    console.log(`  signature.key_id: ${receipt.signature.key_id}`);
    process.exit(0);
  } catch (err: unknown) {
    if (err instanceof PolicyDenyError) {
      console.log(`✗ ${err.decision} — ${err.denyCode}`);
      console.log(`  audit_event_id: ${err.auditEventId}`);
      process.exit(2);
    }
    if (err instanceof SBO3LError) {
      console.log(`✗ daemon error — ${err.code} (status ${err.status})`);
      process.exit(2);
    }
    throw err;
  }
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
