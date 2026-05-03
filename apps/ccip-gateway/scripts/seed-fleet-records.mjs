#!/usr/bin/env node
/*
 * seed-fleet-records.mjs — deterministic generator for the 60-subname
 * mainnet fleet records.json.
 *
 * Output (stdout): a JSON object that's a drop-in replacement for
 * apps/ccip-gateway/data/records.json. Preserves the existing
 * `_comment` and the 2 demo entries (`research-agent.sbo3lagent.eth`,
 * `trader-agent.sbo3lagent.eth`) so the gateway's existing demo
 * paths keep working alongside the 60 fleet entries.
 *
 * Each fleet entry carries the minimum `sbo3l:agent_id` text record
 * required for `verify-ens` to PASS. Optional records (endpoint,
 * pubkey_ed25519, etc.) are intentionally omitted — they get filled
 * in per-subname after Daniel sets up the per-agent runtime. This
 * keeps the initial fleet broadcast cheap.
 *
 * Idempotent. Running twice produces byte-identical output.
 *
 * Usage:
 *   node apps/ccip-gateway/scripts/seed-fleet-records.mjs > records.json.new
 *   diff apps/ccip-gateway/data/records.json records.json.new
 *   mv records.json.new apps/ccip-gateway/data/records.json
 *
 * Configurable via env:
 *   APEX        — parent name (default: sbo3lagent.eth)
 *   NUMBERED    — count of agent-NNN.<apex> entries (default: 50)
 *   SPECIALISTS — comma-separated specialist labels (default: 10 baked-in)
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const RECORDS_PATH = resolve(__dirname, "..", "data", "records.json");

const APEX = process.env.APEX ?? "sbo3lagent.eth";
const NUMBERED = parseInt(process.env.NUMBERED ?? "50", 10);
const SPECIALISTS = (process.env.SPECIALISTS ?? [
  "research",
  "trader",
  "auditor",
  "compliance",
  "treasury",
  "analytics",
  "reputation",
  "oracle",
  "messenger",
  "executor",
].join(",")).split(",");

if (!Number.isFinite(NUMBERED) || NUMBERED < 1 || NUMBERED > 999) {
  console.error("NUMBERED must be a positive integer < 1000");
  process.exit(1);
}

// Preserve the existing records.json header/demo entries.
const existing = JSON.parse(readFileSync(RECORDS_PATH, "utf8"));

const out = {};

// 1. Carry over `_comment` if present.
if (typeof existing._comment === "string") {
  out._comment = existing._comment;
}

// 2. Carry over the existing demo entries verbatim.
for (const [k, v] of Object.entries(existing)) {
  if (k.startsWith("_")) continue;
  out[k] = v;
}

// 3. Generate 50 numbered fleet entries.
for (let i = 1; i <= NUMBERED; i++) {
  const idx = String(i).padStart(3, "0");
  const fqdn = `agent-${idx}.${APEX}`;
  const label = `agent-${idx}`;
  // Don't overwrite an existing entry — let demo records win.
  if (out[fqdn]) continue;
  out[fqdn] = {
    "sbo3l:agent_id": label,
  };
}

// 4. Generate specialist entries.
for (const role of SPECIALISTS) {
  const trimmed = role.trim();
  if (!trimmed) continue;
  const fqdn = `${trimmed}.${APEX}`;
  if (out[fqdn]) continue;
  out[fqdn] = {
    "sbo3l:agent_id": trimmed,
  };
}

process.stdout.write(JSON.stringify(out, null, 2) + "\n");
