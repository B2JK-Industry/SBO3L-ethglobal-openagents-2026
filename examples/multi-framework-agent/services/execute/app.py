"""Execute service — CrewAI framework boundary.

Receives a `next_action` APRP from the plan step, gates it through SBO3L
via sbo3l-crewai's tool descriptor, and returns the signed receipt + the
next-step APRP for the confirm service. Different framework, same SBO3L
audit chain.
"""

from __future__ import annotations

import json
import os
from datetime import datetime, timedelta, timezone
from typing import Any

from fastapi import FastAPI
from ulid import ULID
from sbo3l_crewai import sbo3l_tool
from sbo3l_sdk import SBO3LClientSync

ENDPOINT = os.environ.get("SBO3L_ENDPOINT", "http://sbo3l-server:8730")
KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"
ALLOWED_RECIPIENT_BASE = "0x1111111111111111111111111111111111111111"

app = FastAPI(title="sbo3l-multi-execute")


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "framework": "crewai"}


@app.post("/execute")
def execute(body: dict[str, Any]) -> dict[str, Any]:
    aprp = body.get("aprp")
    if not isinstance(aprp, dict):
        return {"decision": "error", "deny_code": "input.no_aprp", "step": "execute"}

    with SBO3LClientSync(ENDPOINT) as client:
        tool = sbo3l_tool(client=client)
        envelope = json.loads(tool.func(json.dumps(aprp)))

    if envelope.get("decision") != "allow":
        return {
            "decision": envelope.get("decision", envelope.get("error", "error")),
            "deny_code": envelope.get("deny_code", envelope.get("error")),
            "audit_event_id": envelope.get("audit_event_id"),
            "step": "execute",
            "framework": "crewai",
            "kh_workflow_id": KH_WORKFLOW_ID,
        }

    next_action = {
        "agent_id": "research-agent-01",
        "task_id": "multi-fw-confirm-1",
        "intent": "purchase_api_call",
        "amount": {"value": "0.01", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/confirm",
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
        "step": "execute",
        "framework": "crewai",
        "kh_workflow_id": KH_WORKFLOW_ID,
    }
