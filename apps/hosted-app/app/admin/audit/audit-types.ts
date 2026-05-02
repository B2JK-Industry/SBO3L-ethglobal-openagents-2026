// Shared types + WS plumbing for the audit page.
// Both <AuditTimeline> (textual feed) and <DecisionChart> (recharts viz)
// consume the same /v1/events shape. Keeping the union in one place
// avoids drift when Dev 1 ships new event kinds.

export type TimelineEvent =
  | { kind: "agent.discovered";    agent_id: string; ens_name: string; pubkey_b58: string; ts_ms: number }
  | { kind: "attestation.signed";  from: string; to: string; attestation_id: string; ts_ms: number }
  | { kind: "decision.made";       agent_id: string; decision: "allow" | "deny"; deny_code?: string; ts_ms: number; intent?: string }
  | { kind: "audit.checkpoint";    agent_id: string; chain_length: number; root_hash: string; ts_ms: number }
  | { kind: "execution.confirmed"; agent_id: string; execution_ref: string; sponsor: string; ts_ms: number }
  | { kind: "flag.changed";        flag_name: string; enabled: boolean; changed_by: string; ts_ms: number };

export type ConnState = "connecting" | "live" | "offline";

export const KIND_LABEL: Record<TimelineEvent["kind"], string> = {
  "agent.discovered":    "agent.discovered",
  "attestation.signed":  "attestation.signed",
  "decision.made":       "decision.made",
  "audit.checkpoint":    "audit.checkpoint",
  "execution.confirmed": "execution.confirmed",
  "flag.changed":        "flag.changed",
};

export function isTimelineEvent(value: unknown): value is TimelineEvent {
  if (typeof value !== "object" || value === null) return false;
  const v = value as { kind?: unknown };
  return typeof v.kind === "string" && v.kind in KIND_LABEL;
}
