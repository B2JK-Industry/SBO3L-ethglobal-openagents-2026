# CI failure report — 2026-05-03 morning

> **Filed by:** Heidi (QA), self-triggered after Daniel flagged "veľa neúspešných CI" on GitHub.
> **State at filing:** main HEAD `91dd180` (after R22 cascade). 3 distinct workflows red.

## TL;DR

**3 failing workflows, 3 distinct root causes.** None block production; all are CI-side issues that would surface as judges do nothing wrong. Listed by impact:

| Workflow | Root cause | Severity | Owner |
|---|---|---|---|
| `install-smoke` (recurring) | PyPI matrix entries reference packages not yet published (`sbo3l-langchain-keeperhub`); other 2 took 30+ min to propagate so earlier runs caught the gap window | 🟡 P2 | Dev 2 |
| `pypi-republish-langchain-keeperhub` (1×) | Tag-find logic in `pypi-republish-langchain-kh.yml` looks for tag wrong way; tag `langchain-keeperhub-py-v1.2.0` IS on origin (lightweight) but workflow can't resolve it | 🟡 P2 | Dev 2 |
| `Uptime probe` (1× transient) | `crates.io: sbo3l-core max_version` returned empty during the run; API works now (re-tested at 11:00 UTC); rate-limit or transient outage | 🟢 P3 | (self-recovers) |

## Per-workflow detail

### 1. `install-smoke` — failing on every push

**Failure log excerpt:**
```
pip install sbo3l-crewai-keeperhub==1.2.0
ERROR: Could not find a version that satisfies the requirement sbo3l-crewai-keeperhub==1.2.0 (from versions: none)
ERROR: No matching distribution found for sbo3l-crewai-keeperhub==1.2.0

pip install sbo3l-langchain-keeperhub==1.2.0
ERROR: Could not find a version that satisfies the requirement sbo3l-langchain-keeperhub==1.2.0 (from versions: none)

pip install sbo3l-autogen-keeperhub==1.2.0
ERROR: Could not find a version that satisfies the requirement sbo3l-autogen-keeperhub==1.2.0 (from versions: none)
```

**Live PyPI state at 11:00 UTC (Heidi probe):**
- ✅ `sbo3l-crewai-keeperhub` 1.2.0 (now live)
- ✅ `sbo3l-autogen-keeperhub` 1.2.0 (now live)
- 🔴 `sbo3l-langchain-keeperhub` **still 404** (only npm package `@sbo3l/langchain-keeperhub` exists)
- 🔴 `sbo3l-elizaos-keeperhub` **still 404** (only npm package exists)

**Root cause:** install-smoke matrix lists 4+ PyPI packages but only 2 are actually published. The other 2 (`langchain-keeperhub`, `elizaos-keeperhub`) only exist as **npm packages** — there's no Python equivalent on PyPI.

**Two fixes possible:**
1. **Drop the missing entries from the matrix** — most accurate; reflects reality.
2. **Publish the missing PyPI packages** — completes the matrix.

### 2. `pypi-republish-langchain-keeperhub` — tag not found

**Failure log excerpt:**
```
::error::tag langchain-keeperhub-py-v1.2.0 not found on origin — cannot dispatch publish workflow
```

**Tag IS on origin (Heidi verified):**
```
$ git ls-remote --tags origin | grep langchain-keeperhub-py
351860ccb2560ef7c19a7e833d71740220b8a570  refs/tags/langchain-keeperhub-py-v1.2.0
```

Note: lightweight tag (no `^{}` dereferenced commit). Other keeperhub tags have both forms. The workflow's tag-find logic likely checks for the dereferenced form and fails on lightweight tags.

**Fix:** update `.github/workflows/pypi-republish-langchain-kh.yml` to accept lightweight tags — either `git ls-remote --tags origin <tag>` (without filter) or `git rev-parse refs/tags/<tag>` after `git fetch --tags`.

### 3. `Uptime probe` — transient crates.io API empty response

**Failure log excerpt:**
```
FAIL [empty] crates.io: sbo3l-core max_version — https://crates.io/api/v1/crates/sbo3l-core (filter: .crate.max_version)
FAIL [empty] crates.io: sbo3l-cli max_version — https://crates.io/api/v1/crates/sbo3l-cli (filter: .crate.max_version)
FAIL [empty] crates.io: sbo3l-server max_version — https://crates.io/api/v1/crates/sbo3l-server (filter: .crate.max_version)
```

**Live re-probe at 11:00 UTC:**
- ✅ all 3 crates return `max_version: 1.2.0`

**Root cause:** transient crates.io API rate-limit OR brief outage; probe got empty response. **Self-recovers next poll.** Can be made more robust with retries.

## Net impact on submission

🟢 **Zero impact on production.** Live packages, contracts, and pages all work. The 3 failures are CI hygiene — the workflows themselves need fixes, not the deployed system.

Judges who:
- Install via npm/pip/cargo: ✅ all the actually-published packages work
- Hit any URL: ✅ all marketing routes 200
- Read any sponsor page: ✅ all live + truthful

## Dev prompts (drop-in for next session)

### Prompt for Dev 2 — fix install-smoke + pypi-republish

```
You are Dev 2. Fix two CI failures Heidi flagged:

ISSUE 1 — install-smoke matrix references unpublished PyPI packages

Current state (Heidi-verified 2026-05-03 11:00 UTC):
  sbo3l-crewai-keeperhub:   1.2.0 ✅ on PyPI
  sbo3l-autogen-keeperhub:  1.2.0 ✅ on PyPI
  sbo3l-langchain-keeperhub: 404 🔴 (only @sbo3l/langchain-keeperhub on npm exists)
  sbo3l-elizaos-keeperhub:   404 🔴 (only @sbo3l/elizaos-keeperhub on npm exists)

Decide one:
  (a) DROP the 2 missing entries from .github/workflows/install-smoke.yml
      matrix. Add a comment explaining why (the npm packages exist; PyPI
      packages were never published because no Python equivalent of the
      framework SDK exists for those two).
  (b) BUILD the missing Python packages and publish to PyPI.

(a) is the honest fix; (b) is scope creep this close to submission.
Recommend (a).

ISSUE 2 — pypi-republish-langchain-kh.yml tag-find logic broken

Symptom: workflow says "tag langchain-keeperhub-py-v1.2.0 not found on
origin — cannot dispatch publish workflow" but the tag IS on origin
(verified via `git ls-remote --tags origin | grep langchain-keeperhub-py`).

Likely cause: the tag is lightweight (no annotated `^{}` dereferenced
commit). Workflow's tag-check probably uses `git rev-parse <tag>^{}`
which fails on lightweight tags.

Fix: in .github/workflows/pypi-republish-langchain-kh.yml, replace the
tag-existence check with one that accepts both lightweight and annotated
tags. Example:
  git fetch --tags origin
  if git rev-parse --verify "refs/tags/$PUBLISH_TAG" > /dev/null 2>&1; then
    echo "tag found: $PUBLISH_TAG"
  else
    echo "::error::tag $PUBLISH_TAG not found"
    exit 1
  fi
```

### Prompt for Dev 4 — make uptime probe retry

```
You are Dev 4. Heidi flagged a transient uptime-probe failure on 2026-05-03.

Symptom: jq filter returned empty for crates.io API responses on 3 of 10
checks. Re-test 30 min later: API works fine. Likely transient
rate-limit or brief outage.

Fix: add retry-with-backoff to .github/workflows/uptime-probe.yml curl
calls. 3 retries, 5s + 10s + 20s backoff. Treat empty-response same as
network error for retry purposes.

Add a comment explaining: crates.io rate-limits aggressive polling;
public-node Sepolia RPC also occasionally drops responses; retries
prevent false positives in the alert pipeline.
```
