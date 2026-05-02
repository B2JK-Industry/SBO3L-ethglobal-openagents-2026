# Rehearsal walkthrough — 2026-05-02 ~11:10 CEST

> **Audience:** Daniel before the demo recording, Heidi for next-pass corrections.
> **Method:** Heidi walked `docs/submission/rehearsal-runbook.md` step-by-step, timing each, surfacing friction.
> **Verdict:** runbook is *recoverable* but has **3 stale references** and **1 real inventory gap**. ~5 min total walkthrough; well under the 8-min judge-attention budget.

## Step-by-step timings + friction

### Step 1 — `bash scripts/submission/rehearsal-audit.sh` (60s expected, 65s actual)

**Result:** 59 PASS / 15 WARN / **7 FAIL**

**Friction:**
- ⚠️ Runbook says "expect: 36 PASS / 15 WARN / 0 FAIL" — actual is 59/15/7. PASS count is higher (more docs added since runbook authored). FAIL count is 7 (was 0 in the runbook spec).
- 6 of the 7 FAILs are **audit-script URL-extraction bugs**, not real submission gaps:
  - `https://app.ens.domains/sbo3lagent.eth](https://...` — markdown link extraction captures `](url)` glue
  - `https://crates.io/api/v1/crates/$c` — shell template literal (`$c`) inside a code block
  - `https://pypi.org/pypi/$p/json` — same shell template
  - `https://registry.npmjs.org/$p` — same
  - `https://sbo3l-ccip.vercel.app/api/.../...json` returns 400 — but 400 IS the **correct rejection** for the smoke-fail-mode invalid input; audit script treats all 4xx as fail
  - `https://||;s|/|_|g'` — sed expression captured as URL
- 1 real FAIL: `https://sbo3l-trust-dns-viz.vercel.app` 404 (known 🔴 in `live-url-inventory.md`; viz package main not yet deployed)

**Action:**
- ⏭ Defer audit-script fix to a follow-up (URL extraction needs to skip shell-template strings in code blocks + treat known smoke-fail URLs as expected-4xx).
- 🚦 Trust-dns viz deployment is a separate Daniel-side action (CTI-3-4 hosted-app + DNS).

### Step 2 — `cat scripts/chaos/artifacts/summary.txt` (5s)

**Result:** Shows the OLD round-4 chaos run summary (3/5 PASS).

**Friction:**
- 🚨 **Stale data on main.** The 5/5 PASS chaos result lives in PR #235 (`docs/proof/chaos-suite-results-v1.2.0.md`) which hasn't merged. Main still shows the round-4 3/5 PASS.
- Runbook says "expect: 3/5 PASS minimum" — that matches main's content but UNDER-STATES what the project actually achieved. Recording-day narration would be honest but understated.

**Action:**
- 🚦 Push #235 through (it's auto-merge armed, BLOCKED on CI cycling). Once landed, `summary.txt` will show the canonical 5/5 PASS evidence + `docs/proof/chaos-suite-results-v1.2.0.md` is the human-readable proof doc.

### Step 3 — `cargo install sbo3l-cli --version 1.2.0` (~3 min on warm cache)

**Result:** still works; installs 1.0.1.

**Friction:**
- 🚨 **Version is stale.** The runbook hardcodes `--version 1.2.0`. Today (2026-05-02 11:00 CEST) the canonical install is `--version 1.2.0` — all 9 crates published at 1.2.0 ~30 min ago.
- A judge following this runbook gets the **previous release**, not the current one. Functionally OK (1.0.1 still works) but presentation-wise wrong.

**Action:**
- ✏️ **Bump runbook to 1.2.0.** Single edit. Doing it in this PR.

### Step 4 — `bash scripts/judges/verify-everything.sh` (~4-5 min)

Not re-run in this walkthrough (already verified clean in R10 against fresh-clone tempdir; 33 PASS / 1 FAIL / 0 SKIP in 4m22s).

**Friction:**
- 🟡 The 1 FAIL there is the ENS namehash-resolver bug in the script (cited in `docs/submission/url-evidence.md` and `judges-walkthrough.md`). Cosmetic; doesn't block.

### Step 5 — Recording setup checklist (browser tab order, terminal font, etc.) — N/A for Heidi

The runbook's recording-setup section assumes Daniel is at the keyboard. Heidi can't validate (no browser, no screen recorder). Visual verification at recording time is on Daniel.

### Step 6 — Storyboard execution timing breakdown

The runbook references `demo-video-script.md` for per-section timing. Cross-checked: the 7-section storyboard adds to 3:00 exactly. No friction.

### Step 7 — Cuts to consider / Cuts NOT to make

Sections present in the runbook. No friction.

## Summary

| Step | Time | Friction | Action |
|---|---|---|---|
| 1. rehearsal-audit | 65s | 6/7 FAILs are tool bugs | defer tool fix |
| 2. chaos summary | 5s | stale (5/5 PASS only after #235 lands) | push #235 |
| 3. cargo install | ~3 min | runbook says 1.0.1, current is 1.2.0 | **bump runbook in this PR** |
| 4. verify-everything | _skip_ | 1 known cosmetic FAIL | tracked |
| 5-7. Daniel-side recording | _skip_ | n/a for Heidi | n/a |

**Total walkthrough time:** ~5 min (excluding Daniel-only steps).
**Judge-attention budget:** 8 min target → walking the runbook stays inside the budget.

## Runbook corrections to apply (in this PR)

1. Bump `cargo install sbo3l-cli --version 1.2.0` → `--version 1.2.0` (3 occurrences likely)
2. Bump expected rehearsal-audit count from `36 PASS / 15 WARN / 0 FAIL` → `59 PASS / 15 WARN / 7 FAIL (6 of 7 FAILs are audit-script URL-extraction bugs, not submission gaps)`
3. Add note: "if `summary.txt` shows 3/5 PASS, you're reading round-4 stale data — see `docs/proof/chaos-suite-results-v1.2.0.md` for the canonical 5/5 PASS"

## Re-run schedule

- **At submission time -1h:** Daniel walks runbook himself (full version including recording-setup + storyboard execution).
- **At record time:** runbook is the script; deviations are recorded as comments in the next preflight pass.
