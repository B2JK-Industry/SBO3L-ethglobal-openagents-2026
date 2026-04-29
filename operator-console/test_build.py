#!/usr/bin/env python3
# Render-regression coverage for operator-console/build.py.
#
# Drives build.py against operator-console/fixtures/operator-summary.json
# (sbo3l-demo-summary-v1) and operator-console/fixtures/operator-evidence.json
# (sbo3l-operator-evidence-v1), asserts every required proof field renders,
# asserts each B2.v2 real-evidence panel surfaces values pulled directly from
# the evidence fixture, asserts PSM-A1.9/A2/A3/A4/A5 never appear inside a
# blocked/pending placeholder pill (would lie about merged backends), asserts
# the surface never invites JS or network (no <script>, no fetch(, no
# http(s):// URL), and asserts the rendered HTML feeds through `html.parser`
# without error.
#
# Stdlib only. Run from repo root: `python3 operator-console/test_build.py`.

from __future__ import annotations

import copy
import json
import re
import subprocess
import sys
import tempfile
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlparse

HERE = Path(__file__).resolve().parent
BUILD = HERE / "build.py"
FIXTURE = HERE / "fixtures" / "operator-summary.json"
EVIDENCE_FIXTURE = HERE / "fixtures" / "operator-evidence.json"
# Passport P2.2 (post-P2.1 rebase): prefer the runtime capsules emitted by
# the production-shaped runner's step 10b (P2.1 #44). When both runtime
# artifacts are present, the deny tile is exercised against the REAL deny
# capsule and the in-memory deny synthesiser is no longer needed. When the
# runtime artifacts are absent (e.g. CI, which does not run the runner
# before this test), fall back to the on-main golden allow fixture plus
# the in-memory deny synthesis. Assertion logic uses values read FROM the
# loaded capsules, so either source produces a consistent test pass.
RUNTIME_CAPSULE_ALLOW = HERE.parent / "demo-scripts" / "artifacts" / "passport-allow.json"
RUNTIME_CAPSULE_DENY = HERE.parent / "demo-scripts" / "artifacts" / "passport-deny.json"
GOLDEN_CAPSULE_ALLOW = HERE.parent / "test-corpus" / "passport" / "golden_001_allow_keeperhub_mock.json"
CAPSULE_TAMPERED_FIXTURE = HERE.parent / "test-corpus" / "passport" / "tampered_002_mock_anchor_marked_live.json"

if RUNTIME_CAPSULE_ALLOW.is_file() and RUNTIME_CAPSULE_DENY.is_file():
    CAPSULE_ALLOW_FIXTURE = RUNTIME_CAPSULE_ALLOW
    CAPSULE_DENY_FIXTURE: Path | None = RUNTIME_CAPSULE_DENY
    CAPSULE_SOURCE = "runtime artifacts (demo-scripts/artifacts/passport-{allow,deny}.json)"
else:
    CAPSULE_ALLOW_FIXTURE = GOLDEN_CAPSULE_ALLOW
    CAPSULE_DENY_FIXTURE = None
    CAPSULE_SOURCE = "on-main golden allow fixture + in-memory deny synthesiser"

SAFE_HOSTS_EXACT = frozenset({
    "127.0.0.1",
    "localhost",
    "schemas.sbo3l.dev",
    "example.com",
    "example.net",
    "example.org",
})
SAFE_HOST_SUFFIXES = (
    ".invalid",
    ".example",
    ".test",
    ".localhost",
    ".example.com",
    ".example.net",
    ".example.org",
)
URL_PATTERN = re.compile(r"https?://[^\s\"'<>]+", re.IGNORECASE)


def _url_is_safe(url: str) -> bool:
    try:
        parsed = urlparse(url)
    except ValueError:
        return False
    host = (parsed.hostname or "").lower()
    if not host:
        return False
    if host in SAFE_HOSTS_EXACT:
        return True
    return any(host.endswith(suffix) for suffix in SAFE_HOST_SUFFIXES)


def _synth_deny_capsule(allow: dict) -> dict:
    """Construct a deny-path capsule by mutating the allow golden in-memory.
    Used during the P2.2 DRAFT phase before P2.1 emits a runtime
    `passport-deny.json`. Must produce a structurally-valid capsule (matches
    every cross-field invariant in load_capsule)."""
    deny = copy.deepcopy(allow)
    deny["decision"]["result"] = "deny"
    deny["decision"]["matched_rule"] = "deny-unknown-provider"
    deny["decision"]["deny_code"] = "policy.deny_unknown_provider"
    deny["decision"]["receipt"]["decision"] = "deny"
    deny["decision"]["receipt"]["deny_code"] = "policy.deny_unknown_provider"
    deny["decision"]["receipt"]["execution_ref"] = None
    deny["execution"]["status"] = "not_called"
    deny["execution"]["execution_ref"] = None
    return deny


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

    if not CAPSULE_ALLOW_FIXTURE.is_file():
        _fail("passport allow capsule fixture not found",
              str(CAPSULE_ALLOW_FIXTURE))
        return 1
    with CAPSULE_ALLOW_FIXTURE.open(encoding="utf-8") as fh:
        capsule_allow = json.load(fh)
    print(f"  note: capsule source = {CAPSULE_SOURCE}")

    # Drive build.py against both fixtures into a temp file. Generated HTML
    # must never appear in the working tree from a test run. The Passport
    # P2.2 panel is exercised against (a) the runtime allow + deny capsules
    # emitted by the production-shaped runner's step 10b, OR (b) the
    # on-main golden allow fixture plus an in-memory synthesised deny
    # capsule when runtime artifacts are absent (CI fallback).
    with tempfile.TemporaryDirectory() as tmp:
        if CAPSULE_DENY_FIXTURE is not None:
            with CAPSULE_DENY_FIXTURE.open(encoding="utf-8") as fh:
                capsule_deny = json.load(fh)
            deny_path = CAPSULE_DENY_FIXTURE
        else:
            capsule_deny = _synth_deny_capsule(capsule_allow)
            deny_path = Path(tmp) / "synth-deny-capsule.json"
            deny_path.write_text(json.dumps(capsule_deny), encoding="utf-8")
        out_path = Path(tmp) / "index.html"
        proc = subprocess.run(
            [sys.executable, str(BUILD),
             "--input", str(FIXTURE),
             "--evidence", str(EVIDENCE_FIXTURE),
             "--capsule-allow", str(CAPSULE_ALLOW_FIXTURE),
             "--capsule-deny", str(deny_path),
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
        ("page title",                       "SBO3L · Operator Console"),
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
        ("evidence schema id",               "sbo3l-operator-evidence-v1"),
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
        ("PSM-A5 panel header",                  "PSM-A5 · sbo3l doctor"),
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

    # 3b. Passport-capsule panel (P2.2). Allow tile renders from the
    #     on-main golden fixture; deny tile renders from an in-memory
    #     synthesised deny capsule with `execution.status == not_called`.
    print("\n== passport capsule panel (P2.2) ==")
    capsule_required = [
        ("Passport panel header",            "Passport capsule (P2.2)"),
        ("schema id reference",              "sbo3l.passport_capsule.v1"),
        ("Allow tile header",                "Allow capsule"),
        ("Deny tile header",                 "Deny capsule"),
        ("agent ens_name (allow tile)",      capsule_allow["agent"]["ens_name"]),
        ("agent resolver source",            capsule_allow["agent"]["resolver"]),
        ("ENS record key sbo3l:policy_hash", "sbo3l:policy_hash"),
        ("policy_hash short prefix",         capsule_allow["policy"]["policy_hash"][:12]),
        ("policy source (allow)",            capsule_allow["policy"]["source"]),
        ("Allow decision pill in tile",      ">Allow<"),
        ("matched_rule (allow)",             capsule_allow["decision"]["matched_rule"]),
        ("Deny decision pill in tile",       ">Deny<"),
        ("deny_code (deny tile)",            capsule_deny["decision"]["deny_code"]),
        ("execution executor",               capsule_allow["execution"]["executor"]),
        ("execution_ref (allow)",            capsule_allow["execution"]["execution_ref"]),
        ("not_called pill (deny tile)",      ">not_called<"),
        ("mock_anchor_ref (allow)",          capsule_allow["audit"]["checkpoint"]["mock_anchor_ref"]),
        ("mock-anchor pill",                 "mock anchoring, NOT onchain"),
        ("verification doctor_status",       "doctor_status"),
        ("offline_verifiable yes pill",      ">yes<"),
        ("live_claims label",                "live_claims"),
    ]
    for label, needle in capsule_required:
        if not isinstance(needle, str):
            _fail(label, f"non-string needle from capsule: {needle!r}")
            failures += 1
            continue
        if needle in html_text:
            _ok(label, _truncate(needle))
        else:
            _fail(label, f"needle {needle!r} not in HTML")
            failures += 1

    # 4. Forbidden surface (case-insensitive). The Passport panel renders
    #    URL values from agent records (e.g. `sbo3l:mcp_endpoint`), so
    #    the URL check uses the safe-host allowlist (RFC 2606/6761
    #    reserved + 127.0.0.1 + schemas.sbo3l.dev) instead of refusing
    #    every http(s):// occurrence.
    print("\n== forbidden surface ==")
    forbidden = [
        ("no <script> element",            re.compile(r"<\s*script", re.IGNORECASE)),
        ("no fetch( call",                 re.compile(r"\bfetch\s*\(", re.IGNORECASE)),
    ]
    for label, regex in forbidden:
        m = regex.search(html_text)
        if m:
            _fail(label, f"matched {m.group(0)!r} at offset {m.start()}")
            failures += 1
        else:
            _ok(label)
    unsafe_urls = [u for u in URL_PATTERN.findall(html_text) if not _url_is_safe(u)]
    if unsafe_urls:
        _fail("no unsafe http(s):// URL", f"found unsafe URLs: {unsafe_urls[:3]}")
        failures += 1
    else:
        _ok("no unsafe http(s):// URL (only RFC 2606 + schemas.sbo3l.dev)")

    # 5. Well-formedness.
    print("\n== html parse ==")
    try:
        HTMLParser().feed(html_text)
        _ok("html.parser consumed without error")
    except Exception as e:  # pragma: no cover
        _fail("html.parser", str(e))
        failures += 1

    # 6. Fallback-state propagation — for each non-ok evidence-load state,
    #    every fallback panel must surface the SPECIFIC reason instead of
    #    silently misdiagnosing it as "missing". Regression coverage for
    #    the P1 review finding that load_evidence() returned the state
    #    but the renderer dropped it on the floor.
    print("\n== fallback-state propagation ==")
    fallback_cases: list[tuple[str, str, str]] = []
    with tempfile.TemporaryDirectory() as tmp:
        # State 1: missing → file does not exist.
        missing_path = Path(tmp) / "no-such-file.json"
        fallback_cases.append(("missing", str(missing_path),
                               "evidence file missing"))
        # State 2: parse_failed → file exists, not valid JSON.
        parse_path = Path(tmp) / "malformed.json"
        parse_path.write_text("{ this is not valid json", encoding="utf-8")
        fallback_cases.append(("parse_failed", str(parse_path),
                               "JSON parse failed"))
        # State 3: wrong_schema → file is valid JSON but schema is wrong.
        wrong_path = Path(tmp) / "wrong-schema.json"
        wrong_path.write_text(
            json.dumps({"schema": "not-sbo3l-operator-evidence-v1"}),
            encoding="utf-8",
        )
        fallback_cases.append(("wrong_schema", str(wrong_path),
                               "schema is not"))
        for state, ev_arg, expected_phrase in fallback_cases:
            out = Path(tmp) / f"index-{state}.html"
            proc = subprocess.run(
                [sys.executable, str(BUILD),
                 "--input", str(FIXTURE),
                 "--evidence", ev_arg,
                 "--output", str(out)],
                capture_output=True, text=True,
            )
            if proc.returncode != 0:
                _fail(f"fallback {state}: build.py rc={proc.returncode}",
                      proc.stderr.strip())
                failures += 1
                continue
            html_state = out.read_text(encoding="utf-8")
            # Each of the five panels must carry the reason text — counts
            # ≥5 to ensure all five panels ran the new propagation, not
            # just one.
            occurrences = html_state.count(expected_phrase)
            if occurrences >= 5:
                _ok(f"fallback {state}: phrase {expected_phrase!r} present",
                    f"{occurrences} occurrences (≥5 panels)")
            else:
                _fail(f"fallback {state}: phrase {expected_phrase!r} not propagated",
                      f"only {occurrences} occurrences (need ≥5)")
                failures += 1
            # Conversely, the misdiagnosis "evidence file missing" must
            # NOT appear for non-missing states — that was the bug.
            if state != "missing" and "evidence file missing" in html_state:
                _fail(f"fallback {state}: misdiagnosed as 'evidence file missing'",
                      "renderer fell back to the wrong reason text")
                failures += 1
            else:
                _ok(f"fallback {state}: not misdiagnosed as 'evidence file missing'")

    # 7. Passport capsule fallback-state propagation. Each non-ok state for
    #    the allow tile (or the deny tile) must surface the SPECIFIC
    #    "capsule evidence not gathered" placeholder — never a fake-OK
    #    summary. Mirrors the evidence-load propagation above.
    print("\n== passport capsule fallback-state propagation ==")
    capsule_fallback_cases: list[tuple[str, str]] = []
    with tempfile.TemporaryDirectory() as tmp:
        missing_capsule = Path(tmp) / "no-such-capsule.json"
        capsule_fallback_cases.append(("missing", str(missing_capsule)))
        parse_capsule = Path(tmp) / "malformed-capsule.json"
        parse_capsule.write_text("{ this is not valid json", encoding="utf-8")
        capsule_fallback_cases.append(("parse_failed", str(parse_capsule)))
        wrong_capsule = Path(tmp) / "wrong-schema-capsule.json"
        wrong_capsule.write_text(
            json.dumps({"schema": "sbo3l.audit_bundle.v1"}),
            encoding="utf-8",
        )
        capsule_fallback_cases.append(("wrong_schema", str(wrong_capsule)))
        capsule_fallback_cases.append(("tampered", str(CAPSULE_TAMPERED_FIXTURE)))
        for state, capsule_arg in capsule_fallback_cases:
            out = Path(tmp) / f"index-capsule-{state}.html"
            proc = subprocess.run(
                [sys.executable, str(BUILD),
                 "--input", str(FIXTURE),
                 "--evidence", str(EVIDENCE_FIXTURE),
                 "--capsule-allow", capsule_arg,
                 "--capsule-deny", str(missing_capsule),
                 "--output", str(out)],
                capture_output=True, text=True,
            )
            if proc.returncode != 0:
                _fail(f"capsule {state}: build.py rc={proc.returncode}",
                      proc.stderr.strip())
                failures += 1
                continue
            html_state = out.read_text(encoding="utf-8")
            if "capsule evidence not gathered" in html_state:
                _ok(f"capsule {state}: placeholder rendered")
            else:
                _fail(f"capsule {state}: placeholder missing",
                      "expected 'capsule evidence not gathered' in HTML")
                failures += 1
            if f"reason=<code>{state}</code>" in html_state:
                _ok(f"capsule {state}: reason text propagated")
            else:
                _fail(f"capsule {state}: reason text not propagated",
                      f"expected 'reason=<code>{state}</code>'")
                failures += 1

    # required (v1) + evidence_required (v2) + 5×2 negative + capsule_required
    # + forbidden (script/fetch) + url-safety + parse + 3 evidence fallback
    # states × 2 + 4 capsule fallback states × 2.
    total = (
        len(required) + len(evidence_required) + (5 * 2)
        + len(capsule_required)
        + len(forbidden) + 1   # url-safety
        + 1                    # html.parser
        + (len(fallback_cases) * 2)
        + (len(capsule_fallback_cases) * 2)
    )
    print()
    if failures == 0:
        print(f"PASS: {total} checks ok")
        return 0
    print(f"FAIL: {failures} of {total} checks failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
