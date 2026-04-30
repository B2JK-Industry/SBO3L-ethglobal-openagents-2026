#!/usr/bin/env python3
# A4 — regression test for the Uniswap offline-verify segment of
# `uniswap-guarded-swap.sh`.
#
# Mirrors `test_demo_kh_before_after.py` (A3) and the broader stdlib
# convention used by `demo-fixtures/test_fixtures.py`. Runs the demo
# script end-to-end (no `--live` — CI-deterministic mock path only),
# then asserts the artefact-side and stdout-side properties that make
# A4 the differentiator vs other Uniswap-track entries:
#
#   - the script exits 0 (capsule emit + passport verify both succeed)
#   - the executor_evidence block has all 10 UniswapQuoteEvidence keys
#   - `passport verify` reports `structural verify: ok` (offline,
#     no RPC / subgraph / agent backend / KeeperHub call)
#   - the differentiator framing block is in the demo output
#   - the transcript artefact is written + non-empty + carries the
#     same evidence + verify content
#   - no secret-shaped substrings leak into the demo output
#
# Run from repo root: `python3 demo-scripts/sponsors/test_demo_uniswap_evidence.py`.

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DEMO_SCRIPT = REPO_ROOT / "demo-scripts" / "sponsors" / "uniswap-guarded-swap.sh"
TRANSCRIPT = REPO_ROOT / "demo-scripts" / "artifacts" / "uniswap-evidence-offline-verify.txt"

# The 10 UniswapQuoteEvidence fields (P6.1). Pinned here so the test
# catches a future refactor that drops or renames any of them.
REQUIRED_EVIDENCE_KEYS = (
    "quote_id",
    "quote_source",
    "input_token",
    "output_token",
    "route_tokens",
    "notional_in",
    "slippage_cap_bps",
    "quote_timestamp_unix",
    "quote_freshness_seconds",
    "recipient_address",
)

# Substrings that MUST appear in the demo output to prove A4 framed
# the offline-verify path correctly.
REQUIRED_FRAMING = (
    "executor_evidence — what an offline auditor reads",
    "passport verify — schema + 8 cross-field invariants, NO network",
    "Why this is the differentiator",
    "Offline. Post-hoc. Single file.",
)


def fail(msg: str) -> None:
    print(f"FAIL: {msg}", file=sys.stderr)
    sys.exit(1)


def run_demo() -> str:
    """Execute the demo script (mock-only path) from the repo root and
    return its stdout. Surfaces non-zero exit + captured stderr."""
    proc = subprocess.run(
        ["bash", str(DEMO_SCRIPT)],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        fail(
            f"demo script exited {proc.returncode}\n"
            f"--- stdout ---\n{proc.stdout[-2000:]}\n"
            f"--- stderr ---\n{proc.stderr[-2000:]}"
        )
    return proc.stdout


def assert_executor_evidence_keys_present(out: str) -> None:
    # The evidence block is JSON pretty-printed by `jq`; assert each
    # key shows up as a quoted JSON property name. Substring match is
    # enough — we don't need a full JSON parse here, the underlying
    # serialisation is pinned by the executor's Rust unit tests.
    missing = [k for k in REQUIRED_EVIDENCE_KEYS if f'"{k}"' not in out]
    if missing:
        fail(
            f"executor_evidence missing UniswapQuoteEvidence fields: {missing}\n"
            "(demo output prefix: " + out[:400] + " …)"
        )
    print(
        f"  ok   executor_evidence block carries all {len(REQUIRED_EVIDENCE_KEYS)} "
        f"UniswapQuoteEvidence fields"
    )


def assert_passport_verify_ok(out: str) -> None:
    if "structural verify: ok" not in out:
        fail("passport verify did not report 'structural verify: ok'")
    if "decision:      allow" not in out:
        fail("passport verify did not report decision: allow on the Uniswap path")
    if "executor:      uniswap" not in out:
        fail("passport verify did not report executor: uniswap")
    print("  ok   passport verify offline: schema + invariants + decision allow")


def assert_framing_present(out: str) -> None:
    missing = [s for s in REQUIRED_FRAMING if s not in out]
    if missing:
        fail(f"demo output missing differentiator framing: {missing}")
    print("  ok   differentiator framing block present (offline / post-hoc / single file)")


def assert_transcript_written() -> None:
    if not TRANSCRIPT.exists():
        fail(f"transcript file not written: {TRANSCRIPT}")
    if TRANSCRIPT.stat().st_size == 0:
        fail(f"transcript file is empty: {TRANSCRIPT}")
    body = TRANSCRIPT.read_text()
    if "executor_evidence" not in body or "structural verify: ok" not in body:
        fail(
            f"transcript at {TRANSCRIPT.relative_to(REPO_ROOT)} doesn't carry the "
            "evidence + verify content the demo video needs"
        )
    print(f"  ok   transcript written: {TRANSCRIPT.relative_to(REPO_ROOT)}")


def assert_no_secret_leak(out: str) -> None:
    # Defence-in-depth: the Uniswap demo never reads any sponsor token,
    # but assert the demo output never echoes one.
    forbidden = ("wfb_", "kh_", "Bearer ", "0x" + "f" * 64)
    leaks = [s for s in forbidden if s in out]
    if leaks:
        fail(
            f"demo output contains forbidden secret-shaped substrings: {leaks}\n"
            "the demo must never echo a token prefix or signed-key material"
        )
    print("  ok   no secret-shaped substrings in demo output")


def assert_quote_id_shape(out: str) -> None:
    # quote_id is non-deterministic (fresh ULID per run) but its SHAPE
    # is stable: prefix `mock-uniswap-quote-` + 26 Crockford base32
    # chars. Pinning the shape catches a refactor that drops the
    # mock prefix (which would break the truthfulness invariant
    # "demo output discloses mock state explicitly").
    pattern = re.compile(r'"quote_id":\s*"mock-uniswap-quote-[0-9A-HJKMNP-TV-Z]{26}"')
    if not pattern.search(out):
        fail("quote_id does not match `mock-uniswap-quote-<ULID>` shape")
    print("  ok   quote_id carries the `mock-` prefix (truthfulness invariant)")


def main() -> int:
    if not DEMO_SCRIPT.exists():
        fail(f"demo script not found: {DEMO_SCRIPT}")
    print(f"== A4 demo regression test ({DEMO_SCRIPT.relative_to(REPO_ROOT)}) ==\n")
    out = run_demo()
    assert_executor_evidence_keys_present(out)
    assert_passport_verify_ok(out)
    assert_framing_present(out)
    assert_transcript_written()
    assert_no_secret_leak(out)
    assert_quote_id_shape(out)
    print("\nPASS: 6 checks ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
