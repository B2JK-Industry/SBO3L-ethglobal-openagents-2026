-- Mandate initial schema. See docs/spec/17_interface_contracts.md §6.

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version       INTEGER PRIMARY KEY,
    description   TEXT NOT NULL,
    applied_at    TEXT NOT NULL,
    sha256        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_events (
    seq                 INTEGER PRIMARY KEY,
    id                  TEXT NOT NULL UNIQUE,
    ts                  TEXT NOT NULL,
    type                TEXT NOT NULL,
    actor               TEXT NOT NULL,
    subject_id          TEXT NOT NULL,
    payload_hash        TEXT NOT NULL,
    metadata_json       TEXT NOT NULL,
    policy_version      INTEGER,
    policy_hash         TEXT,
    attestation_ref     TEXT,
    prev_event_hash     TEXT NOT NULL,
    event_hash          TEXT NOT NULL,
    signature_alg       TEXT NOT NULL,
    signature_key_id    TEXT NOT NULL,
    signature_hex       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_events_ts ON audit_events(ts);
CREATE INDEX IF NOT EXISTS idx_audit_events_subject ON audit_events(subject_id);

CREATE TABLE IF NOT EXISTS payment_requests (
    id                  TEXT PRIMARY KEY,
    agent_id            TEXT NOT NULL,
    request_hash        TEXT NOT NULL UNIQUE,
    raw_json            TEXT NOT NULL,
    decision            TEXT NOT NULL,
    deny_code           TEXT,
    policy_version      INTEGER,
    policy_hash         TEXT,
    audit_event_id      TEXT NOT NULL,
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_payment_requests_agent ON payment_requests(agent_id);
CREATE INDEX IF NOT EXISTS idx_payment_requests_decision ON payment_requests(decision);

CREATE TABLE IF NOT EXISTS budget_committed (
    agent_id        TEXT NOT NULL,
    scope           TEXT NOT NULL,
    bucket_key      TEXT NOT NULL,
    spent_usd       TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (agent_id, scope, bucket_key)
);

COMMIT;
