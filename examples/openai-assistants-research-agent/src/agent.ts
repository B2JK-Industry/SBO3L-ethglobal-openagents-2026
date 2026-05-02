/**
 * 3-step OpenAI Assistants research agent gated by SBO3L.
 *
 * Walks through:
 *   1. Create an assistant with the SBO3L tool wired in
 *   2. Open a thread, post the user goal, start a run
 *   3. Poll the run; on `requires_action`, dispatch each `tool_call` via
 *      `runSbo3lToolCall` and submit the outputs back
 *   4. Print the final assistant message + every audit_event_id we collected
 *
 * Requires OPENAI_API_KEY + a running SBO3L daemon. For a deterministic
 * no-LLM verification, run `npm run smoke` instead.
 */

import OpenAI from "openai";
import { SBO3LClient } from "@sbo3l/sdk";
import {
  PolicyDenyError,
  runSbo3lToolCall,
  sbo3lAssistantTool,
} from "@sbo3l/openai-assistants";

const ENDPOINT = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
const MODEL = process.env["OPENAI_MODEL"] ?? "gpt-4o-mini";

async function main(): Promise<void> {
  if (process.env["OPENAI_API_KEY"] === undefined) {
    console.error("OPENAI_API_KEY required. For a no-LLM verification, run `npm run smoke`.");
    process.exit(1);
  }

  const openai = new OpenAI();
  const client = new SBO3LClient({ endpoint: ENDPOINT });
  const tool = sbo3lAssistantTool({ client });

  console.log(`▶ creating assistant (${MODEL}) with SBO3L tool wired in`);
  const assistant = await openai.beta.assistants.create({
    model: MODEL,
    name: "SBO3L research agent",
    instructions:
      "You are a research agent that pays for API calls through the SBO3L policy " +
      "boundary. ALWAYS call sbo3l_payment_request BEFORE attempting any paid action. " +
      "On deny, branch on deny_code and either retry with adjusted parameters or " +
      "explain why you cannot proceed. Use chain='base', token='USDC', " +
      "intent='purchase_api_call', risk_class='low'. The allowed recipient on " +
      "chain 'base' is 0x1111111111111111111111111111111111111111. Generate fresh " +
      "ULID/UUID nonces and a 5-minute expiry per request.",
    tools: [tool.definition],
  });

  console.log(`▶ opening thread + posting user goal`);
  const thread = await openai.beta.threads.create({});
  await openai.beta.threads.messages.create(thread.id, {
    role: "user",
    content:
      "Run a $0.05 paid inference call against api.example.com/v1/inference. " +
      "Submit the APRP through SBO3L first, then summarise the receipt.",
  });

  let run = await openai.beta.threads.runs.create(thread.id, { assistant_id: assistant.id });
  const auditEventIds: string[] = [];
  console.log(`▶ run started (${run.id}); polling`);

  for (;;) {
    if (run.status === "completed") break;
    if (run.status === "requires_action" && run.required_action?.type === "submit_tool_outputs") {
      const calls = run.required_action.submit_tool_outputs.tool_calls;
      console.log(`  ↳ requires_action: ${calls.length} tool_call(s)`);
      const outputs = await Promise.all(calls.map((c) => runSbo3lToolCall(tool, c)));
      for (const out of outputs) {
        try {
          const parsed = JSON.parse(out.output) as { audit_event_id?: string };
          if (typeof parsed.audit_event_id === "string") auditEventIds.push(parsed.audit_event_id);
        } catch {
          /* output may be a non-JSON string; ignore for the print path */
        }
      }
      run = await openai.beta.threads.runs.submitToolOutputs(thread.id, run.id, {
        tool_outputs: outputs,
      });
      continue;
    }
    if (run.status === "failed" || run.status === "cancelled" || run.status === "expired") {
      throw new PolicyDenyError("deny", "run.terminal", null, "(no audit)");
    }
    await new Promise((resolve) => setTimeout(resolve, 800));
    run = await openai.beta.threads.runs.retrieve(thread.id, run.id);
  }

  const messages = await openai.beta.threads.messages.list(thread.id, { order: "desc", limit: 1 });
  const reply = messages.data[0]?.content[0];
  console.log(`\n▶ assistant final message:`);
  if (reply?.type === "text") console.log(`  ${reply.text.value}`);
  else console.log(`  (non-text reply: ${reply?.type ?? "none"})`);

  console.log(`\n▶ audit_event_ids collected (${auditEventIds.length}):`);
  for (const id of auditEventIds) console.log(`  ${id}`);
}

main().catch((err: unknown) => {
  console.error(`agent failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(2);
});
