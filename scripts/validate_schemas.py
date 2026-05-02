#!/usr/bin/env python3
"""Validate SBO3L JSON Schemas and the test-corpus against them.

The APRP schema has an external ``$ref`` to the x402 schema by URL, so we wire
a :class:`referencing.Registry` that resolves every SBO3L schema id against
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
    "passport-capsule": REPO_ROOT / "schemas" / "sbo3l.passport_capsule.v1.json",
    "passport-capsule-v2": REPO_ROOT / "schemas" / "sbo3l.passport_capsule.v2.json",
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
    # consistency) are tested by `cargo test -p sbo3l-core passport` and
    # `cargo test -p sbo3l-cli --test passport_cli`. The fixtures below are
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
    #   - tampered_009 (executor_evidence={}): schema-INVALID via the
    #     P6.1-bumped `executor_evidence.minProperties=1` (the new
    #     mode-agnostic sponsor-evidence slot — distinct from
    #     `live_evidence`, which is strictly transport-level / live-only).
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
    CorpusCase(
        "passport-capsule",
        REPO_ROOT / "test-corpus/passport/tampered_009_executor_evidence_empty_object.json",
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

    # Runtime artifacts emitted by `bash demo-scripts/run-production-shaped-mock.sh`
    # step 10b (Passport P2.1). These files are NOT in test-corpus
    # (test-corpus is for static fixtures); they are produced by the
    # actual `sbo3l passport run` CLI on every full runner invocation.
    # Validating them here closes the loop: every capsule the runner
    # writes must pass schema validation before any downstream surface
    # (P2.2 trust-badge / operator-console capsule panels) tries to
    # render it.
    print("\n== runtime artifacts (passport capsules) ==")
    # Runtime artifacts are versioned via the `schema` field. Pick the right
    # schema per artifact (v1 vs v2 — daemon currently emits v2 after the
    # P6.1 executor_evidence schema bump).
    runtime_artifacts = [
        REPO_ROOT / "demo-scripts/artifacts/passport-allow.json",
        REPO_ROOT / "demo-scripts/artifacts/passport-deny.json",
    ]
    for fixture in runtime_artifacts:
        if not fixture.is_file():
            print(
                f"  skip {fixture.relative_to(REPO_ROOT)} "
                f"(not yet emitted; run `bash demo-scripts/run-production-shaped-mock.sh` "
                f"to produce it)"
            )
            continue
        artifact = _load(fixture)
        capsule_schema_id = artifact.get("schema", "")
        if capsule_schema_id == "sbo3l.passport_capsule.v2":
            schema_key = "passport-capsule-v2"
        elif capsule_schema_id == "sbo3l.passport_capsule.v1":
            schema_key = "passport-capsule"
        else:
            ok = False
            print(
                f"  FAIL {fixture.relative_to(REPO_ROOT)} "
                f"-> unrecognised schema id: {capsule_schema_id!r}"
            )
            continue
        schema = _load(SCHEMAS[schema_key])
        validator = jsonschema.Draft202012Validator(schema, registry=registry)
        try:
            validator.validate(artifact)
            print(
                f"  ok   {fixture.relative_to(REPO_ROOT)} "
                f"(schema={schema_key}, runtime artifact)"
            )
        except jsonschema.ValidationError as exc:
            ok = False
            print(
                f"  FAIL {fixture.relative_to(REPO_ROOT)} "
                f"(schema={schema_key}) -> {exc.message}"
            )

    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
