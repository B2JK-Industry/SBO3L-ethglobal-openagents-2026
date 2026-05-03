"""Plan service — LangChain framework boundary.

Receives a user `goal` over HTTP, drafts an APRP for the planning compute,
gates it through SBO3L via sbo3l-langchain's tool descriptor, and returns
the signed receipt + the next-step APRP for the execute service.

Single endpoint: POST /plan with `{"goal": "..."}`. Response shape:

    {
      "decision": "allow" | "deny" | "error",
      "audit_event_id": "evt-...",
      "execution_ref": "kh-...",            # only on allow
      "next_action": { ... APRP ... },      # only on allow
      "deny_code": "...",                   # only on deny / error
    }
"""

from __future__ import annotations

import json
import os
from datetime import datetime, timedelta, timezone
from typing import Any

from fastapi import FastAPI
from ulid import ULID
from sbo3l_langchain import sbo3l_tool
from sbo3l_sdk import SBO3LClientSync

ENDPOINT = os.environ.get("SBO3L_ENDPOINT", "http://sbo3l-server:8730")
KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"

# Allowed recipient for `chain: "base"` per test-corpus/policy/reference_low_risk.json.
# The reference policy's allow-rule (`allow-small-x402-api-call`) requires
# input.recipient.allowed → which only fires when destination.expected_recipient
# matches an entry in policy.recipients[]. Without this address the recipient
# context falls through `known: false`, the allow rule misses, and the request
# denies as policy.deny_recipient_not_allowlisted.
ALLOWED_RECIPIENT_BASE = "0x1111111111111111111111111111111111111111"

app = FastAPI(title="sbo3l-multi-plan")


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "framework": "langchain"}


@app.post("/plan")
def plan(body: dict[str, Any]) -> dict[str, Any]:
    goal = body.get("goal", "research and execute a paid API call")

    # `intent` MUST match the reference policy's allow rule literal
    # ("purchase_api_call"); `pay_compute_job` was a planning placeholder
    # that the policy doesn't recognise → fell through to default-deny.
    # `expected_recipient` populates the recipient context so the
    # input.recipient.allowed clause of the allow rule is satisfied.
    plan_aprp = {
        "agent_id": "research-agent-01",
        "task_id": "multi-fw-plan-1",
        "intent": "purchase_api_call",
        "amount": {"value": "0.01", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/plan",
            "method": "POST",
            "expected_recipient": ALLOWED_RECIPIENT_BASE,
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(ULID()),
        "risk_class": "low",
    }

    with SBO3LClientSync(ENDPOINT) as client:
        tool = sbo3l_tool(client=client)
        envelope = json.loads(tool.func(json.dumps(plan_aprp)))

    if envelope.get("decision") != "allow":
        return {
            "decision": envelope.get("decision", envelope.get("error", "error")),
            "deny_code": envelope.get("deny_code", envelope.get("error")),
            "audit_event_id": envelope.get("audit_event_id"),
            "step": "plan",
            "framework": "langchain",
            "kh_workflow_id": KH_WORKFLOW_ID,
            "goal": goal,
        }

    next_action = {
        "agent_id": "research-agent-01",
        "task_id": "multi-fw-execute-1",
        "intent": "purchase_api_call",
        "amount": {"value": "0.05", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/inference",
            "method": "POST",
            "expected_recipient": ALLOWED_RECIPIENT_BASE,
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(ULID()),
        "risk_class": "low",
    }
    return {
        "decision": "allow",
        "audit_event_id": envelope["audit_event_id"],
        "execution_ref": envelope.get("execution_ref"),
        "next_action": next_action,
        "step": "plan",
        "framework": "langchain",
        "kh_workflow_id": KH_WORKFLOW_ID,
        "goal": goal,
    }
