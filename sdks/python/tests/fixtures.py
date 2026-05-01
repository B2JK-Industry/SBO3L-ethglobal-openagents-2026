"""Test fixtures shared across the pytest suite.

Mirrors `sdks/typescript/test/fixtures.ts` for cross-SDK consistency. Every
golden value is reused from `test-corpus/` where applicable.
"""

from __future__ import annotations

import copy
from typing import Any

GOLDEN_APRP: dict[str, Any] = {
    "agent_id": "research-agent-01",
    "task_id": "demo-task-1",
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
    "x402_payload": None,
    "expiry": "2026-05-01T10:31:00Z",
    "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
    "expected_result": None,
    "risk_class": "low",
}

_HEX64A = "c0bd2fab1234567890abcdef1234567890abcdef1234567890abcdef12345678"
_HEX64B = "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf"
_HEX64C = "6cba2eed67c2dfd623521be0a692b8716f300eb27deb3a7e9ab06d5e8b3bb9e6"
_HEX64D = "ed00a7f7d5caed85960dfb815d079531e6fd2f2019e61c655e5d156e5db0708a"
_SIG128 = (
    "1111111111111111111111111111111111111111111111111111111111111111"
    "1111111111111111111111111111111111111111111111111111111111111111"
)


GOLDEN_CAPSULE_V1: dict[str, Any] = {
    "schema": "sbo3l.passport_capsule.v1",
    "generated_at": "2026-04-29T10:00:00Z",
    "agent": {
        "agent_id": "research-agent-01",
        "ens_name": "research-agent.team.eth",
        "resolver": "offline-fixture",
        "records": {"sbo3l:policy_hash": _HEX64B},
    },
    "request": {
        "aprp": GOLDEN_APRP,
        "request_hash": _HEX64A,
        "idempotency_key": "demo-key-1",
        "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
    },
    "policy": {
        "policy_hash": _HEX64B,
        "policy_version": 1,
        "activated_at": "2026-04-28T10:00:00Z",
        "source": "operator-cli",
    },
    "decision": {
        "result": "allow",
        "matched_rule": "allow-low-risk-x402",
        "deny_code": None,
        "receipt": {
            "receipt_type": "sbo3l.policy_receipt.v1",
            "version": 1,
            "agent_id": "research-agent-01",
            "decision": "allow",
            "deny_code": None,
            "request_hash": _HEX64A,
            "policy_hash": _HEX64B,
            "policy_version": 1,
            "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
            "execution_ref": "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
            "issued_at": "2026-04-29T10:00:00Z",
            "expires_at": None,
            "signature": {
                "algorithm": "ed25519",
                "key_id": "decision-mock-v1",
                "signature_hex": _SIG128,
            },
        },
        "receipt_signature": _SIG128,
    },
    "execution": {
        "executor": "keeperhub",
        "mode": "mock",
        "execution_ref": "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
        "status": "submitted",
        "sponsor_payload_hash": _HEX64C,
        "live_evidence": None,
    },
    "audit": {
        "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
        "prev_event_hash": "0" * 64,
        "event_hash": _HEX64C,
        "bundle_ref": "sbo3l.audit_bundle.v1",
        "checkpoint": {
            "schema": "sbo3l.audit_checkpoint.v1",
            "sequence": 1,
            "latest_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
            "latest_event_hash": _HEX64C,
            "chain_digest": _HEX64D,
            "mock_anchor": True,
            "mock_anchor_ref": "local-mock-anchor-9202d6bc7b751225",
            "created_at": "2026-04-28T19:58:54Z",
        },
    },
    "verification": {
        "doctor_status": "ok",
        "offline_verifiable": True,
        "live_claims": [],
    },
}


GOLDEN_ENVELOPE: dict[str, Any] = {
    "status": "auto_approved",
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-low-risk-x402",
    "request_hash": _HEX64A,
    "policy_hash": _HEX64B,
    "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    "receipt": GOLDEN_CAPSULE_V1["decision"]["receipt"],
}


def clone(x: dict[str, Any]) -> dict[str, Any]:
    """Deep-clone helper so per-test mutations don't bleed."""

    return copy.deepcopy(x)


def build_capsule_v2() -> dict[str, Any]:
    """v2 capsule = v1 with policy_snapshot + audit_segment + bumped schema id."""

    v2 = clone(GOLDEN_CAPSULE_V1)
    v2["schema"] = "sbo3l.passport_capsule.v2"
    v2["policy"]["policy_snapshot"] = {
        "version": 1,
        "rules": [{"id": "allow-low-risk-x402", "effect": "allow"}],
    }
    v2["audit"]["audit_segment"] = {"events": []}
    return v2
