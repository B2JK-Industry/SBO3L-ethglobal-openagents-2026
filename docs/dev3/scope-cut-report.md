# Dev 3 — honest scope cut report

Items the R14 brief asked for that **partially shipped** or **deferred to
design docs**. Each entry: spec → what shipped → unblock criteria for the
follow-up engineer.

## 1. Storage trait abstraction (incremental dual-write)

**Brief asked for:** "Storage trait abstracted: Sqlite + Postgres backends"
with a unified API across both.

**What shipped:** [#315](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/315)
ships the V020 schema + `crate::pg::PgPool` helper alongside the existing
`Storage` (rusqlite). They are **separate types, not a shared trait**.
Per-backend store impls remain SQLite-only.

**Why deferred:** A unified trait spans 9 store files
(`audit_store`, `audit_checkpoint_store`, `budget_store`, `idempotency_store`,
`mock_kms_store`, `nonce_store`, `policy_store`, `tenant`, `db`). Touching all
nine in one PR would have blown the LoC budget by 5×, and the design doc
([`01-postgres-rls-migration.md`](../production/01-postgres-rls-migration.md))
explicitly recommends **dual-write per store, one PR at a time** through the
soak window.

**Unblock criteria:**
1. Stand up Postgres in staging via `docker compose --profile pg`.
2. Pick the lowest-traffic store first (suggested order:
   `audit_checkpoint_store` → `idempotency_store` → `policy_store` →
   `tenant` → `audit_store` → `budget_store` → `nonce_store`).
3. For each: extract `trait XStore`, impl for both backends, switch caller
   to dyn dispatch via runtime config flag, dual-write for ≥7 days.
4. Drop SQLite path only after 30-day soak with zero drift in the
   row-count reconciliation job.

## 2. `sbo3l pg migrate` CLI subcommand

**Brief asked for:** "Migration tool: sqlx migrate" — implied a CLI
runner.

**What shipped:** [#315](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/315)
ships `PgPool::run_migrations()` (one Rust-callable method) and the
`docker-compose.yml` `sbo3l-pg-migrate` service references a
`sbo3l pg migrate` binary path. **The CLI subcommand itself is not
wired.** Apply V020 manually via `psql` in the meantime.

**Why deferred:** The sbo3l-cli crate doesn't depend on
sbo3l-storage at the workspace level (storage is daemon-only), so wiring
the subcommand requires either (a) adding the dep + feature flag dance
to keep CLI binary size sane, or (b) shipping a separate
`sbo3l-pg-migrate` bin crate. Both reasonable, neither trivial.

**Unblock criteria:**
1. Decide between `sbo3l pg migrate` (subcommand) vs `sbo3l-pg-migrate`
   (separate bin). Prefer subcommand for discoverability.
2. Add `sbo3l-storage = { workspace = true, optional = true,
   features = ["postgres"] }` to `sbo3l-cli/Cargo.toml` behind a
   `postgres` cargo feature.
3. Add `pg::Pg(PgArgs)` to the CLI top-level command enum with a
   `migrate` subcommand that calls `PgPool::connect(...).run_migrations()`.
4. Update the docker-compose `sbo3l-pg-migrate` service to build with
   `--features postgres`.

## 3. Playwright e2e tests (daemon-dependent)

**Brief asked for:** "30+ Playwright e2e tests" against a live admin
console.

**What shipped:** Zero Playwright specs. Existing CI smoke covers Astro
build + Lighthouse + screenshot capture, but no full-flow e2e against
the hosted-app + daemon round-trip.

**Why deferred:** Every interesting e2e (Stripe Checkout button →
redirect, /admin/audit live tail, /t/[slug]/admin/policy/edit Monaco
load, capsule QR scan) requires either:
- A live Stripe test-mode account (Daniel hasn't provisioned)
- A running daemon emitting WebSocket events at /v1/admin/events
- Browser permissions (camera for QR, biometrics) that headless Chromium
  can mock but not exercise honestly

Shipping mock-everything specs would have been theatre — they'd pass
green without exercising the actual contract.

**Unblock criteria:**
1. Provision Stripe test-mode keys in Vercel preview env.
2. Start the daemon under docker-compose with seeded fixtures + a
   `cargo run -p sbo3l-load-test` background driver to keep events flowing.
3. Add `apps/hosted-app/playwright.config.ts` + a `tests/e2e/` directory.
4. Per-page spec: assert page loads, assert visible elements, assert
   network calls fire to expected paths. Don't try to assert UI internals
   — that's brittle.
5. Wire to a new GitHub Actions workflow gated on the daemon being up.

## 4. Algolia DocSearch wiring

**Brief asked for:** "Apply for Algolia DocSearch (free OSS, 24h
turnaround)" + Cmd+K shortcut on apps/docs/.

**What shipped:**
- DocSearch application runbook in
  [`docs/dev3/ALGOLIA-DOCSEARCH-SETUP.md`](./ALGOLIA-DOCSEARCH-SETUP.md)
  (#291)
- Cmd+K shortcut in marketing nav that bounces to the deployed docs site
  where Starlight's built-in **Pagefind** takes over (#318) — covers
  ~80% of DocSearch's value
- The 1-PR wiring template ready to drop in once keys arrive

**Why deferred:** Algolia DocSearch approval is a 1–2 week external
process. The 24h turnaround the brief mentions is best-case; real
historical median is closer to 7–10 days. Daniel hasn't applied yet
(no SBO3L confirmation email visible in the inbox check).

**Unblock criteria:**
1. Apply at https://docsearch.algolia.com/apply with the docs URL
   `https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/docs/`
   (post PR #59 redeploy).
2. Wait for Algolia's `appId` + `apiKey` + `indexName` reply.
3. Set `PUBLIC_ALGOLIA_APP_ID` + `PUBLIC_ALGOLIA_SEARCH_KEY` in Vercel
   env (search-only key is safe to expose client-side).
4. Mount `<DocSearch>` via Starlight component override
   (apps/docs/src/components/StarlightOverrides.tsx already exists for
   the version selector).

## 5. Mobile native (Expo skeleton, no app store submission)

**Brief asked for:** "20+ tests (Jest + Detox or Maestro), DEPLOY.md (how
Daniel submits TestFlight + Internal Track), Standalone .apk + .ipa
builds via eas build (when Daniel runs)."

**What shipped:** [#309](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/309)
— complete Expo skeleton:
- 8 routes (signin / 5 tabs / approval detail / scanner)
- Push notification registration
- Biometric-gated approval flow
- GitHub OAuth via expo-auth-session
- Capsule QR scanner via expo-barcode-scanner
- DEPLOY.md with full TestFlight + Play Internal Track runbook
- 2 Jest unit tests (api error formatting, theme tokens)
- eas.json with preview + production build profiles

What's NOT shipped:
- The 20+ Jest/Detox tests (only 2 unit tests)
- A Detox or Maestro e2e harness
- Any actual `eas build` runs (can't authenticate to Expo from this env)
- App Store Connect / Play Console listings
- TestFlight or Internal Track binaries

**Why deferred:**
- `eas build` requires `expo login` — interactive auth Daniel must run
- TestFlight requires a $99/yr Apple Developer account
- Internal Track requires a $25 Google Play Console account
- Detox / Maestro need a simulator running, neither available in CI
  containers without significant setup

**Unblock criteria:**
1. Daniel runs `pnpm --filter @sbo3l/mobile dlx eas-cli login` once.
2. `pnpm --filter @sbo3l/mobile build:ios` (preview profile, simulator
   build, no Apple Dev account needed) — verifies the build works.
3. When ready for store submission:
   a. Buy Apple Developer ($99/yr) + Google Play ($25 one-time)
   b. Add `ascAppId` + `appleTeamId` to `eas.json` `submit.production`
   c. Drop `play-service-account.json` (from Play Console) into
      apps/mobile/
   d. Run `pnpm --filter @sbo3l/mobile submit:ios` and `:android`
4. For tests: add detox or maestro under `apps/mobile/e2e/`, target
   the iOS simulator + Android emulator in CI via reactivecircus/
   android-emulator-runner.

## What this report achieves

The hackathon submission needs to be **accurate about what works** so
judges don't waste time on features that don't exist. Every item above
shipped *something* — design docs, runbooks, scaffolds — but the
production-grade endpoint is a follow-up. The unblock criteria turn that
follow-up into a defined task rather than open-ended "finish it."
