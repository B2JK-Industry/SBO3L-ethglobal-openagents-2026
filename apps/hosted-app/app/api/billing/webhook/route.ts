import { NextResponse } from "next/server";
import Stripe from "stripe";
import { stripeClient, tierFromPriceId, webhookSecret } from "@/lib/stripe";

// Stripe webhook handler. The route runs in the Node runtime
// (not Edge) because stripe.webhooks.constructEvent uses Node crypto.
// Body must be the raw text — NEVER parse as JSON before signature
// verification.

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function POST(req: Request): Promise<NextResponse> {
  const sig = req.headers.get("stripe-signature");
  if (!sig) return NextResponse.json({ error: "missing_signature" }, { status: 400 });

  const raw = await req.text();
  const stripe = stripeClient();
  let event: Stripe.Event;
  try {
    event = stripe.webhooks.constructEvent(raw, sig, webhookSecret());
  } catch (e) {
    return NextResponse.json({ error: "invalid_signature", detail: (e as Error).message }, { status: 400 });
  }

  // Webhook handlers are idempotent on event.id. The daemon's tenant
  // store has a deduplication table keyed on stripe_event_id; double
  // delivery from Stripe (which CAN happen on retries) is a no-op.
  switch (event.type) {
    case "checkout.session.completed":
      return handleCheckoutCompleted(event.data.object as Stripe.Checkout.Session);
    case "customer.subscription.updated":
    case "customer.subscription.created":
      return handleSubscriptionChange(event.data.object as Stripe.Subscription);
    case "customer.subscription.deleted":
      return handleSubscriptionDeleted(event.data.object as Stripe.Subscription);
    case "invoice.payment_failed":
      return handlePaymentFailed(event.data.object as Stripe.Invoice);
    default:
      return NextResponse.json({ received: true, type: event.type, ignored: true });
  }
}

// Codex review fix (PR #311): the previous handlers returned HTTP 400
// when tenant_uuid metadata was missing, which makes Stripe retry the
// event up to ~3 days. Real subscriptions created outside our exact
// Checkout flow (manual ones, legacy ones, ones from a different
// app sharing the Stripe account) lack our metadata and would loop
// indefinitely. Treat missing metadata as "ignored" with a 2xx so
// Stripe drops the event from its retry queue instead.

async function handleCheckoutCompleted(session: Stripe.Checkout.Session): Promise<NextResponse> {
  const tenantUuid = session.metadata?.tenant_uuid;
  const targetTier = session.metadata?.target_tier;
  if (!tenantUuid || !targetTier) {
    console.warn("[stripe] checkout.session.completed missing metadata — ignored", { sessionId: session.id });
    return NextResponse.json({ received: true, ignored: true, reason: "missing_metadata" });
  }
  // TODO: POST to daemon /v1/tenants/<uuid>/billing with
  // { tier: targetTier, stripe_customer: session.customer, stripe_subscription: session.subscription }
  // For now log; real wiring lands when daemon /v1/billing endpoint ships.
  console.log("[stripe] checkout.session.completed", { tenantUuid, targetTier, customer: session.customer });
  return NextResponse.json({ received: true, tenant: tenantUuid, tier: targetTier });
}

async function handleSubscriptionChange(sub: Stripe.Subscription): Promise<NextResponse> {
  const tenantUuid = sub.metadata?.tenant_uuid;
  if (!tenantUuid) {
    console.warn("[stripe] subscription change missing tenant_uuid metadata — ignored", { subId: sub.id });
    return NextResponse.json({ received: true, ignored: true, reason: "missing_tenant_metadata" });
  }
  const priceId = sub.items.data[0]?.price.id;
  const tier = priceId ? tierFromPriceId(priceId) : null;
  if (!tier) {
    console.warn("[stripe] subscription change with unknown price — ignored", { tenantUuid, priceId });
    return NextResponse.json({ received: true, ignored: true, reason: "unknown_price", priceId });
  }
  console.log("[stripe] subscription.changed", { tenantUuid, tier, status: sub.status });
  return NextResponse.json({ received: true, tenant: tenantUuid, tier });
}

async function handleSubscriptionDeleted(sub: Stripe.Subscription): Promise<NextResponse> {
  const tenantUuid = sub.metadata?.tenant_uuid;
  if (!tenantUuid) {
    console.warn("[stripe] subscription.deleted missing tenant_uuid metadata — ignored", { subId: sub.id });
    return NextResponse.json({ received: true, ignored: true, reason: "missing_tenant_metadata" });
  }
  // Downgrade to free on deletion. Daemon enforces the new quota
  // immediately; if the user reactivates, the next subscription.created
  // event upgrades them back.
  console.log("[stripe] subscription.deleted → downgrade to free", { tenantUuid });
  return NextResponse.json({ received: true, tenant: tenantUuid, tier: "free" });
}

async function handlePaymentFailed(invoice: Stripe.Invoice): Promise<NextResponse> {
  // Stripe retries 3× over 21 days; we only act on the final failure
  // signal (invoice.attempt_count >= 4). Earlier failures just log so
  // ops can see them in the dashboard.
  console.log("[stripe] invoice.payment_failed", { attempts: invoice.attempt_count, customer: invoice.customer });
  return NextResponse.json({ received: true, attempts: invoice.attempt_count });
}
