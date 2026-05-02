import Link from "next/link";
import { auth } from "@/auth";
import { redirect } from "next/navigation";
import { membershipsForUser, MOCK_TENANTS } from "@/lib/tenants";

export const dynamic = "force-dynamic";

// Post-login tenant picker. If the user has exactly one tenant
// membership, redirect straight into it. Otherwise show a card
// per tenant.
export default async function TenantsPage(): Promise<JSX.Element> {
  const session = await auth();
  if (!session) redirect("/login");
  const userId = session.user?.githubLogin ?? session.user?.email ?? null;
  const memberships = membershipsForUser(userId);

  if (memberships.length === 1) {
    redirect(`/t/${memberships[0]!.tenant_slug}/dashboard`);
  }

  return (
    <main>
      <section style={{ maxWidth: 720, margin: "4em auto", padding: "0 1.5em" }}>
        <h1 style={{ marginBottom: "0.5em" }}>Pick a tenant</h1>
        <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
          You have access to {memberships.length} tenants. Pick one to continue; switch later via the dropdown in the top-right of any page.
        </p>
        <ul style={{ listStyle: "none", padding: 0, display: "grid", gap: "0.8em" }}>
          {memberships.map((m) => {
            const tenant = MOCK_TENANTS.find((t) => t.slug === m.tenant_slug);
            if (!tenant) return null;
            return (
              <li key={m.tenant_slug}>
                <Link
                  href={`/t/${m.tenant_slug}/dashboard`}
                  style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: "0.5em 1em", padding: "1.2em 1.4em", border: "1px solid var(--border)", borderRadius: "var(--r-md)", background: "var(--code-bg)", color: "var(--fg)", textDecoration: "none" }}
                >
                  <div>
                    <strong>{tenant.display_name}</strong>{" "}
                    <code style={{ color: "var(--muted)", marginLeft: "0.5em", fontSize: "0.85em" }}>{tenant.slug}</code>
                  </div>
                  <span style={{ color: "var(--accent)", fontFamily: "var(--font-mono)", fontSize: "0.82em" }}>{m.role}</span>
                  <p style={{ color: "var(--muted)", fontSize: "0.88em", margin: 0, gridColumn: "1 / -1" }}>
                    Tier <code>{tenant.tier}</code> · ENS <code>{tenant.ens_apex}</code>
                  </p>
                </Link>
              </li>
            );
          })}
        </ul>
      </section>
    </main>
  );
}
