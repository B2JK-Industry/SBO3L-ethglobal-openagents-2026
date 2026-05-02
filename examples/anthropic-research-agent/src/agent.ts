/**
 * Claude tool-use research agent gated by SBO3L.
 *
 * Walks through:
 *   1. Build the SBO3L tool definition
 *   2. Send a user goal to messages.create with that tool wired in
 *   3. Loop while stop_reason === "tool_use": dispatch each tool_use,
 *      push tool_result blocks back, re-call messages.create
 *   4. Print Claude's final text reply + every audit_event_id collected
 *
 * Requires ANTHROPIC_API_KEY + a running SBO3L daemon. For deterministic
 * no-LLM verification, run `npm run smoke` instead.
 */

import Anthropic from "@anthropic-ai/sdk";
import { SBO3LClient } from "@sbo3l/sdk";
import { runSbo3lToolUse, sbo3lTool } from "@sbo3l/anthropic";

const ENDPOINT = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
const MODEL = process.env["ANTHROPIC_MODEL"] ?? "claude-3-5-sonnet-latest";

async function main(): Promise<void> {
  if (process.env["ANTHROPIC_API_KEY"] === undefined) {
    console.error("ANTHROPIC_API_KEY required. For a no-LLM verification, run `npm run smoke`.");
    process.exit(1);
  }

  const claude = new Anthropic();
  const client = new SBO3LClient({ endpoint: ENDPOINT });
  const tool = sbo3lTool({ client });

  console.log(`▶ wiring tool '${tool.name}' into messages.create (${MODEL})`);

  const messages: Anthropic.MessageParam[] = [
    {
      role: "user",
      content:
        "Run a $0.05 paid inference call against api.example.com/v1/inference. " +
        "Submit the APRP through SBO3L first, then summarise the receipt. " +
        "Use chain='base', token='USDC', intent='purchase_api_call', risk_class='low'. " +
        "Allowed recipient on chain 'base' is 0x1111111111111111111111111111111111111111. " +
        "Generate a fresh ULID/UUID nonce + 5-minute expiry per request.",
    },
  ];

  const auditEventIds: string[] = [];
  const SAFETY_TURNS = 6;

  for (let turn = 0; turn < SAFETY_TURNS; turn++) {
    const r = await claude.messages.create({
      model: MODEL,
      max_tokens: 1024,
      tools: [tool.definition],
      messages,
    });

    messages.push({ role: "assistant", content: r.content });

    if (r.stop_reason !== "tool_use") {
      console.log(`\n▶ Claude final reply (stop_reason=${r.stop_reason}):`);
      for (const block of r.content) {
        if (block.type === "text") console.log(`  ${block.text}`);
      }
      console.log(`\n▶ audit_event_ids collected (${auditEventIds.length}):`);
      for (const id of auditEventIds) console.log(`  ${id}`);
      return;
    }

    const toolUses = r.content.filter(
      (c): c is Anthropic.ToolUseBlock => c.type === "tool_use",
    );
    console.log(`  ↳ turn ${turn + 1}: ${toolUses.length} tool_use block(s)`);
    const results = await Promise.all(
      toolUses.map((u) =>
        runSbo3lToolUse(tool, {
          type: "tool_use",
          id: u.id,
          name: u.name,
          input: u.input,
        }),
      ),
    );
    for (const res of results) {
      try {
        const parsed = JSON.parse(res.content) as { audit_event_id?: string };
        if (typeof parsed.audit_event_id === "string") auditEventIds.push(parsed.audit_event_id);
      } catch {
        /* non-JSON output; skip for the audit print path */
      }
    }
    messages.push({ role: "user", content: results });
  }
  console.log(`\n✗ exceeded ${SAFETY_TURNS} turns without stop_reason !== tool_use — aborting`);
}

main().catch((err: unknown) => {
  console.error(`agent failed: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(2);
});
