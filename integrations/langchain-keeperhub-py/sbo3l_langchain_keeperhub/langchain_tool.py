"""Typed `langchain.tools.BaseTool` subclass for the SBO3L → KeeperHub path.

Drops directly into a LangChain `AgentExecutor`. Requires `langchain-core`
as a runtime dep — install via `pip install sbo3l-langchain-keeperhub[langchain]`.

For framework-agnostic use, prefer the plain descriptor returned by
`sbo3l_keeperhub_tool()` from `tool.py`.
"""

from __future__ import annotations

from typing import Any

from langchain_core.tools import BaseTool
from pydantic import BaseModel, ConfigDict, Field, PrivateAttr

from .tool import (
    DEFAULT_KH_WORKFLOW_ID,
    SBO3LClientLike,
    sbo3l_keeperhub_tool,
)


class _Sbo3lKeeperHubInput(BaseModel):
    """Schema LangChain shows the LLM for tool args."""

    aprp_json: str = Field(
        ...,
        description=(
            "JSON-stringified APRP (Agent Payment Request Protocol) object. "
            "Required fields: agent_id, task_id, intent, amount, token, "
            "destination, payment_protocol, chain, provider_url, expiry, "
            "nonce, risk_class. See https://sbo3l.dev/aprp for the schema."
        ),
    )


class Sbo3lKeeperHubTool(BaseTool):  # type: ignore[misc, unused-ignore]
    """LangChain BaseTool that gates KeeperHub execution through SBO3L.

    Construct with an SBO3L client (`SBO3LClientSync` recommended for
    sync agent loops). The tool's `_run` accepts a JSON-stringified APRP
    and returns a JSON envelope with `kh_execution_ref` populated when
    the policy decision is `allow`.

    Example:

        from sbo3l_sdk import SBO3LClientSync
        from sbo3l_langchain_keeperhub import Sbo3lKeeperHubTool

        client = SBO3LClientSync("http://localhost:8730")
        tool = Sbo3lKeeperHubTool(client=client)
        # pass `tool` into your AgentExecutor's tool list
    """

    name: str = "sbo3l_keeperhub_payment_request"
    description: str = (
        "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L "
        "for policy decision. On allow, the SBO3L daemon's KeeperHub adapter "
        "executes the payment by POSTing the IP-1 envelope to a KeeperHub "
        "workflow webhook and returns the captured executionId as "
        "kh_execution_ref. Returns a JSON envelope; on deny, branch on "
        "deny_code to self-correct or escalate."
    )
    args_schema: type[BaseModel] = _Sbo3lKeeperHubInput

    model_config = ConfigDict(arbitrary_types_allowed=True)

    _func: Any = PrivateAttr()

    def __init__(
        self,
        *,
        client: SBO3LClientLike,
        workflow_id: str | None = None,
        name: str | None = None,
        description: str | None = None,
        **kwargs: Any,
    ) -> None:
        if name is not None:
            kwargs["name"] = name
        if description is not None:
            kwargs["description"] = description
        super().__init__(**kwargs)
        descriptor = sbo3l_keeperhub_tool(
            client=client,
            workflow_id=workflow_id or DEFAULT_KH_WORKFLOW_ID,
        )
        self._func = descriptor.func

    def _run(self, aprp_json: str, **_: Any) -> str:
        return str(self._func(aprp_json))

    async def _arun(self, aprp_json: str, **_: Any) -> str:
        # The underlying SBO3LClientSync.submit performs blocking httpx
        # I/O. Calling self._func directly from _arun would stall the
        # event loop and starve other async tools running concurrently
        # in the same agent. asyncio.to_thread offloads the call to the
        # default thread pool — same semantics as LangChain BaseTool's
        # default _arun fallback when this method isn't overridden, but
        # explicit so future maintainers don't accidentally re-introduce
        # the blocking version.
        import asyncio

        return await asyncio.to_thread(self._func, aprp_json)
