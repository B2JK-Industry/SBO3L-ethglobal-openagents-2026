"""Tests for sbo3l_sdk.uniswap — calldata layout, mock swap, APRP builder."""

from __future__ import annotations

import hashlib

from sbo3l_sdk import uniswap
from sbo3l_sdk.uniswap import (
    EXACT_INPUT_SINGLE_SELECTOR,
    SEPOLIA_CHAIN_ID,
    SEPOLIA_SWAP_ROUTER_02,
    SEPOLIA_USDC,
    SEPOLIA_WETH,
    SwapAprpParams,
    SwapParams,
    aprp_for_swap,
    encode_exact_input_single,
    sepolia_etherscan_tx_url,
    swap,
)

RECIPIENT = "0x" + "AA" * 20

BASIC = SwapParams(
    token_in=SEPOLIA_WETH,
    token_out=SEPOLIA_USDC,
    fee=3_000,
    recipient=RECIPIENT,
    amount_in=10_000_000_000_000_000,  # 0.01 WETH
    amount_out_minimum=1_000_000,  # 1 USDC slippage floor
)


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------


class TestConstants:
    def test_sepolia_chain_id(self) -> None:
        assert SEPOLIA_CHAIN_ID == 11_155_111

    def test_selector_pinned(self) -> None:
        assert EXACT_INPUT_SINGLE_SELECTOR == bytes.fromhex("04e45aaf")

    def test_address_shapes(self) -> None:
        for a in (SEPOLIA_SWAP_ROUTER_02, SEPOLIA_USDC, SEPOLIA_WETH):
            assert a.startswith("0x")
            assert len(a) == 42

    def test_uniswap_namespace_re_exported(self) -> None:
        # `from sbo3l_sdk import uniswap` should yield the submodule.
        assert hasattr(uniswap, "swap")
        assert hasattr(uniswap, "encode_exact_input_single")


# ---------------------------------------------------------------------------
# Calldata layout
# ---------------------------------------------------------------------------


class TestEncodeExactInputSingle:
    def test_total_length(self) -> None:
        # 0x prefix (2) + 2 chars per byte * (4 + 7*32) bytes
        assert len(encode_exact_input_single(BASIC)) == 2 + (4 + 7 * 32) * 2

    def test_starts_with_selector(self) -> None:
        assert encode_exact_input_single(BASIC).startswith("0x04e45aaf")

    def test_token_in_padded_to_word_1(self) -> None:
        data = encode_exact_input_single(BASIC)
        # Word 1 hex offset 10..(10+64). First 24 hex chars are zeros.
        assert data[10 : 10 + 24] == "0" * 24
        assert data[10 + 24 : 10 + 64].lower() == SEPOLIA_WETH[2:].lower()

    def test_recipient_at_word_4(self) -> None:
        data = encode_exact_input_single(BASIC)
        # Word 4: hex offset 10 + 64*3 = 202.
        assert data[202 : 202 + 24] == "0" * 24
        assert data[202 + 24 : 202 + 64].lower() == RECIPIENT[2:].lower()

    def test_amount_in_at_word_5(self) -> None:
        data = encode_exact_input_single(BASIC)
        # Word 5: hex offset 10 + 64*4 = 266.
        assert int(data[266 : 266 + 64], 16) == BASIC.amount_in

    def test_sqrt_price_limit_zero_default(self) -> None:
        data = encode_exact_input_single(BASIC)
        # Word 7: offset 10 + 64*6 = 394.
        assert data[394 : 394 + 64] == "0" * 64

    def test_sqrt_price_limit_explicit(self) -> None:
        p = SwapParams(
            token_in=SEPOLIA_WETH,
            token_out=SEPOLIA_USDC,
            fee=3_000,
            recipient=RECIPIENT,
            amount_in=1,
            amount_out_minimum=1,
            sqrt_price_limit_x96=0xDEADBEEF,
        )
        data = encode_exact_input_single(p)
        assert int(data[394 : 394 + 64], 16) == 0xDEADBEEF

    def test_rejects_short_address(self) -> None:
        bad = SwapParams(
            token_in="0x123",
            token_out=SEPOLIA_USDC,
            fee=3000,
            recipient=RECIPIENT,
            amount_in=1,
            amount_out_minimum=1,
        )
        try:
            encode_exact_input_single(bad)
        except ValueError as e:
            assert "40 hex" in str(e)
        else:
            raise AssertionError("expected ValueError")

    def test_rejects_negative_uint(self) -> None:
        bad = SwapParams(
            token_in=SEPOLIA_WETH,
            token_out=SEPOLIA_USDC,
            fee=3000,
            recipient=RECIPIENT,
            amount_in=-1,
            amount_out_minimum=0,
        )
        try:
            encode_exact_input_single(bad)
        except (ValueError, OverflowError):
            pass
        else:
            raise AssertionError("expected ValueError or OverflowError")


# ---------------------------------------------------------------------------
# APRP builder
# ---------------------------------------------------------------------------


class TestAprpForSwap:
    def test_default_pair(self) -> None:
        aprp = aprp_for_swap(
            SwapAprpParams(
                agent_id="research-agent-01",
                task_id="swap-1",
                amount_usd="1.50",
                nonce="01HTAWX5K3R8YV9NQB7C6P2DGM",
                expiry="2026-05-01T10:31:00Z",
            )
        )
        assert aprp["chain"] == "sepolia"
        assert aprp["payment_protocol"] == "erc20_transfer"
        assert aprp["destination"]["token_address"] == SEPOLIA_WETH
        assert aprp["destination"]["recipient"] == SEPOLIA_USDC
        assert aprp["risk_class"] == "medium"

    def test_inverts_when_token_in_usdc(self) -> None:
        aprp = aprp_for_swap(
            SwapAprpParams(
                agent_id="research-agent-01",
                task_id="swap-2",
                amount_usd="1.00",
                nonce="01HTAWX5K3R8YV9NQB7C6P2DGM",
                expiry="2026-05-01T10:31:00Z",
                token_in=SEPOLIA_USDC,
            )
        )
        assert aprp["destination"]["recipient"] == SEPOLIA_WETH


# ---------------------------------------------------------------------------
# swap() mock mode
# ---------------------------------------------------------------------------


class TestSwapMockMode:
    def test_deterministic(self) -> None:
        a = swap(BASIC)
        b = swap(BASIC)
        assert a.mode == "mock"
        assert a.tx_hash == b.tx_hash
        assert a.tx_hash.startswith("0x")
        assert len(a.tx_hash) == 66

    def test_different_amounts_different_hashes(self) -> None:
        a = swap(BASIC)
        b = swap(
            SwapParams(
                token_in=BASIC.token_in,
                token_out=BASIC.token_out,
                fee=BASIC.fee,
                recipient=BASIC.recipient,
                amount_in=BASIC.amount_in + 1,
                amount_out_minimum=BASIC.amount_out_minimum,
            )
        )
        assert a.tx_hash != b.tx_hash

    def test_etherscan_url_sepolia(self) -> None:
        r = swap(BASIC)
        assert r.etherscan_url.startswith("https://sepolia.etherscan.io/tx/")

    def test_calldata_matches_encoder(self) -> None:
        r = swap(BASIC)
        assert r.calldata == encode_exact_input_single(BASIC)
        assert r.to == SEPOLIA_SWAP_ROUTER_02

    def test_pseudo_hash_is_sha256_of_calldata_plus_router(self) -> None:
        r = swap(BASIC)
        expected = (
            "0x"
            + hashlib.sha256((r.calldata + SEPOLIA_SWAP_ROUTER_02).encode("ascii")).hexdigest()[:64]
        )
        assert r.tx_hash == expected


# ---------------------------------------------------------------------------
# etherscan url
# ---------------------------------------------------------------------------


def test_etherscan_url_strips_0x_prefix() -> None:
    assert sepolia_etherscan_tx_url("0xdeadbeef") == "https://sepolia.etherscan.io/tx/0xdeadbeef"
    assert sepolia_etherscan_tx_url("deadbeef") == "https://sepolia.etherscan.io/tx/0xdeadbeef"
