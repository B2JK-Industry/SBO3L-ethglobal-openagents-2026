"""Pydantic-typed SBO3L tool builder for Pydantic AI agents."""

from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Literal, Protocol

from pydantic import BaseModel, Field


class PolicyDenyError(Exception):
    """Raised when SBO3L returns ``deny`` / ``requires_human``."""

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
    """Minimum surface this tool needs from an SBO3L sync client."""

    def submit(
        self,
        request: dict[str, Any],
        *,
        idempotency_key: str | None = ...,
    ) -> Any: ...


# ---------------------------------------------------------------------------
# Pydantic input models — the local-validation surface
# ---------------------------------------------------------------------------


class AprpAmount(BaseModel):
    """Amount in fiat-pegged units."""

    value: str = Field(
        pattern=r"^(0|[1-9][0-9]*)(\.[0-9]{1,18})?$",
        description='Decimal string (e.g. "0.05").',
    )
    currency: Literal["USD"] = "USD"


class AprpDestination(BaseModel):
    """Where the payment goes; shape depends on `type`."""

    type: Literal["x402_endpoint", "eoa", "smart_account", "erc20_transfer"]
    url: str | None = None
    method: Literal["GET", "POST", "PUT", "PATCH", "DELETE"] | None = None
    address: str | None = None
    token_address: str | None = None
    recipient: str | None = None
    expected_recipient: str | None = None


class AprpInput(BaseModel):
    """APRP v1 payload, validated before the SBO3L round-trip.

    Pydantic AI exposes this as the tool's typed parameter. The model's
    field constraints (regex patterns, enum literals) catch malformed
    LLM output without a network hop — same local-first win
    ``@sbo3l/anthropic`` gets via zod.
    """

    agent_id: str = Field(
        pattern=r"^[a-z0-9][a-z0-9_-]{2,63}$",
        description="Stable agent slug (lowercase alphanumeric, _, -; 3-64 chars).",
    )
    task_id: str = Field(
        pattern=r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$",
        description="Caller-chosen task identifier (1-64 chars).",
    )
    intent: Literal[
        "purchase_api_call",
        "purchase_dataset",
        "pay_compute_job",
        "pay_agent_service",
        "tip",
    ]
    amount: AprpAmount
    token: str = Field(
        pattern=r"^[A-Z0-9]{2,16}$",
        description="Settlement token symbol (e.g. USDC, USDT).",
    )
    destination: AprpDestination
    payment_protocol: Literal["x402", "l402", "erc20_transfer", "smart_account_session"]
    chain: str = Field(
        pattern=r"^[a-z0-9][a-z0-9_-]{1,31}$",
        description="Chain id (e.g. base, sepolia).",
    )
    provider_url: str
    expiry: str = Field(description="RFC 3339 timestamp.")
    nonce: str = Field(description="ULID or UUID for replay protection.")
    risk_class: Literal["low", "medium", "high", "critical"]


# ---------------------------------------------------------------------------
# Descriptor + factory
# ---------------------------------------------------------------------------


@dataclass(frozen=True, slots=True)
class SBO3LToolDescriptor:
    """Plain dataclass — no Pydantic AI import.

    Consumers wire ``func`` into their agent (typically via ``@agent.tool_plain``
    or ``@agent.tool``) so the integration package stays Pydantic-AI-version
    agnostic.
    """

    name: str
    description: str
    func: Callable[[str], str]


_DEFAULT_NAME = "sbo3l_payment_request"
_DEFAULT_DESCRIPTION = (
    "Submit a Pydantic-validated APRP v1 payload to SBO3L's policy boundary "
    "BEFORE any payment-shaped action.  Input MUST conform to AprpInput.  Returns a "
    "JSON envelope with `decision` (allow|deny|requires_human), `audit_event_id`, "
    "and (on allow) `execution_ref`.  On deny, branch on `deny_code`."
)


def _coerce_to_dict(obj: Any) -> dict[str, Any]:
    """Convert SBO3LClientSync.submit's response into a plain dict.

    Same pattern as the agno / langchain-py / crewai / llamaindex adapters —
    real client returns Pydantic ``PaymentRequestResponse``; tests pass dict.
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
    """Build the SBO3L Pydantic AI tool descriptor.

    The returned ``func(input_str)`` ALWAYS returns a JSON string so the
    Pydantic AI agent's tool-call loop has a stable shape regardless of
    decision.  It does NOT raise — denies and transport failures both
    surface as JSON envelopes so the model can self-correct rather than
    crashing the agent run.

    Local Pydantic validation runs FIRST: malformed inputs (wrong enum,
    bad regex) surface as ``{"error": "input.bad_arguments", "issues": ...}``
    without a daemon round-trip.
    """

    def _func(input_str: str) -> str:
        # Local Pydantic validation. Catches malformed LLM output before
        # we touch the network — same win @sbo3l/anthropic has with zod.
        try:
            aprp = AprpInput.model_validate_json(input_str)
        except Exception as e:  # pydantic.ValidationError or json.JSONDecodeError
            return json.dumps(
                {
                    "error": "input.bad_arguments",
                    "detail": str(e)[:512],
                }
            )

        body = aprp.model_dump(exclude_none=True)
        kwargs: dict[str, Any] = {}
        if idempotency_key is not None:
            kwargs["idempotency_key"] = idempotency_key(body)

        try:
            raw = client.submit(body, **kwargs)
        except Exception as e:
            code = getattr(e, "code", None)
            return json.dumps(
                {
                    "error": code if isinstance(code, str) else "transport.failed",
                    "detail": str(e)[:512],
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
