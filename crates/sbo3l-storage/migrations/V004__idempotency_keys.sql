-- Mandate V004: HTTP Idempotency-Key safe-retry table.
--
-- Backs `Storage::idempotency_lookup` / `idempotency_store`, called by the
-- HTTP daemon BEFORE schema validation / nonce gate / policy / budget /
-- audit / signing on `POST /v1/payment-requests`.
--
-- Behaviour matrix:
--   - Same Idempotency-Key + same request_hash → return cached response,
--     never re-run the pipeline (so policy/budget/audit/signing never
--     execute twice for one logical request).
--   - Same Idempotency-Key + different request_hash → 409
--     `protocol.idempotency_conflict`.
--   - Cached responses are 200-only — failure responses (4xx/5xx) are
--     intentionally not cached so a client can retry past a transient
--     failure through the full pipeline.
--
-- TTL eviction is out of scope for this migration. APRP requests have an
-- `expires_at` window and a future migration can `DELETE WHERE created_at
-- < now() - <window>` without changing rejection semantics.

BEGIN;

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key             TEXT PRIMARY KEY,
    request_hash    TEXT NOT NULL,
    response_status INTEGER NOT NULL,
    response_body   TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_created_at ON idempotency_keys(created_at);

COMMIT;
