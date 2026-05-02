"""KeeperHub-flavored LangChain tool for SBO3L.

Builds a callable that submits an APRP through SBO3L's policy boundary
with KeeperHub configured as the downstream executor. The tool returns
the combined envelope: SBO3L's signed `PolicyReceipt` plus KeeperHub's
`execution_ref` (the workflow execution id) when allowed.

Wire path:

  1. tool.func(aprp_json) → POST /v1/payment-requests on the SBO3L daemon
  2. SBO3L decides allow / deny / requires_human on the APRP
  3. On allow: SBO3L's executor_callback hands the signed receipt to
     the daemon-side KeeperHub adapter (`crates/sbo3l-keeperhub-adapter`,
     configured via `SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN`
     env vars on the daemon process — NOT on the agent process)
  4. KH adapter POSTs the IP-1 envelope to the workflow webhook,
     captures the executionId, returns it as `receipt.execution_ref`
  5. Tool returns:
       {
         "decision": "allow" | "deny" | "requires_human",
         "kh_workflow_id_advisory": "<advisory tag — daemon env routes the actual call>",
         "kh_execution_ref": "kh-01HTAWX5..." | None,
         "audit_event_id": "evt-...",
         "request_hash": "...", "policy_hash": "...",
         "deny_code": null | "..."
       }

This is a thin shim. The heavy lifting (signing, IP-1 envelope construction,
webhook POST, retry / timeout) is all in the Rust crate
`sbo3l-keeperhub-adapter`, executed inside the daemon. The agent process
only sees the SBO3L HTTP surface.

Why a separate KH-flavored tool (vs the generic `sbo3l_tool` already in
`tools.py`)? Two reasons:
  a) Discoverability — judges scanning the example see "ah, this is the
     KH path" without grepping daemon config.
  b) The return shape names `kh_*` keys explicitly, so a downstream LLM
     can branch on `kh_execution_ref` presence without inferring from
     `execution_ref`'s prefix.
"""

from __future__ import annotations

import json
import os
import uuid
from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Any

from sbo3l_sdk import SBO3LClientSync


#: Live KeeperHub workflow id verified end-to-end on 2026-04-30.
#: Override per-call via `workflow_id` arg (advisory only — see docstring), or globally via
#: `SBO3L_KEEPERHUB_WORKFLOW_ID` env on the agent process (advisory —
#: the daemon ultimately decides which webhook URL it POSTs to).
DEFAULT_KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"


@dataclass(frozen=True, slots=True)
class KeeperHubToolDescriptor:
    """Plain descriptor — wire into LangChain via `StructuredTool.from_function`.

    `func` takes a JSON-stringified APRP and returns a JSON-stringified
    envelope (so the LLM can parse it without a custom tool schema).
    """

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
    "On deny, branch on deny_code."
)


def keeperhub_tool(
    *,
    client: SBO3LClientSync,
    workflow_id: str | None = None,
    name: str = _DEFAULT_NAME,
    description: str = _DEFAULT_DESCRIPTION,
) -> KeeperHubToolDescriptor:
    """Build the KH-flavored LangChain tool descriptor.

    `workflow_id` is **advisory only** — it is reported back in the
    envelope under `kh_workflow_id_advisory` so the agent / audit log
    can record which workflow the caller intended, but the SBO3L
    daemon's KH adapter routes per its own `SBO3L_KEEPERHUB_WEBHOOK_URL`
    env var (the per-call workflow_id is NOT injected into the APRP
    body and the daemon does NOT receive it). If the caller's
    `workflow_id` doesn't match what the daemon is configured for, the
    actual KH execution will land on the daemon-configured workflow,
    not the caller's. Use this field for context tagging only; do not
    rely on it as a routing override.

    `workflow_id` defaults to `DEFAULT_KH_WORKFLOW_ID` (the live workflow
    verified during the ETHGlobal Open Agents 2026 submission).

    Wire it into LangChain via:

        from langchain_core.tools import StructuredTool
        from sbo3l_sdk import SBO3LClientSync
        from sbo3l_langchain_demo.keeperhub_tool import keeperhub_tool

        client = SBO3LClientSync("http://localhost:8730")
        descriptor = keeperhub_tool(client=client)
        tool = StructuredTool.from_function(
            func=descriptor.func,
            name=descriptor.name,
            description=descriptor.description,
        )

    Note the daemon — not the agent — needs `SBO3L_KEEPERHUB_WEBHOOK_URL`
    and `SBO3L_KEEPERHUB_TOKEN` set for live KH execution. Without them
    the daemon's KH adapter falls back to local_mock and returns a
    `kh-<ULID>` ref with `mock=true` evidence.
    """

    kh_workflow_id = (
        workflow_id
        or os.environ.get("SBO3L_KEEPERHUB_WORKFLOW_ID")
        or DEFAULT_KH_WORKFLOW_ID
    )

    def _func(input_str: str) -> str:
        try:
            parsed = json.loads(input_str)
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

        try:
            response = client.submit(parsed, idempotency_key=str(uuid.uuid4()))
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

        r = _coerce_to_dict(response)
        receipt = r.get("receipt") if isinstance(r.get("receipt"), dict) else {}
        execution_ref = receipt.get("execution_ref") if isinstance(receipt, dict) else None

        return json.dumps(
            {
                "decision": r.get("decision"),
                # Renamed from `kh_workflow_id` to make the contract
                # honest: this value is the caller's intended target,
                # NOT the workflow the daemon actually routed to. The
                # daemon-configured webhook URL (env-var-driven) is the
                # source of truth for actual routing — see the
                # `keeperhub_tool` docstring + KeeperHub/cli#52.
                "kh_workflow_id_advisory": kh_workflow_id,
                # `kh_execution_ref` is None on deny / requires_human, OR on
                # allow when the daemon's KH adapter ran in local_mock with
                # no webhook configured but failed to populate execution_ref
                # (shouldn't happen — local_mock always returns kh-<ULID> —
                # but coded defensively).
                "kh_execution_ref": execution_ref if r.get("decision") == "allow" else None,
                "audit_event_id": r.get("audit_event_id"),
                "request_hash": r.get("request_hash"),
                "policy_hash": r.get("policy_hash"),
                "matched_rule_id": r.get("matched_rule_id"),
                "deny_code": r.get("deny_code"),
            }
        )

    return KeeperHubToolDescriptor(name=name, description=description, func=_func)


def build_demo_aprp(
    *,
    agent_id: str = "research-agent-kh-01",
    task_id: str | None = None,
    amount_usd: str = "0.05",
    recipient: str = "0x1111111111111111111111111111111111111111",
) -> dict[str, Any]:
    """Construct a demo APRP body that flows through the daemon's reference
    policy on the allow path. Fresh nonce + 5-min expiry per call.

    The reference policy (`policy/reference/keeperhub.yaml`) allows
    `chain=base + recipient=0x1111...1111 + risk=low + amount<=0.05 USDC`.
    Tweak any of these and the daemon will deny — useful for the
    deny-branch leg of the demo.
    """

    return {
        "agent_id": agent_id,
        "task_id": task_id or f"kh-demo-{uuid.uuid4().hex[:8]}",
        "intent": "purchase_api_call",
        "amount": {"value": amount_usd, "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/inference",
            "method": "POST",
            "expected_recipient": recipient,
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(uuid.uuid4()),
        "risk_class": "low",
    }


def _coerce_to_dict(result: Any) -> dict[str, Any]:
    """Accept either a dict or a Pydantic BaseModel.

    `sbo3l_sdk.SBO3LClientSync.submit()` returns the Pydantic
    `PaymentRequestResponse`; tests using `pytest_httpx` may yield plain
    dicts. Either is fine wire-shape-wise; we read the same fields.
    """

    if isinstance(result, dict):
        return result
    dump = getattr(result, "model_dump", None)
    if callable(dump):
        return dump(mode="json", by_alias=True)  # type: ignore[no-any-return]
    raise TypeError(
        f"client.submit returned {type(result).__name__}; expected dict or Pydantic BaseModel"
    )
