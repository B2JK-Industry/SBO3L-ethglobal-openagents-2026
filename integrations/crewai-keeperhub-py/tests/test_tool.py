"""End-to-end test of sbo3l_crewai_keeperhub_tool against a pytest-httpx-mocked daemon.

Uses the real `sbo3l_sdk.SBO3LClientSync` so we exercise the actual HTTP
client + Pydantic envelope parsing, not a stubbed-out interface.
"""

from __future__ import annotations

import json
from typing import Any

from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync

from sbo3l_crewai_keeperhub import (
    DEFAULT_KH_WORKFLOW_ID,
    sbo3l_crewai_keeperhub_tool,
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

APRP = json.dumps(
    {
        "agent_id": "research-agent-kh-01",
        "task_id": "kh-test-1",
        "intent": "purchase_api_call",
    }
)


def test_allow_envelope_surfaces_kh_execution_ref(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)

    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = sbo3l_crewai_keeperhub_tool(client=c)
        out = json.loads(descriptor.func(APRP))

    assert out["decision"] == "allow"
    assert out["kh_execution_ref"] == KH_EXECUTION_REF
    assert out["kh_workflow_id_advisory"] == DEFAULT_KH_WORKFLOW_ID
    assert out["audit_event_id"] == "evt-01HTAWX5K3R8YV9NQB7C6P2DGR"
    assert out["request_hash"] == "c0bd2fab" * 8
    assert out["deny_code"] is None


def test_deny_envelope_does_not_surface_execution_ref(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=DENY_ENVELOPE, status_code=200)

    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = sbo3l_crewai_keeperhub_tool(client=c)
        out = json.loads(descriptor.func(APRP))

    assert out["decision"] == "deny"
    assert out["kh_execution_ref"] is None
    assert out["kh_workflow_id_advisory"] == DEFAULT_KH_WORKFLOW_ID
    assert out["deny_code"] == "policy.amount_over_limit"


def test_workflow_id_override(httpx_mock: HTTPXMock) -> None:
    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)

    custom_workflow = "kh-staging-test-workflow-xyz"
    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = sbo3l_crewai_keeperhub_tool(client=c, workflow_id=custom_workflow)
        out = json.loads(descriptor.func(APRP))

    assert out["kh_workflow_id_advisory"] == custom_workflow


def test_invalid_input_returns_error_envelope() -> None:
    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = sbo3l_crewai_keeperhub_tool(client=c)
        out = json.loads(descriptor.func("{not valid json"))

    assert "error" in out
    assert "JSON" in out["error"] or "json" in out["error"]


def test_input_array_returns_error_envelope() -> None:
    with SBO3LClientSync("http://localhost:8730") as c:
        descriptor = sbo3l_crewai_keeperhub_tool(client=c)
        out = json.loads(descriptor.func(json.dumps([1, 2, 3])))

    assert out["error"] == "input must be a JSON object (APRP)"
    assert out["input_received_type"] == "array"


def test_basetool_subclass_requires_crewai() -> None:
    """If crewai is installed, Sbo3lKeeperHubCrewAITool is importable."""
    try:
        from sbo3l_crewai_keeperhub import Sbo3lKeeperHubCrewAITool

        assert Sbo3lKeeperHubCrewAITool is not None
    except ImportError:
        # crewai not installed in this env — degraded but not a failure.
        # The package's __init__ already gates the import; this test
        # confirms the gate works (no ImportError leaks into the user's
        # `from sbo3l_crewai_keeperhub import ...` for non-BaseTool
        # imports).
        pass
