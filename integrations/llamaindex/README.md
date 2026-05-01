# `sbo3l-llamaindex`

LlamaIndex tool wrapping SBO3L. Drop into a LlamaIndex agent's tool list to gate every payment intent through SBO3L.

> ⚠ **DRAFT (T-1-6):** depends on F-10 (`sbo3l-sdk`).

## Install

```bash
pip install sbo3l-llamaindex sbo3l-sdk llama-index-core
```

## Quick start

```python
from llama_index.core.tools import FunctionTool
from sbo3l_sdk import SBO3LClientSync, bearer
from sbo3l_llamaindex import sbo3l_tool

client = SBO3LClientSync("http://localhost:8730", auth=bearer("my-token"))
descriptor = sbo3l_tool(client=client)
tool = FunctionTool.from_defaults(
    fn=descriptor.func,
    name=descriptor.name,
    description=descriptor.description,
)
# Pass `tool` into a LlamaIndex agent's tool list (e.g. ReActAgent.from_tools).
```

## Decision envelope

```json
{
  "decision": "allow",
  "deny_code": null,
  "execution_ref": "kh-...",
  "audit_event_id": "evt-...",
  ...
}
```

On `deny`, the agent sees `deny_code` (e.g. `policy.budget_exceeded`).

## License

MIT
