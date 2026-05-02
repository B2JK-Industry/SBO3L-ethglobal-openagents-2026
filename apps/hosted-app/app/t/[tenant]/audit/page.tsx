import { notFound } from "next/navigation";
import { tenantBySlug } from "@/lib/tenants";
import { dataForTenant } from "@/lib/tenant-mock-data";

interface Props { params: Promise<{ tenant: string }> }

export default async function TenantAudit({ params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  if (!tenantBySlug(slug)) notFound();
  const events = dataForTenant(slug)?.audit ?? [];

  return (
    <main>
      <h1>Audit log — {slug}</h1>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em" }}>
        {events.length} events on this tenant's chain. Cross-tenant queries don't exist as a code path (V010 isolation; see <a href="https://sbo3l-docs.vercel.app/concepts/multi-tenant">/concepts/multi-tenant</a>).
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
              <td style={{ padding: "0.5em 0.8em", color: "var(--muted)", fontFamily: "var(--font-mono)" }}>{new Date(e.tsUnixMs).toLocaleTimeString()}</td>
              <td style={{ padding: "0.5em 0.8em", color: "var(--muted)" }}><code>{e.eventType}</code></td>
              <td style={{ padding: "0.5em 0.8em" }}><code>{e.agentId}</code></td>
              <td style={{ padding: "0.5em 0.8em" }}>
                {e.decision === "allow" && <span style={{ color: "var(--accent)" }}>✓ allow</span>}
                {e.decision === "deny" && <span style={{ color: "#ff6b6b" }}>✗ deny <code style={{ marginLeft: "0.4em", fontSize: "0.85em" }}>{e.denyCode}</code></span>}
                {!e.decision && <span style={{ color: "var(--muted)" }}>checkpoint</span>}
              </td>
              <td style={{ padding: "0.5em 0.8em", color: "var(--muted)", fontFamily: "var(--font-mono)" }}>{e.requestHashPrefix}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  );
}
