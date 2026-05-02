import Link from "next/link";
import { notFound } from "next/navigation";
import { auth } from "@/auth";
import { tenantBySlug, userHasAccessTo } from "@/lib/tenants";
import { policyForTenant } from "@/lib/tenant-policies";
import { PolicyEditor } from "./PolicyEditor";

interface Props { params: Promise<{ tenant: string }> }

export const dynamic = "force-dynamic";

export default async function TenantPolicyEditPage({ params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  const tenant = tenantBySlug(slug);
  if (!tenant) notFound();

  const session = await auth();
  const userId = session?.user?.githubLogin ?? session?.user?.email ?? null;
  const membership = userHasAccessTo(userId, slug);
  if (!membership) notFound();
  // Editing the policy is an admin-only operation; viewers + operators
  // see a read-only message linking back to dashboard. notFound() is
  // already enforced by the tenant guard for non-members.
  if (membership.role !== "admin") {
    return (
      <main>
        <h1>Policy editor — {tenant.display_name}</h1>
        <p style={{ color: "var(--muted)", margin: "1em 0", maxWidth: 760 }}>
          Your role is <code>{membership.role}</code>. Only <code>admin</code>
          can modify the tenant policy. Ask an admin in your tenant to invite
          you with the higher role.
        </p>
        <p>
          <Link href={`/t/${slug}/dashboard`}>← Back to dashboard</Link>
        </p>
      </main>
    );
  }

  const policy = policyForTenant(slug);
  if (!policy) notFound();

  return (
    <main>
      <header style={{ marginBottom: "1.2em" }}>
        <h1 style={{ marginBottom: "0.2em" }}>Policy editor — {tenant.display_name}</h1>
        <p style={{ color: "var(--muted)", margin: 0, maxWidth: 760 }}>
          The YAML below overrides <code>sbo3l.root@1</code> for tenant
          <code> {slug}</code>. Saved drafts are signed by your admin key
          and pushed to the daemon's signed-policy store. Live policy serves
          from the daemon — drafts here don't take effect until promoted.
        </p>
      </header>
      <PolicyEditor
        initialYaml={policy.yaml}
        version={policy.version}
        signedBy={policy.signed_by}
        updatedAt={policy.updated_at}
        tenantSlug={slug}
      />
    </main>
  );
}
