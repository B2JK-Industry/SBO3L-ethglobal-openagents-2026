"""3-node LangGraph: plan → policy_guard → execute (or END on deny).

Demonstrates `sbo3l_langgraph.PolicyGuardNode` slotted between a planning
node (which produces an APRP) and an execution node (which would actually
fire the payment downstream).

Use `build_app(client)` to construct the compiled StateGraph; pass any
sync `SBO3LClientLike` (e.g. `SBO3LClientSync`).
"""

from __future__ import annotations

from typing import Any, TypedDict

from langgraph.graph import END, StateGraph
from sbo3l_langgraph import DENIED, PolicyGuardNode, route_after_guard


class State(TypedDict, total=False):
    """LangGraph state. `proposed_action` is what `plan` writes; `policy_guard`
    reads it and writes either `policy_receipt` (allow) or `deny_reason` (deny).
    `execute` reads `policy_receipt` and writes `result`."""

    proposed_action: dict[str, Any]
    policy_receipt: dict[str, Any]
    deny_reason: dict[str, Any]
    result: str


def _plan(_state: State) -> dict[str, Any]:
    """Stand-in `plan` node — a real agent would call an LLM here. We hard-code
    a payment intent so the graph runs deterministically in CI / smoke."""

    return {
        "proposed_action": {
            "agent_id": "research-agent-01",
            "task_id": "demo-langgraph-1",
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
    }


def _execute(state: State) -> dict[str, Any]:
    """Stand-in `execute` node — a real agent would invoke a sponsor adapter
    here (KH, Uniswap, etc.). We just record the execution_ref."""

    receipt = state["policy_receipt"]["receipt"]
    return {"result": f"executed; ref={receipt.get('execution_ref') or '(none)'}"}


def build_app(client: Any) -> Any:  # noqa: ANN401 (langgraph compiled app type)
    """Build + compile the 3-node graph. `client` is any sync SBO3L client."""

    graph: StateGraph = StateGraph(State)
    graph.add_node("plan", _plan)
    graph.add_node("policy_guard", PolicyGuardNode(client))
    graph.add_node("execute", _execute)

    graph.set_entry_point("plan")
    graph.add_edge("plan", "policy_guard")
    graph.add_conditional_edges(
        "policy_guard",
        route_after_guard,
        {"execute": "execute", DENIED: END},
    )
    graph.add_edge("execute", END)

    return graph.compile()
