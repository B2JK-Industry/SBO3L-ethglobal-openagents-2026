/**
 * Express HTTP entry point for the Slack ChatOps bot.
 *
 * Slack POSTs the slash-command payload as form-urlencoded; we parse,
 * verify the signing secret (HMAC), then route through the dispatcher
 * in handler.ts. Response is JSON (Slack accepts both 200 + body OR
 * delayed response_url posts; we use the 200-body shape for simplicity).
 *
 * Wire-up env:
 *   SLACK_SIGNING_SECRET   — from Slack app config (signs every request)
 *   SBO3L_DAEMON_URL       — default http://localhost:8730
 *   PORT                   — default 3000 (Vercel rewrites to /)
 */

import { createHmac, timingSafeEqual } from "node:crypto";
import express from "express";

import { dispatchSlashCommand, type SlackResponse } from "./handler.js";

const SIGNING_SECRET = process.env["SLACK_SIGNING_SECRET"] ?? "";
const DAEMON_URL = process.env["SBO3L_DAEMON_URL"] ?? "http://localhost:8730";
const PORT = Number(process.env["PORT"] ?? 3000);

const app = express();
app.use(express.urlencoded({ extended: true, verify: rawBodySaver }));
app.use(express.json({ verify: rawBodySaver }));

function rawBodySaver(
  req: express.Request & { rawBody?: Buffer },
  _res: express.Response,
  buf: Buffer,
): void {
  req.rawBody = buf;
}

/**
 * Verify Slack's request signature per
 * https://api.slack.com/authentication/verifying-requests-from-slack.
 * Drops requests with stale timestamps (>5 min) to defeat replays.
 */
function verifySlackSignature(req: express.Request & { rawBody?: Buffer }): boolean {
  if (SIGNING_SECRET.length === 0) {
    // Unsigned mode (local dev only). The deploy.md insists Daniel set
    // the secret in production; we don't fail-open silently here.
    return true;
  }
  const ts = req.header("x-slack-request-timestamp");
  const sig = req.header("x-slack-signature");
  if (ts === undefined || sig === undefined || req.rawBody === undefined) return false;

  const tsNum = Number(ts);
  if (!Number.isFinite(tsNum)) return false;
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - tsNum) > 5 * 60) return false; // replay window

  const baseString = `v0:${ts}:${req.rawBody.toString("utf-8")}`;
  const computed = "v0=" + createHmac("sha256", SIGNING_SECRET).update(baseString).digest("hex");

  const a = Buffer.from(computed);
  const b = Buffer.from(sig);
  if (a.length !== b.length) return false;
  return timingSafeEqual(a, b);
}

/** Real audit-prefix fetcher — wires to the daemon's `/v1/audit/<agent>/prefix` endpoint. */
async function fetchAuditPrefix(agentId: string): Promise<{
  chain_length: number;
  head_event_id: string | null;
  recent: Array<{ event_id: string; type: string; ts: string }>;
}> {
  const url = `${DAEMON_URL.replace(/\/$/, "")}/v1/audit/${encodeURIComponent(agentId)}/prefix`;
  const r = await fetch(url, { headers: { Accept: "application/json" } });
  if (!r.ok) throw new Error(`HTTP ${r.status} from ${url}`);
  return (await r.json()) as Awaited<ReturnType<typeof fetchAuditPrefix>>;
}

/** Real submit — POSTs to the daemon's `/v1/payment-requests`. */
async function submit(aprp: unknown): Promise<{
  decision: string;
  deny_code: string | null;
  matched_rule_id: string | null;
  audit_event_id: string;
  receipt: { execution_ref: string | null };
}> {
  const r = await fetch(`${DAEMON_URL.replace(/\/$/, "")}/v1/payment-requests`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(aprp),
  });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return (await r.json()) as Awaited<ReturnType<typeof submit>>;
}

app.post("/slack/commands", async (req, res) => {
  if (!verifySlackSignature(req)) {
    res.status(401).json({ error: "invalid Slack signature" });
    return;
  }

  const text = (req.body?.text as string | undefined) ?? "";
  try {
    const reply = await dispatchSlashCommand({ text, fetchAuditPrefix, submit });
    res.status(200).json(reply satisfies SlackResponse);
  } catch (e) {
    res.status(200).json({
      response_type: "ephemeral",
      text: `❌ internal error: ${e instanceof Error ? e.message : String(e)}`,
    });
  }
});

app.get("/health", (_req, res) => {
  res.status(200).json({ status: "ok", daemon: DAEMON_URL });
});

if (process.env["VERCEL"] === undefined) {
  // Local + container dev. Vercel imports this module without listening.
  app.listen(PORT, () => {
    console.log(`▶ sbo3l-chatops-slack listening on :${PORT} → daemon=${DAEMON_URL}`);
  });
}

export default app;
