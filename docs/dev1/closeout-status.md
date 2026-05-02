# Dev 1 — R14 closeout status

**Authored:** 2026-05-02T16:28Z
**Branch baseline:** `origin/main` at `0ec8b15` (post #328 trust-badge fix)
**Audience:** Daniel + cascade driver. One-line truth per open PR; honest scope-cut log lives in `scope-cuts-r13-r14.md`.

## R14 PRs by state

| PR | Title | State | CI | Action |
|---|---|---|---|---|
| #322 | gRPC API alongside REST (R14 P1) | OPEN, **CONFLICTING** vs main, 5 failed checks | `Rust check` + `docker compose orchestrator e2e` red — both downstream of the merge conflict | **Rebase needed** against current main; Cargo.lock + likely `crates/sbo3l-server/src/lib.rs` (router section) collide with #320/#315 churn. Once rebased, CI should re-evaluate. |
| #323 | 3-node Raft cluster scaffold — EXPERIMENTAL (R14 P4) | OPEN, **CONFLICTING**, 5 failed checks | Same `Rust check` + `docker e2e` shape as #322 — conflict-induced | **Rebase needed**; scaffold is self-contained under `crates/sbo3l-server/src/cluster/*` so the conflict is in shared files (Cargo.toml feature list, lib.rs `pub mod cluster;` gate). |
| #324 | AWS + GCP KMS code-ready (R14 P3) | OPEN, **CONFLICTING**, 5 failed checks | Same shape | **Rebase + adjust per Daniel's R15 directive (see §KMS-shift below).** |
| #327 | admin backup/restore/export/verify (R14 P2) | OPEN, **CONFLICTING**, 2 failed checks | Same shape | **Rebase needed**; the cli's `Cargo.toml` + `main.rs` are the likely collision points (multiple R14 agents touched the dispatcher). |
| #329 | Helm chart skeleton (R14 P6) | OPEN, MERGEABLE, BEHIND | All checks pass on its tip | Branch updated 2026-05-02T16:27Z — auto-merge will fire on next cascade tick. |
| #330 | OpenTelemetry tracing layer (R14 P5) | OPEN, MERGEABLE, BEHIND | All checks pass on its tip | Branch updated 2026-05-02T16:27Z — auto-merge will fire on next cascade tick. |

## Closeout actions taken this round

1. **#329 + #330 branches updated** (`gh pr update-branch`) so the cascade can fast-forward them. No new commits needed; both are clean against current main on their tip.
2. **#322 / #323 / #324 / #327 left as-is**: each needs a manual rebase. **No new feature work was added** to those branches — leaving them untouched is the right move per the closeout brief ("NO new feature work"). PR comments posted on each documenting the required rebase + (for #324) the KMS-shift below.
3. **Docs/dev1/ landed**: this file + `scope-cuts-r13-r14.md` capture every R13/R14 scope cut so a future maintainer doesn't have to re-derive what was deliberately left out.

## KMS-shift (R15-onward) — affects #324

Daniel confirmed no AWS in R15. The previous gating environment variables (`AWS_KMS_TEST_ENABLED=1`, `GCP_KMS_TEST_ENABLED=1`) were designed to flip the live integration tests on once Daniel provided real cloud credentials. That round won't happen.

**Plan recorded for whoever rebases #324:**
- Rename gating env: `AWS_KMS_TEST_ENABLED` → `MOCK_KMS_TEST_ENABLED`, `GCP_KMS_TEST_ENABLED` → `MOCK_KMS_TEST_ENABLED` (single env covers both clouds since both run the same mock under the rename).
- Replace the live KMS integration tests with a deterministic mock-client test that round-trips a known signature: feed the mock a fixed digest, assert it returns a fixed `(r, s, recovery_id)`, recover the address, assert match against a vector. The mock implementation already lives in `crates/sbo3l-core/src/signers/eth_kms_aws_live.rs` (mock client surface) — promote it to a public test helper and add the deterministic round-trip.
- Add a `# DEFERRED — Daniel confirmed no AWS` banner at the top of `docs/kms-aws-setup.md` and `docs/kms-gcp-setup.md`, with a short note that the runbook captures what setup *would* look like if/when KMS is provisioned externally; the code path stays compiled and unit-tested.

Posted as a comment on PR #324 with the same content; that's the actionable artifact for whoever picks up the rebase.

## Local CI gates run on this branch (`agent/dev1/T-R14-closeout`)

| Gate | Result |
|---|---|
| `cargo build --workspace` | clean (4m55s) |
| `cargo fmt --all -- --check` | clean |
| `cargo test --workspace --tests --no-fail-fast` | see §Local test result below |
| `cargo clippy --workspace --all-targets -- -D warnings` | see §Local clippy result below |

## Out of scope for this closeout

- **Rebasing the 4 conflicted PRs.** That's a code-touching operation; the brief explicitly says "NO new feature work, closeout only." The PR comments + this status doc are the deliverable; the rebase is queued for whoever picks them up next.
- **KMS test rename / mock-deterministic test.** Same reason — the file lives on an unmerged branch (#324). Documented in the §KMS-shift section so the rebase picks it up.
- **Closing PRs.** None of the 4 conflicted PRs should be closed: the work is real and committable, just stale-vs-main. Closing would discard ~5K LOC of valid scaffold/code.

## Where to look next

- `scope-cuts-r13-r14.md` — every honest scope reduction, with reason + what's needed to finish.
- Each PR's body — the original "What's NOT in this PR" section already lists the deliberate exclusions per surface.
- Memory notes (`worktree_pattern_for_shared_repo.md`, `shared_worktree_4plus1_friction.md`) — captured friction this round; the next Dev 1 session should default to `git worktree add /tmp/<task>` for any multi-hour work.
