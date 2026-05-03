# `sbo3l-crewai-keeperhub`

> CrewAI Python tool that **gates KeeperHub workflow execution through SBO3L's policy boundary**. Composable with `sbo3l-sdk`.

## Why this exists alongside `langchain-keeperhub` (Devendra's PyPI pkg)

| | `langchain-keeperhub` (Devendra) | `sbo3l-crewai-keeperhub` (this) |
|---|---|---|
| Framework | LangChain | CrewAI |
| What it wraps | KH webhook execution | SBO3L policy gate → KH webhook execution |
| Decision step | (agent decides) | (SBO3L decides; signed receipt) |
| Budget enforcement | no | yes |
| Audit chain | no | yes (hash-chained Ed25519 log) |
| Multi-agent crew composition | n/a | one policy boundary across all crew agents |
| ENS / Turnkey TEE / MCP bridge | yes | no (not duplicated) |

**Composable:** use Devendra's tool for the raw KH binding + ours as the policy gate that decides whether the raw call should fire. Or use ours alone for the full gate-then-execute path. The CrewAI angle adds: **one SBO3L policy boundary shared across N specialized crew agents**, with a single hash-chained audit log spanning every agent's KH executions.

## Install

```bash
pip install "sbo3l-crewai-keeperhub[crewai]" sbo3l-sdk
```

The `[crewai]` extra pulls in `crewai` so the typed `Sbo3lKeeperHubCrewAITool` (a `crewai.tools.BaseTool` subclass) is importable. Without it, only the framework-agnostic `sbo3l_crewai_keeperhub_tool()` factory is available.

## 5-line setup

```python
from sbo3l_sdk import SBO3LClientSync
from sbo3l_crewai_keeperhub import Sbo3lKeeperHubCrewAITool

client = SBO3LClientSync("http://localhost:8730")
tool = Sbo3lKeeperHubCrewAITool(client=client)
# pass `tool` into your CrewAI Agent's tools=[...] list
```

Or framework-agnostic:

```python
from sbo3l_crewai_keeperhub import sbo3l_crewai_keeperhub_tool
descriptor = sbo3l_crewai_keeperhub_tool(client=client)
# descriptor.name + descriptor.description + descriptor.func
```

## Wire path

Same as the sibling LangChain / TS packages — SBO3L decides, KH adapter executes on allow, tool returns `kh_execution_ref`. See the LangChain README's "Wire path" section for the full sequence. All packages share the daemon as the policy boundary, so `kh_execution_ref` matches across SDK languages.

## Multi-agent crew composition

CrewAI's strength is N specialized agents collaborating on a shared task. Wire **one** `Sbo3lKeeperHubCrewAITool` instance into every agent's tool list and you get:

- One policy boundary for the whole crew
- One KH workflow_id (advisory) tagged across every execution
- One hash-chained audit log spanning researcher → executor → auditor agents

See `examples/crewai-keeperhub-demo/agent.py` for a runnable 3-agent crew using this pattern.

## On `kh_workflow_id_advisory`

The `_advisory` suffix is intentional: today the daemon's env-configured webhook URL is the source of truth for actual routing. The per-call `workflow_id` you pass is surfaced in the envelope for **context tagging** / audit logs, not as a routing override. See [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) for the proposed contract that would make per-call routing safe.

## API

```python
sbo3l_crewai_keeperhub_tool(
    *,
    client: SBO3LClientLike,
    workflow_id: str | None = None,           # default: DEFAULT_KH_WORKFLOW_ID
    name: str = "sbo3l_keeperhub_payment_request",
    description: str = ...,
    idempotency_key: Callable[[dict], str] | None = None,
) -> SBO3LKeeperHubToolDescriptor

# Or, with crewai installed:
Sbo3lKeeperHubCrewAITool(
    client=...,                # required, kw-only
    workflow_id=None,          # default: DEFAULT_KH_WORKFLOW_ID
    name=None, description=None,
)
# Real crewai.tools.BaseTool subclass — drops into Agent(tools=[...]).
```

## License

MIT
