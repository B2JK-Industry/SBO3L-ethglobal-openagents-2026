"""SDK conformance — Python runner.

Walks `test-corpus/sdk-conformance/manifest.json` and asserts the
Python SDK's structural verifier produces the same outcome as the
manifest's `verify_ok` field. The Rust runner
(`crates/sbo3l-core/tests/sdk_conformance.rs`) and TS runner
(`sdks/typescript/test/sdk_conformance.test.ts`) walk the same
manifest — drift between SDKs is the regression mode this catches.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from sbo3l_sdk import verify

MANIFEST_SCHEMA = "sbo3l.sdk_conformance_manifest.v1"


def corpus_root() -> Path:
    """Return the absolute path to test-corpus/, regardless of CWD."""
    # tests/test_sdk_conformance.py → tests/ → sdks/python/ → sdks/ → repo root
    return Path(__file__).resolve().parents[3] / "test-corpus"


def load_manifest() -> dict:
    raw = (corpus_root() / "sdk-conformance" / "manifest.json").read_text()
    m = json.loads(raw)
    assert m["schema"] == MANIFEST_SCHEMA, (
        f"manifest schema id drift: {m['schema']} != {MANIFEST_SCHEMA}"
    )
    return m


def load_capsule(rel: str) -> dict:
    return json.loads((corpus_root() / rel).read_text())


@pytest.fixture(scope="module")
def manifest() -> dict:
    return load_manifest()


def test_manifest_has_at_least_one_vector(manifest: dict) -> None:
    assert len(manifest["vectors"]) > 0


def test_manifest_vector_count_pinned(manifest: dict) -> None:
    """Pin the corpus size — adding/removing a fixture must update
    both the manifest and this assertion together. Catches an
    accidental delete during refactors."""
    assert len(manifest["vectors"]) == 19, (
        "manifest vector count drifted; update both the manifest and this constant"
    )


SDK_NAME = "python"


def test_python_sdk_matches_every_manifest_vector(manifest: dict) -> None:
    """The conformance contract: every fixture's structural verify
    outcome agrees with the manifest. Drift between Python and Rust
    (or TS) shows up as a failure here, with a per-vector diff.

    Vectors listed under `known_drift: [..., "python", ...]` are
    SKIPPED — those are documented gaps where the Python SDK
    currently disagrees with the Rust reference. The skip is loud
    (counted in `pending`) so a regression that flips a non-drift
    vector still trips the test, but a known-drift vector doesn't
    keep CI red while the gap is being worked.
    """
    failures: list[str] = []
    pending: list[str] = []
    for vector in manifest["vectors"]:
        if SDK_NAME in vector.get("known_drift", []):
            pending.append(vector["name"])
            continue
        capsule = load_capsule(vector["fixture"])
        result = verify(capsule)
        actual_ok = result.ok
        expected_ok = vector["verify_ok"]
        if actual_ok != expected_ok:
            failure_codes = (
                ",".join(c.code for c in result.failures) if not actual_ok else ""
            )
            failures.append(
                f"[{vector['name']}] expected verify_ok={expected_ok}, got {actual_ok} "
                f"(failures: {failure_codes})"
            )
    if pending:
        # Surface the skipped count so an operator scanning CI
        # logs sees the conformance gap without having to grep.
        print(
            f"\nSDK conformance: {len(pending)} vector(s) pending Python SDK fix "
            f"({', '.join(pending)})"
        )
    assert not failures, (
        f"SDK conformance manifest mismatch ({len(failures)} failures):\n"
        + "\n".join(failures)
    )


def test_python_sdk_schema_version_matches_manifest(manifest: dict) -> None:
    """The Python SDK reports `schema_version` on its verify result;
    every golden vector must report the version the manifest claims.
    Tampered fixtures may carry a deliberately-bad schema, so we
    only assert on golden ones."""
    for vector in manifest["vectors"]:
        if "golden" not in vector["name"]:
            continue
        capsule = load_capsule(vector["fixture"])
        result = verify(capsule)
        assert result.schema_version == vector["schema_version"], (
            f"[{vector['name']}] expected schema_version={vector['schema_version']}, "
            f"got {result.schema_version}"
        )
