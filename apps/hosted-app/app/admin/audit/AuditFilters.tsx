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

// Codex review fix (PR #317): `<input type="datetime-local">` displays
// values as the user's local wall-clock time, but `toISOString()` always
// renders UTC. Slicing to YYYY-MM-DDTHH:MM gave the input a timestamp
// that looks like local time but is actually UTC, shifting the apparent
// hour by the user's offset. Format the local-clock fields by hand so
// the input shows the same time the user picked.
function pad(n: number): string { return n.toString().padStart(2, "0"); }

function tsToInputValue(ts: number | null): string {
  if (ts === null) return "";
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function inputValueToTs(s: string): number | null {
  if (!s) return null;
  // datetime-local strings are local time without offset; Date.parse on
  // a string of the form "YYYY-MM-DDTHH:MM" interprets as local time on
  // every modern engine, so this is the correct inverse of tsToInputValue.
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
          <option value="decision">decision</option>
          <option value="operational">operational</option>
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
  if (f.decision && (e.kind !== "decision" || e.decision !== f.decision)) return false;
  if (f.fromTs !== null && e.ts_ms < f.fromTs) return false;
  if (f.toTs !== null && e.ts_ms > f.toTs) return false;
  if (f.agentSubstring) {
    const needle = f.agentSubstring.toLowerCase();
    if (e.kind === "decision") {
      if (!e.agent_id.toLowerCase().includes(needle)) return false;
    } else {
      // Operational events have no agent_id; agent-filter excludes them.
      return false;
    }
  }
  return true;
}
