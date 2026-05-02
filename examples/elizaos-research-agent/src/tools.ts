/**
 * ElizaOS demo wiring:
 *
 *   - data_fetch: standalone callable (ElizaOS plugins can wrap arbitrary
 *     callables as Actions; for the demo we expose this directly).
 *   - sbo3l plugin: built via @sbo3l/elizaos's `sbo3lPlugin({ client })`.
 *     The plugin's `SBO3L_PAYMENT_REQUEST` Action triggers when message
 *     content contains an APRP (object on `message.content.aprp` or JSON
 *     string in `message.content.text`).
 *
 * KH workflow target: m4t4cnpmhv8qquce3bv3c.
 */

import { SBO3LClient } from "@sbo3l/sdk";
import {
  sbo3lPlugin,
  type ElizaPlugin,
  type SBO3LClientLike,
} from "@sbo3l/elizaos";

/** Live KeeperHub workflow id verified end-to-end on 2026-04-30. */
export const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

/** Fetch a JSON URL — return body or error. */
export async function fetchUrl(url: string): Promise<{ status?: number; body?: string; error?: string }> {
  try {
    const r = await fetch(url, { headers: { Accept: "application/json" } });
    const text = await r.text();
    return { status: r.status, body: text.slice(0, 2000) };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

/** Build the SBO3L Eliza plugin with structural-typing cast for the real client. */
export function buildSbo3lPlugin(client: SBO3LClient): ElizaPlugin {
  return sbo3lPlugin({ client: client as unknown as SBO3LClientLike });
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
