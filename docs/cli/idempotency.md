# `Idempotency-Key` safe-retry

> Production-shaped, persistent, deterministic. RFC 8470-spirit, not a literal RFC 8470 implementation.

`POST /v1/payment-requests` accepts an `Idempotency-Key` header (16–64 ASCII chars, declared in `docs/api/openapi.json` since the project's OpenAPI was first published). Mandate's daemon turns this into:

- **Same key + same canonical request body** → return the original 200 OK response, byte-identical, **without re-running policy / budget / audit / signing**.
- **Same key + different canonical request body** → HTTP 409 `protocol.idempotency_conflict`.
- **No key** → today's behaviour, including HTTP 409 `protocol.nonce_replay` on a reused nonce.

Backed by SQLite migration **V004 `idempotency_keys`**, so the cached response envelope survives a daemon restart against the same database file.

## Behaviour matrix

| Request 1 | Request 2 | Outcome |
|---|---|---|
| `Idempotency-Key: K1`, body B1, **success (200)** | `Idempotency-Key: K1`, body B1 | Cached envelope replayed, byte-identical, no new audit row |
| `Idempotency-Key: K1`, body B1 | `Idempotency-Key: K1`, body B2 | 409 `protocol.idempotency_conflict` |
| `Idempotency-Key: K1`, body B1 (file db) | (drop daemon) `Idempotency-Key: K1`, body B1 | Cached envelope replayed across restart |
| `Idempotency-Key: K1`, body B1 (success) | `Idempotency-Key: K2`, body B1 | 409 `protocol.nonce_replay` (defence in depth) |
| No key, body B1 (success) | No key, body B1 | 409 `protocol.nonce_replay` (legacy behaviour preserved) |
| Malformed key (too short / too long / non-ASCII) | — | 400 `protocol.idempotency_key_invalid` |

The cached response carries the original `audit_event_id`, `request_hash`, `policy_hash`, signed receipt — every byte the original 200 carried. A consumer can verify the cached receipt with the same key it used originally.

## Ordering vs nonce replay

When a request arrives:

1. **Idempotency lookup runs FIRST**, before schema validation, before the nonce gate, before policy / budget / audit / signing.
   - Cache hit + matching canonical body → return cached envelope immediately. The pipeline never executes.
   - Cache hit + different canonical body → 409 `protocol.idempotency_conflict`. The pipeline never executes.
2. **Cache miss** → fall through to the existing pipeline:
   schema validate → request_hash → **nonce gate (V002)** → policy → budget → audit → signed receipt.
3. On a successful 200, the response envelope is written to the cache **after** signing, so a daemon-side error during signing/storage doesn't poison the cache. The only failure-of-failure is "we sign and return 200 but fail to write the cache row" — in that case the next retry just re-runs the pipeline, which fails fast at the nonce gate (409 `protocol.nonce_replay`). Worst case: visible failure, never silent acceptance.

The nonce gate stays **strictly upstream of all signing side effects**. Idempotency layers on top.

## Caching policy

Only **200 OK** responses are cached. 4xx and 5xx responses are not stored:

- **5xx**: probably transient. The client retries through the full pipeline; the nonce gate may or may not have already consumed the nonce, depending on where the 5xx originated. This is the existing nonce semantics.
- **4xx**: deterministic. Re-running the pipeline with the same body yields the same 4xx, so caching it would only avoid a small amount of compute — not enough to justify the cache-row growth.

A future iteration could cache 4xx responses too (RFC 8470 allows it). Out of scope here.

## Header validation

`Idempotency-Key` must be ASCII, 16-64 chars (matches the `IdempotencyKey` parameter in `docs/api/openapi.json`). Out-of-range or non-ASCII values surface as 400 `protocol.idempotency_key_invalid`. Empty / missing header is allowed and bypasses the idempotency layer entirely.

## Persistence

Backed by SQLite (migration `V004__idempotency_keys.sql`):

```sql
CREATE TABLE idempotency_keys (
    key             TEXT PRIMARY KEY,
    request_hash    TEXT NOT NULL,
    response_status INTEGER NOT NULL,
    response_body   TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
```

`PRIMARY KEY (key)` gives atomic INSERT-or-fail semantics. Two concurrent winners with the same key both attempt INSERT; exactly one stores the canonical envelope, the loser silently no-ops. No race-window write-then-overwrite.

## Limitations / known follow-ups

- **No TTL eviction.** The table grows monotonically. APRP requests have an `expires_at`; a future migration can `DELETE WHERE created_at < now() - <window>` without changing semantics.
- **200-only caching.** 4xx responses re-run the pipeline on retry (they re-fail deterministically; the cost is minor).
- **Defence in depth, not replacement for the nonce gate.** A successful retry under K1 returns the cached response. A different K2 with the same nonce still hits the nonce gate → 409 `protocol.nonce_replay`. The two protections are layered, not interchangeable.
- **No support for `request_hash`-based fingerprinting under different keys.** If the client sends K1 + body B1 (success), then K2 + body B1 (different key, same body, fresh nonce), Mandate will run the pipeline and append a new audit row — that's the intended semantics; the agent has explicitly declared two distinct logical operations by giving them two distinct keys.
- **No deduplication on a 5xx-then-retry race.** RFC 8470's full safe-retry model would lock the key during pipeline execution and serialise retries against it; we don't yet. Concurrent first-attempts under the same key produce one cache winner; subsequent retries see the cached response.
- **The daemon (`mandate-server`) accepts the header on the demo path, but the existing 13-step `run-openagents-final.sh` does not exercise it — it remains a server feature. A B-owned demo step can promote PSM-A2 from SKIP in the production-shaped runner.
