"""Auth helper tests."""

from __future__ import annotations

import base64
import json

import pytest

from sbo3l_sdk import (
    assert_jwt_sub_matches,
    auth_header,
    bearer,
    decode_jwt_claims,
    jwt,
    jwt_supplier,
    none,
)


def _b64url(s: str) -> str:
    return base64.urlsafe_b64encode(s.encode()).rstrip(b"=").decode()


def fake_jwt(claims: dict[str, object]) -> str:
    header = _b64url(json.dumps({"alg": "EdDSA", "typ": "JWT"}))
    payload = _b64url(json.dumps(claims))
    return f"{header}.{payload}.AAAA"


class TestAuthHeader:
    async def test_none_returns_none(self) -> None:
        assert await auth_header(none()) is None

    async def test_bearer_format(self) -> None:
        assert await auth_header(bearer("abc")) == "Bearer abc"

    async def test_jwt_format(self) -> None:
        assert await auth_header(jwt("eyJ.x.y")) == "Bearer eyJ.x.y"

    async def test_supplier_invoked_each_call(self) -> None:
        calls = 0

        def supplier() -> str:
            nonlocal calls
            calls += 1
            return "tok"

        cfg = jwt_supplier(supplier)
        await auth_header(cfg)
        await auth_header(cfg)
        assert calls == 2

    async def test_supplier_async(self) -> None:
        async def supplier() -> str:
            return "async-tok"

        cfg = jwt_supplier(supplier)
        assert await auth_header(cfg) == "Bearer async-tok"

    async def test_supplier_must_return_str(self) -> None:
        def supplier() -> str:  # type: ignore[return]
            return None  # type: ignore[return-value]

        cfg = jwt_supplier(supplier)
        with pytest.raises(TypeError):
            await auth_header(cfg)


class TestDecodeJwtClaims:
    def test_decodes_valid(self) -> None:
        t = fake_jwt({"sub": "research-agent-01", "iat": 1700000000})
        claims = decode_jwt_claims(t)
        assert claims["sub"] == "research-agent-01"
        assert claims["iat"] == 1700000000

    def test_rejects_two_segment(self) -> None:
        with pytest.raises(ValueError, match="three dot-separated"):
            decode_jwt_claims("only.two")

    def test_rejects_empty_payload(self) -> None:
        with pytest.raises(ValueError, match="empty payload"):
            decode_jwt_claims("a..c")

    def test_rejects_non_object_payload(self) -> None:
        scalar = f"{_b64url('h')}.{_b64url(json.dumps('scalar'))}.sig"
        with pytest.raises(ValueError, match="not a JSON object"):
            decode_jwt_claims(scalar)


class TestAssertJwtSubMatches:
    def test_passes_on_match(self) -> None:
        t = fake_jwt({"sub": "research-agent-01"})
        assert_jwt_sub_matches(t, "research-agent-01")  # no raise

    def test_raises_on_mismatch(self) -> None:
        t = fake_jwt({"sub": "other-agent"})
        with pytest.raises(ValueError, match="does not match expected"):
            assert_jwt_sub_matches(t, "research-agent-01")

    def test_raises_on_missing_sub(self) -> None:
        t = fake_jwt({"iat": 1})
        with pytest.raises(ValueError, match="missing or non-string"):
            assert_jwt_sub_matches(t, "research-agent-01")
