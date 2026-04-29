# Post-rename code audit

**Subject:** branch `chore/repo-rename-url-update` (PR #59) at HEAD `6f40432` (= main `6ffb5eb` + 1 commit).
**Scope:** read-only verification that the GitHub slug rename
(`mandate-ethglobal-openagents-2026` → `SBO3L-ethglobal-openagents-2026`)
did not break Rust code, schemas, CLI, MCP server, demo runners, or CI.
**Action:** none. Report uncommitted in working tree per brief.

## Build / clippy / test

| Step | Result |
| --- | --- |
| `cargo build --workspace --all-targets` | ✅ green, no new warnings |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ green |
| `cargo test --workspace --all-targets` | ✅ **317 / 317** — matches brief baseline |

## Schema validators

- `python3 scripts/validate_schemas.py` ✅ every static fixture + both runtime artefacts validate
- `python3 scripts/validate_openapi.py` ✅ `docs/api/openapi.json` valid

## Demo runners

- `bash demo-scripts/run-openagents-final.sh` ✅ 13/13 gates green; tagline preserved in summary
- `bash demo-scripts/run-production-shaped-mock.sh` ✅ `Tally: 26 real, 0 mock, 1 skipped`
- `bash demo-scripts/sponsors/uniswap-guarded-swap.sh` ✅ green; `executor_evidence` block prints; capsule `verify` shows `decision: allow`, `executor: uniswap (mode=mock, status=submitted)`
- `python3 demo-fixtures/test_fixtures.py` ✅ all 4 mock fixtures clean + URL self-test pass

## MCP surface

`./target/debug/sbo3l-mcp` + piped `tools/list` JSON-RPC → ✅ server starts, returns 6 tools all carrying the new `sbo3l.*` namespace: `validate_aprp`, `decide`, `run_guarded_execution`, `verify_capsule`, `explain_denial`, `audit_lookup`.

## Schema-id and crate-name greps

`git grep "mandate\.passport_capsule|mandate\.audit_bundle|mandate\.audit_checkpoint"`:

- `test-corpus/passport/tampered_009_executor_evidence_empty_object.json:88,90` — see TG-2.

`git grep "mandate-mcp|mandate-cli|mandate-core|mandate-execution|mandate-policy|mandate-storage|mandate-server|mandate-identity|mandate-keeperhub-adapter"`:

- `docs/spec/12_backlog.md:156` — `pip install mandate-client` (planning-era doc, intentional)
- `test-corpus/audit/chain_v1.jsonl:1` — fixture event `actor:"mandate-server"`. Demo gate 4 still passes (skip-hash verifier doesn't compare actor strings). Cosmetic only — see TG-4.

Crate `[package].name` and `[[bin]].name` values are all `sbo3l-*` / `sbo3l` / `sbo3l-mcp` / `sbo3l-server` / `research-agent`. Clean.

## URL-string greps

`git grep "mandate-ethglobal-openagents-2026" -- '*.rs' '*.toml'` → **0 hits.**

Whole-tree:
- `docs/spec/15_…`, `30_…`, `31_…`, `32_…`, `33_…` — planning-era references, intentional historical (README discloses).
- **`site/index.html:89`** — see TG-1.

## Cargo.toml repository fields

```
Cargo.toml                                       → SBO3L-ethglobal-openagents-2026 ✅
crates/sbo3l-keeperhub-adapter/Cargo.toml        → SBO3L-ethglobal-openagents-2026 ✅
```

## Standalone adapter crate publish dry-run

`cargo publish --dry-run --allow-dirty -p sbo3l-keeperhub-adapter` ❌

```
no matching package named `sbo3l-core` found
location searched: crates.io index
```

⚠️ Same documented blocker shipped with the IP-4 PR (#56): `sbo3l-core` is not on crates.io. **NOT a regression caused by the rename** — adapter `repository` field and README URLs all point at the new slug. The dry-run blocker is upstream of the rename.

## Truthfulness gaps

### TG-1 (medium) — `site/index.html:89` displays the old GitHub slug

Public Pages site renders: `Source: B2JK-Industry/mandate-ethglobal-openagents-2026 on GitHub`. PR #59's sweep missed this string. The old slug 301-redirects, but a judge clicking the source link sees the redirect. Recommend update to the new slug.

### TG-2 (medium) — `tampered_009` has stale schema-ids that mask its stated test purpose

`test-corpus/passport/tampered_009_executor_evidence_empty_object.json` carries `receipt_type: "mandate.policy_receipt.v1"` (L55), `bundle_ref: "mandate.audit_bundle.v1"` (L88, unconstrained — passes), and `checkpoint.schema: "mandate.audit_checkpoint.v1"` (L90). The first and third violate `const` constraints in the receipt + capsule schemas. `crates/sbo3l-core/src/passport.rs:491` asserts `err.code() == "capsule.schema_invalid"` — passes today, but the schema fails on L55/L90 **first**, not on the empty `executor_evidence` object the fixture name claims to test. Update the three strings to their `sbo3l.*` equivalents.

### TG-3 (low / cosmetic) — migration-file comments reference the old `mandate` CLI binary name

- `migrations/V005__mock_kms_keys.sql:3,8` → `mandate key {init,list,rotate}`
- `migrations/V006__active_policy.sql:3,16,39` → `mandate policy {…}`
- `migrations/V007__audit_checkpoints.sql:4` → `mandate audit checkpoint {…}`

Actual binary is `sbo3l`. SQL comments are non-functional but a reader following them gets the wrong command.

### TG-4 (low / cosmetic) — `test-corpus/audit/chain_v1.jsonl` fixture has `actor: "mandate-server"`

Current server emits `actor: "policy_engine"` (`crates/sbo3l-server/src/lib.rs:473`). Demo gate 4 still passes — the structural verifier doesn't compare actor strings. Cosmetic only.

### Items deliberately NOT flagged

- The product **tagline** "Don't give your agent a wallet. Give it a mandate." (README, demo scripts, fixtures, HTML) — intentional brand-pitch line preserved by the rebrand.
- `docs/spec/*` references to `mandate-ethglobal-openagents-2026` — README discloses these as copied from a pre-hackathon planning repository.
- Common-noun phrases like "spending mandates for autonomous agents", "spending-mandate firewall", "the agent's mandate" — intentional product-pitch usage.

## Headline summary

Build / clippy / 317 tests / schema validators / both demo runners / MCP smoke / fixtures validator all green. **Two real truthfulness gaps in user-visible surfaces:** Pages site shows the old slug (TG-1), and `tampered_009` has stale schema-ids that mask its intended test failure mode (TG-2). Two cosmetic low-severity gaps (TG-3 migration comments, TG-4 fixture actor) are non-functional. Cargo-publish dry-run on the adapter crate fails — same documented blocker as PR #56 (`sbo3l-core` not on crates.io), **not** a rename regression.

**Branch hygiene flag:** started on `feat/dev-a-positioning-polish`; `git branch --show-current` returned `chore/repo-rename-url-update` when audit began (recurring branch-hijack). Stashed positioning-polish WIP cleanly; audit ran on PR #59 — which the brief specified as a valid subject. This file uncommitted on `chore/repo-rename-url-update` per the brief.
