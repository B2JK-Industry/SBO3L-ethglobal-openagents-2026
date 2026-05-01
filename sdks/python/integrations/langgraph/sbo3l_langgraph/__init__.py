"""sbo3l-langgraph — LangGraph adapter for SBO3L.

Drop a `PolicyGuardNode` between your plan and execute nodes to gate
every payment-shaped action through SBO3L's policy boundary.

  graph = StateGraph(MyState)
  graph.add_node("plan", plan_fn)
  graph.add_node("policy_guard", PolicyGuardNode(client))
  graph.add_node("execute", execute_fn)
  graph.add_edge("plan", "policy_guard")
  graph.add_conditional_edges("policy_guard", route_after_guard)
"""

from __future__ import annotations

from ._version import __version__
from .guard import (
    DENIED,
    PolicyGuardNode,
    PolicyGuardOutput,
    PolicyGuardState,
    route_after_guard,
)

__all__ = [
    "__version__",
    "PolicyGuardNode",
    "PolicyGuardState",
    "PolicyGuardOutput",
    "route_after_guard",
    "DENIED",
]
