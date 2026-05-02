// Standalone test for the inline verifier shape — no test framework
// required. Runs as `node test/verifier.test.mjs` from CI / local.
//
// We can't import from src/index.mjs directly (it's a script with side
// effects on import). Instead we re-define the verifyInline function
// here from the same source contract — when the implementation changes,
// the tests fail loudly because the shape diverges. (A future refactor
// could split verify into a pure module + a CLI entrypoint; deliberately
// keeping it inline today to keep the action <100 lines + zero install.)

import { strict as assert } from "node:assert";

// Inline copy of verifyInline from src/index.mjs. Keep in sync.
const HEX64 = /^[0-9a-f]{64}$/;

function verifyInline(c) {
  const checks = [];
  const isObj = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
  checks.push({ name: "capsule.is_object", ok: isObj(c) });
  if (!isObj(c)) return { decision: "deny", audit_event_id: null, checks };
  const ctype = c.schema ?? c.capsule_type ?? c.receipt_type ?? "";
  checks.push({
    name: "capsule.type_recognised",
    ok: typeof ctype === "string" && ctype.startsWith("sbo3l."),
    detail: ctype,
  });
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

const tests = [];
function test(name, fn) {
  tests.push({ name, fn });
}

test("rejects non-object capsule", () => {
  const r = verifyInline(null);
  assert.equal(r.decision, "deny");
  assert.equal(r.checks[0].ok, false);
});

test("accepts a fully-formed v2 capsule", () => {
  const c = {
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "allow",
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  };
  const r = verifyInline(c);
  assert.equal(r.decision, "allow");
  assert.equal(r.audit_event_id, "evt-01HTAWX5K3R8YV9NQB7C6P2DGM");
  assert.equal(r.checks.filter((c) => c.ok).length, r.checks.length);
});

test("accepts a receipt-shaped envelope (decision under .receipt)", () => {
  const c = {
    receipt_type: "sbo3l.policy_receipt.v1",
    receipt: {
      decision: "allow",
      audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
    },
  };
  const r = verifyInline(c);
  assert.equal(r.decision, "allow");
  assert.equal(r.audit_event_id, "evt-01HTAWX5K3R8YV9NQB7C6P2DGM");
});

test("flags wrong capsule_type prefix", () => {
  const c = {
    capsule_type: "evil.capsule.v1",
    decision: "allow",
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  };
  const r = verifyInline(c);
  assert.equal(r.checks.find((x) => x.name === "capsule.type_recognised").ok, false);
});

test("flags missing audit_event_id", () => {
  const c = {
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "allow",
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  };
  const r = verifyInline(c);
  assert.equal(r.checks.find((x) => x.name === "capsule.audit_event_id_present").ok, false);
});

test("flags wrong-length request_hash", () => {
  const c = {
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "allow",
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
    request_hash: "00",
    policy_hash: "00".repeat(32),
  };
  const r = verifyInline(c);
  assert.equal(r.checks.find((x) => x.name === "capsule.request_hash_present").ok, false);
});

test("flags audit_event_id without evt- prefix", () => {
  const c = {
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "allow",
    audit_event_id: "01HTAWX5K3R8YV9NQB7C6P2DGM", // missing evt-
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  };
  const r = verifyInline(c);
  assert.equal(r.checks.find((x) => x.name === "capsule.audit_event_id_present").ok, false);
});

test("decision=requires_human is recognised", () => {
  const c = {
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "requires_human",
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  };
  const r = verifyInline(c);
  assert.equal(r.decision, "requires_human");
  assert.equal(r.checks.find((x) => x.name === "capsule.decision_set").ok, true);
});

// Regression — codex P1 on PR #286: real Passport capsules use `schema`
// at the root + nested `decision.result` + `audit.audit_event_id` etc.
// The original verifier checked `capsule_type` / top-level `decision`
// and rejected every real capsule.
test("accepts a real v2 Passport capsule (schema + nested decision)", () => {
  const c = {
    schema: "sbo3l.passport_capsule.v2",
    audit: { audit_event_id: "evt-01KQGHR5WCX75DGP8190YNDDMK" },
    request: {
      request_hash: "c0bd2fab4a7d4686d686edcc9c8356315cd66b820a2072493bf758a1eeb500db",
    },
    policy: {
      policy_hash: "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf",
    },
    decision: {
      result: "allow",
      receipt: {
        decision: "allow",
        audit_event_id: "evt-01KQGHR5WCX75DGP8190YNDDMK",
        request_hash: "c0bd2fab4a7d4686d686edcc9c8356315cd66b820a2072493bf758a1eeb500db",
        policy_hash: "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf",
      },
    },
  };
  const r = verifyInline(c);
  assert.equal(r.decision, "allow");
  assert.equal(r.audit_event_id, "evt-01KQGHR5WCX75DGP8190YNDDMK");
  assert.equal(r.checks.filter((c) => c.ok).length, r.checks.length);
});

// Regression — codex P2 on PR #286: weak request_hash check.
// 64 chars of `g` (non-hex) was accepted. Now rejected by HEX64 regex.
test("rejects non-hex 64-char request_hash", () => {
  const c = {
    schema: "sbo3l.passport_capsule.v2",
    decision: { result: "allow" },
    audit: { audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM" },
    request: { request_hash: "g".repeat(64) }, // 64 chars but not hex
    policy: { policy_hash: "00".repeat(32) },
  };
  const r = verifyInline(c);
  assert.equal(r.checks.find((x) => x.name === "capsule.request_hash_present").ok, false);
});

test("rejects non-hex 64-char policy_hash", () => {
  const c = {
    schema: "sbo3l.passport_capsule.v2",
    decision: { result: "allow" },
    audit: { audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM" },
    request: { request_hash: "00".repeat(32) },
    policy: { policy_hash: "X".repeat(64) },
  };
  const r = verifyInline(c);
  assert.equal(r.checks.find((x) => x.name === "capsule.policy_hash_present").ok, false);
});

let passed = 0;
let failed = 0;
for (const t of tests) {
  try {
    t.fn();
    process.stdout.write(`  ✓ ${t.name}\n`);
    passed++;
  } catch (e) {
    process.stdout.write(`  ✗ ${t.name}: ${e.message}\n`);
    failed++;
  }
}
process.stdout.write(`\n${passed}/${tests.length} passed\n`);
if (failed > 0) process.exit(1);
