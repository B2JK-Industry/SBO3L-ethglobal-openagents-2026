"""Pydantic type-model tests."""

from __future__ import annotations

import pytest
from pydantic import ValidationError

from sbo3l_sdk import (
    PassportCapsuleV1,
    PassportCapsuleV2,
    PaymentRequest,
    PolicyReceipt,
)
from tests.fixtures import GOLDEN_APRP, GOLDEN_CAPSULE_V1, build_capsule_v2, clone


class TestPaymentRequest:
    def test_golden_round_trip(self) -> None:
        m = PaymentRequest.model_validate(GOLDEN_APRP)
        # by_alias=True is irrelevant here (no aliases on APRP), but the
        # resulting JSON must round-trip back to validate cleanly.
        again = PaymentRequest.model_validate(m.model_dump(mode="json"))
        assert again == m

    def test_rejects_unknown_field(self) -> None:
        bad = clone(GOLDEN_APRP)
        bad["wat"] = "no"
        with pytest.raises(ValidationError):
            PaymentRequest.model_validate(bad)

    def test_rejects_bad_agent_id(self) -> None:
        bad = clone(GOLDEN_APRP)
        bad["agent_id"] = "Not-a-valid-id"  # uppercase, fails pattern
        with pytest.raises(ValidationError):
            PaymentRequest.model_validate(bad)

    def test_rejects_bad_money_value(self) -> None:
        bad = clone(GOLDEN_APRP)
        bad["amount"]["value"] = "0.05.05"
        with pytest.raises(ValidationError):
            PaymentRequest.model_validate(bad)

    def test_destination_discriminator(self) -> None:
        # Switch to EOA destination — must be accepted.
        good = clone(GOLDEN_APRP)
        good["destination"] = {
            "type": "eoa",
            "address": "0x" + "a" * 40,
        }
        good["payment_protocol"] = "erc20_transfer"
        m = PaymentRequest.model_validate(good)
        assert m.destination.type == "eoa"  # type: ignore[union-attr]


class TestPolicyReceipt:
    def test_extracted_from_golden(self) -> None:
        receipt = PolicyReceipt.model_validate(GOLDEN_CAPSULE_V1["decision"]["receipt"])
        assert receipt.decision == "allow"
        assert receipt.signature.algorithm == "ed25519"

    def test_rejects_uppercase_hash(self) -> None:
        bad = clone(GOLDEN_CAPSULE_V1["decision"]["receipt"])
        bad["request_hash"] = "AB" * 32
        with pytest.raises(ValidationError):
            PolicyReceipt.model_validate(bad)


class TestPassportCapsule:
    def test_v1_validates_golden(self) -> None:
        m = PassportCapsuleV1.model_validate(GOLDEN_CAPSULE_V1)
        # The schema field is aliased to `schema` on the wire.
        dumped = m.model_dump(mode="json", by_alias=True)
        assert dumped["schema"] == "sbo3l.passport_capsule.v1"

    def test_v2_validates_extension(self) -> None:
        v2 = build_capsule_v2()
        m = PassportCapsuleV2.model_validate(v2)
        assert m.policy.policy_snapshot is not None
        assert m.audit.audit_segment == {"events": []}

    def test_v1_rejects_v2_schema_id(self) -> None:
        bad = clone(GOLDEN_CAPSULE_V1)
        bad["schema"] = "sbo3l.passport_capsule.v2"
        with pytest.raises(ValidationError):
            PassportCapsuleV1.model_validate(bad)

    def test_v2_rejects_v1_schema_id(self) -> None:
        bad = build_capsule_v2()
        bad["schema"] = "sbo3l.passport_capsule.v1"
        with pytest.raises(ValidationError):
            PassportCapsuleV2.model_validate(bad)

    def test_models_are_frozen(self) -> None:
        m = PassportCapsuleV1.model_validate(GOLDEN_CAPSULE_V1)
        with pytest.raises(ValidationError):
            m.generated_at = "2099-01-01T00:00:00Z"  # type: ignore[misc]
