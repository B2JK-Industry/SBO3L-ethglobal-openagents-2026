#!/usr/bin/env python3
# Validation for the production-shaped mock fixtures under demo-fixtures/.
#
# Mirrors the stdlib-only test pattern used by trust-badge/test_build.py
# and operator-console/test_build.py. Asserts that every `mock-*.json`
# in this directory:
#
#   - parses as JSON
#   - declares an envelope: schema, mock=true, explanation, live_replacement
#   - has a non-empty schema id following `mandate-mock-*-v1`
#   - contains no http/https URL outside the safe set (RFC 2606 reserved
#     hostnames + the existing `schemas.mandate.dev` $id pattern)
#   - contains no obvious secret-looking strings (PEM blocks, "private_key",
#     "signing_key", `kh_*` / `wfb_*` workflow tokens)
#   - if it claims `no_private_material`, has zero hex strings >= 64 chars
#     in fields whose names hint at private material
#
# Run from repo root: `python3 demo-fixtures/test_fixtures.py`.

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from urllib.parse import urlparse

HERE = Path(__file__).resolve().parent

# Hostnames that are explicitly safe to reference in fixtures.
#
# Codex P1 review on PR #30 caught that the previous regex-based
# allowlist anchored only at the start of the URL and used `\b` (word
# boundary) at the end of the host, which lets attacker-controlled
# infixes slip through:
#   "https://schemas.mandate.dev.attacker.io/x"
# matches `^https?://schemas\.mandate\.dev/` only if anchored, but a
# regex like `^https?://(?:[a-z0-9-]+\.)*example\.(?:com|net|org|test)\b`
# does match `https://example.com.evil.org/x` because `\b` sits between
# `m` and `.` (word↔non-word). Switching to `urllib.parse.urlparse` +
# exact-host or safe-suffix membership makes the bypass structurally
# impossible.
#
#   - exact hosts: 127.0.0.1, localhost, schemas.mandate.dev
#   - safe suffixes (RFC 2606 / 6761 reserved):
#       .invalid, .example, .test, .localhost
#     The leading dot is required so "evilexample" does NOT end with
#     ".example".
SAFE_HOSTS_EXACT = frozenset({
    "127.0.0.1",
    "localhost",
    "schemas.mandate.dev",
})
SAFE_HOST_SUFFIXES = (
    ".invalid",
    ".example",
    ".test",
    ".localhost",
)

URL_PATTERN = re.compile(r"https?://[^\s\"'<>]+", re.IGNORECASE)
SECRET_PATTERNS = [
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"\b(private_key|signing_key|seed_hex|seed_bytes)\s*[:=]\s*\"[0-9a-fA-F]{32,}", re.IGNORECASE),
    re.compile(r"\bkh_[A-Za-z0-9]{8,}"),
    re.compile(r"\bwfb_[A-Za-z0-9]{8,}"),
]


def _ok(label: str, hint: str = "") -> None:
    suffix = f": {hint}" if hint else ""
    print(f"  ok   {label}{suffix}")


def _fail(label: str, hint: str = "") -> None:
    suffix = f": {hint}" if hint else ""
    print(f"  FAIL {label}{suffix}", file=sys.stderr)


def url_is_safe(url: str) -> bool:
    """True iff `url` is http(s) and its hostname is in the allowlist.

    The hostname comes from `urllib.parse.urlparse(...).hostname`, which
    extracts only the `host` component (port stripped, lowercased) — so
    `https://schemas.mandate.dev.attacker.io/x` resolves to host
    `schemas.mandate.dev.attacker.io`, which is neither in the exact set
    nor ends with a safe suffix → reject.
    """
    try:
        parsed = urlparse(url)
    except ValueError:
        return False
    if parsed.scheme not in ("http", "https"):
        return False
    host = parsed.hostname
    if not host:
        return False
    host = host.lower()
    if host in SAFE_HOSTS_EXACT:
        return True
    return any(host.endswith(suffix) for suffix in SAFE_HOST_SUFFIXES)


def find_urls(raw: str) -> list[str]:
    return [m.group(0) for m in URL_PATTERN.finditer(raw)]


def find_secrets(raw: str) -> list[str]:
    hits: list[str] = []
    for pat in SECRET_PATTERNS:
        m = pat.search(raw)
        if m:
            hits.append(m.group(0))
    return hits


def validate_one(path: Path) -> int:
    """Returns the number of failures found in `path`."""
    failures = 0
    label = path.name
    raw = path.read_text(encoding="utf-8")

    # 1. Parses.
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as e:
        _fail(f"{label}: JSON parse", str(e))
        return 1

    # 2. Envelope: schema, mock=true, explanation, live_replacement.
    schema = doc.get("schema")
    if not isinstance(schema, str) or not re.match(r"^mandate-mock-[a-z0-9-]+-v\d+$", schema):
        _fail(f"{label}: schema", f"missing or malformed schema id (got {schema!r})")
        failures += 1
    else:
        _ok(f"{label}: schema id", schema)

    if doc.get("mock") is not True:
        _fail(f"{label}: mock=true", f"expected mock: true, got {doc.get('mock')!r}")
        failures += 1
    else:
        _ok(f"{label}: mock=true")

    expl = doc.get("explanation")
    if not isinstance(expl, str) or len(expl.strip()) < 40:
        _fail(f"{label}: explanation", "must be a non-empty string of at least 40 chars")
        failures += 1
    else:
        _ok(f"{label}: explanation present", f"{len(expl)} chars")

    live = doc.get("live_replacement")
    if not isinstance(live, str) or len(live.strip()) < 40:
        _fail(f"{label}: live_replacement", "must be a non-empty string of at least 40 chars")
        failures += 1
    else:
        _ok(f"{label}: live_replacement present", f"{len(live)} chars")

    # 3. URL safety.
    bad_urls = [u for u in find_urls(raw) if not url_is_safe(u)]
    if bad_urls:
        _fail(f"{label}: external URL", f"unsafe URL(s) in fixture: {bad_urls!r}")
        failures += 1
    else:
        _ok(f"{label}: no unsafe external URLs")

    # 4. Secret patterns.
    secrets = find_secrets(raw)
    if secrets:
        _fail(f"{label}: secret pattern", f"secret-looking string(s) in fixture: {secrets!r}")
        failures += 1
    else:
        _ok(f"{label}: no secret-looking strings")

    # 5. no_private_material guard (only when the fixture claims it).
    if doc.get("no_private_material") is True:
        suspicious = re.findall(
            r'"(?:signing_key_hex|private_key_hex|seed_hex|seed_bytes_hex)"\s*:\s*"[0-9a-fA-F]{32,}',
            raw,
        )
        if suspicious:
            _fail(f"{label}: no_private_material guard", f"found {len(suspicious)} suspicious field(s)")
            failures += 1
        else:
            _ok(f"{label}: no_private_material guard")

    return failures


def _self_test_url_safety() -> int:
    """Pin the URL allowlist against past-bypass shapes.

    Every case is a regression for the Codex P1 finding on PR #30: the
    old regex anchored only at the start of the URL, which let
    attacker-infixed hostnames pass. After switching to
    `urlparse().hostname` + exact/safe-suffix membership, these all
    resolve correctly.
    """
    cases: list[tuple[str, bool, str]] = [
        # (url, expected_safe, why)
        # --- bypass attempts MUST be rejected ---
        (
            "https://example.com.evil.org/x",
            False,
            "infix bypass: 'example.com' inside an attacker-controlled host",
        ),
        (
            "https://schemas.mandate.dev.attacker.io/x",
            False,
            "infix bypass: 'schemas.mandate.dev' as a left-prefix of attacker host",
        ),
        (
            "https://evilexample/x",
            False,
            "no-leading-dot bypass: 'evilexample' does not end with '.example'",
        ),
        (
            "ftp://schemas.mandate.dev/x",
            False,
            "non-http(s) scheme: only http/https are allowed",
        ),
        # --- legitimate references MUST be accepted ---
        (
            "https://schemas.mandate.dev/x",
            True,
            "exact-host allowlist member",
        ),
        (
            "https://research-agent.team.eth.invalid/x",
            True,
            "RFC 2606 reserved suffix '.invalid'",
        ),
        (
            "http://127.0.0.1:8730/v1",
            True,
            "exact-host loopback IP with port",
        ),
        (
            "http://localhost/x",
            True,
            "exact-host 'localhost'",
        ),
    ]

    print("-- url_is_safe self-test --")
    failures = 0
    for url, expected, why in cases:
        actual = url_is_safe(url)
        verdict = "ok" if actual == expected else "FAIL"
        marker = "accept" if expected else "reject"
        if actual == expected:
            _ok(f"url_is_safe({marker:>6}) {url}", why)
        else:
            failures += 1
            _fail(
                f"url_is_safe({marker:>6}) {url}",
                f"expected {expected}, got {actual} — {why}",
            )
        # Surface the verdict variable so static-analysis tooling that
        # walks the loop sees both branches; no behavioural change.
        del verdict
    return failures


def main() -> int:
    print("== url_is_safe self-test ==")
    self_test_failures = _self_test_url_safety()
    if self_test_failures:
        print(
            f"FAIL: url_is_safe self-test had {self_test_failures} failure(s)",
            file=sys.stderr,
        )
        # Don't short-circuit — still validate fixtures so the operator
        # sees the full picture, but the exit code reflects the failure.

    fixtures = sorted(HERE.glob("mock-*.json"))
    if not fixtures:
        _fail("no mock-*.json fixtures found", str(HERE))
        return 1

    print(f"\n== validating {len(fixtures)} mock fixture(s) ==")
    total_failures = self_test_failures
    for f in fixtures:
        print(f"\n-- {f.name} --")
        total_failures += validate_one(f)

    print()
    if total_failures == 0:
        print(f"PASS: all {len(fixtures)} mock fixture(s) clean + url self-test")
        return 0
    print(f"FAIL: {total_failures} failure(s) across {len(fixtures)} fixture(s)", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
