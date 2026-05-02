# Production migration — SQLite mock → Postgres + RLS

**Status:** design draft (R13 P78)
**Owner:** Dev 3 (frontend) + Grace (daemon storage)
**Trigger:** first paying tenant lands on `app.sbo3l.dev`

## Today

- Daemon writes per-tenant SQLite files at `~/.sbo3l/tenants/<uuid>.db`
  (Grace's slice — see CTI-3-5 §4)
- Hosted-app reads via `GET /v1/tenants/<slug>/*` proxied through the
  daemon socket
- 3 mock tenants seeded in `apps/hosted-app/lib/tenants.ts`
- _Multi-tenant correctness today is "directory boundary"_ — each
  tenant gets a separate file. No row-level cross-tenant queries
  possible, no shared connection pool, no fan-out reporting.

## Why move

1. **Cross-tenant analytics** — admin/audit dashboards across tenants
   require a single queryable surface. SQLite-per-tenant means N
   queries fanned out + joined client-side.
2. **HA** — SQLite single-writer doesn't survive daemon restart
   without filesystem fsync + careful WAL handling. Postgres RDS
   gets us multi-AZ + point-in-time recovery for free.
3. **Migration tooling** — schema bumps in SQLite-per-tenant mean
   walking N files in lockstep. Postgres + sqlx migrations is one
   transaction.

## Schema sketch

```sql
CREATE TABLE tenants (
  uuid          UUID PRIMARY KEY,
  slug          TEXT UNIQUE NOT NULL CHECK (slug ~ '^[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])?$'),
  display_name  TEXT NOT NULL,
  tier          TEXT NOT NULL CHECK (tier IN ('free', 'pro', 'enterprise')),
  ens_apex      TEXT,
  stripe_id     TEXT,                  -- linked at billing onboarding
  created_at    TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE memberships (
  user_sub      TEXT NOT NULL,         -- nextauth subject (github login or email)
  tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
  role          TEXT NOT NULL CHECK (role IN ('admin', 'operator', 'viewer')),
  added_at      TIMESTAMPTZ DEFAULT now(),
  PRIMARY KEY (user_sub, tenant_uuid)
);

CREATE TABLE agents (
  agent_id      TEXT PRIMARY KEY,
  tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
  ens_name      TEXT,
  pubkey_b58    TEXT NOT NULL,
  created_at    TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE audit_events (
  event_id      TEXT PRIMARY KEY,
  tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
  ts_ms         BIGINT NOT NULL,
  kind          TEXT NOT NULL,
  agent_id      TEXT,
  payload_jsonb JSONB NOT NULL
);
-- Postgres takes indexes as separate statements (the inline INDEX
-- clause above is MySQL-style and will fail to parse on PG). The
-- shipped migration in V020__postgres_init.sql already does this
-- correctly; this doc snippet is the example any engineer would copy.
CREATE INDEX idx_audit_tenant_ts ON audit_events (tenant_uuid, ts_ms DESC);

CREATE TABLE capsules (
  capsule_id    TEXT PRIMARY KEY,
  tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
  agent_id      TEXT NOT NULL,
  decision      TEXT,
  emitted_at    TIMESTAMPTZ DEFAULT now(),
  payload_b64   TEXT NOT NULL
);
```

## Row-level security

Every per-tenant table has an RLS policy keyed off
`current_setting('app.tenant_uuid')`:

```sql
ALTER TABLE agents       ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE capsules     ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_agents       ON agents
  USING (tenant_uuid = current_setting('app.tenant_uuid')::uuid);
CREATE POLICY tenant_isolation_audit_events ON audit_events
  USING (tenant_uuid = current_setting('app.tenant_uuid')::uuid);
CREATE POLICY tenant_isolation_capsules     ON capsules
  USING (tenant_uuid = current_setting('app.tenant_uuid')::uuid);
```

Daemon sets the GUC at the start of every transaction:
```sql
SET LOCAL app.tenant_uuid = '<resolved-from-jwt>';
```

RLS contains the failure mode where the daemon **forgets to set**
`app.tenant_uuid` at all — `current_setting` errors out (or, with the
`missing_ok = true` form, returns NULL), and the policy denies. That
case shows up as empty result sets, not data leakage.

**RLS does NOT contain the failure mode where the daemon sets the
GUC to the wrong tenant's UUID** (e.g. via a JWT-claim mix-up). The
policy then evaluates true for that other tenant's rows and serves
them. Defence requires an explicit *application-layer* membership
check before setting the GUC, e.g.:

```rust
let user_id = jwt.sub;
let claimed_tenant = jwt.tenant;
let allowed = memberships
    .has(user_id, claimed_tenant)
    .await?;                               // explicit auth check
if !allowed { return Err(Forbidden); }
sqlx::raw_sql(&format!(
    "SET LOCAL app.tenant_uuid = '{}'", claimed_tenant
)).execute(&mut *tx).await?;
```

Treat the GUC as a *consequence* of an authorization decision the
application has already made, not a substitute for one.

## Migration steps

1. **Stand up Postgres** (Supabase free tier is fine for hackathon →
   pilot migration). Apply schema + RLS policies.
2. **Backfill** — write a one-shot Rust binary that walks
   `~/.sbo3l/tenants/*.db`, opens each, dumps to a single Postgres
   schema. Idempotent on re-run (use `INSERT ... ON CONFLICT DO NOTHING`).
3. **Dual-write window** — daemon writes to both stores for 1 week.
   Compare row counts daily via a CI cron.
4. **Cutover** — flip `daemon.toml` from `storage = "sqlite-per-tenant"`
   to `storage = "postgres"`. Roll back is a config flip + replay
   (the SQLite files stay around).
5. **SQLite drop** — after 30 days of clean Postgres metrics + zero
   incidents, archive the SQLite files to S3 cold storage and remove
   the SQLite code path.

## Risks

- **Row-count drift during dual-write** — need a daily reconciliation
  job that flags any tenant where SQLite and Postgres disagree.
- **GUC leak across queries** — connection pool MUST reset
  `app.tenant_uuid` to NULL on connection return. `SET LOCAL` scopes
  to transaction so commit/rollback resets it; abandoned transactions
  could leak. Mitigation: pool with `idle_in_transaction_session_timeout = 30s`.
- **Migration downtime** — schema changes that touch RLS need a
  `BEGIN; ALTER TABLE ... DISABLE ROW LEVEL SECURITY; ... ENABLE; COMMIT`
  dance to avoid temporarily exposing rows. Document in runbook.

## Rollback plan

The SQLite files are the source of truth until the 30-day soak period
ends. Cutover is a `daemon.toml` flip; reverting takes <60s (restart
the daemon). After the SQLite drop, rollback requires restoring from
the most recent S3 archive — `<24h` tolerable, `<1h` requires the
soak window to be revisited.
