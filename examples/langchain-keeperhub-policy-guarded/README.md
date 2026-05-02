# `examples/langchain-keeperhub-policy-guarded`

Side-by-side demonstration: an APRP submitted through SBO3L's policy boundary, executed via the daemon's KeeperHub adapter, returns a signed receipt with `kh_execution_ref`.

Available in both **TypeScript** (`agent.mjs`) and **Python** (`agent.py`) — identical wire path, different language ergonomics.

## What this demo proves

- A LangChain agent's payment intent goes through SBO3L's policy gate **first**.
- Only on `decision == "allow"` does the daemon's KH adapter fire the webhook.
- On deny / requires_human, the agent sees a structured `deny_code` and `kh_execution_ref` is `None` — KH is never asked to execute.
- The signed `PolicyReceipt` (Ed25519) + hash-chained audit log are byproducts every call.

## Compare to Devendra's `langchain-keeperhub`

Devendra's example wraps the KH execution directly:

```python
# Devendra's pattern (paraphrased — wraps execution only):
from langchain_keeperhub import KeeperHubTool
tool = KeeperHubTool(workflow_id="...", token="...")
result = tool.invoke({"action": "..."})
# → KH executed; no policy gate, no signed receipt.
```

Our pattern adds the gate upstream:

```python
from sbo3l_sdk import SBO3LClientSync
from sbo3l_langchain_keeperhub import Sbo3lKeeperHubTool

client = SBO3LClientSync("http://localhost:8730")
tool = Sbo3lKeeperHubTool(client=client)
result = tool.invoke({"aprp_json": "<...APRP JSON...>"})
# → SBO3L decided allow + KH executed; receipt + audit captured.
# OR → SBO3L decided deny + KH NEVER asked; receipt has deny_code; audit captured.
```

**Composable:** in a real agent stack, both can co-exist. SBO3L's tool gates whether to fire; Devendra's tool actually fires (or is replaced by SBO3L's daemon-side adapter, your choice).

## Run the Python demo

```bash
# Prereq: SBO3L daemon running on localhost:8730 in mock mode
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &

cd examples/langchain-keeperhub-policy-guarded
python3 -m venv .venv
.venv/bin/pip install -e ../../sdks/python -e ../../integrations/langchain-keeperhub-py
.venv/bin/python agent.py
```

## Run the TS demo

```bash
# Same daemon prereq.
cd examples/langchain-keeperhub-policy-guarded
npm install
node agent.mjs
```

## Live KeeperHub mode

Add the daemon-side env vars before starting `sbo3l-server`:

```bash
SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/<id>/webhook \
SBO3L_KEEPERHUB_TOKEN=wfb_<token> \
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &
```

Without these, the daemon's KH adapter falls back to `local_mock` and returns a `kh-<ULID>` ref with `mock=true` evidence — the wire path is still visible end-to-end.
