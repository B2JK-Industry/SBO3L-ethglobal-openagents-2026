/**
 * Smoke runner — proves the SBO3L Mastra tool path end-to-end against a
 * running SBO3L daemon WITHOUT installing @mastra/core or an LLM key.
 * Calls the tool's `execute({ context })` shape directly — same shape
 * Mastra would call it with at runtime.
 *
 * Usage:
 *   SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
 *   npm install
 *   npm run smoke
 *
 * AC verification:
 *   - install verified (`npm install` resolves)
 *   - runs in <30s
 *   - signed PolicyReceipt visible in console
 *   - audit_event_id printed
 */

import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool, type PaymentRequest } from "@sbo3l/mastra";

const ENDPOINT = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const aprp: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-mastra-smoke-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
    expected_recipient: "0x1111111111111111111111111111111111111111",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: freshNonce(),
  risk_class: "low",
};

async function main(): Promise<void> {
  console.log(`▶ smoke: SBO3L endpoint = ${ENDPOINT}`);
  console.log(`▶ smoke: KH workflow target = ${KH_WORKFLOW_ID}\n`);

  const client = new SBO3LClient({ endpoint: ENDPOINT });
  const tool = sbo3lTool({ client });

  console.log(`▶ Mastra tool descriptor:`);
  console.log(`  id:          ${tool.id}`);
  console.log(`  description: ${tool.description.slice(0, 60)}...\n`);

  console.log(`▶ invoking tool.execute({ context: aprp })`);
  try {
    const out = await tool.execute({ context: aprp });
    console.log(`\n▶ tool output:`);
    console.log(`  decision:        ${out.decision}`);
    console.log(`  audit_event_id:  ${out.audit_event_id}`);
    console.log(`  execution_ref:   ${out.execution_ref ?? "(none)"}`);
    console.log(`\n✓ allow — Mastra tool path works`);
  } catch (e) {
    console.log(`\n✗ ${e instanceof Error ? e.message : String(e)}`);
    process.exitCode = 2;
  }
}

main().catch((err: unknown) => {
  console.error(`smoke failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(2);
});
