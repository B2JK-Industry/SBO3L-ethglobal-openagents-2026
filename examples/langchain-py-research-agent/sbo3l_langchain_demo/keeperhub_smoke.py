"""KH-flavored smoke runner — submits one APRP via the SBO3L daemon's
KeeperHub adapter and prints the captured KH `execution_ref`.

No LangChain / OpenAI required. Mirror of `smoke.py` but built around
`keeperhub_tool` so the KH path is the explicit demonstration.

Prereqs:
  - SBO3L daemon running on localhost:8730 with KH adapter configured:
      SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/<id>/webhook \\
      SBO3L_KEEPERHUB_TOKEN=wfb_<token> \\
      SBO3L_ALLOW_UNAUTHENTICATED=1 \\
      SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 \\
        cargo run --bin sbo3l-server &
  - Without webhook env vars the daemon's KH adapter falls back to
    local_mock and returns a kh-<ULID> ref with mock=true. The demo
    still prints the (mock) execution_ref so the wire path is visible.

Run:
  python -m sbo3l_langchain_demo.keeperhub_smoke
"""

from __future__ import annotations

import json
import sys

from .keeperhub_tool import build_demo_aprp, keeperhub_tool
from .tools import default_client


def main() -> int:
    aprp = build_demo_aprp()
    print(f"▶ KH smoke: workflow target = {aprp.get('task_id')}")
    print(f"▶ APRP: agent={aprp['agent_id']} amount={aprp['amount']['value']} {aprp['amount']['currency']} chain={aprp['chain']}")

    with default_client() as client:
        descriptor = keeperhub_tool(client=client)
        print(f"\n▶ tool: {descriptor.name}")
        envelope_raw = descriptor.func(json.dumps(aprp))

    envelope = json.loads(envelope_raw)
    print("  envelope:")
    for k, v in envelope.items():
        print(f"    {k}: {json.dumps(v)}")

    if envelope.get("decision") == "allow":
        ref = envelope.get("kh_execution_ref")
        if ref:
            print(f"\n✓ allow + KH executed — kh_execution_ref={ref}")
            print(f"  workflow_advisory={envelope.get('kh_workflow_id_advisory')}")
            print(f"  audit_event_id={envelope.get('audit_event_id')}")
            return 0
        print(
            "\n⚠ allow but no kh_execution_ref — KH adapter likely returned"
            " an empty execution_ref. Check daemon logs / webhook config."
        )
        return 3

    if "error" in envelope:
        print(f"\n✗ transport error — {envelope['error']}")
        return 2

    print(
        f"\n✗ {envelope.get('decision', '?')} — deny_code {envelope.get('deny_code', '?')}"
    )
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
