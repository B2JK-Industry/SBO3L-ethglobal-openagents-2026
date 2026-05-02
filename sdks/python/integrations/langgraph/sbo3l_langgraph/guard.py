"""LangGraph PolicyGuardNode — gates an agent's proposed action through SBO3L.

Pattern: a 3-node StateGraph

    plan  →  policy_guard  →  execute

`plan` populates `state["proposed_action"]` with an APRP body. `policy_guard`
(this module) calls `SBO3LClient.submit()`, then writes either:

  - state["policy_receipt"] = {... allow envelope ...}  → routes to `execute`
  - state["deny_reason"] = {"code": "...", "decision": "..."}  → routes to END

Use `route_after_guard(state)` as the conditional-edge function from
`policy_guard`. It returns the literal string `"execute"` when allowed and
`"DENIED"` when denied; wire those into your graph as you prefer.

Real `sbo3l_sdk` integration — no SDK mocks. The constructor takes any
client matching `SBO3LClientLike` (sync `submit` or awaitable). Tests use
SBO3LClientSync with httpx_mock to keep the daemon out of CI.
"""

from __future__ import annotations

from collections.abc import Awaitable
from dataclasses import dataclass
from typing import Any, Protocol, TypedDict

from sbo3l_sdk import PaymentRequestResponse, SBO3LError


class SBO3LClientLike(Protocol):
    """Minimum surface this guard needs from an SBO3L client."""

    def submit(
        self,
        request: dict[str, Any],
        *,
        idempotency_key: str | None = ...,
    ) -> PaymentRequestResponse | Awaitable[PaymentRequestResponse]: ...


class PolicyGuardState(TypedDict, total=False):
    """State fields read/written by `PolicyGuardNode`.

    Your graph's State TypedDict (or Pydantic model) should include these
    keys (they may live alongside your own keys; the guard reads only
    `proposed_action` and writes only `policy_receipt` / `deny_reason`).
    """

    #: The APRP body produced by the upstream `plan` node. The guard reads it.
    proposed_action: dict[str, Any]

    #: Set on `decision == "allow"`. Carries the full `PaymentRequestResponse`
    #: shape (decision, deny_code, matched_rule_id, request_hash, policy_hash,
    #: audit_event_id, receipt). Downstream `execute` node reads this.
    policy_receipt: dict[str, Any]

    #: Set on `decision in {"deny", "requires_human"}`. Carries `code`
    #: (deny_code or `auth.required` etc.), `decision`, `audit_event_id`,
    #: and `matched_rule_id`. The graph's conditional edge routes to END
    #: based on this.
    deny_reason: dict[str, Any]


@dataclass(frozen=True, slots=True)
class PolicyGuardOutput:
    """Structured return value of `PolicyGuardNode.__call__`.

    LangGraph's StateGraph also accepts a plain dict; this dataclass exists
    for callers building their own dispatch loops or wanting typed access.
    """

    decision: str  # "allow" | "deny" | "requires_human" | "error"
    receipt: dict[str, Any] | None
    deny_reason: dict[str, Any] | None


#: Sentinel route name returned by `route_after_guard` on deny. Wire this
#: into your `add_conditional_edges` mapping (e.g. `{"DENIED": END}`).
DENIED: str = "DENIED"


class PolicyGuardNode:
    """Callable LangGraph node that runs SBO3L policy decision on `state["proposed_action"]`.

    Construct with an SBO3L client (sync `SBO3LClientSync` recommended for
    most LangGraph use; async `SBO3LClient` works only inside async graphs).
    The instance is a plain callable — pass it directly to
    `graph.add_node("policy_guard", PolicyGuardNode(client))`.

    Optional `idempotency_key` callback derives a per-call key from the
    proposed action (useful when the same task may be re-entered from a
    retry loop).
    """

    __slots__ = ("_client", "_idempotency_key")

    def __init__(
        self,
        client: SBO3LClientLike,
        *,
        idempotency_key: object | None = None,
    ) -> None:
        self._client = client
        self._idempotency_key = idempotency_key

    def __call__(self, state: dict[str, Any]) -> dict[str, Any]:
        """Run the policy decision on `state["proposed_action"]`.

        Returns a dict with EITHER `policy_receipt` (allow) OR `deny_reason`
        (deny / requires_human / transport-error). LangGraph merges this
        return value into the next state.
        """

        action = state.get("proposed_action")
        if not isinstance(action, dict):
            return {
                "deny_reason": {
                    "code": "input.no_proposed_action",
                    "decision": "error",
                    "detail": "state['proposed_action'] missing or not a dict",
                }
            }

        kwargs: dict[str, Any] = {}
        if self._idempotency_key is not None and callable(self._idempotency_key):
            kwargs["idempotency_key"] = self._idempotency_key(action)

        try:
            result = self._client.submit(action, **kwargs)
            if hasattr(result, "__await__"):
                # Async client used in a sync-graph context — surface a clear
                # error. Use SBO3LClientSync for sync graphs.
                # Close the coroutine before returning so Python's GC
                # doesn't emit `RuntimeWarning: coroutine ... was never
                # awaited` (which fails any test runner using
                # -W error::RuntimeWarning).
                close = getattr(result, "close", None)
                if callable(close):
                    try:
                        close()
                    except Exception:  # noqa: BLE001 — closing a half-started
                        # coroutine can raise; we have no recovery and the
                        # caller already gets a structured deny envelope.
                        pass
                return {
                    "deny_reason": {
                        "code": "transport.async_client_in_sync_graph",
                        "decision": "error",
                        "detail": (
                            "client.submit returned an awaitable; pass a sync "
                            "client (SBO3LClientSync) to PolicyGuardNode, or "
                            "use the AsyncPolicyGuardNode (TODO)."
                        ),
                    }
                }
            response: PaymentRequestResponse = result
        except SBO3LError as e:
            return {
                "deny_reason": {
                    "code": e.code,
                    "decision": "error",
                    "status": e.status,
                    "detail": str(e),
                }
            }
        except Exception as e:
            return {
                "deny_reason": {
                    "code": "transport.failed",
                    "decision": "error",
                    "detail": str(e),
                }
            }

        if response.decision == "allow":
            return {
                "policy_receipt": response.model_dump(mode="json", by_alias=True),
            }

        # deny / requires_human
        return {
            "deny_reason": {
                "code": response.deny_code or f"policy.{response.decision}",
                "decision": response.decision,
                "matched_rule_id": response.matched_rule_id,
                "audit_event_id": response.audit_event_id,
            }
        }


def route_after_guard(state: dict[str, Any]) -> str:
    """Conditional-edge function for the `policy_guard` node.

    Use as:

        graph.add_conditional_edges(
            "policy_guard",
            route_after_guard,
            {"execute": "execute", DENIED: END},
        )

    Returns ``"execute"`` only when the LATEST guard run produced an allow —
    i.e. ``policy_receipt`` is present AND no fresh ``deny_reason`` is
    set in the same state.

    Why we check ``deny_reason`` FIRST:
    LangGraph merges partial state updates from each node. A retried or
    re-entered guard call can append a fresh deny on top of an older
    cached allow receipt, leaving both fields populated. Routing on
    ``policy_receipt`` alone would then execute downstream work after a
    deny — a safety violation. Checking ``deny_reason`` first short-
    circuits that race; the receipt is only honoured when no deny
    coexists.
    """

    if isinstance(state.get("deny_reason"), dict):
        return DENIED
    if isinstance(state.get("policy_receipt"), dict):
        return "execute"
    return DENIED
