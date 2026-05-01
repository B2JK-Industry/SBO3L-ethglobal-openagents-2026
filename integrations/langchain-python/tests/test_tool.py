"""Tests for sbo3l_langchain.sbo3l_tool."""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock

import pytest

from sbo3l_langchain import SBO3LSubmitResult, sbo3l_tool

APRP = json.dumps(
    {
        "agent_id": "research-agent-01",
        "task_id": "demo-1",
        "intent": "purchase_api_call",
    }
)

ALLOW_RESULT: SBO3LSubmitResult = {
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-low-risk-x402",
    "request_hash": "c0bd2fab" * 8,
    "policy_hash": "e044f13c" * 8,
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    "receipt": {"execution_ref": "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"},
}

DENY_RESULT: SBO3LSubmitResult = {
    "decision": "deny",
    "deny_code": "policy.budget_exceeded",
    "matched_rule_id": "daily-budget",
    "request_hash": "c0bd2fab" * 8,
    "policy_hash": "e044f13c" * 8,
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGT",
    "receipt": {"execution_ref": None},
}


def fake_client(result: SBO3LSubmitResult) -> Any:
    c = MagicMock()
    c.submit = MagicMock(return_value=result)
    return c


class TestDescriptorShape:
    def test_returns_descriptor(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        assert t.name == "sbo3l_payment_request"
        assert len(t.description) > 50
        assert callable(t.func)

    def test_custom_name_description(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT), name="my_tool", description="custom")
        assert t.name == "my_tool"
        assert t.description == "custom"


class TestHappyPath:
    def test_allow_envelope(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func(APRP))
        assert out["decision"] == "allow"
        assert out["execution_ref"] == "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"
        assert out["matched_rule_id"] == "allow-low-risk-x402"

    def test_deny_envelope(self) -> None:
        t = sbo3l_tool(client=fake_client(DENY_RESULT))
        out = json.loads(t.func(APRP))
        assert out["decision"] == "deny"
        assert out["deny_code"] == "policy.budget_exceeded"
        assert out["execution_ref"] is None

    def test_forwards_aprp(self) -> None:
        client = fake_client(ALLOW_RESULT)
        t = sbo3l_tool(client=client)
        t.func(APRP)
        client.submit.assert_called_once()
        body = client.submit.call_args.args[0]
        assert body["agent_id"] == "research-agent-01"


class TestInputValidation:
    def test_rejects_non_json(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func("not-json"))
        assert out["error"] == "input is not valid JSON"

    def test_rejects_array(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func("[1,2,3]"))
        assert out["error"] == "input must be a JSON object (APRP)"
        assert out["input_received_type"] == "array"

    def test_rejects_null(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func("null"))
        assert out["error"] == "input must be a JSON object (APRP)"
        assert out["input_received_type"] == "null"


class TestErrorHandling:
    def test_surfaces_sbo3l_error_code(self) -> None:
        err = type(
            "FakeSBO3LError",
            (Exception,),
            {"code": "auth.required", "status": 401},
        )("auth required")
        client = MagicMock()
        client.submit = MagicMock(side_effect=err)
        t = sbo3l_tool(client=client)
        out = json.loads(t.func(APRP))
        assert out["error"] == "auth.required"
        assert out["status"] == 401

    def test_falls_back_to_transport_failed(self) -> None:
        client = MagicMock()
        client.submit = MagicMock(side_effect=ConnectionError("ECONNREFUSED"))
        t = sbo3l_tool(client=client)
        out = json.loads(t.func(APRP))
        assert out["error"] == "transport.failed"
        assert "ECONNREFUSED" in out["detail"]


class _FakeAwaitable:
    """Bare-minimum awaitable for the async-guard test — no coroutine leak."""

    def __await__(self) -> Any:  # pragma: no cover (never awaited in this test)
        yield  # pragma: no cover

    def close(self) -> None:
        pass


class TestAsyncClientGuard:
    def test_async_client_in_sync_tool_warns(self) -> None:
        client = MagicMock()
        client.submit = MagicMock(return_value=_FakeAwaitable())
        t = sbo3l_tool(client=client)
        out = json.loads(t.func(APRP))
        assert out["error"] == "transport.async_client_in_sync_tool"


class TestIdempotency:
    def test_callback_invoked(self) -> None:
        client = fake_client(ALLOW_RESULT)
        t = sbo3l_tool(
            client=client,
            idempotency_key=lambda body: f"{body['task_id']}-key",
        )
        t.func(APRP)
        kwargs = client.submit.call_args.kwargs
        assert kwargs["idempotency_key"] == "demo-1-key"

    def test_omitted_when_not_set(self) -> None:
        client = fake_client(ALLOW_RESULT)
        t = sbo3l_tool(client=client)
        t.func(APRP)
        kwargs = client.submit.call_args.kwargs
        assert "idempotency_key" not in kwargs


def test_module_version_exported() -> None:
    import sbo3l_langchain

    assert isinstance(sbo3l_langchain.__version__, str)
    # Loose semver-shape check
    parts = sbo3l_langchain.__version__.split(".")
    assert len(parts) == 3
    pytest.importorskip("sbo3l_langchain")  # smoke import works
