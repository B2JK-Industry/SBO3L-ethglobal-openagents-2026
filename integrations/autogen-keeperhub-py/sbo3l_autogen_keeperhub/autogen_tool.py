"""AutoGen-specific helper to register the SBO3L → KeeperHub tool with a
ConversableAgent (legacy `pyautogen` 0.2.x line) OR with an
AssistantAgent (modern `autogen-agentchat` 0.4.x+ line).

Drops directly into either Microsoft AutoGen distribution. Requires
`autogen-agentchat>=0.4` as the modern runtime dep — install via
`pip install sbo3l-autogen-keeperhub[autogen]`. The legacy 0.2.x
`pyautogen` ConversableAgent surface is also supported when present
because we duck-type against the agent rather than importing the class.

For framework-agnostic use, prefer the plain descriptor returned by
`sbo3l_autogen_keeperhub_tool()` from `tool.py`.
"""

from __future__ import annotations

import asyncio
from typing import Any, Protocol, runtime_checkable

from .tool import (
    SBO3LClientLike,
    SBO3LKeeperHubToolDescriptor,
    sbo3l_autogen_keeperhub_tool,
)


@runtime_checkable
class _LegacyConversableAgentLike(Protocol):
    """Legacy 0.2.x `autogen.ConversableAgent` surface.

    `register_function(function_map={name: callable})` registers a tool
    that the agent can dispatch synchronously inside its conversation
    loop. We duck-type rather than import to keep the helper working
    with both pyautogen<0.3 and any drop-in mock.
    """

    def register_function(self, function_map: dict[str, Any]) -> None: ...


def register_sbo3l_keeperhub_tool(
    agent: Any,
    *,
    client: SBO3LClientLike,
    workflow_id: str | None = None,
    name: str | None = None,
    description: str | None = None,
) -> SBO3LKeeperHubToolDescriptor:
    """Register the SBO3L → KeeperHub tool with an AutoGen agent.

    Wraps the descriptor's sync callable in an async-safe shim that
    offloads the blocking SBO3L HTTP call to a worker thread when the
    helper is invoked from inside a running event loop. AutoGen's
    legacy `initiate_chat` runs synchronously by default, but
    `a_initiate_chat` and the agentchat 0.4.x runtime both drive
    function calls on the main loop — calling `descriptor.func`
    directly there would stall every other concurrent agent in the
    graph.

    Dispatches against two agent shapes (in order):

      1. **Legacy** `ConversableAgent.register_function(function_map=...)`
         (pyautogen 0.2.x). Registers under the descriptor's name.
      2. **Modern** `AssistantAgent` with mutable `tools` constructor
         arg (autogen-agentchat 0.4.x+). Appends the callable to the
         agent's existing tools list, OR (if the attribute is missing)
         walks `_workbench`/`_tools` private attrs as a last resort.

    For the modern shape, prefer constructing the agent with
    `tools=[descriptor.func]` directly — this helper is a convenience
    for the legacy line and for re-binding tools post-construction.

    Returns the descriptor so callers can introspect the registered
    name + description (e.g. to wire a matching `register_for_llm`
    decorator on a sibling agent in the legacy line).

    Example (legacy 0.2.x):

        from autogen import ConversableAgent
        from sbo3l_sdk import SBO3LClientSync
        from sbo3l_autogen_keeperhub import register_sbo3l_keeperhub_tool

        client = SBO3LClientSync("http://localhost:8730")
        executor = ConversableAgent(name="executor", llm_config=False)
        register_sbo3l_keeperhub_tool(executor, client=client)

    Example (modern 0.4.x+):

        from autogen_agentchat.agents import AssistantAgent
        from sbo3l_sdk import SBO3LClientSync
        from sbo3l_autogen_keeperhub import (
            register_sbo3l_keeperhub_tool,
            sbo3l_autogen_keeperhub_tool,
        )

        client = SBO3LClientSync("http://localhost:8730")
        descriptor = sbo3l_autogen_keeperhub_tool(client=client)
        # Preferred: pass the callable directly to the constructor.
        executor = AssistantAgent(
            "executor", model_client=..., tools=[descriptor.func]
        )
    """

    descriptor_kwargs: dict[str, Any] = {}
    if name is not None:
        descriptor_kwargs["name"] = name
    if description is not None:
        descriptor_kwargs["description"] = description

    descriptor = sbo3l_autogen_keeperhub_tool(
        client=client,
        workflow_id=workflow_id,
        **descriptor_kwargs,
    )

    sync_func = descriptor.func

    async def _async_wrapper(aprp_json: str) -> str:
        """Async path: offload sync HTTP to a thread."""
        return await asyncio.to_thread(sync_func, aprp_json)

    # Probe whether the agent prefers async or sync registration. If
    # we're inside a running event loop, register the async wrapper
    # (autogen-agentchat 0.4.x dispatches tools on the main loop). If
    # not, hand back the plain sync callable (legacy 0.2.x dispatches
    # in a sync conversation loop).
    try:
        asyncio.get_running_loop()
        registered: Any = _async_wrapper
    except RuntimeError:
        registered = sync_func

    if isinstance(agent, _LegacyConversableAgentLike):
        # Legacy line: pyautogen ConversableAgent.register_function.
        agent.register_function(function_map={descriptor.name: registered})
        return descriptor

    # Modern line: agentchat 0.4.x AssistantAgent. Tools live as a list
    # on the agent post-construction; the public field is private but
    # stable across the 0.4-0.7 series. Append the callable.
    tools_attr = getattr(agent, "_tools", None)
    if isinstance(tools_attr, list):
        tools_attr.append(registered)
        return descriptor

    raise TypeError(
        f"Unsupported AutoGen agent surface: {type(agent).__name__}. "
        "Expected either a legacy ConversableAgent (with .register_function) "
        "or a modern AssistantAgent (with mutable ._tools). For the modern "
        "line, prefer passing the descriptor.func directly to the agent's "
        "tools=[...] constructor argument."
    )


def aprp_function_signature() -> dict[str, Any]:
    """Return the OpenAI-function-calling JSON schema for the SBO3L tool.

    Useful for the legacy AutoGen line's `register_for_llm` decorator
    pattern, which wants a name + description + JSON-schema parameters
    block that the LLM sees. Hands back the canonical schema — input is
    a JSON-stringified APRP — so callers that prefer the decorator
    pattern over `register_function` get a consistent surface.
    """

    return {
        "name": "sbo3l_keeperhub_payment_request",
        "description": (
            "Submit an Agent Payment Request Protocol (APRP) JSON object to SBO3L "
            "for policy decision. On allow, the SBO3L daemon's KeeperHub adapter "
            "executes the payment by POSTing the IP-1 envelope to a KeeperHub "
            "workflow webhook and returns the captured executionId as "
            "kh_execution_ref. On deny, branch on deny_code to self-correct or "
            "escalate."
        ),
        "parameters": {
            "type": "object",
            "required": ["aprp_json"],
            "properties": {
                "aprp_json": {
                    "type": "string",
                    "description": (
                        "JSON-stringified APRP object. Required fields: "
                        "agent_id, task_id, intent, amount, token, destination, "
                        "payment_protocol, chain, provider_url, expiry, nonce, "
                        "risk_class. See https://sbo3l.dev/aprp for the schema."
                    ),
                }
            },
        },
    }


__all__ = [
    "aprp_function_signature",
    "register_sbo3l_keeperhub_tool",
]
