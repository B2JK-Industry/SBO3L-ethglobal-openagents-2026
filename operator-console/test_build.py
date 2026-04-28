#!/usr/bin/env python3
# Render-regression coverage for operator-console/build.py.
#
# Drives build.py against operator-console/fixtures/operator-summary.json,
# asserts every required proof field renders, asserts the five backend-
# blocked panels render with their PSM-* backlog labels, asserts the mock
# disclosure pills are present, asserts the surface never invites JS or
# network (no <script>, no fetch(, no http(s):// URL), and asserts the
# rendered HTML feeds through `html.parser` without error.
#
# Stdlib only. Run from repo root: `python3 operator-console/test_build.py`.

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from html.parser import HTMLParser
from pathlib import Path

HERE = Path(__file__).resolve().parent
BUILD = HERE / "build.py"
FIXTURE = HERE / "fixtures" / "operator-summary.json"


def _ok(label: str, hint: str = "") -> None:
    suffix = f": {hint}" if hint else ""
    print(f"  ok   {label}{suffix}")


def _fail(label: str, hint: str = "") -> None:
    suffix = f": {hint}" if hint else ""
    print(f"  FAIL {label}{suffix}", file=sys.stderr)


def _truncate(s: str, n: int = 60) -> str:
    return s if len(s) <= n else s[:n] + "…"


def main() -> int:
    if not BUILD.is_file():
        _fail("build.py not found", str(BUILD))
        return 1
    if not FIXTURE.is_file():
        _fail("fixture not found", str(FIXTURE))
        return 1

    with FIXTURE.open(encoding="utf-8") as fh:
        summary = json.load(fh)

    # Drive build.py against the fixture into a temp file. Generated HTML must
    # never appear in the working tree from a test run.
    with tempfile.TemporaryDirectory() as tmp:
        out_path = Path(tmp) / "index.html"
        proc = subprocess.run(
            [sys.executable, str(BUILD),
             "--input", str(FIXTURE),
             "--output", str(out_path)],
            capture_output=True, text=True,
        )
        if proc.returncode != 0:
            _fail("build.py exited non-zero",
                  f"rc={proc.returncode} stderr={proc.stderr.strip()!r}")
            return 1
        if not out_path.is_file():
            _fail("expected output not produced", str(out_path))
            return 1
        html_text = out_path.read_text(encoding="utf-8")

    failures = 0

    # 1. Required content checks.
    legit = summary["scenarios"]["legit_x402"]
    pi = summary["scenarios"]["prompt_injection"]
    required = [
        ("page title",                       "Mandate · Operator Console"),
        ("agent_id",                         summary["agent_id"]),
        ("demo_commit (full 40-char SHA)",   summary["demo_commit"]),
        ("tagline",                          "Don't give your agent a wallet"),
        ("schema",                           summary["schema"]),
        # timeline header
        ("timeline section header",          "Allow / deny timeline"),
        # allow event
        ("legit decision pill",              ">Allow<"),
        ("legit matched_rule",               legit["matched_rule"]),
        ("legit request_hash",               legit["request_hash"]),
        ("legit policy_hash",                legit["policy_hash"]),
        ("legit audit_event",                legit["audit_event"]),
        ("legit receipt_signature",          legit["receipt_signature"]),
        ("legit keeperhub_execution_ref",    legit["keeperhub_execution_ref"]),
        # deny event
        ("deny decision pill",               ">Deny<"),
        ("deny deny_code",                   pi["deny_code"]),
        ("deny matched_rule",                pi["matched_rule"]),
        ("deny request_hash",                pi["request_hash"]),
        ("deny policy_hash",                 pi["policy_hash"]),
        ("deny audit_event",                 pi["audit_event"]),
        ("deny receipt_signature",           pi["receipt_signature"]),
        # boolean labels
        ("denied_action_executed label",     "denied_action_executed"),
        ("keeperhub_refused label",          "keeperhub_refused"),
        # no-key proof panel
        ("no-key proof header",              "No-key proof"),
        ("no-key proof PASS",                ">PASS<"),
        ("no-key source label",              "agent_source_signer_references"),
        ("no-key cargo label",               "agent_cargo_signer_deps"),
        ("no-key fixtures label",            "agent_key_material_files"),
        # audit chain panel
        ("audit chain header",               "Audit-chain tamper detection"),
        ("audit structural_verify label",    "structural_verify_accepts_tampered_actor"),
        ("audit strict_hash_verify label",   "strict_hash_verify_rejects_tampered"),
        # mock disclosure pills
        ("mock sponsor disclosure header",   "Mock sponsor disclosure"),
        ("mock pill",                        ">mock<"),
        ("offline fixture pill (ENS)",       ">offline fixture<"),
        ("local_mock pill (Uniswap)",        ">local_mock<"),
        # bundle panel — default 'not provided' state
        ("audit-bundle panel header",        "Audit-bundle verification"),
        ("bundle not provided pill",         ">not provided<"),
    ]
    print("== required content ==")
    for label, needle in required:
        if not isinstance(needle, str):
            _fail(label, f"non-string needle in fixture: {needle!r}")
            failures += 1
            continue
        if needle in html_text:
            _ok(label, _truncate(needle))
        else:
            _fail(label, f"needle {needle!r} not in HTML")
            failures += 1

    # 2. Backlog panels — each PSM-* item must surface with its label.
    # PSM-A2 is "pending" (backend merged on main; console panel landing in
    # B2.v2). The other four are still "blocked" (backend not merged yet).
    print("\n== backlog placeholders ==")
    blocked = [
        ("policy lifecycle panel",   "PSM-A3",   "policy lifecycle"),
        ("mock KMS CLI panel",       "PSM-A1.9", "Mock KMS CLI surface"),
        ("audit checkpoint panel",   "PSM-A4",   "Audit checkpoints"),
        ("doctor panel",             "PSM-A5",   "Operator readiness summary"),
    ]
    pending = [
        ("Idempotency-Key panel",    "PSM-A2",   "Idempotency-Key"),
    ]
    blocked_pill_pattern = re.compile(
        r'class="pill blocked"[^>]*>not implemented yet — backlog ',
        re.IGNORECASE,
    )
    pending_pill_pattern = re.compile(
        r'class="pill pending"[^>]*>PSM-[A-Z0-9.]+ merged · console panel landing in B2\.v2 ',
        re.IGNORECASE,
    )
    if not blocked_pill_pattern.search(html_text):
        _fail("blocked-pill class+text", "no <span class=\"pill blocked\"> ... 'not implemented yet — backlog'")
        failures += 1
    else:
        _ok("blocked-pill class+text present")
    if not pending_pill_pattern.search(html_text):
        _fail("pending-pill class+text", "no <span class=\"pill pending\"> ... 'PSM-X merged · console panel landing in B2.v2'")
        failures += 1
    else:
        _ok("pending-pill class+text present")
    for label, backlog_id, descr in blocked:
        if backlog_id in html_text and descr in html_text:
            _ok(f"{label} ({backlog_id}) [blocked]", descr)
        else:
            _fail(label, f"backlog {backlog_id} or descr {descr!r} not in HTML")
            failures += 1
    for label, backlog_id, descr in pending:
        if backlog_id in html_text and descr in html_text:
            _ok(f"{label} ({backlog_id}) [pending]", descr)
        else:
            _fail(label, f"backlog {backlog_id} or descr {descr!r} not in HTML")
            failures += 1
    # PSM-A2 must NOT appear inside a blocked-pill — that would lie about
    # the merged backend. Defensive negative assertion.
    a2_blocked_pattern = re.compile(
        r'class="pill blocked"[^>]*>not implemented yet — backlog\s*PSM-A2\b',
        re.IGNORECASE,
    )
    if a2_blocked_pattern.search(html_text):
        _fail("PSM-A2 must not be inside blocked-pill", "found a blocked-pill claiming PSM-A2 is unmerged")
        failures += 1
    else:
        _ok("PSM-A2 not inside any blocked-pill (avoids false 'unmerged' claim)")

    # 3. Forbidden surface (case-insensitive).
    print("\n== forbidden surface ==")
    forbidden = [
        ("no <script> element",            re.compile(r"<\s*script", re.IGNORECASE)),
        ("no fetch( call",                 re.compile(r"\bfetch\s*\(", re.IGNORECASE)),
        ("no external http(s):// URL",     re.compile(r"https?://", re.IGNORECASE)),
    ]
    for label, regex in forbidden:
        m = regex.search(html_text)
        if m:
            _fail(label, f"matched {m.group(0)!r} at offset {m.start()}")
            failures += 1
        else:
            _ok(label)

    # 4. Well-formedness.
    print("\n== html parse ==")
    try:
        HTMLParser().feed(html_text)
        _ok("html.parser consumed without error")
    except Exception as e:  # pragma: no cover
        _fail("html.parser", str(e))
        failures += 1

    # required + (blocked-pill class) + (pending-pill class) + blocked + pending
    # + (PSM-A2-not-in-blocked negative) + forbidden + (html.parser).
    total = len(required) + 1 + 1 + len(blocked) + len(pending) + 1 + len(forbidden) + 1
    print()
    if failures == 0:
        print(f"PASS: {total} checks ok")
        return 0
    print(f"FAIL: {failures} of {total} checks failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
