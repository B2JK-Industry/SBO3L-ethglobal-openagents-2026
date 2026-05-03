"""sbo3l-autogen-keeperhub — Microsoft AutoGen Python tool that gates
KeeperHub workflow execution through SBO3L's policy boundary.

# Why this exists alongside `langchain-keeperhub` (Devendra's PyPI pkg)

Devendra's `langchain-keeperhub` wraps **execution** — agent → KH webhook
→ result. Our package gates execution **upstream**: agent → SBO3L
(policy + budget + audit + signed receipt) → if allow → KH webhook →
result. The two are **composable**: a developer can use Devendra's tool
for the raw KH binding and ours as the policy gate that decides whether
the raw call should fire at all. Or use ours alone for the full
gate-then-execute path.

# The wire path

  1. Tool input: JSON-stringified APRP (Agent Payment Request Protocol).
  2. POST to SBO3L daemon's /v1/payment-requests.
  3. SBO3L decides allow / deny / requires_human against the loaded
     policy + budget + nonce + provider trust list.
  4. On allow: SBO3L's executor_callback hands the signed PolicyReceipt
     to the daemon-side KeeperHub adapter (configured via
     SBO3L_KEEPERHUB_WEBHOOK_URL + SBO3L_KEEPERHUB_TOKEN env vars on the
     daemon process — NOT on the agent).
  5. KH adapter POSTs the IP-1 envelope to the workflow webhook,
     captures executionId, surfaces it as receipt.execution_ref.
  6. Tool returns:
       {decision, kh_workflow_id_advisory, kh_execution_ref,
        audit_event_id, request_hash, policy_hash, deny_code}.

# Two ways to consume

Either as a plain descriptor (no AutoGen dep — same shape as
`sbo3l-langchain-keeperhub`):

    from sbo3l_sdk import SBO3LClientSync
    from sbo3l_autogen_keeperhub import sbo3l_autogen_keeperhub_tool

    client = SBO3LClientSync("http://localhost:8730")
    descriptor = sbo3l_autogen_keeperhub_tool(client=client)

Or registered directly with an AutoGen `ConversableAgent` (when
`pyautogen` is installed — we don't pull it in):

    from autogen import ConversableAgent
    from sbo3l_sdk import SBO3LClientSync
    from sbo3l_autogen_keeperhub import register_sbo3l_keeperhub_tool

    client = SBO3LClientSync("http://localhost:8730")
    executor = ConversableAgent(name="executor", llm_config=False)
    register_sbo3l_keeperhub_tool(executor, client=client)
"""

from __future__ import annotations

from ._version import __version__
from .tool import (
    DEFAULT_KH_WORKFLOW_ID,
    SBO3LClientLike,
    SBO3LKeeperHubSubmitResult,
    SBO3LKeeperHubToolDescriptor,
    sbo3l_autogen_keeperhub_tool,
)

# register_sbo3l_keeperhub_tool is duck-typed against the agent and
# carries no hard import on AutoGen — the wrapper itself loads
# unconditionally. We still gate the public re-export on the AutoGen
# distribution being importable (either modern `autogen-agentchat` /
# `autogen_agentchat` or the legacy `autogen` / `pyautogen<0.3` line)
# so that `from sbo3l_autogen_keeperhub import *` mirrors the
# langchain-keeperhub pattern: typed framework helpers only surface
# when the framework is on path.
try:
    import autogen_agentchat  # noqa: F401  (modern line: autogen-agentchat)
except ImportError:
    try:
        import autogen  # noqa: F401  (legacy line: pyautogen<0.3)
    except ImportError:
        _AUTOGEN_AVAILABLE = False
    else:
        _AUTOGEN_AVAILABLE = True
else:
    _AUTOGEN_AVAILABLE = True

if _AUTOGEN_AVAILABLE:
    from .autogen_tool import register_sbo3l_keeperhub_tool

    __all__ = [
        "DEFAULT_KH_WORKFLOW_ID",
        "SBO3LClientLike",
        "SBO3LKeeperHubSubmitResult",
        "SBO3LKeeperHubToolDescriptor",
        "__version__",
        "register_sbo3l_keeperhub_tool",
        "sbo3l_autogen_keeperhub_tool",
    ]
else:
    __all__ = [
        "DEFAULT_KH_WORKFLOW_ID",
        "SBO3LClientLike",
        "SBO3LKeeperHubSubmitResult",
        "SBO3LKeeperHubToolDescriptor",
        "__version__",
        "sbo3l_autogen_keeperhub_tool",
    ]
