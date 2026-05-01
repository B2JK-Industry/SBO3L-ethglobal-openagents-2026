"""sbo3l-langchain — LangChain Python tool wrapping SBO3L.

Drop a `sbo3l_tool(client=...)` into your LangChain agent's tool list to
gate every payment intent through SBO3L's policy boundary.

The integration uses **structural typing** for the SBO3L client: any
object with a `submit(request, *, idempotency_key=None)` method matching
`SBO3LClientLike` works. Install `sbo3l-sdk` separately as an optional
extra to get the canonical client.
"""

from __future__ import annotations

from ._version import __version__
from .tool import (
    SBO3LClientLike,
    SBO3LSubmitResult,
    SBO3LToolDescriptor,
    sbo3l_tool,
)

__all__ = [
    "__version__",
    "sbo3l_tool",
    "SBO3LClientLike",
    "SBO3LSubmitResult",
    "SBO3LToolDescriptor",
]
