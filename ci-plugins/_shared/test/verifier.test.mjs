// Standalone test for the shared verifier — runs as plain
// `node ci-plugins/_shared/test/verifier.test.mjs`. No test framework.
//
// We exercise the verifier as a child process (the way every plugin
// uses it), asserting on stdout + exit code + stderr.

import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { strict as assert } from "node:assert";

const SCRIPT = resolve(import.meta.dirname, "..", "verifier.mjs");

function runVerifier(capsule, opts = {}) {
  const dir = mkdtempSync(join(tmpdir(), "sbo3l-vrf-"));
  const path = join(dir, "capsule.json");
  if (capsule !== null) writeFileSync(path, capsule);

  const env = {
    ...process.env,
    SBO3L_CAPSULE_PATH: capsule === null ? "/no/such/file.json" : path,
    SBO3L_FAIL_ON_DENY: opts.failOnDeny ?? "true",
    SBO3L_REPORT_FORMAT: opts.format ?? "markdown",
  };

  const r = spawnSync("node", [SCRIPT], { env, encoding: "utf-8" });
  rmSync(dir, { recursive: true, force: true });
  return { code: r.status, stdout: r.stdout, stderr: r.stderr };
}

const valid = JSON.stringify({
  capsule_type: "sbo3l.passport_capsule.v2",
  decision: "allow",
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
});

const denyCapsule = JSON.stringify({
  capsule_type: "sbo3l.passport_capsule.v2",
  decision: "deny",
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
});

const tests = [];
function test(name, fn) {
  tests.push({ name, fn });
}

test("missing SBO3L_CAPSULE_PATH → exit 2", () => {
  const r = spawnSync("node", [SCRIPT], { env: process.env, encoding: "utf-8" });
  assert.equal(r.status, 2);
  assert.match(r.stderr, /SBO3L_CAPSULE_PATH is required/);
});

test("nonexistent capsule file → exit 2", () => {
  const r = runVerifier(null);
  assert.equal(r.code, 2);
  assert.match(r.stderr, /capsule file not found/);
});

test("malformed JSON capsule → exit 1", () => {
  const r = runVerifier("{not-json");
  assert.equal(r.code, 1);
  assert.match(r.stderr, /not valid JSON/);
});

test("valid v2 capsule → exit 0 + markdown report on stdout + ok on stderr", () => {
  const r = runVerifier(valid);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /SBO3L Verify Capsule/);
  assert.match(r.stdout, /✅/);
  assert.match(r.stderr, /verify ok/);
});

test("valid v2 capsule with --format=json → JSON on stdout", () => {
  const r = runVerifier(valid, { format: "json" });
  assert.equal(r.code, 0);
  const parsed = JSON.parse(r.stdout.trim());
  assert.equal(parsed.decision, "allow");
  assert.equal(parsed.checks_passed, "6/6");
  assert.equal(parsed.audit_event_id, "evt-01HTAWX5K3R8YV9NQB7C6P2DGM");
});

test("deny capsule + fail-on-deny=true → exit 1", () => {
  const r = runVerifier(denyCapsule, { failOnDeny: "true" });
  assert.equal(r.code, 1);
  assert.match(r.stderr, /capsule decision is 'deny'/);
});

test("deny capsule + fail-on-deny=false → exit 0", () => {
  const r = runVerifier(denyCapsule, { failOnDeny: "false" });
  assert.equal(r.code, 0);
  assert.match(r.stderr, /verify ok/);
});

test("v2 capsule missing audit_event_id → exit 1 (verifier check fail)", () => {
  const broken = JSON.stringify({
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "allow",
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  });
  const r = runVerifier(broken);
  assert.equal(r.code, 1);
  assert.match(r.stderr, /verifier check\(s\) failed/);
});

test("legacy receipt-shaped envelope (decision under .receipt) → exit 0", () => {
  const legacy = JSON.stringify({
    receipt_type: "sbo3l.policy_receipt.v1",
    receipt: {
      decision: "allow",
      audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
    },
  });
  const r = runVerifier(legacy);
  assert.equal(r.code, 0);
});

test("decision=requires_human + fail-on-deny=true → exit 1", () => {
  const rh = JSON.stringify({
    capsule_type: "sbo3l.passport_capsule.v2",
    decision: "requires_human",
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
  });
  const r = runVerifier(rh, { failOnDeny: "true" });
  assert.equal(r.code, 1);
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
