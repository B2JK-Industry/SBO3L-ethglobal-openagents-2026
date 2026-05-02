"""End-to-end test — APRP submit + swap calldata, real sbo3l_sdk + httpx_mock."""

from __future__ import annotations

from typing import Any

from pytest_httpx import HTTPXMock
from sbo3l_sdk import SBO3LClientSync, uniswap

ALLOW_ENVELOPE: dict[str, Any] = {
    "status": "auto_approved",
    "decision": "allow",
    "deny_code": None,
    "matched_rule_id": "allow-medium-risk-erc20",
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


def test_aprp_submit_then_swap_construction(httpx_mock: HTTPXMock) -> None:
    """SBO3L policy gate clears, then SDK builds correct SwapRouter02 calldata."""

    httpx_mock.add_response(json=ALLOW_ENVELOPE, status_code=200)

    aprp = uniswap.aprp_for_swap(
        uniswap.SwapAprpParams(
            agent_id="research-agent-01",
            task_id="uniswap-py-test-1",
            amount_usd="1.50",
            nonce="01HTAWX5K3R8YV9NQB7C6P2DGM",
            expiry="2026-05-01T10:31:00Z",
        )
    )

    with SBO3LClientSync("http://localhost:8730") as client:
        decision = client.submit(aprp)

    assert decision.decision == "allow"

    result = uniswap.swap(
        uniswap.SwapParams(
            token_in=uniswap.SEPOLIA_WETH,
            token_out=uniswap.SEPOLIA_USDC,
            fee=3_000,
            recipient="0x" + "AA" * 20,
            amount_in=10_000_000_000_000_000,
            amount_out_minimum=1_000_000,
        )
    )
    assert result.mode == "mock"
    assert result.to == uniswap.SEPOLIA_SWAP_ROUTER_02
    assert result.tx_hash.startswith("0x") and len(result.tx_hash) == 66
    assert result.etherscan_url.startswith("https://sepolia.etherscan.io/tx/")
    # Calldata is selector + 7 × 32-byte words (hex form: 2 + 458 chars)
    assert len(result.calldata) == 2 + (4 + 7 * 32) * 2


def test_smoke_module_importable() -> None:
    from sbo3l_uniswap_demo import smoke

    assert callable(smoke.main)
