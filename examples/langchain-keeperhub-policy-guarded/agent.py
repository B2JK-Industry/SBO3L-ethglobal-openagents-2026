"""Side-by-side Python demo of the SBO3L → KeeperHub policy gate.

Submits two APRPs:
  1. A small (within-budget) one — expects ALLOW + kh_execution_ref
  2. A huge (over-budget) one     — expects DENY + no execution

Both use the real SBO3L daemon over localhost (mock mode default —
without SBO3L_KEEPERHUB_WEBHOOK_URL set on the daemon, the KH adapter
falls back to local_mock and still returns a kh-<ULID> ref so the wire
path is visible end-to-end).

Run:
  SBO3L_ALLOW_UNAUTHENTICATED=1 \\
  SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \\
    cargo run --bin sbo3l-server &
  python agent.py
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from datetime import datetime, timedelta, timezone

from sbo3l_sdk import SBO3LClientSync
from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool


def aprp(*, amount_usd: str, agent_id: str = "research-agent-kh-01") -> dict:
    return {
        "agent_id": agent_id,
        "task_id": f"kh-demo-{uuid.uuid4().hex[:8]}",
        "intent": "purchase_api_call",
        "amount": {"value": amount_usd, "currency": "USD"},
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


def _print_envelope(label: str, envelope: dict) -> None:
    print(f"\n=== {label} ===")
    for k, v in envelope.items():
        print(f"  {k}: {json.dumps(v)}")


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"▶ daemon endpoint: {endpoint}")

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_keeperhub_tool(client=client)

        # Path 1 — within-budget (reference policy allows ≤0.05 USDC on base).
        small = aprp(amount_usd="0.05")
        env1 = json.loads(descriptor.func(json.dumps(small)))
        _print_envelope("ALLOW path (amount=0.05)", env1)

        # Path 2 — over-budget (same policy denies amount > 0.05).
        huge = aprp(amount_usd="10000.00")
        env2 = json.loads(descriptor.func(json.dumps(huge)))
        _print_envelope("DENY path (amount=10000.00)", env2)

    print("\n--- summary ---")
    print(f"  small.kh_execution_ref: {env1.get('kh_execution_ref')}")
    print(f"  huge.kh_execution_ref:  {env2.get('kh_execution_ref')}")
    print(f"  small.audit_event_id:   {env1.get('audit_event_id')}")
    print(f"  huge.audit_event_id:    {env2.get('audit_event_id')}")
    print(f"  huge.deny_code:         {env2.get('deny_code')}")

    if env1.get("decision") == "allow" and env1.get("kh_execution_ref"):
        if env2.get("decision") == "deny" and env2.get("kh_execution_ref") is None:
            print("\n✓ gate-then-execute proven: small executed, huge blocked.")
            return 0

    print("\n✗ unexpected: see envelopes above.")
    return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(2)
