"""Auth helpers for the SBO3L Python SDK.

The wire shape (bearer or JWT in `Authorization: Bearer ...`, JWT with
`sub == agent_id`) is set by F-1 in `crates/sbo3l-server/src/auth.rs`. These
helpers only assemble the header — JWT signing is the caller's job (or a
downstream signing service in the no-key boundary model). The SDK never
holds a private key.
"""

from __future__ import annotations

import base64
import json
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any, Literal


@dataclass(frozen=True, slots=True)
class _AuthBearer:
    kind: Literal["bearer"]
    token: str


@dataclass(frozen=True, slots=True)
class _AuthJwt:
    kind: Literal["jwt"]
    token: str


@dataclass(frozen=True, slots=True)
class _AuthJwtSupplier:
    kind: Literal["jwt-supplier"]
    supplier: Callable[[], str | Awaitable[str]]


@dataclass(frozen=True, slots=True)
class _AuthNone:
    kind: Literal["none"] = "none"


AuthConfig = _AuthBearer | _AuthJwt | _AuthJwtSupplier | _AuthNone


def bearer(token: str) -> _AuthBearer:
    """Bearer-token auth (matches the F-1 server's `SBO3L_BEARER_TOKEN_HASH` mode)."""

    return _AuthBearer(kind="bearer", token=token)


def jwt(token: str) -> _AuthJwt:
    """Static JWT (EdDSA) auth (matches F-1's `SBO3L_JWT_PUBKEY_HEX` mode)."""

    return _AuthJwt(kind="jwt", token=token)


def jwt_supplier(
    supplier: Callable[[], str | Awaitable[str]],
) -> _AuthJwtSupplier:
    """Per-request JWT supplier — invoked on every call so callers can rotate."""

    return _AuthJwtSupplier(kind="jwt-supplier", supplier=supplier)


def none() -> _AuthNone:
    """No auth — daemon will reject unless `SBO3L_ALLOW_UNAUTHENTICATED=1`."""

    return _AuthNone()


async def auth_header(auth: AuthConfig) -> str | None:
    """Build the `Authorization` header value for the given config.

    Returns `None` for kind=`none` so callers can omit the header entirely
    (rather than sending an empty `Authorization`).
    """

    if isinstance(auth, _AuthNone):
        return None
    if isinstance(auth, _AuthBearer):
        return f"Bearer {auth.token}"
    if isinstance(auth, _AuthJwt):
        return f"Bearer {auth.token}"
    if isinstance(auth, _AuthJwtSupplier):
        result = auth.supplier()
        token: object
        if isinstance(result, Awaitable):
            token = await result
        else:
            token = result
        if not isinstance(token, str):
            raise TypeError(f"jwt_supplier must return str, got {type(token).__name__}")
        return f"Bearer {token}"
    raise TypeError(f"unknown auth kind: {auth!r}")  # pragma: no cover


def decode_jwt_claims(token: str) -> dict[str, Any]:
    """Decode a JWT *without verifying its signature*.

    Returns the parsed claim set. The SDK does not verify JWT signatures
    client-side — the daemon does that against `SBO3L_JWT_PUBKEY_HEX`. Use
    this helper to inspect a JWT before sending (e.g. confirm `sub` matches).

    Raises `ValueError` on malformed tokens.
    """

    parts = token.split(".")
    if len(parts) != 3:
        raise ValueError("invalid JWT: expected three dot-separated segments")
    payload = parts[1]
    if len(payload) == 0:
        raise ValueError("invalid JWT: empty payload segment")
    raw = _base64url_decode(payload)
    try:
        claims = json.loads(raw)
    except json.JSONDecodeError as e:
        raise ValueError(f"invalid JWT: payload is not valid JSON ({e})") from e
    if not isinstance(claims, dict):
        raise ValueError("invalid JWT: payload is not a JSON object")
    return claims


def assert_jwt_sub_matches(token: str, expected_agent_id: str) -> None:
    """Confirm a JWT's `sub` claim equals `expected_agent_id`.

    Mirrors the F-1 daemon-side check; running it client-side surfaces a
    misconfigured token before the round-trip.
    """

    claims = decode_jwt_claims(token)
    sub = claims.get("sub")
    if not isinstance(sub, str):
        raise ValueError("invalid JWT: missing or non-string 'sub' claim")
    if sub != expected_agent_id:
        raise ValueError(
            f"JWT 'sub' claim {sub!r} does not match expected agent_id {expected_agent_id!r}"
        )


def _base64url_decode(s: str) -> bytes:
    pad = "=" * (-len(s) % 4)
    return base64.urlsafe_b64decode(s + pad)
