-- Mandate V005: persistent **mock** KMS keyring metadata.
--
-- Backs `Storage::mock_kms_*` and the `mandate key {init,list,rotate} --mock`
-- CLI surface (PSM-A1.9). Holds *only* public-key material — no seeds,
-- no private keys. Callers supply the deterministic `--root-seed` to the
-- CLI on every operation; this table stores the resulting per-version
-- public key so:
--   * `mandate key list` can dump the keyring without re-deriving;
--   * the doctor can confirm a keyring exists and report its size;
--   * a future B-owned demo step can show the keyring on the production-
--     shaped runner without having to run a Rust binary.
--
-- This is mock infrastructure. A real KMS keyring would live behind a
-- key-management API; the per-version row would carry an opaque KMS key
-- ARN/handle, NOT a deterministic public key derived from a local seed.
-- Documented in `docs/cli/mock-kms.md`.

BEGIN;

CREATE TABLE IF NOT EXISTS mock_kms_keys (
    role        TEXT NOT NULL,
    version     INTEGER NOT NULL,
    key_id      TEXT NOT NULL UNIQUE,
    public_hex  TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    PRIMARY KEY (role, version)
);

CREATE INDEX IF NOT EXISTS idx_mock_kms_keys_role ON mock_kms_keys(role);
CREATE INDEX IF NOT EXISTS idx_mock_kms_keys_created_at ON mock_kms_keys(created_at);

COMMIT;
