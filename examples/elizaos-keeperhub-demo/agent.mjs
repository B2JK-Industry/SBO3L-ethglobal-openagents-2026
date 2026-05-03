// Minimal ElizaOS-shape demo for `@sbo3l/elizaos-keeperhub`.
//
// Real ElizaOS character configs reference plugins via
// `plugins: ["@sbo3l/elizaos-keeperhub"]` and let the framework pick when to
// fire the registered Action. This demo bypasses ElizaOS bootstrap (which is
// preview / heavy) so the same Action code path runs against a tiny harness:
//
//   1. Hardcode a chat turn carrying an APRP (no LLM in the loop)
//   2. Build a synthetic Eliza-shaped message
//   3. validate() + handler() the SBO3L_KEEPERHUB_PAYMENT_REQUEST Action
//   4. Print the envelope + exit non-zero on deny / error
//
// Use `agent_id="research-agent-01"` — the only id registered in the bundled
// reference policy. To swap, point the daemon at a custom policy via
// SBO3L_POLICY before running.

import { SBO3LClient } from "@sbo3l/sdk";
import {
  DEFAULT_KH_WORKFLOW_ID,
  sbo3lElizaKeeperHubAction,
} from "@sbo3l/elizaos-keeperhub";

const APRP = {
  agent_id: "research-agent-01",
  task_id: `kh-elizaos-demo-${Date.now()}`,
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
  // Future timestamp guarantees the daemon won't reject as expired.
  expiry: new Date(Date.now() + 5 * 60_000).toISOString(),
  nonce: `nonce-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
  risk_class: "low",
};

// Hardcoded "chat turn" — the agent says this; in a real ElizaOS character
// the framework would parse the LLM's reply and assemble the message.
const CHAT_TURN = {
  user: "research-agent-01",
  content: {
    text:
      "I need to pay 0.05 USDC for an inference call routed via " +
      "KeeperHub workflow.",
    aprp: APRP,
    action: "SBO3L_KEEPERHUB_PAYMENT_REQUEST",
  },
};

async function main() {
  const endpoint = process.env.SBO3L_ENDPOINT ?? "http://localhost:8730";
  console.log(`▶ daemon: ${endpoint}`);
  console.log(`▶ KH workflow target = ${DEFAULT_KH_WORKFLOW_ID}\n`);

  const client = new SBO3LClient({ endpoint });
  const action = sbo3lElizaKeeperHubAction({ client });

  console.log(`▶ Action: ${action.name}`);
  console.log(`  similes: ${action.similes.join(", ")}\n`);

  const isValid = await action.validate({}, CHAT_TURN);
  if (!isValid) {
    console.error("error: action.validate returned false (no APRP extracted)");
    process.exit(1);
  }
  console.log("  ✓ validate passed");

  let envelopeText;
  await action.handler({}, CHAT_TURN, undefined, undefined, ({ text }) => {
    envelopeText = text;
  });

  if (envelopeText === undefined) {
    console.error("error: action.handler did not invoke callback");
    process.exit(1);
  }

  const envelope = JSON.parse(envelopeText);
  console.log("\n=== envelope ===");
  for (const [k, v] of Object.entries(envelope)) {
    console.log(`  ${k}: ${JSON.stringify(v)}`);
  }

  if (envelope.decision === "allow" && envelope.kh_execution_ref) {
    console.log(
      `\n✓ allow + KH executed → kh_execution_ref=${envelope.kh_execution_ref}`,
    );
    console.log(`  audit_event_id: ${envelope.audit_event_id}`);
    process.exit(0);
  }
  if (typeof envelope.error === "string") {
    console.log(`\n✗ transport error: ${envelope.error}`);
    process.exit(2);
  }
  console.log(`\n✗ unexpected decision: ${String(envelope.decision)}`);
  process.exit(1);
}

main().catch((err) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
