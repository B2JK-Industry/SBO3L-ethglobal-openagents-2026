/**
 * Minimal SBO3L TypeScript example agent.
 *
 *   1. Loads a golden APRP from `test-corpus/`.
 *   2. Submits it via `@sbo3l/sdk`.
 *   3. Prints decision + execution_ref.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { SBO3LClient, type PaymentRequest } from "@sbo3l/sdk";

const REPO_ROOT = resolve(import.meta.dirname, "..", "..", "..");
const GOLDEN = resolve(REPO_ROOT, "test-corpus", "aprp", "golden_001_minimal.json");

async function main(): Promise<void> {
  const endpoint = process.env["SBO3L_ENDPOINT"] ?? "http://localhost:8730";
  const bearer = process.env["SBO3L_BEARER_TOKEN"];

  const aprp = JSON.parse(readFileSync(GOLDEN, "utf-8")) as PaymentRequest;
  const client = new SBO3LClient({
    endpoint,
    ...(bearer !== undefined ? { auth: { kind: "bearer", token: bearer } } : {}),
  });

  const r = await client.submit(aprp);
  console.log(`decision: ${r.decision}`);
  console.log(`execution_ref: ${r.receipt.execution_ref ?? "(none)"}`);
  console.log(`audit_event_id: ${r.audit_event_id}`);
  console.log(`request_hash: ${r.request_hash}`);
  console.log(`policy_hash: ${r.policy_hash}`);
}

main().catch((err: unknown) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`error: ${msg}`);
  process.exit(1);
});
