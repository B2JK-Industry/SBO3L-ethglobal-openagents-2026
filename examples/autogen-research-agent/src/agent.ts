/**
 * AutoGen-shaped research agent — uses OpenAI's function-calling API
 * directly (the same shape AutoGen registers with the LLM via
 * register_for_llm). The function descriptors built by `@sbo3l/autogen`
 * plug into any AutoGen runtime that accepts {name, description,
 * parameters JSON Schema, async call}.
 *
 * For production AutoGen agents, register the descriptors via
 * `ConversableAgent.register_for_llm` (Python) or the equivalent TS API.
 *
 * Usage:
 *   export OPENAI_API_KEY=sk-...
 *   npm install
 *   npm run agent
 */

import OpenAI from "openai";
import {
  dataFetchFunction,
  buildSbo3lPayFunction,
  defaultClient,
  KH_WORKFLOW_ID,
} from "./tools.js";
import type { AutoGenFunctionDescriptor, SBO3LFunctionResult } from "@sbo3l/autogen";

const SYSTEM_PROMPT = `You are an autonomous research agent. The user wants to spend a small \
amount of money on an API call. You have two functions:

  - data_fetch: GET a JSON URL — useful for inspecting the provider before paying.
  - sbo3l_payment_request: submit an APRP to SBO3L for a policy decision. SBO3L will gate \
    the payment and route via KeeperHub workflow ${KH_WORKFLOW_ID} when allowed.

ALWAYS go through sbo3l_payment_request to make any payment.`;

async function main(): Promise<void> {
  if (process.env["OPENAI_API_KEY"] === undefined) {
    console.error("error: set OPENAI_API_KEY (or use `npm run smoke` for the no-LLM path).");
    process.exit(1);
  }

  const client = defaultClient();
  const sbo3lPayFn = buildSbo3lPayFunction(client);
  const fns: Record<string, AutoGenFunctionDescriptor> = {
    data_fetch: dataFetchFunction,
    sbo3l_payment_request: sbo3lPayFn,
  };

  const openai = new OpenAI();
  const userTask =
    process.argv[2] ??
    "Pay 0.05 USDC to https://api.example.com/v1/inference for an inference call. " +
      "Agent id research-agent-01, task demo-autogen-1, nonce 01HTAWX5K3R8YV9NQB7C6P2DGM, " +
      "expiry 2026-05-01T10:31:00Z, low risk, x402 protocol on base.";

  console.log(`▶ user: ${userTask}\n`);

  const messages: OpenAI.Chat.ChatCompletionMessageParam[] = [
    { role: "system", content: SYSTEM_PROMPT },
    { role: "user", content: userTask },
  ];
  const tools = Object.values(fns).map((f) => ({
    type: "function" as const,
    function: {
      name: f.name,
      description: f.description,
      parameters: f.parameters,
    },
  }));

  for (let step = 0; step < 6; step++) {
    const r = await openai.chat.completions.create({
      model: "gpt-4o-mini",
      messages,
      tools,
      tool_choice: "auto",
    });
    const msg = r.choices[0]?.message;
    if (msg === undefined) {
      console.error("error: empty completion");
      process.exit(1);
    }
    messages.push(msg);

    if (msg.tool_calls === undefined || msg.tool_calls.length === 0) {
      console.log(`▶ agent: ${msg.content ?? "(no content)"}`);
      return;
    }

    for (const call of msg.tool_calls) {
      const fn = fns[call.function.name];
      if (fn === undefined) {
        messages.push({
          role: "tool",
          tool_call_id: call.id,
          content: JSON.stringify({ error: `unknown function ${call.function.name}` }),
        });
        continue;
      }
      const args = JSON.parse(call.function.arguments) as Record<string, unknown>;
      console.log(`  → ${call.function.name}(${JSON.stringify(args).slice(0, 80)}...)`);
      const result: unknown = await fn.call(args);
      const printable =
        (result as SBO3LFunctionResult).decision !== undefined
          ? `decision=${(result as SBO3LFunctionResult).decision}`
          : JSON.stringify(result).slice(0, 80);
      console.log(`    ← ${printable}`);
      messages.push({
        role: "tool",
        tool_call_id: call.id,
        content: JSON.stringify(result),
      });
    }
  }

  console.error("error: agent did not finish in 6 steps");
  process.exit(1);
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
