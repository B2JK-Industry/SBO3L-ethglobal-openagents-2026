#!/usr/bin/env python3
"""Validate Mandate JSON Schemas and the test-corpus against them.

The APRP schema has an external ``$ref`` to the x402 schema by URL, so we wire
a :class:`referencing.Registry` that resolves every Mandate schema id against
the local file in ``schemas/``. This avoids any network call.
"""
from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parent.parent
SCHEMAS = {
    "aprp": REPO_ROOT / "schemas" / "aprp_v1.json",
    "policy": REPO_ROOT / "schemas" / "policy_v1.json",
    "x402": REPO_ROOT / "schemas" / "x402_v1.json",
    "policy-receipt": REPO_ROOT / "schemas" / "policy_receipt_v1.json",
    "decision-token": REPO_ROOT / "schemas" / "decision_token_v1.json",
    "audit-event": REPO_ROOT / "schemas" / "audit_event_v1.json",
    "passport-capsule": REPO_ROOT / "schemas" / "mandate.passport_capsule.v1.json",
}


@dataclass(frozen=True)
class CorpusCase:
    schema: str
    fixture: Path
    expect_valid: bool


CORPUS: list[CorpusCase] = [
    CorpusCase("aprp", REPO_ROOT / "test-corpus/aprp/golden_001_minimal.json", True),
    CorpusCase("aprp", REPO_ROOT / "test-corpus/aprp/deny_prompt_injection_request.json", True),
    CorpusCase("aprp", REPO_ROOT / "test-corpus/aprp/adversarial_unknown_field.json", False),
    CorpusCase("policy", REPO_ROOT / "test-corpus/policy/reference_low_risk.json", True),
    # Passport capsule (P1.1). Schema-only validation here; the cross-field
    # truthfulness invariants (deny→no execution, live→evidence, hash internal-
    # consistency) are tested by `cargo test -p mandate-core passport` and
    # `cargo test -p mandate-cli --test passport_cli`. The fixtures below are
    # *only* labelled `expect_valid` by their schema-shape outcome:
    #   - golden_001: schema-valid.
    #   - tampered_001 (deny+execution_ref): schema-valid; rejected at the
    #     verifier layer, not at the schema layer.
    #   - tampered_002 (mock_anchor=false): schema-INVALID via const true.
    #   - tampered_003 (live+no_evidence): schema-valid; verifier-only.
    #   - tampered_004/005 (hash mismatch): schema-valid; verifier-only.
    #   - tampered_006 (bad mock_anchor_ref): schema-INVALID via pattern.
    #   - tampered_007 (unknown root field): schema-INVALID via
    #     additionalProperties=false.
    #   - tampered_008 (live+empty evidence): schema-INVALID via
    #     live_evidence.minProperties=1.
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/golden_001_allow_keeperhub_mock.json",
        True,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_001_deny_with_execution_ref.json",
        True,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_002_mock_anchor_marked_live.json",
        False,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_003_live_mode_without_evidence.json",
        True,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_004_request_hash_mismatch.json",
        True,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_005_policy_hash_mismatch.json",
        True,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_006_malformed_checkpoint.json",
        False,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_007_unknown_field.json",
        False,
    ),
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_008_live_mode_empty_evidence.json",
        False,
    ),
]


def _load(path: Path) -> dict:
    with path.open() as f:
        return json.load(f)


def _build_registry():
    from referencing import Registry, Resource

    resources: list[tuple[str, Resource]] = []
    for path in SCHEMAS.values():
        doc = _load(path)
        schema_id = doc.get("$id")
        if not schema_id:
            continue
        resources.append((schema_id, Resource.from_contents(doc)))
    return Registry().with_resources(resources)


def main() -> int:
    try:
        import jsonschema
    except ImportError as exc:  # pragma: no cover - user-facing
        print(
            "error: missing dependency jsonschema. "
            "Install with: pip install 'jsonschema[format]>=4.21'",
            file=sys.stderr,
        )
        print(f"  detail: {exc}", file=sys.stderr)
        return 2

    registry = _build_registry()
    ok = True

    print("== schema metaschemas ==")
    for name, path in SCHEMAS.items():
        try:
            jsonschema.Draft202012Validator.check_schema(_load(path))
            print(f"  ok   {name}: {path.relative_to(REPO_ROOT)}")
        except Exception as exc:
            ok = False
            print(f"  FAIL {name}: {path.relative_to(REPO_ROOT)} -> {exc}")

    print("\n== test corpus ==")
    for case in CORPUS:
        schema = _load(SCHEMAS[case.schema])
        validator = jsonschema.Draft202012Validator(schema, registry=registry)
        try:
            validator.validate(_load(case.fixture))
            actual = True
        except jsonschema.ValidationError:
            actual = False

        status = "ok  " if actual == case.expect_valid else "FAIL"
        rel = case.fixture.relative_to(REPO_ROOT)
        print(
            f"  {status} {rel} (schema={case.schema}, "
            f"expect_valid={case.expect_valid}, actual={actual})"
        )
        if actual != case.expect_valid:
            ok = False

    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
