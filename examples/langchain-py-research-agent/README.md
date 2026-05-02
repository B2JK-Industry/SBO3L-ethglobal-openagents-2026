# `examples/langchain-py-research-agent`

End-to-end LangChain Python research agent that asks SBO3L to authorize every payment-shaped action. Reasons across 2 tools (`data_fetch` + `sbo3l_payment_request`) and routes allowed payments through the live KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

Mirror of `examples/langchain-ts-research-agent` in Python.

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/langchain-py-research-agent && python3 -m venv .venv && .venv/bin/pip install -e ../../sdks/python -e ../../integrations/langchain-python -e .
.venv/bin/python -m sbo3l_langchain_demo.smoke   # no OpenAI / no langchain needed
```

## With LangChain + an LLM

```bash
.venv/bin/pip install -e ".[langchain]"
export OPENAI_API_KEY=sk-...
.venv/bin/python -m sbo3l_langchain_demo.agent
```

LangChain's `create_openai_functions_agent` picks the tool sequence: it inspects the provider via `data_fetch`, then submits an APRP through `sbo3l_payment_request`. SBO3L decides; on allow the daemon's KH adapter routes to KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

## Tools

| Tool | Description |
|---|---|
| `data_fetch` | GET a JSON URL, return body. The agent uses it to inspect a provider before paying. |
| `sbo3l_payment_request` | Submit an APRP via `sbo3l-langchain` → SBO3L policy boundary → KH adapter. |

## Expected smoke output

```
▶ smoke: KH workflow target = m4t4cnpmhv8qquce3bv3c

▶ tool: data_fetch (GitHub status — public, low-noise)
  ✓ HTTP 200

▶ tool: sbo3l_payment_request (APRP → SBO3L → KH adapter)
  envelope:
    decision: "allow"
    execution_ref: "kh-..."
    audit_event_id: "evt-..."
    ...

✓ allow — execution_ref kh-...
  audit_event_id: evt-...
```

Total wall-clock: < 30 s.

## Tests

```bash
.venv/bin/pip install pytest pytest-httpx
.venv/bin/pytest -q
```

2 tests verify the SBO3L tool path against a mocked-httpx daemon (real `sbo3l_sdk.SBO3LClientSync`).

## License

MIT
