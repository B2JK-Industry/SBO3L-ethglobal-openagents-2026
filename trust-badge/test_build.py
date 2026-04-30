#!/usr/bin/env python3
# Render-regression coverage for trust-badge/build.py.
#
# Drives build.py against trust-badge/fixtures/demo-summary.json, captures the
# generated HTML, and asserts:
#   - every required proof field from the fixture is present in the HTML
#     (agent id, full commit SHA, both request hashes, policy hash, both
#     audit events, both receipt signatures, no-key proof labels + counts,
#     denied_action_executed, keeperhub_refused, mock disclosure, tagline)
#   - the surface never invites JS or network: no <script> element,
#     no fetch( call, no http(s):// URL anywhere
#   - the rendered HTML parses without error
#
# Stdlib only (json / re / subprocess / sys / tempfile / pathlib / html.parser).
# Run from repo root: `python3 trust-badge/test_build.py`.

from __future__ import annotations

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
FIXTURE = HERE / "fixtures" / "demo-summary.json"
# Passport P2.2 (post-P2.1 rebase): prefer the runtime capsule emitted by the
# production-shaped runner's step 10b (P2.1 #44). Fall back to the on-main
# golden fixture when the runtime artifact is absent — for example in CI,
# which does not run the production-shaped runner before this test. The
# assertion logic uses values read FROM the loaded capsule, so either source
# produces a consistent, truthful test pass.
RUNTIME_CAPSULE = HERE.parent / "demo-scripts" / "artifacts" / "passport-allow.json"
GOLDEN_CAPSULE = HERE.parent / "test-corpus" / "passport" / "golden_001_allow_keeperhub_mock.json"
TAMPERED_FIXTURE = HERE.parent / "test-corpus" / "passport" / "tampered_002_mock_anchor_marked_live.json"

if RUNTIME_CAPSULE.is_file():
    CAPSULE_FIXTURE = RUNTIME_CAPSULE
    CAPSULE_SOURCE = "runtime artifact (demo-scripts/artifacts/passport-allow.json)"
else:
    CAPSULE_FIXTURE = GOLDEN_CAPSULE
    CAPSULE_SOURCE = "on-main golden fixture (test-corpus/passport/golden_001_*.json)"

# Same safe-host allowlist as `demo-fixtures/test_fixtures.py` so the
# "no external URLs" check rejects unsafe URLs only — `schemas.sbo3l.dev`
# (the canonical $id host for SBO3L's own JSON-Schema files) and the
# RFC 2606/6761 reserved suffixes remain allowed.
SAFE_HOSTS_EXACT = frozenset({
    "127.0.0.1",
    "localhost",
    "schemas.sbo3l.dev",
    # Canonical public GitHub Pages host for SBO3L's static proof
    # site. ENS fixtures publish `sbo3l:proof_uri` here; if the
    # trust-badge renderer ever surfaces that URL into the rendered
    # HTML, the URL-scan stays clean. Kept consistent with the same
    # constant in `demo-fixtures/test_fixtures.py` +
    # `operator-console/test_build.py`.
    "b2jk-industry.github.io",
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

    if not CAPSULE_FIXTURE.is_file():
        _fail("capsule fixture not found", str(CAPSULE_FIXTURE))
        return 1
    with CAPSULE_FIXTURE.open(encoding="utf-8") as fh:
        capsule = json.load(fh)
    print(f"  note: capsule source = {CAPSULE_SOURCE}")

    # 1. Drive build.py against the fixture into a temp file. We do not write
    #    the rendered HTML into trust-badge/index.html — generated artefacts
    #    must never appear in the working tree from a test run.
    #    The Passport-capsule tile is exercised explicitly with the on-main
    #    golden fixture (STEP 1 / DRAFT). After P2.1 lands and emits runtime
    #    artifacts, the same flag will point at `passport-allow.json`.
    with tempfile.TemporaryDirectory() as tmp:
        out_path = Path(tmp) / "index.html"
        proc = subprocess.run(
            [sys.executable, str(BUILD),
             "--input", str(FIXTURE),
             "--capsule", str(CAPSULE_FIXTURE),
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

    # 2. Required-content checks. Each entry asserts that a specific string
    #    from the fixture (or a stable rendered marker) appears in the HTML.
    legit = summary["scenarios"]["legit_x402"]
    pi = summary["scenarios"]["prompt_injection"]
    required = [
        ("agent_id",                         summary["agent_id"]),
        ("demo_commit (full 40-char SHA)",   summary["demo_commit"]),
        ("tagline",                          "Don't give your agent a wallet"),
        ("schema",                           summary["schema"]),
        # allow scenario
        ("legit decision pill",              ">Allow<"),
        ("legit matched_rule",               legit["matched_rule"]),
        ("legit request_hash",               legit["request_hash"]),
        ("legit policy_hash",                legit["policy_hash"]),
        ("legit audit_event",                legit["audit_event"]),
        ("legit receipt_signature",          legit["receipt_signature"]),
        ("legit keeperhub_execution_ref",    legit["keeperhub_execution_ref"]),
        # deny scenario
        ("deny decision pill",               ">Deny<"),
        ("deny deny_code",                   pi["deny_code"]),
        ("deny matched_rule",                pi["matched_rule"]),
        ("deny request_hash",                pi["request_hash"]),
        ("deny policy_hash",                 pi["policy_hash"]),
        ("deny audit_event",                 pi["audit_event"]),
        ("deny receipt_signature",           pi["receipt_signature"]),
        # boolean labels (rendered with name + green pill when desired)
        ("denied_action_executed label",     "denied_action_executed"),
        ("keeperhub_refused label",          "keeperhub_refused"),
        # no-key proof
        ("no_key_proof status PASS",         ">PASS<"),
        ("no_key_proof source label",        "agent_source_signer_references"),
        ("no_key_proof cargo label",         "agent_cargo_signer_deps"),
        ("no_key_proof fixtures label",      "agent_key_material_files"),
        # audit chain
        ("audit structural_verify label",    "structural_verify_accepts_tampered_actor"),
        ("audit strict_hash_verify label",   "strict_hash_verify_rejects_tampered"),
        # mock disclosure
        ("mock disclosure pill",             ">mock<"),
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

    # 2b. Passport-capsule tile content. The capsule schema/verifier
    #     landed in PR #42 (Passport P1.1); the trust-badge renders ONE
    #     summary tile from a `sbo3l.passport_capsule.v1` artifact.
    print("\n== passport capsule tile ==")
    capsule_required = [
        ("capsule tile header",              "Passport capsule"),
        ("capsule agent ens_name",           capsule["agent"]["ens_name"]),
        ("capsule resolver source",          capsule["agent"]["resolver"]),
        ("capsule allow decision pill",      ">Allow<"),
        ("capsule matched_rule",             capsule["decision"]["matched_rule"]),
        ("capsule executor",                 capsule["execution"]["executor"]),
        ("capsule execution_ref",            capsule["execution"]["execution_ref"]),
        ("capsule mock_anchor_ref",          capsule["audit"]["checkpoint"]["mock_anchor_ref"]),
        ("capsule mock-anchor pill",         "mock anchoring, NOT onchain"),
        ("capsule offline_verifiable yes",   ">yes<"),
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

    # 3. Negative checks. Case-insensitive to catch <Script>, FETCH(, etc.
    print("\n== forbidden surface ==")
    forbidden = [
        ("no <script> element",             re.compile(r"<\s*script", re.IGNORECASE)),
        ("no fetch( call",                  re.compile(r"\bfetch\s*\(", re.IGNORECASE)),
    ]
    for label, regex in forbidden:
        m = regex.search(html_text)
        if m:
            _fail(label, f"matched {m.group(0)!r} at offset {m.start()}")
            failures += 1
        else:
            _ok(label)

    # 3b. URL-safety check. The Passport capsule tile renders agent
    #     records like `sbo3l:mcp_endpoint` whose value is a URL; we
    #     allow URLs only when the host is in the SBO3L safe-host
    #     allowlist (RFC 2606/6761 reserved + 127.0.0.1 +
    #     schemas.sbo3l.dev). Anything else is a network-leak risk.
    unsafe_urls = [u for u in URL_PATTERN.findall(html_text) if not _url_is_safe(u)]
    if unsafe_urls:
        _fail("no unsafe http(s):// URL", f"found unsafe URLs: {unsafe_urls[:3]}")
        failures += 1
    else:
        _ok("no unsafe http(s):// URL (only RFC 2606 + schemas.sbo3l.dev)")

    # 4. Well-formedness — html.parser is lenient but catches obvious damage.
    print("\n== html parse ==")
    try:
        HTMLParser().feed(html_text)
        _ok("html.parser consumed without error")
    except Exception as e:  # pragma: no cover
        _fail("html.parser", str(e))
        failures += 1

    # 5. Capsule failure-state propagation. The trust-badge MUST render
    #    the "capsule evidence not gathered" placeholder when the capsule
    #    is missing / parse-failed / wrong-schema / structurally tampered
    #    — never a fake-OK summary tile.
    print("\n== capsule failure-state propagation ==")
    fallback_cases: list[tuple[str, str]] = []
    with tempfile.TemporaryDirectory() as tmp:
        # missing — no such file.
        missing_path = Path(tmp) / "no-such-capsule.json"
        fallback_cases.append(("missing", str(missing_path)))
        # parse_failed — file exists but is not valid JSON.
        parse_path = Path(tmp) / "malformed.json"
        parse_path.write_text("{ this is not valid json", encoding="utf-8")
        fallback_cases.append(("parse_failed", str(parse_path)))
        # wrong_schema — valid JSON but schema id is not the expected one.
        wrong_path = Path(tmp) / "wrong-schema.json"
        wrong_path.write_text(
            json.dumps({"schema": "sbo3l.audit_bundle.v1"}),
            encoding="utf-8",
        )
        fallback_cases.append(("wrong_schema", str(wrong_path)))
        # tampered — schema matches but a structural invariant fails.
        fallback_cases.append(("tampered", str(TAMPERED_FIXTURE)))
        for state, capsule_arg in fallback_cases:
            out = Path(tmp) / f"index-{state}.html"
            proc = subprocess.run(
                [sys.executable, str(BUILD),
                 "--input", str(FIXTURE),
                 "--capsule", capsule_arg,
                 "--output", str(out)],
                capture_output=True, text=True,
            )
            if proc.returncode != 0:
                _fail(f"capsule {state}: build.py rc={proc.returncode}",
                      proc.stderr.strip())
                failures += 1
                continue
            html_state = out.read_text(encoding="utf-8")
            # Honest placeholder must be present.
            if "capsule evidence not gathered" in html_state:
                _ok(f"capsule {state}: placeholder rendered")
            else:
                _fail(f"capsule {state}: placeholder missing",
                      "expected 'capsule evidence not gathered' in HTML")
                failures += 1
            # And the explicit reason must surface so operators are sent
            # down the right remediation path — never misdiagnosed.
            if f"reason=<code>{state}</code>" in html_state:
                _ok(f"capsule {state}: reason text propagated")
            else:
                _fail(f"capsule {state}: reason text not propagated",
                      f"expected 'reason=<code>{state}</code>'")
                failures += 1

    # required (demo summary) + capsule_required + forbidden + url-safety
    # + html.parser + 4 fallback cases × 2 assertions.
    total = (
        len(required) + len(capsule_required) + len(forbidden)
        + 1   # url-safety check
        + 1   # html.parser
        + (len(fallback_cases) * 2)
    )
    print()
    if failures == 0:
        print(f"PASS: {total} checks ok")
        return 0
    print(f"FAIL: {failures} of {total} checks failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
