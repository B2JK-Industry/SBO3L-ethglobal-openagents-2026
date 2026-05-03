# `examples/autogen-keeperhub-demo`

End-to-end runnable demo of the **SBO3L → KeeperHub policy gate inside an AutoGen 2-agent conversation**.

| File | Pattern | LOC | What it proves |
|---|---|---|---|
| [`agent.py`](agent.py) | Planner -> Executor with tool dispatch | ~140 | Each conversation turn spawns a KH execution gated by SBO3L; audit log links turn -> SBO3L `audit_event_id` -> KH `execution_ref`. |

## What it shows

A 2-agent AutoGen-style conversation:

  - **Planner** produces a 3-step research plan (`search` -> `rerank` -> `summarize`).
  - **Executor** holds the SBO3L → KeeperHub tool. Each tool call:
    1. Fires `sbo3l_keeperhub_payment_request(aprp_json=...)` against the SBO3L daemon
    2. SBO3L decides allow / deny / requires_human against the loaded policy
    3. On allow, daemon-side KH adapter executes the payment + returns `kh_execution_ref`
    4. Executor reports the receipt + execution ref back to the planner

The audit log printed at the end shows each conversation turn linked to its SBO3L `audit_event_id` and KH `execution_ref` — the cross-system trace a judge wants to see.

## Why no LLM in the demo

The planner's "reasoning" is hardcoded so the wire path stays visible without an OpenAI API key. The executor uses a duck-typed `_MockAgent` that mirrors AutoGen's legacy `ConversableAgent.register_function(function_map=...)` surface exactly — swap it for the real class + an `AssistantAgent` planner with a real `model_client` and the same code shape works unchanged.

## Prereqs

```bash
# 1. Run the SBO3L daemon in mock mode (one-time per session)
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &

# 2. Install demo deps
cd examples/autogen-keeperhub-demo
python3 -m venv .venv
.venv/bin/pip install \
  -e ../../sdks/python \
  -e ../../integrations/autogen-keeperhub-py
```

## Run

```bash
.venv/bin/python agent.py
```

Expected:

```
daemon: http://localhost:8730

planner: 3-step research plan: ['search', 'rerank', 'summarize']

--- conversation turn 1: planner -> executor: run 'search' ---
  decision: allow
  kh_execution_ref: kh-...
  audit_event_id: evt-...
[...]

=== audit log: turn -> SBO3L decision -> KH execution_ref ===
  turn=1  task=search      decision=allow   audit=evt-...  kh=kh-...
  turn=2  task=rerank      decision=allow   audit=evt-...  kh=kh-...
  turn=3  task=summarize   decision=allow   audit=evt-...  kh=kh-...

all 3 turns allowed + executed via KH
```

## Live KeeperHub mode

Add daemon-side env vars before starting `sbo3l-server`:

```bash
SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/<id>/webhook \
SBO3L_KEEPERHUB_TOKEN=wfb_<token> \
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &
```

Without these, the daemon's KH adapter falls back to `local_mock` and returns `kh-<ULID>` refs with `mock=true` evidence — the wire path stays visible end-to-end.

## Wiring with the real `autogen-agentchat` package

```python
from autogen_agentchat.agents import AssistantAgent
from sbo3l_sdk import SBO3LClientSync
from sbo3l_autogen_keeperhub import sbo3l_autogen_keeperhub_tool

with SBO3LClientSync("http://localhost:8730") as client:
    descriptor = sbo3l_autogen_keeperhub_tool(client=client)
    executor = AssistantAgent(
        name="executor",
        model_client=...,                    # OpenAIChatCompletionClient(...)
        tools=[descriptor.func],             # SBO3L tool drops in here
    )
```

For the legacy `pyautogen<0.3` `ConversableAgent`, use `register_sbo3l_keeperhub_tool(executor, client=client)` — it duck-types against either surface.
