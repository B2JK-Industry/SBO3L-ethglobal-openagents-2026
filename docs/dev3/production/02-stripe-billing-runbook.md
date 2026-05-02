# Stripe billing — Free / Pro / Enterprise rollout runbook

**Status:** design draft (R13 P78)
**Owner:** Dev 3 (hosted-app) + business
**Trigger:** SBO3L Cloud beta opens (post-hackathon)

## Tiers

| Tier | Price | Tenants | Agents | Decisions / mo | Audit retention | Support |
|---|---|---|---|---|---|---|
| **Free** | $0 | 1 | 3 | 10K | 30 days | Community |
| **Pro** | $29 / mo | 5 | unlimited | 1M | 1 year | Email, 24h SLA |
| **Enterprise** | from $499 / mo | unlimited | unlimited | unlimited | 7 years | Dedicated, 1h SLA |

Tier is a column on the `tenants` table (see Postgres design doc); the
daemon enforces caps at policy-decision time (deny with `policy.tier_quota`).

## Stripe objects

- **Product** — single \"SBO3L Cloud\" product
- **Prices** — one per tier:
  - `price_free` ($0/mo, recurring) — kept in Stripe so subscription
    state machine is uniform
  - `price_pro_monthly` ($29/mo)
  - `price_enterprise_starter_monthly` ($499/mo)
- **Customer** — one per tenant (NOT per user). Tenant `stripe_id`
  column links to Customer.
- **Subscription** — one active per tenant. Tier upgrades = swap
  subscription items.
- **Invoice** — Stripe-hosted, emailed monthly.

## Webhook flow

1. User clicks \"Upgrade to Pro\" on `/t/<slug>/admin/billing`
2. Hosted-app creates a Checkout Session with
   `metadata: { tenant_uuid }` + redirect URLs
3. User completes Checkout on Stripe-hosted page
4. Stripe POSTs `checkout.session.completed` webhook → hosted-app
5. Webhook handler:
   - verifies signature (HMAC `STRIPE_WEBHOOK_SECRET`)
   - extracts `tenant_uuid` from session metadata
   - updates `tenants.tier = 'pro'` and `tenants.stripe_id = customer_id`
   - publishes `tenant.tier_changed` to the daemon's event bus so
     in-flight policy decisions pick up the new quota immediately
6. User redirects to `/t/<slug>/admin/billing?upgraded=1`

## Routes

- `POST /api/billing/checkout` — body `{ tenant_slug, target_tier }`,
  returns `{ url }` for client redirect. Admin-role required.
- `POST /api/billing/portal` — returns Stripe Customer Portal URL so
  customers can update payment method + cancel without leaving the
  app
- `POST /api/billing/webhook` — Stripe-only, signature-verified.
  Idempotent on `event.id`.

## Failure modes + mitigation

- **Webhook missed / delayed** — reconcile nightly: walk Stripe
  subscriptions, compare to `tenants.tier`. Backfill mismatches with
  a one-shot job. Alert if drift > 1.
- **Card declined on renewal** — Stripe retries 3× over 21 days.
  After final failure, downgrade to Free + email tenant admins.
  Daemon enforces the new quota immediately, so Pro features
  silently start denying. Add a UI banner that surfaces the
  past-due state from the Customer Portal.
- **Refund / chargeback** — manual ops procedure. Log in Stripe
  Dashboard, replay the relevant webhook to flip tier. Track in
  `accounting/incidents.md` for finance reconciliation.

## Test plan

- Stripe CLI replays for the 4 webhook events we care about:
  `checkout.session.completed`, `customer.subscription.updated`,
  `customer.subscription.deleted`, `invoice.payment_failed`
- Local fixture mode: `STRIPE_MODE=test` mounts a mocked checkout
  + auto-success webhook so PR CI can run end-to-end without
  hitting Stripe
- E2E playwright spec: \"contoso (free) upgrades to pro and
  immediately spends past the free quota\" → expect allow

## What this PR does NOT cover

- **Annual billing** — needs a `Price` per period, doable later
- **Coupon codes / referrals** — out of scope for v1
- **Self-serve enterprise** — enterprise is sales-led, not Checkout-led
- **Tax handling** — Stripe Tax is one toggle but adds VAT complexity
  (need EU billing addresses); deferred until first EU tenant
- **Per-seat pricing** — tier is per-tenant, not per-user. Adding
  per-seat means Subscription items per-membership, which is a
  whole different state machine.

## Rollback

Until first paying tenant: feature-flag the billing routes off and
default everyone to `free` tier. Daemon already treats unknown
tier as the most-restrictive (free) — so even a partial rollout
fails closed.
