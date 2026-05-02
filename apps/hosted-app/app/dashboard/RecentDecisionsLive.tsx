"use client";

import { useEffect, useState, type ReactElement } from "react";

interface DecisionRow {
  ts: number;
  agentId: string;
  intent: string;
  decision: "allow" | "deny";
  denyCode?: string;
}

const AGENTS = ["research-01", "trader-02", "auditor-03"];
const INTENTS = ["swap WETH→USDC", "pay vendor", "quote check", "audit checkpoint"];

// Mock SSE — emits a fake decision every ~3 seconds. Replaced in slice 2
// by `new EventSource("/api/feed")` consuming Dev 1's ws_events endpoint
// proxied through the daemon. Same row-shape contract.
export function RecentDecisionsLive(): ReactElement {
  const [rows, setRows] = useState<DecisionRow[]>([]);

  useEffect(() => {
    const tick = (): void => {
      const i = Math.floor(Math.random() * AGENTS.length);
      const agentId = AGENTS[i] ?? "research-01";
      const intent = INTENTS[Math.floor(Math.random() * INTENTS.length)] ?? "swap";
      const allow = Math.random() > 0.15;
      const row: DecisionRow = {
        ts: Date.now(),
        agentId,
        intent,
        decision: allow ? "allow" : "deny",
        denyCode: allow ? undefined : "policy.budget_exceeded",
      };
      setRows((prev) => [row, ...prev].slice(0, 20));
    };
    tick();
    const id = window.setInterval(tick, 3000);
    return () => window.clearInterval(id);
  }, []);

  return (
    <section style={{ marginTop: "2em" }}>
      <h2 style={{ marginBottom: "1em", fontSize: "1.1em" }}>Recent decisions (live)</h2>
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
      <p style={{ color: "var(--muted)", fontSize: "0.85em", marginTop: "0.8em" }}>
        Mock SSE feed — replaced by daemon EventSource in CTI-3-4 main slice 2.
      </p>
    </section>
  );
}
