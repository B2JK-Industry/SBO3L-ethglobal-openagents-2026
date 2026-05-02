/**
 * Two tools the Vercel AI research agent reasons across:
 *
 *   1. `data_fetch` — GET a JSON URL (research before paying). Built via
 *      `ai.tool()` with a zod schema so the LLM is constrained to a {url}
 *      argument shape.
 *   2. `sbo3l_pay` — wraps `@sbo3l/sdk`'s SBO3LClient via `@sbo3l/vercel-ai`.
 *      Real APRP submit; on `allow` returns the signed PolicyReceipt; on
 *      `deny` throws `PolicyDenyError` so the LLM sees the deny code.
 *      Routes to KeeperHub workflow `m4t4cnpmhv8qquce3bv3c` when allowed.
 */

import { tool } from "ai";
import { z } from "zod";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool } from "@sbo3l/vercel-ai";

/** Live KeeperHub workflow id verified end-to-end on 2026-04-30. */
export const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

/** `data_fetch` — GET a JSON URL, return body. */
export const dataFetchTool = tool({
  description:
    "Fetch a JSON document from a URL. Use this BEFORE deciding to spend money on an API.",
  parameters: z.object({
    url: z.string().url().describe("Absolute URL to GET. Returns body and HTTP status."),
  }),
  execute: async ({ url }): Promise<{ status: number; body: string } | { error: string }> => {
    try {
      const r = await fetch(url, { headers: { Accept: "application/json" } });
      const text = await r.text();
      return { status: r.status, body: text.slice(0, 2000) };
    } catch (e) {
      return { error: e instanceof Error ? e.message : String(e) };
    }
  },
});

/** Build the SBO3L `pay` tool wired into the research agent. */
export function buildSbo3lTool(client: SBO3LClient): ReturnType<typeof sbo3lTool> {
  return sbo3lTool({ client });
}

/** Build a default SBO3L client from env (SBO3L_ENDPOINT, SBO3L_BEARER_TOKEN). */
export function defaultClient(): SBO3LClient {
  const endpoint = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
  const bearer = process.env["SBO3L_BEARER_TOKEN"];
  return new SBO3LClient({
    endpoint,
    ...(bearer !== undefined ? { auth: { kind: "bearer" as const, token: bearer } } : {}),
  });
}
