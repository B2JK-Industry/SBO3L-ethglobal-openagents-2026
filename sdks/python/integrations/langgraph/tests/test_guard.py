"""Tests for sbo3l_langgraph.PolicyGuardNode.

Per Daniel's no-mocks-of-SDK QA rule (2026-05-01): we use the REAL
sbo3l_sdk.SBO3LClientSync. Underlying httpx is mocked via pytest-httpx
to keep the daemon out of CI while exercising the full SDK code path.
"""

from __future__ import annotations

import json
from typing import Any

import httpx
import pytest
from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync

from sbo3l_langgraph import (
    DENIED,
    PolicyGuardNode,
    route_after_guard,
)

_DAEMON = "http://localhost:8730"

APRP: dict[str, Any] = {
    "agent_id": "research-agent-01",
    "task_id": "demo-1",
    "intent": "purchase_api_call",
    "amount": {"value": "0.05", "currency": "USD"},
    "token": "USDC",
    "destination": {
        "type": "x402_endpoint",
        "url": "https://api.example.com/v1/inference",
        "method": "POST",
    },
    "payment_protocol": "x402",
    "chain": "base",
    "provider_url": "https://api.example.com",
    "expiry": "2026-05-01T10:31:00Z",
    "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
    "risk_class": "low",
}

ALLOW_ENVELOPE: dict[str, Any] = {
    "status": "auto_approved",
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-low-risk-x402",
    "request_hash": "c0bd2fab" * 8,
    "policy_hash": "e044f13c" * 8,
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    "receipt": {
        "receipt_type": "sbo3l.policy_receipt.v1",
        "version": 1,
        "agent_id": "research-agent-01",
        "decision": "allow",
        "deny_code": None,
        "request_hash": "c0bd2fab" * 8,
        "policy_hash": "e044f13c" * 8,
        "policy_version": 1,
        "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
        "execution_ref": "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
        "issued_at": "2026-04-29T10:00:00Z",
        "expires_at": None,
        "signature": {
            "algorithm": "ed25519",
            "key_id": "decision-mock-v1",
            "signature_hex": "1" * 128,
        },
    },
}

DENY_ENVELOPE: dict[str, Any] = {
    **ALLOW_ENVELOPE,
    "status": "rejected",
    "decision": "deny",
    "deny_code": "policy.budget_exceeded",
    "matched_rule_id": "daily-budget",
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGT",
    "receipt": {
        **ALLOW_ENVELOPE["receipt"],
        "decision": "deny",
        "deny_code": "policy.budget_exceeded",
    },
}


# ---------------------------------------------------------------------------


class TestPolicyGuardAllowPath:
    def test_writes_policy_receipt_on_allow(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c)
            out = guard({"proposed_action": APRP})
        assert "policy_receipt" in out
        assert "deny_reason" not in out
        assert out["policy_receipt"]["decision"] == "allow"
        assert out["policy_receipt"]["receipt"]["execution_ref"] == "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"

    def test_forwards_aprp_to_sdk(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c)
            guard({"proposed_action": APRP})
        req = httpx_mock.get_request()
        assert req is not None
        body = json.loads(req.content)
        assert body["agent_id"] == "research-agent-01"

    def test_idempotency_callback_invoked(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c, idempotency_key=lambda a: f"{a['task_id']}-key")
            guard({"proposed_action": APRP})
        req = httpx_mock.get_request()
        assert req is not None
        assert req.headers["Idempotency-Key"] == "demo-1-key"


class TestPolicyGuardDenyPath:
    def test_writes_deny_reason_on_deny(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=DENY_ENVELOPE, status_code=200)
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c)
            out = guard({"proposed_action": APRP})
        assert "deny_reason" in out
        assert "policy_receipt" not in out
        assert out["deny_reason"]["code"] == "policy.budget_exceeded"
        assert out["deny_reason"]["decision"] == "deny"
        assert out["deny_reason"]["matched_rule_id"] == "daily-budget"
        assert out["deny_reason"]["audit_event_id"] == "evt-01HTAWX5K3R8YV9NQB7C6P2DGT"

    def test_writes_deny_reason_on_requires_human(self, httpx_mock: HTTPXMock) -> None:
        env = {
            **DENY_ENVELOPE,
            "decision": "requires_human",
            "deny_code": None,
            "receipt": {
                **DENY_ENVELOPE["receipt"],
                "decision": "requires_human",
                "deny_code": None,
            },
        }
        httpx_mock.add_response(json=env, status_code=200)
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c)
            out = guard({"proposed_action": APRP})
        assert out["deny_reason"]["decision"] == "requires_human"
        assert out["deny_reason"]["code"] == "policy.requires_human"


class TestPolicyGuardErrorPath:
    def test_no_proposed_action(self) -> None:
        # Construct guard with a stub client (won't be called).
        guard = PolicyGuardNode(_StubClient())
        out = guard({})
        assert out["deny_reason"]["code"] == "input.no_proposed_action"

    def test_proposed_action_not_dict(self) -> None:
        guard = PolicyGuardNode(_StubClient())
        out = guard({"proposed_action": "not-a-dict"})
        assert out["deny_reason"]["code"] == "input.no_proposed_action"

    def test_sbo3l_error_surfaced(self, httpx_mock: HTTPXMock) -> None:
        problem = {
            "type": "https://schemas.sbo3l.dev/errors/auth.required",
            "title": "Authentication required",
            "status": 401,
            "detail": "missing",
            "code": "auth.required",
        }
        httpx_mock.add_response(json=problem, status_code=401)
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c)
            out = guard({"proposed_action": APRP})
        assert out["deny_reason"]["code"] == "auth.required"
        assert out["deny_reason"]["status"] == 401
        assert out["deny_reason"]["decision"] == "error"

    def test_transport_failure_surfaced(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_exception(httpx.ConnectError("ECONNREFUSED"))
        with SBO3LClientSync(_DAEMON) as c:
            guard = PolicyGuardNode(c)
            out = guard({"proposed_action": APRP})
        assert out["deny_reason"]["code"] == "transport.failed"
        assert "ECONNREFUSED" in out["deny_reason"]["detail"]

    def test_async_client_in_sync_graph(self) -> None:
        guard = PolicyGuardNode(_AsyncStubClient())
        out = guard({"proposed_action": APRP})
        assert out["deny_reason"]["code"] == "transport.async_client_in_sync_graph"


class TestRouteAfterGuard:
    def test_routes_to_execute_on_allow(self) -> None:
        assert route_after_guard({"policy_receipt": {"decision": "allow"}}) == "execute"

    def test_routes_to_denied_on_deny(self) -> None:
        assert route_after_guard({"deny_reason": {"code": "x"}}) == DENIED

    def test_routes_to_denied_on_empty(self) -> None:
        assert route_after_guard({}) == DENIED


def test_version_exported() -> None:
    import sbo3l_langgraph

    assert isinstance(sbo3l_langgraph.__version__, str)


# ---------------------------------------------------------------------------


class _StubClient:
    """Sync client stub that is never called (used for input-validation tests)."""

    def submit(self, request: dict[str, Any], **_: Any) -> Any:  # pragma: no cover
        raise AssertionError("stub.submit should not be called for input-validation paths")


class _AsyncStubClient:
    """Stub whose `submit` returns an awaitable so we can test the sync-vs-async guard."""

    def submit(self, request: dict[str, Any], **_: Any) -> Any:
        return _FakeAwaitable()


class _FakeAwaitable:
    def __await__(self) -> Any:  # pragma: no cover (never awaited in this test)
        yield  # pragma: no cover

    def close(self) -> None:
        pass


# Suppress pytest_httpx's strict-unmatched-request check when a test
# deliberately doesn't fire a request (input-validation paths).
@pytest.fixture(autouse=True)
def _allow_no_requests(httpx_mock: HTTPXMock) -> None:
    httpx_mock.non_mocked_hosts = []
