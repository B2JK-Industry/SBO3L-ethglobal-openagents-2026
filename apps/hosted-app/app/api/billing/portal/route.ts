import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { tenantBySlug, userHasAccessTo } from "@/lib/tenants";
import { billingForTenant } from "@/lib/tenant-billing";
import { createPortalSession } from "@/lib/stripe";

interface RequestBody { tenant_slug: string }

// Returns a Stripe Customer Portal URL so users can update payment
// method / cancel without leaving the app. Admin-only.
export async function POST(req: Request): Promise<NextResponse> {
  const session = await auth();
  const userId = session?.user?.githubLogin ?? session?.user?.email ?? null;
  if (!userId) return NextResponse.json({ error: "unauthenticated" }, { status: 401 });

  let body: RequestBody;
  try {
    body = (await req.json()) as RequestBody;
  } catch {
    return NextResponse.json({ error: "invalid_body" }, { status: 400 });
  }

  const tenant = tenantBySlug(body.tenant_slug);
  if (!tenant) return NextResponse.json({ error: "not_found" }, { status: 404 });
  const membership = userHasAccessTo(userId, body.tenant_slug);
  if (!membership || membership.role !== "admin") {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }

  // Mock fixtures don't store a real Stripe customer ID; produce a
  // friendly error until the daemon's /v1/tenants/<uuid>/billing
  // endpoint exposes the real customer. Production reads it from the
  // tenants table (see Postgres migration design).
  const billing = billingForTenant(body.tenant_slug);
  const customerId = (billing as { stripe_customer_id?: string } | undefined)?.stripe_customer_id;
  if (!customerId) {
    return NextResponse.json({ error: "no_stripe_customer", detail: "Tenant has no Stripe customer linked yet — complete a Checkout flow first." }, { status: 409 });
  }

  const origin = req.headers.get("origin") ?? "https://sbo3l-app.vercel.app";
  try {
    const url = await createPortalSession({
      customerId,
      returnUrl: `${origin}/t/${body.tenant_slug}/admin/billing`,
    });
    return NextResponse.json({ url });
  } catch (e) {
    return NextResponse.json({ error: "stripe_error", detail: (e as Error).message }, { status: 502 });
  }
}
