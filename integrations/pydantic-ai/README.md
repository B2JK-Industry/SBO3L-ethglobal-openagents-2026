# `sbo3l-pydantic-ai`

Pydantic AI adapter for SBO3L. Wraps a Pydantic-typed APRP submit as a tool that plugs directly into a `pydantic_ai.Agent`.

## Install

```bash
pip install sbo3l-pydantic-ai
# or, with the optional Pydantic AI runtime:
pip install "sbo3l-pydantic-ai[pydantic-ai]"
```

## Wire into a Pydantic AI Agent

```python
from pydantic_ai import Agent
from sbo3l_sdk import SBO3LClientSync
from sbo3l_pydantic_ai import sbo3l_payment_request_func, AprpInput

client = SBO3LClientSync("http://localhost:8730")
descriptor = sbo3l_payment_request_func(client=client)

agent = Agent("openai:gpt-4o-mini")

@agent.tool_plain
def sbo3l_payment_request(aprp: AprpInput) -> str:
    return descriptor.func(aprp.model_dump_json())

result = agent.run_sync("Pay 0.05 USDC for an inference call against api.example.com.")
print(result.output)
```

## Local-first validation

The headline win for Pydantic AI: `AprpInput` runs Pydantic validation BEFORE the daemon round-trip. A wrong `intent` enum or malformed `agent_id` regex surfaces as `{"error": "input.bad_arguments", "detail": ...}` without a network hit — same pattern `@sbo3l/anthropic` gets via zod.

## Behaviour

- **allow** → `{"decision": "allow", "audit_event_id": "evt-...", "execution_ref": "kh-...", ...}`
- **deny** / **requires_human** → `{"error": "policy.deny", "decision": "deny", "deny_code": "...", "audit_event_id": "...", ...}` (NO raise — agent loop continues)
- **bad input** → `{"error": "input.bad_arguments", "detail": "<pydantic validation error>"}`
- **transport fail** → `{"error": "transport.failed", "detail": "..."}` (or the SDK's domain code if available)

## Tests

```bash
pip install -e ".[dev]"
pytest -q
mypy --strict sbo3l_pydantic_ai
ruff check .
```

19 pytest tests cover Pydantic input validation, descriptor shape, allow + deny + requires_human envelopes, local-validation-before-network guarantee, idempotency-key forwarding, transport-failure code preservation, and the Pydantic ↔ dict coercion helper.
