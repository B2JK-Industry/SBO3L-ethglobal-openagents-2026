import Link from "next/link";
import { notFound } from "next/navigation";
import { auth } from "@/auth";
import { tenantBySlug, userHasAccessTo } from "@/lib/tenants";
import { billingForTenant, TIER_LIMITS, fmtNumber, type Tier } from "@/lib/tenant-billing";
import { UpgradeButton } from "./UpgradeButton";

interface Props { params: Promise<{ tenant: string }> }

export const dynamic = "force-dynamic";

// Codex review fix (PR #297): formatting an ISO date string with
// `new Date(...).toLocaleDateString()` interprets bare YYYY-MM-DD
// as UTC midnight and then displays in the runtime's local timezone.
// On a server in negative UTC offset (Pacific time, etc.) this
// shows the day BEFORE the stored date. Use a fixed UTC formatter
// so the displayed invoice date matches the stored value byte-for-byte.
function formatStableDate(iso: string): string {
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return new Intl.DateTimeFormat("en-CA", { timeZone: "UTC", year: "numeric", month: "2-digit", day: "2-digit" }).format(d);
  } catch {
    return iso;
  }
}

export default async function TenantBillingPage({ params }: Props): Promise<JSX.Element> {
  const { tenant: slug } = await params;
  const tenant = tenantBySlug(slug);
  if (!tenant) notFound();

  const session = await auth();
  const userId = session?.user?.githubLogin ?? session?.user?.email ?? null;
  const membership = userHasAccessTo(userId, slug);
  if (!membership) notFound();
  if (membership.role !== "admin") {
    return (
      <main>
        <h1>Billing — {tenant.display_name}</h1>
        <p style={{ color: "var(--muted)", margin: "1em 0" }}>
          Billing settings are <code>admin</code>-only. Your role is <code>{membership.role}</code>.
        </p>
        <p><Link href={`/t/${slug}/dashboard`}>← Back to dashboard</Link></p>
      </main>
    );
  }

  const billing = billingForTenant(slug);
  if (!billing) notFound();

  const current = TIER_LIMITS[billing.tier];
  const decisionsCap = typeof current.decisions_per_month === "number" ? current.decisions_per_month : Infinity;
  const decisionsPct = decisionsCap === Infinity ? 0 : Math.min(100, (billing.decisions_this_month / decisionsCap) * 100);

  return (
    <main>
      <header style={{ marginBottom: "1.5em" }}>
        <h1 style={{ marginBottom: "0.2em" }}>Billing — {tenant.display_name}</h1>
        <p style={{ color: "var(--muted)", margin: 0, maxWidth: 760 }}>
          Tier <code>{billing.tier}</code>{billing.next_invoice_at && <> · next invoice {formatStableDate(billing.next_invoice_at)}</>}
          {billing.payment_method && <> · {billing.payment_method}</>}.
        </p>
      </header>

      <section style={{ background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: "var(--r-md)", padding: "1em 1.2em", marginBottom: "1.5em" }}>
        <h2 style={{ margin: "0 0 0.4em", fontSize: "0.95em" }}>This month's usage</h2>
        <div style={{ display: "grid", gap: "0.4em", fontFamily: "var(--font-mono)", fontSize: "0.88em" }}>
          <div>Decisions: <strong>{billing.decisions_this_month.toLocaleString()}</strong> / {fmtNumber(current.decisions_per_month)}</div>
          {decisionsCap !== Infinity && (
            <div role="progressbar" aria-valuenow={Math.round(decisionsPct)} aria-valuemin={0} aria-valuemax={100} style={{ height: 6, background: "var(--border)", borderRadius: 3, overflow: "hidden" }}>
              <div style={{ width: `${decisionsPct}%`, height: "100%", background: decisionsPct > 80 ? "#f87171" : "var(--accent)" }} />
            </div>
          )}
          <div>Agents: <strong>{billing.agents_count}</strong> / {fmtNumber(current.agents)}</div>
        </div>
      </section>

      <h2 style={{ fontSize: "1em", margin: "0 0 0.6em" }}>Plans</h2>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "0.8em" }}>
        {(Object.keys(TIER_LIMITS) as Tier[]).map((tier) => {
          const limits = TIER_LIMITS[tier];
          const isCurrent = tier === billing.tier;
          return (
            <div key={tier} style={{ border: `1px solid ${isCurrent ? "var(--accent)" : "var(--border)"}`, borderRadius: "var(--r-md)", padding: "1em 1.1em", background: "var(--code-bg)" }}>
              <h3 style={{ margin: "0 0 0.2em", textTransform: "capitalize", fontSize: "1.05em" }}>{tier}</h3>
              <p style={{ margin: "0 0 0.6em", color: "var(--muted)", fontFamily: "var(--font-mono)", fontSize: "1.1em" }}>
                {limits.monthly_usd === 0 ? "$0" : `$${limits.monthly_usd}`}<span style={{ fontSize: "0.7em" }}>/mo</span>
              </p>
              <ul style={{ listStyle: "none", padding: 0, margin: 0, fontSize: "0.85em", display: "grid", gap: "0.2em", color: "var(--muted)" }}>
                <li>· {fmtNumber(limits.agents)} agents</li>
                <li>· {fmtNumber(limits.decisions_per_month)} decisions/mo</li>
                <li>· {limits.audit_retention_days}-day audit</li>
                <li>· {limits.support_sla}</li>
              </ul>
              <div style={{ marginTop: "0.8em" }}>
                {/*
                  Codex review fix (PR #311): the previous "Downgrade"
                  button hit /api/billing/checkout which creates a NEW
                  Stripe subscription line item. For an
                  enterprise → pro downgrade that produced a duplicate
                  active subscription instead of changing the existing
                  plan. Downgrades must use the Customer Portal
                  (Stripe-hosted, lets the user pick the new plan from
                  their existing subscription). Upgrades remain on
                  Checkout where new-subscription semantics are correct.
                */}
                {isCurrent ? (
                  <span style={{ fontSize: "0.85em", color: "var(--accent)" }}>● current plan</span>
                ) : limits.monthly_usd < TIER_LIMITS[billing.tier].monthly_usd ? (
                  <span style={{ fontSize: "0.78em", color: "var(--muted)" }}>
                    Downgrade via Customer Portal
                  </span>
                ) : (
                  <UpgradeButton
                    tenantSlug={slug}
                    targetTier={tier as "pro" | "enterprise"}
                    isUpgrade={true}
                    label="Upgrade"
                  />
                )}
              </div>
            </div>
          );
        })}
      </div>

      <aside style={{ marginTop: "1.5em", padding: "1em 1.2em", background: "var(--code-bg)", border: "1px solid var(--border)", borderLeft: "3px solid var(--accent)", borderRadius: "var(--r-md)", color: "var(--muted)", fontSize: "0.88em" }}>
        <strong style={{ color: "var(--fg)" }}>Stripe wired in test mode.</strong>{" "}
        Checkout buttons hit <code>/api/billing/checkout</code> which creates a real Stripe Session in the test sandbox. Set <code>STRIPE_SECRET_KEY</code> + price IDs in Vercel env to switch to live. See <Link href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/dev3/production/02-stripe-billing-runbook.md" target="_blank" rel="noopener">02-stripe-billing-runbook.md</Link> for the daemon-side wiring still pending (tenant tier writeback on subscription.updated webhook).
      </aside>
    </main>
  );
}
