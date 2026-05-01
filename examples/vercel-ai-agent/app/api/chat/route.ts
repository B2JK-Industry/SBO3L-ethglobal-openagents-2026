/**
 * Next.js Route Handler — minimal Vercel AI SDK + SBO3L example.
 *
 * POST /api/chat with `{ messages: [{ role, content }] }` →
 * the LLM streams tokens; if it decides to pay, it calls the `pay` tool;
 * SBO3L's policy boundary decides allow/deny; the LLM sees the receipt
 * (allow) or `PolicyDenyError` (deny) and continues.
 */

import { openai } from "@ai-sdk/openai";
import { streamText } from "ai";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool } from "@sbo3l/vercel-ai";

export const runtime = "nodejs";
export const maxDuration = 30;

const client = new SBO3LClient({
  endpoint: process.env.SBO3L_ENDPOINT ?? "http://localhost:8730",
  ...(process.env.SBO3L_BEARER_TOKEN !== undefined
    ? { auth: { kind: "bearer" as const, token: process.env.SBO3L_BEARER_TOKEN } }
    : {}),
});

export async function POST(req: Request): Promise<Response> {
  const { messages } = (await req.json()) as { messages: Array<{ role: string; content: string }> };

  const result = await streamText({
    model: openai("gpt-4o-mini"),
    system:
      "You are an autonomous research agent. When the user asks you to make a payment, " +
      "ALWAYS go through the `pay` tool — never claim a payment was made without it. " +
      "If the policy denies your payment, explain the deny_code to the user and suggest " +
      "what they can change (lower amount, different token, etc.).",
    messages: messages.map((m) => ({ role: m.role as "user" | "assistant", content: m.content })),
    tools: { pay: sbo3lTool({ client }) },
    maxSteps: 5,
  });

  return result.toDataStreamResponse();
}
