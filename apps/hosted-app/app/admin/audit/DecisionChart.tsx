"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import {
  PieChart, Pie, Cell, ResponsiveContainer, Tooltip,
  BarChart, Bar, XAxis, YAxis,
} from "recharts";
import { isTimelineEvent, type ConnState, type TimelineEvent } from "./audit-types";

interface Props {
  wsUrl: string;
}

const MAX_EVENTS = 500;
const ALLOW_FILL = "#4ade80";
const DENY_FILL = "#f87171";

export function DecisionChart({ wsUrl }: Props): JSX.Element {
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [state, setState] = useState<ConnState>("connecting");
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<number | null>(null);

  useEffect(() => {
    // Codex review fix (PR #288): the previous close handler scheduled
    // a reconnect unconditionally, including when the component
    // unmounted or wsUrl changed. That left a background reconnect
    // loop after unmount and re-opened sockets to stale URLs after
    // prop changes. `shouldReconnect` flag gates reconnects to
    // unexpected disconnects only.
    let shouldReconnect = true;
    const connect = (): void => {
      if (!shouldReconnect) return;
      setState("connecting");
      let ws: WebSocket;
      try {
        ws = new WebSocket(wsUrl);
      } catch {
        setState("offline");
        if (shouldReconnect) reconnectTimer.current = window.setTimeout(connect, 3000);
        return;
      }
      wsRef.current = ws;
      ws.addEventListener("open", () => setState("live"));
      ws.addEventListener("message", (msg) => {
        try {
          const ev = JSON.parse(msg.data as string) as unknown;
          if (!isTimelineEvent(ev)) return;
          setEvents((prev) => [ev, ...prev].slice(0, MAX_EVENTS));
        } catch { /* ignore malformed */ }
      });
      ws.addEventListener("close", () => {
        if (!shouldReconnect) return;
        setState("offline");
        reconnectTimer.current = window.setTimeout(connect, 3000);
      });
      ws.addEventListener("error", () => setState("offline"));
    };
    connect();
    return () => {
      shouldReconnect = false;
      if (reconnectTimer.current !== null) window.clearTimeout(reconnectTimer.current);
      wsRef.current?.close();
    };
  }, [wsUrl]);

  const decisionData = useMemo(() => {
    let allow = 0, deny = 0;
    for (const e of events) {
      if (e.kind !== "decision") continue;
      if (e.decision === "allow") allow += 1;
      else deny += 1;
    }
    return [
      { name: "allow", value: allow, fill: ALLOW_FILL },
      { name: "deny",  value: deny,  fill: DENY_FILL },
    ];
  }, [events]);

  const denyCodeData = useMemo(() => {
    const counts = new Map<string, number>();
    for (const e of events) {
      if (e.kind !== "decision" || e.decision !== "deny" || !e.deny_code) continue;
      counts.set(e.deny_code, (counts.get(e.deny_code) ?? 0) + 1);
    }
    return [...counts.entries()]
      .map(([code, count]) => ({ code, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 6);
  }, [events]);

  const totalDecisions = decisionData[0].value + decisionData[1].value;

  return (
    <section style={{ marginBottom: "1.5em", display: "grid", gridTemplateColumns: "1fr 2fr", gap: "1em", alignItems: "stretch" }}>
      <div style={{ background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: "var(--r-md)", padding: "0.8em 1em" }}>
        <h2 style={{ margin: "0 0 0.4em", fontSize: "0.95em" }}>
          Decisions
          <span style={{ marginLeft: "0.6em", color: "var(--muted)", fontSize: "0.8em", fontFamily: "var(--font-mono)" }}>
            {state === "live" ? `● ${totalDecisions} in window` : `● ${state}`}
          </span>
        </h2>
        {totalDecisions === 0 ? (
          <p style={{ color: "var(--muted)", fontSize: "0.85em", margin: 0, padding: "2em 0", textAlign: "center" }}>
            Waiting for decisions…
          </p>
        ) : (
          <ResponsiveContainer width="100%" height={180}>
            <PieChart>
              <Pie data={decisionData} dataKey="value" nameKey="name" innerRadius={40} outerRadius={70} paddingAngle={2}>
                {decisionData.map((d) => <Cell key={d.name} fill={d.fill} />)}
              </Pie>
              <Tooltip contentStyle={{ background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: 4 }} />
            </PieChart>
          </ResponsiveContainer>
        )}
      </div>
      <div style={{ background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: "var(--r-md)", padding: "0.8em 1em" }}>
        <h2 style={{ margin: "0 0 0.4em", fontSize: "0.95em" }}>Top deny reasons</h2>
        {denyCodeData.length === 0 ? (
          <p style={{ color: "var(--muted)", fontSize: "0.85em", margin: 0, padding: "2em 0", textAlign: "center" }}>
            No denies in window — policies are letting everything through.
          </p>
        ) : (
          <ResponsiveContainer width="100%" height={180}>
            <BarChart data={denyCodeData} layout="vertical" margin={{ left: 12, right: 12, top: 4, bottom: 4 }}>
              <XAxis type="number" hide />
              <YAxis type="category" dataKey="code" width={140} tick={{ fill: "var(--muted)", fontSize: 11, fontFamily: "var(--font-mono)" }} axisLine={false} tickLine={false} />
              <Tooltip contentStyle={{ background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: 4 }} />
              <Bar dataKey="count" fill={DENY_FILL} radius={[0, 4, 4, 0]} />
            </BarChart>
          </ResponsiveContainer>
        )}
      </div>
    </section>
  );
}
