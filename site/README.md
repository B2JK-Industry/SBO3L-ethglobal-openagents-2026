# `site/` — public proof surface (GitHub Pages)

> **Source-side documentation only — this file is not deployed.** The Pages workflow intentionally skips `cp site/README.md _site/README.md` because this README describes the offline-surface byte-grep pattern in text, which would itself trip the grep on the deployed site. Visitors land on `site/index.html`; reviewers reading `git log site/` land here.

The committed contents of this directory are the **source** for the GitHub Pages public proof surface deployed by [`.github/workflows/pages.yml`](../.github/workflows/pages.yml).

| Path | Source / generated | What it is |
|---|---|---|
| `site/index.html` | source (committed) | Landing page. Plain HTML, no JS, no client-side network calls, no external asset. Same offline / no-network discipline as `trust-badge/index.html` and `operator-console/index.html`. |
| `site/README.md` | source (committed) | This file. Describes what the deployed site shows. |
| `site/trust-badge/index.html` | **generated at deploy time** | `python3 trust-badge/build.py` output, built from `trust-badge/fixtures/demo-summary.json` + `test-corpus/passport/golden_001_allow_keeperhub_mock.json`. Not committed (gitignored). |
| `site/operator-console/index.html` | **generated at deploy time** | `python3 operator-console/build.py` output, built from `operator-console/fixtures/operator-summary.json` + `operator-console/fixtures/operator-evidence.json` + the same golden capsule. Not committed. |
| `site/capsule.json` | **generated at deploy time** | Copy of `test-corpus/passport/golden_001_allow_keeperhub_mock.json` — the canonical, deterministic Passport capsule fixture committed alongside the schema + verifier. Not committed under `site/`. |

The workflow runs offline: it does not fetch external resources at build time, and the deployed HTML does not fetch external resources at view time. The byte-grep `'<script|fetch\(|https?://(?!safe-allowlist)'` returns 0 matches against every deployed file.

## Why fixtures, not runtime artefacts

The Pages site is a **stable public URL** — judges should see the same shape on every visit. Committed fixtures are:

- deterministic (no fresh ULIDs / hashes per deploy);
- verified by `trust-badge/test_build.py` and `operator-console/test_build.py` on every CI run;
- explicitly disclosed as fixture data in the rendered HTML (`agent_id: fixture-agent-01`, etc.);
- stable across pushes, so a `git diff site/` between deploys is meaningful.

The runtime artefacts under `demo-scripts/artifacts/` change on every demo run; they are right for local development and the operator-evidence transcript, but wrong for a stable judge-facing URL.

## Why this exists separately from `trust-badge/` and `operator-console/`

`trust-badge/` and `operator-console/` are the source surfaces — they own their own `build.py`, regression tests, and fixture data, and the rendered HTML is the test artefact (gitignored under each). `site/` is the **deployment wrapper**: it composes those rendered artefacts into a single landing page and one canonical capsule download, behind one stable URL. The committed contents are intentionally minimal so that a reviewer reading `git log site/` sees deployment-pipeline changes, not data drift.
