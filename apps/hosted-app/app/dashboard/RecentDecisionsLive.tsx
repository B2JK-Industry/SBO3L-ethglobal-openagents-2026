"use client";

import { useEffect, useRef, useState } from "react";

interface DecisionRow {
  ts: number;
  agentId: string;
  intent: string;
  decision: "allow" | "deny";
  denyCode?: string;
}

interface Props {
  wsUrl: string;
}

const AGENTS = ["research-01", "trader-02", "auditor-03"];
const INTENTS = ["swap WETH→USDC", "pay vendor", "quote check", "audit checkpoint"];

type ConnState = "connecting" | "live" | "demo";

// Real daemon WebSocket consumer with mock fallback. Connects to the
// /v1/events endpoint Dev 1 ships in #129. On connection error or close
// (no daemon running, network blip, server restart), falls back to a
// local mock generator so the demo never goes silent.
export function RecentDecisionsLive({ wsUrl }: Props): JSX.Element {
  const [rows, setRows] = useState<DecisionRow[]>([]);
  const [state, setState] = useState<ConnState>("connecting");
  const mockIntervalRef = useRef<number | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const startMock = (): void => {
      setState("demo");
      const tick = (): void => {
        const i = Math.floor(Math.random() * AGENTS.length);
        const allow = Math.random() > 0.15;
        const row: DecisionRow = {
          ts: Date.now(),
          agentId: AGENTS[i] ?? "research-01",
          intent: INTENTS[Math.floor(Math.random() * INTENTS.length)] ?? "swap",
          decision: allow ? "allow" : "deny",
          denyCode: allow ? undefined : "policy.budget_exceeded",
        };
        setRows((prev) => [row, ...prev].slice(0, 20));
      };
      tick();
      mockIntervalRef.current = window.setInterval(tick, 3000);
    };

    const stopMock = (): void => {
      if (mockIntervalRef.current !== null) {
        window.clearInterval(mockIntervalRef.current);
        mockIntervalRef.current = null;
      }
    };

    try {
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      ws.addEventListener("open", () => {
        stopMock();
        setState("live");
      });
      ws.addEventListener("message", (msg) => {
        try {
          const ev = JSON.parse(msg.data as string) as {
            kind: "decision.made";
            ts_ms: number;
            agent_id: string;
            intent?: string;
            decision: "allow" | "deny";
            deny_code?: string;
          };
          if (ev.kind !== "decision.made") return;
          setRows((prev) =>
            [
              {
                ts: ev.ts_ms,
                agentId: ev.agent_id,
                intent: ev.intent ?? "—",
                decision: ev.decision,
                denyCode: ev.deny_code,
              },
              ...prev,
            ].slice(0, 20),
          );
        } catch {
          /* ignore malformed payload */
        }
      });
      ws.addEventListener("error", () => {
        if (state !== "demo") startMock();
      });
      ws.addEventListener("close", () => {
        if (state !== "demo") startMock();
      });
    } catch {
      startMock();
    }

    return () => {
      stopMock();
      wsRef.current?.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wsUrl]);

  return (
    <section style={{ marginTop: "2em" }}>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1em" }}>
        <h2 style={{ fontSize: "1.1em" }}>Recent decisions (live)</h2>
        <span style={{ fontSize: "0.8em", color: "var(--muted)", fontFamily: "var(--font-mono)" }}>
          ●{" "}
          {state === "live" ? <span style={{ color: "var(--accent)" }}>live · {wsUrl}</span> : null}
          {state === "demo" ? <span style={{ color: "var(--muted)" }}>demo mode (daemon unreachable)</span> : null}
          {state === "connecting" ? <span>connecting…</span> : null}
        </span>
      </header>
      <div style={{ border: "1px solid var(--border)", borderRadius: "var(--r-md)", background: "var(--code-bg)", overflow: "hidden" }}>
        {rows.length === 0 && <p style={{ padding: "1em", color: "var(--muted)" }}>Waiting for first event…</p>}
        {rows.map((r, idx) => (
          <div
            key={`${r.ts}-${idx}`}
            style={{
              display: "grid",
              gridTemplateColumns: "auto 1fr 1fr auto",
              gap: "1em",
              padding: "0.6em 1em",
              borderTop: idx === 0 ? "none" : "1px solid var(--border)",
              fontSize: "0.9em",
            }}
          >
            <code style={{ color: "var(--muted)" }}>{new Date(r.ts).toLocaleTimeString()}</code>
            <code>{r.agentId}</code>
            <span style={{ color: "var(--muted)" }}>{r.intent}</span>
            <span style={{ color: r.decision === "allow" ? "var(--accent)" : "#ff6b6b", fontWeight: 600 }}>
              {r.decision === "allow" ? "✓ allow" : `✗ deny ${r.denyCode ?? ""}`}
            </span>
          </div>
        ))}
      </div>
    </section>
  );
}
