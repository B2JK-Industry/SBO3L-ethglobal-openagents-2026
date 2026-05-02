"""`swap()` — agent-side execution after SBO3L's policy gate has cleared.

Mock mode (default): deterministic pseudo-tx-hash from SwapParams + router.
Live mode (`SBO3L_LIVE_ETH=1` env or `env: SwapEnv` arg): builds calldata
and signs/broadcasts via `web3.py`. Live mode requires `web3` extra:

    pip install "sbo3l-sdk[web3]"

The signing primitive is `web3.py`'s `Account.sign_transaction` (so the
private key never leaves the agent process). SBO3L's daemon is policy-only
— the no-key boundary is preserved.

The signing import is deferred so callers without web3 installed can still
use `encode_exact_input_single` + `swap()` in mock mode.
"""

from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass
from typing import Any, Literal

from .sepolia import (
    EXACT_INPUT_SINGLE_SELECTOR,
    SEPOLIA_CHAIN_ID,
    SEPOLIA_SWAP_ROUTER_02,
    sepolia_etherscan_tx_url,
)


@dataclass(frozen=True, slots=True)
class SwapParams:
    """Single-pool exact-input swap parameters. Mirrors Rust's `SwapParams`
    and TS's `SwapParams`. All amounts in token's smallest unit."""

    token_in: str
    token_out: str
    fee: int
    recipient: str
    amount_in: int
    amount_out_minimum: int
    sqrt_price_limit_x96: int = 0


@dataclass(frozen=True, slots=True)
class SwapEnv:
    """Live-mode env overrides. Pass to `swap(env=...)` to bypass `os.environ`."""

    rpc_url: str
    private_key_hex: str
    chain_id: int = SEPOLIA_CHAIN_ID
    router_address: str = SEPOLIA_SWAP_ROUTER_02


@dataclass(frozen=True, slots=True)
class SwapResult:
    """`mock` mode: pseudo-tx-hash; `live` mode: real on-chain tx hash."""

    mode: Literal["mock", "live"]
    tx_hash: str
    etherscan_url: str
    calldata: str  # 0x-prefixed
    to: str  # router address


def encode_exact_input_single(p: SwapParams) -> str:
    """Encode the SwapRouter02 `exactInputSingle` calldata. Mirrors Rust's
    `encode_exact_input_single` and TS's `encodeExactInputSingle`. Returns
    a 0x-prefixed lowercase hex string."""

    out = bytearray()
    out += EXACT_INPUT_SINGLE_SELECTOR
    out += _address_padded(_parse_address(p.token_in))
    out += _address_padded(_parse_address(p.token_out))
    out += _uint_padded(p.fee)
    out += _address_padded(_parse_address(p.recipient))
    out += _uint_padded(p.amount_in)
    out += _uint_padded(p.amount_out_minimum)
    out += _uint_padded(p.sqrt_price_limit_x96)
    return "0x" + out.hex()


def swap(params: SwapParams, env: SwapEnv | None = None) -> SwapResult:
    """Build + (in live mode) sign + broadcast a SwapRouter02 swap.

    Mock mode (no `env` and `SBO3L_LIVE_ETH != "1"`): returns deterministic
    pseudo-tx-hash so demos run in CI without secrets.

    Live mode: requires `web3.py` installed. Raises `RuntimeError` with a
    clear hint if web3 isn't available.
    """

    calldata = encode_exact_input_single(params)
    router = env.router_address if env else SEPOLIA_SWAP_ROUTER_02

    live_enabled = env is not None or os.environ.get("SBO3L_LIVE_ETH") == "1"
    if not live_enabled:
        # Deterministic pseudo-hash matches TS implementation: sha256(calldata + router)[:64]
        h = hashlib.sha256((calldata + router).encode("ascii")).hexdigest()[:64]
        tx_hash = "0x" + h
        return SwapResult(
            mode="mock",
            tx_hash=tx_hash,
            etherscan_url=sepolia_etherscan_tx_url(tx_hash),
            calldata=calldata,
            to=router,
        )

    live_env = env if env else _env_from_process()
    tx_hash = _broadcast_legacy_tx(
        live_env.rpc_url,
        live_env.private_key_hex,
        live_env.chain_id,
        router,
        calldata,
    )
    return SwapResult(
        mode="live",
        tx_hash=tx_hash,
        etherscan_url=sepolia_etherscan_tx_url(tx_hash),
        calldata=calldata,
        to=router,
    )


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _env_from_process() -> SwapEnv:
    rpc_url = os.environ.get("SBO3L_ETH_RPC_URL")
    private_key_hex = os.environ.get("SBO3L_ETH_PRIVATE_KEY")
    if rpc_url is None or private_key_hex is None:
        raise RuntimeError(
            "live swap requires SBO3L_ETH_RPC_URL + SBO3L_ETH_PRIVATE_KEY (or pass `env`)"
        )
    return SwapEnv(rpc_url=rpc_url, private_key_hex=private_key_hex)


def _parse_address(s: str) -> bytes:
    trimmed = s[2:] if s.startswith(("0x", "0X")) else s
    if len(trimmed) != 40:
        raise ValueError(f"address must be 0x + 40 hex chars, got {len(trimmed)}")
    try:
        return bytes.fromhex(trimmed)
    except ValueError as e:
        raise ValueError(f"address contains non-hex character: {e}") from e


def _address_padded(addr: bytes) -> bytes:
    """Left-pad a 20-byte address to a 32-byte word."""

    if len(addr) != 20:
        raise ValueError("address must be exactly 20 bytes")
    return b"\x00" * 12 + addr


def _uint_padded(v: int) -> bytes:
    """Right-pack a non-negative int into a 32-byte big-endian word."""

    if v < 0:
        raise ValueError("uint must be non-negative")
    return v.to_bytes(32, "big")


def _broadcast_legacy_tx(
    rpc_url: str,
    private_key_hex: str,
    chain_id: int,
    to: str,
    data: str,
) -> str:
    """Sign + broadcast a legacy (type-0) transaction via web3.py.

    Sepolia accepts both legacy and EIP-1559; legacy is simpler (no fee
    oracle dependency) and sufficient for the demo path. Production
    callers should use EIP-1559 with a proper fee bumping strategy.
    """

    try:
        from web3 import Web3  # type: ignore[import-not-found]
    except ImportError as e:
        raise RuntimeError(
            'live swap requires `web3.py` — install with `pip install "sbo3l-sdk[web3]"` '
            "or `pip install web3`."
        ) from e

    w3 = Web3(Web3.HTTPProvider(rpc_url))
    if not w3.is_connected():
        raise RuntimeError(f"could not connect to RPC at {rpc_url}")

    account = w3.eth.account.from_key(private_key_hex)
    nonce = w3.eth.get_transaction_count(account.address, "pending")
    gas_price = w3.eth.gas_price

    tx: dict[str, Any] = {
        "nonce": nonce,
        "gasPrice": gas_price,
        "gas": 300_000,
        "to": Web3.to_checksum_address(to),
        "value": 0,
        "data": data,
        "chainId": chain_id,
    }

    signed = account.sign_transaction(tx)
    tx_hash_bytes = w3.eth.send_raw_transaction(signed.rawTransaction)
    # web3.py returns HexBytes; .hex() is typed loosely in some versions,
    # so cast to str to keep `mypy --strict` happy.
    return str(tx_hash_bytes.hex())
