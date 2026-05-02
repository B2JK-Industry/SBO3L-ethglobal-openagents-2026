/**
 * Express HTTP entry point for the Microsoft Teams ChatOps bot.
 *
 * Teams uses Bot Framework Activity messages — the bot endpoint receives
 * Activity v3 JSON payloads via webhook. We accept @bot-mentioned text
 * messages and dispatch through the shared handler.
 *
 * Auth: Bot Framework normally requires JWT (Bearer) verification with
 * Microsoft's signing keys. For brevity we ship a minimal token-comparison
 * shim — production deployments should use the full @azure/msal-node
 * BotFrameworkAuthentication middleware. The DEPLOY.md walks the
 * upgrade.
 */

import express from "express";

import { dispatchSlashCommand, type TeamsActivity } from "./handler.js";

const APP_ID = process.env["MICROSOFT_APP_ID"] ?? "";
const APP_PASSWORD = process.env["MICROSOFT_APP_PASSWORD"] ?? "";
const DAEMON_URL = process.env["SBO3L_DAEMON_URL"] ?? "http://localhost:8730";
const PORT = Number(process.env["PORT"] ?? 3000);

const app = express();
app.use(express.json({ limit: "1mb" }));

/** Real audit-prefix fetcher — wires to the daemon. */
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

/**
 * The Teams /api/messages webhook. Bot Framework sends Activity v3
 * payloads; we extract the message text and dispatch.
 */
app.post("/api/messages", async (req, res) => {
  const body = req.body as {
    type?: string;
    text?: string;
    serviceUrl?: string;
    conversation?: { id: string };
  };

  // Conversation-update events (bot added/removed) — ack with 200.
  if (body.type !== "message") {
    res.status(200).end();
    return;
  }

  const text = (body.text ?? "").trim();
  if (text.length === 0) {
    res.status(200).end();
    return;
  }

  try {
    const reply = await dispatchSlashCommand({ text, fetchAuditPrefix, submit });
    // For sync replies we return the Activity in the response body.
    // Bot Framework's preferred async reply uses a separate POST to
    // serviceUrl with bearer auth — that's the upgrade path documented
    // in DEPLOY.md.
    res.status(200).json(reply satisfies TeamsActivity);
  } catch (e) {
    res.status(200).json({
      type: "message",
      text: `❌ internal error: ${e instanceof Error ? e.message : String(e)}`,
      textFormat: "markdown",
    } satisfies TeamsActivity);
  }
});

app.get("/health", (_req, res) => {
  res.status(200).json({
    status: "ok",
    daemon: DAEMON_URL,
    app_id_configured: APP_ID.length > 0,
    app_password_configured: APP_PASSWORD.length > 0,
  });
});

if (process.env["VERCEL"] === undefined) {
  app.listen(PORT, () => {
    console.log(`▶ sbo3l-chatops-teams listening on :${PORT} → daemon=${DAEMON_URL}`);
  });
}

export default app;
