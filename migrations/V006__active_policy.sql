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

-- Singleton: at most one row may be active at any moment. The partial
-- index keys only NULL `deactivated_at` rows, so historical rows do
-- not contend.
CREATE UNIQUE INDEX IF NOT EXISTS idx_active_policy_singleton
    ON active_policy(deactivated_at) WHERE deactivated_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_active_policy_hash ON active_policy(policy_hash);

COMMIT;
