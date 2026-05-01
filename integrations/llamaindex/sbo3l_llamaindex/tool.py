"""LlamaIndex tool factory for SBO3L.

The descriptor returned by `sbo3l_tool(...)` plugs into:

  - `llama_index.core.tools.FunctionTool.from_defaults(fn=..., name=..., description=...)`
  - any framework that accepts (name, description, callable).

We do NOT import from `llama_index` here — that would make it a hard dep.
Consumers wire the descriptor into `FunctionTool.from_defaults`.
"""

from __future__ import annotations

import json
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any, Protocol, TypedDict


class SBO3LSubmitResult(TypedDict):
    """Subset of the SBO3L response envelope this tool returns to the LLM."""

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
    ) -> SBO3LSubmitResult | Awaitable[SBO3LSubmitResult]: ...


@dataclass(frozen=True, slots=True)
class SBO3LToolDescriptor:
    """Plain descriptor that consumers wire into `FunctionTool.from_defaults`."""

    name: str
    description: str
    func: Callable[[str], str]


_DEFAULT_NAME = "sbo3l_payment_request"
_DEFAULT_DESCRIPTION = (
    "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L for policy "
    "decision. Input MUST be a JSON-stringified APRP object containing fields: agent_id, "
    "task_id, intent, amount, token, destination, payment_protocol, chain, provider_url, "
    "expiry, nonce, risk_class. Returns a JSON object with decision (allow|deny|"
    "requires_human), execution_ref (when allowed), and audit_event_id. On deny, branch "
    "on deny_code to self-correct or escalate."
)


def sbo3l_tool(
    *,
    client: SBO3LClientLike,
    name: str = _DEFAULT_NAME,
    description: str = _DEFAULT_DESCRIPTION,
    idempotency_key: Callable[[dict[str, Any]], str] | None = None,
) -> SBO3LToolDescriptor:
    """Build the SBO3L LlamaIndex tool descriptor.

    Wire it into LlamaIndex via:

        from llama_index.core.tools import FunctionTool
        from sbo3l_sdk import SBO3LClientSync
        from sbo3l_llamaindex import sbo3l_tool

        client = SBO3LClientSync("http://localhost:8730")
        descriptor = sbo3l_tool(client=client)
        tool = FunctionTool.from_defaults(
            fn=descriptor.func,
            name=descriptor.name,
            description=descriptor.description,
        )

    On `deny`, the LLM sees `deny_code`. Transport / auth failures surface
    as a JSON envelope with `error` (RFC 7807 domain code).
    """

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
        if idempotency_key is not None:
            kwargs["idempotency_key"] = idempotency_key(body)

        try:
            result = client.submit(body, **kwargs)
            if hasattr(result, "__await__"):
                return json.dumps(
                    {
                        "error": "transport.async_client_in_sync_tool",
                        "detail": (
                            "client.submit returned an awaitable; pass a sync client "
                            "(SBO3LClientSync) to the LlamaIndex tool."
                        ),
                    }
                )
            r: SBO3LSubmitResult = result
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

        receipt = r["receipt"] if isinstance(r["receipt"], dict) else {}
        return json.dumps(
            {
                "decision": r["decision"],
                "deny_code": r.get("deny_code"),
                "matched_rule_id": r.get("matched_rule_id"),
                "execution_ref": receipt.get("execution_ref"),
                "audit_event_id": r["audit_event_id"],
                "request_hash": r["request_hash"],
                "policy_hash": r["policy_hash"],
            }
        )

    return SBO3LToolDescriptor(name=name, description=description, func=_func)
