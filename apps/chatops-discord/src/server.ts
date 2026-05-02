/**
 * Express HTTP entry point for the Discord ChatOps bot.
 *
 * Discord's interaction API:
 *   - POSTs JSON {type: 1|2|3|...} to the Interactions Endpoint URL
 *   - Type 1 = PING (heartbeat) — must reply with {type: 1} (PONG)
 *   - Type 2 = APPLICATION_COMMAND (slash command) — we dispatch
 *   - Every request signed by Discord's Ed25519 key (verify with tweetnacl)
 */

import { readFileSync } from "node:fs";
import express from "express";
import nacl from "tweetnacl";

import { dispatchSlashCommand, type DiscordResponse } from "./handler.js";

const PUBLIC_KEY_HEX = process.env["DISCORD_PUBLIC_KEY"] ?? "";
const DAEMON_URL = process.env["SBO3L_DAEMON_URL"] ?? "http://localhost:8730";
const PORT = Number(process.env["PORT"] ?? 3000);

const app = express();
app.use(
  express.json({
    verify: (req: express.Request & { rawBody?: Buffer }, _res, buf) => {
      req.rawBody = buf;
    },
  }),
);

/**
 * Verify Discord's Ed25519 signature per
 * https://discord.com/developers/docs/interactions/receiving-and-responding#security-and-authorization.
 *
 * Falls open in unsigned mode (PUBLIC_KEY_HEX empty) for local dev only —
 * DEPLOY.md insists Daniel set the key in production.
 */
function verifyDiscordSignature(req: express.Request & { rawBody?: Buffer }): boolean {
  if (PUBLIC_KEY_HEX.length === 0) return true; // dev mode
  const sig = req.header("x-signature-ed25519");
  const ts = req.header("x-signature-timestamp");
  if (sig === undefined || ts === undefined || req.rawBody === undefined) return false;

  try {
    const message = Buffer.concat([Buffer.from(ts, "utf-8"), req.rawBody]);
    return nacl.sign.detached.verify(
      message,
      Buffer.from(sig, "hex"),
      Buffer.from(PUBLIC_KEY_HEX, "hex"),
    );
  } catch {
    return false;
  }
}

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

/** Real submit — POSTs to the daemon's /v1/payment-requests. */
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

app.post("/discord/interactions", async (req, res) => {
  if (!verifyDiscordSignature(req)) {
    res.status(401).send("invalid request signature");
    return;
  }

  const body = req.body as { type: number; data?: { name: string; options?: Array<{ name: string; value?: string; options?: Array<{ name: string; value?: string }> }> } };
  if (body.type === 1) {
    res.status(200).json({ type: 1 } satisfies DiscordResponse);
    return;
  }
  if (body.type !== 2) {
    res.status(400).json({ error: "unsupported interaction type" });
    return;
  }

  // Discord encodes subcommands in the options tree:
  //   /sbo3l verify capsule:<json>  →
  //     data.options = [{ name: "verify", options: [{ name: "capsule", value: "<json>" }] }]
  const sub = body.data?.options?.[0];
  const subcommand = sub?.name ?? "help";
  const option = sub?.options?.[0]?.value ?? "";

  try {
    const reply = await dispatchSlashCommand({ subcommand, option, fetchAuditPrefix, submit });
    res.status(200).json(reply satisfies DiscordResponse);
  } catch (e) {
    res.status(200).json({
      type: 4,
      data: {
        content: `❌ internal error: ${e instanceof Error ? e.message : String(e)}`,
        flags: 64,
      },
    });
  }
});

app.get("/health", (_req, res) => {
  res.status(200).json({ status: "ok", daemon: DAEMON_URL });
});

if (process.env["VERCEL"] === undefined) {
  app.listen(PORT, () => {
    console.log(`▶ sbo3l-chatops-discord listening on :${PORT} → daemon=${DAEMON_URL}`);
  });
}

export default app;
