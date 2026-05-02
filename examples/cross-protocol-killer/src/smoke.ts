/**
 * Smoke runner — proves the wiring without running the full 10-step
 * loop. Builds one APRP, prints its shape, and exits.
 *
 * Useful in CI to verify the demo's package resolves + types compile
 * without spinning up a daemon or any of the framework integrations.
 */

import { buildAprpForStep, fmtTimestamp, printStepHeader, printStepResult } from "./steps.js";

printStepHeader(0, "smoke");

const aprp = buildAprpForStep({
  framework: "smoke",
  intent: "purchase_api_call",
  amount: "0.05",
  step: 0,
});

process.stdout.write(`  ✅ APRP built — ${Object.keys(aprp).length} fields\n`);
process.stdout.write(`     intent=${aprp.intent}\n`);
process.stdout.write(`     chain=${aprp.chain}\n`);
process.stdout.write(`     destination.expected_recipient=${
  (aprp.destination as { expected_recipient?: string }).expected_recipient ?? "(none)"
}\n`);

printStepResult({
  step: 0,
  framework: "smoke",
  ts: fmtTimestamp(),
  decision: "allow",
  audit_event_id: "evt-SMOKE000000000000000000",
  execution_ref: "kh-smoke-1",
  prev_audit_event_id: null,
  mock: true,
});

process.stdout.write(`\n✓ smoke ok — wiring sound, ready for full demo run\n`);
