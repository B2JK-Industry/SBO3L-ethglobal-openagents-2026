"""sbo3l-langchain-keeperhub — LangChain Python tool that gates KeeperHub
workflow execution through SBO3L's policy boundary.

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

Either as a plain descriptor (no LangChain dep — same shape as
`sbo3l-langchain`):

    from sbo3l_sdk import SBO3LClientSync
    from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool

    client = SBO3LClientSync("http://localhost:8730")
    descriptor = sbo3l_keeperhub_tool(client=client)

Or as a typed langchain.tools.BaseTool subclass (when langchain-core is
a runtime dep of your agent — we don't pull it in):

    from sbo3l_langchain_keeperhub import Sbo3lKeeperHubTool
    from sbo3l_sdk import SBO3LClientSync

    client = SBO3LClientSync("http://localhost:8730")
    tool = Sbo3lKeeperHubTool(client=client)
    # tool is a real BaseTool instance — pass into your AgentExecutor
"""

from __future__ import annotations

from ._version import __version__
from .tool import (
    DEFAULT_KH_WORKFLOW_ID,
    SBO3LClientLike,
    SBO3LKeeperHubSubmitResult,
    SBO3LKeeperHubToolDescriptor,
    sbo3l_keeperhub_tool,
)

# Sbo3lKeeperHubTool requires langchain-core; lazy-import so importing
# this package without the optional dep doesn't error.
try:
    from .langchain_tool import Sbo3lKeeperHubTool

    __all__ = [
        "DEFAULT_KH_WORKFLOW_ID",
        "SBO3LClientLike",
        "SBO3LKeeperHubSubmitResult",
        "SBO3LKeeperHubToolDescriptor",
        "Sbo3lKeeperHubTool",
        "__version__",
        "sbo3l_keeperhub_tool",
    ]
except ImportError:  # langchain-core not installed
    __all__ = [
        "DEFAULT_KH_WORKFLOW_ID",
        "SBO3LClientLike",
        "SBO3LKeeperHubSubmitResult",
        "SBO3LKeeperHubToolDescriptor",
        "__version__",
        "sbo3l_keeperhub_tool",
    ]
