# `examples/langchain-keeperhub-demos`

Five focused, one-file Python demos showing different shapes of the SBO3L → KeeperHub policy gate. Each is ~50-100 LOC, runnable against a mock-mode SBO3L daemon, and demonstrates a distinct usage pattern.

| File | Pattern | LOC | What it proves |
|---|---|---|---|
| [`simple-keepalive.py`](simple-keepalive.py) | Minimum viable | ~60 | One APRP → SBO3L decide → KH execution_ref. The bare floor. |
| [`multi-step-research.py`](multi-step-research.py) | Chain N workflows under one budget | ~95 | 3 sequential APRPs share one budget cap; second is allowed only if first didn't exhaust it. |
| [`cross-agent-attestation.py`](cross-agent-attestation.py) | Agent-A delegates to Agent-B | ~95 | A's signed receipt becomes B's input; SBO3L verifies A's attestation before allowing B's KH call. |
| [`retry-with-backoff.py`](retry-with-backoff.py) | Transport retries on 5xx | ~90 | Adapter wrapper retries SBO3L transport errors with exponential backoff; passes through deny verbatim. |
| [`observability.py`](observability.py) | OpenTelemetry spans + receipts | ~95 | Each tool call emits an OTel span carrying SBO3L's `audit_event_id` + `kh_execution_ref` as attributes. |

Plus this overview README.

## Why one-file-per-pattern

A judge or eval reader can open each file in isolation and grok the pattern in 30 seconds. No cross-file state, no setup boilerplate, no shared utility module. Every file's surface area is its own — copy any one of them into your own project and it runs.

## Common prereqs

```bash
# 1. Run the SBO3L daemon in mock mode (one-time per session)
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &

# 2. Install this dir's deps (one-time)
cd examples/langchain-keeperhub-demos
python3 -m venv .venv
.venv/bin/pip install \
  -e ../../sdks/python \
  -e ../../integrations/langchain-keeperhub-py
# Some demos need extras — see each file's docstring.
```

Then run any demo:

```bash
.venv/bin/python simple-keepalive.py
.venv/bin/python multi-step-research.py
.venv/bin/python cross-agent-attestation.py
.venv/bin/python retry-with-backoff.py
.venv/bin/python observability.py
```

Each prints a structured envelope and exits 0 on the happy path.

## Live KeeperHub mode

Add daemon-side env vars before starting `sbo3l-server`:

```bash
SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/<id>/webhook \
SBO3L_KEEPERHUB_TOKEN=wfb_<token> \
SBO3L_ALLOW_UNAUTHENTICATED=1 \
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \
  cargo run --bin sbo3l-server &
```

Without these, the daemon's KH adapter falls back to `local_mock` and returns `kh-<ULID>` refs with `mock=true` evidence — the wire path stays visible end-to-end across all demos.

## Composability with Devendra's `langchain-keeperhub`

These demos ship our **policy-guarded** path. Devendra's package handles raw KH execution with ENS / Turnkey TEE / MCP bridge. Both can co-exist in one agent stack:

- Use Devendra's tool for the raw KH webhook call
- Use ours as the policy gate that decides whether to fire the raw call

Or use ours alone for the full gate-then-execute path. See [`@sbo3l/langchain-keeperhub`](../../integrations/langchain-keeperhub-py/README.md) for the comparison table.
