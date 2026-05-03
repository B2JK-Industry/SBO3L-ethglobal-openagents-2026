"""simple-keepalive — minimum viable SBO3L → KH policy gate.

The bare floor: one APRP, one decision, one KH execution. No retry,
no chaining, no observability — just the wire path with the smallest
possible surface area.

Run:
    python simple-keepalive.py

Expected: ALLOW envelope with kh_execution_ref populated.
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from datetime import datetime, timedelta, timezone

from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool
from sbo3l_sdk import SBO3LClientSync


def aprp() -> dict:
    return {
        "agent_id": "keepalive-agent-01",
        "task_id": f"keepalive-{uuid.uuid4().hex[:8]}",
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


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"▶ daemon: {endpoint}")

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_keeperhub_tool(client=client)
        envelope = json.loads(descriptor.func(json.dumps(aprp())))

    print("\n=== envelope ===")
    for k, v in envelope.items():
        print(f"  {k}: {json.dumps(v)}")

    if envelope.get("decision") == "allow" and envelope.get("kh_execution_ref"):
        print(f"\n✓ allow + KH executed → kh_execution_ref={envelope['kh_execution_ref']}")
        return 0
    if "error" in envelope:
        print(f"\n✗ transport error: {envelope['error']}")
        return 2
    print(f"\n✗ unexpected decision: {envelope.get('decision')!r}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
