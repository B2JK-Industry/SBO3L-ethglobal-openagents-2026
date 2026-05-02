// Tenant resolution + 3 mock tenants (CTI-3-5 §7 Q2: slug,
// kebab-case). Cross-tenant access enforced server-side in
// /t/[tenant]/layout.tsx via notFound().
import type { Role } from "./roles";

export const SLUG_RE = /^[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])?$/;

export interface Tenant {
  slug: string;
  display_name: string;
  uuid: string;
  tier: "free" | "pro" | "enterprise";
  ens_apex: string;
  created_at: string;
}

// Three seeded tenants per round-12 brief. UUIDs are deterministic
// (uuidv5 over a fixed namespace + slug) so they're stable across
// rebuilds; the mock data layer doesn't actually use them yet but
// the daemon-side per-tenant SQLite path layout (Grace's slice)
// will key off uuid for renaming-stable file paths.
export const MOCK_TENANTS: Tenant[] = [
  { slug: "acme",     display_name: "Acme Corp",      uuid: "tnt-01HZRG-acme",     tier: "pro",        ens_apex: "acme.sbo3lagent.eth",     created_at: "2026-04-15T10:00:00Z" },
  { slug: "contoso",  display_name: "Contoso Ltd",    uuid: "tnt-01HZRG-contoso",  tier: "free",       ens_apex: "contoso.sbo3lagent.eth",  created_at: "2026-04-22T14:00:00Z" },
  { slug: "fabrikam", display_name: "Fabrikam Inc",   uuid: "tnt-01HZRG-fabrikam", tier: "enterprise", ens_apex: "fabrikam.sbo3lagent.eth", created_at: "2026-04-29T09:00:00Z" },
];

export interface TenantMembership {
  tenant_slug: string;
  role: Role;
  added_at: string;
}

// Mock membership map keyed by user identifier. In production this
// comes from a daemon-side per-tenant memberships table (Grace's
// slice). Today we hard-wire so all three mock tenants are visible
// to anyone signed in for the scaffold demo.
const MOCK_MEMBERSHIPS_BY_USER: Record<string, TenantMembership[]> = {
  // Default fallback — every signed-in user has access to all three
  // tenants with operator role. Replace with real lookups when the
  // memberships endpoint ships.
  "*": [
    { tenant_slug: "acme",     role: "operator", added_at: "2026-04-15T10:00:00Z" },
    { tenant_slug: "contoso",  role: "viewer",   added_at: "2026-04-22T14:00:00Z" },
    { tenant_slug: "fabrikam", role: "admin",    added_at: "2026-04-29T09:00:00Z" },
  ],
};

export function membershipsForUser(userId?: string | null): TenantMembership[] {
  if (userId && MOCK_MEMBERSHIPS_BY_USER[userId]) return MOCK_MEMBERSHIPS_BY_USER[userId]!;
  return MOCK_MEMBERSHIPS_BY_USER["*"]!;
}

export function tenantBySlug(slug: string): Tenant | undefined {
  if (!SLUG_RE.test(slug)) return undefined;
  return MOCK_TENANTS.find((t) => t.slug === slug);
}

export function userHasAccessTo(userId: string | null | undefined, tenantSlug: string): TenantMembership | undefined {
  return membershipsForUser(userId).find((m) => m.tenant_slug === tenantSlug);
}

export function isValidSlug(slug: string): boolean {
  return slug.length >= 3 && slug.length <= 32 && SLUG_RE.test(slug);
}
