import { mockAuditEvents } from "@/lib/mock-data";

export default function AuditPage() {
  const events = mockAuditEvents; // CTI-3-4 main slice 2: paginated, filterable, daemon-backed

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5em" }}>
        <h1>Audit log</h1>
        <button title="Run strict-hash verifier against the local chain">Verify (strict)</button>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
        Hash-chained, Ed25519-signed event log. Strict-hash verifier runs all 6 capsule checks (see{" "}
        <a href="https://sbo3l-docs.vercel.app/concepts/audit-log">/concepts/audit-log</a>).
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
      <p style={{ color: "var(--muted)", marginTop: "2em", fontSize: "0.9em" }}>
        Mock data. Real strict-hash verifier integration lands in CTI-3-4 main slice 2.
      </p>
    </main>
  );
}
