#!/usr/bin/env python3
# Generate a judge-facing, offline-only proof viewer at trust-badge/index.html
# from the structurally-deterministic JSON written by the demo runner's step
# 13 (demo-scripts/artifacts/latest-demo-summary.json).
#
# Why pre-rendered HTML instead of "static HTML + JS that fetches the JSON":
# browsers block fetch() from file:// due to same-origin, so a fetch-based
# viewer silently fails when a judge double-clicks the file. Pre-rendering
# every value into the HTML works directly from file:// with no local server.
#
# Stdlib only (json / html / argparse / pathlib). No JS, no external CSS, no
# external fonts. The HTML is one self-contained file.

from __future__ import annotations

import argparse
import html
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_INPUT = REPO_ROOT / "demo-scripts" / "artifacts" / "latest-demo-summary.json"
DEFAULT_OUTPUT = REPO_ROOT / "trust-badge" / "index.html"
EXPECTED_SCHEMA = "mandate-demo-summary-v1"


def esc(value) -> str:
    if value is None:
        return '<span class="na">—</span>'
    return html.escape(str(value), quote=True)


def pill(text, kind: str) -> str:
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
    # `keeperhub_mock: true` is the honest disclosure — render as a neutral
    # tag, not a "bad" colour. Live mode would render bad here only if it
    # were claimed without being implemented; this build never claims live.
    if value is True:
        return pill("mock", "neutral")
    if value is False:
        return pill("live", "neutral")
    return pill("?", "neutral")


def render(summary: dict) -> str:
    legit = summary.get("scenarios", {}).get("legit_x402", {}) or {}
    pi = summary.get("scenarios", {}).get("prompt_injection", {}) or {}
    nkp = summary.get("no_key_proof", {}) or {}
    nkp_checks = nkp.get("checks", {}) or {}
    audit = summary.get("audit_chain", {}) or {}

    commit = summary.get("demo_commit") or ""
    commit_short = commit[:12] if commit else ""

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Mandate · Trust Badge</title>
<style>
*{{box-sizing:border-box}}
html,body{{margin:0;padding:0;background:#0e1116;color:#e6edf3;
  font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,"Liberation Mono","Courier New",monospace;
  font-size:13px;line-height:1.45}}
.wrap{{max-width:1100px;margin:0 auto;padding:18px}}
header{{border:1px solid #30363d;padding:12px 16px;margin-bottom:14px;background:#161b22}}
header h1{{margin:0 0 2px 0;font-size:16px;font-weight:600;letter-spacing:.2px}}
header .tag{{color:#8b949e;font-size:12px;margin-bottom:8px}}
header .meta{{color:#8b949e;font-size:11px;display:flex;flex-wrap:wrap;gap:14px}}
header .meta b{{color:#e6edf3;font-weight:500}}
.grid{{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:12px}}
.panel{{border:1px solid #30363d;background:#161b22}}
.panel h2{{margin:0;padding:8px 14px;border-bottom:1px solid #30363d;
  font-size:11px;font-weight:600;letter-spacing:.6px;text-transform:uppercase;color:#8b949e}}
.panel .body{{padding:10px 14px}}
.kv{{display:grid;grid-template-columns:max-content 1fr;column-gap:14px;row-gap:4px;margin:0}}
.kv dt{{color:#8b949e;font-weight:400;white-space:nowrap}}
.kv dd{{margin:0;word-break:break-all}}
.scenario.allow h2 span.tag{{color:#3fb950}}
.scenario.deny  h2 span.tag{{color:#f85149}}
.pill{{display:inline-block;padding:1px 7px;border-radius:2px;font-size:11px;
  font-weight:600;letter-spacing:.3px;font-family:inherit}}
.pill.ok{{background:#0f2b15;color:#3fb950;border:1px solid #2ea043}}
.pill.bad{{background:#3a1216;color:#f85149;border:1px solid #f85149}}
.pill.neutral{{background:#21262d;color:#8b949e;border:1px solid #30363d}}
.has-tip{{cursor:help;border-bottom:1px dotted #484f58}}
.na{{color:#484f58}}
footer{{margin-top:6px;color:#8b949e;font-size:11px;border-top:1px solid #30363d;padding-top:10px}}
footer code{{background:#21262d;padding:1px 4px;border-radius:2px}}
</style>
</head>
<body>
<div class="wrap">
<header>
<h1>Mandate · Trust Badge</h1>
<div class="tag">"Don't give your agent a wallet. Give it a mandate."</div>
<div class="meta">
<span><b>agent</b> {esc(summary.get("agent_id"))}</span>
<span><b>commit</b> <span class="has-tip" title="{esc(commit)}">{esc(commit_short)}</span></span>
<span><b>generated</b> {esc(summary.get("generated_at_iso"))}</span>
<span><b>schema</b> {esc(summary.get("schema"))}</span>
</div>
</header>

<div class="grid">

<section class="panel scenario allow">
<h2><span class="tag">Allow</span> · legit-x402</h2>
<div class="body">
<dl class="kv">
<dt>decision</dt><dd>{expect_pill(legit.get("decision"), "Allow")}</dd>
<dt>matched_rule</dt><dd>{esc(legit.get("matched_rule"))}</dd>
<dt>request_hash</dt><dd>{esc(legit.get("request_hash"))}</dd>
<dt>policy_hash</dt><dd>{esc(legit.get("policy_hash"))}</dd>
<dt>audit_event</dt><dd>{esc(legit.get("audit_event"))}</dd>
<dt>receipt_signature</dt><dd>{esc(legit.get("receipt_signature"))}</dd>
<dt>keeperhub_execution_ref</dt><dd>{esc(legit.get("keeperhub_execution_ref"))} {mock_pill(legit.get("keeperhub_mock"))}</dd>
</dl>
</div>
</section>

<section class="panel scenario deny">
<h2><span class="tag">Deny</span> · prompt-injection</h2>
<div class="body">
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
</div>
</section>

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
<h2>Audit chain tamper detection</h2>
<div class="body">
<dl class="kv">
<dt>structural_verify_accepts_tampered_actor</dt><dd>{expect_pill(audit.get("structural_verify_accepts_tampered_actor"), True, label_ok="true", label_bad="false")}</dd>
<dt>strict_hash_verify_rejects_tampered</dt><dd>{expect_pill(audit.get("strict_hash_verify_rejects_tampered"), True, label_ok="true", label_bad="false")}</dd>
</dl>
</div>
</section>

</div>

<footer>
Generated from <code>demo-scripts/artifacts/latest-demo-summary.json</code>.
KeeperHub and Uniswap executors in this demo are <span class="pill neutral">mock</span>; ENS uses an offline resolver fixture. Mocks remain explicitly labelled — this viewer never silently upgrades a mock to a live claim.
</footer>
</div>
</body>
</html>
"""


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Render trust-badge/index.html from the demo's transcript JSON.",
    )
    parser.add_argument("--input", default=str(DEFAULT_INPUT),
                        help="Path to the demo summary JSON (default: %(default)s)")
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT),
                        help="Path to write the static HTML viewer (default: %(default)s)")
    args = parser.parse_args()

    in_path = Path(args.input)
    out_path = Path(args.output)

    if not in_path.is_file():
        print(f"trust-badge: input not found: {in_path}", file=sys.stderr)
        print("trust-badge: run `bash demo-scripts/run-openagents-final.sh` first.", file=sys.stderr)
        return 1
    try:
        with in_path.open(encoding="utf-8") as fh:
            summary = json.load(fh)
    except (OSError, json.JSONDecodeError) as e:
        print(f"trust-badge: failed to read {in_path}: {e}", file=sys.stderr)
        return 1

    actual_schema = summary.get("schema")
    if actual_schema != EXPECTED_SCHEMA:
        print(
            f"trust-badge: unexpected schema {actual_schema!r} "
            f"(want {EXPECTED_SCHEMA!r}); refusing to render to avoid silent drift.",
            file=sys.stderr,
        )
        return 1

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(render(summary), encoding="utf-8")
    print(f"trust-badge: wrote {out_path} ({out_path.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
