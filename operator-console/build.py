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
DEFAULT_OUTPUT = REPO_ROOT / "operator-console" / "index.html"
EXPECTED_SCHEMA = "mandate-demo-summary-v1"

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
    try:
        proc = subprocess.run(
            [str(binary), "audit", "verify-bundle", "--path", str(bundle_path)],
            capture_output=True, text=True, timeout=30,
        )
    except FileNotFoundError:
        return {"state": "binary_missing", "path": str(binary)}
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


def render(summary: dict, bundle_result: dict) -> str:
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
<h2>Backend backlog (placeholders, not implemented yet)</h2>
<div class="body">
<dl class="kv">
<dt>HTTP <code>Idempotency-Key</code> safe-retry</dt><dd>{pending_pill("PSM-A2", "demo-scripts/run-production-shaped-mock.sh")}</dd>
<dt>Active policy lifecycle (<code>mandate policy current</code> / <code>activate</code> / <code>diff</code>)</dt><dd>{blocked_pill("PSM-A3")}</dd>
<dt>Mock KMS CLI surface (<code>mandate key list --mock</code> / <code>key rotate --mock</code>) + storage</dt><dd>{pending_pill("PSM-A1.9", "demo-scripts/run-production-shaped-mock.sh")}</dd>
<dt>Audit checkpoints (<code>mandate audit checkpoint create</code> / <code>verify</code>)</dt><dd>{blocked_pill("PSM-A4")}</dd>
<dt>Operator readiness summary (<code>mandate doctor</code>)</dt><dd>{pending_pill("PSM-A5", "demo-scripts/run-production-shaped-mock.sh")}</dd>
</dl>
<p class="empty">Each placeholder lights up automatically once Developer A's corresponding PR lands and a follow-up B-side PR consumes the new value. The console renders honestly today: nothing is faked, nothing is hidden.</p>
</div>
</section>

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

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(render(summary, bundle_result), encoding="utf-8")
    print(f"operator-console: wrote {out_path} ({out_path.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
