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
  if (e.kind === "decision") {
    return `chain_seq=${e.chain_seq} hash=${e.audit_event_hash.slice(0, 14)}…`;
  }
  return `${e.op_kind}: ${e.message}`;
}

function escapeCsvField(value: string): string {
  // RFC 4180: quote when the field contains comma, quote, or newline,
  // and double any embedded quotes.
  if (/[",\r\n]/.test(value)) return `"${value.replace(/"/g, '""')}"`;
  return value;
}

export function eventsToCsv(events: TimelineEvent[]): string {
  const rows = events.map((e) => {
    const agentId = e.kind === "decision" ? e.agent_id : "";
    const decision = e.kind === "decision" ? e.decision : "";
    const denyCode = e.kind === "decision" ? (e.deny_code ?? "") : "";
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
