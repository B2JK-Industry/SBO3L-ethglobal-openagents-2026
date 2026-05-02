"""Run the 3-node graph end-to-end against a running SBO3L daemon.

Usage:
    SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
    .venv/bin/python main.py
"""

from __future__ import annotations

import os
import sys

from sbo3l_sdk import SBO3LClientSync, bearer

from sbo3l_langgraph_demo import KH_WORKFLOW_ID, build_app


def main() -> int:
    print(f"▶ KH workflow target = {KH_WORKFLOW_ID}\n")

    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    bearer_token = os.environ.get("SBO3L_BEARER_TOKEN")
    auth = bearer(bearer_token) if bearer_token else None

    with SBO3LClientSync(endpoint, auth=auth) as client:
        app = build_app(client)
        final = app.invoke({"user_request": "Pay 0.05 USDC for an inference call to api.example.com"})

    if "policy_receipt" in final:
        receipt = final["policy_receipt"]["receipt"]
        print(f"✓ allow — {final.get('result', '(no result)')}")
        print(f"  audit_event_id: {final['policy_receipt']['audit_event_id']}")
        print(f"  execution_ref:  {receipt.get('execution_ref') or '(none)'}")
        return 0
    if "deny_reason" in final:
        dr = final["deny_reason"]
        print(f"✗ {dr.get('decision')} — {dr.get('code')}")
        if "audit_event_id" in dr:
            print(f"  audit_event_id: {dr['audit_event_id']}")
        if "detail" in dr:
            print(f"  detail: {dr['detail']}")
        return 2
    print("error: graph completed with neither policy_receipt nor deny_reason", file=sys.stderr)
    return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
