-- Mandate V002: persistent APRP nonce replay protection.
--
-- Backs `Storage::nonce_try_claim`, which the HTTP daemon calls before any
-- policy / budget / audit / signing work. The PRIMARY KEY on `nonce` gives
-- us atomic insert-or-fail semantics: two concurrent requests with the same
-- nonce both attempt INSERT, exactly one succeeds, the loser sees
-- `SQLITE_CONSTRAINT_PRIMARYKEY` and is rejected with HTTP 409
-- `protocol.nonce_replay`.
--
-- `agent_id` is recorded for diagnostics. `seen_at` is recorded so a future
-- TTL-eviction sweep can drop entries past the APRP `expires_at` window
-- without changing rejection semantics — out of scope for this migration.

BEGIN;

CREATE TABLE IF NOT EXISTS nonce_replay (
    nonce       TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL,
    seen_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nonce_replay_seen_at ON nonce_replay(seen_at);

COMMIT;
