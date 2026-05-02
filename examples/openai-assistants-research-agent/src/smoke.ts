/**
 * Smoke runner — proves the SBO3L tool path end-to-end against a running
 * SBO3L daemon WITHOUT an OpenAI key. Hand-builds the kind of `tool_call`
 * the OpenAI Assistants run-poller would produce, then dispatches it
 * through `runSbo3lToolCall`.
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
import {
  runSbo3lToolCall,
  sbo3lAssistantTool,
  type AssistantToolCall,
  type PaymentRequest,
} from "@sbo3l/openai-assistants";

const ENDPOINT = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const aprp: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-openai-assistants-smoke-1",
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
  const tool = sbo3lAssistantTool({ client });

  console.log(`▶ tool definition emitted to assistants.create({ tools: [...] }):`);
  console.log(`  name:       ${tool.definition.function.name}`);
  console.log(`  type:       ${tool.definition.type}`);
  console.log(`  parameters: <APRP v1 JSON schema, ${tool.definition.function.parameters.required.length} required fields>\n`);

  // Hand-build the tool_call that openai.beta.threads.runs would produce
  // when status === "requires_action". This lets us exercise the full
  // dispatch path WITHOUT an OpenAI key.
  const toolCall: AssistantToolCall = {
    id: "call-smoke-001",
    type: "function",
    function: { name: tool.name, arguments: JSON.stringify(aprp) },
  };

  console.log(`▶ dispatching synthetic tool_call ${toolCall.id} → SBO3L → KH adapter`);
  const out = await runSbo3lToolCall(tool, toolCall);
  const parsed = JSON.parse(out.output) as Record<string, unknown>;

  console.log(`\n▶ output (would be passed to submitToolOutputs):`);
  for (const [k, v] of Object.entries(parsed)) {
    console.log(`  ${k}: ${JSON.stringify(v)}`);
  }

  if (typeof parsed["execution_ref"] === "string" || parsed["decision"] === "allow") {
    console.log(`\n✓ allow — execution_ref ${parsed["execution_ref"] ?? "(none)"}`);
    console.log(`  audit_event_id: ${parsed["audit_event_id"] ?? "(unknown)"}`);
    return;
  }
  if (typeof parsed["error"] === "string") {
    console.log(`\n✗ ${parsed["error"]} — ${JSON.stringify(parsed)}`);
    process.exitCode = 2;
    return;
  }
  console.log(`\n? unexpected output shape — ${JSON.stringify(parsed)}`);
  process.exitCode = 2;
}

main().catch((err: unknown) => {
  console.error(`smoke failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(2);
});
