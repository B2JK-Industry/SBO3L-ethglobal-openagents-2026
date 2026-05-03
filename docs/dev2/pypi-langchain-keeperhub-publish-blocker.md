# Blocker: PyPI Trusted Publisher for sbo3l-langchain-keeperhub

**Status:** ⛔ BLOCKED on Daniel-side PyPI configuration
**Time to fix:** 5 minutes
**Priority:** P2 (nice-to-have; npm version IS live)

## Symptom

GitHub Actions workflow `Integrations publish` fires on
`langchain-keeperhub-py-v*` tag pushes. Build+test jobs PASS (mypy
strict + ruff + pytest all green). The `pypi publish` job FAILS with:

```
Trusted publishing exchange failure:
This generally indicates a trusted publisher configuration error,
but could also indicate an internal error on GitHub or PyPI's part.
```

Run example: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/actions/runs/25265949883/job/74080426020

## Root cause

`sbo3l-langchain-keeperhub` is a NEW PyPI package (doesn't exist yet
on PyPI). PyPI Trusted Publishers require either:
1. The package to ALREADY exist with claimed ownership, OR
2. A "pending publisher" entry pre-registered on PyPI before first
   publish.

Neither is in place. The workflow uses GitHub Environment
`pypi-langchain-keeperhub-py` for OIDC token exchange but PyPI has
no matching trusted publisher config.

## Fix (Daniel-side)

1. Login to PyPI: <https://pypi.org/manage/account/publishing/>
2. Click "Add a new pending publisher"
3. Fill in:
   - **PyPI Project Name:** `sbo3l-langchain-keeperhub`
   - **Owner:** `B2JK-Industry`
   - **Repository name:** `SBO3L-ethglobal-openagents-2026`
   - **Workflow name:** `integrations-publish.yml`
   - **Environment name:** `pypi-langchain-keeperhub-py`
4. Save.
5. Re-fire workflow: `gh workflow run "Integrations publish" --ref langchain-keeperhub-py-v1.2.0`

After successful first publish, PyPI converts the pending publisher
into a real one. Subsequent publishes auto-work.

## Workaround (if Daniel can't do this before submit)

`@sbo3l/langchain-keeperhub` IS live on npm (verified 2026-05-03).
That covers the TypeScript half of the framework-plugin claim. Python
half is shipped IN-REPO at `integrations/langchain-keeperhub-py/` —
judges can `pip install -e integrations/langchain-keeperhub-py` from
a fresh clone. No PyPI dependency needed for the demo.

Submission narrative: cite npm publish + in-repo Python source + the
GitHub workflow run showing all build+test green (proof the package
WOULD publish if PyPI Trusted Publisher were configured).

## Re-tag instruction (after Daniel fix)

```bash
cd /Users/danielbabjak/Desktop/MandateETHGlobal/mandate-ethglobal-openagents-2026
git tag -d langchain-keeperhub-py-v1.2.0
git push origin :refs/tags/langchain-keeperhub-py-v1.2.0
git tag langchain-keeperhub-py-v1.2.0 main
git push origin langchain-keeperhub-py-v1.2.0
# Wait ~5 min, verify:
curl -s "https://pypi.org/pypi/sbo3l-langchain-keeperhub/json" | python3 -c "import json,sys; print(json.load(sys.stdin).get('info',{}).get('version'))"
```
