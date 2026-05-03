# `sbo3l-autogen-keeperhub`

> Microsoft AutoGen Python tool that **gates KeeperHub workflow execution through SBO3L's policy boundary**. Composable with `sbo3l-sdk`.

## Why this exists alongside `langchain-keeperhub` (Devendra's PyPI pkg)

| | `langchain-keeperhub` (Devendra) | `sbo3l-autogen-keeperhub` (this) |
|---|---|---|
| Target framework | LangChain | Microsoft AutoGen (`pyautogen`) |
| What it wraps | KH webhook execution | SBO3L policy gate → KH webhook execution |
| Decision step | x (agent decides) | yes (SBO3L decides; signed receipt) |
| Budget enforcement | x | yes |
| Audit chain | x | yes (hash-chained Ed25519 log) |
| ENS / Turnkey TEE / MCP bridge | yes | x (not duplicated) |

**Composable:** use Devendra's tool for the raw KH binding (in a LangChain branch of your stack) + ours as the policy gate that decides whether the raw call should fire (in an AutoGen branch). Or use ours alone for the full gate-then-execute path inside an AutoGen `ConversableAgent`.

## Install

```bash
pip install "sbo3l-autogen-keeperhub[autogen]" sbo3l-sdk
```

The `[autogen]` extra pulls in `pyautogen` so the `register_sbo3l_keeperhub_tool` helper can drop directly into a `ConversableAgent`'s function registry. Without it, only the framework-agnostic `sbo3l_autogen_keeperhub_tool()` factory is available.

## 5-line setup

```python
from autogen import ConversableAgent
from sbo3l_sdk import SBO3LClientSync
from sbo3l_autogen_keeperhub import register_sbo3l_keeperhub_tool

client = SBO3LClientSync("http://localhost:8730")
executor = ConversableAgent(name="executor", llm_config=False)
register_sbo3l_keeperhub_tool(executor, client=client)
```

Or framework-agnostic:

```python
from sbo3l_autogen_keeperhub import sbo3l_autogen_keeperhub_tool
descriptor = sbo3l_autogen_keeperhub_tool(client=client)
# descriptor.name + descriptor.description + descriptor.func
```

## Wire path

Same as the LangChain + TS packages — SBO3L decides → KH adapter executes on allow → tool returns `kh_execution_ref`. The daemon is the policy boundary, so `kh_execution_ref` matches across SDK languages and frameworks.

  1. AutoGen agent calls `sbo3l_keeperhub_payment_request(aprp_json=...)` via its function registry.
  2. The registered callable POSTs the APRP to SBO3L's `/v1/payment-requests`.
  3. SBO3L decides allow / deny / requires_human against the loaded policy + budget + nonce + provider trust list.
  4. On allow: SBO3L's executor_callback hands the signed `PolicyReceipt` to the daemon-side KeeperHub adapter.
  5. KH adapter POSTs the IP-1 envelope to the workflow webhook, captures `executionId`, surfaces it as `receipt.execution_ref`.
  6. Tool returns: `{decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, deny_code}`.

## On `kh_workflow_id_advisory`

The `_advisory` suffix is intentional: today the daemon's env-configured webhook URL is the source of truth for actual routing. The per-call `workflow_id` you pass is surfaced in the envelope for **context tagging** / audit logs, not as a routing override. See [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) for the proposed contract that would make per-call routing safe.

## API

```python
sbo3l_autogen_keeperhub_tool(
    *,
    client: SBO3LClientLike,
    workflow_id: str | None = None,           # default: DEFAULT_KH_WORKFLOW_ID
    name: str = "sbo3l_keeperhub_payment_request",
    description: str = ...,
    idempotency_key: Callable[[dict], str] | None = None,
) -> SBO3LKeeperHubToolDescriptor

# Or, with pyautogen installed:
register_sbo3l_keeperhub_tool(
    agent,                          # ConversableAgent (or duck-typed equivalent)
    *,
    client=...,                     # required, kw-only
    workflow_id=None,               # default: DEFAULT_KH_WORKFLOW_ID
    name=None, description=None,
) -> SBO3LKeeperHubToolDescriptor
# Registers the SBO3L tool with the agent's function_map and returns
# the descriptor (so callers can mirror name+description on a sibling
# proposer agent's register_for_llm decorator).
```

## License

MIT
