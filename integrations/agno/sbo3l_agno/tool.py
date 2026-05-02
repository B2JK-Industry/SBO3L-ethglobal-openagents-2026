"""Agno-shaped tool builder for SBO3L."""

from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Protocol


class PolicyDenyError(Exception):
    """Raised when SBO3L returns ``deny`` / ``requires_human``.

    Agno surfaces this to the LLM as a tool execution error.  The LLM can
    then branch on the structured envelope (``deny_code``, ``audit_event_id``)
    rather than crashing the run.
    """

    def __init__(
        self,
        decision: str,
        deny_code: str | None,
        matched_rule_id: str | None,
        audit_event_id: str,
    ) -> None:
        self.decision = decision
        self.deny_code = deny_code
        self.matched_rule_id = matched_rule_id
        self.audit_event_id = audit_event_id
        if decision == "deny":
            msg = f"SBO3L denied payment intent ({deny_code or 'policy.unknown'})"
        else:
            msg = f"SBO3L requires human approval ({deny_code or 'policy.requires_human'})"
        super().__init__(msg)


class SBO3LClientLike(Protocol):
    """Minimum surface this tool needs from an SBO3L sync client.

    Matches ``sbo3l_sdk.SBO3LClientSync.submit``.  We use a Protocol so
    tests can pass a fake without instantiating the real client.
    """

    def submit(
        self,
        request: dict[str, Any],
        *,
        idempotency_key: str | None = ...,
    ) -> Any: ...


@dataclass(frozen=True, slots=True)
class SBO3LToolDescriptor:
    """Descriptor consumers wire into Agno's ``Toolkit.register`` path.

    Plain dataclass — no Agno import.  Consumers wrap it themselves so
    the integration package stays Agno-version agnostic.
    """

    name: str
    description: str
    func: Callable[[str], str]


_DEFAULT_NAME = "sbo3l_payment_request"
_DEFAULT_DESCRIPTION = (
    "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L's policy "
    "boundary BEFORE any payment-shaped action.  Input MUST be a JSON-stringified "
    "APRP object with fields: agent_id, task_id, intent, amount, token, destination, "
    "payment_protocol, chain, provider_url, expiry, nonce, risk_class.  Returns a JSON "
    "object with `decision` (allow|deny|requires_human), `execution_ref` (when allowed), "
    "and `audit_event_id`.  On deny, branch on `deny_code` to self-correct or escalate."
)


def _coerce_to_dict(obj: Any) -> dict[str, Any]:
    """Convert SBO3LClientSync.submit's response into a plain dict.

    The real client returns a Pydantic ``PaymentRequestResponse``; tests may
    pass a plain dict.  Either shape lands here through the same callable
    path.  We never trust ``__getitem__`` — it isn't on the Pydantic
    model — so we go through ``model_dump`` when available.
    """
    if isinstance(obj, dict):
        return obj
    dump = getattr(obj, "model_dump", None)
    if callable(dump):
        result = dump()
        if isinstance(result, dict):
            return result
    raise TypeError(f"cannot coerce {type(obj).__name__} to dict")


def sbo3l_payment_request_func(
    *,
    client: SBO3LClientLike,
    name: str = _DEFAULT_NAME,
    description: str = _DEFAULT_DESCRIPTION,
    idempotency_key: Callable[[dict[str, Any]], str] | None = None,
) -> SBO3LToolDescriptor:
    """Build the SBO3L Agno tool descriptor.

    The returned ``func(input_str)`` ALWAYS returns a JSON string so Agno's
    function-calling loop has a stable shape regardless of decision.  It does
    NOT raise — denies and transport failures both surface as JSON envelopes
    so the model can self-correct rather than crashing the run.
    """

    def _func(input_str: str) -> str:
        try:
            parsed: object = json.loads(input_str)
        except json.JSONDecodeError as e:
            return json.dumps(
                {"error": "input.bad_arguments", "detail": str(e)}
            )

        if not isinstance(parsed, dict):
            type_name = (
                "null"
                if parsed is None
                else "array" if isinstance(parsed, list) else type(parsed).__name__
            )
            return json.dumps(
                {
                    "error": "input.bad_arguments",
                    "detail": f"input must be a JSON object (APRP), got {type_name}",
                }
            )

        body: dict[str, Any] = parsed
        kwargs: dict[str, Any] = {}

        # The user-supplied idempotency_key callback runs INSIDE the same
        # try block as client.submit so an exception from the callback
        # (e.g. KeyError from a missing field) surfaces as a structured
        # tool error envelope instead of escaping into Agno's loop and
        # crashing the run. The tool's contract promises "never raises".
        try:
            if idempotency_key is not None:
                kwargs["idempotency_key"] = idempotency_key(body)
            raw = client.submit(body, **kwargs)
        except Exception as e:  # noqa: BLE001 — surface anything to the LLM
            code = getattr(e, "code", None)
            return json.dumps(
                {
                    "error": code if isinstance(code, str) else "transport.failed",
                    "detail": str(e),
                }
            )

        try:
            envelope = _coerce_to_dict(raw)
        except TypeError as e:
            return json.dumps({"error": "transport.unexpected_response", "detail": str(e)})

        decision = envelope.get("decision")
        if decision != "allow":
            return json.dumps(
                {
                    "error": "policy.deny" if decision == "deny" else "policy.requires_human",
                    "decision": decision,
                    "deny_code": envelope.get("deny_code"),
                    "matched_rule_id": envelope.get("matched_rule_id"),
                    "audit_event_id": envelope.get("audit_event_id"),
                }
            )

        receipt_raw = envelope.get("receipt")
        receipt = receipt_raw if isinstance(receipt_raw, dict) else {}
        return json.dumps(
            {
                "decision": "allow",
                "audit_event_id": envelope.get("audit_event_id"),
                "execution_ref": receipt.get("execution_ref"),
                "request_hash": envelope.get("request_hash"),
                "policy_hash": envelope.get("policy_hash"),
            }
        )

    return SBO3LToolDescriptor(name=name, description=description, func=_func)
