# CTI-3-5 — Multi-tenant operator console

> **Status:** Design draft. **Audience:** Daniel (review + approve), Dev 3 (next-round implementer), Dev 4 / Grace (deploy + tenant data isolation), Heidi (test plan ramifications).
> **Outcome:** alignment on multi-tenant routing, auth, isolation, and billing surface for the production-grade hosted-app, before any code lands.
> **Not normative.** A future PR titled "feat(hosted-app): CTI-3-5 prep" turns this into code; this doc only commits us to the shape.

The current `apps/hosted-app/` is a single-tenant demo: one logged-in user sees one daemon's data. CTI-3-5 turns it into the production operator console for the SBO3L self-hosted product — multiple tenants per deploy, per-tenant data isolation, per-tier billing gates, per-tenant policy + audit + agents + capsule libraries.

## 1. Constraints inherited from current state

1. **NextAuth v5 + multi-provider auth** — already shipped (#190). Sessions are JWT-backed today; multi-tenant adds a `tenant_id` claim to the JWT.
2. **Role-based middleware gate** — `meetsRole()` + ROLE_GATES — also from #190. Multi-tenant adds tenant scoping on top of the role check.
3. **Daemon HTTP API** at `${SBO3L_DAEMON_URL}/v1/*` — current calls are tenant-implicit (one daemon = one tenant). Multi-tenant requires the daemon to filter responses by `tenant_id` (Grace's slice).
4. **Strict CSP** with `script-src 'self' 'unsafe-inline'` for Next.js RSC streaming. Multi-tenant doesn't change CSP; the route prefix is server-rendered.
5. **WebSocket bus** at `/v1/events` — Dev 1's bus emits all events; per-tenant filtering is a daemon-side query parameter (`?tenant_id=…`) that the operator console threads from session.
6. **17 open PRs** add `/admin/*` routes, `/policy/edit`, `/dashboard`, `/agents`, `/audit`, `/capsules`, `/trust-dns`, `/marketplace/*`, `/submission/*` — every protected route needs to live under `/t/<tenant>/*` once CTI-3-5 lands.

## 2. Routing model

```
/                                   → marketing landing (unchanged)
/login                              → NextAuth sign-in (unchanged; provider buttons + role pill on /403)
/tenants                            → post-login tenant picker (NEW)

/t/<tenant>/dashboard               → moved from /dashboard
/t/<tenant>/agents
/t/<tenant>/audit
/t/<tenant>/capsules
/t/<tenant>/policy/edit
/t/<tenant>/admin/users             → tenant-scoped admins (per-tenant users table)
/t/<tenant>/admin/flags             → tenant-scoped feature flags
/t/<tenant>/admin/audit             → live timeline filtered to tenant
/t/<tenant>/admin/keys              → tenant's KMS backend (one signer per tenant; provider can be different)
/t/<tenant>/admin/billing           → NEW — subscription tier, usage, invoices

/admin                              → root-admin only (cross-tenant; SBO3L operator)
/admin/tenants                      → list + create + suspend tenants
/admin/billing                      → cross-tenant revenue + usage rollup
```

Two role planes:

- **Tenant roles** (`viewer` / `operator` / `admin`) — same triple as today, scoped to one tenant.
- **Root role** (`root_admin`) — SBO3L operator; only path: `/admin/*` (no `/t/` prefix). Out of band from tenant role tree; can impersonate any tenant for support but actions are audit-logged with `acting_as` field.

## 3. Auth model

JWT claims (extends current `auth.config.ts`):

```json
{
  "sub": "<provider>:<account-id>",
  "email": "user@example.com",
  "githubLogin": "alice",
  "tenants": [
    { "id": "acme-corp", "role": "admin", "added_at": "2026-04-15T..." },
    { "id": "research-lab", "role": "operator", "added_at": "2026-05-01T..." }
  ],
  "root_admin": false
}
```

The `tenants` array means a user can belong to N tenants with potentially different roles per tenant. Tenant picker at `/tenants` renders one card per entry; clicking sets a `current_tenant_id` cookie (HttpOnly, SameSite=Lax) and redirects to `/t/<id>/dashboard`. Switching tenants clears the cookie + re-shows the picker.

`middleware.ts` extends the existing ROLE_GATES table with tenant-prefix awareness:

```ts
// Tenant-scoped routes
{ prefix: "/t/:tenant/policy/edit",  need: "admin",    tenantScoped: true },
{ prefix: "/t/:tenant/admin",        need: "admin",    tenantScoped: true },
{ prefix: "/t/:tenant",              need: "viewer",   tenantScoped: true },
// Root-admin
{ prefix: "/admin",                  need: "root_admin", tenantScoped: false },
```

When `tenantScoped: true`, middleware:

1. Extracts `<tenant>` from path.
2. Looks up the user's role within that tenant from `session.tenants[].role`.
3. Applies `meetsRole(role, gate.need)`.
4. On mismatch → `/403` with tenant + role context.

## 4. Tenant isolation (data plane — Grace's slice)

Three boundaries:

### 4.1 Daemon-side per-tenant SQLite

Each tenant gets `/data/tenants/<tenant>/sbo3l.db`. Daemon process resolves `tenant_id` from the request's `Authorization` header (JWT) → opens the right SQLite file → all reads + writes scoped to that file. No cross-tenant query path exists; isolation is structural.

### 4.2 Per-tenant signing key

Each tenant configures its own `Signer` backend (in-memory / file / KMS). Tenants on Free tier share an instance-level signer; Pro/Enterprise tiers get dedicated KMS keys. Surface in `/t/<tenant>/admin/keys`.

### 4.3 Per-tenant ENS record set

Tenants register agents under their own subname tree:
- `acme-corp.sbo3lagent.eth` is the tenant's apex
- `research-01.acme-corp.sbo3lagent.eth` is one of its agents

Agent registration via Durin checks the user's `tenants[].role >= operator` for the requested tenant before issuing the subname.

## 5. Billing surface

Three tiers, gating shown in UI:

| Tier | Agents/tenant | Decisions/day | Audit retention | KMS | Price |
|---|---|---|---|---|---|
| **Free** | 3 | 1k | 7d | shared in-memory | $0 |
| **Pro** | 25 | 25k | 90d | dedicated KMS | $99/mo |
| **Enterprise** | unlimited | metered | 1y + cold storage | dedicated KMS + multi-region | contact |

Soft-fail UX: when a tenant hits a tier limit, the daemon returns `policy.tier_limit_exceeded` (new domain code) and the hosted app surfaces a `<TierExceededBanner>` with upgrade CTA. No hard cutoff at 100% — overage charges for Pro at $0.01 per kdecisions, subject to monthly cap.

`/t/<tenant>/admin/billing` shows:
- Current tier + monthly cost
- Usage strip (agents / decisions / audit days) with per-metric progress bars
- Invoice history (last 12 months)
- "Upgrade" / "Downgrade" buttons → Stripe Checkout

Stripe webhook fires on subscription changes; daemon reads the tier from the tenant record on next request. Tier-change audit event: `billing.tier_changed`.

## 6. Implementation plan (next round)

A spec-prep PR (this doc only) → then:

| Slice | Branch | Scope | LoC est |
|---|---|---|---|
| **a** | `agent/dev3/CTI-3-5-prep` | route tree refactor — `/t/[tenant]/*` skeletons that mirror existing /dashboard etc.; middleware extension; tenant picker at `/tenants`; JWT claim type augmentation | ~450 |
| **b** | `agent/dev3/CTI-3-5-billing-shell` | `/t/[tenant]/admin/billing` page + Stripe Checkout button (env-gated; no real Stripe call until Daniel adds keys) | ~300 |
| **c** | Cross-stack — Daniel + Grace + Dev 4 | per-tenant SQLite path layout (Grace) + tenant management endpoints `/v1/admin/tenants/*` (Dev 1) | n/a — daemon work |
| **d** | `agent/dev3/CTI-3-5-cross-tenant-admin` | `/admin/tenants` listing + create / suspend; `/admin/billing` cross-tenant rollup | ~350 |

Total Dev 3 surface: ~1,100 LoC across 3 PRs, sequenced. Each within the 500-LoC cap.

## 7. Open questions

1. **Stripe vs Paddle vs Lago** — billing provider choice. Stripe is most operator-familiar; Paddle handles VAT / sales tax automatically (relevant for international tenants). Default: Stripe + Lemon Squeezy fallback for non-US. Daniel decides.
2. **Tenant ID format** — slug (`acme-corp`) vs UUID (`tnt_01HZRG...`)? Slugs are user-facing-friendly but require uniqueness checks. Default: slug, validated at create time, reserved namespace for SBO3L-internal tenants.
3. **Tenant picker on every login vs. remembered choice?** — Default: remembered via `current_tenant_id` cookie; switcher always visible in nav header.
4. **Root admin elevation** — auto-grant to `ADMIN_GITHUB_LOGINS` env var users (current convention) or require an explicit per-deploy setup step? Default: continue env-config (consistent with current role assignment).
5. **WebSocket per-tenant filtering** — query param `?tenant_id=…` (cookie-derived) sent on connect; daemon filters server-side. Alternative: subprotocol negotiation. Default: query param (simpler).

## 8. Heidi-facing testability

- **Multi-tenant route contract** — Playwright e2e for: login → tenant picker → switch → cross-tenant access denied → upgrade flow.
- **Tier limit fixture** — replay 1001 decisions on a Free tenant; assert `policy.tier_limit_exceeded` on the 1001st with UI surfacing the upgrade banner.
- **Tenant isolation regression** — admin from `acme-corp` cannot see `research-lab` data via direct URL `/t/research-lab/dashboard` (returns `/403`).
- **Audit event provenance** — every tenant-scoped action writes the audit event into `/data/tenants/<tenant>/sbo3l.db`, not the wrong tenant's DB.

## 9. What this doc is NOT

- **Not a Stripe integration spec.** Billing endpoints + webhook handlers are sketched at the route-shape level; full Stripe wiring lands in slice **b**.
- **Not a daemon-side spec.** Per-tenant SQLite path layout, tenant lifecycle endpoints, and WS filtering are Grace + Dev 1 surface — referenced but not authored here.
- **Not a marketing spec.** Public marketing pricing pages live in `apps/marketing/` (`/pricing` is a placeholder today; CTI-3-5 doesn't change it).
- **Not committed.** This is a draft for Daniel review. Open questions in §7 must be answered before slice **a** opens.

## 10. Review notes

If you're short on time, just answer the §7 open questions:

- **Q1 (billing provider)** unblocks slice **b**.
- **Q2 (tenant ID format)** unblocks slice **a**.
- **Q4 (root admin elevation)** unblocks the cross-tenant admin slice **d**.

Q3 (picker behaviour) and Q5 (WS filter) can be decided in PR review.
