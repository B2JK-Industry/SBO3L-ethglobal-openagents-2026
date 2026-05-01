-- Mandate V006: persistent active-policy lifecycle (PSM-A3).
--
-- Backs `Storage::policy_*` and the `mandate policy {validate,current,activate,diff}`
-- CLI surface. Stores every activated policy version verbatim (the
-- canonical JSON the operator handed to `policy activate`) plus
-- metadata (hash, source, activation timestamps).
--
-- Singleton invariant: at any moment at most one row has
-- `deactivated_at IS NULL`. Enforced via a partial UNIQUE index — see
-- `idx_active_policy_singleton` below — so a buggy CLI cannot leave
-- two simultaneously-active policies. Activating a new policy
-- atomically marks the previous active row's `deactivated_at`.
--
-- This is **local production-shaped lifecycle**, not remote
-- governance. There is no on-chain anchor, no distributed consensus,
-- no signing on activation; whoever runs `mandate policy activate`
-- on this DB activates the policy. Documented in
-- `docs/cli/policy.md`.

BEGIN;

CREATE TABLE IF NOT EXISTS active_policy (
    version        INTEGER PRIMARY KEY,
    policy_hash    TEXT NOT NULL UNIQUE,
    policy_json    TEXT NOT NULL,
    activated_at   TEXT NOT NULL,
    deactivated_at TEXT NULL,
    source         TEXT NOT NULL  -- e.g. "operator-cli", "embedded-ref-v1"
);

-- Singleton: at most one row may be active at any moment.
--
-- Codex P1 review on PR #35 caught that a partial UNIQUE index keyed
-- directly on `deactivated_at` does NOT enforce the singleton in
-- SQLite — `UNIQUE` indexes treat each `NULL` as distinct, so two
-- rows with `deactivated_at IS NULL` both pass the constraint and
-- multiple active policies are possible through manual / concurrent
-- writes (the in-tx CLI guard we already have only protects the
-- `mandate policy activate` path).
--
-- The fix keys the index on `(deactivated_at IS NULL)`, an integer
-- expression that is `1` for active rows and `0` for deactivated
-- rows. Combined with the partial `WHERE deactivated_at IS NULL`,
-- the index contains at most one entry — the value `1` — and a
-- second active insert fails with a UNIQUE constraint error at the
-- DB layer. Historical (deactivated) rows are excluded from the
-- partial index entirely and do not contend.
CREATE UNIQUE INDEX IF NOT EXISTS idx_active_policy_singleton
    ON active_policy((deactivated_at IS NULL))
    WHERE deactivated_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_active_policy_hash ON active_policy(policy_hash);

COMMIT;
