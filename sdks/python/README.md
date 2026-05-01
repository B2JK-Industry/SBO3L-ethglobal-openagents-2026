# `sbo3l-sdk`

Official Python SDK for [SBO3L](https://sbo3l.dev) — the cryptographically
verifiable trust layer for autonomous AI agents.

> Don't give your agent a wallet. Give it a mandate.

> ⚠ **Status — DRAFT (F-10):** package metadata, public API, and v2 capsule
> support are scaffolded against the v1 schemas. Final shape is gated on
> F-1 (auth middleware ✅ merged) and F-6 (capsule v2 schema, pending). Do
> not publish to PyPI until F-6 lands. Tracked in
> [`docs/win-backlog/05-phase-1.md`](../../docs/win-backlog/05-phase-1.md).

## What it does

- **`POST /v1/payment-requests`** wrapped as a typed `submit()` method
  (async-first via `httpx.AsyncClient`, sync mirror via `httpx.Client`).
- **Pydantic v2 strict** wire types for APRP, PolicyReceipt, and the
  Passport capsule (v1 + v2 hooks). Strict-frozen models reject unknown
  fields end-to-end.
- **Bearer + JWT auth** helpers that match the F-1 daemon contract.
- **Client-side structural verifier** for Passport capsules. The
  cryptographic checks live in the Rust CLI `sbo3l-cli passport verify
  --strict` — this SDK does the structural and cross-field checks so
  callers can fail fast in Python before round-tripping.

## Install

```bash
pip install sbo3l-sdk
```

Requires Python ≥ 3.10.

## Quick start (async)

```python
import asyncio
from sbo3l_sdk import SBO3LClient, bearer

async def main() -> None:
    async with SBO3LClient(
        "http://localhost:8730",
        auth=bearer("my-bearer-token"),
    ) as client:
        response = await client.submit({
            "agent_id": "research-agent-01",
            "task_id": "demo-task-1",
            "intent": "purchase_api_call",
            "amount": {"value": "0.05", "currency": "USD"},
            "token": "USDC",
            "destination": {
                "type": "x402_endpoint",
                "url": "https://api.example.com/v1/inference",
                "method": "POST",
            },
            "payment_protocol": "x402",
            "chain": "base",
            "provider_url": "https://api.example.com",
            "expiry": "2026-05-01T10:31:00Z",
            "nonce": "01HTAWX5K3R8YV9NQB7C6P2DGM",
            "risk_class": "low",
        })
        if response.decision == "allow":
            print("execution_ref:", response.receipt.execution_ref)

asyncio.run(main())
```

## Sync variant

```python
from sbo3l_sdk import SBO3LClientSync, bearer

with SBO3LClientSync("http://localhost:8730", auth=bearer("tok")) as client:
    response = client.submit(aprp_dict)
```

The sync client uses `httpx.Client` directly (no `asyncio.run` shim), so it's
safe to use even from within a running event loop.

## JWT auth (per-agent)

```python
from sbo3l_sdk import SBO3LClient, jwt, assert_jwt_sub_matches

token = await my_signer.sign({"sub": "research-agent-01", "iat": now()})
assert_jwt_sub_matches(token, "research-agent-01")  # local sanity check

async with SBO3LClient("http://localhost:8730", auth=jwt(token)) as client:
    ...
```

The SDK never holds a private key. The `sub == agent_id` match is enforced
server-side (F-1); the client-side `assert_jwt_sub_matches` is a fail-fast
convenience.

## Idempotency-safe retry

```python
import secrets
key = secrets.token_hex(20)  # 40 ASCII chars
await client.submit(aprp, idempotency_key=key)  # first call
await client.submit(aprp, idempotency_key=key)  # cached envelope, no side effects
```

Same key + different body → HTTP 409 `protocol.idempotency_conflict`.

## Verifying a capsule client-side

```python
import json
from sbo3l_sdk import verify

capsule = json.loads(open("capsule.json").read())
result = verify(capsule)
if not result.ok:
    for f in result.failures:
        print(f"[{f.code}] {f.description}: {f.detail or ''}")
```

For full crypto verification, use the Rust CLI:

```bash
sbo3l-cli passport verify --strict --path capsule.json
```

## Errors

| Class | When |
|---|---|
| `SBO3LError` | Daemon returned a non-2xx. Carries the RFC 7807 problem-detail; `.code` and `.status` are first-class. |
| `SBO3LTransportError` | Network/transport failure (timeout, DNS, refused). |
| `PassportVerificationError` | Raised by `verify_or_raise()`. Carries `.codes` (tuple of failure codes). |

```python
from sbo3l_sdk import SBO3LError

try:
    await client.submit(aprp)
except SBO3LError as e:
    if e.code == "auth.required":
        # re-acquire token
        ...
    else:
        raise
```

## Compatibility

- **Python:** ≥ 3.10.
- **Pydantic:** v2.6+ (strict mode).
- **httpx:** 0.27+.
- **Daemon:** SBO3L server `0.1.0+`.

## Development

```bash
python3 -m venv .venv
.venv/bin/pip install -e ".[dev]"
.venv/bin/pytest
.venv/bin/ruff check .
.venv/bin/mypy --strict sbo3l_sdk
```

## License

MIT — see `LICENSE` at the repo root.
