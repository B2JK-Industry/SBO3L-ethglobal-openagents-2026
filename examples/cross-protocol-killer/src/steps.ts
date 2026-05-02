/**
 * Per-step helpers for the cross-protocol killer demo. Kept separate
 * from agent.ts so the verify-output tool can reuse the StepResult
 * shape without dragging in the full agent loop.
 */

import type { PaymentRequest } from "@sbo3l/sdk";

export type StepDecision = "allow" | "deny" | "requires_human" | "error";

export interface VerifyCheck {
  name: string;
  ok: boolean;
}

export interface StepResult {
  step: number;
  framework: string;
  ts: string;
  decision: StepDecision;
  audit_event_id: string;
  execution_ref: string | null;
  /** prev_audit_event_id from the prior step's audit_event_id — null for step 1. */
  prev_audit_event_id: string | null;
  /** True when running against a mock daemon (no live submit). */
  mock: boolean;
  deny_code?: string | null;
  error?: string;
  /** Set on step 9 (capsule-builder). */
  capsule?: Record<string, unknown>;
  /** Set on step 10 (verifier). */
  verify_checks?: VerifyCheck[];
}

const RECIPIENT_BASE = "0x1111111111111111111111111111111111111111";

function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function fmtTimestamp(): string {
  return new Date().toISOString();
}

interface BuildAprpInput {
  framework: string;
  intent: "purchase_api_call" | "purchase_dataset" | "pay_compute_job" | "pay_agent_service" | "tip";
  amount: string;
  step: number;
}

export function buildAprpForStep(input: BuildAprpInput): PaymentRequest {
  return {
    agent_id: "research-agent-01",
    task_id: `cross-protocol-step-${input.step}-${input.framework}`,
    intent: input.intent,
    amount: { value: input.amount, currency: "USD" },
    token: "USDC",
    destination: {
      type: "x402_endpoint",
      url: `https://api.example.com/v1/${input.framework}`,
      method: "POST",
      expected_recipient: RECIPIENT_BASE,
    },
    payment_protocol: "x402",
    chain: "base",
    provider_url: "https://api.example.com",
    expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
    nonce: freshNonce(),
    risk_class: input.amount === "0.00" ? "low" : "low",
  } as PaymentRequest;
}

export function printStepHeader(step: number, framework: string): void {
  process.stdout.write(`\n▶ step ${step}: ${framework}\n`);
}

export function printStepResult(r: StepResult): void {
  const decisionGlyph = r.decision === "allow" ? "✅" : r.decision === "error" ? "✗" : "⊗";
  process.stdout.write(`  ${decisionGlyph} ${r.decision.padEnd(15)} `);
  process.stdout.write(`audit_event_id=${r.audit_event_id}\n`);
  if (r.execution_ref !== null && r.execution_ref !== undefined) {
    process.stdout.write(`     execution_ref=${r.execution_ref}\n`);
  }
  if (r.prev_audit_event_id !== null) {
    process.stdout.write(`     prev_event_hash → ${r.prev_audit_event_id}\n`);
  }
  if (r.deny_code !== null && r.deny_code !== undefined) {
    process.stdout.write(`     deny_code=${r.deny_code}\n`);
  }
  if (r.error !== undefined) {
    process.stdout.write(`     error=${r.error}\n`);
  }
  if (r.verify_checks !== undefined) {
    for (const c of r.verify_checks) {
      process.stdout.write(`     ${c.ok ? "✅" : "✗"}  ${c.name}\n`);
    }
  }
  if (r.capsule !== undefined) {
    process.stdout.write(`     capsule_type=${(r.capsule as Record<string, unknown>)["capsule_type"] ?? "?"}\n`);
    process.stdout.write(`     chain_length=${(r.capsule as Record<string, unknown>)["chain_length"] ?? "?"}\n`);
  }
}
