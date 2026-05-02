"""End-to-end test of the demo's tool path with real sbo3l_sdk + httpx_mock."""

from __future__ import annotations

import json
from typing import Any

from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync

from sbo3l_llamaindex_demo.tools import build_sbo3l_pay_func

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

APRP = json.dumps(
    {"agent_id": "research-agent-01", "task_id": "demo-llamaindex-1", "intent": "purchase_api_call"}
)


def test_returns_allow_envelope(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
    with SBO3LClientSync("http://localhost:8730") as c:
        sbo3l_pay = build_sbo3l_pay_func(c)
        out = json.loads(sbo3l_pay(APRP))
    assert out["decision"] == "allow"
    assert out["execution_ref"] == "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"


def test_smoke_module_importable() -> None:
    from sbo3l_llamaindex_demo import smoke

    assert callable(smoke.main)
