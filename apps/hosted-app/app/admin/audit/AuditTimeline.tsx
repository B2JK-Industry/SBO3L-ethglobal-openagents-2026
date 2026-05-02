"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { isTimelineEvent, type ConnState, type TimelineEvent } from "./audit-types";
import { AuditFilters, EMPTY_FILTERS, eventMatchesFilters, type FilterState } from "./AuditFilters";
import { downloadBlob, eventsToCsv, eventsToJsonl } from "./exports";

// Mirrors apps/trust-dns-viz/src/events.ts. Server emits the same
// shape on /v1/admin/events. Type union lives in ./audit-types so
// DecisionChart can share it without circular imports.
export type { TimelineEvent } from "./audit-types";

interface Props {
  wsUrl: string;
}

const MAX_EVENTS = 500;

export function AuditTimeline({ wsUrl }: Props): JSX.Element {
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [state, setState] = useState<ConnState>("connecting");
  const [filters, setFilters] = useState<FilterState>(EMPTY_FILTERS);
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
        } catch { /* ignore malformed payload */ }
      });
      ws.addEventListener("close", () => {
        setState("offline");
        reconnectTimer.current = window.setTimeout(connect, 3000);
      });
      ws.addEventListener("error", () => setState("offline"));
    };
    connect();
    return () => {
      if (reconnectTimer.current !== null) window.clearTimeout(reconnectTimer.current);
      wsRef.current?.close();
    };
  }, [wsUrl]);

  const filtered = useMemo(
    () => events.filter((e) => eventMatchesFilters(e, filters)),
    [events, filters],
  );

  const exportFilename = (ext: string): string =>
    `sbo3l-audit-${new Date().toISOString().replace(/[:.]/g, "-")}.${ext}`;

  const onExportJsonl = (): void => downloadBlob(eventsToJsonl(filtered), "application/x-jsonlines", exportFilename("jsonl"));
  const onExportCsv   = (): void => downloadBlob(eventsToCsv(filtered),   "text/csv",                 exportFilename("csv"));

  return (
    <div>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.6em", flexWrap: "wrap", gap: "0.6em" }}>
        <span style={{ color: "var(--muted)", fontSize: "0.85em", fontFamily: "var(--font-mono)" }}>
          {state === "live" && <span style={{ color: "var(--accent)" }}>● live · {events.length}/{MAX_EVENTS} buffered</span>}
          {state === "connecting" && <span>● connecting…</span>}
          {state === "offline" && <span style={{ color: "#ff6b6b" }}>● offline · auto-reconnecting</span>}
        </span>
        <div style={{ display: "flex", gap: "0.5em" }}>
          <button onClick={onExportCsv}   disabled={filtered.length === 0} className="ghost">Export CSV ({filtered.length})</button>
          <button onClick={onExportJsonl} disabled={filtered.length === 0} className="ghost">Export JSONL ({filtered.length})</button>
        </div>
      </header>

      <AuditFilters value={filters} onChange={setFilters} matchedCount={filtered.length} totalCount={events.length} />

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
