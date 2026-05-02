"""Tests for sbo3l-pydantic-ai.

MagicMock client (langchain-py pattern) — keeps tests independent of
SDK Pydantic strict-mode wire validation.
"""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock

import pytest
from pydantic import ValidationError

from sbo3l_pydantic_ai import (
    AprpInput,
    sbo3l_payment_request_func,
)
from sbo3l_pydantic_ai.tool import _coerce_to_dict

VALID_APRP_DICT: dict[str, Any] = {
    "agent_id": "research-agent-01",
    "task_id": "demo-pydantic-1",
    "intent": "purchase_api_call",
    "amount": {"value": "0.05", "currency": "USD"},
    "token": "USDC",
    "destination": {
        "type": "x402_endpoint",
        "url": "https://api.example.com/v1/inference",
        "method": "POST",
        "expected_recipient": "0x1111111111111111111111111111111111111111",
    },
    "payment_protocol": "x402",
    "chain": "base",
    "provider_url": "https://api.example.com",
    "expiry": "2099-01-01T00:00:00Z",
    "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
    "risk_class": "low",
}

VALID_APRP_JSON = json.dumps(VALID_APRP_DICT)

ALLOW_RESULT: dict[str, Any] = {
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-x402",
    "request_hash": "00" * 32,
    "policy_hash": "00" * 32,
    "audit_event_id": "evt-allow-1",
    "receipt": {"execution_ref": "kh-1"},
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


# ---------------------------------------------------------------------------
# AprpInput model
# ---------------------------------------------------------------------------


def test_aprp_input_accepts_canonical_fixture() -> None:
    AprpInput.model_validate(VALID_APRP_DICT)


def test_aprp_input_rejects_unknown_intent() -> None:
    bad = {**VALID_APRP_DICT, "intent": "buy_compute_job"}
    with pytest.raises(ValidationError):
        AprpInput.model_validate(bad)


def test_aprp_input_rejects_amount_currency_eur() -> None:
    bad = {**VALID_APRP_DICT, "amount": {"value": "0.05", "currency": "EUR"}}
    with pytest.raises(ValidationError):
        AprpInput.model_validate(bad)


def test_aprp_input_rejects_uppercase_agent_id() -> None:
    bad = {**VALID_APRP_DICT, "agent_id": "Has-Capital-Letters"}
    with pytest.raises(ValidationError):
        AprpInput.model_validate(bad)


def test_aprp_input_rejects_non_decimal_amount() -> None:
    bad = {**VALID_APRP_DICT, "amount": {"value": "not-a-number", "currency": "USD"}}
    with pytest.raises(ValidationError):
        AprpInput.model_validate(bad)


# ---------------------------------------------------------------------------
# Descriptor + happy path
# ---------------------------------------------------------------------------


def test_descriptor_default_name() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    assert desc.name == "sbo3l_payment_request"
    assert callable(desc.func)
    assert "Pydantic-validated" in desc.description


def test_descriptor_name_override() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT), name="pay")
    assert desc.name == "pay"


def test_allow_returns_envelope_with_execution_ref() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    out = json.loads(desc.func(VALID_APRP_JSON))
    assert out["decision"] == "allow"
    assert out["audit_event_id"] == "evt-allow-1"
    assert out["execution_ref"] == "kh-1"


def test_deny_does_not_throw_returns_structured_envelope() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(DENY_RESULT))
    out = json.loads(desc.func(VALID_APRP_JSON))
    assert out["error"] == "policy.deny"
    assert out["decision"] == "deny"
    assert out["deny_code"] == "policy.budget_exceeded"


def test_requires_human_uses_distinct_error() -> None:
    rh = {
        **DENY_RESULT,
        "decision": "requires_human",
        "deny_code": "policy.requires_human_review",
    }
    desc = sbo3l_payment_request_func(client=fake_client(rh))
    out = json.loads(desc.func(VALID_APRP_JSON))
    assert out["error"] == "policy.requires_human"


# ---------------------------------------------------------------------------
# Local validation — the headline win for Pydantic AI
# ---------------------------------------------------------------------------


def test_local_pydantic_validation_runs_BEFORE_network() -> None:
    """Bad input should NEVER reach client.submit."""
    client = MagicMock()
    client.submit = MagicMock(return_value=ALLOW_RESULT)
    desc = sbo3l_payment_request_func(client=client)
    bad_json = json.dumps({**VALID_APRP_DICT, "intent": "wat"})
    out = json.loads(desc.func(bad_json))
    assert out["error"] == "input.bad_arguments"
    client.submit.assert_not_called()  # critical — no network hit


def test_bad_json_input_returns_input_bad_arguments() -> None:
    desc = sbo3l_payment_request_func(client=fake_client(ALLOW_RESULT))
    out = json.loads(desc.func("{not-json"))
    assert out["error"] == "input.bad_arguments"
    assert isinstance(out["detail"], str)


# ---------------------------------------------------------------------------
# Idempotency + transport
# ---------------------------------------------------------------------------


def test_idempotency_key_callback_is_invoked() -> None:
    seen: list[str] = []

    def derive(body: dict[str, Any]) -> str:
        key = f"key-{body['task_id']}"
        seen.append(key)
        return key

    client = fake_client(ALLOW_RESULT)
    desc = sbo3l_payment_request_func(client=client, idempotency_key=derive)
    desc.func(VALID_APRP_JSON)
    assert seen == ["key-demo-pydantic-1"]
    assert client.submit.call_args.kwargs.get("idempotency_key") == "key-demo-pydantic-1"


def test_transport_failure_surfaces_as_envelope() -> None:
    client = MagicMock()
    client.submit = MagicMock(side_effect=RuntimeError("network down"))
    desc = sbo3l_payment_request_func(client=client)
    out = json.loads(desc.func(VALID_APRP_JSON))
    assert out["error"] == "transport.failed"
    assert "network down" in out["detail"]


def test_transport_failure_with_domain_code_preserves_it() -> None:
    err = RuntimeError("auth fail")
    err.code = "auth.bad_token"  # type: ignore[attr-defined]
    client = MagicMock()
    client.submit = MagicMock(side_effect=err)
    desc = sbo3l_payment_request_func(client=client)
    out = json.loads(desc.func(VALID_APRP_JSON))
    assert out["error"] == "auth.bad_token"


# ---------------------------------------------------------------------------
# _coerce_to_dict edges
# ---------------------------------------------------------------------------


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
