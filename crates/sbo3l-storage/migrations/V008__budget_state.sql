-- SBO3L V008: persistent budget state (F-2).
--
-- Backs `sbo3l-policy::BudgetTracker` and the daemon's
-- `POST /v1/payment-requests` enforcement so a budget commit survives
-- daemon restart, crashes, and multi-process deployment. Replaces the
-- in-memory `HashMap<(agent_id, scope, scope_key), Decimal>` previously
-- held in `AppState::budgets`.
--
-- Rows model committed spend per (agent, scope, scope_key) bucket:
--   * scope = 'per_tx'        — informational only; the per-tx cap is
--                                evaluated against the request's own
--                                amount, never accumulated, never
--                                persisted (no rows are inserted with
--                                this scope; the CHECK accepts it for
--                                forward-compat with future per-tx
--                                instrumentation).
--   * scope = 'daily'         — scope_key is the UTC date in
--                                `%Y-%m-%d`; rolls over implicitly when
--                                a new day's request creates a new row.
--   * scope = 'monthly'       — scope_key is `%Y-%m` UTC.
--   * scope = 'per_provider'  — scope_key is the provider id (or, when
--                                the policy has no entry, the verbatim
--                                request `provider_url`).
--
-- `cap_cents` is denormalised into the row at upsert time so a strict
-- offline verifier can re-derive the deny decision from the row alone
-- without re-reading the policy. `reset_at_unix` records the next
-- bucket boundary as a unix epoch second; it is informational for
-- diagnostics today and the field a future GC sweep would key on.

BEGIN;

CREATE TABLE IF NOT EXISTS budget_state (
  agent_id      TEXT NOT NULL,
  scope         TEXT NOT NULL CHECK (scope IN ('per_tx', 'daily', 'monthly', 'per_provider')),
  scope_key     TEXT NOT NULL,
  spent_cents   INTEGER NOT NULL DEFAULT 0 CHECK (spent_cents >= 0),
  cap_cents     INTEGER NOT NULL CHECK (cap_cents >= 0),
  reset_at_unix INTEGER,
  PRIMARY KEY (agent_id, scope, scope_key)
);

CREATE INDEX IF NOT EXISTS idx_budget_state_agent
  ON budget_state(agent_id, scope);

COMMIT;
