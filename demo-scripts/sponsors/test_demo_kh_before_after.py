#!/usr/bin/env python3
# A3 — regression test for `keeperhub-before-after.sh`.
#
# Mirrors the stdlib-only test pattern used by `demo-fixtures/test_fixtures.py`
# (no third-party deps; `python3 demo-scripts/sponsors/test_demo_kh_before_after.py`
# from the repo root). Runs the demo script end-to-end and asserts the
# transcript carries the documented BEFORE / AFTER / response shape with
# every `sbo3l_*` envelope key present, plus determinism (running the
# script twice produces byte-identical transcripts — the demo video
# can't drift between takes).
#
# Why a Python test instead of a Rust integration test: the existing
# convention for sponsor-demo-shaped artefacts is stdlib Python
# (`demo-fixtures/test_fixtures.py`, `trust-badge/test_build.py`,
# `operator-console/test_build.py`). The Rust example
# `before_after_envelope.rs` already has its own unit-test coverage in
# the adapter crate (`tests::keeperhub_live_constructs_envelope_via_from_receipt`
# pins the envelope shape upstream); this test pins the *demo wrapper's*
# observable behaviour at the artefact boundary.
#
# Run from repo root.

from __future__ import annotations

import filecmp
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DEMO_SCRIPT = REPO_ROOT / "demo-scripts" / "sponsors" / "keeperhub-before-after.sh"
TRANSCRIPT = REPO_ROOT / "demo-scripts" / "artifacts" / "keeperhub-before-after.txt"

# The four IP-1 envelope fields that MUST appear in the AFTER block.
# `sbo3l_passport_capsule_hash` is target-only and intentionally omitted
# (Sbo3lEnvelope::from_receipt sets it to None; serde skip_serializing_if
# omits it from the wire form).
REQUIRED_AFTER_KEYS = (
    "sbo3l_request_hash",
    "sbo3l_policy_hash",
    "sbo3l_receipt_signature",
    "sbo3l_audit_event_id",
)

# The deterministic placeholder id printed in the AFTER response block.
# If the example ever returns a different id, this test catches it.
EXPECTED_PLACEHOLDER_ID = "kh-demo-placeholder-fixed-for-determinism"


def fail(msg: str) -> None:
    print(f"FAIL: {msg}", file=sys.stderr)
    sys.exit(1)


def run_demo() -> str:
    """Execute the demo script from the repo root and return its
    stdout. Surfaces any non-zero exit + the captured stderr."""
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
            f"--- stdout ---\n{proc.stdout}\n"
            f"--- stderr ---\n{proc.stderr}"
        )
    return proc.stdout


def assert_block_headers_present(out: str) -> None:
    for header in (
        "== BEFORE SBO3L — raw KeeperHub workflow-webhook submission ==",
        "== AFTER SBO3L — same workflow with IP-1 envelope attached ==",
        "== AFTER SBO3L — KeeperHub response shape (placeholder id) ==",
        "== Why this matters ==",
    ):
        if header not in out:
            fail(f"demo output missing required header: {header!r}")
    print("  ok   four required block headers present")


def assert_envelope_keys_in_after(out: str) -> None:
    # Slice the AFTER body region. The AFTER body block lives between
    # its header and the response-shape header.
    after_start = out.index("== AFTER SBO3L — same workflow with IP-1 envelope attached ==")
    after_end = out.index("== AFTER SBO3L — KeeperHub response shape")
    after_block = out[after_start:after_end]
    for key in REQUIRED_AFTER_KEYS:
        if f'"{key}"' not in after_block:
            fail(f"AFTER block missing IP-1 envelope key: {key}")
    print(f"  ok   AFTER block carries all 4 IP-1 envelope keys: {', '.join(REQUIRED_AFTER_KEYS)}")


def assert_executionId_placeholder_present(out: str) -> None:
    if EXPECTED_PLACEHOLDER_ID not in out:
        fail(
            f"AFTER response block missing the deterministic placeholder id; "
            f"expected {EXPECTED_PLACEHOLDER_ID!r} for demo-video stability"
        )
    print(f"  ok   response block carries deterministic executionId placeholder")


def assert_transcript_written(out: str) -> None:
    if not TRANSCRIPT.exists():
        fail(f"transcript file not written: {TRANSCRIPT}")
    if TRANSCRIPT.stat().st_size == 0:
        fail(f"transcript file is empty: {TRANSCRIPT}")
    print(f"  ok   transcript written: {TRANSCRIPT.relative_to(REPO_ROOT)}")


def assert_no_secret_leak(out: str) -> None:
    # Hard guarantee: the demo MUST NOT print any real wfb_ / kh_ token,
    # even if the operator happens to have those env vars set in the
    # shell that runs the test. The demo script never reads them, but
    # this is defence-in-depth so a future "convenience" change can't
    # accidentally leak.
    forbidden_substrings = ("wfb_", "kh_", "Bearer ")
    leaks = [s for s in forbidden_substrings if s in out]
    if leaks:
        fail(
            f"demo output contains forbidden secret-shaped substrings: {leaks}\n"
            "the demo must never echo a token prefix or Authorization header"
        )
    print("  ok   no secret-shaped substrings (wfb_/kh_/Bearer) in demo output")


def assert_deterministic_across_runs() -> None:
    # Run the script a second time into a temp file and byte-compare
    # against the canonical transcript. Determinism is what makes this
    # demo viable for a recorded video (no take-to-take drift).
    with tempfile.TemporaryDirectory() as td:
        snapshot = Path(td) / "second-run.txt"
        shutil.copy(TRANSCRIPT, snapshot)
        run_demo()  # rewrites TRANSCRIPT
        if not filecmp.cmp(snapshot, TRANSCRIPT, shallow=False):
            fail("demo script is non-deterministic — two runs produced different transcripts")
    print("  ok   demo is byte-deterministic across two runs")


def main() -> int:
    if not DEMO_SCRIPT.exists():
        fail(f"demo script not found: {DEMO_SCRIPT}")
    print(f"== A3 demo regression test ({DEMO_SCRIPT.relative_to(REPO_ROOT)}) ==\n")
    out = run_demo()
    assert_block_headers_present(out)
    assert_envelope_keys_in_after(out)
    assert_executionId_placeholder_present(out)
    assert_transcript_written(out)
    assert_no_secret_leak(out)
    assert_deterministic_across_runs()
    print("\nPASS: 6 checks ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
