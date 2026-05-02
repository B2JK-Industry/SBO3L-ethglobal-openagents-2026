"""APRP-builder for Uniswap swaps (Python mirror of TS `aprp_for_swap`)."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .sepolia import SEPOLIA_USDC, SEPOLIA_WETH


@dataclass(frozen=True, slots=True)
class SwapAprpParams:
    """Inputs for `aprp_for_swap` — kept positional-friendly via dataclass."""

    agent_id: str
    task_id: str
    amount_usd: str
    nonce: str
    expiry: str
    token_in: str | None = None
    token_out: str | None = None
    risk_class: str = "medium"


def aprp_for_swap(p: SwapAprpParams) -> dict[str, Any]:
    """Build an APRP body for a Uniswap swap intent. Submit via
    `SBO3LClientSync.submit()` BEFORE invoking the actual swap."""

    token_in = p.token_in if p.token_in else SEPOLIA_WETH
    token_out = (
        p.token_out
        if p.token_out
        else (SEPOLIA_USDC if token_in.lower() == SEPOLIA_WETH.lower() else SEPOLIA_WETH)
    )
    return {
        "agent_id": p.agent_id,
        "task_id": p.task_id,
        "intent": "purchase_api_call",
        "amount": {"value": p.amount_usd, "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "erc20_transfer",
            "token_address": token_in,
            "recipient": token_out,
        },
        "payment_protocol": "erc20_transfer",
        "chain": "sepolia",
        "provider_url": "https://sepolia.etherscan.io",
        "expiry": p.expiry,
        "nonce": p.nonce,
        "risk_class": p.risk_class,
    }
