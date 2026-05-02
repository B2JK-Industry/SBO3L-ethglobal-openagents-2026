-- T-3 multi-tenant scoping. Adds a `tenant_id` column to the
-- audit_events table so a single SBO3L instance can serve N
-- isolated tenants — each tenant sees only their own audit chain.
--
-- Design notes:
--
-- 1. `tenant_id TEXT NOT NULL DEFAULT 'default'` — additive
--    migration. Existing rows backfill to `'default'`, new
--    single-tenant deployments stay on the same row distribution
--    they had pre-V010. Multi-tenant deployments set
--    `SBO3L_MULTI_TENANT=1` and route per-tenant via the new
--    `audit_*_for_tenant` methods on `Storage`.
--
-- 2. `seq INTEGER PRIMARY KEY` is preserved as a GLOBAL identity
--    across all tenants. Each tenant's logical chain is the
--    subsequence WHERE tenant_id=X, with `prev_event_hash` linking
--    only events within that subsequence. The audit chain's
--    cryptographic integrity is therefore per-tenant: a tampered
--    event in tenant A's chain doesn't disturb tenant B's
--    verification, and a tenant cannot forge an event linking to
--    another tenant's prev_event_hash because the per-tenant
--    `audit_last_for_tenant` query returns only their own tail.
--
-- 3. Index on `(tenant_id, seq)` so the per-tenant queries
--    (`audit_count_for_tenant`, `audit_list_for_tenant`,
--    `audit_last_for_tenant`) hit a single covering index instead
--    of a full table scan.

BEGIN;

ALTER TABLE audit_events
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_audit_events_tenant_seq
    ON audit_events(tenant_id, seq);

COMMIT;
