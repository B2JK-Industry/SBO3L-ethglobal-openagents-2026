#!/usr/bin/env node
// SBO3L Verify GitHub Action — verifies a Passport capsule, writes a
// markdown report into $GITHUB_STEP_SUMMARY, posts a PR comment, and
// surfaces outputs via $GITHUB_OUTPUT.
//
// Self-contained: no npm install at runtime. The verify logic mirrors
// the SDK's verify() shape (without dragging in the full SDK as an
// install dep) — this lets the action stay <100 lines + zero install
// time per use.

import { readFileSync, appendFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const WORKSPACE = process.env.GITHUB_WORKSPACE ?? process.cwd();
const CAPSULE_REL = process.env.SBO3L_CAPSULE_PATH;
const FAIL_ON_DENY = (process.env.SBO3L_FAIL_ON_DENY ?? "true") === "true";
const COMMENT_ON_PR = process.env.SBO3L_COMMENT_ON_PR ?? "auto";
const TOKEN = process.env.GITHUB_TOKEN;
const EVENT_NAME = process.env.GITHUB_EVENT_NAME;
const REPO = process.env.GITHUB_REPOSITORY;
const PR_NUMBER = process.env.GITHUB_PR_NUMBER;

const STEP_SUMMARY = process.env.GITHUB_STEP_SUMMARY;
const STEP_OUTPUT = process.env.GITHUB_OUTPUT;

if (!CAPSULE_REL) {
  console.error("SBO3L_CAPSULE_PATH is required");
  process.exit(2);
}

const capsulePath = resolve(WORKSPACE, CAPSULE_REL);
if (!existsSync(capsulePath)) {
  console.error(`capsule file not found: ${capsulePath}`);
  process.exit(1);
}

let capsule;
try {
  capsule = JSON.parse(readFileSync(capsulePath, "utf-8"));
} catch (e) {
  console.error(`capsule is not valid JSON: ${e.message}`);
  process.exit(1);
}

// --- Inline verifier --------------------------------------------------------
// Mirrors sdks/typescript/src/passport.ts::verify shape. We inline the
// checks here so the action has zero npm install cost — the full SDK
// is fetched only when consumers want extended Ed25519 + ENS lookups.
function verifyInline(c) {
  const checks = [];
  const isObj = (v) => v !== null && typeof v === "object" && !Array.isArray(v);

  checks.push({
    name: "capsule.is_object",
    ok: isObj(c),
  });
  if (!isObj(c)) return { decision: "deny", checks };

  const ctype = c.capsule_type ?? c.receipt_type ?? "";
  checks.push({
    name: "capsule.type_recognised",
    ok: typeof ctype === "string" && ctype.startsWith("sbo3l."),
    detail: ctype,
  });

  const decision = c.decision ?? c.receipt?.decision ?? "unknown";
  checks.push({
    name: "capsule.decision_set",
    ok: ["allow", "deny", "requires_human"].includes(decision),
    detail: decision,
  });

  const auditId = c.audit_event_id ?? c.receipt?.audit_event_id ?? null;
  checks.push({
    name: "capsule.audit_event_id_present",
    ok: typeof auditId === "string" && /^evt-/.test(auditId),
    detail: auditId,
  });

  const requestHash = c.request_hash ?? c.receipt?.request_hash ?? null;
  checks.push({
    name: "capsule.request_hash_present",
    ok: typeof requestHash === "string" && requestHash.length === 64,
  });

  const policyHash = c.policy_hash ?? c.receipt?.policy_hash ?? null;
  checks.push({
    name: "capsule.policy_hash_present",
    ok: typeof policyHash === "string" && policyHash.length === 64,
  });

  return { decision, audit_event_id: auditId, checks };
}

const result = verifyInline(capsule);
const passed = result.checks.filter((c) => c.ok).length;
const total = result.checks.length;

// --- Build markdown report ---------------------------------------------------
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
const report = lines.join("\n") + "\n";

// --- Write step summary ------------------------------------------------------
if (STEP_SUMMARY) {
  appendFileSync(STEP_SUMMARY, report);
}

// --- Write outputs -----------------------------------------------------------
if (STEP_OUTPUT) {
  appendFileSync(
    STEP_OUTPUT,
    [
      `decision=${result.decision}`,
      `audit-event-id=${result.audit_event_id ?? ""}`,
      `checks-passed=${passed}/${total}`,
    ].join("\n") + "\n",
  );
}

// --- Post PR comment (best-effort) -------------------------------------------
const shouldComment =
  COMMENT_ON_PR === "true" ||
  (COMMENT_ON_PR === "auto" && EVENT_NAME === "pull_request" && PR_NUMBER);

if (shouldComment && TOKEN && REPO && PR_NUMBER) {
  try {
    const r = await fetch(
      `https://api.github.com/repos/${REPO}/issues/${PR_NUMBER}/comments`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${TOKEN}`,
          Accept: "application/vnd.github+json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ body: report }),
      },
    );
    if (!r.ok) {
      console.warn(`PR comment failed: HTTP ${r.status} (continuing)`);
    } else {
      console.log("✓ PR comment posted");
    }
  } catch (e) {
    console.warn(`PR comment failed: ${e.message} (continuing)`);
  }
} else if (shouldComment) {
  console.warn(
    "PR comment requested but missing GITHUB_TOKEN / repo / pr-number — skipping",
  );
}

// --- Exit code ---------------------------------------------------------------
const checksPassed = passed === total;
const allowed = result.decision === "allow";

console.log(`decision=${result.decision} checks=${passed}/${total}`);

if (!checksPassed) {
  console.error(`❌ ${total - passed} verifier check(s) failed`);
  process.exit(1);
}
if (FAIL_ON_DENY && !allowed) {
  console.error(`❌ capsule decision is '${result.decision}' (fail-on-deny=true)`);
  process.exit(1);
}
console.log("✓ verify ok");
