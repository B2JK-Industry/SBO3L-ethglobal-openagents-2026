"""Full LlamaIndex agent — needs OPENAI_API_KEY and the `[llamaindex]` extra.

Usage:
    SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
    .venv/bin/pip install -e ".[llamaindex]"
    export OPENAI_API_KEY=sk-...
    .venv/bin/python -m sbo3l_llamaindex_demo.agent
"""

from __future__ import annotations

import os
import sys

from .tools import KH_WORKFLOW_ID, build_sbo3l_pay_func, default_client, fetch_url


def main() -> int:
    if "OPENAI_API_KEY" not in os.environ:
        print(
            "error: OPENAI_API_KEY required. "
            "Use `python -m sbo3l_llamaindex_demo.smoke` for the no-LLM path.",
            file=sys.stderr,
        )
        return 1

    try:
        from llama_index.core.agent import ReActAgent
        from llama_index.core.tools import FunctionTool
        from llama_index.llms.openai import OpenAI
    except ImportError:
        print(
            'error: install llama_index with `pip install -e ".[llamaindex]"` first.',
            file=sys.stderr,
        )
        return 1

    client = default_client()
    sbo3l_pay = build_sbo3l_pay_func(client)

    tools = [
        FunctionTool.from_defaults(
            fn=fetch_url,
            name="data_fetch",
            description="GET a JSON URL and return body. Use BEFORE paying.",
        ),
        FunctionTool.from_defaults(
            fn=sbo3l_pay,
            name="sbo3l_payment_request",
            description=(
                "Submit a JSON-stringified APRP to SBO3L for policy decision. "
                f"Routes to KeeperHub workflow {KH_WORKFLOW_ID} when allowed. "
                "Returns decision envelope; on deny, branch on deny_code."
            ),
        ),
    ]

    llm = OpenAI(model="gpt-4o-mini", temperature=0.0)
    agent = ReActAgent.from_tools(tools, llm=llm, verbose=True, max_iterations=6)

    user_task = (
        "Pay 0.05 USDC for an inference call to https://api.example.com/v1/inference. "
        "agent_id research-agent-01, task_id demo-llamaindex-1, "
        "nonce 01HTAWX5K3R8YV9NQB7C6P2DGM, expiry 2026-05-01T10:31:00Z, "
        "low risk, x402 protocol on base."
    )
    print(f"▶ user: {user_task}\n")
    response = agent.chat(user_task)
    print(f"\n▶ agent: {response}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
