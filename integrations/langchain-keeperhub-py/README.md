# `sbo3l-langchain-keeperhub`

> LangChain Python tool that **gates KeeperHub workflow execution through SBO3L's policy boundary**. Composable with `sbo3l-sdk`.

## Why this exists alongside `langchain-keeperhub` (Devendra's PyPI pkg)

| | `langchain-keeperhub` (Devendra) | `sbo3l-langchain-keeperhub` (this) |
|---|---|---|
| What it wraps | KH webhook execution | SBO3L policy gate → KH webhook execution |
| Decision step | ✗ (agent decides) | ✓ (SBO3L decides; signed receipt) |
| Budget enforcement | ✗ | ✓ |
| Audit chain | ✗ | ✓ (hash-chained Ed25519 log) |
| ENS / Turnkey TEE / MCP bridge | ✓ | ✗ (not duplicated) |

**Composable:** use Devendra's tool for the raw KH binding + ours as the policy gate that decides whether the raw call should fire. Or use ours alone for the full gate-then-execute path.

## Install

```bash
pip install "sbo3l-langchain-keeperhub[langchain]" sbo3l-sdk
```

The `[langchain]` extra pulls in `langchain-core` so the typed `Sbo3lKeeperHubTool` (a `BaseTool` subclass) is importable. Without it, only the framework-agnostic `sbo3l_keeperhub_tool()` factory is available.

## 5-line setup

```python
from sbo3l_sdk import SBO3LClientSync
from sbo3l_langchain_keeperhub import Sbo3lKeeperHubTool

client = SBO3LClientSync("http://localhost:8730")
tool = Sbo3lKeeperHubTool(client=client)
# pass `tool` into your AgentExecutor's tool list
```

Or framework-agnostic:

```python
from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool
descriptor = sbo3l_keeperhub_tool(client=client)
# descriptor.name + descriptor.description + descriptor.func
```

## Wire path

Same as the TS package — SBO3L decides → KH adapter executes on allow → tool returns `kh_execution_ref`. See the TS README's "Wire path" section for details. Both packages share the daemon as the policy boundary, so `kh_execution_ref` matches across SDK languages.

## On `kh_workflow_id_advisory`

The `_advisory` suffix is intentional: today the daemon's env-configured webhook URL is the source of truth for actual routing. The per-call `workflow_id` you pass is surfaced in the envelope for **context tagging** / audit logs, not as a routing override. See [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) for the proposed contract that would make per-call routing safe.

## API

```python
sbo3l_keeperhub_tool(
    *,
    client: SBO3LClientLike,
    workflow_id: str | None = None,           # default: DEFAULT_KH_WORKFLOW_ID
    name: str = "sbo3l_keeperhub_payment_request",
    description: str = ...,
    idempotency_key: Callable[[dict], str] | None = None,
) -> SBO3LKeeperHubToolDescriptor

# Or, with langchain-core installed:
Sbo3lKeeperHubTool(
    client=...,                # required, kw-only
    workflow_id=None,          # default: DEFAULT_KH_WORKFLOW_ID
    name=None, description=None,
)
# Real langchain.tools.BaseTool subclass — drops into AgentExecutor.
```

## License

MIT
