// Mock fixtures for the prep slice. Replaced by real daemon calls in
// CTI-3-4 main slice 2 (consumes Dev 1's WS/SSE endpoint + adds DB
// adapter once Grace's Postgres lands).

export interface Agent {
  agentId: string;
  ensName: string;
  pubkeyPrefix: string;
  createdAt: string;
}

export interface AuditEvent {
  eventId: string;
  tsUnixMs: number;
  eventType: "policy.decision" | "audit.checkpoint";
  agentId: string;
  decision?: "allow" | "deny";
  denyCode?: string;
  requestHashPrefix: string;
}

export interface Capsule {
  capsuleId: string;
  agentId: string;
  decision: "allow" | "deny";
  emittedAt: string;
  sizeBytes: number;
}

export const mockAgents: Agent[] = [
  { agentId: "research-01", ensName: "research-01.sbo3lagent.eth", pubkeyPrefix: "ed25519:9aF3..", createdAt: "2026-04-15T10:23:00Z" },
  { agentId: "trader-02",   ensName: "trader-02.sbo3lagent.eth",   pubkeyPrefix: "ed25519:7bC8..", createdAt: "2026-04-22T14:01:00Z" },
  { agentId: "auditor-03",  ensName: "auditor-03.sbo3lagent.eth",  pubkeyPrefix: "ed25519:2dE1..", createdAt: "2026-04-29T09:15:00Z" },
];

export const mockAuditEvents: AuditEvent[] = [
  { eventId: "01HZ...A1", tsUnixMs: 1714564961000, eventType: "policy.decision", agentId: "research-01", decision: "allow", requestHashPrefix: "0xe044f1.." },
  { eventId: "01HZ...A2", tsUnixMs: 1714564838000, eventType: "policy.decision", agentId: "research-01", decision: "deny",  denyCode: "policy.budget_exceeded", requestHashPrefix: "0x12ab34.." },
  { eventId: "01HZ...A3", tsUnixMs: 1714564613000, eventType: "policy.decision", agentId: "trader-02",   decision: "allow", requestHashPrefix: "0x9bc7de.." },
  { eventId: "01HZ...A4", tsUnixMs: 1714564401000, eventType: "audit.checkpoint", agentId: "research-01", requestHashPrefix: "0x4f10ee.." },
  { eventId: "01HZ...A5", tsUnixMs: 1714564102000, eventType: "policy.decision", agentId: "auditor-03",  decision: "deny",  denyCode: "policy.deny_unknown_provider", requestHashPrefix: "0xab12cd.." },
];

export const mockCapsules: Capsule[] = [
  { capsuleId: "cap-01HZ...A1", agentId: "research-01", decision: "allow", emittedAt: "2026-05-01T08:42:41Z", sizeBytes: 11_240 },
  { capsuleId: "cap-01HZ...A3", agentId: "trader-02",   decision: "allow", emittedAt: "2026-05-01T08:36:53Z", sizeBytes: 10_802 },
  { capsuleId: "cap-01HZ...A5", agentId: "auditor-03",  decision: "deny",  emittedAt: "2026-05-01T08:28:22Z", sizeBytes: 9_614  },
];
