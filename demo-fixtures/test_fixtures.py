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

HERE = Path(__file__).resolve().parent

# Hostnames that are explicitly safe to reference in fixtures:
#   - example.* (RFC 2606 §3) reserved for documentation
#   - test/example/invalid/localhost TLDs (RFC 6761 / 2606)
#   - schemas.mandate.dev / 127.0.0.1 — already in pre-existing fixtures
SAFE_HOST_PATTERNS = [
    re.compile(r"^https?://(?:[a-z0-9-]+\.)*example\.(?:com|net|org|test)\b", re.IGNORECASE),
    re.compile(r"^https?://(?:[a-z0-9-]+\.)*invalid\b", re.IGNORECASE),
    re.compile(r"^https?://(?:[a-z0-9-]+\.)*localhost\b", re.IGNORECASE),
    re.compile(r"^https?://127\.0\.0\.1\b"),
    re.compile(r"^https?://schemas\.mandate\.dev/", re.IGNORECASE),
]

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
    return any(p.match(url) for p in SAFE_HOST_PATTERNS)


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


def main() -> int:
    fixtures = sorted(HERE.glob("mock-*.json"))
    if not fixtures:
        _fail("no mock-*.json fixtures found", str(HERE))
        return 1

    print(f"== validating {len(fixtures)} mock fixture(s) ==")
    total_failures = 0
    for f in fixtures:
        print(f"\n-- {f.name} --")
        total_failures += validate_one(f)

    print()
    if total_failures == 0:
        print(f"PASS: all {len(fixtures)} mock fixture(s) clean")
        return 0
    print(f"FAIL: {total_failures} failure(s) across {len(fixtures)} fixture(s)", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
