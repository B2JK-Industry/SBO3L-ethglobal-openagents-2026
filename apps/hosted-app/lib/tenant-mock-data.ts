// Per-tenant mock fixtures — keyed by tenant slug. Extends the
// existing tenant-implicit lib/mock-data.ts (which now stays as the
// "default" tenant view for backward compat with /dashboard etc.).
//
// CTI-3-5 slice-c (Grace + Dev 1) replaces these with daemon
// round-trips scoped by JWT tenant claim. The shape mirrors what
// the daemon endpoints return so the swap is one-file.

import type { Agent, AuditEvent, Capsule } from "./mock-data";

export interface TenantData {
  agents: Agent[];
  audit: AuditEvent[];
  capsules: Capsule[];
}

// 3 tenants × distinct fixtures so cross-tenant isolation is
// visually obvious during demo (different agent counts, different
// audit chains).
export const TENANT_DATA: Record<string, TenantData> = {
  acme: {
    agents: [
      { agentId: "research-acme-01", ensName: "research-01.acme.sbo3lagent.eth", pubkeyPrefix: "ed25519:9aF3..", createdAt: "2026-04-15T10:23:00Z" },
      { agentId: "trader-acme-02",   ensName: "trader-02.acme.sbo3lagent.eth",   pubkeyPrefix: "ed25519:7bC8..", createdAt: "2026-04-22T14:01:00Z" },
      { agentId: "auditor-acme-03",  ensName: "auditor-03.acme.sbo3lagent.eth",  pubkeyPrefix: "ed25519:2dE1..", createdAt: "2026-04-29T09:15:00Z" },
    ],
    audit: [
      { eventId: "01HZ-ACME-A1", tsUnixMs: 1714564961000, eventType: "policy.decision", agentId: "research-acme-01", decision: "allow", requestHashPrefix: "0xe044f1.." },
      { eventId: "01HZ-ACME-A2", tsUnixMs: 1714564838000, eventType: "policy.decision", agentId: "trader-acme-02",   decision: "allow", requestHashPrefix: "0x9bc7de.." },
      { eventId: "01HZ-ACME-A3", tsUnixMs: 1714564613000, eventType: "audit.checkpoint", agentId: "research-acme-01", requestHashPrefix: "0x4f10ee.." },
    ],
    capsules: [
      { capsuleId: "cap-acme-A1", agentId: "research-acme-01", decision: "allow", emittedAt: "2026-05-02T08:42:41Z", sizeBytes: 11_240 },
      { capsuleId: "cap-acme-A2", agentId: "trader-acme-02",   decision: "allow", emittedAt: "2026-05-02T08:36:53Z", sizeBytes: 10_802 },
    ],
  },
  contoso: {
    agents: [
      { agentId: "research-contoso", ensName: "research.contoso.sbo3lagent.eth", pubkeyPrefix: "ed25519:5f8A..", createdAt: "2026-04-22T14:00:00Z" },
    ],
    audit: [
      { eventId: "01HZ-CONT-B1", tsUnixMs: 1714564401000, eventType: "policy.decision", agentId: "research-contoso", decision: "deny", denyCode: "policy.budget_exceeded", requestHashPrefix: "0x12ab34.." },
    ],
    capsules: [
      { capsuleId: "cap-contoso-B1", agentId: "research-contoso", decision: "deny", emittedAt: "2026-05-02T08:28:22Z", sizeBytes: 9_614 },
    ],
  },
  fabrikam: {
    agents: [
      { agentId: "fabrikam-treasury", ensName: "treasury.fabrikam.sbo3lagent.eth", pubkeyPrefix: "ed25519:3e7C..", createdAt: "2026-04-29T09:00:00Z" },
      { agentId: "fabrikam-research", ensName: "research.fabrikam.sbo3lagent.eth", pubkeyPrefix: "ed25519:4d8B..", createdAt: "2026-04-30T10:30:00Z" },
      { agentId: "fabrikam-auditor",  ensName: "auditor.fabrikam.sbo3lagent.eth",  pubkeyPrefix: "ed25519:6e9D..", createdAt: "2026-05-01T11:15:00Z" },
      { agentId: "fabrikam-router",   ensName: "router.fabrikam.sbo3lagent.eth",   pubkeyPrefix: "ed25519:8a1F..", createdAt: "2026-05-01T16:45:00Z" },
    ],
    audit: [
      { eventId: "01HZ-FAB-C1", tsUnixMs: 1714565100000, eventType: "policy.decision", agentId: "fabrikam-treasury", decision: "allow", requestHashPrefix: "0xab12cd.." },
      { eventId: "01HZ-FAB-C2", tsUnixMs: 1714565205000, eventType: "policy.decision", agentId: "fabrikam-research", decision: "allow", requestHashPrefix: "0xef34gh.." },
      { eventId: "01HZ-FAB-C3", tsUnixMs: 1714565310000, eventType: "policy.decision", agentId: "fabrikam-auditor",  decision: "deny", denyCode: "policy.deny_unknown_provider", requestHashPrefix: "0x56ij78.." },
      { eventId: "01HZ-FAB-C4", tsUnixMs: 1714565415000, eventType: "audit.checkpoint", agentId: "fabrikam-router",   requestHashPrefix: "0x90kl12.." },
    ],
    capsules: [
      { capsuleId: "cap-fab-C1", agentId: "fabrikam-treasury", decision: "allow", emittedAt: "2026-05-02T09:01:00Z", sizeBytes: 11_980 },
      { capsuleId: "cap-fab-C2", agentId: "fabrikam-research", decision: "allow", emittedAt: "2026-05-02T09:02:45Z", sizeBytes: 12_104 },
      { capsuleId: "cap-fab-C3", agentId: "fabrikam-auditor",  decision: "deny", emittedAt: "2026-05-02T09:04:30Z", sizeBytes: 9_810 },
    ],
  },
};

export function dataForTenant(slug: string): TenantData | undefined {
  return TENANT_DATA[slug];
}
