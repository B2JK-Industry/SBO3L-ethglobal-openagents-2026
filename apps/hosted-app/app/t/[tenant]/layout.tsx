import { notFound } from "next/navigation";
import Link from "next/link";
import { auth, signOut } from "@/auth";
import { tenantBySlug, userHasAccessTo, membershipsForUser, MOCK_TENANTS } from "@/lib/tenants";
import { TenantSwitcher } from "@/components/TenantSwitcher";

interface Props {
  children: React.ReactNode;
  params: Promise<{ tenant: string }>;
}

// Server-side tenant guard — runs on every nested page render.
// Validates slug shape, looks up the tenant, and confirms the user
// has a membership entry for it. notFound() on any failure (404 vs
// 403 chosen deliberately: "we don't reveal whether the tenant
// exists at all to users without access" — minor info-disclosure
// hardening for the multi-tenant boundary).
export default async function TenantLayout({ children, params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  const tenant = tenantBySlug(slug);
  if (!tenant) notFound();

  const session = await auth();
  const userId = session?.user?.githubLogin ?? session?.user?.email ?? null;
  const membership = userHasAccessTo(userId, slug);
  if (!membership) notFound();

  const memberships = membershipsForUser(userId);

  return (
    <>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "0.7em 1.5em", borderBottom: "1px solid var(--border)" }}>
        <nav style={{ display: "flex", gap: "1.2em", alignItems: "center", flexWrap: "wrap", fontSize: "0.92em" }}>
          <Link href={`/t/${slug}/dashboard`} style={{ fontWeight: 700 }}>SBO3L</Link>
          <Link href={`/t/${slug}/dashboard`}>Dashboard</Link>
          <Link href={`/t/${slug}/agents`}>Agents</Link>
          <Link href={`/t/${slug}/audit`}>Audit</Link>
          <Link href={`/t/${slug}/capsules`}>Capsules</Link>
          {(membership.role === "admin" || membership.role === "operator") && (
            <Link href={`/t/${slug}/admin/audit`}>Admin</Link>
          )}
        </nav>
        <div style={{ display: "flex", gap: "0.6em", alignItems: "center" }}>
          <TenantSwitcher
            current={slug}
            memberships={memberships}
            tenants={MOCK_TENANTS}
            basePath="/dashboard"
          />
          <form action={async () => { "use server"; await signOut({ redirectTo: "/" }); }}>
            <button type="submit" className="ghost" style={{ fontSize: "0.85em" }}>Sign out</button>
          </form>
        </div>
      </header>
      {children}
    </>
  );
}
