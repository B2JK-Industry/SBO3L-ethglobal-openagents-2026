/**
 * Smoke runner — proves the SBO3L tool path end-to-end against a running
 * SBO3L daemon WITHOUT an Anthropic key. Hand-builds the kind of
 * `tool_use` content block Claude would produce and dispatches it
 * through `runSbo3lToolUse`.
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
  runSbo3lToolUse,
  sbo3lTool,
  type AnthropicToolUseBlock,
  type PaymentRequest,
} from "@sbo3l/anthropic";

const ENDPOINT = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const aprp: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-anthropic-smoke-1",
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

  console.log(`▶ tool definition emitted to messages.create({ tools: [...] }):`);
  console.log(`  name:         ${tool.definition.name}`);
  console.log(`  input_schema: <APRP v1, ${tool.definition.input_schema.required.length} required fields>\n`);

  const toolUse: AnthropicToolUseBlock = {
    type: "tool_use",
    id: "toolu_smoke_001",
    name: tool.name,
    input: aprp,
  };

  console.log(`▶ dispatching synthetic tool_use ${toolUse.id} → SBO3L → KH adapter`);
  const out = await runSbo3lToolUse(tool, toolUse);
  const parsed = JSON.parse(out.content) as Record<string, unknown>;

  console.log(`\n▶ tool_result block (would be pushed into next messages.create):`);
  console.log(`  is_error: ${out.is_error ?? false}`);
  for (const [k, v] of Object.entries(parsed)) {
    console.log(`  content.${k}: ${JSON.stringify(v)}`);
  }

  if (!out.is_error && (typeof parsed["execution_ref"] === "string" || parsed["decision"] === "allow")) {
    console.log(`\n✓ allow — execution_ref ${parsed["execution_ref"] ?? "(none)"}`);
    console.log(`  audit_event_id: ${parsed["audit_event_id"] ?? "(unknown)"}`);
    return;
  }
  console.log(`\n✗ ${parsed["error"] ?? "unknown"} — ${JSON.stringify(parsed)}`);
  process.exitCode = 2;
}

main().catch((err: unknown) => {
  console.error(`smoke failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(2);
});
