# `sbo3l-crewai`

CrewAI tool wrapping SBO3L. Drop into a CrewAI Agent's tool list to gate every payment intent through SBO3L's policy boundary.

> ⚠ **DRAFT (T-1-3):** depends on F-10 (`sbo3l-sdk`) merging + publishing.

## Install

```bash
pip install sbo3l-crewai sbo3l-sdk crewai-tools
```

## Quick start

```python
from crewai_tools import BaseTool
from sbo3l_sdk import SBO3LClientSync, bearer
from sbo3l_crewai import sbo3l_tool

client = SBO3LClientSync("http://localhost:8730", auth=bearer("my-token"))
descriptor = sbo3l_tool(client=client)

class SBO3LPaymentTool(BaseTool):
    name: str = descriptor.name
    description: str = descriptor.description

    def _run(self, aprp_json: str) -> str:
        return descriptor.func(aprp_json)
```

Pass `SBO3LPaymentTool()` into a CrewAI Agent's `tools=[...]`.

## What it does

The CrewAI agent emits a tool call with a JSON-stringified APRP. Returns:

```json
{
  "decision": "allow",
  "deny_code": null,
  "execution_ref": "kh-...",
  "audit_event_id": "evt-...",
  ...
}
```

On `deny`, the agent sees `deny_code` and can self-correct or escalate.

## License

MIT
