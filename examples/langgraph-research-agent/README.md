# `examples/langgraph-research-agent`

End-to-end LangGraph research agent — 3-node graph with `plan` reasoning across 2 tools (`data_fetch` + APRP-builder), `policy_guard` (real `sbo3l_langgraph.PolicyGuardNode`), and `execute`. Routes through KH workflow `m4t4cnpmhv8qquce3bv3c`. Deny short-circuits to `END` (load-bearing safety guarantee — `execute` MUST NOT run when SBO3L denies).

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/langgraph-research-agent && python3 -m venv .venv && .venv/bin/pip install -e ../../sdks/python -e ../../sdks/python/integrations/langgraph -e .
.venv/bin/python main.py
```

## Graph

```
   plan  ──▶  policy_guard  ──▶  execute
   (2 tools)                        │
                                    └─▶ END  (when SBO3L denies)
```

- **plan** (`_plan_node`) — calls `data_fetch` (provider metadata) + `aprp_build` (assemble APRP). Writes `state["proposed_action"]`.
- **policy_guard** (`PolicyGuardNode`) — reads `proposed_action`, runs SBO3L decision, writes either `policy_receipt` (allow) or `deny_reason` (deny / requires_human / error).
- **execute** (`_execute_node`) — reads `policy_receipt`, reports the `execution_ref`. Real agents would call a sponsor adapter (KH, Uniswap) here.

## Tests

```bash
.venv/bin/pip install pytest pytest-httpx
.venv/bin/pytest -q
```

2 end-to-end tests: allow path walks all 3 nodes; deny path short-circuits to END (verifies `execute` does NOT run on deny — the safety guarantee of the whole pattern).

## License

MIT
