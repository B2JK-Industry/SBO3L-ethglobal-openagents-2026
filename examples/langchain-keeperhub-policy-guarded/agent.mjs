// Side-by-side TS demo of the SBO3L → KeeperHub policy gate.
//
// Submits two APRPs:
//   1. A small (within-budget) one — expects ALLOW + kh_execution_ref
//   2. A huge (over-budget) one     — expects DENY + no execution
//
// Run:
//   SBO3L_ALLOW_UNAUTHENTICATED=1 \
//   SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
//     cargo run --bin sbo3l-server &
//   node agent.mjs

import { randomUUID } from "node:crypto";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lKeeperHubTool } from "@sbo3l/langchain-keeperhub";

function aprp({ amountUsd, agentId = "research-agent-kh-01" }) {
  return {
    agent_id: agentId,
    task_id: `kh-demo-${randomUUID().slice(0, 8)}`,
    intent: "purchase_api_call",
    amount: { value: amountUsd, currency: "USD" },
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
    expiry: new Date(Date.now() + 5 * 60_000).toISOString(),
    nonce: randomUUID(),
    risk_class: "low",
  };
}

function printEnvelope(label, envelope) {
  console.log(`\n=== ${label} ===`);
  for (const [k, v] of Object.entries(envelope)) {
    console.log(`  ${k}: ${JSON.stringify(v)}`);
  }
}

async function main() {
  const endpoint = process.env.SBO3L_ENDPOINT ?? "http://localhost:8730";
  console.log(`▶ daemon endpoint: ${endpoint}`);

  const client = new SBO3LClient({ endpoint });
  const tool = sbo3lKeeperHubTool({ client });

  // Path 1 — within-budget.
  const small = aprp({ amountUsd: "0.05" });
  const env1 = JSON.parse(await tool.func(JSON.stringify(small)));
  printEnvelope("ALLOW path (amount=0.05)", env1);

  // Path 2 — over-budget.
  const huge = aprp({ amountUsd: "10000.00" });
  const env2 = JSON.parse(await tool.func(JSON.stringify(huge)));
  printEnvelope("DENY path (amount=10000.00)", env2);

  console.log("\n--- summary ---");
  console.log(`  small.kh_execution_ref: ${env1.kh_execution_ref}`);
  console.log(`  huge.kh_execution_ref:  ${env2.kh_execution_ref}`);
  console.log(`  small.audit_event_id:   ${env1.audit_event_id}`);
  console.log(`  huge.audit_event_id:    ${env2.audit_event_id}`);
  console.log(`  huge.deny_code:         ${env2.deny_code}`);

  if (env1.decision === "allow" && env1.kh_execution_ref) {
    if (env2.decision === "deny" && env2.kh_execution_ref === null) {
      console.log("\n✓ gate-then-execute proven: small executed, huge blocked.");
      return 0;
    }
  }

  console.log("\n✗ unexpected: see envelopes above.");
  return 1;
}

try {
  process.exit(await main());
} catch (e) {
  console.error("error:", e);
  process.exit(2);
}
