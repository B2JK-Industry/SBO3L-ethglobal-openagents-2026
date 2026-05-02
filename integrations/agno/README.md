# `sbo3l-agno`

[Agno](https://docs.agno.com) (formerly Phidata) adapter for SBO3L.

## Install

```bash
pip install sbo3l-agno
# or, with the optional Agno runtime:
pip install "sbo3l-agno[agno]"
```

## Wire into an Agno Toolkit

```python
from agno.agent import Agent
from agno.tools.toolkit import Toolkit
from sbo3l_sdk import SBO3LClientSync
from sbo3l_agno import sbo3l_payment_request_func

client = SBO3LClientSync("http://localhost:8730")

class SBO3LToolkit(Toolkit):
    def __init__(self):
        super().__init__(name="sbo3l")
        descriptor = sbo3l_payment_request_func(client=client)
        self.register(descriptor.func, name=descriptor.name, description=descriptor.description)

agent = Agent(model=..., tools=[SBO3LToolkit()])
```

## Behaviour

- The tool's input is a JSON-stringified APRP v1 object.
- On `allow` the output is a JSON envelope with `decision`, `audit_event_id`, `execution_ref`, `request_hash`, `policy_hash`.
- On `deny` / `requires_human` the output is a JSON envelope with `error: "policy.deny"`, `deny_code`, `audit_event_id` — the tool **never raises**, so Agno's function-call loop continues and the LLM can self-correct.
- On bad input (malformed JSON, non-object) the output is `{"error": "input.bad_arguments", "detail": ...}`.
- On transport failure the output is `{"error": "transport.failed", "detail": ...}` (or the SDK's domain code if available).

## Testing

```bash
pip install -e ".[dev]"
pytest -q
```

11 tests cover descriptor shape, allow / deny envelopes, bad-input branches, idempotency-key forwarding, and the Pydantic ↔ dict coercion helper.
