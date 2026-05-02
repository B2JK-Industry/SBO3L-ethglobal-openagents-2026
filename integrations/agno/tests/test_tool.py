"""Tests for the Agno SBO3L tool builder.

Mocks the SBO3L client via MagicMock — same pattern as
``integrations/langchain-python/tests/test_tool.py``.  This keeps the
tests independent of the SDK's wire validation (Pydantic strict-mode
PolicyReceipt has many required fields the tool itself never inspects).
"""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock

import pytest

from sbo3l_agno import sbo3l_payment_request_func
from sbo3l_agno.tool import _coerce_to_dict

APRP = json.dumps(
    {
        "agent_id": "research-agent-01",
        "task_id": "demo-agno-1",
        "intent": "purchase_api_call",
    }
)

ALLOW_RESULT: dict[str, Any] = {
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-small-x402-api-call",
    "request_hash": "00" * 32,
    "policy_hash": "00" * 32,
    "audit_event_id": "evt-allow-1",
    "receipt": {"execution_ref": "kh-allow-1"},
}

DENY_RESULT: dict[str, Any] = {
    "decision": "deny",
    "deny_code": "policy.budget_exceeded",
    "matched_rule_id": "cap-per-tx",
    "request_hash": "00" * 32,
    "policy_hash": "00" * 32,
    "audit_event_id": "evt-deny-1",
    "receipt": {"execution_ref": None},
}


def fake_client(result: dict[str, Any]) -> Any:
    c = MagicMock()
    c.submit = MagicMock(return_value=result)
    return c


def test_descriptor_default_name() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    assert desc.name == "sbo3l_payment_request"
    assert callable(desc.func)
    assert "APRP" in desc.description


def test_descriptor_name_override() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT), name="pay")
    assert desc.name == "pay"


def test_descriptor_description_override() -> None:
    desc = sbo3l_payment_request_func(
        client=fake_client(ALLOW_RESULT), description="custom"
    )
    assert desc.description == "custom"


def test_allow_returns_envelope_with_execution_ref() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    out = json.loads(desc.func(APRP))
    assert out["decision"] == "allow"
    assert out["audit_event_id"] == "evt-allow-1"
    assert out["execution_ref"] == "kh-allow-1"
    assert out["request_hash"] == "00" * 32


def test_deny_does_not_throw_returns_structured_envelope() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(DENY_RESULT))
    out = json.loads(desc.func(APRP))
    assert out["error"] == "policy.deny"
    assert out["decision"] == "deny"
    assert out["deny_code"] == "policy.budget_exceeded"
    assert out["audit_event_id"] == "evt-deny-1"


def test_requires_human_uses_distinct_error_code() -> None:
    requires_human_result = {
        **DENY_RESULT,
        "decision": "requires_human",
        "deny_code": "policy.requires_human_review",
    }
    desc = sbo3l_payment_request_func(client=fake_client(requires_human_result))
    out = json.loads(desc.func(APRP))
    assert out["error"] == "policy.requires_human"
    assert out["decision"] == "requires_human"


def test_bad_json_input_returns_input_bad_arguments() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    out = json.loads(desc.func("{not-json"))
    assert out["error"] == "input.bad_arguments"
    assert isinstance(out["detail"], str)


def test_non_object_input_returns_input_bad_arguments() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    out = json.loads(desc.func(json.dumps([1, 2, 3])))
    assert out["error"] == "input.bad_arguments"
    assert "array" in out["detail"]


def test_idempotency_key_callback_is_invoked() -> None:
    seen: list[str] = []

    def derive(body: dict[str, Any]) -> str:
        key = f"key-{body['task_id']}"
        seen.append(key)
        return key

    client = fake_client(ALLOW_RESULT)
    desc = sbo3l_payment_request_func(client=client, idempotency_key=derive)
    desc.func(APRP)
    assert seen == ["key-demo-agno-1"]
    # The idempotency_key kwarg made it into client.submit
    assert client.submit.call_args.kwargs.get("idempotency_key") == "key-demo-agno-1"


def test_transport_failure_surfaces_as_envelope() -> None:
    client = MagicMock()
    client.submit = MagicMock(side_effect=RuntimeError("network down"))
    desc = sbo3l_payment_request_func(client=client)
    out = json.loads(desc.func(APRP))
    assert out["error"] == "transport.failed"
    assert "network down" in out["detail"]


def test_transport_failure_with_domain_code_preserves_it() -> None:
    err = RuntimeError("auth fail")
    err.code = "auth.bad_token"  # type: ignore[attr-defined]
    client = MagicMock()
    client.submit = MagicMock(side_effect=err)
    desc = sbo3l_payment_request_func(client=client)
    out = json.loads(desc.func(APRP))
    assert out["error"] == "auth.bad_token"


def test_coerce_to_dict_passes_dict_through() -> None:
    src = {"x": 1}
    assert _coerce_to_dict(src) is src


def test_coerce_to_dict_calls_model_dump() -> None:
    class Fake:
        def model_dump(self) -> dict[str, Any]:
            return {"y": 2}

    assert _coerce_to_dict(Fake()) == {"y": 2}


def test_coerce_to_dict_rejects_non_dict_non_pydantic() -> None:
    with pytest.raises(TypeError):
        _coerce_to_dict(42)
