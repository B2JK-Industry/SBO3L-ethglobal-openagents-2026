"""Minimal SBO3L Python example agent.

1. Loads a golden APRP from `test-corpus/`.
2. Submits via `sbo3l-sdk` (sync client; safe inside any event loop).
3. Prints decision + execution_ref.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from sbo3l_sdk import SBO3LClientSync, bearer

REPO_ROOT = Path(__file__).resolve().parents[2]
GOLDEN = REPO_ROOT / "test-corpus" / "aprp" / "golden_001_minimal.json"


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    bearer_token = os.environ.get("SBO3L_BEARER_TOKEN")

    aprp = json.loads(GOLDEN.read_text())
    auth = bearer(bearer_token) if bearer_token else None

    with SBO3LClientSync(endpoint, auth=auth) as client:
        r = client.submit(aprp)

    print(f"decision: {r.decision}")
    print(f"execution_ref: {r.receipt.execution_ref or '(none)'}")
    print(f"audit_event_id: {r.audit_event_id}")
    print(f"request_hash: {r.request_hash}")
    print(f"policy_hash: {r.policy_hash}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
