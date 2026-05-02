import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { tenantBySlug, userHasAccessTo } from "@/lib/tenants";
import { createCheckoutSession } from "@/lib/stripe";

interface RequestBody {
  tenant_slug: string;
  target_tier: "pro" | "enterprise";
}

export async function POST(req: Request): Promise<NextResponse> {
  const session = await auth();
  const userId = session?.user?.githubLogin ?? session?.user?.email ?? null;
  const userEmail = session?.user?.email ?? undefined;
  if (!userId) return NextResponse.json({ error: "unauthenticated" }, { status: 401 });

  let body: RequestBody;
  try {
    body = (await req.json()) as RequestBody;
  } catch {
    return NextResponse.json({ error: "invalid_body" }, { status: 400 });
  }
  if (!body.tenant_slug || !["pro", "enterprise"].includes(body.target_tier)) {
    return NextResponse.json({ error: "invalid_body" }, { status: 400 });
  }

  const tenant = tenantBySlug(body.tenant_slug);
  if (!tenant) return NextResponse.json({ error: "not_found" }, { status: 404 });

  const membership = userHasAccessTo(userId, body.tenant_slug);
  if (!membership || membership.role !== "admin") {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }

  const origin = req.headers.get("origin") ?? "https://sbo3l-app.vercel.app";
  try {
    const result = await createCheckoutSession({
      tenantUuid: tenant.uuid,
      tenantSlug: body.tenant_slug,
      customerEmail: userEmail,
      targetTier: body.target_tier,
      successUrl: `${origin}/t/${body.tenant_slug}/admin/billing?upgraded=1&session={CHECKOUT_SESSION_ID}`,
      cancelUrl:  `${origin}/t/${body.tenant_slug}/admin/billing?canceled=1`,
    });
    return NextResponse.json(result);
  } catch (e) {
    return NextResponse.json({ error: "stripe_error", detail: (e as Error).message }, { status: 502 });
  }
}
