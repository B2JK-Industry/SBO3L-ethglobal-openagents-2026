import { notFound } from "next/navigation";
import { tenantBySlug } from "@/lib/tenants";
import { dataForTenant } from "@/lib/tenant-mock-data";

interface Props { params: Promise<{ tenant: string }> }

export default async function TenantAgents({ params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  if (!tenantBySlug(slug)) notFound();
  const data = dataForTenant(slug);
  const agents = data?.agents ?? [];

  return (
    <main>
      <h1>Agents — {slug}</h1>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em" }}>{agents.length} agents registered to this tenant.</p>
      {agents.length === 0 && <p>No agents yet.</p>}
      {agents.length > 0 && (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
              <th style={{ padding: "0.5em 0.8em" }}>Agent ID</th>
              <th style={{ padding: "0.5em 0.8em" }}>ENS</th>
              <th style={{ padding: "0.5em 0.8em" }}>Pubkey</th>
              <th style={{ padding: "0.5em 0.8em" }}>Created</th>
            </tr>
          </thead>
          <tbody>
            {agents.map((a) => (
              <tr key={a.agentId} style={{ borderBottom: "1px solid var(--border)" }}>
                <td style={{ padding: "0.6em 0.8em" }}><code>{a.agentId}</code></td>
                <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}>{a.ensName}</td>
                <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}><code>{a.pubkeyPrefix}</code></td>
                <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}>{new Date(a.createdAt).toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </main>
  );
}
