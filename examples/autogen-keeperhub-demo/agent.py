"""autogen-keeperhub-demo — 2-agent AutoGen conversation gated by SBO3L.

Demonstrates the AutoGen-specific composition shape:

  1. **Planner** decides what work needs doing (3 sequential research tasks).
  2. **Executor** holds the SBO3L → KeeperHub tool and runs each task
     by calling `sbo3l_keeperhub_payment_request(aprp_json=...)` against
     the SBO3L daemon. Each call returns either a signed PolicyReceipt +
     KH execution_ref (allow) or a deny envelope with branch-on code.

The conversation is hardcoded (no OpenAI API key required) — every
"reasoning" turn is a plain Python statement so the wire path
(planner-message → executor-tool-call → SBO3L decide → KH execute)
stays visible without an LLM in the loop. The same code shape works
unchanged when you swap the planner for a real `AssistantAgent` with
`model_client=OpenAIChatCompletionClient(...)`.

Run:
    python agent.py

Expected output:
  - 3 ALLOW envelopes with kh_execution_ref populated
  - One audit log dump linking each conversation turn → SBO3L
    audit_event_id → KH execution_ref
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from typing import Any

from sbo3l_autogen_keeperhub import sbo3l_autogen_keeperhub_tool
from sbo3l_sdk import SBO3LClientSync


@dataclass
class _MockAgent:
    """Minimal duck-typed stand-in for autogen.ConversableAgent.

    AutoGen 0.2.x's `ConversableAgent.register_function(function_map=...)`
    surface — the only piece we need to demo the SBO3L registration
    pattern without requiring the real package (which carries an
    OpenAI/Anthropic API key requirement once a real LLM client is
    wired up). The mock dispatches `function_call` exactly the way the
    real ConversableAgent does: looks up by name, calls with kwargs.
    """

    name: str
    function_map: dict[str, Any] = field(default_factory=dict)

    def register_function(self, function_map: dict[str, Any]) -> None:
        self.function_map.update(function_map)

    def call_tool(self, tool_name: str, **kwargs: Any) -> str:
        if tool_name not in self.function_map:
            raise KeyError(f"agent {self.name!r} has no tool {tool_name!r}")
        # The function_map callable on the legacy ConversableAgent path
        # is registered with a single `aprp_json: str` positional arg.
        # AutoGen marshals the LLM's tool-call JSON into kwargs; we
        # mirror that here.
        if "aprp_json" in kwargs:
            return str(self.function_map[tool_name](kwargs["aprp_json"]))
        # Convenience: if caller passed a dict APRP, JSON-stringify it.
        if "aprp" in kwargs:
            return str(self.function_map[tool_name](json.dumps(kwargs["aprp"])))
        raise TypeError(
            f"call_tool({tool_name!r}) expected 'aprp_json' or 'aprp' kwarg, "
            f"got {sorted(kwargs)}"
        )


def _aprp(task: str) -> dict[str, Any]:
    return {
        # research-agent-01 is the only agent_id registered in the bundled
        # reference policy. Demos that hardcode a different id are denied
        # before policy evaluation (auth.agent_not_found). Use SBO3L_POLICY
        # to load a custom policy if you want a different label here.
        "agent_id": "research-agent-01",
        "task_id": f"autogen-{task}-{uuid.uuid4().hex[:8]}",
        "intent": "purchase_api_call",
        "amount": {"value": "0.05", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": f"https://api.example.com/v1/{task}",
            "method": "POST",
            "expected_recipient": "0x1111111111111111111111111111111111111111",
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(uuid.uuid4()),
        "risk_class": "low",
    }


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"daemon: {endpoint}")

    # Real AutoGen would be:
    #   from autogen import ConversableAgent
    #   executor = ConversableAgent(name="executor", llm_config=False)
    # We use the duck-typed _MockAgent so the demo runs end-to-end
    # without an LLM API key. The SBO3L registration step + tool
    # dispatch shape is identical between mock + real.
    executor = _MockAgent(name="executor")

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_autogen_keeperhub_tool(client=client)
        executor.register_function(function_map={descriptor.name: descriptor.func})

        # The "planner" — in a real demo this is an AssistantAgent
        # producing tool-calls from natural-language reasoning. Here
        # it's a hardcoded plan so the audit log captures the same
        # event shape without an LLM.
        plan = ["search", "rerank", "summarize"]
        print(f"\nplanner: 3-step research plan: {plan}")

        audit_log: list[dict[str, Any]] = []
        for step, task in enumerate(plan, 1):
            print(f"\n--- conversation turn {step}: planner -> executor: run {task!r} ---")
            envelope_str = executor.call_tool(
                descriptor.name,
                aprp_json=json.dumps(_aprp(task)),
            )
            envelope = json.loads(envelope_str)
            print(f"  decision: {envelope.get('decision')}")
            print(f"  kh_execution_ref: {envelope.get('kh_execution_ref')}")
            print(f"  audit_event_id: {envelope.get('audit_event_id')}")
            if envelope.get("deny_code"):
                print(f"  deny_code: {envelope['deny_code']}")

            audit_log.append(
                {
                    "turn": step,
                    "task": task,
                    "decision": envelope.get("decision"),
                    "audit_event_id": envelope.get("audit_event_id"),
                    "kh_execution_ref": envelope.get("kh_execution_ref"),
                }
            )

    print("\n=== audit log: turn -> SBO3L decision -> KH execution_ref ===")
    for row in audit_log:
        print(
            f"  turn={row['turn']:>1}  task={row['task']:<10}  "
            f"decision={row['decision']:<6}  audit={row['audit_event_id']}  "
            f"kh={row['kh_execution_ref']}"
        )

    failures = [r for r in audit_log if r["decision"] != "allow"]
    if failures:
        print(f"\nfailed turns: {len(failures)}/{len(audit_log)}")
        return 1
    print(f"\nall {len(audit_log)} turns allowed + executed via KH")
    return 0


if __name__ == "__main__":
    sys.exit(main())
