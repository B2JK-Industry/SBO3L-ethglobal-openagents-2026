"""Tests for sbo3l_llamaindex.sbo3l_tool."""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock

from sbo3l_llamaindex import SBO3LSubmitResult, sbo3l_tool

APRP = json.dumps(
    {"agent_id": "research-agent-01", "task_id": "demo-1", "intent": "purchase_api_call"}
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


class TestDescriptor:
    def test_default_name(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        assert t.name == "sbo3l_payment_request"

    def test_custom_name(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT), name="my_tool")
        assert t.name == "my_tool"


class TestRouting:
    def test_allow_envelope(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func(APRP))
        assert out["decision"] == "allow"
        assert out["execution_ref"] == "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"

    def test_deny_envelope(self) -> None:
        t = sbo3l_tool(client=fake_client(DENY_RESULT))
        out = json.loads(t.func(APRP))
        assert out["decision"] == "deny"
        assert out["deny_code"] == "policy.budget_exceeded"

    def test_forwards_aprp(self) -> None:
        client = fake_client(ALLOW_RESULT)
        t = sbo3l_tool(client=client)
        t.func(APRP)
        body = client.submit.call_args.args[0]
        assert body["agent_id"] == "research-agent-01"


class TestInputValidation:
    def test_rejects_non_json(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func("not-json"))
        assert out["error"] == "input is not valid JSON"

    def test_rejects_array(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func("[1]"))
        assert out["input_received_type"] == "array"

    def test_rejects_null(self) -> None:
        t = sbo3l_tool(client=fake_client(ALLOW_RESULT))
        out = json.loads(t.func("null"))
        assert out["input_received_type"] == "null"


class TestErrors:
    def test_surfaces_sbo3l_code(self) -> None:
        err = type("E", (Exception,), {"code": "auth.required", "status": 401})("x")
        client = MagicMock()
        client.submit = MagicMock(side_effect=err)
        t = sbo3l_tool(client=client)
        out = json.loads(t.func(APRP))
        assert out["error"] == "auth.required"
        assert out["status"] == 401

    def test_transport_fallback(self) -> None:
        client = MagicMock()
        client.submit = MagicMock(side_effect=ConnectionError("ECONNREFUSED"))
        t = sbo3l_tool(client=client)
        out = json.loads(t.func(APRP))
        assert out["error"] == "transport.failed"


class TestIdempotency:
    def test_callback_invoked(self) -> None:
        client = fake_client(ALLOW_RESULT)
        t = sbo3l_tool(client=client, idempotency_key=lambda body: f"{body['task_id']}-key")
        t.func(APRP)
        assert client.submit.call_args.kwargs["idempotency_key"] == "demo-1-key"


def test_version_exported() -> None:
    import sbo3l_llamaindex

    assert isinstance(sbo3l_llamaindex.__version__, str)
