/**
 * Walks a saved demo transcript and verifies the audit chain offline.
 *
 *   npm run demo > /tmp/run.log
 *   npm run verify-output -- --file /tmp/run.log
 *
 * Six checks (mirrors the in-demo verifier so judges can reproduce the
 * proof from a saved transcript without running the demo again):
 *
 *   1. transcript parses as JSON array of StepResult
 *   2. step numbers are 1..10 in order
 *   3. each step's prev_audit_event_id matches the prior step's
 *      audit_event_id (chain link integrity)
 *   4. step 9 carries a capsule with capsule_type === sbo3l.passport_capsule.v2
 *   5. step 10 has verify_checks all ok
 *   6. final step's decision === allow
 */

import { readFile } from "node:fs/promises";

import type { StepResult } from "./steps.js";

interface CliFlags {
  file: string | undefined;
}

function parseFlags(argv: string[]): CliFlags {
  const flags: CliFlags = { file: undefined };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--file") {
      flags.file = argv[i + 1];
      i++;
    }
  }
  return flags;
}

async function loadTranscript(path: string): Promise<StepResult[]> {
  const raw = await readFile(path, "utf-8");
  // The demo emits the transcript as the line `__TRANSCRIPT_JSON__=...`.
  // Pull that out so verify-output works on the raw stdout dump too.
  const match = raw.match(/__TRANSCRIPT_JSON__=(.+)$/m);
  if (match !== null && match[1] !== undefined) {
    return JSON.parse(match[1]) as StepResult[];
  }
  // Otherwise assume the file IS the JSON.
  return JSON.parse(raw) as StepResult[];
}

interface VerifyCheck {
  name: string;
  ok: boolean;
  detail?: string;
}

function runChecks(transcript: StepResult[]): VerifyCheck[] {
  const checks: VerifyCheck[] = [];

  checks.push({
    name: "transcript.is_array",
    ok: Array.isArray(transcript),
  });

  if (!Array.isArray(transcript)) return checks;

  checks.push({
    name: "transcript.length=10",
    ok: transcript.length === 10,
    detail: `actual=${transcript.length}`,
  });

  let stepsOrdered = true;
  for (let i = 0; i < transcript.length; i++) {
    if (transcript[i]?.step !== i + 1) {
      stepsOrdered = false;
      break;
    }
  }
  checks.push({ name: "transcript.steps_in_order", ok: stepsOrdered });

  let chainOk = true;
  for (let i = 1; i < transcript.length; i++) {
    const cur = transcript[i];
    const prev = transcript[i - 1];
    if (cur === undefined || prev === undefined) {
      chainOk = false;
      break;
    }
    if (cur.prev_audit_event_id !== prev.audit_event_id) {
      chainOk = false;
      break;
    }
  }
  checks.push({ name: "audit.chain_links_consistent", ok: chainOk });

  const cap = transcript[8]?.capsule;
  checks.push({
    name: "capsule.schema_v2",
    ok:
      cap !== undefined &&
      (cap as Record<string, unknown>)["capsule_type"] === "sbo3l.passport_capsule.v2",
  });

  const verifierStep = transcript[9];
  const verifyChecks = verifierStep?.verify_checks ?? [];
  const allVerifyOk = verifyChecks.length > 0 && verifyChecks.every((c) => c.ok);
  checks.push({
    name: "verifier.all_checks_ok",
    ok: allVerifyOk,
    detail: `${verifyChecks.filter((c) => c.ok).length}/${verifyChecks.length}`,
  });

  checks.push({
    name: "final.decision=allow",
    ok: verifierStep?.decision === "allow",
  });

  return checks;
}

async function main(): Promise<void> {
  const flags = parseFlags(process.argv.slice(2));
  if (flags.file === undefined) {
    process.stderr.write("verify-output: --file <path> is required\n");
    process.exit(2);
  }

  let transcript: StepResult[];
  try {
    transcript = await loadTranscript(flags.file);
  } catch (e) {
    process.stderr.write(
      `verify-output: cannot read ${flags.file}: ${e instanceof Error ? e.message : String(e)}\n`,
    );
    process.exit(1);
  }

  const checks = runChecks(transcript);
  process.stdout.write(`\nverify-output checks (${checks.filter((c) => c.ok).length}/${checks.length}):\n`);
  for (const c of checks) {
    const glyph = c.ok ? "✅" : "✗";
    const detail = c.detail !== undefined ? ` (${c.detail})` : "";
    process.stdout.write(`  ${glyph}  ${c.name}${detail}\n`);
  }

  if (checks.some((c) => !c.ok)) {
    process.exit(1);
  }
  process.stdout.write(`\n✓ transcript verifies — full audit chain consistent.\n`);
}

main().catch((err: unknown) => {
  process.stderr.write(`verify-output failed: ${err instanceof Error ? err.message : String(err)}\n`);
  process.exit(2);
});
