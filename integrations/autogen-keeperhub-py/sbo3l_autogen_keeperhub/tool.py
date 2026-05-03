"""Plain descriptor + factory for the SBO3L → KeeperHub tool.

`sbo3l_autogen_keeperhub_tool(client=...)` returns an
`SBO3LKeeperHubToolDescriptor` that any tool framework taking
`(name, description, callable)` accepts. For an AutoGen-specific
helper that registers the descriptor with a `ConversableAgent`'s
function registry, see `autogen_tool.register_sbo3l_keeperhub_tool`.
"""

from __future__ import annotations

import json
import os
import uuid
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any, Protocol, TypedDict

#: Live KeeperHub workflow id verified end-to-end on 2026-04-30.
#: Override per-call via `workflow_id` arg, or globally via the
#: `SBO3L_KEEPERHUB_WORKFLOW_ID` env var on the agent process.
#: Note: this value is **advisory** — the daemon's env-configured
#: webhook URL is the source of truth for actual routing.
DEFAULT_KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"


class SBO3LKeeperHubSubmitResult(TypedDict):
    """Subset of the SBO3L response envelope this Tool returns to the LLM."""

    decision: str
    deny_code: str | None
    matched_rule_id: str | None
    request_hash: str
    policy_hash: str
    audit_event_id: str
    receipt: dict[str, Any]


class SBO3LClientLike(Protocol):
    """Minimum surface this tool needs from an SBO3L client."""

    def submit(
        self,
        request: dict[str, Any],
        *,
        idempotency_key: str | None = ...,
    ) -> SBO3LKeeperHubSubmitResult | Awaitable[SBO3LKeeperHubSubmitResult]: ...


@dataclass(frozen=True, slots=True)
class SBO3LKeeperHubToolDescriptor:
    """Plain descriptor — wire into AutoGen via
    `ConversableAgent.register_function`, or pass into any framework
    taking `(name, description, callable)`."""

    name: str
    description: str
    func: Callable[[str], str]


_DEFAULT_NAME = "sbo3l_keeperhub_payment_request"
_DEFAULT_DESCRIPTION = (
    "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L for "
    "policy decision. On allow, the SBO3L daemon's KeeperHub adapter executes "
    "the payment by POSTing the IP-1 envelope to a KeeperHub workflow webhook "
    "and returns the captured executionId as kh_execution_ref. Input MUST be a "
    "JSON-stringified APRP. Returns: {decision, kh_workflow_id_advisory, "
    "kh_execution_ref, audit_event_id, request_hash, policy_hash, deny_code}. "
    "On deny, branch on deny_code to self-correct or escalate."
)


def sbo3l_autogen_keeperhub_tool(
    *,
    client: SBO3LClientLike,
    workflow_id: str | None = None,
    name: str = _DEFAULT_NAME,
    description: str = _DEFAULT_DESCRIPTION,
    idempotency_key: Callable[[dict[str, Any]], str] | None = None,
) -> SBO3LKeeperHubToolDescriptor:
    """Build the SBO3L → KeeperHub tool descriptor.

    `workflow_id` is **advisory only** — surfaced in the envelope as
    `kh_workflow_id_advisory` for context tagging / audit logs. Actual
    routing is the daemon's env-configured `SBO3L_KEEPERHUB_WEBHOOK_URL`.

    Wire into AutoGen via:

        from autogen import ConversableAgent
        from sbo3l_sdk import SBO3LClientSync
        from sbo3l_autogen_keeperhub import sbo3l_autogen_keeperhub_tool

        client = SBO3LClientSync("http://localhost:8730")
        descriptor = sbo3l_autogen_keeperhub_tool(client=client)
        executor = ConversableAgent(name="executor", llm_config=False)
        executor.register_function(
            function_map={descriptor.name: descriptor.func},
        )

    Or use the higher-level helper `register_sbo3l_keeperhub_tool` from
    `sbo3l_autogen_keeperhub` directly.
    """

    kh_workflow_id = (
        workflow_id or os.environ.get("SBO3L_KEEPERHUB_WORKFLOW_ID") or DEFAULT_KH_WORKFLOW_ID
    )

    def _func(input_str: str) -> str:
        try:
            parsed: object = json.loads(input_str)
        except json.JSONDecodeError as e:
            return json.dumps({"error": "input is not valid JSON", "detail": str(e)})

        if not isinstance(parsed, dict):
            ty = (
                "null"
                if parsed is None
                else "array"
                if isinstance(parsed, list)
                else type(parsed).__name__
            )
            return json.dumps(
                {"error": "input must be a JSON object (APRP)", "input_received_type": ty}
            )

        body: dict[str, Any] = parsed
        kwargs: dict[str, Any] = {}
        try:
            if idempotency_key is not None:
                kwargs["idempotency_key"] = idempotency_key(body)
            else:
                kwargs["idempotency_key"] = str(uuid.uuid4())

            result = client.submit(body, **kwargs)
            if hasattr(result, "__await__"):
                # Async client passed to a sync tool. Close the coroutine
                # to avoid `RuntimeWarning: coroutine never awaited`.
                close = getattr(result, "close", None)
                if callable(close):
                    try:
                        close()
                    except Exception:
                        pass
                return json.dumps(
                    {
                        "error": "transport.async_client_in_sync_tool",
                        "detail": (
                            "client.submit returned an awaitable; pass a sync client "
                            "(SBO3LClientSync) to sbo3l_autogen_keeperhub_tool."
                        ),
                    }
                )
            r = _coerce_to_dict(result)
        except Exception as e:
            code = getattr(e, "code", None)
            status = getattr(e, "status", None)
            return json.dumps(
                {
                    "error": code if isinstance(code, str) else "transport.failed",
                    "status": status if isinstance(status, int) else None,
                    "detail": str(e),
                }
            )

        receipt = r.get("receipt") if isinstance(r.get("receipt"), dict) else {}
        execution_ref = receipt.get("execution_ref") if isinstance(receipt, dict) else None

        return json.dumps(
            {
                "decision": r.get("decision"),
                "kh_workflow_id_advisory": kh_workflow_id,
                # Only surface kh_execution_ref on allow — daemon never
                # asks the KH adapter to run on deny / requires_human.
                "kh_execution_ref": execution_ref if r.get("decision") == "allow" else None,
                "audit_event_id": r.get("audit_event_id"),
                "request_hash": r.get("request_hash"),
                "policy_hash": r.get("policy_hash"),
                "matched_rule_id": r.get("matched_rule_id"),
                "deny_code": r.get("deny_code"),
            }
        )

    return SBO3LKeeperHubToolDescriptor(name=name, description=description, func=_func)


def _coerce_to_dict(result: Any) -> dict[str, Any]:
    """Accept either a dict or a Pydantic BaseModel."""

    if isinstance(result, dict):
        return result
    dump = getattr(result, "model_dump", None)
    if callable(dump):
        return dump(mode="json", by_alias=True)  # type: ignore[no-any-return]
    raise TypeError(
        f"client.submit returned {type(result).__name__}; expected dict or Pydantic BaseModel"
    )
