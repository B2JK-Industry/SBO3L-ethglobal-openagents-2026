"""Smoke runner — proves the demo's tool path end-to-end against a running
SBO3L daemon WITHOUT crewai or an OpenAI key. Hardcodes the tool calls a
reasonable agent would make so CI / Heidi can verify the integration
without burning LLM tokens or pulling crewai's heavy dep tree.

Usage:
    SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
    .venv/bin/pip install -e .
    .venv/bin/python -m sbo3l_crewai_demo.smoke

AC verification:
    - install verified (`pip install -e .` resolves the workspace deps)
    - runs end-to-end in <30s (no LLM round-trips, no crewai bootstrap)
    - signed PolicyReceipt visible in console
    - audit_event_id printed
"""

from __future__ import annotations

import json
import sys

from .tools import KH_WORKFLOW_ID, build_sbo3l_pay_func, default_client, fetch_url

APRP = {
    "agent_id": "research-agent-01",
    "task_id": "demo-crewai-smoke-1",
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
}


def main() -> int:
    print(f"▶ smoke: KH workflow target = {KH_WORKFLOW_ID}\n")

    print("▶ tool: data_fetch (GitHub status — public, low-noise)")
    fetch_out = json.loads(fetch_url("https://www.githubstatus.com/api/v2/status.json"))
    if "error" in fetch_out:
        print(f"  fetch warning: {fetch_out['error']} (continuing — SBO3L gate is the test)")
    else:
        print(f"  ✓ HTTP {fetch_out['status']}")

    print("\n▶ tool: sbo3l_payment_request (APRP → SBO3L → KH adapter)")
    with default_client() as client:
        sbo3l_pay = build_sbo3l_pay_func(client)
        decision_raw = sbo3l_pay(json.dumps(APRP))
    decision = json.loads(decision_raw)

    print("  envelope:")
    for k, v in decision.items():
        print(f"    {k}: {json.dumps(v)}")

    if decision.get("decision") == "allow":
        print(f"\n✓ allow — execution_ref {decision.get('execution_ref') or '(none)'}")
        print(f"  audit_event_id: {decision.get('audit_event_id') or '(unknown)'}")
        return 0

    if "error" in decision:
        print(f"\n✗ transport error — {decision['error']}")
        return 2

    print(
        f"\n✗ {decision.get('decision', '?')} — deny_code {decision.get('deny_code', '?')}"
    )
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
