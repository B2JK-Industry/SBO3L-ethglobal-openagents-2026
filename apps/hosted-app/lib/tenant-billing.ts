// Mock per-tenant billing snapshot. Real source: Stripe Subscription
// for the tenant's customer. Hosted-app reads via /v1/tenants/<slug>/billing
// (proxied to Stripe Subscription Get + cached for 60s).
//
// See docs/dev3/production/02-stripe-billing-runbook.md for the
// production wiring plan.

export type Tier = "free" | "pro" | "enterprise";

export interface TierLimits {
  tenants: number | "unlimited";
  agents: number | "unlimited";
  decisions_per_month: number | "unlimited";
  audit_retention_days: number;
  support_sla: string;
  monthly_usd: number;
}

export const TIER_LIMITS: Record<Tier, TierLimits> = {
  free: {
    tenants: 1,
    agents: 3,
    decisions_per_month: 10_000,
    audit_retention_days: 30,
    support_sla: "Community",
    monthly_usd: 0,
  },
  pro: {
    tenants: 5,
    agents: "unlimited",
    decisions_per_month: 1_000_000,
    audit_retention_days: 365,
    support_sla: "Email · 24h response",
    monthly_usd: 29,
  },
  enterprise: {
    tenants: "unlimited",
    agents: "unlimited",
    decisions_per_month: "unlimited",
    audit_retention_days: 2555,
    support_sla: "Dedicated · 1h response",
    monthly_usd: 499,
  },
};

export interface TenantBilling {
  tier: Tier;
  next_invoice_at?: string;
  decisions_this_month: number;
  agents_count: number;
  payment_method?: string;
}

const BILLING_SNAPSHOTS: Record<string, TenantBilling> = {
  acme:     { tier: "pro",        next_invoice_at: "2026-06-01", decisions_this_month: 47_320, agents_count: 3, payment_method: "Visa •• 4242" },
  contoso:  { tier: "free",                                       decisions_this_month: 1_204,  agents_count: 1 },
  fabrikam: { tier: "enterprise", next_invoice_at: "2026-06-01", decisions_this_month: 2_104_802, agents_count: 4, payment_method: "Wire transfer · NET-30" },
};

export function billingForTenant(slug: string): TenantBilling | undefined {
  return BILLING_SNAPSHOTS[slug];
}

export function fmtNumber(value: number | "unlimited"): string {
  if (value === "unlimited") return "unlimited";
  return value.toLocaleString();
}
