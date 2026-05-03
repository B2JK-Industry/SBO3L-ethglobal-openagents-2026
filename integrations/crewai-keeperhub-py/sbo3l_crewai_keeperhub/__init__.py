"""sbo3l-crewai-keeperhub — CrewAI Python tool that gates KeeperHub
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

Either as a plain descriptor (no CrewAI dep — same shape as the
sibling SBO3L framework adapters):

    from sbo3l_sdk import SBO3LClientSync
    from sbo3l_crewai_keeperhub import sbo3l_crewai_keeperhub_tool

    client = SBO3LClientSync("http://localhost:8730")
    descriptor = sbo3l_crewai_keeperhub_tool(client=client)

Or as a typed crewai.tools.BaseTool subclass (when crewai is a runtime
dep of your agent — we don't pull it in):

    from sbo3l_crewai_keeperhub import Sbo3lKeeperHubCrewAITool
    from sbo3l_sdk import SBO3LClientSync

    client = SBO3LClientSync("http://localhost:8730")
    tool = Sbo3lKeeperHubCrewAITool(client=client)
    # tool is a real BaseTool instance — pass into your CrewAI Agent
"""

from __future__ import annotations

from ._version import __version__
from .tool import (
    DEFAULT_KH_WORKFLOW_ID,
    SBO3LClientLike,
    SBO3LKeeperHubSubmitResult,
    SBO3LKeeperHubToolDescriptor,
    sbo3l_crewai_keeperhub_tool,
)

# Sbo3lKeeperHubCrewAITool requires crewai; lazy-import so importing
# this package without the optional dep doesn't error.
try:
    from .crewai_tool import Sbo3lKeeperHubCrewAITool

    __all__ = [
        "DEFAULT_KH_WORKFLOW_ID",
        "SBO3LClientLike",
        "SBO3LKeeperHubSubmitResult",
        "SBO3LKeeperHubToolDescriptor",
        "Sbo3lKeeperHubCrewAITool",
        "__version__",
        "sbo3l_crewai_keeperhub_tool",
    ]
except ImportError:  # crewai not installed
    __all__ = [
        "DEFAULT_KH_WORKFLOW_ID",
        "SBO3LClientLike",
        "SBO3LKeeperHubSubmitResult",
        "SBO3LKeeperHubToolDescriptor",
        "__version__",
        "sbo3l_crewai_keeperhub_tool",
    ]
