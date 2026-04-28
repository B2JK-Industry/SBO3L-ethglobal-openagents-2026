# Operator Console — local proof + readiness viewer

A static, offline operator surface for Mandate. Sister artefact to
`trust-badge/`: where the trust badge is the dense **one-screen judge**
view, the operator console is the longer **operational** view — same
offline / no-JS / no-network discipline, more panels, with explicit
"not implemented yet" placeholders for the production-shaped capabilities
still on Developer A's backlog.

This is **B2.v1**. The five blocked panels light up in tiny follow-up PRs
as Developer A's backend work lands.

## Trust badge vs operator console

Two separate static surfaces, intentionally distinct:

| | `trust-badge/` | `operator-console/` |
|---|---|---|
| Audience | Judge / sponsor reviewer | Operator / auditor |
| Goal | Land the proof in 10 seconds | Walk every panel an operator might check |
| Layout | One-screen, dense, side-by-side allow/deny | Vertical timeline + multi-panel grid |
| Size | ~5 KB | ~8 KB |
| Panels | 4 functional | 6 functional + 5 honest backlog placeholders |
| Schema | `mandate-demo-summary-v1` | `mandate-demo-summary-v1` (same, no bump) |
| Build | `python3 trust-badge/build.py` | `python3 operator-console/build.py` |
| Test | `python3 trust-badge/test_build.py` | `python3 operator-console/test_build.py` |

Both consume the same demo runner transcript without changing it. Both are
fully offline (no JS, no `fetch()`, no external CSS/fonts/URLs). Both ship
with a stdlib regression test that asserts every required field, every
mock disclosure, the no-network surface, and `html.parser` well-formedness.

The blocked panels in this console are not stubbed marketing copy — they
are real placeholders that surface their `PSM-*` backlog id and a yellow
`not implemented yet` pill. They light up automatically as Developer A's
backend PRs land and a tiny B-side follow-up consumes the new value.

## What it shows today

**Functional panels (rendered from the demo runner's transcript JSON):**

- **Header** — `agent_id`, `demo_commit` (12-char visible, full 40-char SHA in a `title=""` tooltip), `generated_at_iso`, `schema`, tagline.
- **Allow / deny timeline** — both demo scenarios as ordered events:
  - Allow · `legit-x402` → matched_rule, request_hash, policy_hash, audit_event, receipt_signature, KeeperHub `kh-<ULID>` execution_ref, `mock` tag.
  - Deny · `prompt-injection` → deny_code, matched_rule, request_hash, policy_hash, audit_event, receipt_signature, `denied_action_executed: false`, `keeperhub_refused: true`.
- **No-key proof** — `status` (PASS/FAIL pill) + the three falsifiable counts.
- **Audit-chain tamper detection** — both verifier outcomes as boolean pills.
- **Mock sponsor disclosure** — KeeperHub allow path + mock tag, KeeperHub deny path refusal, denied-action-executed status, ENS offline-fixture pill, Uniswap `local_mock` pill.
- **Audit-bundle verification (optional)** — when invoked with `--bundle <path>`, runs `mandate audit verify-bundle` and renders the parsed result. Without `--bundle`, renders an honest "bundle not provided" state with the exact commands to produce one.

**Pending-panel placeholder (backend already merged on `main`; console panel landing in B2.v2):**

- HTTP `Idempotency-Key` safe-retry — **PSM-A2 merged on `main`** (PR #23). Console panel intentionally still pending B2.v2; the four-case behaviour matrix is exercised today by `demo-scripts/run-production-shaped-mock.sh` step 7. The placeholder pill is blue (`pending`), not yellow (`blocked`), to avoid the dishonest "not implemented yet" claim.

**Blocked-panel placeholders (backend not yet merged):**

- Active policy lifecycle (`mandate policy current` / `activate` / `diff`) — backlog **PSM-A3**
- Mock KMS CLI surface (`mandate key list --mock` / `mandate key rotate --mock`) + storage — backlog **PSM-A1.9**
- Audit checkpoints (`mandate audit checkpoint create` / `verify`) — backlog **PSM-A4**
- Operator readiness summary (`mandate doctor`) — backlog **PSM-A5**

Each placeholder lights up when the corresponding A-side PR lands and a
follow-up B-side PR consumes the new value. The console renders honestly
today: nothing is faked, nothing is hidden.

## How to use

From the repo root:

```bash
# 1. Run the demo. Step 13 writes the deterministic JSON used as input.
bash demo-scripts/run-openagents-final.sh

# 2. Render the operator console.
python3 operator-console/build.py

# 3. Open it.
open operator-console/index.html        # macOS
xdg-open operator-console/index.html    # Linux
start operator-console/index.html       # Windows
```

The HTML is self-contained: no JS, no external CSS, no external fonts, no
network calls. It works directly from `file://` — no local web server
needed.

### Optional: render the audit-bundle verification panel

If you've produced a `mandate.audit_bundle.v1` JSON file (e.g. via the
production-shaped runner), pass its path:

```bash
python3 operator-console/build.py --bundle /path/to/bundle.json
```

The build will run `mandate audit verify-bundle --path /path/to/bundle.json`
once and render its parsed `decision`, `deny_code`, `chain_length` and
`audit_event_id`. Verification failures and missing-binary / missing-bundle
states render as explicit failure panels — never silently skipped.

## Render regression test

Stdlib-only end-to-end test:

```bash
python3 operator-console/test_build.py
```

It drives `build.py` against `operator-console/fixtures/operator-summary.json`
and asserts:

- every required proof field renders
- every blocked-on-A panel renders with its `PSM-*` backlog label
- mock disclosures (`mock`, `offline fixture`, `local_mock`) are present
- the surface never invites JS or network — no `<script>`, no `fetch(`, no `http(s)://`
- `html.parser` feeds the output without error

The fixture lives at `operator-console/fixtures/operator-summary.json` with
deterministic, fictional values (no real secrets, no real signatures).

## CLI

```text
python3 operator-console/build.py [--input PATH] [--output PATH] [--bundle PATH] [--mandate-bin PATH]

  --input         Demo summary JSON
                  (default: demo-scripts/artifacts/latest-demo-summary.json)
  --output        Static HTML console
                  (default: operator-console/index.html)
  --bundle        Optional path to a `mandate.audit_bundle.v1` JSON file. When set,
                  build runs `mandate audit verify-bundle --path <path>` and renders
                  the parsed result. When unset, the console renders an honest
                  'bundle not provided' state.
  --mandate-bin   Optional override for the `mandate` binary path
                  (default: target/debug/mandate). Only consulted when --bundle is set.
```

The script refuses to render if the input's `"schema"` field is anything
other than `mandate-demo-summary-v1` — protects against silent drift if the
JSON contract changes in a future PR.

## Files

```
operator-console/
  build.py                       Generator. Stdlib only (json, html, argparse, pathlib, re, subprocess, html.parser).
  test_build.py                  Render regression test. Stdlib only.
  fixtures/operator-summary.json Deterministic input for test_build.py.
  README.md                      This file.
  .gitignore                     Excludes the generated index.html from commits.
  index.html                     Generated by build.py. Never hand-edited. Not committed.
```

## Roadmap (B2.v2+)

B2.v1 ships the dense, honest-placeholder console today. The five
backend-blocked panels each light up in a tiny B-side follow-up PR
once their A-side dependency lands on `main`:

| Panel | Lights up after | What B2.v2+ does |
|---|---|---|
| HTTP `Idempotency-Key` safe-retry | **PSM-A2** | Reads new fields from the runner transcript (`idempotency.observed: true/false`, `idempotency.same_request_replayed`, `idempotency.different_request_conflict_409`). Replaces the placeholder with a real status panel showing the three cases. No demo-script edits beyond emitting the new fields. |
| Active policy lifecycle | **PSM-A3** | Probes `mandate policy current` via subprocess. Renders `policy_hash`, `policy_version`, `activated_at`, `approved_by`. Falls back to placeholder if the binary is missing. |
| Mock KMS CLI surface | **PSM-A1.9** | Probes `mandate key list --mock`. Renders `key_id`, `key_version`, public key hex. Replaces the hardcoded dev pubkey constants in the production-shaped runner with values pulled from this CLI. |
| Audit checkpoints | **PSM-A4** | Probes `mandate audit checkpoint create / verify`. Renders the latest checkpoint structure (seq, latest_event_hash, mock_anchor_ref, signature, verification status). |
| Operator readiness summary | **PSM-A5** | Probes `mandate doctor --format json`. Embeds the JSON output as a panel-grouped readiness summary. |

Discipline for every B2.v2+ PR:

- One panel per PR, behind one A-side dependency.
- Same `mandate-demo-summary-v1` schema unless a new field is genuinely needed (then bump to `-v2` with `trust-badge/build.py`'s schema-pin guard updated in lockstep).
- Same regression-test pattern: assert the panel renders the new value AND assert the placeholder no longer appears.
- No marketing copy, no fake values, no silent fallback to a placeholder when the backend value is malformed (render an explicit failure state instead).

B2.v2 implementation is **unblocked** — PSM-A2 has merged on `main` (PR #23). The console panel update is the next B-side PR; today the production-shaped runner walks the four-case Idempotency-Key behaviour matrix end-to-end against a real `mandate-server` daemon, and the operator-console PSM-A2 row carries a blue `pending` pill pointing at that runner.

## Honest scope

- **Sister surface, not a replacement.** The trust badge stays the
  one-screen judge artefact. The operator console is the longer
  operational view. Both consume the same `mandate-demo-summary-v1`
  transcript without changing it.
- **Mocks remain mocks.** The console renders KeeperHub / Uniswap
  executors with `mock` / `local_mock` tags and reproduces the demo's
  "denied actions never reach the sponsor" claim from the captured
  `keeperhub_refused: true`. No interpretive marketing copy.
- **Blocked is blocked.** Every backend-blocked panel surfaces its
  `PSM-*` backlog id and a `not implemented yet` pill. No backend value
  is rendered until the corresponding A-side PR lands.
- **Verifies, does not validate.** Where the optional `--bundle` panel
  verifies, it does so by spawning the real `mandate audit verify-bundle`
  CLI — no in-Python re-implementation of any cryptographic claim.
