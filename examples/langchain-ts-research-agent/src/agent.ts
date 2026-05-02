/**
 * LangChain JS research agent — reasons across 2 tools and asks SBO3L to
 * authorize each financial action. Uses OpenAI's function-calling agent so
 * the LLM picks tool calls + arguments without bespoke prompts.
 *
 * Flow:
 *   1. User: "Should I pay 0.05 USDC for an inference call to api.example.com?"
 *   2. Agent calls `data_fetch` to inspect the provider's metadata.
 *   3. Agent reasons over the result.
 *   4. Agent calls `sbo3l_payment_request` with an APRP body.
 *   5. SBO3L decides allow/deny → KH workflow `m4t4cnpmhv8qquce3bv3c` fires.
 *   6. Agent reports the signed receipt.
 *
 * Prereq: OPENAI_API_KEY in env, plus a running SBO3L daemon
 * (`SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server`).
 */

import { ChatOpenAI } from "@langchain/openai";
import { AgentExecutor, createOpenAIFunctionsAgent } from "langchain/agents";
import { ChatPromptTemplate } from "@langchain/core/prompts";
import { dataFetchTool, buildSbo3lPayTool, defaultClient, KH_WORKFLOW_ID } from "./tools.js";

const SYSTEM_PROMPT = `You are an autonomous research agent. The user wants to spend a small \
amount of money on an API call. You have two tools:

  - data_fetch: GET a JSON URL, useful for inspecting the provider before paying.
  - sbo3l_payment_request: submit an APRP (Agent Payment Request Protocol) to SBO3L for a \
    policy decision. SBO3L will gate the payment and route via KeeperHub workflow ${KH_WORKFLOW_ID} \
    when allowed.

ALWAYS: (a) call data_fetch first if you have a URL, (b) call sbo3l_payment_request to make any \
payment — never claim a payment was made without it, (c) on a deny, branch on deny_code and \
explain to the user.`;

async function main(): Promise<void> {
  if (process.env["OPENAI_API_KEY"] === undefined) {
    console.error("error: set OPENAI_API_KEY (or use `npm run smoke` for the no-LLM path).");
    process.exit(1);
  }

  const client = defaultClient();
  const tools = [dataFetchTool, buildSbo3lPayTool(client)];

  const llm = new ChatOpenAI({ model: "gpt-4o-mini", temperature: 0 });
  const prompt = ChatPromptTemplate.fromMessages([
    ["system", SYSTEM_PROMPT],
    ["user", "{input}"],
    ["placeholder", "{agent_scratchpad}"],
  ]);

  const agent = await createOpenAIFunctionsAgent({ llm, tools, prompt });
  const executor = new AgentExecutor({ agent, tools, maxIterations: 6, verbose: false });

  const userTask =
    process.argv[2] ??
    "Pay 0.05 USDC to https://api.example.com/v1/inference for an inference call. " +
      "Agent id research-agent-01, task demo-langchain-1, nonce 01HTAWX5K3R8YV9NQB7C6P2DGM, " +
      "expiry 2026-05-01T10:31:00Z, low risk, x402 protocol on base.";

  console.log(`▶ user: ${userTask}\n`);
  const result = await executor.invoke({ input: userTask });
  console.log(`\n▶ agent: ${(result as { output: string }).output}`);
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
