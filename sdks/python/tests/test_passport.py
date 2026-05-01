"""Structural verifier tests."""

from __future__ import annotations

import pytest

from sbo3l_sdk import PassportVerificationError, verify, verify_or_raise
from tests.fixtures import GOLDEN_CAPSULE_V1, build_capsule_v2, clone


class TestGoldenV1:
    def test_accepts_golden_v1(self) -> None:
        r = verify(GOLDEN_CAPSULE_V1)
        assert r.ok is True
        assert r.failures == ()
        assert r.schema_version == 1

    def test_verify_or_raise_returns_dict(self) -> None:
        out = verify_or_raise(GOLDEN_CAPSULE_V1)
        assert out is GOLDEN_CAPSULE_V1


class TestGoldenV2:
    def test_accepts_v2(self) -> None:
        r = verify(build_capsule_v2())
        assert r.ok
        assert r.schema_version == 2

    def test_includes_v2_only_checks(self) -> None:
        r = verify(build_capsule_v2())
        codes = {c.code for c in r.checks}
        assert "capsule.policy_snapshot_shape" in codes
        assert "capsule.audit_segment_shape" in codes

    def test_rejects_non_object_policy_snapshot(self) -> None:
        bad = build_capsule_v2()
        bad["policy"]["policy_snapshot"] = "not-an-object"
        r = verify(bad)
        assert not r.ok
        assert "capsule.policy_snapshot_shape" in r.codes()


class TestSchemaDiscrimination:
    def test_unknown_schema_id(self) -> None:
        bad = clone(GOLDEN_CAPSULE_V1)
        bad["schema"] = "sbo3l.passport_capsule.v9"
        r = verify(bad)
        assert not r.ok
        assert r.schema_version is None
        assert r.failures[0].code == "capsule.schema_unknown"

    def test_rejects_none(self) -> None:
        r = verify(None)
        assert not r.ok
        assert r.schema_version is None

    def test_rejects_string(self) -> None:
        r = verify("not-a-capsule")
        assert not r.ok


class TestCrossField:
    def test_request_hash_mismatch(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["decision"]["receipt"]["request_hash"] = "deadbeef" * 8
        r = verify(c)
        assert not r.ok
        assert "capsule.request_hash_match_receipt" in r.codes()

    def test_policy_hash_mismatch(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["decision"]["receipt"]["policy_hash"] = "ab" * 32
        r = verify(c)
        assert not r.ok
        assert "capsule.policy_hash_match_receipt" in r.codes()

    def test_decision_mismatch(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["decision"]["result"] = "deny"
        r = verify(c)
        assert not r.ok
        assert "capsule.decision_match_receipt" in r.codes()

    def test_audit_event_id_mismatch(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["audit"]["audit_event_id"] = "evt-01ABCDEFGHJKMNPQRSTVWXYZ12"
        r = verify(c)
        assert not r.ok
        assert "capsule.audit_event_id_match_receipt" in r.codes()


class TestHexShapes:
    def test_malformed_request_hash(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["request"]["request_hash"] = "NOTHEX"
        r = verify(c)
        assert not r.ok
        assert "capsule.request_hash_shape" in r.codes()

    def test_short_signature(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["decision"]["receipt_signature"] = "11"
        r = verify(c)
        assert not r.ok
        assert "capsule.receipt_signature_shape" in r.codes()

    def test_uppercase_policy_hash(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["policy"]["policy_hash"] = "AB" * 32
        c["decision"]["receipt"]["policy_hash"] = "AB" * 32
        r = verify(c)
        assert not r.ok
        assert "capsule.policy_hash_shape" in r.codes()


class TestExecutionInvariants:
    def test_deny_must_be_not_called(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["decision"]["result"] = "deny"
        c["decision"]["receipt"]["decision"] = "deny"
        c["decision"]["deny_code"] = "policy.budget_exceeded"
        c["decision"]["receipt"]["deny_code"] = "policy.budget_exceeded"
        # status stays 'submitted' — should fail truthfulness rule
        r = verify(c)
        assert not r.ok
        assert "capsule.deny_status_must_be_not_called" in r.codes()

    def test_live_mode_requires_evidence(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["execution"]["mode"] = "live"
        c["execution"]["live_evidence"] = None
        r = verify(c)
        assert not r.ok
        assert "capsule.live_mode_requires_evidence" in r.codes()

    def test_live_mode_with_evidence_passes(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["execution"]["mode"] = "live"
        c["execution"]["live_evidence"] = {"transport": "https"}
        r = verify(c)
        assert "capsule.live_mode_requires_evidence" not in r.codes()

    def test_mock_mode_rejects_live_evidence(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["execution"]["live_evidence"] = {"transport": "https"}
        r = verify(c)
        assert not r.ok
        assert "capsule.mock_mode_rejects_live_evidence" in r.codes()


class TestCheckpoint:
    def test_rejects_mock_anchor_false(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["audit"]["checkpoint"]["mock_anchor"] = False
        r = verify(c)
        assert not r.ok
        assert "capsule.checkpoint_mock_anchor_required" in r.codes()

    def test_rejects_bad_anchor_ref(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["audit"]["checkpoint"]["mock_anchor_ref"] = "wrong-prefix-9202d6bc7b751225"
        r = verify(c)
        assert not r.ok
        assert "capsule.checkpoint_mock_anchor_ref_shape" in r.codes()


class TestVerifyOrRaise:
    def test_raises_on_bad_capsule(self) -> None:
        with pytest.raises(PassportVerificationError):
            verify_or_raise({"schema": "wrong"})

    def test_raises_carries_codes(self) -> None:
        c = clone(GOLDEN_CAPSULE_V1)
        c["request"]["request_hash"] = "NOTHEX"
        with pytest.raises(PassportVerificationError) as exc_info:
            verify_or_raise(c)
        assert "capsule.request_hash_shape" in exc_info.value.codes
