import { describe, it, expect } from "@jest/globals";
import { eventsToCsv, eventsToJsonl } from "../app/admin/audit/exports";
import { eventMatchesFilters, EMPTY_FILTERS } from "../app/admin/audit/AuditFilters";
import type { TimelineEvent } from "../app/admin/audit/audit-types";

const sample: TimelineEvent[] = [
  { kind: "decision.made", agent_id: "research-01", decision: "allow", ts_ms: 1714564961000, intent: "erc20.transfer" },
  { kind: "decision.made", agent_id: "trader-02",   decision: "deny",  deny_code: "policy.budget_exceeded", ts_ms: 1714565000000 },
  { kind: "audit.checkpoint", agent_id: "research-01", chain_length: 42, root_hash: "0xabcdef1234567890abc", ts_ms: 1714565100000 },
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
      kind: "agent.discovered",
      agent_id: "test",
      ens_name: "weird,name\"quoted.eth",
      pubkey_b58: "ed25519:abc",
      ts_ms: 1,
    }];
    const csv = eventsToCsv(tricky);
    expect(csv).toContain('"test (weird,name""quoted.eth)"');
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

  it("decision filter narrows to allow/deny", () => {
    const allowOnly = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, decision: "allow" }));
    expect(allowOnly).toHaveLength(1);
    expect(allowOnly[0].kind).toBe("decision.made");
  });

  it("agent substring matches across kinds", () => {
    const matched = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, agentSubstring: "research" }));
    expect(matched).toHaveLength(2);
  });

  it("date range filters inclusive", () => {
    const matched = sample.filter((e) => eventMatchesFilters(e, { ...EMPTY_FILTERS, fromTs: 1714565000000, toTs: 1714565100000 }));
    expect(matched).toHaveLength(2);
  });
});
