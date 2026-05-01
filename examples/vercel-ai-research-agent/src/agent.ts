/**
 * Vercel AI SDK research agent — `generateText` with two tools and an LLM
 * that picks the call sequence. Demonstrates the agent paying through SBO3L.
 *
 * Prereq: OPENAI_API_KEY in env, plus a running SBO3L daemon
 * (`SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server`).
 *
 * Usage:
 *   export OPENAI_API_KEY=sk-...
 *   npm install
 *   npm run agent  # or: tsx src/agent.ts "<prompt>"
 */

import { openai } from "@ai-sdk/openai";
import { generateText } from "ai";
import { dataFetchTool, buildSbo3lTool, defaultClient, KH_WORKFLOW_ID } from "./tools.js";

const SYSTEM_PROMPT = `You are an autonomous research agent. The user wants to spend a small \
amount of money on an API call. You have two tools:

  - data_fetch: GET a JSON URL, useful for inspecting the provider before paying.
  - sbo3l_payment_request: submit an APRP (Agent Payment Request Protocol) to SBO3L for a \
    policy decision. SBO3L gates the payment and routes via KeeperHub workflow ${KH_WORKFLOW_ID} \
    when allowed.

ALWAYS: call sbo3l_payment_request to make any payment — never claim a payment was made \
without it. On a deny, the tool will throw PolicyDenyError; explain the deny_code to the user.`;

async function main(): Promise<void> {
  if (process.env["OPENAI_API_KEY"] === undefined) {
    console.error("error: set OPENAI_API_KEY (or use `npm run smoke` for the no-LLM path).");
    process.exit(1);
  }

  const client = defaultClient();
  const tools = {
    data_fetch: dataFetchTool,
    sbo3l_payment_request: buildSbo3lTool(client),
  };

  const userTask =
    process.argv[2] ??
    "Pay 0.05 USDC to https://api.example.com/v1/inference for an inference call. " +
      "Agent id research-agent-01, task demo-vercel-ai-1, nonce 01HTAWX5K3R8YV9NQB7C6P2DGM, " +
      "expiry 2026-05-01T10:31:00Z, low risk, x402 protocol on base.";

  console.log(`▶ user: ${userTask}\n`);

  const result = await generateText({
    model: openai("gpt-4o-mini"),
    system: SYSTEM_PROMPT,
    prompt: userTask,
    tools,
    maxSteps: 6,
  });

  console.log(`\n▶ agent: ${result.text}`);
  console.log(
    `\n▶ tool calls: ${result.toolCalls.map((c) => c.toolName).join(" → ") || "(none)"}`,
  );
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
