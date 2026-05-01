"""SBO3L HTTP client. Async-first via httpx; sync wrappers in `sync.py`.

Wraps `POST /v1/payment-requests` and `GET /v1/health`.
"""

from __future__ import annotations

import json
from typing import Any

import httpx

from ._version import __version__
from .auth import AuthConfig, auth_header, bearer, none
from .errors import SBO3LError, SBO3LTransportError
from .passport import VerifyResult, verify, verify_or_raise
from .types import PaymentRequest, PaymentRequestResponse, ProblemDetail


class SBO3LClient:
    """Async HTTP client for an SBO3L daemon.

    Example::

        from sbo3l_sdk import SBO3LClient, bearer

        async with SBO3LClient(
            "http://localhost:8730", auth=bearer("my-token")
        ) as client:
            response = await client.submit(aprp)
    """

    __slots__ = ("_auth", "_endpoint", "_http", "_owns_http", "_user_agent")

    def __init__(
        self,
        endpoint: str,
        *,
        auth: AuthConfig | str | None = None,
        http: httpx.AsyncClient | None = None,
        timeout: float = 30.0,
        user_agent: str | None = None,
    ) -> None:
        """Construct an async client.

        Args:
            endpoint: Daemon base URL, e.g. ``http://localhost:8730``.
            auth: Auth config — pass an `AuthConfig` from this module, a raw
                bearer-token string, or `None` for no auth.
            http: Inject an existing `httpx.AsyncClient` (e.g. for connection
                pooling or test mocking). When omitted, the client owns and
                closes its inner `httpx.AsyncClient` on `aclose()`.
            timeout: Per-request timeout in seconds.
            user_agent: Optional suffix appended to the default
                `sbo3l-sdk/<version>` UA string.
        """

        self._endpoint = endpoint.rstrip("/")
        self._auth = _normalize_auth(auth)
        if http is None:
            self._http = httpx.AsyncClient(timeout=timeout)
            self._owns_http = True
        else:
            self._http = http
            self._owns_http = False
        ua = f"sbo3l-sdk/{__version__}"
        self._user_agent = f"{ua} {user_agent}" if user_agent else ua

    @property
    def endpoint(self) -> str:
        return self._endpoint

    async def submit(
        self,
        request: PaymentRequest | dict[str, Any],
        *,
        idempotency_key: str | None = None,
    ) -> PaymentRequestResponse:
        """Submit an APRP to ``POST /v1/payment-requests``.

        Accepts either a typed `PaymentRequest` (validated client-side) or a
        raw dict (validated server-side only).

        Raises:
            SBO3LError: daemon returned a non-2xx; the carried RFC 7807
                problem-detail is available on `.problem`/`.code`/`.status`.
            SBO3LTransportError: network or transport failure.
        """

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
        auth = await auth_header(self._auth)
        if auth is not None:
            headers["Authorization"] = auth
        if idempotency_key is not None:
            headers["Idempotency-Key"] = idempotency_key

        try:
            res = await self._http.post(
                f"{self._endpoint}/v1/payment-requests",
                content=body,
                headers=headers,
            )
        except httpx.HTTPError as e:
            raise SBO3LTransportError(str(e)) from e

        return _parse_envelope(res)

    async def health(self) -> bool:
        """Hit ``GET /v1/health`` — returns True iff daemon answers `ok`."""

        try:
            res = await self._http.get(
                f"{self._endpoint}/v1/health",
                headers={"User-Agent": self._user_agent, "Accept": "text/plain"},
            )
        except httpx.HTTPError as e:
            raise SBO3LTransportError(str(e)) from e
        return res.status_code == 200 and res.text.strip() == "ok"

    @staticmethod
    def verify(capsule: Any) -> VerifyResult:
        """Re-export of `passport.verify` for ergonomic ``client.verify(capsule)`` use."""

        return verify(capsule)

    @staticmethod
    def verify_or_raise(capsule: Any) -> dict[str, Any]:
        """Re-export of `passport.verify_or_raise`."""

        return verify_or_raise(capsule)

    async def aclose(self) -> None:
        """Close the underlying httpx client (only if this object owns it)."""

        if self._owns_http:
            await self._http.aclose()

    async def __aenter__(self) -> SBO3LClient:
        return self

    async def __aexit__(self, *_exc: object) -> None:
        await self.aclose()


def _normalize_auth(auth: AuthConfig | str | None) -> AuthConfig:
    if auth is None:
        return none()
    if isinstance(auth, str):
        return bearer(auth)
    return auth


def _parse_envelope(res: httpx.Response) -> PaymentRequestResponse:
    if res.status_code == 200:
        try:
            parsed = res.json()
        except json.JSONDecodeError as e:
            raise SBO3LTransportError(f"daemon returned 200 but body is not JSON: {e}") from e
        return PaymentRequestResponse.model_validate(parsed)

    # Non-200: expect an RFC 7807 body. Validate; if anything is off, surface
    # a synthetic Problem so callers always see `SBO3LError` (not random
    # parse exceptions).
    try:
        raw = res.json()
    except json.JSONDecodeError:
        raise SBO3LError(
            ProblemDetail(
                type="https://schemas.sbo3l.dev/errors/transport.unparseable_error",
                title="Daemon returned non-JSON error body",
                status=res.status_code,
                detail=res.text[:512],
                code="transport.unparseable_error",
            )
        ) from None
    try:
        problem = ProblemDetail.model_validate(raw)
    except Exception:
        raise SBO3LError(
            ProblemDetail(
                type="https://schemas.sbo3l.dev/errors/transport.unexpected_error_shape",
                title="Daemon returned non-Problem JSON error body",
                status=res.status_code,
                detail=json.dumps(raw)[:512],
                code="transport.unexpected_error_shape",
            )
        ) from None
    raise SBO3LError(problem)
