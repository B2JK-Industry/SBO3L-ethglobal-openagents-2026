"""Full CrewAI research agent — needs OPENAI_API_KEY (or any LiteLLM-compatible model).

Usage:
    SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
    export OPENAI_API_KEY=sk-...
    .venv/bin/python -m sbo3l_crewai_demo.agent

Skipped automatically if `crewai` isn't installed — install via the optional
extra: `.venv/bin/pip install -e ".[crewai]"`.
"""

from __future__ import annotations

import os
import sys

from .tools import KH_WORKFLOW_ID, build_sbo3l_pay_func, default_client, fetch_url


def main() -> int:
    if "OPENAI_API_KEY" not in os.environ:
        print(
            "error: OPENAI_API_KEY required for the full agent loop. "
            "Use `python -m sbo3l_crewai_demo.smoke` for the no-LLM path.",
            file=sys.stderr,
        )
        return 1

    try:
        from crewai import Agent, Crew, Task  # type: ignore[import-not-found]
        from crewai_tools import BaseTool  # type: ignore[import-not-found]
    except ImportError:
        print(
            'error: install crewai with `pip install -e ".[crewai]"` first.',
            file=sys.stderr,
        )
        return 1

    client = default_client()
    sbo3l_pay = build_sbo3l_pay_func(client)

    class FetchTool(BaseTool):  # type: ignore[misc, no-any-unimported]
        name: str = "data_fetch"
        description: str = (
            "GET a JSON URL and return the body. Use this BEFORE deciding to spend money."
        )

        def _run(self, url: str) -> str:
            return fetch_url(url)

    class Sbo3lPayTool(BaseTool):  # type: ignore[misc, no-any-unimported]
        name: str = "sbo3l_payment_request"
        description: str = (
            "Submit an APRP JSON to SBO3L for policy decision. "
            f"Routes to KeeperHub workflow {KH_WORKFLOW_ID} when allowed. "
            "Input MUST be a JSON-stringified APRP."
        )

        def _run(self, aprp_json: str) -> str:
            return sbo3l_pay(aprp_json)

    researcher = Agent(  # type: ignore[no-untyped-call]
        role="Autonomous Research Agent",
        goal=(
            "Decide whether to pay 0.05 USDC for an inference call to api.example.com. "
            "Use data_fetch to inspect the provider, then sbo3l_payment_request to make the payment."
        ),
        backstory=(
            "You are a frugal but decisive research agent. You always go through SBO3L's "
            "policy boundary for every payment — never claim a payment was made without it."
        ),
        tools=[FetchTool(), Sbo3lPayTool()],
        verbose=True,
    )

    task = Task(  # type: ignore[no-untyped-call]
        description=(
            "Pay 0.05 USDC for an inference call to https://api.example.com/v1/inference. "
            "Agent id research-agent-01, task demo-crewai-1, nonce 01HTAWX5K3R8YV9NQB7C6P2DGM, "
            "expiry 2026-05-01T10:31:00Z, low risk, x402 protocol on base."
        ),
        agent=researcher,
        expected_output="The signed PolicyReceipt JSON from sbo3l_payment_request.",
    )

    crew = Crew(agents=[researcher], tasks=[task], verbose=True)  # type: ignore[no-untyped-call]
    result = crew.kickoff()
    print(f"\n▶ result:\n{result}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
