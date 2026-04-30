import type { IncomingMessage, ServerResponse } from "node:http";

import { DiscordWebhookTransport, DryRunTransport } from "../src/agent-bridge.js";
import { LinearGraphQLClient } from "../src/linear-client.js";
import { handleLinearWebhook } from "../src/linear-webhook.js";
import { verifyLinearSignature } from "../src/signature.js";
import type { LinearWebhookEvent } from "../src/types.js";

/**
 * Vercel HTTP entry point. Verifies the Linear webhook signature against the
 * raw body, parses, dispatches to the core handler, returns 2xx on success
 * (so Linear stops retrying) or 401/400/500 on validation/processing errors.
 */
export default async function handler(
  req: IncomingMessage,
  res: ServerResponse,
): Promise<void> {
  if (req.method !== "POST") {
    res.statusCode = 405;
    res.setHeader("Allow", "POST");
    res.end("Method Not Allowed");
    return;
  }

  let raw: string;
  try {
    raw = await readRawBody(req);
  } catch (err) {
    res.statusCode = 400;
    res.end(`bad body: ${(err as Error).message}`);
    return;
  }

  const secret = process.env["LINEAR_WEBHOOK_SECRET"];
  if (!secret) {
    res.statusCode = 500;
    res.end("server misconfigured: LINEAR_WEBHOOK_SECRET missing");
    return;
  }

  const sigHeader = headerValue(req.headers["linear-signature"]);
  if (!verifyLinearSignature(raw, sigHeader, secret)) {
    res.statusCode = 401;
    res.end("invalid signature");
    return;
  }

  let event: LinearWebhookEvent;
  try {
    event = JSON.parse(raw) as LinearWebhookEvent;
  } catch (err) {
    res.statusCode = 400;
    res.end(`bad json: ${(err as Error).message}`);
    return;
  }

  try {
    const apiKey = required("LINEAR_API_KEY");
    const inProgressStateId = required("LINEAR_STATE_IN_PROGRESS");
    const linear = new LinearGraphQLClient(apiKey, inProgressStateId);
    const dryRun = process.env["ORCHESTRATOR_DRY_RUN"] === "1";
    const transport = dryRun
      ? new DryRunTransport()
      : new DiscordWebhookTransport();

    const outcome = await handleLinearWebhook(event, {
      linear,
      transport,
      env: process.env,
    });

    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify(outcome));
  } catch (err) {
    res.statusCode = 500;
    res.end(`handler error: ${(err as Error).message}`);
  }
}

function readRawBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

function headerValue(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) return value[0];
  return value;
}

function required(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`missing required env var ${name}`);
  return v;
}
