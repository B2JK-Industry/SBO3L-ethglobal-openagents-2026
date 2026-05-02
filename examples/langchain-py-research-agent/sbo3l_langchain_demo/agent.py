"""Full LangChain Python research agent — needs OPENAI_API_KEY and the
`[langchain]` extra (`pip install -e ".[langchain]"`). Uses
`create_openai_functions_agent` so the LLM picks tool calls.

Usage:
    SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
    .venv/bin/pip install -e ".[langchain]"
    export OPENAI_API_KEY=sk-...
    .venv/bin/python -m sbo3l_langchain_demo.agent
"""

from __future__ import annotations

import os
import sys

from .tools import KH_WORKFLOW_ID, build_sbo3l_pay_func, default_client, fetch_url


def main() -> int:
    if "OPENAI_API_KEY" not in os.environ:
        print(
            "error: OPENAI_API_KEY required. "
            "Use `python -m sbo3l_langchain_demo.smoke` for the no-LLM path.",
            file=sys.stderr,
        )
        return 1

    try:
        from langchain.agents import (
            AgentExecutor,
            create_openai_functions_agent,
        )
        from langchain_core.prompts import ChatPromptTemplate
        from langchain_core.tools import StructuredTool
        from langchain_openai import ChatOpenAI
    except ImportError:
        print(
            'error: install langchain + openai with `pip install -e ".[langchain]"` first.',
            file=sys.stderr,
        )
        return 1

    client = default_client()
    sbo3l_pay = build_sbo3l_pay_func(client)

    tools = [
        StructuredTool.from_function(
            func=fetch_url,
            name="data_fetch",
            description="GET a JSON URL and return body. Use BEFORE paying.",
        ),
        StructuredTool.from_function(
            func=sbo3l_pay,
            name="sbo3l_payment_request",
            description=(
                "Submit a JSON-stringified APRP to SBO3L for policy decision. "
                f"Routes to KeeperHub workflow {KH_WORKFLOW_ID} when allowed. "
                "Returns decision envelope; on deny, branch on deny_code."
            ),
        ),
    ]

    system = (
        "You are an autonomous research agent. ALWAYS go through the "
        "sbo3l_payment_request tool for any payment — never claim a payment was made "
        "without it. On a deny, explain the deny_code to the user."
    )
    prompt = ChatPromptTemplate.from_messages(
        [("system", system), ("user", "{input}"), ("placeholder", "{agent_scratchpad}")]
    )

    llm = ChatOpenAI(model="gpt-4o-mini", temperature=0)
    agent = create_openai_functions_agent(llm, tools, prompt)
    executor = AgentExecutor(agent=agent, tools=tools, max_iterations=6, verbose=False)

    user_task = (
        "Pay 0.05 USDC for an inference call to https://api.example.com/v1/inference. "
        "agent_id research-agent-01, task_id demo-langchain-py-1, "
        "nonce 01HTAWX5K3R8YV9NQB7C6P2DGM, expiry 2026-05-01T10:31:00Z, "
        "low risk, x402 protocol on base."
    )
    print(f"▶ user: {user_task}\n")
    result = executor.invoke({"input": user_task})
    print(f"\n▶ agent: {result.get('output', '(no output)')}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
