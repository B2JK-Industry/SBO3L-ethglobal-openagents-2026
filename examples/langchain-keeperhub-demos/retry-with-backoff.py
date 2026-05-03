"""retry-with-backoff — handle SBO3L transport 5xx with exponential backoff.

The SBO3L tool wrapper returns a structured `{"error": ...}` envelope
on transport failure (network down, daemon 5xx, daemon parse error).
Agents in production wrap that with retry-on-transient logic. This
demo shows the pattern: jittered exponential backoff, max 3 attempts,
deny passed through verbatim (deny is NOT a transport failure).

The wrapper:
  - on transport.failed / transport.* → retry with backoff
  - on decision deny / requires_human → return immediately (policy
    decision, not transport — retrying would just burn nonces)
  - on decision allow → return immediately (success)
  - after max_retries → return last error envelope

Run:
    python retry-with-backoff.py

Expected: ALLOW envelope on the first try (mock daemon is healthy).
The retry loop's structure is what's interesting; the live demo path
is the no-retry case because nothing's failing.
"""

from __future__ import annotations

import json
import os
import random
import sys
import time
import uuid
from datetime import datetime, timedelta, timezone

from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool
from sbo3l_sdk import SBO3LClientSync


def aprp() -> dict:
    return {
        "agent_id": "retry-demo-agent",
        "task_id": f"retry-{uuid.uuid4().hex[:8]}",
        "intent": "purchase_api_call",
        "amount": {"value": "0.05", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/inference",
            "method": "POST",
            "expected_recipient": "0x1111111111111111111111111111111111111111",
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(uuid.uuid4()),
        "risk_class": "low",
    }


def with_retry(
    tool_func,
    payload: str,
    *,
    max_attempts: int = 3,
    base_backoff_s: float = 0.25,
) -> dict:
    """Call the SBO3L → KH tool with retry-on-transport-error semantics.

    Distinguishes:
      - transport.* error  → transient → retry with jittered backoff
      - decision allow/deny/requires_human → terminal → return immediately
    """
    last: dict = {"error": "no.attempts.made"}
    for attempt in range(1, max_attempts + 1):
        envelope = json.loads(tool_func(payload))
        last = envelope
        err = envelope.get("error")
        if err is None:
            # decision returned (allow / deny / requires_human) — terminal.
            return envelope
        if not str(err).startswith("transport"):
            # Non-transport error envelope (e.g. malformed input) —
            # don't retry, the input itself is bad.
            return envelope
        if attempt >= max_attempts:
            return envelope
        wait = base_backoff_s * (2 ** (attempt - 1)) + random.uniform(0, base_backoff_s)
        print(f"  attempt {attempt} → {err}; backing off {wait:.2f}s before retry…")
        time.sleep(wait)
    return last


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"▶ daemon: {endpoint}")
    print("▶ wrapper: max 3 attempts, base backoff 0.25s with jitter")

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_keeperhub_tool(client=client)
        envelope = with_retry(descriptor.func, json.dumps(aprp()))

    print("\n=== envelope ===")
    for k, v in envelope.items():
        print(f"  {k}: {json.dumps(v)}")

    if envelope.get("decision") == "allow":
        print(f"\n✓ allow → kh_execution_ref={envelope.get('kh_execution_ref')}")
        return 0
    if "error" in envelope:
        print(f"\n✗ all retries exhausted: {envelope['error']}")
        return 2
    print(f"\n? unexpected: {envelope.get('decision')}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
