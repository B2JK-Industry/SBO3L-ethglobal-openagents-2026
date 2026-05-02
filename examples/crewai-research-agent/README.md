# `examples/crewai-research-agent`

End-to-end CrewAI research agent that asks SBO3L to authorize every payment-shaped action. Reasons across 2 tools (`data_fetch` + `sbo3l_payment_request`) and routes allowed payments through the live KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/crewai-research-agent && python3 -m venv .venv && .venv/bin/pip install -e ../../sdks/python -e ../../integrations/crewai -e .
.venv/bin/python -m sbo3l_crewai_demo.smoke   # no OpenAI / no crewai needed
```

## With CrewAI + an LLM (full Crew)

```bash
.venv/bin/pip install -e ".[crewai]"
export OPENAI_API_KEY=sk-...
.venv/bin/python -m sbo3l_crewai_demo.agent
```

The `crewai.Agent` reasons across both tools, fetches provider metadata via `data_fetch`, then submits an APRP through `sbo3l_payment_request`. SBO3L decides allow/deny; on allow, the daemon's KH adapter routes to KeeperHub workflow `m4t4cnpmhv8qquce3bv3c`.

## Tools

| Tool | Description |
|---|---|
| `data_fetch` | GET a JSON URL, return body. The agent uses it to inspect a provider before paying. |
| `sbo3l_payment_request` | Submit an APRP via `sbo3l-crewai` → SBO3L policy boundary → KH adapter. |

## Expected smoke output

```
▶ smoke: KH workflow target = m4t4cnpmhv8qquce3bv3c

▶ tool: data_fetch (GitHub status — public, low-noise)
  ✓ HTTP 200

▶ tool: sbo3l_payment_request (APRP → SBO3L → KH adapter)
  envelope:
    decision: "allow"
    deny_code: null
    matched_rule_id: "allow-low-risk-x402"
    execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS"
    audit_event_id: "evt-..."
    request_hash: "..."
    policy_hash: "..."

✓ allow — execution_ref kh-01HTAWX5K3R8YV9NQB7C6P2DGS
  audit_event_id: evt-...
```

Total wall-clock: < 30 s on a laptop with the daemon already running.

## Tests

```bash
.venv/bin/pip install pytest pytest-httpx
.venv/bin/pytest -q
```

2 tests verify the SBO3L tool path against a mocked-httpx daemon (real `sbo3l_sdk.SBO3LClientSync`, no SDK mocks per QA rule).

## Files

- `sbo3l_crewai_demo/tools.py` — `fetch_url` + `build_sbo3l_pay_func` (real `sbo3l_sdk.SBO3LClientSync`).
- `sbo3l_crewai_demo/agent.py` — full `crewai.Crew` with two `BaseTool` subclasses (needs OPENAI_API_KEY + `[crewai]` extra).
- `sbo3l_crewai_demo/smoke.py` — no-LLM smoke; exercises the tool path directly.
- `test_smoke.py` — pytest with httpx mock.

## License

MIT
