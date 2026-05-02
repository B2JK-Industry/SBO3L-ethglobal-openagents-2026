/**
 * Two tools the LangChain research agent reasons across:
 *
 *   1. `data_fetch` — fetch a JSON snippet from a public URL. Used as the
 *      "research" step before deciding whether the result is worth paying
 *      a downstream API for.
 *   2. `sbo3l_pay` — wrap @sbo3l/sdk via @sbo3l/langchain so every payment
 *      passes through SBO3L's policy boundary. Routes to KeeperHub workflow
 *      `m4t4cnpmhv8qquce3bv3c` (the live KH workflow verified in
 *      `submission_2026-04-30_live_verification.md`) when the daemon's KH
 *      adapter is in `live` mode.
 */

import { DynamicTool } from "@langchain/core/tools";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lTool, type SBO3LClientLike } from "@sbo3l/langchain";

/** Live KeeperHub workflow id verified end-to-end on 2026-04-30. */
export const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

/**
 * `data_fetch` — minimal HTTP GET tool. Returns the JSON body so the LLM
 * can reason over it. Real research agents would use a search engine or
 * vector store; we keep this tiny so the demo runs <30s.
 */
export const dataFetchTool = new DynamicTool({
  name: "data_fetch",
  description:
    "Fetch a JSON document from a URL. Input MUST be a JSON-stringified object {\"url\": \"https://...\"}. " +
    "Returns the response body as a JSON string. Use this BEFORE deciding to spend money on an API.",
  func: async (input: string): Promise<string> => {
    let url: string;
    try {
      const parsed = JSON.parse(input) as { url?: string };
      if (typeof parsed.url !== "string") return JSON.stringify({ error: "missing 'url' field" });
      url = parsed.url;
    } catch (e) {
      return JSON.stringify({
        error: "input must be JSON {\"url\": ...}",
        detail: e instanceof Error ? e.message : String(e),
      });
    }
    try {
      const r = await fetch(url, { headers: { Accept: "application/json" } });
      const text = await r.text();
      return JSON.stringify({ status: r.status, body: text.slice(0, 2000) });
    } catch (e) {
      return JSON.stringify({
        error: "fetch failed",
        detail: e instanceof Error ? e.message : String(e),
      });
    }
  },
});

/** Build the SBO3L payment tool wired into the research agent.
 *
 * The structural cast (`as unknown as SBO3LClientLike`) bridges
 * `SBO3LClient` (whose `submit` takes the typed `PaymentRequest`) to the
 * integration's wider `SBO3LClientLike` shape (whose `submit` takes
 * `Record<string, unknown>`). Runtime is identical — `PaymentRequest` IS
 * a record at runtime; the cast only quiets TypeScript's strict variance.
 */
export function buildSbo3lPayTool(client: SBO3LClient): DynamicTool {
  const desc = sbo3lTool({ client: client as unknown as SBO3LClientLike });
  return new DynamicTool({
    name: desc.name,
    description: desc.description,
    func: desc.func,
  });
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
