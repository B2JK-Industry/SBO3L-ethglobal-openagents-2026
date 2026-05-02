import type { TimelineEvent } from "./audit-types";

// CSV columns chosen to make the file useful in Excel / Google Sheets
// without the reader needing to understand SBO3L semantics: timestamp
// in ISO + UNIX-ms, kind, agent, decision, deny code, free-form
// description in the last column. JSONL stays as the lossless path
// (no field flattening).

const CSV_HEADERS = [
  "timestamp_iso",
  "ts_ms",
  "kind",
  "agent_id",
  "decision",
  "deny_code",
  "description",
] as const;

function describe(e: TimelineEvent): string {
  switch (e.kind) {
    case "agent.discovered":    return `${e.agent_id} (${e.ens_name})`;
    case "attestation.signed":  return `${e.from} → ${e.to} ${e.attestation_id}`;
    case "decision.made":       return e.intent ?? "";
    case "audit.checkpoint":    return `chain_length=${e.chain_length} root=${e.root_hash.slice(0, 14)}…`;
    case "execution.confirmed": return `→ ${e.sponsor} ref=${e.execution_ref}`;
    case "flag.changed":        return `${e.flag_name} → ${e.enabled ? "on" : "off"} (by ${e.changed_by})`;
  }
}

function escapeCsvField(value: string): string {
  // RFC 4180: quote when the field contains comma, quote, or newline,
  // and double any embedded quotes.
  if (/[",\r\n]/.test(value)) return `"${value.replace(/"/g, '""')}"`;
  return value;
}

export function eventsToCsv(events: TimelineEvent[]): string {
  const rows = events.map((e) => {
    const agentId = "agent_id" in e ? e.agent_id : e.kind === "attestation.signed" ? `${e.from}→${e.to}` : "";
    const decision = e.kind === "decision.made" ? e.decision : "";
    const denyCode = e.kind === "decision.made" ? e.deny_code ?? "" : "";
    return [
      new Date(e.ts_ms).toISOString(),
      String(e.ts_ms),
      e.kind,
      agentId,
      decision,
      denyCode,
      describe(e),
    ];
  });
  return [
    CSV_HEADERS.join(","),
    ...rows.map((r) => r.map(escapeCsvField).join(",")),
  ].join("\r\n") + "\r\n";
}

export function eventsToJsonl(events: TimelineEvent[]): string {
  return events.map((e) => JSON.stringify(e)).join("\n") + "\n";
}

export function downloadBlob(content: string, mime: string, filename: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.append(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
