import { notFound } from "next/navigation";
import { tenantBySlug } from "@/lib/tenants";
import { dataForTenant } from "@/lib/tenant-mock-data";

interface Props { params: Promise<{ tenant: string }> }

export default async function TenantCapsules({ params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  if (!tenantBySlug(slug)) notFound();
  const capsules = dataForTenant(slug)?.capsules ?? [];

  return (
    <main>
      <h1>Capsules — {slug}</h1>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em" }}>
        {capsules.length} capsules emitted by agents in this tenant. Each is offline-verifiable; click <a href="https://sbo3l-marketing.vercel.app/proof">/proof</a> to verify in your browser.
      </p>
      <ul style={{ listStyle: "none", padding: 0, display: "grid", gap: "0.8em" }}>
        {capsules.map((c) => (
          <li key={c.capsuleId} style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "1em", alignItems: "center", padding: "1em", border: "1px solid var(--border)", borderRadius: "var(--r-md)", background: "var(--code-bg)" }}>
            <div>
              <code style={{ fontSize: "0.85em" }}>{c.capsuleId}</code>
              <div style={{ color: "var(--muted)", fontSize: "0.85em", marginTop: "0.3em" }}>
                <code>{c.agentId}</code> · {new Date(c.emittedAt).toLocaleString()}
              </div>
            </div>
            <span style={{ color: c.decision === "allow" ? "var(--accent)" : "#ff6b6b", fontWeight: 600 }}>{c.decision}</span>
            <span style={{ color: "var(--muted)", fontSize: "0.85em" }}>{(c.sizeBytes / 1024).toFixed(1)} KB</span>
          </li>
        ))}
      </ul>
    </main>
  );
}
