import { NextResponse } from "next/server";

// Public audit chain explorer. Returns the latest 100 events (with
// agent_id anonymized — the playground's audit chain is the demo
// surface, not real customer data).

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const DEFAULT_LIMIT = 100;
const MAX_LIMIT = 500;

export async function GET(req: Request): Promise<NextResponse> {
  const url = new URL(req.url);
  const limitRaw = url.searchParams.get("limit");
  const limit = Math.min(Math.max(parseInt(limitRaw ?? `${DEFAULT_LIMIT}`, 10) || DEFAULT_LIMIT, 1), MAX_LIMIT);

  // TODO: import { queryAuditChain } from "@/lib/db" → SELECT
  // event_id, ts_ms, kind, decision, deny_code, anchor_tx
  // FROM audit_events
  // ORDER BY ts_ms DESC LIMIT $1
  // Plus join against the latest anchor row for the on-chain link.

  return NextResponse.json({
    schema: "sbo3l.playground_api.placeholder.v1",
    status: "skeleton",
    todo: "wire Postgres query + anchor join per DEPLOY.md",
    requested_limit: limit,
    events: [],
    latest_anchor: null,
    github: "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/tree/main/apps/sbo3l-playground-api",
  });
}
