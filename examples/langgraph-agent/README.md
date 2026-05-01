# `examples/langgraph-agent`

Minimal 3-node LangGraph using `sbo3l-langgraph`'s `PolicyGuardNode`:

```
   plan  ──►  policy_guard  ──►  execute
                  │
                  └─►  END  (when SBO3L denies)
```

> ⚠ **DRAFT (T-1-8):** depends on `sbo3l-sdk` + `sbo3l-langgraph` being published to PyPI.
> While unpublished, install both via `pip install -e ../../sdks/python` and
> `pip install -e ../../sdks/python/integrations/langgraph`.

## Run

```bash
# 1. Start the SBO3L daemon
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &

# 2. Set up venv and install workspace deps
cd examples/langgraph-agent
python3 -m venv .venv
.venv/bin/pip install -e ../../sdks/python
.venv/bin/pip install -e ../../sdks/python/integrations/langgraph
.venv/bin/pip install langgraph

# 3. Run the example
.venv/bin/python main.py
```

Expected output (allow):

```
✓ allow — executed; ref=kh-01HTAWX5K3R8YV9NQB7C6P2DGS
  audit_event_id: evt-...
  execution_ref:  kh-...
```

## Tests

```bash
.venv/bin/pip install pytest pytest-httpx
.venv/bin/pytest -q
```

Both allow and deny paths run end-to-end through the compiled graph; `execute` MUST NOT run when SBO3L denies (verified by the deny test).

## What's where

- `sbo3l_example_langgraph/graph.py` — `build_app(client)` returns the compiled 3-node graph.
- `main.py` — runs the graph against a real daemon, prints the outcome.
- `test_graph.py` — pytest with `pytest-httpx` mock; exercises both paths.

## License

MIT
