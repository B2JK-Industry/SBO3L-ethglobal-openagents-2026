import { tenantBySlug, userHasAccessTo } from "@/lib/tenants";
import { dataForTenant } from "@/lib/tenant-mock-data";
import { auth } from "@/auth";
import { notFound } from "next/navigation";

interface Props { params: Promise<{ tenant: string }> }

export default async function TenantDashboard({ params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  const tenant = tenantBySlug(slug);
  if (!tenant) notFound();

  const session = await auth();
  const userId = session?.user?.githubLogin ?? session?.user?.email ?? null;
  const membership = userHasAccessTo(userId, slug);
  if (!membership) notFound();

  const data = dataForTenant(slug);
  const decisionsTotal = data?.audit.filter((e) => e.eventType === "policy.decision").length ?? 0;
  const allowCount = data?.audit.filter((e) => e.decision === "allow").length ?? 0;
  const denyCount = data?.audit.filter((e) => e.decision === "deny").length ?? 0;

  return (
    <main>
      <header style={{ marginBottom: "2em" }}>
        <h1>Dashboard — {tenant.display_name}</h1>
        <p style={{ color: "var(--muted)", marginTop: "0.4em" }}>
          Tier <code>{tenant.tier}</code> · ENS apex <code>{tenant.ens_apex}</code> · your role <code>{membership.role}</code>
        </p>
      </header>

      <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "1em" }}>
        <Card title="Agents"            value={`${data?.agents.length ?? 0}`}  hint="registered to this tenant" />
        <Card title="Decisions"         value={`${decisionsTotal}`}            hint={`allow ${allowCount} · deny ${denyCount}`} />
        <Card title="Capsules emitted"  value={`${data?.capsules.length ?? 0}`} hint="downloadable + offline-verifiable" />
      </section>
    </main>
  );
}

function Card({ title, value, hint }: { title: string; value: string; hint: string }): JSX.Element {
  return (
    <article style={{ border: "1px solid var(--border)", borderRadius: "var(--r-lg)", padding: "1.2em", background: "var(--code-bg)" }}>
      <h2 style={{ fontSize: "0.9em", color: "var(--muted)", fontWeight: 500 }}>{title}</h2>
      <p style={{ fontSize: "1.6em", fontWeight: 700, color: "var(--accent)", margin: "0.3em 0" }}>{value}</p>
      <p style={{ fontSize: "0.85em", color: "var(--muted)" }}>{hint}</p>
    </article>
  );
}
