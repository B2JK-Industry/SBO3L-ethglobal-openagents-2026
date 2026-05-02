"""End-to-end test of the KH-flavored tool path with real sbo3l_sdk + httpx_mock.

The mock daemon returns an allow envelope with `execution_ref` populated as
if the daemon's KeeperHub adapter ran. We verify the keeperhub_tool wrapper
surfaces it as `kh_execution_ref` and exposes the configured workflow id.
"""

from __future__ import annotations

import json
from typing import Any

from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync

from sbo3l_langchain_demo.keeperhub_tool import (
    DEFAULT_KH_WORKFLOW_ID,
    build_demo_aprp,
    keeperhub_tool,
)

KH_EXECUTION_REF = "kh-01HTAWX5K3R8YV9NQB7C6P2DGZ"

ALLOW_ENVELOPE: dict[str, Any] = {
    "status": "auto_approved",
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-low-risk-x402-keeperhub",
    "request_hash": "c0bd2fab" * 8,
    "policy_hash": "e044f13c" * 8,
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    "receipt": {
        "receipt_type": "sbo3l.policy_receipt.v1",
        "version": 1,
        "agent_id": "research-agent-kh-01",
        "decision": "allow",
        "deny_code": None,
        "request_hash": "c0bd2fab" * 8,
        "policy_hash": "e044f13c" * 8,
        "policy_version": 1,
        "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
        "execution_ref": KH_EXECUTION_REF,
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
    "status": "rejected",
    "decision": "deny",
    "deny_code": "policy.amount_over_limit",
    "matched_rule_id": "deny-high-amount",
    "request_hash": "deadbeef" * 8,
    "policy_hash": "cafebabe" * 8,
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGS",
    "receipt": {
        "receipt_type": "sbo3l.policy_receipt.v1",
        "version": 1,
        "agent_id": "research-agent-kh-01",
        "decision": "deny",
        "deny_code": "policy.amount_over_limit",
        "request_hash": "deadbeef" * 8,
        "policy_hash": "cafebabe" * 8,
        "policy_version": 1,
        "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGS",
        "execution_ref": None,
        "issued_at": "2026-04-29T10:00:00Z",
        "expires_at": None,
        "signature": {
            "algorithm": "ed25519",
            "key_id": "decision-mock-v1",
            "signature_hex": "2" * 128,
        },
    },
}


def test_allow_envelope_surfaces_kh_execution_ref(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
    aprp = build_demo_aprp()

    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = keeperhub_tool(client=c)
        out = json.loads(descriptor.func(json.dumps(aprp)))

    assert out["decision"] == "allow"
    assert out["kh_execution_ref"] == KH_EXECUTION_REF
    assert out["kh_workflow_id"] == DEFAULT_KH_WORKFLOW_ID
    assert out["audit_event_id"] == "evt-01HTAWX5K3R8YV9NQB7C6P2DGR"
    assert out["request_hash"] == "c0bd2fab" * 8
    assert out["deny_code"] is None


def test_deny_envelope_does_not_surface_execution_ref(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=DENY_ENVELOPE, status_code=200)
    aprp = build_demo_aprp(amount_usd="10000.00")  # well over policy limit

    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = keeperhub_tool(client=c)
        out = json.loads(descriptor.func(json.dumps(aprp)))

    assert out["decision"] == "deny"
    # execution_ref must be None on deny — daemon never asks the KH adapter to run.
    assert out["kh_execution_ref"] is None
    # workflow_id is still surfaced for context (the agent needs to know which
    # workflow was *attempted* even though execution didn't happen).
    assert out["kh_workflow_id"] == DEFAULT_KH_WORKFLOW_ID
    assert out["deny_code"] == "policy.amount_over_limit"


def test_workflow_id_override(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)
    aprp = build_demo_aprp()

    custom_workflow = "kh-staging-test-workflow-xyz"
    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = keeperhub_tool(client=c, workflow_id=custom_workflow)
        out = json.loads(descriptor.func(json.dumps(aprp)))

    assert out["kh_workflow_id"] == custom_workflow


def test_invalid_input_returns_error_envelope() -> None:
    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = keeperhub_tool(client=c)
        bad_json = descriptor.func("{not valid json")
        out = json.loads(bad_json)

    assert "error" in out
    assert "JSON" in out["error"] or "json" in out["error"]


def test_input_array_returns_error_envelope() -> None:
    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = keeperhub_tool(client=c)
        out = json.loads(descriptor.func(json.dumps([1, 2, 3])))

    assert "error" in out
    assert out.get("input_received_type") == "array"


def test_smoke_module_importable() -> None:
    from sbo3l_langchain_demo import keeperhub_smoke

    assert callable(keeperhub_smoke.main)
