# `mandate doctor`

> Operator readiness summary. Honest about scope: every check is `ok`, `skip`, `warn`, or `fail` — never silently `ok` on a missing feature.

## Quick start

```bash
# Run against an in-memory fresh DB (verifies the binary itself works):
mandate doctor

# Run against a daemon's SQLite store:
mandate doctor --db /var/lib/mandate/mandate.sqlite

# Machine-readable, stable schema (production-shaped runner consumes this):
mandate doctor --db /var/lib/mandate/mandate.sqlite --json
```

## What it checks

| Row | Meaning |
|---|---|
| `migrations` | `schema_migrations` populated; lists `V<n>:<description>` for every applied migration. |
| `nonce_replay` | V002 table present + row count. (Implemented today.) |
| `idempotency_keys` | V004 table present + row count. **Skip** if the migration hasn't run — reason references **PSM-A2** so an operator knows what to enable. |
| `audit_chain` | Count + structural verify (`prev_event_hash` linkage and `event_hash` recompute). Signatures are **not** verified — the doctor doesn't have access to the daemon's signer pubkey. **Skip** if the chain is empty. |
| `mock_kms_keys` | **Skip** today — Mock KMS keyring persistence is tracked as **PSM-A1.9**. The in-process `MockKmsSigner` (PSM-A1) still works without persistence. |
| `active_policy` | **Skip** today — policy lifecycle is **PSM-A3**. The daemon currently uses the embedded reference policy. |
| `payment_requests` | Core V001 table present + row count. |

## Status semantics

- **`ok`** — feature implemented and DB backs it. Detail explains the row count or migration list.
- **`skip`** — feature is **not implemented in this build** OR the optional table hasn't been migrated yet. Skip is **never** a fake `ok`. Skip rows always include a reason that references the backlog item that would promote them (PSM-A2, PSM-A1.9, PSM-A3).
- **`warn`** — implemented but the current state is anomalous. Reserved for future heuristics.
- **`fail`** — implemented and the integrity check failed (e.g. audit chain present but `prev_event_hash` linkage is broken). The doctor exits non-zero in this case.

## Aggregate verdict

The `overall` field at the top of the report is:

- `ok` — every row is `ok` or `skip`.
- `warn` — at least one `warn`, no `fail`.
- `fail` — at least one `fail`. The doctor exits with code `1`.

Code `2` is reserved for "DB itself could not be opened" (different error class — DB-level, not check-level).

## JSON envelope

The JSON shape is **stable** under `report_type: "mandate.doctor.v1"`. Consumers (especially the production-shaped runner) can rely on:

```json
{
  "report_type": "mandate.doctor.v1",
  "overall":     "ok|warn|fail",
  "db_path":     "...",
  "checks": [
    { "name": "migrations",       "status": "ok",   "detail": "V001:init, V002:nonce_replay" },
    { "name": "nonce_replay",     "status": "ok",   "detail": "table present, rows=0" },
    { "name": "idempotency_keys", "status": "skip", "reason": "table not present — ... PSM-A2 ..." },
    { "name": "audit_chain",      "status": "skip", "reason": "no audit events yet — ..." },
    ...
  ]
}
```

`status` is the discriminator. `ok` and `warn` rows carry `detail`; `skip` rows carry `reason`; `fail` rows carry `error`.

## What the doctor does NOT check

- **Live network reachability.** The doctor is offline by design.
- **KMS / HSM connectivity.** Mock KMS keyring is local; production KMS would be a separate diagnostic.
- **Sponsor credentials.** ENS / KeeperHub / Uniswap connectivity is not part of the doctor's scope.
- **Signer keypair correctness.** Audit chain verification skips signatures (it doesn't have the pubkey to verify against). Wire that up if you persist the daemon's signing pubkey somewhere the doctor can read.

These omissions are deliberate — the doctor is for "is the local Mandate state internally consistent and migrations-current?", not "is every external dependency live?"

## Truthfulness rules (for the people who read every doctor report)

1. Skip means **not implemented yet**, never silently passing.
2. Skip rows reference the backlog item that would promote them — so an operator can plan the upgrade path.
3. The `overall` verdict deliberately treats `skip` as `ok` for the aggregate — a fresh production-shaped mock build is supposed to have skips, and we don't want to fail-loud on expected absences.
4. The doctor never claims production readiness. The output makes it obvious which features are mock and which are absent.
