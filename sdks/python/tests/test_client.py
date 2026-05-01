"""HTTP client tests — covers async + sync surfaces against pytest-httpx."""

from __future__ import annotations

import json

import httpx
import pytest
from pytest_httpx import HTTPXMock

from sbo3l_sdk import (
    SBO3LClient,
    SBO3LClientSync,
    SBO3LError,
    SBO3LTransportError,
    bearer,
    none,
)
from tests.fixtures import GOLDEN_APRP, GOLDEN_ENVELOPE

_DAEMON = "http://localhost:8730"


# ---------------------------------------------------------------------------
# Async — happy path
# ---------------------------------------------------------------------------


class TestAsyncClientHappy:
    async def test_strips_trailing_slash(self) -> None:
        async with SBO3LClient(_DAEMON + "/") as c:
            assert c.endpoint == _DAEMON

    async def test_string_auth_becomes_bearer(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=GOLDEN_ENVELOPE, status_code=200)
        async with SBO3LClient(_DAEMON, auth="shorthand-token") as c:
            await c.submit(GOLDEN_APRP)
        req = httpx_mock.get_request()
        assert req is not None
        assert req.headers["Authorization"] == "Bearer shorthand-token"

    async def test_submit_round_trips(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=GOLDEN_ENVELOPE, status_code=200)
        async with SBO3LClient(_DAEMON, auth=bearer("tok")) as c:
            r = await c.submit(GOLDEN_APRP)
        assert r.decision == "allow"
        req = httpx_mock.get_request()
        assert req is not None
        assert str(req.url) == f"{_DAEMON}/v1/payment-requests"
        assert req.headers["Content-Type"] == "application/json"
        assert req.headers["Authorization"] == "Bearer tok"
        assert req.headers["User-Agent"].startswith("sbo3l-sdk/0.1.0")
        body = json.loads(req.content)
        assert body["agent_id"] == GOLDEN_APRP["agent_id"]

    async def test_idempotency_key_forwarded(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=GOLDEN_ENVELOPE, status_code=200)
        async with SBO3LClient(_DAEMON) as c:
            await c.submit(GOLDEN_APRP, idempotency_key="0123456789abcdef0123")
        req = httpx_mock.get_request()
        assert req is not None
        assert req.headers["Idempotency-Key"] == "0123456789abcdef0123"

    async def test_no_auth_omits_authorization(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=GOLDEN_ENVELOPE, status_code=200)
        async with SBO3LClient(_DAEMON, auth=none()) as c:
            await c.submit(GOLDEN_APRP)
        req = httpx_mock.get_request()
        assert req is not None
        assert "Authorization" not in req.headers

    async def test_user_agent_suffix(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=GOLDEN_ENVELOPE, status_code=200)
        async with SBO3LClient(_DAEMON, user_agent="research-agent/1.0") as c:
            await c.submit(GOLDEN_APRP)
        req = httpx_mock.get_request()
        assert req is not None
        assert req.headers["User-Agent"] == "sbo3l-sdk/0.1.0 research-agent/1.0"


# ---------------------------------------------------------------------------
# Async — error envelopes
# ---------------------------------------------------------------------------


class TestAsyncClientErrors:
    async def test_rfc7807_401(self, httpx_mock: HTTPXMock) -> None:
        problem = {
            "type": "https://schemas.sbo3l.dev/errors/auth.required",
            "title": "Authentication required",
            "status": 401,
            "detail": "Authorization header missing",
            "code": "auth.required",
        }
        httpx_mock.add_response(json=problem, status_code=401)
        async with SBO3LClient(_DAEMON) as c:
            with pytest.raises(SBO3LError) as exc:
                await c.submit(GOLDEN_APRP)
        assert exc.value.code == "auth.required"
        assert exc.value.status == 401

    async def test_non_problem_error_body_synthesized(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json={"wat": "no"}, status_code=500)
        async with SBO3LClient(_DAEMON) as c:
            with pytest.raises(SBO3LError) as exc:
                await c.submit(GOLDEN_APRP)
        assert exc.value.code == "transport.unexpected_error_shape"
        assert exc.value.status == 500

    async def test_unparseable_error_body(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(text="totally-not-json", status_code=502)
        async with SBO3LClient(_DAEMON) as c:
            with pytest.raises(SBO3LError) as exc:
                await c.submit(GOLDEN_APRP)
        assert exc.value.code == "transport.unparseable_error"
        assert exc.value.status == 502

    async def test_network_failure(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_exception(httpx.ConnectError("ECONNREFUSED"))
        async with SBO3LClient(_DAEMON) as c:
            with pytest.raises(SBO3LTransportError):
                await c.submit(GOLDEN_APRP)

    async def test_health_ok(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(text="ok\n", status_code=200)
        async with SBO3LClient(_DAEMON) as c:
            assert await c.health() is True

    async def test_health_non_200(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(text="no", status_code=503)
        async with SBO3LClient(_DAEMON) as c:
            assert await c.health() is False

    async def test_health_unexpected_body(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(text="up", status_code=200)
        async with SBO3LClient(_DAEMON) as c:
            assert await c.health() is False


# ---------------------------------------------------------------------------
# Sync mirror
# ---------------------------------------------------------------------------


class TestSyncClient:
    def test_submit_round_trips(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(json=GOLDEN_ENVELOPE, status_code=200)
        with SBO3LClientSync(_DAEMON, auth=bearer("tok")) as c:
            r = c.submit(GOLDEN_APRP)
        assert r.decision == "allow"
        req = httpx_mock.get_request()
        assert req is not None
        assert req.headers["Authorization"] == "Bearer tok"

    def test_health_ok(self, httpx_mock: HTTPXMock) -> None:
        httpx_mock.add_response(text="ok\n", status_code=200)
        with SBO3LClientSync(_DAEMON) as c:
            assert c.health() is True

    def test_rfc7807_401(self, httpx_mock: HTTPXMock) -> None:
        problem = {
            "type": "https://schemas.sbo3l.dev/errors/auth.required",
            "title": "Authentication required",
            "status": 401,
            "detail": "missing",
            "code": "auth.required",
        }
        httpx_mock.add_response(json=problem, status_code=401)
        with SBO3LClientSync(_DAEMON) as c:
            with pytest.raises(SBO3LError):
                c.submit(GOLDEN_APRP)
