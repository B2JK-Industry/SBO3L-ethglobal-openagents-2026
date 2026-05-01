"""Sync wrappers for the SBO3L Python SDK.

Async-first is the canonical API (see `sbo3l_sdk.client.SBO3LClient`); this
module exposes a synchronous mirror via `httpx.Client`. Sync calls do NOT
delegate via `asyncio.run` — they call `httpx.Client` directly so they're
safe to use inside an existing event loop.
"""

from __future__ import annotations

import json
from typing import Any

import httpx

from ._version import __version__
from .auth import AuthConfig, _AuthBearer, _AuthJwt, _AuthNone
from .client import _normalize_auth, _parse_envelope
from .errors import SBO3LTransportError
from .types import PaymentRequest, PaymentRequestResponse


class SBO3LClientSync:
    """Synchronous SBO3L client. Use `SBO3LClient` for async-first code.

    JWT-supplier auth is **not** supported in the sync client — supplier
    callbacks may be `async`, and we don't run an event loop here. Use the
    async client for rotation, or pass a fresh static `jwt(token)` per call.
    """

    __slots__ = ("_auth", "_endpoint", "_http", "_owns_http", "_user_agent")

    def __init__(
        self,
        endpoint: str,
        *,
        auth: AuthConfig | str | None = None,
        http: httpx.Client | None = None,
        timeout: float = 30.0,
        user_agent: str | None = None,
    ) -> None:
        self._endpoint = endpoint.rstrip("/")
        self._auth = _normalize_auth(auth)
        if http is None:
            self._http = httpx.Client(timeout=timeout)
            self._owns_http = True
        else:
            self._http = http
            self._owns_http = False
        ua = f"sbo3l-sdk/{__version__}"
        self._user_agent = f"{ua} {user_agent}" if user_agent else ua

    @property
    def endpoint(self) -> str:
        return self._endpoint

    def submit(
        self,
        request: PaymentRequest | dict[str, Any],
        *,
        idempotency_key: str | None = None,
    ) -> PaymentRequestResponse:
        if isinstance(request, PaymentRequest):
            payload = request.model_dump(mode="json", by_alias=True)
        else:
            payload = request
        body = json.dumps(payload).encode("utf-8")

        headers: dict[str, str] = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": self._user_agent,
        }
        auth_header_value = self._sync_auth_header()
        if auth_header_value is not None:
            headers["Authorization"] = auth_header_value
        if idempotency_key is not None:
            headers["Idempotency-Key"] = idempotency_key

        try:
            res = self._http.post(
                f"{self._endpoint}/v1/payment-requests",
                content=body,
                headers=headers,
            )
        except httpx.HTTPError as e:
            raise SBO3LTransportError(str(e)) from e

        return _parse_envelope(res)

    def health(self) -> bool:
        try:
            res = self._http.get(
                f"{self._endpoint}/v1/health",
                headers={"User-Agent": self._user_agent, "Accept": "text/plain"},
            )
        except httpx.HTTPError as e:
            raise SBO3LTransportError(str(e)) from e
        return res.status_code == 200 and res.text.strip() == "ok"

    def close(self) -> None:
        if self._owns_http:
            self._http.close()

    def __enter__(self) -> SBO3LClientSync:
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()

    def _sync_auth_header(self) -> str | None:
        a = self._auth
        if isinstance(a, _AuthNone):
            return None
        if isinstance(a, (_AuthBearer, _AuthJwt)):
            return f"Bearer {a.token}"
        # _AuthJwtSupplier intentionally rejected in sync client.
        raise TypeError(
            "jwt_supplier auth is not supported in SBO3LClientSync; "
            "use SBO3LClient (async) or pass a static jwt(token)."
        )
