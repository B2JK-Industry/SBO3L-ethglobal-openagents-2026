#!/usr/bin/env python3
# Render-regression coverage for operator-console/build.py.
#
# Drives build.py against operator-console/fixtures/operator-summary.json
# (mandate-demo-summary-v1) and operator-console/fixtures/operator-evidence.json
# (mandate-operator-evidence-v1), asserts every required proof field renders,
# asserts each B2.v2 real-evidence panel surfaces values pulled directly from
# the evidence fixture, asserts PSM-A1.9/A2/A3/A4/A5 never appear inside a
# blocked/pending placeholder pill (would lie about merged backends), asserts
# the surface never invites JS or network (no <script>, no fetch(, no
# http(s):// URL), and asserts the rendered HTML feeds through `html.parser`
# without error.
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
EVIDENCE_FIXTURE = HERE / "fixtures" / "operator-evidence.json"


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
    if not EVIDENCE_FIXTURE.is_file():
        _fail("evidence fixture not found", str(EVIDENCE_FIXTURE))
        return 1

    with FIXTURE.open(encoding="utf-8") as fh:
        summary = json.load(fh)
    with EVIDENCE_FIXTURE.open(encoding="utf-8") as fh:
        evidence = json.load(fh)

    # Drive build.py against both fixtures into a temp file. Generated HTML
    # must never appear in the working tree from a test run.
    with tempfile.TemporaryDirectory() as tmp:
        out_path = Path(tmp) / "index.html"
        proc = subprocess.run(
            [sys.executable, str(BUILD),
             "--input", str(FIXTURE),
             "--evidence", str(EVIDENCE_FIXTURE),
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

    # 1. Required content checks (v1 panels — demo-summary).
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
        # B2.v2 evidence-panel section header
        ("evidence-panel section header",    "Real-evidence panels (B2.v2)"),
        ("evidence schema id",               "mandate-operator-evidence-v1"),
    ]
    print("== required content (v1 panels) ==")
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

    # 2. Required content checks (B2.v2 real-evidence panels — pulled from
    #    the evidence fixture so any drift in the renderer or fixture
    #    schema fails this test loudly).
    print("\n== required content (B2.v2 evidence panels) ==")
    idem = evidence["psm_a2_idempotency"]
    kms_keys = evidence["psm_a1_9_mock_kms"]["keys"]
    policy = evidence["psm_a3_active_policy"]
    cp_create = evidence["psm_a4_audit_checkpoints"]["create"]
    doctor_report = evidence["psm_a5_doctor"]["report"]
    doctor_summary = evidence["psm_a5_doctor"]["checks_summary"]

    evidence_required: list[tuple[str, str]] = [
        # PSM-A2 idempotency (4-case behaviour matrix)
        ("PSM-A2 panel header",                  "PSM-A2 · HTTP Idempotency-Key safe-retry"),
        ("PSM-A2 case-1 audit_event_id",         idem["case_1_first_post"]["audit_event_id"]),
        ("PSM-A2 case-1 decision",               idem["case_1_first_post"]["decision"]),
        ("PSM-A2 case-3 conflict code",          idem["case_3_idempotency_conflict"]["code"]),
        ("PSM-A2 case-4 nonce-replay code",      idem["case_4_nonce_replay_with_new_key"]["code"]),
        # PSM-A5 doctor (--json grouped ok/skip/fail)
        ("PSM-A5 panel header",                  "PSM-A5 · mandate doctor"),
        ("PSM-A5 report_type",                   doctor_report["report_type"]),
        ("PSM-A5 ok count",                      f"ok={doctor_summary['ok']}"),
        ("PSM-A5 skip count",                    f"skip={doctor_summary['skip']}"),
        ("PSM-A5 fail count",                    f"fail={doctor_summary['fail']}"),
        ("PSM-A5 migrations check name",         doctor_report["checks"][0]["name"]),
        ("PSM-A5 migrations detail",             doctor_report["checks"][0]["detail"]),
        ("PSM-A5 audit_chain skip reason",       doctor_report["checks"][3]["reason"]),
        # PSM-A1.9 mock KMS keyring
        ("PSM-A1.9 panel header",                "PSM-A1.9 · Mock KMS keyring"),
        ("PSM-A1.9 mock-not-prod pill",          "mock, not production KMS"),
        ("PSM-A1.9 v1 key_id",                   kms_keys[0]["key_id"]),
        ("PSM-A1.9 v1 verifying_key prefix",     kms_keys[0]["verifying_key_hex_prefix"]),
        ("PSM-A1.9 v2 key_id",                   kms_keys[1]["key_id"]),
        ("PSM-A1.9 v2 verifying_key prefix",     kms_keys[1]["verifying_key_hex_prefix"]),
        ("PSM-A1.9 audit-mock role",             kms_keys[0]["role"]),
        # PSM-A3 active policy lifecycle
        ("PSM-A3 panel header",                  "PSM-A3 · Active policy lifecycle"),
        ("PSM-A3 policy_hash",                   policy["policy_hash"]),
        ("PSM-A3 source",                        policy["source"]),
        ("PSM-A3 activated_at",                  policy["activated_at"]),
        # PSM-A4 audit checkpoints
        ("PSM-A4 panel header",                  "PSM-A4 · Audit checkpoints"),
        ("PSM-A4 mock-anchor pill",              "mock anchoring, NOT onchain"),
        ("PSM-A4 schema",                        cp_create["schema"]),
        ("PSM-A4 latest_event_id",               cp_create["latest_event_id"]),
        ("PSM-A4 latest_event_hash",             cp_create["latest_event_hash"]),
        ("PSM-A4 chain_digest",                  cp_create["chain_digest"]),
        ("PSM-A4 mock_anchor_ref",               cp_create["mock_anchor_ref"]),
    ]
    for label, needle in evidence_required:
        if not isinstance(needle, str):
            _fail(label, f"non-string needle in evidence fixture: {needle!r}")
            failures += 1
            continue
        if needle in html_text:
            _ok(label, _truncate(needle))
        else:
            _fail(label, f"needle {needle!r} not in HTML")
            failures += 1

    # 3. Negative assertions — PSM-A1.9/A2/A3/A4/A5 backends are merged on
    #    `main` and are now rendered as real evidence panels. They MUST
    #    NOT appear inside a blocked-pill OR a pending-pill anywhere in
    #    the HTML — that would lie about the production-shaped state
    #    (either claiming the backend is unmerged, or claiming the console
    #    panel still hasn't landed).
    print("\n== negative assertions (no PSM-A* in blocked/pending pills) ==")
    for backlog_id in ("PSM-A2", "PSM-A1.9", "PSM-A3", "PSM-A4", "PSM-A5"):
        blocked_pat = re.compile(
            rf'class="pill blocked"[^>]*>[^<]*\b{re.escape(backlog_id)}\b',
            re.IGNORECASE,
        )
        pending_pat = re.compile(
            rf'class="pill pending"[^>]*>[^<]*\b{re.escape(backlog_id)}\b',
            re.IGNORECASE,
        )
        if blocked_pat.search(html_text):
            _fail(f"{backlog_id} must not be inside blocked-pill",
                  f"found a blocked-pill claiming {backlog_id} is unmerged")
            failures += 1
        else:
            _ok(f"{backlog_id} not inside any blocked-pill")
        if pending_pat.search(html_text):
            _fail(f"{backlog_id} must not be inside pending-pill",
                  f"found a pending-pill claiming {backlog_id} console panel hasn't landed")
            failures += 1
        else:
            _ok(f"{backlog_id} not inside any pending-pill")

    # 4. Forbidden surface (case-insensitive).
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

    # 5. Well-formedness.
    print("\n== html parse ==")
    try:
        HTMLParser().feed(html_text)
        _ok("html.parser consumed without error")
    except Exception as e:  # pragma: no cover
        _fail("html.parser", str(e))
        failures += 1

    # required (v1) + evidence_required (v2) + 5×2 negative + forbidden + parse.
    total = len(required) + len(evidence_required) + (5 * 2) + len(forbidden) + 1
    print()
    if failures == 0:
        print(f"PASS: {total} checks ok")
        return 0
    print(f"FAIL: {failures} of {total} checks failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
