"""End-to-end test of the demo graph using real sbo3l_sdk + httpx_mock.

The graph's `plan` node calls `data_fetch` (real httpx GET) — we mock both
that AND the SBO3L `submit` POST.
"""

from __future__ import annotations

from typing import Any

from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync

from sbo3l_langgraph_demo import build_app

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


def test_graph_executes_on_allow(httpx_mock: HTTPXMock) -> None:
    # plan's data_fetch (api.example.com) — return a stub
    httpx_mock.add_response(
        url="https://api.example.com/v1", json={"ok": True}, status_code=200
    )
    # SBO3L submit — return allow envelope
    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)

    with SBO3LClientSync("http://localhost:8730") as c:
        app = build_app(c)
        final = app.invoke({"user_request": "pay 0.05 USDC"})

    assert "policy_receipt" in final
    assert "result" in final
    assert "kh-01HTAWX5K3R8YV9NQB7C6P2DGS" in final["result"]


def test_graph_short_circuits_on_deny(httpx_mock: HTTPXMock) -> None:
    deny = {
        **ALLOW_ENVELOPE,
        "decision": "deny",
        "deny_code": "policy.budget_exceeded",
        "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGT",
        "receipt": {
            **ALLOW_ENVELOPE["receipt"],
            "decision": "deny",
            "deny_code": "policy.budget_exceeded",
        },
    }
    httpx_mock.add_response(
        url="https://api.example.com/v1", json={"ok": True}, status_code=200
    )
    httpx_mock.add_response(json=deny, status_code=200)

    with SBO3LClientSync("http://localhost:8730") as c:
        app = build_app(c)
        final = app.invoke({"user_request": "pay 1000 USDC"})

    assert "deny_reason" in final
    assert final["deny_reason"]["code"] == "policy.budget_exceeded"
    assert "result" not in final  # execute must NOT run on deny
