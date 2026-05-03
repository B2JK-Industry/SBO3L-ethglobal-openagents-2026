"""crewai-keeperhub-demo — 3 specialized CrewAI agents sharing ONE SBO3L
policy boundary, ONE KH workflow_id (advisory), ONE hash-chained audit log.

Each agent submits an APRP to the shared `Sbo3lKeeperHubCrewAITool` and
prints the returned envelope. The auditor agent then replays the audit
chain to prove all 3 calls landed under the same policy boundary.

We intentionally drive the underlying tool **directly** (rather than
through a full crewai.Crew kickoff with an LLM) so the demo runs without
needing OPENAI_API_KEY / ANTHROPIC_API_KEY — the wire path we're proving
is SBO3L → KH, not LLM → SBO3L. The CrewAI Agent objects exist so
readers see the multi-agent composition shape, but the crew kickoff is
mocked at the tool-call boundary. To run end-to-end through an LLM,
replace the direct `tool._run(...)` calls with a real `Crew.kickoff()`.

Run:
    python agent.py

Expected: 3 envelopes (researcher, executor, auditor) — first 2 ALLOW
with kh_execution_ref, third READ-ONLY summary linking the chain.
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from datetime import datetime, timedelta, timezone

from sbo3l_sdk import SBO3LClientSync

from sbo3l_crewai_keeperhub import Sbo3lKeeperHubCrewAITool


def _aprp(task_id_prefix: str, intent: str) -> dict:
    return {
        # research-agent-01 is the only agent_id registered in the bundled
        # reference policy. Demos that hardcode a different id are denied
        # before policy evaluation (auth.agent_not_found). Use SBO3L_POLICY
        # to load a custom policy if you want a different label here.
        # Note: agent_id is the SBO3L identity — distinct from the CrewAI
        # Agent.role label below. SBO3L's policy boundary is per-identity;
        # the 3 CrewAI agents share one identity by design.
        "agent_id": "research-agent-01",
        "task_id": f"{task_id_prefix}-{uuid.uuid4().hex[:8]}",
        "intent": intent,
        "amount": {"value": "0.05", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/inference",
            "method": "POST",
            "expected_recipient": "0x1111111111111111111111111111111111111111",
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(uuid.uuid4()),
        "risk_class": "low",
    }


def _print_envelope(label: str, env: dict) -> None:
    print(f"\n=== {label} ===")
    for k, v in env.items():
        print(f"  {k}: {json.dumps(v)}")


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"daemon: {endpoint}")
    print("composition: 3 CrewAI agents → 1 Sbo3lKeeperHubCrewAITool → 1 SBO3L boundary")

    # Build the 3 specialized agents. Importing crewai here (not at module
    # top) so a missing crewai install fails with a clear message at run
    # time rather than at file load — useful when reading the demo source
    # without the runtime deps installed.
    try:
        from crewai import Agent
    except ImportError:
        print("ERROR: crewai not installed. Run: pip install 'crewai>=0.30,<2'", file=sys.stderr)
        return 4

    with SBO3LClientSync(endpoint) as client:
        tool = Sbo3lKeeperHubCrewAITool(client=client)

        researcher = Agent(
            role="researcher",
            goal="Survey upstream APIs and surface candidates worth purchasing.",
            backstory="A careful surveyor — never pays without policy approval.",
            tools=[tool],
            allow_delegation=False,
            verbose=False,
        )
        executor = Agent(
            role="executor",
            goal="Pay for the chosen API call once policy + budget approve.",
            backstory="The hands-on agent — carries out the purchase under SBO3L's gate.",
            tools=[tool],
            allow_delegation=False,
            verbose=False,
        )
        auditor = Agent(
            role="auditor",
            goal="Verify the receipt chain and confirm boundary integrity.",
            backstory="Read-only — replays the audit log, never moves money.",
            tools=[tool],
            allow_delegation=False,
            verbose=False,
        )

        envelopes: list[dict] = []

        # Researcher fires its APRP (purchase a fact-check API call).
        researcher_env = json.loads(tool._run(json.dumps(_aprp("researcher", "purchase_api_call"))))
        _print_envelope(f"agent={researcher.role}", researcher_env)
        envelopes.append(researcher_env)

        # Executor fires its APRP (the actual production call).
        executor_env = json.loads(tool._run(json.dumps(_aprp("executor", "purchase_api_call"))))
        _print_envelope(f"agent={executor.role}", executor_env)
        envelopes.append(executor_env)

        # Auditor is read-only — does NOT submit a third APRP (which would
        # fire a third paid KH execution, defeating the "verify, don't
        # spend" framing). In a live setup the auditor calls SBO3L's
        # `sbo3l.audit_lookup` MCP tool against the prior agents'
        # audit_event_ids. For the demo we synthesise the verification
        # locally by re-checking the policy_hash + workflow_id consistency
        # of the researcher + executor envelopes — the same invariants the
        # real audit lookup would assert.
        auditor_env = {
            "agent": auditor.role,
            "verification_target_audit_event_ids": [
                researcher_env.get("audit_event_id"),
                executor_env.get("audit_event_id"),
            ],
            "verification_target_kh_execution_refs": [
                researcher_env.get("kh_execution_ref"),
                executor_env.get("kh_execution_ref"),
            ],
            # Auditor's invariant assertions — synthesised; in production
            # this comes from sbo3l.audit_lookup against the daemon.
            "policy_hash_consistent": (
                researcher_env.get("policy_hash") == executor_env.get("policy_hash")
                and researcher_env.get("policy_hash") is not None
            ),
            "kh_workflow_consistent": (
                researcher_env.get("kh_workflow_id_advisory")
                == executor_env.get("kh_workflow_id_advisory")
            ),
            "spend_executed_by_auditor": False,  # explicit — read-only
        }
        _print_envelope(f"agent={auditor.role} (read-only verification)", auditor_env)
        envelopes.append(auditor_env)

    print("\n=== boundary-integrity check ===")
    # Only count submitting agents (researcher + executor) for the
    # boundary-shape comparison. The auditor's envelope is a synthetic
    # verification record — it doesn't carry policy_hash or
    # kh_workflow_id_advisory because it never submitted to SBO3L.
    submitting = [e for e in envelopes if "policy_hash" in e]
    policy_hashes = {e.get("policy_hash") for e in submitting if e.get("policy_hash")}
    workflow_ids = {e.get("kh_workflow_id_advisory") for e in submitting}
    print(f"  unique policy_hash across submitting agents: {len(policy_hashes)} (expect: 1)")
    print(f"  unique kh_workflow_id_advisory:              {len(workflow_ids)} (expect: 1)")
    for i, e in enumerate(envelopes, start=1):
        if "policy_hash" in e:
            ref = e.get("kh_execution_ref")
            ev = e.get("audit_event_id")
            dec = e.get("decision")
            print(f"  agent {i} (submit): decision={dec} audit_event_id={ev} kh_execution_ref={ref}")
        else:
            targets = e.get("verification_target_audit_event_ids", [])
            print(
                f"  agent {i} (read-only verify): "
                f"targets={len(targets)} policy_consistent={e.get('policy_hash_consistent')} "
                f"workflow_consistent={e.get('kh_workflow_consistent')} "
                f"spent={e.get('spend_executed_by_auditor')}"
            )

    if len(policy_hashes) == 1 and len(workflow_ids) == 1:
        print("\nOK: 2 submitting agents bound to ONE policy + ONE KH workflow; auditor verified read-only")
        return 0

    print("\nFAIL: agents drifted across multiple policies / workflows", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
