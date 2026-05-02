"""sbo3l-pydantic-ai — Pydantic AI adapter for SBO3L.

Pydantic AI's `Agent.tool` decorator expects a callable whose parameters
are Pydantic-typed. This adapter exposes a builder that returns a
descriptor + the typed AprpInput model that consumers wire into their
agent.

Typical wiring:

    from pydantic_ai import Agent
    from sbo3l_sdk import SBO3LClientSync
    from sbo3l_pydantic_ai import sbo3l_payment_request_func, AprpInput

    client = SBO3LClientSync("http://localhost:8730")
    descriptor = sbo3l_payment_request_func(client=client)

    agent = Agent("openai:gpt-4o-mini")

    @agent.tool_plain
    def sbo3l_payment_request(aprp: AprpInput) -> str:
        return descriptor.func(aprp.model_dump_json())

The Pydantic-validated `AprpInput` catches malformed model output BEFORE
the daemon round-trip — the same local-validation win @sbo3l/anthropic
gets via zod.
"""

from __future__ import annotations

from .tool import (
    AprpAmount,
    AprpDestination,
    AprpInput,
    PolicyDenyError,
    SBO3LClientLike,
    SBO3LToolDescriptor,
    sbo3l_payment_request_func,
)

__all__ = [
    "AprpAmount",
    "AprpDestination",
    "AprpInput",
    "PolicyDenyError",
    "SBO3LClientLike",
    "SBO3LToolDescriptor",
    "sbo3l_payment_request_func",
]

__version__ = "1.2.0"
