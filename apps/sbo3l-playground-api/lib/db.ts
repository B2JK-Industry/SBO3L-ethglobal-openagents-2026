// Postgres client — Vercel Postgres via @vercel/postgres.
//
// SKELETON. Add the dependency + uncomment the imports once Daniel
// runs `vercel postgres create sbo3l-playground-db` (DEPLOY.md
// step 2). Until then this module exports placeholders so route
// handlers can typecheck against the eventual API surface.
//
// import { sql } from "@vercel/postgres";

export interface AuditEventRow {
  event_id: string;
  ts_ms: number;
  kind: string;
  agent_id: string | null;
  decision: "allow" | "deny" | "require_human" | null;
  deny_code: string | null;
  request_hash: string;
  policy_hash: string;
  capsule_id: string | null;
  anchor_tx: string | null;
}

export async function appendAuditEvent(_row: Omit<AuditEventRow, "event_id">): Promise<string> {
  // TODO: INSERT INTO audit_events (...) VALUES (...) RETURNING event_id
  // Schema lives in apps/sbo3l-playground-api/lib/migrations/V001_init.sql
  // (TODO: write that file once provisioned — Postgres + sqlx-style
  // migrator in next round).
  throw new Error("db.appendAuditEvent: skeleton — wire @vercel/postgres per DEPLOY.md");
}

export async function queryAuditChain(_limit: number): Promise<AuditEventRow[]> {
  // TODO: SELECT ... FROM audit_events ORDER BY ts_ms DESC LIMIT $1
  return [];
}

export async function checkNonceUnseen(_nonce: string): Promise<boolean> {
  // TODO: INSERT INTO seen_nonces (nonce) VALUES ($1)
  // ON CONFLICT DO NOTHING; check rowcount
  return true;
}
