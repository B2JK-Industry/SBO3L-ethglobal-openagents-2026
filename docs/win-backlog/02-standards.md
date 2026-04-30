# Development + QA + PR Standards

> Every developer follows these standards. Heidi blocks merge for violations.

## Code quality bars

### Rust crates
- `cargo fmt --check` clean (rustfmt.toml is canonical)
- `cargo clippy --workspace --all-targets -- -D warnings` clean (no warnings, ever)
- `cargo test --workspace --all-targets` green (377/377+ baseline)
- `cargo audit --deny warnings` clean
- `cargo doc --no-deps --workspace` builds without warnings
- Public items have `///` doc comments
- `unsafe` requires inline justification comment
- Library crates: no `panic!`, `unwrap()`, or `expect()` outside `#[cfg(test)]` (use `Result` + domain errors)
- Binary crates: `panic!` allowed only at `main()` level for unrecoverable startup errors
- All errors have a domain code (e.g. `auth.invalid_token`, `policy.budget_exceeded`)
- Newtype wrappers preferred over `String` / `u64` for domain types (e.g. `AgentId(String)`, not raw `String`)

### TypeScript SDKs / examples
- 100% TypeScript type coverage (no `any`, no `// @ts-ignore` without justification)
- `tsc --noEmit` clean
- `eslint .` clean (config: airbnb-base + @typescript-eslint/strict)
- `prettier --check .` clean
- Public API: every exported function/type has JSDoc
- Peer deps not bundled (don't bundle `node-fetch` or polyfills)
- Tree-shakeable (named exports only, no default re-exports)

### Python SDKs / examples
- `ruff check` clean (config: ruff.toml at repo root)
- `ruff format` clean
- `mypy --strict` clean
- Pydantic v2 strict mode for all data models
- Async-first: every public function has async variant; sync wrappers via `asyncio.run()`
- Type hints everywhere (no untyped function signatures)

### Schemas / OpenAPI
- `python3 scripts/validate_schemas.py` green
- `python3 scripts/validate_openapi.py` green
- Every breaking schema change is a major version bump (`sbo3l.passport_capsule.v2`, not silent v1 mutation)
- `additionalProperties: false` (deny unknown fields) end-to-end
- Required fields explicit (no implicit defaults except where documented)

### Documentation
- Every public claim has a code reference (file:line) or test name
- Every code block runnable as-shown
- Audience + outcome stated at top of each doc
- No jargon without first-use definition or link
- All paths repo-relative (e.g. `crates/sbo3l-server/src/lib.rs`, not `/Users/...`)

### Frontend / visualization
- Lighthouse perf score > 90
- WCAG AA compliance (contrast, aria, keyboard nav)
- No external font / CSS / script CDNs (offline-verifiable surface)
- JS bundle < 200 KB gzipped per page
- No third-party analytics (privacy)

## Branch + commit + PR rules

### Branch naming
```
agent/<your-name>/<ticket-id>
agent/alice/F-1
agent/bob/T-3-2
```

Daniel manual branches: `chore/`, `fix/`, `docs/`, `feat/` prefixes.

### Commit format (conventional commits, mandatory)
```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:** `feat` | `fix` | `docs` | `chore` | `test` | `refactor` | `perf` | `ci` | `build`
**Scope:** crate name, `cli`, `mcp`, `docs`, `ci`, etc.
**Subject:** imperative mood, no trailing period, < 72 chars
**Body:** what + why, wrap at 72 chars
**Footer:** `Co-Authored-By: <agent name> <agent-email>` for AI agents

Example:
```
feat(server): real auth middleware (bearer + JWT)

Adds Authorization header validation. Bearer tokens hashed with bcrypt;
JWT validated with claim sub == APRP.agent_id. Default-deny unless
SBO3L_ALLOW_UNAUTHENTICATED=1 dev flag.

Closes F-1.

Co-Authored-By: Alice (Rust core agent) <alice@sbo3l.dev>
```

### PR rules
- One ticket = one PR. No bundling.
- PR title = ticket title verbatim (e.g. "feat(server): real auth middleware (bearer + JWT)")
- PR body sections (in order):
  - `## Summary` — 1-3 bullets of what changed
  - `## Why` — 1-2 sentences linking to ticket motivation
  - `## Test plan` — checklist matching ticket's QA test plan
  - `## Out of scope` — explicit list of what was NOT touched
- Linked ticket: `Closes <ticket-id>` in body
- Size cap: 500 LoC per PR (excluding generated files, tests). Over → split into prep PR + main PR.
- Reviewers: `@daniel` + `@heidi` (both required)
- Merge strategy: squash-merge (history clean on main)
- Branch protection: CI must be green, 2 approvals, branch up-to-date with main

### Reviews
- Daniel: scope + design + security + integrity
- Heidi: acceptance criteria + test plan + regression
- Both must approve. Either can request changes.
- Reviewer turnaround: same-day for PRs opened before 18:00, next-day otherwise
- If review takes > 24h, ping in coordination channel

## Testing standards

### Unit tests
- Same crate as code under test
- Run via `cargo test --lib`
- Fast (< 1ms per test ideal)
- Cover happy path + 2+ edge cases per public function
- Property-based testing for parsers / serializers / hashers (use `proptest` in Rust, `hypothesis` in Python)

### Integration tests
- `tests/` directory in each crate
- Run via `cargo test --test <name>`
- Each test sets up own SQLite DB (use `:memory:` or `tempfile::TempDir`)
- No shared state between tests
- Test names: `test_<feature>_<scenario>` (e.g. `test_auth_jwt_agent_id_mismatch`)

### E2E tests
- `demo-scripts/` and `demo-scripts/sponsors/`
- Run via `bash demo-scripts/<name>.sh`
- Must be deterministic (set `SBO3L_DETERMINISTIC=1` if needed)
- Output captured to `demo-scripts/artifacts/` for fixture comparison

### Regression tests
- `python3 demo-fixtures/test_fixtures.py` — fixture validation
- `python3 trust-badge/test_build.py` — proof viewer
- `python3 operator-console/test_build.py` — operator console
- All gated in CI as of P0d (#79)

### Fuzz tests
- `cargo fuzz` for parsers (APRP, audit events, capsules)
- Run for at least 5 minutes during CI on main
- Crash-corpus checked in to `fuzz/corpus/`

### Mutation tests
- `cargo mutants` for critical paths (policy decision, signature verification, hash chain)
- Survival rate < 5% target (fewer surviving mutants = better)
- Run weekly, not per-PR (slow)

## Test plan format (for tickets)

Every ticket has a "QA Test Plan" section with:
1. **Setup:** any env vars, DB resets, daemon starts
2. **Test commands:** literal bash blocks, copy-paste runnable
3. **Expected output:** exact strings or regex patterns
4. **Cleanup:** if any (kill processes, rm files)

Heidi runs these literally. If a command fails, ticket is rejected back to author.

## Security standards

### Secrets
- Never commit secrets to repo. Pre-commit hook runs `git-secrets` + custom regex (`wfb_[A-Za-z0-9]{20,}`, `kh_[A-Za-z0-9]{20,}`, AWS keys, etc.)
- Real secrets only in `.env` (gitignored), GitHub Secrets, or cloud KMS
- Doc examples use placeholder tokens like `wfb_REPLACE_WITH_YOUR_TOKEN`

### Auth
- All `POST /v1/payment-requests` require `Authorization` header (Phase 1 F-1)
- Default-deny if no auth; dev mode behind explicit env var
- Errors don't leak sensitive info (return generic `auth.required` not `token expired at 2026-04-30T18:00:00Z`)

### Logging
- No tokens, no raw signatures, no private keys in logs
- API responses redacted in test logs (Heidi grep-checks)
- Use `tracing` crate with structured fields; redact sensitive fields explicitly

### Dependencies
- `cargo audit` clean before merge
- `npm audit --production` clean (TS SDKs)
- `pip-audit` clean (Python SDKs)
- Pin major versions in `Cargo.toml` / `package.json` / `pyproject.toml`
- Re-pin every 90 days (security update sweep)

## Performance standards

### Rust core
- `POST /v1/payment-requests` p99 < 50ms on commodity hardware
- Daemon cold-start < 2s
- SQLite migrations idempotent + fast (< 100ms each)
- Audit chain verify: O(n) over chain length, no n² blowup

### Capsule operations
- `passport run`: < 100ms (excludes daemon RTT)
- `passport verify` (structural): < 10ms
- `passport verify --strict`: < 100ms with embedded fields, < 200ms with aux files

### Frontend
- Time to first paint < 1.5s (Lighthouse)
- Total Blocking Time < 200ms
- Trust DNS visualization: 60fps with 100 agents on screen

## Observability standards (Phase 3 hosted version)

- Every server endpoint emits OpenTelemetry trace
- Logs structured JSON via `tracing-subscriber`
- Metrics: `sbo3l_request_total{decision, deny_code}`, `sbo3l_audit_chain_length`, `sbo3l_capsule_emit_duration_seconds`
- Health check: `GET /health` returns `200 OK` with version + DB status

## Definition of done (per ticket)

A ticket is **DONE** when:
- [ ] All acceptance criteria checked
- [ ] PR opened with proper title + body
- [ ] CI green (Rust + schemas + Python regression + linting)
- [ ] Heidi has run QA Test Plan literally and reported PASS
- [ ] Daniel has reviewed and approved
- [ ] Squash-merged to main
- [ ] Linear ticket moved to "Done" with PR link
- [ ] If ticket touches public API: changelog entry added

A ticket is **NOT DONE** if any of the above is missing, even if the code "looks fine".

## Conflict resolution

If two agents touch the same file:
1. First-merged wins; second agent rebases
2. If rebase conflicts non-trivial, second agent posts in coordination channel
3. Daniel arbitrates (decides whose change keeps semantic, whose adapts)
4. Never force-push to shared branches

If two agents disagree on architecture:
1. Post both proposals in coordination channel as comments on the contested ticket
2. Daniel decides within 24h
3. Loser closes their PR (if any), follows winning approach
4. No silent overrides

## Daily rhythm

| Time | Activity |
|---|---|
| 09:00 | Async standup post in coordination channel: yesterday / today / blockers |
| 09:00-18:00 | Work on assigned ticket. Open PRs as ready. |
| 18:00 | Daniel reviews PRs, merges greens |
| 18:00-21:00 | Authors address review feedback if any |
| 21:00 | Heidi runs end-of-day regression sweep on main |
| 21:30 | Heidi posts regression report; flags any new regressions |

## Escalation paths

| Issue | Where to post | SLA |
|---|---|---|
| Blocker (dependency unmet) | Coordination channel, tag @daniel | Same-day response |
| Architectural dispute | Coordination channel, tag @daniel + opposing agent | 24h decision |
| Security incident (secret leak, vulnerability found) | Direct DM @daniel + coordination channel | Immediate |
| Heidi rejects PR | Coordination channel, fix forward | Within current day |
| CI flake | Issue against `B2JK-Industry/SBO3L-...` | Async; not blocker if reproducible-only |

## Rule of last resort

> If you're not sure whether to do something, **don't do it.** Ask in coordination channel.

Better to wait 4 hours for clarification than to ship something off-mission.
