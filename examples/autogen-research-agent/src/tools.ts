/**
 * Two AutoGen-shaped function descriptors:
 *
 *   1. data_fetch — GET a JSON URL. JSON Schema for {url} parameter.
 *   2. sbo3l_payment_request — built via @sbo3l/autogen's sbo3lFunction.
 *      Real @sbo3l/sdk SBO3LClient under the hood. Routes to KeeperHub
 *      workflow m4t4cnpmhv8qquce3bv3c when SBO3L allows.
 */

import { SBO3LClient } from "@sbo3l/sdk";
import {
  sbo3lFunction,
  type AutoGenFunctionDescriptor,
  type SBO3LClientLike,
} from "@sbo3l/autogen";

/** Live KeeperHub workflow id verified end-to-end on 2026-04-30. */
export const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

/** AutoGen-shaped data_fetch function. JSON Schema constrains the LLM's args. */
export const dataFetchFunction: AutoGenFunctionDescriptor = {
  name: "data_fetch",
  description:
    "GET a JSON URL and return its body. Use this BEFORE deciding to spend money on an API.",
  parameters: {
    type: "object",
    required: ["url"],
    properties: {
      url: { type: "string", description: "Absolute URL to GET." },
    },
  },
  call: async (args): Promise<{ status?: number; body?: string; error?: string }> => {
    const url = (args as { url?: unknown }).url;
    if (typeof url !== "string") return { error: "missing 'url' argument" };
    try {
      const r = await fetch(url, { headers: { Accept: "application/json" } });
      const text = await r.text();
      return { status: r.status, body: text.slice(0, 2000) };
    } catch (e) {
      return { error: e instanceof Error ? e.message : String(e) };
    }
  },
};

/** Build the SBO3L payment function descriptor wired into the research agent.
 *
 * Cast bridges `SBO3LClient` (typed `submit(PaymentRequest)`) to the
 * integration's wider `SBO3LClientLike` shape — runtime identical, only
 * quiets TypeScript variance. */
export function buildSbo3lPayFunction(client: SBO3LClient): AutoGenFunctionDescriptor {
  return sbo3lFunction({ client: client as unknown as SBO3LClientLike });
}

/** Build a default SBO3L client from env. */
export function defaultClient(): SBO3LClient {
  const endpoint = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
  const bearer = process.env["SBO3L_BEARER_TOKEN"];
  return new SBO3LClient({
    endpoint,
    ...(bearer !== undefined ? { auth: { kind: "bearer" as const, token: bearer } } : {}),
  });
}
