"""End-to-end test of the 3-node graph using real sbo3l_sdk + httpx_mock.

Per Daniel's no-mocks-of-SDK QA rule: real SBO3LClientSync is wired into
the graph; httpx layer is mocked to keep the daemon out of CI.
"""

from __future__ import annotations

from typing import Any

from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync

from sbo3l_example_langgraph import build_app

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


class TestGraphAllowPath:
    def test_full_graph_run_executes_on_allow(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
        with SBO3LClientSync("http://localhost:8730") as c:
            app = build_app(c)
            final = app.invoke({})
        assert "policy_receipt" in final
        assert "result" in final
        assert "kh-01HTAWX5K3R8YV9NQB7C6P2DGS" in final["result"]
        assert "deny_reason" not in final


class TestGraphDenyPath:
    def test_graph_short_circuits_on_deny(self, httpx_mock: HTTPXMock) -> None:
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
        httpx_mock.add_response(json=deny, status_code=200)
        with SBO3LClientSync("http://localhost:8730") as c:
            app = build_app(c)
            final = app.invoke({})
        assert "deny_reason" in final
        assert final["deny_reason"]["code"] == "policy.budget_exceeded"
        # `execute` node MUST NOT have run
        assert "result" not in final
