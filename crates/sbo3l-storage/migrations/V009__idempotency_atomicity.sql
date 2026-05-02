-- SBO3L V009: idempotency atomicity (F-3).
--
-- Adds a `state` column to `idempotency_keys` so the request path can
-- atomically CLAIM a key BEFORE running the pipeline, instead of running
-- the pipeline first and INSERTing post-success. Closes the F-3 race
-- where two concurrent same-key requests both observed cache miss in the
-- pre-claim lookup, both ran the full pipeline (consuming two nonces, two
-- budget commits, two audit rows), and then only the second's INSERT
-- failed on PRIMARY KEY constraint.
--
-- State semantics:
--   * `processing`  — the key has been claimed by an in-flight request.
--                     A concurrent same-key request returns HTTP 409
--                     `protocol.idempotency_in_flight`.
--   * `succeeded`   — the request finished with HTTP 200; the cached
--                     response in `(response_status, response_body)` is
--                     authoritative for byte-identical replay on
--                     same-key + same-body retry.
--   * `failed`      — the request finished with a non-200 status. The
--                     row is left in place for a 60-second grace window
--                     during which a same-key retry returns
--                     `idempotency_in_flight`; past the grace window
--                     the row is reclaimable for a fresh attempt.
--
-- Pre-V009 rows pre-date the state machine and were inserted only on
-- success, so the migration backfills them to `succeeded`.
--
-- The `(state, created_at)` index supports the grace-window query
-- `WHERE key = ? AND state = 'failed' AND created_at < ?`.

BEGIN;

ALTER TABLE idempotency_keys
  ADD COLUMN state TEXT NOT NULL DEFAULT 'succeeded'
    CHECK (state IN ('processing', 'succeeded', 'failed'));

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_state_created_at
  ON idempotency_keys(state, created_at);

COMMIT;
