# `sbo3l-langgraph`

LangGraph adapter for SBO3L. Drop a `PolicyGuardNode` between your `plan` and `execute` nodes to gate every payment-shaped action through SBO3L's policy boundary.

## Install

```bash
pip install sbo3l-langgraph sbo3l-sdk langgraph
```

## 3-node graph

```
   plan  ──►  policy_guard  ──►  execute
                  │
                  └─►  END  (when SBO3L denies)
```

```python
from langgraph.graph import StateGraph, END
from sbo3l_sdk import SBO3LClientSync
from sbo3l_langgraph import PolicyGuardNode, route_after_guard, DENIED

class State(TypedDict, total=False):
    proposed_action: dict       # APRP body (written by `plan`)
    policy_receipt: dict        # set on allow (read by `execute`)
    deny_reason: dict           # set on deny (read by error handler)
    result: str                 # downstream `execute` writes this

client = SBO3LClientSync("http://localhost:8730")

def plan(state):
    return {"proposed_action": {
        "agent_id": "research-agent-01",
        "task_id": "demo-1",
        "intent": "purchase_api_call",
        # ...full APRP body...
    }}

def execute(state):
    receipt = state["policy_receipt"]
    return {"result": f"executed; ref={receipt['receipt']['execution_ref']}"}

graph = StateGraph(State)
graph.add_node("plan", plan)
graph.add_node("policy_guard", PolicyGuardNode(client))
graph.add_node("execute", execute)
graph.set_entry_point("plan")
graph.add_edge("plan", "policy_guard")
graph.add_conditional_edges(
    "policy_guard",
    route_after_guard,
    {"execute": "execute", DENIED: END},
)
graph.add_edge("execute", END)

app = graph.compile()
final = app.invoke({})
```

## Behavior

**Allow:** writes `state["policy_receipt"]` with the full `PaymentRequestResponse` shape (decision, deny_code, matched_rule_id, request_hash, policy_hash, audit_event_id, receipt). The conditional edge routes to `execute`.

**Deny / requires_human:** writes `state["deny_reason"]` with `code` (deny_code or `policy.deny`/`policy.requires_human` fallback), `decision`, `matched_rule_id`, `audit_event_id`. Conditional edge routes to `END` (or wherever you wire `DENIED`).

**Transport / auth failures:** also writes `state["deny_reason"]` with `code` set to the RFC 7807 domain code (e.g. `auth.required`) or `transport.failed` for network errors. `decision: "error"`.

## Idempotency

```python
PolicyGuardNode(
    client,
    idempotency_key=lambda action: f"{action['task_id']}-{action['nonce']}",
)
```

## Sync vs async

`PolicyGuardNode` is sync — pair it with `SBO3LClientSync` (httpx.Client under the hood, safe inside any event loop).

If your graph is async (`graph.astream()`), use the async client (`SBO3LClient`) — but pass it via a sync wrapper or use the future `AsyncPolicyGuardNode` (TODO follow-up). For now passing async client to sync `PolicyGuardNode` writes a `transport.async_client_in_sync_graph` deny_reason — clear failure mode, no silent blocking.

## License

MIT
