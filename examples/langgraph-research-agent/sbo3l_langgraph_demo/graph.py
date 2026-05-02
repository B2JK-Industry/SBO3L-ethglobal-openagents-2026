"""3-node LangGraph: plan (with 2 tools) → policy_guard → execute (or END on deny).

  ┌──────┐    ┌──────────────┐    ┌──────────┐
  │ plan │ ─▶ │ policy_guard │ ─▶ │ execute  │
  └──────┘    └──────────────┘    └──────────┘
                    │
                    └──▶  END  (when SBO3L denies)

`plan` uses two tools (data_fetch + aprp_build) to research a provider
then construct an APRP. `policy_guard` is `sbo3l_langgraph.PolicyGuardNode`.
`execute` reads the signed receipt and reports the execution_ref.
"""

from __future__ import annotations

import json
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, TypedDict

import httpx
from langgraph.graph import END, StateGraph
from sbo3l_langgraph import DENIED, PolicyGuardNode, route_after_guard

KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"


class State(TypedDict, total=False):
    """LangGraph state. `plan` writes `proposed_action` + `provider_status`;
    `policy_guard` writes `policy_receipt` (allow) or `deny_reason` (deny);
    `execute` writes `result`."""

    user_request: str
    provider_status: dict[str, Any]
    proposed_action: dict[str, Any]
    policy_receipt: dict[str, Any]
    deny_reason: dict[str, Any]
    result: str


def _data_fetch(url: str) -> dict[str, Any]:
    try:
        with httpx.Client(timeout=10.0) as client:
            r = client.get(url, headers={"Accept": "application/json"})
        return {"status": r.status_code, "body": r.text[:2000]}
    except httpx.HTTPError as e:
        return {"error": str(e)}


def _aprp_build(provider_url: str, value: str) -> dict[str, Any]:
    """Construct an APRP body from provider URL + amount.

    `nonce` and `expiry` are fresh per call — the daemon's
    protocol.nonce_replay guard rejects duplicate (nonce, agent_id) tuples,
    so a static nonce only succeeds the first time. agent_id / task_id /
    chain stay deterministic so CI assertions can pin them.
    """

    return {
        "agent_id": "research-agent-01",
        "task_id": "demo-langgraph-research-1",
        "intent": "purchase_api_call",
        "amount": {"value": value, "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": provider_url,
            "method": "POST",
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": provider_url.rsplit("/", 1)[0] if "/" in provider_url else provider_url,
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(uuid.uuid4()),
        "risk_class": "low",
    }


def _plan_node(state: State) -> dict[str, Any]:
    """Plan node — uses 2 tools (data_fetch + aprp_build). Real LLM-driven
    plan would parse `state["user_request"]` to extract URL + amount; for
    deterministic CI we hardcode the inputs."""

    provider_url = "https://api.example.com/v1/inference"
    value = "0.05"

    status = _data_fetch(provider_url.rsplit("/", 1)[0])
    aprp = _aprp_build(provider_url, value)
    return {"provider_status": status, "proposed_action": aprp}


def _execute_node(state: State) -> dict[str, Any]:
    """Execute node — reads the signed receipt and reports the execution_ref.
    Real agents would call a sponsor adapter (KH, Uniswap) here."""

    receipt = state["policy_receipt"]["receipt"]
    return {
        "result": (
            f"executed via KH workflow {KH_WORKFLOW_ID}; "
            f"ref={receipt.get('execution_ref') or '(none)'}"
        )
    }


def build_app(client: Any) -> Any:  # noqa: ANN401
    """Build + compile the 3-node graph. `client` is any sync SBO3L client
    (e.g. `sbo3l_sdk.SBO3LClientSync`)."""

    graph: StateGraph = StateGraph(State)
    graph.add_node("plan", _plan_node)
    graph.add_node("policy_guard", PolicyGuardNode(client))
    graph.add_node("execute", _execute_node)

    graph.set_entry_point("plan")
    graph.add_edge("plan", "policy_guard")
    graph.add_conditional_edges(
        "policy_guard",
        route_after_guard,
        {"execute": "execute", DENIED: END},
    )
    graph.add_edge("execute", END)

    return graph.compile()
