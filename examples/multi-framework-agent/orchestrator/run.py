"""Multi-framework orchestrator — walks Plan → Execute → Confirm and
prints the audit chain that spans all three framework boundaries.

  user goal
     ▼
  ┌──────────────┐
  │ plan service │  LangChain → SBO3L  →  audit_event_id evt-A, execution_ref kh-A
  └──────┬───────┘
         ▼ next_action APRP
  ┌──────────────┐
  │ exec service │  CrewAI    → SBO3L  →  audit_event_id evt-B, execution_ref kh-B
  └──────┬───────┘
         ▼ next_action APRP
  ┌──────────────┐
  │ confirm svc  │  AutoGen   → SBO3L  →  audit_event_id evt-C, execution_ref kh-C
  └──────────────┘

All three audit events live on the SAME hash-chained audit log inside the
shared sbo3l-server container. Use `sbo3l-cli audit list` (or the MCP
`sbo3l.audit_lookup` tool) inside the daemon container to walk the chain.
"""

from __future__ import annotations

import json
import os
import sys

import httpx

PLAN_URL = os.environ.get("PLAN_URL", "http://plan:8001")
EXECUTE_URL = os.environ.get("EXECUTE_URL", "http://execute:8002")
CONFIRM_URL = os.environ.get("CONFIRM_URL", "http://confirm:8003")
SBO3L_ENDPOINT = os.environ.get("SBO3L_ENDPOINT", "http://sbo3l-server:8730")


def _post(url: str, body: dict[str, object]) -> dict[str, object]:
    with httpx.Client(timeout=30.0) as client:
        r = client.post(url, json=body)
    r.raise_for_status()
    return r.json()  # type: ignore[no-any-return]


def main() -> int:
    user_goal = "Execute one paid API call and confirm the result."
    print("═" * 60)
    print("SBO3L cross-framework demo — single audit chain across 3 LLM frameworks")
    print(f"daemon: {SBO3L_ENDPOINT}")
    print(f"goal:   {user_goal}")
    print("═" * 60)

    audit_chain: list[dict[str, object]] = []

    # ───── Step 1: PLAN (LangChain → SBO3L) ─────
    print("\n▶ step 1: plan (LangChain framework)")
    plan_resp = _post(f"{PLAN_URL}/plan", {"goal": user_goal})
    print(f"  decision:        {plan_resp.get('decision')}")
    print(f"  audit_event_id:  {plan_resp.get('audit_event_id')}")
    print(f"  execution_ref:   {plan_resp.get('execution_ref')}")
    if plan_resp.get("decision") != "allow":
        print(f"  ✗ plan denied — {plan_resp.get('deny_code')}")
        return 2
    audit_chain.append(
        {
            "step": "plan",
            "framework": "langchain",
            "audit_event_id": plan_resp["audit_event_id"],
            "execution_ref": plan_resp.get("execution_ref"),
        }
    )

    # ───── Step 2: EXECUTE (CrewAI → SBO3L) ─────
    print("\n▶ step 2: execute (CrewAI framework)")
    execute_resp = _post(f"{EXECUTE_URL}/execute", {"aprp": plan_resp["next_action"]})
    print(f"  decision:        {execute_resp.get('decision')}")
    print(f"  audit_event_id:  {execute_resp.get('audit_event_id')}")
    print(f"  execution_ref:   {execute_resp.get('execution_ref')}")
    if execute_resp.get("decision") != "allow":
        print(f"  ✗ execute denied — {execute_resp.get('deny_code')}")
        return 2
    audit_chain.append(
        {
            "step": "execute",
            "framework": "crewai",
            "audit_event_id": execute_resp["audit_event_id"],
            "execution_ref": execute_resp.get("execution_ref"),
        }
    )

    # ───── Step 3: CONFIRM (AutoGen → SBO3L) ─────
    print("\n▶ step 3: confirm (AutoGen framework)")
    confirm_resp = _post(f"{CONFIRM_URL}/confirm", {"aprp": execute_resp["next_action"]})
    print(f"  decision:        {confirm_resp.get('decision')}")
    print(f"  audit_event_id:  {confirm_resp.get('audit_event_id')}")
    print(f"  execution_ref:   {confirm_resp.get('execution_ref')}")
    if confirm_resp.get("decision") != "allow":
        print(f"  ✗ confirm denied — {confirm_resp.get('deny_code')}")
        return 2
    audit_chain.append(
        {
            "step": "confirm",
            "framework": "autogen",
            "audit_event_id": confirm_resp["audit_event_id"],
            "execution_ref": confirm_resp.get("execution_ref"),
        }
    )

    # ───── Unified audit chain ─────
    print("\n" + "═" * 60)
    print("✓ all 3 framework boundaries cleared SBO3L policy")
    print("\nUnified audit chain (single hash-chained log inside sbo3l-server):")
    print(json.dumps(audit_chain, indent=2))
    print("\nTo walk the chain, exec into the daemon container:")
    print(
        "  docker compose exec sbo3l-server sbo3l-cli audit list "
        f"--from {audit_chain[0]['audit_event_id']} "
        f"--to {audit_chain[-1]['audit_event_id']}"
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
