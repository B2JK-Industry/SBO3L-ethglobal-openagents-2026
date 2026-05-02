#!/usr/bin/env node
// Shared inline verifier — used by GitLab + CircleCI + Jenkins plugins.
// Same shape as actions/sbo3l-verify/src/index.mjs, but platform-agnostic
// (writes JSON + markdown to stdout; CI plugin scripts route them).
//
// Inputs (all via env, so each CI's plumbing maps cleanly):
//   SBO3L_CAPSULE_PATH   — path to capsule JSON (required)
//   SBO3L_FAIL_ON_DENY   — "true" | "false" (default "true")
//   SBO3L_REPORT_FORMAT  — "json" | "markdown" (default "markdown")
//
// Exit codes:
//   0  — verify ok + decision allowed (or fail-on-deny=false)
//   1  — any verifier check failed OR decision is deny/requires_human + fail-on-deny=true
//   2  — usage / file-not-found

import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const CAPSULE_REL = process.env.SBO3L_CAPSULE_PATH;
const FAIL_ON_DENY = (process.env.SBO3L_FAIL_ON_DENY ?? "true") === "true";
const FORMAT = process.env.SBO3L_REPORT_FORMAT ?? "markdown";

if (!CAPSULE_REL) {
  process.stderr.write("SBO3L_CAPSULE_PATH is required\n");
  process.exit(2);
}

const capsulePath = resolve(process.cwd(), CAPSULE_REL);
if (!existsSync(capsulePath)) {
  process.stderr.write(`capsule file not found: ${capsulePath}\n`);
  process.exit(2);
}

let capsule;
try {
  capsule = JSON.parse(readFileSync(capsulePath, "utf-8"));
} catch (e) {
  process.stderr.write(`capsule is not valid JSON: ${e.message}\n`);
  process.exit(1);
}

// Hex regex shared by request_hash + policy_hash checks (64 hex chars).
const HEX64 = /^[0-9a-f]{64}$/;

function verifyInline(c) {
  const checks = [];
  const isObj = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
  checks.push({ name: "capsule.is_object", ok: isObj(c) });
  if (!isObj(c)) return { decision: "deny", audit_event_id: null, checks };

  // Real Passport capsules use `schema` at the root (per
  // sdks/typescript/src/passport.ts + test-corpus/passport/v2_*.json).
  // The legacy fallback to `capsule_type`/`receipt_type` lets a
  // bare-receipt envelope still verify — we accept either shape.
  const ctype = c.schema ?? c.capsule_type ?? c.receipt_type ?? "";
  checks.push({
    name: "capsule.type_recognised",
    ok: typeof ctype === "string" && ctype.startsWith("sbo3l."),
    detail: ctype,
  });

  // Real capsules store the verdict at `decision.result`; the inner
  // `decision.receipt.decision` is the same value mirrored. Bare-
  // receipt envelopes carry the verdict at top-level `decision`. Fall
  // through all three so we accept any canonical shape.
  const decision =
    c.decision?.result ??
    c.decision?.receipt?.decision ??
    (typeof c.decision === "string" ? c.decision : undefined) ??
    c.receipt?.decision ??
    "unknown";
  checks.push({
    name: "capsule.decision_set",
    ok: ["allow", "deny", "requires_human"].includes(decision),
    detail: decision,
  });

  // audit_event_id lives at top level on v2 capsules (`audit.audit_event_id`),
  // and on bare receipts at the receipt root.
  const auditId =
    c.audit?.audit_event_id ??
    c.decision?.receipt?.audit_event_id ??
    c.audit_event_id ??
    c.receipt?.audit_event_id ??
    null;
  checks.push({
    name: "capsule.audit_event_id_present",
    ok: typeof auditId === "string" && /^evt-/.test(auditId),
    detail: auditId,
  });

  const requestHash =
    c.request?.request_hash ??
    c.decision?.receipt?.request_hash ??
    c.request_hash ??
    c.receipt?.request_hash ??
    null;
  checks.push({
    name: "capsule.request_hash_present",
    ok: typeof requestHash === "string" && HEX64.test(requestHash),
  });

  const policyHash =
    c.policy?.policy_hash ??
    c.decision?.receipt?.policy_hash ??
    c.policy_hash ??
    c.receipt?.policy_hash ??
    null;
  checks.push({
    name: "capsule.policy_hash_present",
    ok: typeof policyHash === "string" && HEX64.test(policyHash),
  });

  return { decision, audit_event_id: auditId, checks };
}

const result = verifyInline(capsule);
const passed = result.checks.filter((c) => c.ok).length;
const total = result.checks.length;
const allChecksPassed = passed === total;
const allowed = result.decision === "allow";

if (FORMAT === "json") {
  process.stdout.write(
    JSON.stringify({
      capsule_path: CAPSULE_REL,
      decision: result.decision,
      audit_event_id: result.audit_event_id,
      checks_passed: `${passed}/${total}`,
      checks: result.checks,
    }) + "\n",
  );
} else {
  const lines = [];
  lines.push("## SBO3L Verify Capsule");
  lines.push("");
  lines.push(`**Capsule:** \`${CAPSULE_REL}\``);
  lines.push(`**Decision:** \`${result.decision}\``);
  if (result.audit_event_id) lines.push(`**Audit event id:** \`${result.audit_event_id}\``);
  lines.push(`**Checks:** ${passed} / ${total}`);
  lines.push("");
  lines.push("| Check | Result | Detail |");
  lines.push("|---|---|---|");
  for (const c of result.checks) {
    const glyph = c.ok ? "✅" : "❌";
    const detail = c.detail !== undefined ? `\`${c.detail}\`` : "—";
    lines.push(`| \`${c.name}\` | ${glyph} | ${detail} |`);
  }
  process.stdout.write(lines.join("\n") + "\n");
}

if (!allChecksPassed) {
  process.stderr.write(`❌ ${total - passed} verifier check(s) failed\n`);
  process.exit(1);
}
if (FAIL_ON_DENY && !allowed) {
  process.stderr.write(`❌ capsule decision is '${result.decision}' (fail-on-deny=true)\n`);
  process.exit(1);
}
process.stderr.write("✓ verify ok\n");
