# `sbo3l-langchain`

LangChain Python tool wrapping SBO3L. Mirror of `@sbo3l/langchain` (TypeScript).

> ⚠ **DRAFT (T-1-2):** depends on F-10 (`sbo3l-sdk`) merging + publishing to PyPI.

## Install

```bash
pip install sbo3l-langchain sbo3l-sdk langchain-core
```

## Quick start

```python
from langchain_core.tools import StructuredTool
from sbo3l_sdk import SBO3LClientSync, bearer
from sbo3l_langchain import sbo3l_tool

client = SBO3LClientSync(
    "http://localhost:8730",
    auth=bearer("my-bearer-token"),
)
descriptor = sbo3l_tool(client=client)
tool = StructuredTool.from_function(
    func=descriptor.func,
    name=descriptor.name,
    description=descriptor.description,
)
# Pass `tool` into your LangChain agent's tool list.
```

## What it does

The agent emits a tool call with a JSON-stringified APRP. The tool forwards
to `client.submit()` and returns a JSON envelope:

```json
{
  "decision": "allow",
  "deny_code": null,
  "matched_rule_id": "allow-low-risk-x402",
  "execution_ref": "kh-...",
  "audit_event_id": "evt-...",
  "request_hash": "...",
  "policy_hash": "..."
}
```

On `deny`, the LLM sees `deny_code` (`policy.budget_exceeded`,
`policy.token_unsupported`, etc.) and can self-correct or escalate.

## Idempotency

```python
descriptor = sbo3l_tool(
    client=client,
    idempotency_key=lambda body: f"{body['task_id']}-{body['nonce']}",
)
```

## Async clients

This factory returns a sync callback. Use `SBO3LClientSync` (httpx.Client
under the hood — safe inside any event loop). If you must use the async
client, create your own async tool that awaits `client.submit(...)`.

## Errors

Transport / auth failures surface as a JSON envelope with `error` (RFC 7807
domain code, e.g. `auth.required`) and `status`.

## License

MIT
