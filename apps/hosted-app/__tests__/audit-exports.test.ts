import { describe, it, expect } from "@jest/globals";
import { eventsToCsv, eventsToJsonl } from "../app/admin/audit/exports";
import { eventMatchesFilters, EMPTY_FILTERS } from "../app/admin/audit/AuditFilters";
import type { TimelineEvent } from "../app/admin/audit/audit-types";

// Codex review fix (PR #317): test sample updated to match the actual
// admin_events wire format (`decision` / `operational` kinds), not the
// dotted-kind union the previous test used. Field shape mirrors
// crates/sbo3l-server/src/admin_events.rs::AdminEvent.

const sample: TimelineEvent[] = [
  {
    kind: "decision",
    id: 1,
    tenant_id: "default",
    agent_id: "research-01",
    decision: "allow",
    deny_code: null,
    severity: "info",
    audit_event_hash: "0xa1b2c3d4e5f60718",
    chain_seq: 17,
    ts_ms: 1714564961000,
  },
  {
    kind: "decision",
    id: 2,
    tenant_id: "default",
    agent_id: "trader-02",
    decision: "deny",
    deny_code: "policy.budget_exceeded",
    severity: "warn",
    audit_event_hash: "0xb2c3d4e5f6071829",
    chain_seq: 18,
    ts_ms: 1714565000000,
  },
  {
    kind: "operational",
    id: 3,
    tenant_id: "default",
    op_kind: "signer.rotate",
    message: "rotated to key v3",
    severity: "info",
    ts_ms: 1714565100000,
  },
];

describe("eventsToCsv", () => {
  it("emits RFC4180-compliant header + rows", () => {
    const csv = eventsToCsv(sample);
    expect(csv.startsWith("timestamp_iso,ts_ms,kind,agent_id,decision,deny_code,description")).toBe(true);
    expect(csv).toContain("research-01");
    expect(csv).toContain("policy.budget_exceeded");
  });

  it("escapes commas + quotes in description fields", () => {
    const tricky: TimelineEvent[] = [{
      kind: "operational",
      id: 99,
      tenant_id: "default",
      op_kind: "test.export",
      message: 'weird,name"quoted',
      severity: "info",
      ts_ms: 1,
    }];
    const csv = eventsToCsv(tricky);
    // describe() builds "test.export: weird,name\"quoted" — has both
    // a comma and a doubled quote, so RFC4180 wraps + doubles.
    expect(csv).toContain('"test.export: weird,name""quoted"');
  });
});

describe("eventsToJsonl", () => {
  it("one JSON object per line, trailing newline", () => {
    const out = eventsToJsonl(sample);
    const lines = out.trimEnd().split("\n");
    expect(lines).toHaveLength(3);
    for (const l of lines) expect(() => JSON.parse(l)).not.toThrow();
  });
});

describe("eventMatchesFilters", () => {
  it("empty filter matches everything", () => {
    for (const e of sample) expect(eventMatchesFilters(e, EMPTY_FILTERS)).toBe(true);
  });

  it("decision filter narrows to allow", () => {
    const allowOnly = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, decision: "allow" }));
    expect(allowOnly).toHaveLength(1);
    expect(allowOnly[0]?.kind).toBe("decision");
  });

  it("kind filter narrows to operational", () => {
    const opOnly = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, kind: "operational" }));
    expect(opOnly).toHaveLength(1);
  });

  it("agent substring matches decision events only", () => {
    const matched = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, agentSubstring: "research" }));
    expect(matched).toHaveLength(1);
    expect(matched[0]?.kind).toBe("decision");
  });

  it("date range filters inclusive", () => {
    const matched = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, fromTs: 1714565000000, toTs: 1714565100000 }));
    expect(matched).toHaveLength(2);
  });
});
