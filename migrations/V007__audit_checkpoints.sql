-- Mandate V007: persistent audit checkpoints with **mock** anchoring.
--
-- Backs `Storage::audit_checkpoint_*` and the
-- `mandate audit checkpoint {create,verify}` CLI surface (PSM-A4).
-- Stores one row per checkpoint: the chain tip captured at the
-- moment of creation (sequence + latest event id + latest event
-- hash + an aggregated chain-prefix digest) plus a mock anchor
-- reference that simulates the shape of an on-chain anchor without
-- ever leaving the process.
--
-- Truthfulness rules:
-- - This is **mock** anchoring, NOT real on-chain anchoring. The
--   `mock_anchor_ref` is a deterministic local id derived from the
--   checkpoint's content, never broadcast and never attested by any
--   chain. A real anchor would be e.g. a Merkle root committed to
--   an L2 contract or an Ethereum transaction hash.
-- - The CLI surface enforces a `mock-anchor:` prefix on every output
--   line for loud disclosure. See `docs/cli/audit-checkpoint.md`.
-- - `chain_digest` is `SHA-256(event_hash[0] || event_hash[1] || …
--   || event_hash[N-1])` over the chain prefix through `sequence`.
--   That makes the whole prefix verifiable from a single 32-byte
--   commitment without depending on the audit-event hash linkage.
-- - `(sequence, latest_event_hash, chain_digest)` is conceptually
--   redundant — `chain_digest` alone could anchor the prefix — but
--   we keep all three in the row so an offline verifier can sanity-
--   check the components independently.

BEGIN;

CREATE TABLE IF NOT EXISTS audit_checkpoints (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence          INTEGER NOT NULL,            -- highest seq covered
    latest_event_id   TEXT NOT NULL,               -- chain tip id
    latest_event_hash TEXT NOT NULL,               -- 64 hex chars
    chain_digest      TEXT NOT NULL,               -- 64 hex chars
    mock_anchor_ref   TEXT NOT NULL UNIQUE,        -- "local-mock-anchor-<8 hex>"
    created_at        TEXT NOT NULL                -- RFC3339
);

-- Sequence index: lookup "latest checkpoint for the prefix through
-- seq N" in O(log n) without scanning the table.
CREATE INDEX IF NOT EXISTS idx_audit_checkpoints_sequence
    ON audit_checkpoints(sequence);
-- Chain-digest index: lookup "is this prefix already checkpointed"
-- without table scan.
CREATE INDEX IF NOT EXISTS idx_audit_checkpoints_chain_digest
    ON audit_checkpoints(chain_digest);

COMMIT;
