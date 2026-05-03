# `examples/crewai-keeperhub-demo`

A runnable smoke for the SBO3L → KeeperHub policy gate composed into a **3-agent CrewAI crew**: one shared policy boundary, one KH workflow_id (advisory), one hash-chained audit log spanning every agent.

## What it shows

| Agent | Role | Task prefix | KH execution surface |
|---|---|---|---|
| Researcher | Surveys upstream API | `researcher-…` | One APRP per fact-check call |
| Executor | Pays for the chosen API call | `executor-…` | The actual purchase |
| Auditor | Verifies the receipt chain | `auditor-…` | Read-only — replays the audit log |

All three share **one** `Sbo3lKeeperHubCrewAITool` instance + **one** `SBO3LClientSync`. The SBO3L daemon enforces:

- One agent identity (`research-agent-01`, the only id registered in the bundled reference policy)
- One policy boundary across all 3 agents
- One hash-chained Ed25519 audit log — researcher's request_hash links to executor's request_hash links to auditor's read

This is the CrewAI-specific composition angle that no execution-only wrapper (Devendra's, the various LangChain plugins) can match — they wrap the call site, not the policy boundary.

## Prereqs

```bash
# 1. Run the SBO3L daemon in mock mode (one-time per session)
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &

# 2. Install demo deps (one-time)
cd examples/crewai-keeperhub-demo
python3 -m venv .venv
.venv/bin/pip install \
  -e ../../sdks/python \
  -e ../../integrations/crewai-keeperhub-py \
  "crewai>=0.30,<2"
```

## Run

```bash
.venv/bin/python agent.py
```

Expected output: 3 envelopes (one per agent), each with `decision`, `kh_execution_ref` (on allow), and `audit_event_id`. The auditor agent prints the linked-by-request_hash chain proving all 3 calls share one policy boundary.

## Live KeeperHub mode

Add daemon-side env vars before starting `sbo3l-server`:

```bash
SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/<id>/webhook \
SBO3L_KEEPERHUB_TOKEN=wfb_<token> \
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &
```

Without these, the daemon's KH adapter falls back to `local_mock` and returns `kh-<ULID>` refs with `mock=true` evidence — the wire path stays visible end-to-end.

## Composability with Devendra's `langchain-keeperhub`

This demo ships our **policy-guarded multi-agent crew** path. Devendra's package handles raw KH execution with ENS / Turnkey TEE / MCP bridge. Both can co-exist in one crew:

- Use Devendra's tool for the raw KH webhook call inside one agent
- Use ours as the policy gate that decides whether the raw call should fire — across all agents

Or use ours alone for the full gate-then-execute path. See [`integrations/crewai-keeperhub-py/README.md`](../../integrations/crewai-keeperhub-py/README.md) for the comparison table.
