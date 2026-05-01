"""sbo3l-crewai — CrewAI tool wrapping SBO3L.

Drop a `sbo3l_tool(client=...)` into a CrewAI Agent's tool list to gate
every payment intent through SBO3L's policy boundary. The integration
uses structural typing — install `sbo3l-sdk` separately as an optional
extra (`pip install sbo3l-crewai[sdk]`).
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
