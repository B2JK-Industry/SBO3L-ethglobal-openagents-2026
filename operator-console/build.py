#!/usr/bin/env python3
# Generate operator-console/index.html from the demo runner's deterministic
# transcript JSON (`demo-scripts/artifacts/latest-demo-summary.json`,
# schema `mandate-demo-summary-v1`).
#
# This is a separate surface from `trust-badge/build.py`. The trust badge is
# the dense one-screen judge artefact. The operator console is the longer
# operational view — same offline / no-JS / no-network discipline, but with
# more panels and explicit "not implemented yet" placeholders for the
# Developer-A backlog items (PSM-A1.9 / A2 / A3 / A4 / A5) so an operator
# can see at a glance what's wired and what isn't.
#
# Stdlib only (json / html / argparse / pathlib / re / subprocess /
# html.parser). No JS, no external CSS, no external fonts, no `fetch()`.

from __future__ import annotations

import argparse
import html
import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_INPUT = REPO_ROOT / "demo-scripts" / "artifacts" / "latest-demo-summary.json"
DEFAULT_EVIDENCE = REPO_ROOT / "demo-scripts" / "artifacts" / "latest-operator-evidence.json"
DEFAULT_OUTPUT = REPO_ROOT / "operator-console" / "index.html"
EXPECTED_SCHEMA = "mandate-demo-summary-v1"
EXPECTED_EVIDENCE_SCHEMA = "mandate-operator-evidence-v1"
# Passport capsule (P2.2). Defaults point at the post-P2.1 runtime artifacts;
# during the P2.2 DRAFT phase (P2.1 not yet merged) the runner does not emit
# these files, so the operator console falls through to the honest "capsule
# evidence not gathered" placeholder. Tests pin the render path against the
# on-main golden fixture in `test-corpus/passport/`.
DEFAULT_CAPSULE_ALLOW = REPO_ROOT / "demo-scripts" / "artifacts" / "passport-allow.json"
DEFAULT_CAPSULE_DENY = REPO_ROOT / "demo-scripts" / "artifacts" / "passport-deny.json"
EXPECTED_CAPSULE_SCHEMA = "mandate.passport_capsule.v1"

# --- helpers ---------------------------------------------------------------


def esc(value) -> str:
    if value is None:
        return '<span class="na">—</span>'
    return html.escape(str(value), quote=True)


def pill(text: str, kind: str) -> str:
    return f'<span class="pill {kind}">{esc(text)}</span>'


def expect_pill(value, expected, *, label_ok: str | None = None, label_bad: str | None = None) -> str:
    if value == expected:
        return pill(label_ok if label_ok is not None else str(value), "ok")
    if value is None:
        return pill("missing", "bad")
    return pill(label_bad if label_bad is not None else str(value), "bad")


def status_pill(value: str | None) -> str:
    v = (value or "").upper()
    if v == "PASS":
        return pill("PASS", "ok")
    if v == "FAIL":
        return pill("FAIL", "bad")
    return pill(v or "UNKNOWN", "neutral")


def mock_pill(value) -> str:
    if value is True:
        return pill("mock", "neutral")
    if value is False:
        return pill("live", "neutral")
    return pill("?", "neutral")


def blocked_pill(backlog_id: str) -> str:
    return f'<span class="pill blocked">not implemented yet — backlog {esc(backlog_id)}</span>'


def pending_pill(backlog_id: str, evidence_path: str) -> str:
    """
    Backend already merged on `main`, console panel intentionally still
    landing in a follow-up B2.v2 PR. We refuse to keep the placeholder as
    `blocked_pill` once the backend lights up — that would lie about the
    production-shaped state. The pill points the operator at the runner
    that does exercise the backend today.
    """
    return (
        f'<span class="pill pending">{esc(backlog_id)} merged · console panel landing in B2.v2 '
        f'(today: walked by <code>{esc(evidence_path)}</code>)</span>'
    )


# --- evidence (operator-evidence-v1) ---------------------------------------


def load_evidence(path: Path) -> tuple[dict | None, str]:
    """
    Load `mandate-operator-evidence-v1` evidence written by the
    production-shaped runner's step 12.

    Returns `(evidence_dict, "ok")` on success, or `(None, reason)` for
    every failure mode the operator should see explicitly:

      "missing"       — file does not exist
      "unreadable"    — exists but cannot be opened
      "parse_failed"  — exists but is not valid JSON
      "wrong_schema"  — JSON parses but `schema` is not the expected id
    """
    if not path.is_file():
        return None, "missing"
    try:
        with path.open(encoding="utf-8") as fh:
            doc = json.load(fh)
    except OSError:
        return None, "unreadable"
    except json.JSONDecodeError:
        return None, "parse_failed"
    if doc.get("schema") != EXPECTED_EVIDENCE_SCHEMA:
        return None, "wrong_schema"
    return doc, "ok"


# Human-readable reason text per `load_evidence()` state. Used so the
# fallback panel surfaces the actual cause (unreadable / parse_failed /
# wrong_schema) instead of misdiagnosing every failure as "missing".
_EVIDENCE_REASON_TEXT = {
    "missing":       "evidence file missing",
    "unreadable":    "evidence file present but unreadable (filesystem error)",
    "parse_failed":  "evidence file present but JSON parse failed",
    "wrong_schema":  f"evidence file present but schema is not '{EXPECTED_EVIDENCE_SCHEMA}'",
}


def _evidence_reason_text(state: str | None) -> str:
    if state in _EVIDENCE_REASON_TEXT:
        return _EVIDENCE_REASON_TEXT[state]
    return f"evidence not loaded (state={state!r})"


def _evidence_unavailable_panel(panel_title: str, reason: str, expected_path: Path) -> str:
    """Honest 'evidence not gathered' placeholder — never a fake-OK pill."""
    return f"""
<section class="panel full">
<h2>{esc(panel_title)}</h2>
<div class="body">
<p class="empty">Real evidence not available — reason: <code>{esc(reason)}</code>. Expected at <code>{esc(str(expected_path))}</code>.</p>
<p class="empty">Generate it with:<br><code>bash demo-scripts/run-production-shaped-mock.sh</code> (writes step 12's transcript at the path above; schema <code>{esc(EXPECTED_EVIDENCE_SCHEMA)}</code>).</p>
</div>
</section>"""


def render_idempotency_panel(evidence: dict | None, evidence_state: str | None = None) -> str:
    if evidence is None:
        return _evidence_unavailable_panel(
            "PSM-A2 · HTTP Idempotency-Key safe-retry (4-case behaviour matrix)",
            _evidence_reason_text(evidence_state), DEFAULT_EVIDENCE,
        )
    idem = evidence.get("psm_a2_idempotency", {}) or {}
    c1 = idem.get("case_1_first_post", {}) or {}
    c2 = idem.get("case_2_cached_replay", {}) or {}
    c3 = idem.get("case_3_idempotency_conflict", {}) or {}
    c4 = idem.get("case_4_nonce_replay_with_new_key", {}) or {}
    return f"""
<section class="panel full">
<h2>PSM-A2 · HTTP Idempotency-Key safe-retry (4-case behaviour matrix)</h2>
<div class="body">
<dl class="kv">
<dt>case 1 — first POST (key=K1, body=B1)</dt><dd>{expect_pill(c1.get("http_status"), 200, label_ok="200", label_bad=str(c1.get("http_status")))} · audit_event=<code>{esc(c1.get("audit_event_id"))}</code> · decision=<code>{esc(c1.get("decision"))}</code></dd>
<dt>case 2 — same key + same body retry</dt><dd>{expect_pill(c2.get("http_status"), 200, label_ok="200", label_bad=str(c2.get("http_status")))} · byte_identical_to_case_1={expect_pill(c2.get("byte_identical_to_case_1"), True, label_ok="true", label_bad="false")}</dd>
<dt>case 3 — same key + mutated body</dt><dd>{expect_pill(c3.get("http_status"), 409, label_ok="409", label_bad=str(c3.get("http_status")))} · code=<code>{esc(c3.get("code"))}</code></dd>
<dt>case 4 — new key + same nonce</dt><dd>{expect_pill(c4.get("http_status"), 409, label_ok="409", label_bad=str(c4.get("http_status")))} · code=<code>{esc(c4.get("code"))}</code></dd>
</dl>
<p class="empty">Source: <code>demo-scripts/run-production-shaped-mock.sh</code> step 7 (real <code>mandate-server</code> on <code>127.0.0.1:18730</code>, persistent SQLite).</p>
</div>
</section>"""


def render_doctor_panel(evidence: dict | None, evidence_state: str | None = None) -> str:
    if evidence is None:
        return _evidence_unavailable_panel(
            "PSM-A5 · mandate doctor",
            _evidence_reason_text(evidence_state), DEFAULT_EVIDENCE,
        )
    doc = evidence.get("psm_a5_doctor", {}) or {}
    if doc.get("malformed"):
        report = doc.get("report") or {}
        return f"""
<section class="panel full">
<h2>PSM-A5 · mandate doctor</h2>
<div class="body">
<p class="empty">{pill("MALFORMED", "bad")} <code>mandate doctor --json</code> output did not parse. First 120 bytes: <code>{esc(report.get("_raw_first_120"))}</code></p>
</div>
</section>"""
    report = doc.get("report") or {}
    summary = doc.get("checks_summary") or {"ok": 0, "skip": 0, "fail": 0}
    overall = report.get("overall")
    rows_ok, rows_skip, rows_fail = [], [], []
    for c in (report.get("checks") or []):
        name = c.get("name", "?")
        status = c.get("status", "?")
        detail = c.get("detail") or c.get("reason") or ""
        row = f'<dt><code>{esc(name)}</code></dt><dd>{esc(detail)}</dd>'
        if status == "ok":
            rows_ok.append(row)
        elif status == "skip":
            rows_skip.append(row)
        elif status == "fail":
            rows_fail.append(row)
    def _group(label, kind, rows):
        if not rows:
            return ""
        body = "\n".join(rows)
        return f'<h3 class="group {kind}">{esc(label)} ({len(rows)})</h3><dl class="kv">{body}</dl>'
    return f"""
<section class="panel full">
<h2>PSM-A5 · mandate doctor</h2>
<div class="body">
<p class="empty">overall={expect_pill(overall, "ok", label_ok="ok", label_bad=str(overall))} · ok={summary.get("ok", 0)} skip={summary.get("skip", 0)} fail={summary.get("fail", 0)} · report_type=<code>{esc(report.get("report_type"))}</code></p>
{_group("ok", "ok", rows_ok)}
{_group("skip", "skip", rows_skip)}
{_group("fail", "fail", rows_fail)}
<p class="empty">Source: <code>mandate doctor --json</code> (production-shaped runner step 2, in-memory DB).</p>
</div>
</section>"""


def render_kms_panel(evidence: dict | None, evidence_state: str | None = None) -> str:
    if evidence is None:
        return _evidence_unavailable_panel(
            "PSM-A1.9 · Mock KMS keyring (mock, not production KMS)",
            _evidence_reason_text(evidence_state), DEFAULT_EVIDENCE,
        )
    kms = evidence.get("psm_a1_9_mock_kms", {}) or {}
    keys = kms.get("keys") or []
    if not keys:
        rows_html = '<p class="empty">No keys captured. Run <code>bash demo-scripts/run-production-shaped-mock.sh</code> step 3 against a fresh DB.</p>'
    else:
        rows = []
        for k in keys:
            rows.append(
                f'<tr><td><code>{esc(k.get("role"))}</code></td>'
                f'<td>v{esc(k.get("version"))}</td>'
                f'<td><code>{esc(k.get("key_id"))}</code></td>'
                f'<td><code>{esc(k.get("verifying_key_hex_prefix"))}…</code></td>'
                f'<td>{esc(k.get("created_at"))}</td>'
                f'<td>{mock_pill(k.get("mock"))}</td></tr>'
            )
        rows_html = (
            '<table class="evidence-table"><thead><tr>'
            '<th>role</th><th>version</th><th>key_id</th><th>verifying_key_hex (prefix)</th><th>created_at</th><th>mock</th>'
            '</tr></thead><tbody>'
            + "\n".join(rows) + '</tbody></table>'
        )
    return f"""
<section class="panel full">
<h2>PSM-A1.9 · Mock KMS keyring <span class="pill neutral">mock, not production KMS</span></h2>
<div class="body">
{rows_html}
<p class="empty">{esc(kms.get("_mock_label", ""))} Source: <code>mandate key list --mock --db &lt;path&gt;</code> (production-shaped runner step 3, post-rotate).</p>
</div>
</section>"""


def render_policy_panel(evidence: dict | None, evidence_state: str | None = None) -> str:
    if evidence is None:
        return _evidence_unavailable_panel(
            "PSM-A3 · Active policy lifecycle",
            _evidence_reason_text(evidence_state), DEFAULT_EVIDENCE,
        )
    p = evidence.get("psm_a3_active_policy")
    if not p:
        return f"""
<section class="panel full">
<h2>PSM-A3 · Active policy lifecycle</h2>
<div class="body">
<p class="empty">{pill("no active policy", "neutral")} The runner reached step 4 but no <code>active_policy</code> row was captured. <code>mandate policy current --db &lt;path&gt;</code> exits 3 on a fresh DB — that is the honest signal, not a fake "ok".</p>
</div>
</section>"""
    return f"""
<section class="panel full">
<h2>PSM-A3 · Active policy lifecycle</h2>
<div class="body">
<dl class="kv">
<dt>version</dt><dd><code>{esc(p.get("version"))}</code></dd>
<dt>policy_hash</dt><dd><code>{esc(p.get("policy_hash"))}</code></dd>
<dt>source</dt><dd><code>{esc(p.get("source"))}</code></dd>
<dt>activated_at</dt><dd><code>{esc(p.get("activated_at"))}</code></dd>
</dl>
<p class="empty">Local production-shaped lifecycle, not remote governance — there is no on-chain anchor, no consensus, no signing on activation; whoever opens the DB activates the policy. Source: <code>mandate policy current --db &lt;path&gt;</code> (production-shaped runner step 4 after <code>policy activate</code>).</p>
</div>
</section>"""


def render_checkpoint_panel(evidence: dict | None, evidence_state: str | None = None) -> str:
    if evidence is None:
        return _evidence_unavailable_panel(
            "PSM-A4 · Audit checkpoints — mock anchoring, NOT onchain",
            _evidence_reason_text(evidence_state), DEFAULT_EVIDENCE,
        )
    cp = evidence.get("psm_a4_audit_checkpoints", {}) or {}
    create = cp.get("create") or {}
    verify = cp.get("verify") or {}
    return f"""
<section class="panel full">
<h2>PSM-A4 · Audit checkpoints <span class="pill neutral">mock anchoring, NOT onchain</span></h2>
<div class="body">
<dl class="kv">
<dt>schema</dt><dd><code>{esc(create.get("schema"))}</code></dd>
<dt>sequence</dt><dd>{esc(create.get("sequence"))}</dd>
<dt>latest_event_id</dt><dd><code>{esc(create.get("latest_event_id"))}</code></dd>
<dt>latest_event_hash</dt><dd><code>{esc(create.get("latest_event_hash"))}</code></dd>
<dt>chain_digest</dt><dd><code>{esc(create.get("chain_digest"))}</code></dd>
<dt>mock_anchor_ref</dt><dd><code>{esc(create.get("mock_anchor_ref"))}</code></dd>
<dt>created_at</dt><dd><code>{esc(create.get("created_at"))}</code></dd>
<dt>structural_verify_ok</dt><dd>{expect_pill(verify.get("structural_verify_ok"), True, label_ok="true", label_bad="false")}</dd>
<dt>db_cross_check_ok</dt><dd>{expect_pill(verify.get("db_cross_check_ok"), True, label_ok="true", label_bad="false")}</dd>
<dt>verify result_ok</dt><dd>{expect_pill(verify.get("result_ok"), True, label_ok="true", label_bad="false")}</dd>
</dl>
<p class="empty">{esc(cp.get("_mock_anchor_label", "Mock anchoring, NOT onchain."))} Source: <code>mandate audit checkpoint create</code> + <code>verify</code> (production-shaped runner step 10).</p>
</div>
</section>"""


# --- bundle verification (optional) ----------------------------------------


def verify_bundle(bundle_path: Path, mandate_bin: Path | None) -> dict:
    """
    Run `mandate audit verify-bundle --path <bundle>` and parse its stdout.

    Returns one of:
      {"state": "not_provided"}                       — no --bundle flag
      {"state": "ok", "decision": …, "deny_code": …, "chain_length": …, "audit_event_id": …}
      {"state": "bundle_missing", "path": str}        — file does not exist
      {"state": "binary_missing", "path": str}        — `mandate` binary not found
      {"state": "verify_failed", "rc": int, "stderr": str, "stdout": str}
      {"state": "parse_failed", "stdout": str}        — verify exited 0 but output unrecognised
    """
    if not bundle_path.is_file():
        return {"state": "bundle_missing", "path": str(bundle_path)}
    binary = mandate_bin or (REPO_ROOT / "target" / "debug" / "mandate")
    if not binary.is_file():
        return {"state": "binary_missing", "path": str(binary)}
    # PR #24 P2 review: catch the full set of subprocess failures the
    # `mandate audit verify-bundle` invocation can raise. The original
    # only-FileNotFoundError handler crashed the whole build with a
    # traceback for `subprocess.TimeoutExpired` (slow verify) or any
    # `OSError` (e.g. PermissionError on a non-executable --mandate-bin),
    # even though this panel is explicitly designed to render a failure
    # state instead of aborting the render.
    try:
        proc = subprocess.run(
            [str(binary), "audit", "verify-bundle", "--path", str(bundle_path)],
            capture_output=True, text=True, timeout=30,
        )
    except FileNotFoundError:
        return {"state": "binary_missing", "path": str(binary)}
    except subprocess.TimeoutExpired as e:
        return {"state": "verify_failed", "rc": -1,
                "stderr": f"verify-bundle timed out after {e.timeout}s "
                          f"(binary may be hung).",
                "stdout": ""}
    except OSError as e:
        return {"state": "verify_failed", "rc": -1,
                "stderr": f"verify-bundle could not be invoked: {e}",
                "stdout": ""}
    if proc.returncode != 0:
        return {"state": "verify_failed", "rc": proc.returncode,
                "stderr": proc.stderr.strip(), "stdout": proc.stdout.strip()}
    line = proc.stdout.strip().splitlines()[0] if proc.stdout.strip() else ""
    m = re.match(
        r"^ok: bundle verified \(decision=([^,]+), deny_code=([^,]+), "
        r"chain_length=(\d+), audit_event_id=([^)]+)\)\s*$",
        line,
    )
    if not m:
        return {"state": "parse_failed", "stdout": proc.stdout.strip()}
    return {
        "state": "ok",
        "decision": m.group(1),
        "deny_code": None if m.group(2) == "None" else m.group(2),
        "chain_length": int(m.group(3)),
        "audit_event_id": m.group(4),
    }


def render_bundle_panel(result: dict) -> str:
    state = result.get("state")
    if state == "not_provided":
        return f"""
<section class="panel">
<h2>Audit-bundle verification</h2>
<div class="body">
<p class="empty">Bundle not provided. Pass <code>--bundle &lt;path&gt;</code> to render the verification result of a previously-exported <code>mandate.audit_bundle.v1</code> file. {pill("not provided", "neutral")}</p>
<p class="empty">Build a bundle from a live demo run with:
<br><code>./demo-agents/research-agent/run --scenario legit-x402 --storage-path /tmp/m.db --save-receipt /tmp/r.json</code>
<br><code>./target/debug/mandate audit export --receipt /tmp/r.json --db /tmp/m.db --receipt-pubkey &lt;hex&gt; --audit-pubkey &lt;hex&gt; --out /tmp/bundle.json</code></p>
</div>
</section>"""
    if state == "ok":
        return f"""
<section class="panel">
<h2>Audit-bundle verification</h2>
<div class="body">
<dl class="kv">
<dt>verify-bundle</dt><dd>{pill("ok", "ok")}</dd>
<dt>decision</dt><dd>{esc(result.get("decision"))}</dd>
<dt>deny_code</dt><dd>{esc(result.get("deny_code"))}</dd>
<dt>chain_length</dt><dd>{esc(result.get("chain_length"))}</dd>
<dt>audit_event_id</dt><dd>{esc(result.get("audit_event_id"))}</dd>
</dl>
</div>
</section>"""
    if state == "bundle_missing":
        return f"""
<section class="panel">
<h2>Audit-bundle verification</h2>
<div class="body">
<p class="empty">Bundle file not found: <code>{esc(result.get("path"))}</code> {pill("missing", "bad")}</p>
</div>
</section>"""
    if state == "binary_missing":
        return f"""
<section class="panel">
<h2>Audit-bundle verification</h2>
<div class="body">
<p class="empty">Cannot run <code>mandate audit verify-bundle</code> — the <code>mandate</code> binary was not found at <code>{esc(result.get("path"))}</code>. Run <code>cargo build --bin mandate</code> first. {pill("binary missing", "bad")}</p>
</div>
</section>"""
    if state == "verify_failed":
        return f"""
<section class="panel">
<h2>Audit-bundle verification</h2>
<div class="body">
<dl class="kv">
<dt>verify-bundle</dt><dd>{pill("FAIL (rc=" + str(result.get("rc")) + ")", "bad")}</dd>
<dt>stderr</dt><dd>{esc(result.get("stderr"))}</dd>
<dt>stdout</dt><dd>{esc(result.get("stdout"))}</dd>
</dl>
</div>
</section>"""
    # parse_failed
    return f"""
<section class="panel">
<h2>Audit-bundle verification</h2>
<div class="body">
<p class="empty">verify-bundle exited 0 but output was unrecognised. {pill("parse failed", "bad")}</p>
<dl class="kv"><dt>stdout</dt><dd>{esc(result.get("stdout"))}</dd></dl>
</div>
</section>"""


# --- main render -----------------------------------------------------------


# --- passport capsule (P2.2) ----------------------------------------------


def load_capsule(path: Path) -> tuple[dict | None, str]:
    """
    Load a `mandate.passport_capsule.v1` capsule emitted by Passport P2.1.

    Returns `(capsule_dict, "ok")` on success, or `(None, reason)` for every
    failure mode the operator console surfaces explicitly. The "tampered"
    state is the Python-side mirror of the simple cross-field invariants
    that `mandate passport verify` rejects, so a tampered capsule renders
    an honest placeholder instead of a fake-OK tile.
    """
    if not path.is_file():
        return None, "missing"
    try:
        with path.open(encoding="utf-8") as fh:
            doc = json.load(fh)
    except OSError:
        return None, "unreadable"
    except json.JSONDecodeError:
        return None, "parse_failed"
    if not isinstance(doc, dict) or doc.get("schema") != EXPECTED_CAPSULE_SCHEMA:
        return None, "wrong_schema"
    if _capsule_structural_violation(doc) is not None:
        return None, "tampered"
    return doc, "ok"


def _capsule_structural_violation(doc: dict) -> str | None:
    """Mirror of the simple cross-field invariants in
    `crates/mandate-core/src/passport.rs::verify_capsule`. Returns a one-line
    reason on failure or None on pass. Python does not verify cryptographic
    hashes — Rust `mandate passport verify` remains the source of truth."""
    decision = doc.get("decision") or {}
    execution = doc.get("execution") or {}
    audit = doc.get("audit") or {}
    checkpoint = audit.get("checkpoint") or {}
    request = doc.get("request") or {}
    policy = doc.get("policy") or {}
    receipt = decision.get("receipt") or {}

    if decision.get("result") == "deny":
        if execution.get("status") != "not_called":
            return "deny capsule must have execution.status='not_called'"
        if execution.get("execution_ref") is not None:
            return "deny capsule must not carry execution.execution_ref"

    if checkpoint.get("mock_anchor") is not True:
        return "audit.checkpoint.mock_anchor must be true (this build supports only mock anchoring)"

    anchor_ref = checkpoint.get("mock_anchor_ref")
    if not isinstance(anchor_ref, str) or not anchor_ref.startswith("local-mock-anchor-"):
        return "audit.checkpoint.mock_anchor_ref must start with 'local-mock-anchor-'"

    if execution.get("mode") == "live":
        live_evidence = execution.get("live_evidence")
        if not isinstance(live_evidence, dict) or not live_evidence:
            return "live mode requires non-empty execution.live_evidence"
        has_concrete = any(
            isinstance(live_evidence.get(k), str) and live_evidence.get(k)
            for k in ("transport", "response_ref", "block_ref")
        )
        if not has_concrete:
            return "live mode requires concrete transport/response_ref/block_ref"

    if request.get("request_hash") != receipt.get("request_hash"):
        return "request.request_hash must match decision.receipt.request_hash"
    if policy.get("policy_hash") != receipt.get("policy_hash"):
        return "policy.policy_hash must match decision.receipt.policy_hash"

    return None


_CAPSULE_REASON_TEXT = {
    "missing":      "capsule file missing",
    "unreadable":   "capsule file present but unreadable (filesystem error)",
    "parse_failed": "capsule file present but JSON parse failed",
    "wrong_schema":
        f"capsule file present but schema is not '{EXPECTED_CAPSULE_SCHEMA}'",
    "tampered":
        "capsule loaded and schema id matches, but a structural invariant "
        "failed (deny/execution / mock_anchor / live evidence / hash "
        "consistency). Run `mandate passport verify` for full detail.",
}


def _capsule_reason_text(state: str | None) -> str:
    return _CAPSULE_REASON_TEXT.get(state, f"capsule not loaded (state={state!r})")


def _short_hash(h: str | None, n: int = 12) -> str:
    if not isinstance(h, str):
        return ""
    return h[:n]


def _render_capsule_unavailable_tile(label: str, state: str,
                                     capsule_path: Path) -> str:
    reason = _capsule_reason_text(state)
    return f"""
<article class="capsule-tile unavailable">
<header>{esc(label)} · {pill("capsule evidence not gathered", "bad")} reason=<code>{esc(state)}</code></header>
<p class="empty">{esc(reason)}. Expected at <code>{esc(str(capsule_path))}</code> · schema <code>{esc(EXPECTED_CAPSULE_SCHEMA)}</code>.</p>
<p class="empty">Once Passport P2.1 emits a capsule into <code>demo-scripts/artifacts/</code>, this tile renders the captured proof. Never a fake-OK.</p>
</article>"""


def _render_capsule_tile(label: str, capsule: dict) -> str:
    agent = capsule.get("agent", {}) or {}
    records = agent.get("records", {}) or {}
    policy = capsule.get("policy", {}) or {}
    decision = capsule.get("decision", {}) or {}
    execution = capsule.get("execution", {}) or {}
    audit = capsule.get("audit", {}) or {}
    checkpoint = audit.get("checkpoint", {}) or {}
    verification = capsule.get("verification", {}) or {}

    # ENS records: render the raw key/value map, no invented values.
    if records:
        record_rows = "\n".join(
            f'<dt><code>{esc(k)}</code></dt><dd><code>{esc(v)}</code></dd>'
            for k, v in records.items()
        )
        records_block = f'<dl class="kv records">{record_rows}</dl>'
    else:
        records_block = '<p class="empty">No records published.</p>'

    # Decision: pill + matched_rule (allow) or pill + deny_code (deny).
    result = decision.get("result")
    if result == "allow":
        decision_block = (
            f'{pill("Allow", "ok")} · matched_rule=<code>{esc(decision.get("matched_rule"))}</code>'
        )
    elif result == "deny":
        decision_block = (
            f'{pill("Deny", "ok")} · deny_code=<code>{esc(decision.get("deny_code"))}</code>'
            + (f' · matched_rule=<code>{esc(decision.get("matched_rule"))}</code>'
               if decision.get("matched_rule") else "")
        )
    else:
        decision_block = pill("?", "bad") + f' · result=<code>{esc(result)}</code>'

    # Execution: render executor + mode + execution_ref, OR "not_called" pill on deny.
    if execution.get("status") == "not_called":
        execution_block = (
            f'<code>{esc(execution.get("executor"))}</code> '
            f'{pill("not_called", "neutral")} · execution_ref=<code>{esc(execution.get("execution_ref"))}</code>'
        )
    else:
        execution_block = (
            f'<code>{esc(execution.get("executor"))}</code> '
            f'{pill(execution.get("mode") or "?", "neutral")} · '
            f'execution_ref=<code>{esc(execution.get("execution_ref"))}</code> · '
            f'status=<code>{esc(execution.get("status"))}</code>'
        )

    # Audit checkpoint: explicit "mock anchoring, NOT onchain" pill when
    # mock_anchor is true (the only mode this build supports).
    if checkpoint.get("mock_anchor") is True:
        anchor_pill = pill("mock anchoring, NOT onchain", "neutral")
    else:
        anchor_pill = pill("?", "bad")

    # verification.live_claims: list of strings; render verbatim or "(none)".
    live_claims = verification.get("live_claims") or []
    if isinstance(live_claims, list) and live_claims:
        live_claims_block = ", ".join(f'<code>{esc(c)}</code>' for c in live_claims)
    else:
        live_claims_block = '<span class="na">(none)</span>'

    return f"""
<article class="capsule-tile {esc(result or "unknown")}">
<header>{esc(label)} · agent=<code>{esc(agent.get("ens_name"))}</code> · resolver=<code>{esc(agent.get("resolver"))}</code></header>
<dl class="kv">
<dt>ENS records</dt><dd>{records_block}</dd>
<dt>active policy</dt><dd>policy_hash=<code>{esc(_short_hash(policy.get("policy_hash")))}…</code> · version=<code>{esc(policy.get("policy_version"))}</code> · activated_at=<code>{esc(policy.get("activated_at"))}</code> · source=<code>{esc(policy.get("source"))}</code></dd>
<dt>decision</dt><dd>{decision_block}</dd>
<dt>execution</dt><dd>{execution_block}</dd>
<dt>audit checkpoint</dt><dd><code>{esc(checkpoint.get("mock_anchor_ref"))}</code> {anchor_pill} · sequence=<code>{esc(checkpoint.get("sequence"))}</code></dd>
<dt>verification</dt><dd>doctor_status={pill(verification.get("doctor_status") or "?", "neutral")} · offline_verifiable={expect_pill(verification.get("offline_verifiable"), True, label_ok="yes", label_bad="no")} · live_claims=[{live_claims_block}]</dd>
</dl>
</article>"""


def render_passport_panel(
    capsule_allow: dict | None, allow_state: str, allow_path: Path,
    capsule_deny: dict | None, deny_state: str, deny_path: Path,
) -> str:
    """
    Render the Passport-capsule panel. Stacks one or two tiles depending on
    which capsules loaded successfully:

      - Allow capsule loads ok       → allow tile rendered with values.
      - Allow capsule fails          → unavailable tile (placeholder).
      - Deny capsule loads ok        → deny tile rendered with values.
      - Deny capsule fails           → unavailable tile (placeholder).

    Both tiles always render — never silently dropped. During the P2.2
    DRAFT phase only the allow tile usually loads (against the on-main
    `test-corpus/passport/golden_001_allow_keeperhub_mock.json` fixture);
    the deny tile renders the honest placeholder until P2.1 emits a deny
    capsule into `demo-scripts/artifacts/`.
    """
    if capsule_allow is not None:
        allow_block = _render_capsule_tile("Allow capsule", capsule_allow)
    else:
        allow_block = _render_capsule_unavailable_tile(
            "Allow capsule", allow_state, allow_path,
        )
    if capsule_deny is not None:
        deny_block = _render_capsule_tile("Deny capsule", capsule_deny)
    else:
        deny_block = _render_capsule_unavailable_tile(
            "Deny capsule", deny_state, deny_path,
        )
    return f"""
<section class="panel full">
<h2>Passport capsule (P2.2)</h2>
<div class="body">
<p class="empty">Each tile below renders one <code>{esc(EXPECTED_CAPSULE_SCHEMA)}</code> capsule (allow path / deny path). When a capsule file is missing, malformed, or carries the wrong schema, that tile says so explicitly — never a fake-OK.</p>
{allow_block}
{deny_block}
</div>
</section>"""


def render(summary: dict, bundle_result: dict, evidence: dict | None,
           evidence_state: str | None = None,
           capsule_allow: dict | None = None, allow_state: str = "missing",
           allow_path: Path = DEFAULT_CAPSULE_ALLOW,
           capsule_deny: dict | None = None, deny_state: str = "missing",
           deny_path: Path = DEFAULT_CAPSULE_DENY) -> str:
    legit = summary.get("scenarios", {}).get("legit_x402", {}) or {}
    pi = summary.get("scenarios", {}).get("prompt_injection", {}) or {}
    nkp = summary.get("no_key_proof", {}) or {}
    nkp_checks = nkp.get("checks", {}) or {}
    audit = summary.get("audit_chain", {}) or {}
    commit = summary.get("demo_commit") or ""
    commit_short = commit[:12] if commit else ""

    timeline_legit = f"""
<article class="event allow">
<header><span class="pill ok">Allow</span> · legit-x402 · {esc(legit.get("matched_rule"))}</header>
<dl class="kv">
<dt>decision</dt><dd>{expect_pill(legit.get("decision"), "Allow")}</dd>
<dt>matched_rule</dt><dd>{esc(legit.get("matched_rule"))}</dd>
<dt>request_hash</dt><dd>{esc(legit.get("request_hash"))}</dd>
<dt>policy_hash</dt><dd>{esc(legit.get("policy_hash"))}</dd>
<dt>audit_event</dt><dd>{esc(legit.get("audit_event"))}</dd>
<dt>receipt_signature</dt><dd>{esc(legit.get("receipt_signature"))}</dd>
<dt>keeperhub_execution_ref</dt><dd>{esc(legit.get("keeperhub_execution_ref"))} {mock_pill(legit.get("keeperhub_mock"))}</dd>
</dl>
</article>"""

    timeline_pi = f"""
<article class="event deny">
<header><span class="pill ok">Deny</span> · prompt-injection · {esc(pi.get("deny_code"))}</header>
<dl class="kv">
<dt>decision</dt><dd>{expect_pill(pi.get("decision"), "Deny")}</dd>
<dt>deny_code</dt><dd>{esc(pi.get("deny_code"))}</dd>
<dt>matched_rule</dt><dd>{esc(pi.get("matched_rule"))}</dd>
<dt>request_hash</dt><dd>{esc(pi.get("request_hash"))}</dd>
<dt>policy_hash</dt><dd>{esc(pi.get("policy_hash"))}</dd>
<dt>audit_event</dt><dd>{esc(pi.get("audit_event"))}</dd>
<dt>receipt_signature</dt><dd>{esc(pi.get("receipt_signature"))}</dd>
<dt>denied_action_executed</dt><dd>{expect_pill(pi.get("denied_action_executed"), False, label_ok="false", label_bad="true")}</dd>
<dt>keeperhub_refused</dt><dd>{expect_pill(pi.get("keeperhub_refused"), True, label_ok="true", label_bad="false")}</dd>
</dl>
</article>"""

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Mandate · Operator Console</title>
<style>
*{{box-sizing:border-box}}
html,body{{margin:0;padding:0;background:#0e1116;color:#e6edf3;
  font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,"Liberation Mono","Courier New",monospace;
  font-size:13px;line-height:1.45}}
.wrap{{max-width:1180px;margin:0 auto;padding:18px}}
header.top{{border:1px solid #30363d;padding:12px 16px;margin-bottom:14px;background:#161b22}}
header.top h1{{margin:0 0 2px 0;font-size:16px;font-weight:600;letter-spacing:.2px}}
header.top .tag{{color:#8b949e;font-size:12px;margin-bottom:8px}}
header.top .meta{{color:#8b949e;font-size:11px;display:flex;flex-wrap:wrap;gap:14px}}
header.top .meta b{{color:#e6edf3;font-weight:500}}
.grid{{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:12px}}
.full{{grid-column:1 / -1}}
.panel{{border:1px solid #30363d;background:#161b22}}
.panel h2{{margin:0;padding:8px 14px;border-bottom:1px solid #30363d;
  font-size:11px;font-weight:600;letter-spacing:.6px;text-transform:uppercase;color:#8b949e}}
.panel .body{{padding:10px 14px}}
.kv{{display:grid;grid-template-columns:max-content 1fr;column-gap:14px;row-gap:4px;margin:0}}
.kv dt{{color:#8b949e;font-weight:400;white-space:nowrap}}
.kv dd{{margin:0;word-break:break-all}}
.event{{border:1px solid #30363d;background:#0e1116;padding:8px 12px;margin:8px 14px}}
.event header{{font-size:12px;color:#e6edf3;margin-bottom:6px;border-bottom:1px solid #21262d;padding-bottom:4px}}
.event.allow{{border-left:3px solid #2ea043}}
.event.deny{{border-left:3px solid #f85149}}
.pill{{display:inline-block;padding:1px 7px;border-radius:2px;font-size:11px;
  font-weight:600;letter-spacing:.3px;font-family:inherit}}
.pill.ok{{background:#0f2b15;color:#3fb950;border:1px solid #2ea043}}
.pill.bad{{background:#3a1216;color:#f85149;border:1px solid #f85149}}
.pill.neutral{{background:#21262d;color:#8b949e;border:1px solid #30363d}}
.pill.blocked{{background:#1f1810;color:#d29922;border:1px solid #d29922}}
.pill.pending{{background:#0d2030;color:#58a6ff;border:1px solid #58a6ff}}
.has-tip{{cursor:help;border-bottom:1px dotted #484f58}}
.na{{color:#484f58}}
.empty{{color:#8b949e;margin:0 0 6px 0}}
.evidence-table{{width:100%;border-collapse:collapse;font-size:12px}}
.evidence-table th,.evidence-table td{{padding:4px 8px;border-bottom:1px solid #30363d;text-align:left;vertical-align:top;word-break:break-all}}
.evidence-table th{{color:#8b949e;font-weight:500}}
.group{{margin:8px 0 2px 0;font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.5px}}
.group.ok{{color:#3fb950}}
.group.skip{{color:#d29922}}
.group.fail{{color:#f85149}}
.capsule-tile{{border:1px solid #30363d;background:#0e1116;padding:8px 12px;margin:8px 14px}}
.capsule-tile header{{font-size:12px;color:#e6edf3;margin-bottom:6px;border-bottom:1px solid #21262d;padding-bottom:4px}}
.capsule-tile.allow{{border-left:3px solid #2ea043}}
.capsule-tile.deny{{border-left:3px solid #f85149}}
.capsule-tile.unavailable{{border-left:3px solid #d29922}}
.records{{padding-left:14px;border-left:1px solid #21262d}}
footer{{margin-top:6px;color:#8b949e;font-size:11px;border-top:1px solid #30363d;padding-top:10px}}
footer code{{background:#21262d;padding:1px 4px;border-radius:2px}}
</style>
</head>
<body>
<div class="wrap">

<header class="top">
<h1>Mandate · Operator Console</h1>
<div class="tag">"Don't give your agent a wallet. Give it a mandate."</div>
<div class="meta">
<span><b>agent</b> {esc(summary.get("agent_id"))}</span>
<span><b>commit</b> <span class="has-tip" title="{esc(commit)}">{esc(commit_short)}</span></span>
<span><b>generated</b> {esc(summary.get("generated_at_iso"))}</span>
<span><b>schema</b> {esc(summary.get("schema"))}</span>
</div>
</header>

<section class="panel full">
<h2>Allow / deny timeline</h2>
{timeline_legit}
{timeline_pi}
</section>

<div class="grid">

<section class="panel">
<h2>No-key proof</h2>
<div class="body">
<dl class="kv">
<dt>status</dt><dd>{status_pill(nkp.get("status"))}</dd>
<dt>agent_source_signer_references</dt><dd>{esc(nkp_checks.get("agent_source_signer_references"))}</dd>
<dt>agent_cargo_signer_deps</dt><dd>{esc(nkp_checks.get("agent_cargo_signer_deps"))}</dd>
<dt>agent_key_material_files</dt><dd>{esc(nkp_checks.get("agent_key_material_files"))}</dd>
</dl>
</div>
</section>

<section class="panel">
<h2>Audit-chain tamper detection</h2>
<div class="body">
<dl class="kv">
<dt>structural_verify_accepts_tampered_actor</dt><dd>{expect_pill(audit.get("structural_verify_accepts_tampered_actor"), True, label_ok="true", label_bad="false")}</dd>
<dt>strict_hash_verify_rejects_tampered</dt><dd>{expect_pill(audit.get("strict_hash_verify_rejects_tampered"), True, label_ok="true", label_bad="false")}</dd>
</dl>
</div>
</section>

<section class="panel">
<h2>Mock sponsor disclosure</h2>
<div class="body">
<dl class="kv">
<dt>KeeperHub allow path</dt><dd>{esc(legit.get("keeperhub_execution_ref"))} {mock_pill(legit.get("keeperhub_mock"))}</dd>
<dt>KeeperHub deny path</dt><dd>{expect_pill(pi.get("keeperhub_refused"), True, label_ok="refused", label_bad="not refused")}</dd>
<dt>denied action executed</dt><dd>{expect_pill(pi.get("denied_action_executed"), False, label_ok="false", label_bad="true")}</dd>
<dt>ENS resolver</dt><dd>{pill("offline fixture", "neutral")}</dd>
<dt>Uniswap executor</dt><dd>{pill("local_mock", "neutral")}</dd>
</dl>
</div>
</section>

{render_bundle_panel(bundle_result)}

</div>

<section class="panel full">
<h2>Real-evidence panels (B2.v2)</h2>
<div class="body">
<p class="empty">Each panel below renders evidence captured by <code>demo-scripts/run-production-shaped-mock.sh</code>'s step 12 transcript (<code>{esc(EXPECTED_EVIDENCE_SCHEMA)}</code>). When the transcript is missing or unreadable, the panel says so explicitly — never a fake-OK.</p>
</div>
</section>

{render_idempotency_panel(evidence, evidence_state)}
{render_doctor_panel(evidence, evidence_state)}
{render_kms_panel(evidence, evidence_state)}
{render_policy_panel(evidence, evidence_state)}
{render_checkpoint_panel(evidence, evidence_state)}

{render_passport_panel(capsule_allow, allow_state, allow_path, capsule_deny, deny_state, deny_path)}

<footer>
Generated from <code>demo-scripts/artifacts/latest-demo-summary.json</code>.
KeeperHub and Uniswap executors are local mocks; ENS uses an offline resolver fixture; the dev signing seeds in <code>mandate-server</code> are deterministic public constants labelled <code>⚠ DEV ONLY ⚠</code>. Mocks remain explicitly labelled.
</footer>
</div>
</body>
</html>
"""


# --- main ------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Render operator-console/index.html from the demo's transcript JSON.",
    )
    parser.add_argument("--input", default=str(DEFAULT_INPUT),
                        help="Path to the demo summary JSON (default: %(default)s)")
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT),
                        help="Path to write the static HTML console (default: %(default)s)")
    parser.add_argument("--bundle", default=None,
                        help="Optional path to a `mandate.audit_bundle.v1` JSON file. "
                             "When set, runs `mandate audit verify-bundle` against it "
                             "and renders the parsed result. When unset, the console "
                             "renders an honest 'bundle not provided' state.")
    parser.add_argument("--mandate-bin", default=None,
                        help="Optional override for the `mandate` binary path "
                             "(default: target/debug/mandate). Only consulted when --bundle is set.")
    parser.add_argument("--evidence", default=str(DEFAULT_EVIDENCE),
                        help="Path to the operator-evidence transcript "
                             "(default: %(default)s). Written by the production-shaped "
                             "runner's step 12 with schema 'mandate-operator-evidence-v1'. "
                             "When missing/malformed/wrong-schema, the five real-evidence "
                             "panels render an explicit 'not gathered' placeholder.")
    parser.add_argument("--capsule-allow", default=str(DEFAULT_CAPSULE_ALLOW),
                        help="Path to the allow-path Passport capsule "
                             "(default: %(default)s). Schema "
                             "'mandate.passport_capsule.v1'. When "
                             "missing/malformed/wrong-schema, the allow tile "
                             "renders an explicit 'capsule evidence not "
                             "gathered' placeholder; never a fake-OK.")
    parser.add_argument("--capsule-deny", default=str(DEFAULT_CAPSULE_DENY),
                        help="Path to the deny-path Passport capsule "
                             "(default: %(default)s). Same schema and "
                             "placeholder semantics as --capsule-allow.")
    args = parser.parse_args()

    in_path = Path(args.input)
    out_path = Path(args.output)

    if not in_path.is_file():
        print(f"operator-console: input not found: {in_path}", file=sys.stderr)
        print("operator-console: run `bash demo-scripts/run-openagents-final.sh` first.", file=sys.stderr)
        return 1
    try:
        with in_path.open(encoding="utf-8") as fh:
            summary = json.load(fh)
    except (OSError, json.JSONDecodeError) as e:
        print(f"operator-console: failed to read {in_path}: {e}", file=sys.stderr)
        return 1

    actual_schema = summary.get("schema")
    if actual_schema != EXPECTED_SCHEMA:
        print(
            f"operator-console: unexpected schema {actual_schema!r} "
            f"(want {EXPECTED_SCHEMA!r}); refusing to render to avoid silent drift.",
            file=sys.stderr,
        )
        return 1

    if args.bundle is None:
        bundle_result = {"state": "not_provided"}
    else:
        bundle_result = verify_bundle(
            Path(args.bundle),
            Path(args.mandate_bin) if args.mandate_bin else None,
        )

    evidence, evidence_state = load_evidence(Path(args.evidence))
    if evidence_state != "ok":
        print(
            f"operator-console: evidence not loaded (state={evidence_state}); "
            f"the five real-evidence panels will render 'not gathered' placeholders. "
            f"Run `bash demo-scripts/run-production-shaped-mock.sh` to populate "
            f"{args.evidence}.",
            file=sys.stderr,
        )

    allow_path = Path(args.capsule_allow)
    deny_path = Path(args.capsule_deny)
    capsule_allow, allow_state = load_capsule(allow_path)
    capsule_deny, deny_state = load_capsule(deny_path)
    for label, path, state in (
        ("allow", allow_path, allow_state),
        ("deny", deny_path, deny_state),
    ):
        if state != "ok":
            print(
                f"operator-console: {label} capsule not loaded (state={state}); "
                f"that tile will render the 'capsule evidence not gathered' "
                f"placeholder. Expected at {path}.",
                file=sys.stderr,
            )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        render(
            summary, bundle_result, evidence, evidence_state,
            capsule_allow, allow_state, allow_path,
            capsule_deny, deny_state, deny_path,
        ),
        encoding="utf-8",
    )
    print(f"operator-console: wrote {out_path} ({out_path.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
