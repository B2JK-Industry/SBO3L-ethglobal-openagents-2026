/**
 * vercel-ai-keeperhub-demo — minimum viable SBO3L → KH policy gate
 * wired as a Vercel AI SDK `tool()`.
 *
 * Runs two ways:
 *
 *   1. Plain Node:  `node agent.mjs`
 *      Calls the tool's `execute()` directly with a sample APRP — no
 *      OpenAI API key needed. Prints the envelope. This is the path
 *      used by CI / judges who don't want to provision an OpenAI key.
 *
 *   2. Vercel Edge / Next.js Route Handler:  drop the `handler` export
 *      into `app/api/agent/route.ts`. With `OPENAI_API_KEY` set, it
 *      streams a `streamText` reply where the LLM picks `sbo3lKeeperHub`
 *      as a tool.
 *
 * Expected (path 1): ALLOW envelope with kh_execution_ref populated
 * (assuming a daemon at SBO3L_ENDPOINT + the bundled reference policy).
 */

import { randomUUID } from "node:crypto";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lVercelAIKeeperHubTool } from "@sbo3l/vercel-ai-keeperhub";

function aprp() {
  return {
    // research-agent-01 is the only agent_id registered in the bundled
    // reference policy. Demos that hardcode a different id are denied
    // before policy evaluation (auth.agent_not_found).
    agent_id: "research-agent-01",
    task_id: `vercel-ai-kh-${randomUUID().slice(0, 8)}`,
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
    expiry: new Date(Date.now() + 5 * 60_000).toISOString(),
    nonce: randomUUID(),
    risk_class: "low",
  };
}

/**
 * Path 2 — Edge / Route Handler entry point. Use as:
 *
 *   // app/api/agent/route.ts
 *   export { handler as POST } from "./agent.mjs";
 *
 * Requires OPENAI_API_KEY at runtime.
 */
export async function handler(req) {
  const { streamText } = await import("ai");
  const { openai } = await import("@ai-sdk/openai");

  const endpoint = process.env.SBO3L_ENDPOINT ?? "http://localhost:8730";
  const client = new SBO3LClient({ endpoint });

  const { messages } = await req.json();

  const result = streamText({
    model: openai("gpt-4o"),
    tools: {
      sbo3lKeeperHub: sbo3lVercelAIKeeperHubTool({ client }),
    },
    messages,
  });

  return result.toDataStreamResponse();
}

/**
 * Path 1 — direct tool execute (no OpenAI key needed).
 */
async function main() {
  const endpoint = process.env.SBO3L_ENDPOINT ?? "http://localhost:8730";
  console.log(`> daemon: ${endpoint}`);

  const client = new SBO3LClient({ endpoint });
  const tool = sbo3lVercelAIKeeperHubTool({ client });

  // Vercel AI SDK's tool().execute is what the LLM would invoke. We call
  // it directly with a sample APRP — same code path the LLM exercises.
  const envelope = await tool.execute({ aprp: aprp() }, {});

  console.log("\n=== envelope ===");
  for (const [k, v] of Object.entries(envelope)) {
    console.log(`  ${k}: ${JSON.stringify(v)}`);
  }

  if (envelope.decision === "allow" && envelope.kh_execution_ref) {
    console.log(
      `\n+ allow + KH executed -> kh_execution_ref=${envelope.kh_execution_ref}`,
    );
    process.exit(0);
  }
  if ("error" in envelope) {
    console.log(`\n- transport error: ${envelope.error}`);
    process.exit(2);
  }
  console.log(`\n- unexpected decision: ${envelope.decision}`);
  process.exit(1);
}

// Only run main() when invoked directly, not when imported as Edge handler.
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    console.error("fatal:", e);
    process.exit(3);
  });
}
