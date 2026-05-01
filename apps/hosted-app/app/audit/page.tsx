import Link from "next/link";
import { DaemonStatusBanner } from "@/components/DaemonStatusBanner";
import { listAudit, DaemonError, type AuditEvent } from "@/lib/sbo3l-client";
import { mockAuditEvents } from "@/lib/mock-data";

interface NormalizedEvent {
  eventId: string;
  tsUnixMs: number;
  eventType: AuditEvent["event_type"];
  agentId: string;
  decision?: "allow" | "deny";
  denyCode?: string;
  requestHashPrefix: string;
}

function normalize(e: AuditEvent): NormalizedEvent {
  return {
    eventId: e.event_id,
    tsUnixMs: e.ts_unix_ms,
    eventType: e.event_type,
    agentId: e.agent_id,
    decision: e.decision,
    denyCode: e.deny_code,
    requestHashPrefix: e.request_hash.slice(0, 10) + "..",
  };
}

interface PageProps {
  searchParams: Promise<{ cursor?: string; limit?: string }>;
}

export default async function AuditPage({ searchParams }: PageProps) {
  const params = await searchParams;
  const limit = Number.parseInt(params.limit ?? "50", 10);
  let events: NormalizedEvent[];
  let nextCursor: string | null = null;
  let chainLength: number | null = null;

  try {
    const page = await listAudit({ cursor: params.cursor, limit });
    events = page.events.map(normalize);
    nextCursor = page.next_cursor;
    chainLength = page.chain_length;
  } catch (err) {
    if (!(err instanceof DaemonError)) throw err;
    events = mockAuditEvents.map((m) => ({
      eventId: m.eventId,
      tsUnixMs: m.tsUnixMs,
      eventType: m.eventType,
      agentId: m.agentId,
      decision: m.decision,
      denyCode: m.denyCode,
      requestHashPrefix: m.requestHashPrefix,
    }));
  }

  return (
    <main>
      <DaemonStatusBanner />
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5em" }}>
        <h1>Audit log</h1>
        {chainLength !== null && (
          <span style={{ color: "var(--muted)", fontSize: "0.9em" }}>
            chain length: <strong style={{ color: "var(--fg)" }}>{chainLength}</strong>
          </span>
        )}
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
        Hash-chained, Ed25519-signed event log. See{" "}
        <a href="https://sbo3l-docs.vercel.app/concepts/audit-log">/concepts/audit-log</a>.
      </p>
      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.92em" }}>
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
            <th style={{ padding: "0.5em 0.8em" }}>Time</th>
            <th style={{ padding: "0.5em 0.8em" }}>Type</th>
            <th style={{ padding: "0.5em 0.8em" }}>Agent</th>
            <th style={{ padding: "0.5em 0.8em" }}>Outcome</th>
            <th style={{ padding: "0.5em 0.8em" }}>Request hash</th>
          </tr>
        </thead>
        <tbody>
          {events.map((e) => (
            <tr key={e.eventId} style={{ borderBottom: "1px solid var(--border)" }}>
              <td style={{ padding: "0.5em 0.8em", color: "var(--muted)", fontFamily: "var(--font-mono)" }}>
                {new Date(e.tsUnixMs).toLocaleTimeString()}
              </td>
              <td style={{ padding: "0.5em 0.8em", color: "var(--muted)" }}>
                <code>{e.eventType}</code>
              </td>
              <td style={{ padding: "0.5em 0.8em" }}>
                <code>{e.agentId}</code>
              </td>
              <td style={{ padding: "0.5em 0.8em" }}>
                {e.decision === "allow" && <span style={{ color: "var(--accent)" }}>✓ allow</span>}
                {e.decision === "deny" && (
                  <span style={{ color: "#ff6b6b" }}>
                    ✗ deny <code style={{ marginLeft: "0.4em", fontSize: "0.85em" }}>{e.denyCode}</code>
                  </span>
                )}
                {!e.decision && <span style={{ color: "var(--muted)" }}>checkpoint</span>}
              </td>
              <td style={{ padding: "0.5em 0.8em", color: "var(--muted)", fontFamily: "var(--font-mono)" }}>
                {e.requestHashPrefix}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {nextCursor && (
        <nav style={{ marginTop: "1.5em" }}>
          <Link href={{ pathname: "/audit", query: { cursor: nextCursor, limit } }}>Next page →</Link>
        </nav>
      )}
    </main>
  );
}
