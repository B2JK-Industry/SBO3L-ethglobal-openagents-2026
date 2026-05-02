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
EXPECTED_SCHEMA = "sbo3l-demo-summary-v1"
# Passport capsule (P2.2). Default points at the post-P2.1 runtime artifact;
# during the P2.2 DRAFT phase (P2.1 not yet merged) the runner does not emit
# this file, so the trust-badge falls through to the honest "capsule evidence
# not gathered" placeholder. Tests pin the render path against the on-main
# golden fixture in `test-corpus/passport/`.
DEFAULT_CAPSULE = REPO_ROOT / "demo-scripts" / "artifacts" / "passport-allow.json"
# Accept both v1 (Passport P2.1) and v2 (P6.1 schema bump that added
# execution.executor_evidence). The viewer's read paths are forward-compatible.
EXPECTED_CAPSULE_SCHEMA = "sbo3l.passport_capsule.v1"
ACCEPTED_CAPSULE_SCHEMAS = {"sbo3l.passport_capsule.v1", "sbo3l.passport_capsule.v2"}


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


# --- passport capsule (P2.2) ----------------------------------------------


def load_capsule(path: Path) -> tuple[dict | None, str]:
    """
    Load a `sbo3l.passport_capsule.v1` capsule emitted by Passport P2.1.

    Returns `(capsule_dict, "ok")` on success, or `(None, reason)` for every
    failure mode the trust-badge surfaces explicitly to the operator:

      "missing"       — file does not exist
      "unreadable"    — exists but cannot be opened
      "parse_failed"  — exists but is not valid JSON
      "wrong_schema"  — JSON parses but `schema` is not the expected id
      "tampered"      — JSON parses, schema id matches, but a cross-field
                        structural invariant fails (deny→no execution,
                        mock_anchor must be true with local- prefix, live
                        mode requires concrete evidence, request/policy
                        hash internal-consistency). Mirrors the simple
                        invariants enforced by `sbo3l passport verify`
                        in `crates/sbo3l-core/src/passport.rs` so that
                        a tampered capsule renders the honest placeholder
                        instead of a fake-OK summary tile.
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
    if not isinstance(doc, dict) or doc.get("schema") not in ACCEPTED_CAPSULE_SCHEMAS:
        return None, "wrong_schema"
    if _capsule_structural_violation(doc) is not None:
        return None, "tampered"
    return doc, "ok"


def _capsule_structural_violation(doc: dict) -> str | None:
    """
    Returns a one-line reason string when the capsule fails one of the
    cross-field invariants the Rust verifier enforces; returns None when
    every invariant the Python build can check from the JSON alone passes.

    This is intentionally a SUBSET of `verify_capsule` — Python does not
    verify cryptographic hashes, only the structural invariants that
    a tampered fixture can be expected to break. The Rust binary remains
    the source of truth for full verification (`sbo3l passport verify`).
    """
    decision = doc.get("decision") or {}
    execution = doc.get("execution") or {}
    audit = doc.get("audit") or {}
    checkpoint = audit.get("checkpoint") or {}
    request = doc.get("request") or {}
    policy = doc.get("policy") or {}
    receipt = decision.get("receipt") or {}

    # Invariant: deny ⇒ no execution.
    if decision.get("result") == "deny":
        if execution.get("status") != "not_called":
            return "deny capsule must have execution.status='not_called'"
        if execution.get("execution_ref") is not None:
            return "deny capsule must not carry execution.execution_ref"

    # Invariant: this build only emits mock-anchored checkpoints; live
    # onchain anchoring is target/future. A capsule claiming
    # mock_anchor=false is either lying or generated by a future build.
    if checkpoint.get("mock_anchor") is not True:
        return "audit.checkpoint.mock_anchor must be true (this build supports only mock anchoring)"

    # Invariant: mock anchor reference must use the documented local prefix.
    anchor_ref = checkpoint.get("mock_anchor_ref")
    if not isinstance(anchor_ref, str) or not anchor_ref.startswith("local-mock-anchor-"):
        return "audit.checkpoint.mock_anchor_ref must start with 'local-mock-anchor-'"

    # Invariant: live mode ⇒ concrete live evidence.
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

    # Invariant: request_hash and policy_hash internal consistency.
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
        "consistency). Run `sbo3l passport verify` for full detail.",
}


def _capsule_reason_text(state: str | None) -> str:
    return _CAPSULE_REASON_TEXT.get(state, f"capsule not loaded (state={state!r})")


def _decision_label(decision: dict) -> str:
    """One-line decision summary: 'allow · matched_rule' or 'deny · deny_code'."""
    result = decision.get("result")
    if result == "allow":
        return f'{pill("Allow", "ok")} · matched_rule=<code>{esc(decision.get("matched_rule"))}</code>'
    if result == "deny":
        return f'{pill("Deny", "ok")} · deny_code=<code>{esc(decision.get("deny_code"))}</code>'
    return pill("?", "bad") + f' · result=<code>{esc(result)}</code>'


def _execution_label(execution: dict) -> str:
    """Executor + mode + execution_ref, or explicit not-called pill on deny path."""
    status = execution.get("status")
    executor = execution.get("executor")
    mode = execution.get("mode")
    ref = execution.get("execution_ref")
    if status == "not_called":
        return f'<code>{esc(executor)}</code> {pill("not called", "neutral")} · status=<code>{esc(status)}</code>'
    return (
        f'<code>{esc(executor)}</code> {pill(mode or "?", "neutral")} · '
        f'execution_ref=<code>{esc(ref)}</code> · status=<code>{esc(status)}</code>'
    )


def _mock_anchor_pill(checkpoint: dict) -> str:
    """Always-explicit 'mock anchoring, NOT onchain' pill when mock_anchor is true."""
    if checkpoint.get("mock_anchor") is True:
        return pill("mock anchoring, NOT onchain", "neutral")
    return pill("?", "bad")


def render_capsule_panel(capsule: dict | None, state: str,
                         capsule_path: Path) -> str:
    if capsule is None:
        reason = _capsule_reason_text(state)
        return f"""
<section class="panel">
<h2>Passport capsule</h2>
<div class="body">
<p class="empty">{pill("capsule evidence not gathered", "bad")} reason=<code>{esc(state)}</code> ({esc(reason)})</p>
<p class="empty">Expected at <code>{esc(str(capsule_path))}</code> · schema <code>{esc(EXPECTED_CAPSULE_SCHEMA)}</code>. Once Passport P2.1 emits a capsule into <code>demo-scripts/artifacts/</code>, this tile renders the captured proof.</p>
</div>
</section>"""

    agent = capsule.get("agent", {}) or {}
    decision = capsule.get("decision", {}) or {}
    execution = capsule.get("execution", {}) or {}
    audit = capsule.get("audit", {}) or {}
    checkpoint = audit.get("checkpoint", {}) or {}
    verification = capsule.get("verification", {}) or {}
    return f"""
<section class="panel">
<h2>Passport capsule</h2>
<div class="body">
<dl class="kv">
<dt>agent</dt><dd><code>{esc(agent.get("ens_name"))}</code> · resolver=<code>{esc(agent.get("resolver"))}</code></dd>
<dt>decision</dt><dd>{_decision_label(decision)}</dd>
<dt>execution</dt><dd>{_execution_label(execution)}</dd>
<dt>audit checkpoint</dt><dd><code>{esc(checkpoint.get("mock_anchor_ref"))}</code> {_mock_anchor_pill(checkpoint)}</dd>
<dt>offline_verifiable</dt><dd>{expect_pill(verification.get("offline_verifiable"), True, label_ok="yes", label_bad="no")}</dd>
</dl>
</div>
</section>"""


def render(summary: dict, capsule_state: tuple[dict | None, str],
           capsule_path: Path) -> str:
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
<title>SBO3L · Trust Badge</title>
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
<h1>SBO3L · Trust Badge</h1>
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

{render_capsule_panel(capsule_state[0], capsule_state[1], capsule_path)}

</div>

<footer>
Generated from <code>demo-scripts/artifacts/latest-demo-summary.json</code> plus an optional <code>{esc(EXPECTED_CAPSULE_SCHEMA)}</code> capsule. KeeperHub and Uniswap executors in this demo are <span class="pill neutral">mock</span>; ENS uses an offline resolver fixture. Mocks remain explicitly labelled — this viewer never silently upgrades a mock to a live claim.
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
    parser.add_argument("--capsule", default=str(DEFAULT_CAPSULE),
                        help="Path to a `sbo3l.passport_capsule.v1` JSON "
                             "(default: %(default)s). When missing/malformed/"
                             "wrong-schema the capsule tile renders an "
                             "explicit 'capsule evidence not gathered' "
                             "placeholder; never a fake-OK.")
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

    capsule_path = Path(args.capsule)
    capsule, capsule_state = load_capsule(capsule_path)
    if capsule_state != "ok":
        print(
            f"trust-badge: capsule not loaded (state={capsule_state}); "
            f"the Passport capsule tile will render the 'capsule evidence "
            f"not gathered' placeholder. Expected at {capsule_path}.",
            file=sys.stderr,
        )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        render(summary, (capsule, capsule_state), capsule_path),
        encoding="utf-8",
    )
    print(f"trust-badge: wrote {out_path} ({out_path.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
