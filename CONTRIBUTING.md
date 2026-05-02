# Contributing to SBO3L

Thanks for considering a contribution. This doc covers what to expect.

## TL;DR

1. Read [`SECURITY.md`](SECURITY.md) before reporting a vulnerability — do **NOT** open public issues for security findings.
2. For features / bugs / docs: open a GitHub issue first to discuss; PRs without prior discussion may be closed.
3. PRs must pass all CI checks (regression-on-main, supply-chain, proptest, fuzz, mutation testing, benchmarks).
4. Sign your commits (recommended) and add a `Co-Authored-By` trailer if AI-assisted.

## Repo layout

| Path | What |
|---|---|
| `crates/sbo3l-{core,storage,policy,identity,execution,server,mcp,cli,...}/` | Rust workspace |
| `sdks/typescript/` | `@sbo3l/sdk` + per-framework `@sbo3l/*` integrations |
| `sdks/python/` | `sbo3l-sdk` + `sbo3l-{langchain,crewai,llamaindex,langgraph}` |
| `apps/{marketing,hosted-app,trust-dns-viz,observability,mobile}/` | User-facing surfaces |
| `contracts/` | Solidity (AnchorRegistry, OffchainResolver, ReputationBond, SubnameAuction, ReputationRegistry) |
| `fuzz/` | cargo-fuzz harnesses (5 targets) |
| `benchmarks/competitive/` | Criterion benchmark suite |
| `scripts/` | Operator scripts (run-competitive-benchmarks, build-wasm-verifier, etc.) |
| `docs/` | Submission package, compliance, security, win-backlog (Phase 1/2/3 ACs) |

## Local dev

### Rust

```bash
cargo check --workspace
cargo test --workspace --tests --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

### TypeScript SDK

```bash
cd sdks/typescript
npm install --no-audit --no-fund
npm run build
npm test
```

### Python SDK

```bash
cd sdks/python
python -m pip install -e ".[dev]"
ruff check .
ruff format --check .
mypy --strict sbo3l_sdk
pytest -q
```

### Marketing site

```bash
cd apps/marketing
npm install
npm run dev   # http://localhost:4321
```

### Daemon

```bash
cargo run --bin sbo3l-server
# Default listens on 127.0.0.1:18731
# Set SBO3L_ALLOW_UNAUTHENTICATED=1 to skip auth (DEV ONLY — banner-warned)
```

## PR conventions

### Commit messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): one-line summary

Optional body explaining the why.

Fixes #123
Co-Authored-By: AI Assistant Name <noreply@example.com>
```

Common types: `feat`, `fix`, `docs`, `test`, `perf`, `refactor`, `chore`, `ci`, `build`.

Common scopes: `core`, `policy`, `server`, `cli`, `identity`, `execution`, `marketing`, `hosted-app`, `submission`, `compliance`, `security`, `bench`, `fuzz`.

### Branch naming

```
agent/<name>/<task-id>           # e.g. agent/dev1/T-3-1
agent/qa/<task-name>             # QA agent (Heidi)
fix/<short-desc>                 # Hotfixes
docs/<short-desc>                # Doc-only changes
chore/<short-desc>
```

### PR scope

- Keep PRs small and reviewable. Default target: < 500 LoC change.
- One logical concern per PR. Don't bundle a refactor + new feature.
- If the PR closes a Phase 1/2/3 ticket, link it: `Closes T-3-5`.

### CI checks

Every PR runs:

- `regression-on-main.yml` — full Rust workspace test + clippy + fmt
- `supply-chain.yml` — cargo-audit + npm-audit + SBOM + gitleaks
- `proptest.yml` — 4 invariants × 256 cases
- `fuzz.yml` — _scheduled only_ (nightly cron)
- `mutation-testing.yml` — _scheduled only_ (weekly cron)
- `multi-framework-smoke.yml` — Docker compose end-to-end
- `lighthouse.yml` — marketing site performance + a11y
- `codex-review.yml` — automated PR review with severity tags

PRs cannot merge if any required check is red. Branch protection on `main` enforces this.

### Security

If your change touches:
- Auth (`crates/sbo3l-server/src/auth.rs`)
- Signing (`crates/sbo3l-identity/`)
- Capsule format (`crates/sbo3l-core/src/passport.rs`)
- Audit chain (`crates/sbo3l-core/src/audit.rs`)
- Policy enforcement (`crates/sbo3l-policy/`)

…then ping a maintainer for an extra security review. These touch the trust boundary.

## Reporting a security issue

**Do NOT open a public GitHub issue.**

Use the channels in [`SECURITY.md`](SECURITY.md):
- GitHub Security Advisory (preferred; encrypted)
- `security@sbo3l.dev` email
- HackerOne (when live; see `docs/security/bounty-platform-integration.md`)
- Immunefi for crypto bugs (when live)

We pay bounties: $1K-5K Critical / $250-1K High / $50-250 Medium / swag Low.

## Code of Conduct

This project follows the [Contributor Covenant 2.1](CODE_OF_CONDUCT.md). Report
violations to `conduct@sbo3l.dev`.

## License

Apache-2.0. By contributing, you agree your contributions are licensed under
the same terms.

## Thanks

If your contribution surfaces a security issue, you're added to the Hall of
Fame in [`SECURITY.md`](SECURITY.md).

If your contribution lands a feature, we cite you in the release notes
([`CHANGELOG.md`](CHANGELOG.md)) and (if you're OK with it) the GitHub Release
page.
