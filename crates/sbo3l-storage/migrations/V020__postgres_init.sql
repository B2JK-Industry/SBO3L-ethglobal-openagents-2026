-- V020 — Postgres-flavoured initial schema for the multi-tenant
-- production deployment (Track: docs/dev3/production/01-postgres-rls-migration.md).
--
-- Live behind the `postgres` cargo feature. SQLite remains the
-- default backend (V001..V010 untouched) — this file is only read
-- by the sqlx-postgres migrator under crate::pg.
--
-- Every per-tenant table has Row-Level Security keyed off the
-- `app.tenant_uuid` GUC. Daemon sets `SET LOCAL app.tenant_uuid = ...`
-- at the start of every transaction; misconfiguration shows up as
-- empty results, not data leakage.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ─── Tenants ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS tenants (
    uuid          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug          TEXT UNIQUE NOT NULL CHECK (slug ~ '^[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])?$'),
    display_name  TEXT NOT NULL,
    tier          TEXT NOT NULL CHECK (tier IN ('free', 'pro', 'enterprise')),
    ens_apex      TEXT,
    stripe_customer_id      TEXT,
    stripe_subscription_id  TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tenants_stripe_customer ON tenants(stripe_customer_id) WHERE stripe_customer_id IS NOT NULL;

-- ─── Memberships ─────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS memberships (
    user_sub      TEXT NOT NULL,
    tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'operator', 'viewer')),
    added_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_sub, tenant_uuid)
);

CREATE INDEX IF NOT EXISTS idx_memberships_tenant ON memberships(tenant_uuid);

-- ─── Agents ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS agents (
    agent_id      TEXT PRIMARY KEY,
    tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
    ens_name      TEXT,
    pubkey_b58    TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_agents_tenant ON agents(tenant_uuid);

-- ─── Audit events ────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS audit_events (
    event_id      TEXT PRIMARY KEY,
    tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
    ts_ms         BIGINT NOT NULL,
    kind          TEXT NOT NULL,
    agent_id      TEXT,
    decision      TEXT CHECK (decision IS NULL OR decision IN ('allow', 'deny')),
    deny_code     TEXT,
    payload       JSONB NOT NULL,
    chain_prev    BYTEA,
    chain_hash    BYTEA NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_tenant_ts ON audit_events(tenant_uuid, ts_ms DESC);
CREATE INDEX IF NOT EXISTS idx_audit_agent     ON audit_events(agent_id) WHERE agent_id IS NOT NULL;

-- ─── Capsules ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS capsules (
    capsule_id    TEXT PRIMARY KEY,
    tenant_uuid   UUID NOT NULL REFERENCES tenants(uuid) ON DELETE CASCADE,
    agent_id      TEXT NOT NULL,
    decision      TEXT,
    emitted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    payload_b64   TEXT NOT NULL,
    size_bytes    INTEGER GENERATED ALWAYS AS (length(payload_b64)) STORED
);

CREATE INDEX IF NOT EXISTS idx_capsules_tenant ON capsules(tenant_uuid);
CREATE INDEX IF NOT EXISTS idx_capsules_agent  ON capsules(agent_id);

-- ─── Stripe events (idempotency table for webhook handler) ───────
CREATE TABLE IF NOT EXISTS stripe_events (
    stripe_event_id   TEXT PRIMARY KEY,
    type              TEXT NOT NULL,
    received_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed         BOOLEAN NOT NULL DEFAULT FALSE
);

-- ─── Row-Level Security ──────────────────────────────────────────
-- Daemon sets `SET LOCAL app.tenant_uuid = '<uuid>'` at the start of
-- every per-tenant transaction. The policy below filters every read
-- + write to that UUID. A leak in the daemon shows up as empty
-- results (queries return zero rows) rather than cross-tenant
-- data exposure.

ALTER TABLE agents       ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE capsules     ENABLE ROW LEVEL SECURITY;

CREATE POLICY agents_tenant_isolation
    ON agents
    USING (tenant_uuid = current_setting('app.tenant_uuid', TRUE)::uuid)
    WITH CHECK (tenant_uuid = current_setting('app.tenant_uuid', TRUE)::uuid);

CREATE POLICY audit_events_tenant_isolation
    ON audit_events
    USING (tenant_uuid = current_setting('app.tenant_uuid', TRUE)::uuid)
    WITH CHECK (tenant_uuid = current_setting('app.tenant_uuid', TRUE)::uuid);

CREATE POLICY capsules_tenant_isolation
    ON capsules
    USING (tenant_uuid = current_setting('app.tenant_uuid', TRUE)::uuid)
    WITH CHECK (tenant_uuid = current_setting('app.tenant_uuid', TRUE)::uuid);

-- The `tenants` table itself is admin-scope; superuser bypasses RLS
-- (which is what we want for tenant CRUD by ops). No policy applied.
-- Memberships are read by the auth layer before tenant_uuid is set,
-- so they too remain RLS-free; access there is enforced at the
-- application layer (auth middleware).

-- ─── updated_at trigger ──────────────────────────────────────────
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tenants_set_updated_at ON tenants;
CREATE TRIGGER tenants_set_updated_at
    BEFORE UPDATE ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
