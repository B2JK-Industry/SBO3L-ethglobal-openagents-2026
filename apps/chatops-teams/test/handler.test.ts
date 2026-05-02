import { describe, expect, it, vi } from "vitest";

import {
  dispatchSlashCommand,
  handleAudit,
  handleDecide,
  handleVerify,
} from "../src/handler.js";

const VALID_CAPSULE = JSON.stringify({
  capsule_type: "sbo3l.passport_capsule.v2",
  decision: "allow",
  audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
});

const VALID_APRP = JSON.stringify({
  agent_id: "research-agent-01",
  task_id: "demo-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: { type: "x402_endpoint" },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: "2099-01-01T00:00:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  risk_class: "low",
});

describe("handleVerify", () => {
  it("usage hint when capsule text empty", () => {
    const r = handleVerify({ capsuleText: "" });
    expect(r.type).toBe("message");
    expect(r.textFormat).toBe("markdown");
    expect(r.text).toContain("Usage:");
  });

  it("rejects non-JSON capsule", () => {
    const r = handleVerify({ capsuleText: "{not-json" });
    expect(r.text).toContain("not valid JSON");
  });

  it("renders 6/6 ✅ on a valid v2 capsule", () => {
    const r = handleVerify({ capsuleText: VALID_CAPSULE });
    expect(r.text).toContain("6 / 6");
    expect(r.text).toContain("✅");
    expect(r.text).toContain("evt-01HTAWX5K3R8YV9NQB7C6P2DGM");
  });

  it("flags missing audit_event_id", () => {
    const broken = JSON.stringify({
      capsule_type: "sbo3l.passport_capsule.v2",
      decision: "allow",
      request_hash: "00".repeat(32),
      policy_hash: "00".repeat(32),
    });
    const r = handleVerify({ capsuleText: broken });
    expect(r.text).toContain("audit_event_id_present");
    expect(r.text).toContain("❌");
  });
});

describe("handleAudit", () => {
  const fakePrefix = vi.fn().mockResolvedValue({
    chain_length: 42,
    head_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGM",
    recent: [
      { event_id: "evt-001", type: "payment_request", ts: "2026-05-02T10:00:00Z" },
      { event_id: "evt-002", type: "policy_decision", ts: "2026-05-02T10:00:01Z" },
    ],
  });

  it("usage hint when agent id empty", async () => {
    const r = await handleAudit({ agentId: "", fetchAuditPrefix: fakePrefix });
    expect(r.text).toContain("Usage:");
  });

  it("renders chain length + recent events", async () => {
    const r = await handleAudit({
      agentId: "research-agent-01",
      fetchAuditPrefix: fakePrefix,
    });
    expect(r.text).toContain("research-agent-01");
    expect(r.text).toContain("Chain length: **42**");
    expect(r.text).toContain("evt-001");
    expect(r.text).toContain("policy_decision");
  });

  it("surfaces daemon error", async () => {
    const failing = vi.fn().mockRejectedValue(new Error("502 bad gateway"));
    const r = await handleAudit({ agentId: "x", fetchAuditPrefix: failing });
    expect(r.text).toContain("daemon error");
    expect(r.text).toContain("502 bad gateway");
  });
});

describe("handleDecide", () => {
  const fakeSubmit = vi.fn().mockResolvedValue({
    decision: "allow",
    deny_code: null,
    matched_rule_id: "allow-x402",
    audit_event_id: "evt-allow-1",
    receipt: { execution_ref: "kh-1" },
  });

  it("usage hint when APRP text empty", async () => {
    const r = await handleDecide({ aprpText: "", submit: fakeSubmit });
    expect(r.text).toContain("Usage:");
  });

  it("rejects non-JSON APRP", async () => {
    const r = await handleDecide({ aprpText: "{not-json", submit: fakeSubmit });
    expect(r.text).toContain("not valid JSON");
  });

  it("renders allow with audit_event_id + execution_ref", async () => {
    const r = await handleDecide({ aprpText: VALID_APRP, submit: fakeSubmit });
    expect(r.text).toContain("✅");
    expect(r.text).toContain("evt-allow-1");
    expect(r.text).toContain("kh-1");
    expect(r.text).toContain("allow-x402");
  });

  it("renders deny with deny_code", async () => {
    const denySubmit = vi.fn().mockResolvedValue({
      decision: "deny",
      deny_code: "policy.budget_exceeded",
      matched_rule_id: null,
      audit_event_id: "evt-deny-1",
      receipt: { execution_ref: null },
    });
    const r = await handleDecide({ aprpText: VALID_APRP, submit: denySubmit });
    expect(r.text).toContain("⊗");
    expect(r.text).toContain("policy.budget_exceeded");
    expect(r.text).toContain("evt-deny-1");
  });

  it("surfaces daemon error", async () => {
    const failing = vi.fn().mockRejectedValue(new Error("connection refused"));
    const r = await handleDecide({ aprpText: VALID_APRP, submit: failing });
    expect(r.text).toContain("daemon error");
  });
});

describe("dispatchSlashCommand", () => {
  const fakePrefix = vi.fn().mockResolvedValue({
    chain_length: 0,
    head_event_id: null,
    recent: [],
  });
  const fakeSubmit = vi.fn().mockResolvedValue({
    decision: "allow",
    deny_code: null,
    matched_rule_id: "allow-x402",
    audit_event_id: "evt-allow-1",
    receipt: { execution_ref: "kh-1" },
  });

  it("empty text → help", async () => {
    const r = await dispatchSlashCommand({ text: "", fetchAuditPrefix: fakePrefix, submit: fakeSubmit });
    expect(r.text).toContain("/sbo3l verify");
    expect(r.text).toContain("/sbo3l audit");
    expect(r.text).toContain("/sbo3l decide");
  });

  it("explicit help → help", async () => {
    const r = await dispatchSlashCommand({ text: "help", fetchAuditPrefix: fakePrefix, submit: fakeSubmit });
    expect(r.text).toContain("/sbo3l verify");
  });

  it("strips leading @SBO3L bot mention", async () => {
    const r = await dispatchSlashCommand({
      text: `@SBO3L verify ${VALID_CAPSULE}`,
      fetchAuditPrefix: fakePrefix,
      submit: fakeSubmit,
    });
    expect(r.text).toContain("6 / 6");
  });

  it("unknown subcommand → error message", async () => {
    const r = await dispatchSlashCommand({ text: "bogus arg", fetchAuditPrefix: fakePrefix, submit: fakeSubmit });
    expect(r.text).toContain("Unknown subcommand");
    expect(r.text).toContain("bogus");
  });

  it("verify <capsule> dispatches to handleVerify", async () => {
    const r = await dispatchSlashCommand({
      text: `verify ${VALID_CAPSULE}`,
      fetchAuditPrefix: fakePrefix,
      submit: fakeSubmit,
    });
    expect(r.text).toContain("6 / 6");
  });

  it("audit <agent> dispatches to handleAudit", async () => {
    const r = await dispatchSlashCommand({
      text: "audit research-agent-01",
      fetchAuditPrefix: fakePrefix,
      submit: fakeSubmit,
    });
    expect(r.text).toContain("research-agent-01");
  });

  it("decide <APRP> dispatches to handleDecide", async () => {
    const r = await dispatchSlashCommand({
      text: `decide ${VALID_APRP}`,
      fetchAuditPrefix: fakePrefix,
      submit: fakeSubmit,
    });
    expect(r.text).toContain("evt-allow-1");
  });
});
