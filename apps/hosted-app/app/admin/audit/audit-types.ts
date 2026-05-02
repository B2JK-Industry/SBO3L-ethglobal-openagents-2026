// Shared types for the /admin/audit page. Both <AuditTimeline> (textual
// feed) and <DecisionChart> (recharts viz) consume `/v1/admin/events`,
// emitted by crate::sbo3l_server::admin_events::AdminEvent.
//
// Codex review fix (PR #317): the previous union used dotted kinds
// (`decision.made`, `execution.confirmed`, …) which never match
// AdminEvent's serde-tagged kinds (`decision`, `operational`). Result:
// every incoming WS frame failed `isTimelineEvent` and the timeline /
// chart stayed empty even when the socket was live.
//
// Wire format anchor: crates/sbo3l-server/src/admin_events.rs defines
// `#[serde(tag = "kind")] enum AdminEvent { Decision {...}, Operational {...} }`.

export interface DecisionEvent {
  kind: "decision";
  id: number;
  tenant_id: string;
  agent_id: string;
  decision: "allow" | "deny";
  deny_code: string | null;
  severity: "info" | "warn" | "error";
  audit_event_hash: string;
  chain_seq: number;
  ts_ms: number;
}

export interface OperationalEvent {
  kind: "operational";
  id: number;
  tenant_id: string;
  op_kind: string;
  message: string;
  severity: "info" | "warn" | "error";
  ts_ms: number;
}

export type TimelineEvent = DecisionEvent | OperationalEvent;

export type ConnState = "connecting" | "live" | "offline";

export const KIND_LABEL: Record<TimelineEvent["kind"], string> = {
  decision: "decision",
  operational: "operational",
};

export function isTimelineEvent(value: unknown): value is TimelineEvent {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  if (typeof v.kind !== "string") return false;
  if (typeof v.ts_ms !== "number") return false;
  if (typeof v.tenant_id !== "string") return false;
  if (typeof v.id !== "number") return false;
  if (v.kind === "decision") {
    return typeof v.agent_id === "string"
      && (v.decision === "allow" || v.decision === "deny");
  }
  if (v.kind === "operational") {
    return typeof v.op_kind === "string" && typeof v.message === "string";
  }
  return false;
}
