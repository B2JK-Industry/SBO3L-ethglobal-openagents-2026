# KeeperHub × LangChain (Python) — 5-min quickstart

Run a LangChain agent that submits a payment intent through SBO3L and fires a real KeeperHub workflow on `allow`.

**Bounty:** KeeperHub ($2.5K / $1.5K / $500 + $250 Builder Feedback)
**Framework:** LangChain (Python)
**Time:** 5 min

## 1. Install (3 lines)

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install sbo3l-sdk sbo3l-langchain langchain-openai
export OPENAI_API_KEY=sk-...
```

## 2. Configure

The SBO3L daemon is at `http://localhost:8730` (see [common prerequisites](index.md#common-prerequisites-all-guides)).
KH workflow id `m4t4cnpmhv8qquce3bv3c` is wired into the daemon's KH adapter; nothing to set in the demo.

## 3. Code (`agent.py`)

```python
import os, uuid
from datetime import datetime, timedelta, timezone
from langchain.agents import AgentExecutor, create_openai_functions_agent
from langchain.tools import StructuredTool
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
from langchain_openai import ChatOpenAI

from sbo3l_sdk import SBO3LClientSync
from sbo3l_langchain import sbo3l_tool

client = SBO3LClientSync(os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730"))
sbo3l = sbo3l_tool(client=client)

tool = StructuredTool.from_function(name=sbo3l.name, description=sbo3l.description, func=sbo3l.func)

aprp = {
    "agent_id": "research-agent-01",
    "task_id": "kh-quickstart-1",
    "intent": "purchase_api_call",
    "amount": {"value": "0.05", "currency": "USD"},
    "token": "USDC",
    "destination": {
        "type": "x402_endpoint",
        "url": "https://api.example.com/v1/inference",
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

prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a research agent. Always call sbo3l_payment_request before paying."),
    ("user", f"Submit this APRP: {aprp}"),
    MessagesPlaceholder("agent_scratchpad"),
])

llm = ChatOpenAI(model="gpt-4o-mini", temperature=0)
agent = create_openai_functions_agent(llm=llm, tools=[tool], prompt=prompt)
executor = AgentExecutor(agent=agent, tools=[tool], verbose=True)

print(executor.invoke({"input": "go"}))
```

## 4. Run

```bash
python agent.py
```

## 5. What you'll see

```
> Entering new AgentExecutor chain...
Invoking: `sbo3l_payment_request` with `{...APRP JSON...}`
{"decision": "allow", "execution_ref": "kh-...", "audit_event_id": "evt-..."}
> Finished chain.
```

Verify the receipt offline:

```bash
sbo3l passport verify --strict --capsule <(curl -s http://localhost:8730/v1/audit/<audit_event_id>/capsule)
```

## 6. Troubleshoot

- **`protocol.nonce_replay`** — your APRP has a static `nonce`. The snippet uses `uuid.uuid4()` per call; re-check you didn't paste a literal.
- **`policy.deny_recipient_not_allowlisted`** — the policy's allowed recipient on `chain: base` is `0x1111...1111`. Use that exact address in `destination.expected_recipient`.
- **`policy.deny_unknown_provider`** — the reference policy trusts `api.example.com`. Use a different `provider_url` and you must add it to `policy.providers[]`.
- **Tool call never fires** — make sure your `OPENAI_API_KEY` is set and the model supports function-calling (`gpt-4o-mini` ✓).
- **SDK install fails** — Python 3.10+ required.

## Next

- [KH × OpenAI Assistants](keeperhub-with-openai-assistants.md) for a TS variant
- [Cross-framework killer demo](../../examples/multi-framework-agent/README.md) — LangChain → CrewAI → AutoGen, single audit chain
