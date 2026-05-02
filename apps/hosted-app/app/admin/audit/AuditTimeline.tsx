"use client";

import { useEffect, useMemo, useRef, useState } from "react";

// Mirrors apps/trust-dns-viz/src/events.ts. Server emits the same
// shape on /v1/events. Keep the union narrow + extend as Dev 1 ships
// new event kinds.
export type TimelineEvent =
  | { kind: "agent.discovered";    agent_id: string; ens_name: string; pubkey_b58: string; ts_ms: number }
  | { kind: "attestation.signed";  from: string; to: string; attestation_id: string; ts_ms: number }
  | { kind: "decision.made";       agent_id: string; decision: "allow" | "deny"; deny_code?: string; ts_ms: number; intent?: string }
  | { kind: "audit.checkpoint";    agent_id: string; chain_length: number; root_hash: string; ts_ms: number }
  | { kind: "execution.confirmed"; agent_id: string; execution_ref: string; sponsor: string; ts_ms: number }
  | { kind: "flag.changed";        flag_name: string; enabled: boolean; changed_by: string; ts_ms: number };

interface Props {
  wsUrl: string;
}

const MAX_EVENTS = 200;
const KIND_LABEL: Record<TimelineEvent["kind"], string> = {
  "agent.discovered":    "agent.discovered",
  "attestation.signed":  "attestation.signed",
  "decision.made":       "decision.made",
  "audit.checkpoint":    "audit.checkpoint",
  "execution.confirmed": "execution.confirmed",
  "flag.changed":        "flag.changed",
};

type ConnState = "connecting" | "live" | "offline";

export function AuditTimeline({ wsUrl }: Props): JSX.Element {
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [state, setState] = useState<ConnState>("connecting");
  const [agentFilter, setAgentFilter] = useState("");
  const [kindFilter, setKindFilter] = useState<TimelineEvent["kind"] | "">("");
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<number | null>(null);

  useEffect(() => {
    const connect = (): void => {
      setState("connecting");
      let ws: WebSocket;
      try {
        ws = new WebSocket(wsUrl);
      } catch {
        setState("offline");
        reconnectTimer.current = window.setTimeout(connect, 3000);
        return;
      }
      wsRef.current = ws;

      ws.addEventListener("open", () => setState("live"));
      ws.addEventListener("message", (msg) => {
        try {
          const ev = JSON.parse(msg.data as string) as TimelineEvent;
          if (!isTimelineEvent(ev)) return;
          setEvents((prev) => [ev, ...prev].slice(0, MAX_EVENTS));
        } catch {
          /* ignore malformed payload */
        }
      });
      ws.addEventListener("close", () => {
        setState("offline");
        reconnectTimer.current = window.setTimeout(connect, 3000);
      });
      ws.addEventListener("error", () => {
        setState("offline");
      });
    };

    connect();
    return () => {
      if (reconnectTimer.current !== null) window.clearTimeout(reconnectTimer.current);
      wsRef.current?.close();
    };
  }, [wsUrl]);

  const filtered = useMemo(
    () => events.filter((e) => matchesFilter(e, agentFilter, kindFilter)),
    [events, agentFilter, kindFilter],
  );

  const onExport = (): void => {
    const lines = events.map((e) => JSON.stringify(e)).join("\n");
    const blob = new Blob([lines + "\n"], { type: "application/x-jsonlines" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `sbo3l-audit-${new Date().toISOString().replace(/[:.]/g, "-")}.jsonl`;
    document.body.append(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  };

  return (
    <div>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1em", flexWrap: "wrap", gap: "0.6em" }}>
        <span style={{ color: "var(--muted)", fontSize: "0.85em", fontFamily: "var(--font-mono)" }}>
          {state === "live" && <span style={{ color: "var(--accent)" }}>● live · {events.length}/{MAX_EVENTS} events buffered</span>}
          {state === "connecting" && <span>● connecting…</span>}
          {state === "offline" && <span style={{ color: "#ff6b6b" }}>● offline · auto-reconnecting</span>}
        </span>
        <button onClick={onExport} disabled={events.length === 0} className="ghost">
          Export {events.length} events as JSONL
        </button>
      </header>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0.6em", marginBottom: "1em" }}>
        <input
          type="text"
          placeholder="Filter by agent_id (substring)"
          value={agentFilter}
          onChange={(ev) => setAgentFilter(ev.target.value)}
          aria-label="Filter by agent ID"
          style={{ background: "var(--code-bg)", color: "var(--fg)", border: "1px solid var(--border)", borderRadius: "var(--r-sm)", padding: "0.5em 0.7em", fontFamily: "var(--font-mono)" }}
        />
        <select
          value={kindFilter}
          onChange={(ev) => setKindFilter(ev.target.value as TimelineEvent["kind"] | "")}
          aria-label="Filter by event kind"
          style={{ background: "var(--code-bg)", color: "var(--fg)", border: "1px solid var(--border)", borderRadius: "var(--r-sm)", padding: "0.5em 0.7em", fontFamily: "var(--font-mono)" }}
        >
          <option value="">all event kinds</option>
          {Object.values(KIND_LABEL).map((k) => (
            <option key={k} value={k}>{k}</option>
          ))}
        </select>
      </div>

      <ol style={{ listStyle: "none", padding: 0, margin: 0, display: "grid", gap: "0.4em" }}>
        {filtered.length === 0 && (
          <li style={{ color: "var(--muted)", textAlign: "center", padding: "2em 0" }}>
            {state === "live" ? "Waiting for events that match your filter." : "No events yet — daemon not connected."}
          </li>
        )}
        {filtered.map((e, idx) => (
          <li
            key={`${e.ts_ms}-${idx}`}
            style={{
              display: "grid",
              gridTemplateColumns: "8em 9em 1fr",
              gap: "0.8em",
              padding: "0.55em 0.9em",
              border: "1px solid var(--border)",
              borderRadius: "var(--r-sm)",
              background: "var(--code-bg)",
              fontSize: "0.88em",
              fontFamily: "var(--font-mono)",
            }}
          >
            <code style={{ color: "var(--muted)" }}>{new Date(e.ts_ms).toLocaleTimeString()}</code>
            <code style={{ color: kindColor(e.kind) }}>{e.kind}</code>
            <span>{describe(e)}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}

function isTimelineEvent(value: unknown): value is TimelineEvent {
  if (typeof value !== "object" || value === null) return false;
  const v = value as { kind?: unknown };
  return typeof v.kind === "string" && v.kind in KIND_LABEL;
}

function matchesFilter(e: TimelineEvent, agentFilter: string, kindFilter: TimelineEvent["kind"] | ""): boolean {
  if (kindFilter && e.kind !== kindFilter) return false;
  if (!agentFilter) return true;
  const f = agentFilter.toLowerCase();
  if ("agent_id" in e && e.agent_id.toLowerCase().includes(f)) return true;
  if (e.kind === "attestation.signed" && (e.from.toLowerCase().includes(f) || e.to.toLowerCase().includes(f))) return true;
  return false;
}

function describe(e: TimelineEvent): string {
  switch (e.kind) {
    case "agent.discovered":    return `${e.agent_id} (${e.ens_name})`;
    case "attestation.signed":  return `${e.from} → ${e.to}  ${e.attestation_id}`;
    case "decision.made":       return `${e.agent_id}  ${e.decision}${e.deny_code ? ` (${e.deny_code})` : ""}${e.intent ? ` · ${e.intent}` : ""}`;
    case "audit.checkpoint":    return `${e.agent_id}  chain_length=${e.chain_length}  root=${e.root_hash.slice(0, 14)}…`;
    case "execution.confirmed": return `${e.agent_id} → ${e.sponsor}  ref=${e.execution_ref}`;
    case "flag.changed":        return `${e.flag_name} → ${e.enabled ? "on" : "off"} (by ${e.changed_by})`;
  }
}

function kindColor(kind: TimelineEvent["kind"]): string {
  switch (kind) {
    case "decision.made":       return "var(--accent)";
    case "execution.confirmed": return "var(--accent)";
    case "flag.changed":        return "#ffce5c";
    case "audit.checkpoint":    return "var(--muted)";
    default:                    return "var(--fg)";
  }
}
