"use client";

import type { TimelineEvent } from "./audit-types";

export interface FilterState {
  agentSubstring: string;
  kind: TimelineEvent["kind"] | "";
  decision: "allow" | "deny" | "";
  fromTs: number | null;
  toTs: number | null;
}

export const EMPTY_FILTERS: FilterState = {
  agentSubstring: "",
  kind: "",
  decision: "",
  fromTs: null,
  toTs: null,
};

interface Props {
  value: FilterState;
  onChange: (next: FilterState) => void;
  matchedCount: number;
  totalCount: number;
}

const inputStyle = {
  background: "var(--code-bg)",
  color: "var(--fg)",
  border: "1px solid var(--border)",
  borderRadius: "var(--r-sm)",
  padding: "0.45em 0.7em",
  fontFamily: "var(--font-mono)",
  fontSize: "0.85em",
} as const;

function tsToInputValue(ts: number | null): string {
  if (ts === null) return "";
  return new Date(ts).toISOString().slice(0, 16);
}

function inputValueToTs(s: string): number | null {
  if (!s) return null;
  const t = Date.parse(s);
  return Number.isFinite(t) ? t : null;
}

export function AuditFilters({ value, onChange, matchedCount, totalCount }: Props): JSX.Element {
  const set = <K extends keyof FilterState>(key: K, v: FilterState[K]): void => onChange({ ...value, [key]: v });
  const reset = (): void => onChange(EMPTY_FILTERS);
  const isFiltered = value.agentSubstring || value.kind || value.decision || value.fromTs !== null || value.toTs !== null;

  return (
    <section style={{ background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: "var(--r-md)", padding: "0.8em 1em", marginBottom: "1em" }}>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.6em" }}>
        <span style={{ fontSize: "0.88em", color: "var(--muted)", fontFamily: "var(--font-mono)" }}>
          {isFiltered ? `${matchedCount.toLocaleString()} / ${totalCount.toLocaleString()} match` : `${totalCount.toLocaleString()} events`}
        </span>
        {isFiltered && (
          <button onClick={reset} className="ghost" style={{ fontSize: "0.78em" }}>Clear filters</button>
        )}
      </header>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))", gap: "0.5em" }}>
        <input
          aria-label="Agent ID substring"
          type="text"
          placeholder="Agent ID substring"
          value={value.agentSubstring}
          onChange={(e) => set("agentSubstring", e.target.value)}
          style={inputStyle}
        />
        <select
          aria-label="Decision"
          value={value.decision}
          onChange={(e) => set("decision", e.target.value as FilterState["decision"])}
          style={inputStyle}
        >
          <option value="">all decisions</option>
          <option value="allow">allow</option>
          <option value="deny">deny</option>
        </select>
        <select
          aria-label="Event kind"
          value={value.kind}
          onChange={(e) => set("kind", e.target.value as FilterState["kind"])}
          style={inputStyle}
        >
          <option value="">all kinds</option>
          <option value="decision.made">decision.made</option>
          <option value="execution.confirmed">execution.confirmed</option>
          <option value="audit.checkpoint">audit.checkpoint</option>
          <option value="flag.changed">flag.changed</option>
          <option value="agent.discovered">agent.discovered</option>
          <option value="attestation.signed">attestation.signed</option>
        </select>
        <input
          aria-label="From timestamp"
          type="datetime-local"
          value={tsToInputValue(value.fromTs)}
          onChange={(e) => set("fromTs", inputValueToTs(e.target.value))}
          style={inputStyle}
        />
        <input
          aria-label="To timestamp"
          type="datetime-local"
          value={tsToInputValue(value.toTs)}
          onChange={(e) => set("toTs", inputValueToTs(e.target.value))}
          style={inputStyle}
        />
      </div>
    </section>
  );
}

export function eventMatchesFilters(e: TimelineEvent, f: FilterState): boolean {
  if (f.kind && e.kind !== f.kind) return false;
  if (f.decision && (e.kind !== "decision.made" || e.decision !== f.decision)) return false;
  if (f.fromTs !== null && e.ts_ms < f.fromTs) return false;
  if (f.toTs !== null && e.ts_ms > f.toTs) return false;
  if (f.agentSubstring) {
    const needle = f.agentSubstring.toLowerCase();
    const haystacks: string[] = [];
    if ("agent_id" in e && e.agent_id) haystacks.push(e.agent_id.toLowerCase());
    if (e.kind === "attestation.signed") haystacks.push(e.from.toLowerCase(), e.to.toLowerCase());
    if (!haystacks.some((s) => s.includes(needle))) return false;
  }
  return true;
}
