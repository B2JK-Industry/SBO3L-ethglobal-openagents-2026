"""multi-step-research — chain 3 KH workflows under one shared budget.

A research agent plans 3 sequential paid steps:
  1. fetch quote        ($0.02)
  2. enrich with metadata ($0.02)
  3. write to storage   ($0.05)

Total budget: $0.05. Steps 1+2 fit (=$0.04 cumulative). Step 3
exhausts the remainder — SBO3L should DENY before KH ever runs it,
preserving the budget guarantee even under chained-tool reasoning.

The point: one budget, three calls, observable per-step decisions.

Run:
    python multi-step-research.py

Expected: step 1 ALLOW, step 2 ALLOW, step 3 DENY (budget.exhausted).
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from decimal import Decimal

from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool
from sbo3l_sdk import SBO3LClientSync


@dataclass
class Step:
    label: str
    amount_usd: str  # decimal string
    intent: str = "purchase_api_call"


PLAN = [
    Step("fetch quote", "0.02"),
    Step("enrich with metadata", "0.02"),
    Step("write to storage", "0.05"),  # cumulative = 0.09 > 0.05 budget
]
BUDGET_USD = Decimal("0.05")


def aprp(step: Step, agent_id: str) -> dict:
    return {
        "agent_id": agent_id,
        "task_id": f"research-{uuid.uuid4().hex[:8]}-{step.label.replace(' ', '-')}",
        "intent": step.intent,
        "amount": {"value": step.amount_usd, "currency": "USD"},
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
    # research-agent-01 is the only agent_id registered in the bundled
    # reference policy; demos using a different id are denied before
    # policy evaluation (auth.agent_not_found). Override via SBO3L_POLICY
    # to a custom policy if you want a different label here.
    agent_id = "research-agent-01"
    print(f"▶ daemon: {endpoint}")
    print(f"▶ agent: {agent_id}, budget: {BUDGET_USD} USD, plan: {len(PLAN)} steps")

    spent = Decimal("0")
    results = []

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_keeperhub_tool(client=client)
        for i, step in enumerate(PLAN, start=1):
            envelope = json.loads(descriptor.func(json.dumps(aprp(step, agent_id))))
            decision = envelope.get("decision")
            print(f"\n--- step {i}: {step.label} (${step.amount_usd}) → {decision} ---")
            if decision == "allow":
                spent += Decimal(step.amount_usd)
                print(f"  kh_execution_ref: {envelope.get('kh_execution_ref')}")
                print(f"  cumulative spent: {spent} / {BUDGET_USD} USD")
            else:
                print(f"  deny_code: {envelope.get('deny_code')}")
                print(f"  audit_event_id: {envelope.get('audit_event_id')}")
            results.append((step, envelope))

    print("\n=== summary ===")
    for step, env in results:
        marker = "✓" if env.get("decision") == "allow" else "✗"
        print(f"  {marker} {step.label}: {env.get('decision')} ({env.get('deny_code') or ''})")
    print(f"  total spent (allowed only): {spent} / {BUDGET_USD} USD")

    # Happy path: 2 allows + 1 deny on the budget-exhausting step.
    decisions = [env.get("decision") for _, env in results]
    if decisions[:2] == ["allow", "allow"] and decisions[2] in ("deny", "requires_human"):
        print("\n✓ budget enforcement proven across chained steps.")
        return 0
    print(f"\n? unexpected sequence: {decisions}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
