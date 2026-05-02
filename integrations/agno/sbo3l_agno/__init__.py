"""sbo3l-agno — Agno (formerly Phidata) adapter for SBO3L.

Agno's `Toolkit` API expects callables decorated with metadata. This
adapter exposes a builder that returns a tuple `(callable, metadata)`
that consumers wire into their `Toolkit` subclass — keeps Agno itself
optional so the integration can be tested without `pip install agno`.

Typical wiring:

    from agno.agent import Agent
    from agno.tools.toolkit import Toolkit
    from sbo3l_sdk import SBO3LClientSync
    from sbo3l_agno import sbo3l_payment_request_func

    client = SBO3LClientSync("http://localhost:8730")

    class SBO3LToolkit(Toolkit):
        def __init__(self):
            super().__init__(name="sbo3l")
            self.register(sbo3l_payment_request_func(client=client))

    agent = Agent(model=OpenAIChat(id="gpt-4o"), tools=[SBO3LToolkit()])
"""

from __future__ import annotations

from .tool import (
    PolicyDenyError,
    SBO3LClientLike,
    SBO3LToolDescriptor,
    sbo3l_payment_request_func,
)

__all__ = [
    "PolicyDenyError",
    "SBO3LClientLike",
    "SBO3LToolDescriptor",
    "sbo3l_payment_request_func",
]

__version__ = "1.2.0"
