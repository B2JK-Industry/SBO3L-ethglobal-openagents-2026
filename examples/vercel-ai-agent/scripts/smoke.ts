/**
 * Smoke test — exercises the @sbo3l/vercel-ai tool end-to-end against a
 * running daemon WITHOUT requiring an OpenAI key. Calls the tool's
 * `execute()` directly with a hard-coded APRP. Demonstrates the "boots and
 * signs a payment in dev mode" acceptance criterion from T-1-7.
 *
 * Usage:
 *   SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
 *   npx tsx scripts/smoke.ts
 */

import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool, PolicyDenyError } from "@sbo3l/vercel-ai";

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-vercel-1",
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
  expiry: "2026-05-01T10:31:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  risk_class: "low" as const,
};

async function main(): Promise<void> {
  const client = new SBO3LClient({
    endpoint: process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730",
  });

  const t = sbo3lTool({ client });

  try {
    const receipt = await t.execute!(APRP, {});
    console.log(`✓ allow — execution_ref: ${receipt.execution_ref ?? "(none)"}`);
    console.log(`  audit_event_id: ${receipt.audit_event_id}`);
    console.log(`  signature.algorithm: ${receipt.signature.algorithm}`);
    process.exit(0);
  } catch (err: unknown) {
    if (err instanceof PolicyDenyError) {
      console.log(`✗ deny — ${err.denyCode} (decision=${err.decision})`);
      console.log(`  audit_event_id: ${err.auditEventId}`);
      process.exit(2);
    }
    throw err;
  }
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
