# Post-rename docs audit

**Branch (read-only):** `feat/dev-b-crypto-resilience-ip` (Dev B unrelated WIP). No file modified beyond this report.
**State:** `main = 6ffb5eb`; PR #59 (`chore/repo-rename-url-update`, 14 files, +42/-42, **OPEN, CLEAN**) staged but not merged. Findings below describe the post-rename world #59 is supposed to deliver, plus one file #59 misses.

## Old-slug residue

`git grep -n "mandate-ethglobal-openagents-2026"` (excl `docs/spec/`): **43 hits across 15 files**. PR #59 covers **14 of 15**. The one #59 misses:

- **`site/index.html:89`** — deployed Pages landing-page footer. **HIGH IMPACT** (judge-facing). Recommend adding to #59 or follow-up.

`git grep -n "mandate-ethglobal" -- '*.md'` (excl `docs/spec/`): 38 hits, all in #59's diff.

## Public proof URL coverage

`b2jk-industry.github.io/mandate-` (stale Pages) — **4 hits** (`README:17`, `SUBMISSION_FORM_DRAFT:189`, `SUBMISSION_NOTES:41`, `SBO3L_PASSPORT_BACKLOG:617`). All in #59. `b2jk-industry.github.io/SBO3L-` on current main: 0 hits — present only in #59's diff.

## GitHub Pages deployment

```
curl -sI .../SBO3L-ethglobal-openagents-2026/   →  HTTP/2 200
curl -sI .../mandate-ethglobal-openagents-2026/  →  HTTP/2 404
```

Pages does NOT redirect on rename (per brief expectation). `gh run list --workflow pages.yml` shows last 5 runs `success`.

## Submission docs sweep

- **`README.md`** — SBO3L brand ✅; old repo URL at lines 17, 64; old Pages URL at 17; **stale 310/310** (line 13).
- **`SUBMISSION_FORM_DRAFT.md`** — SBO3L brand ✅; old repo URL at 189, 197, 209; **stale 310/310** (line 48).
- **`SUBMISSION_NOTES.md`** — SBO3L brand ✅; old Pages URL at 41; **stale `tests (310 passing)`** (line 42).
- **`IMPLEMENTATION_STATUS.md`** — SBO3L brand ✅; old repo URL at 8, 12; **stale `300/300`** (line 20, even older than the 310s).
- **`FEEDBACK.md`** — clean URLs; **2 stray `Mandate`s at lines 133–134** ("anchor the hash directly into the Mandate decision") — brand-drift leftover.
- **`AI_USAGE.md`** — clean.

All URL drift is in #59's diff. Test-count drift is **not** in #59.

## Partner one-pagers

`docs/partner-onepagers/keeperhub.md:38` — one stale repo-URL hit; covered by #59. `ens.md` + `uniswap.md` — clean. `git grep build_envelope` confirms `sbo3l_keeperhub_adapter::build_envelope` exists; the IP-1 helper claim is accurate.

## Demo video script

`demo-scripts/demo-video-script.md` — brand text all `SBO3L` ✅; title-card at line 45 has the old repo URL (covered by #59). Tagline `give it a mandate` is the intentional wordplay (lowercase verb), not brand drift.

## CI workflows

`.github/workflows/{ci,codex-review,pages}.yml` — zero literal repo-URL hits; all use `${{ github.repository }}` or are slug-agnostic. **Rename-safe.** ✅

## Standalone crate external surface

`crates/sbo3l-keeperhub-adapter/`:
- `Cargo.toml:7` `repository =` old slug — in #59. No `documentation` / `homepage` fields (acceptable).
- `README.md` (8 hits at lines 6, 92, 95, 99, 103, 112, 119, 137) — all in #59.
- `CHANGELOG.md:12` — 1 hit, in #59.
- `src/lib.rs:35,43` — module doc-comments, in #59.
- `description` brand-clean ✅.

## External proof (GitHub web)

```
GET .../SBO3L-ethglobal-openagents-2026   →  HTTP/2 200
GET .../mandate-ethglobal-openagents-2026  →  HTTP/2 301
```

Old → new redirect verified. `git remote -v` on this clone still points at the old URL string (functionally OK due to 301; a `git remote set-url` cleanup is cosmetic).

## Truthfulness gaps

- **TRUTHFULNESS GAP** — `README:13`, `SUBMISSION_FORM_DRAFT:48`, `SUBMISSION_NOTES:42` claim **`310/310`**; `IMPLEMENTATION_STATUS:20` claims **`300/300`**; `cargo test --workspace --all-targets` on `main = 6ffb5eb` reports **`317/317`**. Stale across 4 submission docs. PR #59 does NOT touch test counts.
- **TRUTHFULNESS GAP** — `FEEDBACK.md:133–134` has 2 stray capital-`Mandate`s inside Uniswap prose. Brand-drift leftover; should be `SBO3L`.
- **TRUTHFULNESS GAP** — `site/index.html:89` (deployed Pages footer) still says `B2JK-Industry/mandate-…`. PR #59 does NOT touch this file. The judge-facing site itself will keep the old brand after #59 merges.

## Headline summary + recommended follow-up PR scope

**Verdict:** PR #59 is **substantially correct** — URL pattern correct, every file it touches lands on the new slug, external GitHub + Pages already on the new URL. **Two coverage gaps + one truthfulness drift** justify a small follow-up PR before submission:

1. **Add `site/index.html:89` to #59** (or tiny follow-up). Without this the public Pages-site footer keeps the old brand for every judge that lands on the URL.
2. **Test-count refresh** across `README.md`, `SUBMISSION_FORM_DRAFT.md`, `SUBMISSION_NOTES.md`, `IMPLEMENTATION_STATUS.md`: `310/310` (and `300/300`) → `317/317`. Independent of the rename.
3. **Fix the 2 stray `Mandate`s in `FEEDBACK.md:133–134`** to `SBO3L` or recast the sentence.

Recommended follow-up scope: one PR, ≤6 lines across 4 files. CI workflows + the standalone crate require no further changes once #59 merges.
