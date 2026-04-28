# Operator Console — local proof + readiness viewer

A static, offline operator surface for Mandate. Sister artefact to
`trust-badge/`: where the trust badge is the dense **one-screen judge**
view, the operator console is the longer **operational** view — same
offline / no-JS / no-network discipline, more panels, with **real**
evidence rendered straight from the production-shaped runner's transcript
for every merged A-side surface (PSM-A1.9 / A2 / A3 / A4 / A5).

This is **B2.v2**. All five A-side backlog items have merged on `main`,
the production-shaped runner emits a deterministic `mandate-operator-evidence-v1`
transcript, and each former pending-pill panel now renders the actual
captured values. When the evidence transcript is missing/malformed/wrong-schema
each panel renders an honest "evidence not gathered" placeholder — never
a fake-OK pill.

## Trust badge vs operator console

Two separate static surfaces, intentionally distinct:

| | `trust-badge/` | `operator-console/` |
|---|---|---|
| Audience | Judge / sponsor reviewer | Operator / auditor |
| Goal | Land the proof in 10 seconds | Walk every panel an operator might check |
| Layout | One-screen, dense, side-by-side allow/deny | Vertical timeline + multi-panel grid |
| Size | ~5 KB | ~13 KB |
| Panels | 4 functional | 11 functional (6 demo-summary + 5 real-evidence) |
| Schema | `mandate-demo-summary-v1` | `mandate-demo-summary-v1` (timeline / no-key / audit-chain / mock disclosure / bundle) **+** `mandate-operator-evidence-v1` (PSM-A* real-evidence panels) |
| Build | `python3 trust-badge/build.py` | `python3 operator-console/build.py` |
| Test | `python3 trust-badge/test_build.py` | `python3 operator-console/test_build.py` |

Both consume the demo runner transcripts without changing them. Both are
fully offline (no JS, no `fetch()`, no external CSS/fonts/URLs). Both ship
with a stdlib regression test that asserts every required field, every
mock disclosure, the no-network surface, and `html.parser` well-formedness.

## What it shows today

**Demo-summary panels (rendered from `mandate-demo-summary-v1`):**

- **Header** — `agent_id`, `demo_commit` (12-char visible, full 40-char SHA in a `title=""` tooltip), `generated_at_iso`, `schema`, tagline.
- **Allow / deny timeline** — both demo scenarios as ordered events:
  - Allow · `legit-x402` → matched_rule, request_hash, policy_hash, audit_event, receipt_signature, KeeperHub `kh-<ULID>` execution_ref, `mock` tag.
  - Deny · `prompt-injection` → deny_code, matched_rule, request_hash, policy_hash, audit_event, receipt_signature, `denied_action_executed: false`, `keeperhub_refused: true`.
- **No-key proof** — `status` (PASS/FAIL pill) + the three falsifiable counts.
- **Audit-chain tamper detection** — both verifier outcomes as boolean pills.
- **Mock sponsor disclosure** — KeeperHub allow path + mock tag, KeeperHub deny path refusal, denied-action-executed status, ENS offline-fixture pill, Uniswap `local_mock` pill.
- **Audit-bundle verification (optional)** — when invoked with `--bundle <path>`, runs `mandate audit verify-bundle` and renders the parsed result. Without `--bundle`, renders an honest "bundle not provided" state with the exact commands to produce one.

**Real-evidence panels (rendered from `mandate-operator-evidence-v1`):**

- **PSM-A2 · HTTP Idempotency-Key safe-retry (4-case behaviour matrix)** — case 1 first POST (200, audit_event_id, decision), case 2 same key + same body retry (200 byte-identical), case 3 same key + mutated body (409 `protocol.idempotency_conflict`), case 4 new key + same nonce (409 `protocol.nonce_replay`). Captured by `demo-scripts/run-production-shaped-mock.sh` step 7 against a real `mandate-server` daemon on `127.0.0.1:18730` with persistent SQLite at `idempotency.db`.
- **PSM-A5 · `mandate doctor`** — `report_type`, overall pill, ok/skip/fail counts, plus per-status grouped check rows (color-coded green/yellow/red). Captured from `mandate doctor --json` (production-shaped runner step 2).
- **PSM-A1.9 · Mock KMS keyring (mock, not production KMS)** — tabular keyring listing with role, version, key_id, public-key hex prefix, created_at, mock-pill. Captured from `mandate key list --mock --db` (production-shaped runner step 3, post-rotate). Every line carries the `mock-kms:` prefix in the runner output.
- **PSM-A3 · Active policy lifecycle** — version, policy_hash, source, activated_at. Captured from `mandate policy current --db` (production-shaped runner step 4 after `policy activate`). Local lifecycle, not remote governance — there is no on-chain anchor, no consensus, no signing on activation; the panel says so explicitly.
- **PSM-A4 · Audit checkpoints (mock anchoring, NOT onchain)** — schema, sequence, latest_event_id, latest_event_hash, chain_digest, mock_anchor_ref, created_at, plus three boolean pills for `structural_verify_ok`, `db_cross_check_ok`, and `verify result_ok`. Captured from `mandate audit checkpoint create` + `verify` (production-shaped runner step 10). Every line in the runner output carries the `mock-anchor:` prefix.

When the evidence transcript is missing, unreadable, fails JSON parse, or
carries a wrong `schema`, each of the five panels renders an explicit
"evidence not gathered" placeholder showing the failure reason and the
exact command to regenerate it (`bash demo-scripts/run-production-shaped-mock.sh`).
The console never substitutes a fake-OK pill for missing evidence.

## How to use

From the repo root:

```bash
# 1. Run the demo. Step 13 writes the deterministic JSON used by the demo-summary panels.
bash demo-scripts/run-openagents-final.sh

# 2. Run the production-shaped runner. Step 12 writes the mandate-operator-evidence-v1
#    transcript used by the five real-evidence panels.
bash demo-scripts/run-production-shaped-mock.sh

# 3. Render the operator console.
python3 operator-console/build.py

# 4. Open it.
open operator-console/index.html        # macOS
xdg-open operator-console/index.html    # Linux
start operator-console/index.html       # Windows
```

The HTML is self-contained: no JS, no external CSS, no external fonts, no
network calls. It works directly from `file://` — no local web server
needed.

If you skip step 2, the five real-evidence panels render their honest
"evidence not gathered" placeholders — the demo-summary panels still
render normally.

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
(demo-summary) and `operator-console/fixtures/operator-evidence.json`
(operator-evidence) and asserts:

- every required proof field renders (timeline, no-key, audit-chain, mock disclosure, bundle, plus the five real-evidence panels);
- every real-evidence panel surfaces the values pulled directly from the evidence fixture (idempotency case codes, doctor report fields, KMS key_id / pubkey prefixes, policy_hash / activated_at, audit-checkpoint mock_anchor_ref / chain_digest / verify booleans);
- **negative**: PSM-A1.9 / A2 / A3 / A4 / A5 must NOT appear inside any `class="pill blocked"` or `class="pill pending"` — would lie about the merged state of the backend or the missing console panel;
- mock disclosures (`mock`, `offline fixture`, `local_mock`, `mock, not production KMS`, `mock anchoring, NOT onchain`) are present;
- the surface never invites JS or network — no `<script>`, no `fetch(`, no `http(s)://`;
- `html.parser` feeds the output without error.

The fixtures live at `operator-console/fixtures/operator-summary.json`
and `operator-console/fixtures/operator-evidence.json` with deterministic,
fictional values (no real secrets, no real signatures).

## CLI

```text
python3 operator-console/build.py [--input PATH] [--evidence PATH] [--output PATH] [--bundle PATH] [--mandate-bin PATH]

  --input         Demo summary JSON (mandate-demo-summary-v1)
                  (default: demo-scripts/artifacts/latest-demo-summary.json)
  --evidence      Operator evidence JSON (mandate-operator-evidence-v1)
                  (default: demo-scripts/artifacts/latest-operator-evidence.json)
                  Written by the production-shaped runner's step 12. When
                  missing/malformed/wrong-schema, the five real-evidence
                  panels render an explicit 'not gathered' placeholder.
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
JSON contract changes in a future PR. The evidence transcript is checked
against `mandate-operator-evidence-v1` separately; mismatches there fall
through to the explicit 'wrong schema' placeholder rather than aborting
the render.

## Files

```
operator-console/
  build.py                         Generator. Stdlib only (json, html, argparse, pathlib, re, subprocess, html.parser).
  test_build.py                    Render regression test. Stdlib only.
  fixtures/operator-summary.json   Deterministic demo-summary input for test_build.py.
  fixtures/operator-evidence.json  Deterministic operator-evidence input for test_build.py.
  README.md                        This file.
  .gitignore                       Excludes the generated index.html from commits.
  index.html                       Generated by build.py. Never hand-edited. Not committed.
```

## Honest scope

- **Sister surface, not a replacement.** The trust badge stays the
  one-screen judge artefact. The operator console is the longer
  operational view. Both consume the same `mandate-demo-summary-v1`
  transcript without changing it. The operator console additionally
  consumes `mandate-operator-evidence-v1` for the PSM-A* panels — a
  separate transcript so the trust-badge contract is untouched.
- **Mocks remain mocks.** The console renders KeeperHub / Uniswap
  executors with `mock` / `local_mock` tags and reproduces the demo's
  "denied actions never reach the sponsor" claim from the captured
  `keeperhub_refused: true`. The PSM-A1.9 panel labels itself "mock,
  not production KMS"; the PSM-A4 panel labels itself "mock anchoring,
  NOT onchain". No interpretive marketing copy.
- **No fake-OK on missing evidence.** Each real-evidence panel renders
  an explicit reason (`missing` / `unreadable` / `parse_failed` /
  `wrong_schema`) and the exact command to regenerate the transcript
  when it cannot be read.
- **Verifies, does not validate.** Where the optional `--bundle` panel
  verifies, it does so by spawning the real `mandate audit verify-bundle`
  CLI — no in-Python re-implementation of any cryptographic claim.
