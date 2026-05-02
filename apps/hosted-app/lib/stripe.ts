// Stripe wiring. Uses test-mode keys by default — set STRIPE_SECRET_KEY
// in Vercel env to switch to live mode at production cutover.
//
// Tier mapping is the contract the hosted-app + daemon agree on:
// - Free       → price_free          (free 100 req/day, 30-day audit)
// - Pro $29    → price_pro_monthly   (100K req/day, 1y audit)
// - Enterprise → price_enterprise    (custom — contact sales)
//
// Test fixtures for the price IDs come from the Stripe sandbox account
// per the runbook (docs/dev3/production/02-stripe-billing-runbook.md).
// Override via env so we don't ship real IDs in source.

import Stripe from "stripe";
import type { Tier } from "./tenant-billing";

const TEST_KEY_FALLBACK = "sk_test_dummy_replace_with_real_test_key";

export function stripeClient(): Stripe {
  const key = process.env.STRIPE_SECRET_KEY ?? TEST_KEY_FALLBACK;
  return new Stripe(key, { apiVersion: "2025-01-27.acacia" });
}

export function priceIdFor(tier: Tier): string {
  switch (tier) {
    case "free":       return process.env.STRIPE_PRICE_FREE       ?? "price_test_free_replace";
    case "pro":        return process.env.STRIPE_PRICE_PRO        ?? "price_test_pro_replace";
    case "enterprise": return process.env.STRIPE_PRICE_ENTERPRISE ?? "price_test_enterprise_replace";
  }
}

export function webhookSecret(): string {
  return process.env.STRIPE_WEBHOOK_SECRET ?? "whsec_test_dummy_replace";
}

// Hard / soft limits per tier — daemon enforces these via the tenant's
// monthly decisions counter. Soft = warn, hard = deny with
// `policy.tier_quota_exceeded`.
export const TIER_QUOTA = {
  free:       { soft_per_day: 80,        hard_per_day: 100 },
  pro:        { soft_per_day: 80_000,    hard_per_day: 100_000 },
  enterprise: { soft_per_day: Infinity,  hard_per_day: Infinity },
} as const;

export function quotaPctOfHard(tier: Tier, decisionsToday: number): number {
  const hard = TIER_QUOTA[tier].hard_per_day;
  if (hard === Infinity) return 0;
  return Math.min(100, (decisionsToday / hard) * 100);
}

export interface CheckoutResult {
  url: string;
  sessionId: string;
}

export async function createCheckoutSession(opts: {
  tenantUuid: string;
  tenantSlug: string;
  customerEmail?: string;
  targetTier: Exclude<Tier, "free">;
  successUrl: string;
  cancelUrl: string;
}): Promise<CheckoutResult> {
  const stripe = stripeClient();
  const session = await stripe.checkout.sessions.create({
    mode: "subscription",
    line_items: [{ price: priceIdFor(opts.targetTier), quantity: 1 }],
    success_url: opts.successUrl,
    cancel_url: opts.cancelUrl,
    customer_email: opts.customerEmail,
    metadata: { tenant_uuid: opts.tenantUuid, tenant_slug: opts.tenantSlug, target_tier: opts.targetTier },
    subscription_data: {
      metadata: { tenant_uuid: opts.tenantUuid, tenant_slug: opts.tenantSlug },
    },
  });
  if (!session.url) throw new Error("Stripe checkout session missing URL");
  return { url: session.url, sessionId: session.id };
}

export async function createPortalSession(opts: { customerId: string; returnUrl: string }): Promise<string> {
  const stripe = stripeClient();
  const session = await stripe.billingPortal.sessions.create({
    customer: opts.customerId,
    return_url: opts.returnUrl,
  });
  return session.url;
}

// Map a Stripe Price ID back to the SBO3L tier. Inverse of priceIdFor()
// — used by the webhook handler to translate subscription items into
// the tenant tier column update.
export function tierFromPriceId(priceId: string): Tier | null {
  if (priceId === priceIdFor("pro")) return "pro";
  if (priceId === priceIdFor("enterprise")) return "enterprise";
  if (priceId === priceIdFor("free")) return "free";
  return null;
}
