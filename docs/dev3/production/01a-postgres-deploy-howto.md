# Postgres backend — deploy how-to

Companion to [`01-postgres-rls-migration.md`](./01-postgres-rls-migration.md)
(the design doc) and the implementation under
[`crates/sbo3l-storage/src/pg.rs`](../../../crates/sbo3l-storage/src/pg.rs) +
[`migrations/V020__postgres_init.sql`](../../../crates/sbo3l-storage/migrations/V020__postgres_init.sql).

## Local — docker-compose

```sh
docker compose --profile pg up postgres -d
docker compose --profile pg run --rm sbo3l-pg-migrate
```

The `pg` profile spins up Postgres 16 + applies V020 (schema + RLS policies).
SQLite remains the default backend; Postgres only activates when the daemon
is built with `--features postgres` and `DATABASE_URL` is set.

## Daemon build with Postgres feature

```sh
cargo build -p sbo3l-server --features sbo3l-storage/postgres
```

## Env vars (Vercel / Fly.io / Railway)

| Var | Required | Default | Notes |
|---|---|---|---|
| `DATABASE_URL` | yes | — | `postgres://user:pass@host:5432/db` |
| `DATABASE_MAX_CONNECTIONS` | no | 20 | sqlx pool ceiling |

## Tenant isolation contract

The daemon **must** open every per-tenant query inside a transaction
acquired via `PgPool::tenant_tx(uuid)`:

```rust
let mut tx = pool.tenant_tx(tenant.uuid).await?;
// ... per-tenant queries here ...
tx.commit().await?;
```

`tenant_tx` issues `SET LOCAL app.tenant_uuid = '<uuid>'` inside the
transaction. RLS policies on `agents`, `audit_events`, `capsules` filter
to that GUC value. The `SET LOCAL` is reset on commit/rollback — abandoned
transactions can leak the GUC across pool acquisitions, so production
deploys MUST set `idle_in_transaction_session_timeout = '30s'` on the
Postgres role.

Admin-scope queries (tenants table CRUD, memberships) use `admin_tx()`
which skips the GUC. Never use `admin_tx()` for per-tenant data.

## Recommended hosts

- **Supabase** (free tier) — for hackathon → early-pilot. RLS + connection
  pooling out of the box. Pause after 7 days idle.
- **Neon** — branching makes preview deploys clean; free tier ~512MB.
- **Fly.io Postgres** — when daemon already on Fly. ~$3/mo for 1GB
  shared-cpu-1x.
- **RDS / Cloud SQL** — for production scale; multi-AZ + PITR.

## Rollback

V020 is idempotent on re-run — safe to apply repeatedly. To roll back:

```sql
DROP TABLE capsules, audit_events, agents, memberships, tenants, stripe_events CASCADE;
DROP FUNCTION set_updated_at();
```

The dual-write window (per the design doc) keeps SQLite as source of truth
during cutover, so dropping Postgres is non-destructive until cutover day.

## What this PR does NOT do

- The full `Storage` trait abstraction across SQLite + Postgres backends
  (incremental — one store per follow-up PR per the design doc's
  dual-write phase).
- The `sbo3l pg migrate` CLI subcommand referenced by the docker-compose
  `sbo3l-pg-migrate` service — that ships in a follow-up. Until then,
  apply `V020__postgres_init.sql` manually via `psql`.
- testcontainers integration tests — sqlx's offline mode + the existing
  unit tests cover the Rust side; live Postgres tests require a CI runner
  with a Postgres service container (see `.github/workflows/ci.yml` `pg`
  job, also a follow-up).
