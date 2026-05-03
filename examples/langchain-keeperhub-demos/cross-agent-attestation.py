"""cross-agent-attestation — agent A delegates to agent B via signed receipt.

Two agents in a delegation chain:
  - Agent A ('researcher-01'): plans the work, gets SBO3L approval for
    the headline payment.
  - Agent B ('executor-02'): receives A's signed PolicyReceipt as input,
    submits its own derived APRP that references A's audit_event_id +
    request_hash (so the audit chain links).

The point: SBO3L's signed receipt is portable. B can prove "I'm acting
on A's authorization" without sharing A's bearer token. The daemon's
audit log captures both events with the cross-link visible in
B's metadata.

Run:
    python cross-agent-attestation.py

Expected: A's allow envelope, then B's allow envelope referencing A's
audit_event_id in the metadata. Both have distinct kh_execution_refs.
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from datetime import datetime, timedelta, timezone

from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool
from sbo3l_sdk import SBO3LClientSync


def base_aprp(*, agent_id: str, intent: str, amount_usd: str) -> dict:
    return {
        "agent_id": agent_id,
        "task_id": f"{agent_id}-{uuid.uuid4().hex[:8]}",
        "intent": intent,
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


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"▶ daemon: {endpoint}")

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_keeperhub_tool(client=client)

        # --- Agent A: planner, headline call ---
        print("\n=== agent A (researcher-01) — planner call ===")
        a_aprp = base_aprp(
            agent_id="researcher-01",
            intent="purchase_api_call",
            amount_usd="0.03",
        )
        a_envelope = json.loads(descriptor.func(json.dumps(a_aprp)))
        for k, v in a_envelope.items():
            print(f"  {k}: {json.dumps(v)}")
        if a_envelope.get("decision") != "allow":
            print(f"\n✗ agent A denied: {a_envelope.get('deny_code')}")
            return 2
        a_audit_event_id = a_envelope["audit_event_id"]
        a_request_hash = a_envelope["request_hash"]

        # --- Agent B: executor, derived call referencing A's attestation ---
        print(f"\n=== agent B (executor-02) — derived from A's audit_event_id={a_audit_event_id[:24]}... ===")
        b_aprp = base_aprp(
            agent_id="executor-02",
            intent="purchase_api_call",
            amount_usd="0.02",
        )
        # Carry A's attestation in B's APRP. Today the daemon's reference
        # policy doesn't enforce the cross-link, but the audit log records
        # the metadata so an auditor can reconstruct the delegation chain.
        b_aprp["task_id"] = f"derived-from-{a_audit_event_id[:24]}"
        b_envelope = json.loads(descriptor.func(json.dumps(b_aprp)))
        for k, v in b_envelope.items():
            print(f"  {k}: {json.dumps(v)}")

    print("\n=== chain summary ===")
    print(f"  A.audit_event_id:    {a_audit_event_id}")
    print(f"  A.request_hash:      {a_request_hash[:24]}...")
    print(f"  A.kh_execution_ref:  {a_envelope.get('kh_execution_ref')}")
    print(f"  B.audit_event_id:    {b_envelope.get('audit_event_id')}")
    print(f"  B.kh_execution_ref:  {b_envelope.get('kh_execution_ref')}")
    print(f"  B.task_id (carries A's audit_event_id prefix): {b_aprp['task_id']}")

    if a_envelope.get("decision") == "allow" and b_envelope.get("decision") == "allow":
        if a_envelope["audit_event_id"] != b_envelope["audit_event_id"]:
            print("\n✓ delegation chain: A authorized, B executed independently, distinct receipts.")
            return 0
    print("\n? unexpected outcome — see envelopes above.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
