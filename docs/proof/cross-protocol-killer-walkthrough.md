# Cross-protocol KILLER demo — proof of run

Generated 2026-05-02 by `examples/cross-protocol-killer/`. Captures the
full mock-mode transcript so judges can verify the walk offline without
running the demo.

## What this proves

One agent. **Eight framework boundaries** (ENS resolver + 6 LLM frameworks + KH execution + Uniswap quote). **One audit chain**. **One signed capsule**. **Six ✅ verifier checks**.

The "wallet vs mandate" thesis: a wallet is a **single** secret an agent uses to fire many actions; a mandate is a **policy boundary** every action flows through. This demo makes the difference visible in 60 seconds.

## Run

```bash
cd examples/cross-protocol-killer
npm install
npm run demo
```

Default mode is **mock** (no daemon needed; deterministic output for CI). Real-daemon + live-KH + live-Uniswap modes are flagged via:

```bash
npm run demo -- --daemon http://localhost:8730
npm run demo -- --daemon ... --live-kh
npm run demo -- --daemon ... --live-uniswap
```

## Mock-mode output (this run)

```
══════════════════════════════════════════════════════════════════
SBO3L cross-protocol KILLER demo (10 steps, 1 audit chain)
mode: MOCK (daemon=n/a)
live-kh: false    live-uniswap: false
══════════════════════════════════════════════════════════════════

▶ step 1: ens-resolver
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0100
     execution_ref=kh-mock-1

▶ step 2: langchain-ts
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0200
     execution_ref=kh-mock-2
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0100

▶ step 3: crewai-py
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0300
     execution_ref=kh-mock-3
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0200

▶ step 4: autogen
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0400
     execution_ref=kh-mock-4
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0300

▶ step 5: llamaindex-py
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0500
     execution_ref=kh-mock-5
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0400

▶ step 6: vercel-ai
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0600
     execution_ref=kh-mock-6
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0500

▶ step 7: keeperhub
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0700
     execution_ref=kh-mock-7
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0600

▶ step 8: uniswap
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0800
     execution_ref=kh-mock-8
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0700

▶ step 9: capsule-builder
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P0900
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0800
     capsule_type=sbo3l.passport_capsule.v2
     chain_length=8

▶ step 10: verifier
  ✅ allow           audit_event_id=evt-01HTAWX5K3R8YV9NQB7C6P1000
     prev_event_hash → evt-01HTAWX5K3R8YV9NQB7C6P0900
     ✅  capsule.schema_v2
     ✅  capsule.chain_length_matches
     ✅  capsule.issued_at_present
     ✅  audit.all_events_have_id
     ✅  audit.chain_links_consistent
     ✅  audit.allow_count=8/8

══════════════════════════════════════════════════════════════════
SUMMARY
══════════════════════════════════════════════════════════════════
  steps total:        10
  framework allows:   8/8
  capsule built:      true
  verifier:           6/6 ✅
  audit chain length: 10 events
══════════════════════════════════════════════════════════════════
```

## Offline verifier output

```
$ npm run demo > /tmp/run.log
$ npm run verify-output -- --file /tmp/run.log

verify-output checks (7/7):
  ✅  transcript.is_array
  ✅  transcript.length=10 (actual=10)
  ✅  transcript.steps_in_order
  ✅  audit.chain_links_consistent
  ✅  capsule.schema_v2
  ✅  verifier.all_checks_ok (6/6)
  ✅  final.decision=allow

✓ transcript verifies — full audit chain consistent.
```

## Live mode requirements

To run with `--daemon http://localhost:8730 --live-kh --live-uniswap`:

| Requirement | Why |
|---|---|
| running `sbo3l-server` | hosts the policy boundary + audit chain |
| `KH_WORKFLOW_ID=m4t4cnpmhv8qquce3bv3c` env on daemon | step 7 fires real KH webhook |
| `SBO3L_ETH_RPC_URL` env on daemon | step 8 calls Sepolia QuoterV2 |
| funded Sepolia wallet (`SBO3L_ETH_PRIVATE_KEY`) | only if step 8 broadcasts (it doesn't — quote-only) |

In live mode, every `audit_event_id` is a real ULID issued by the daemon, every receipt is Ed25519-signed, and the final capsule passes the `sbo3l-cli passport verify --strict` check (not the demo's synthetic one).

## Test surface

```
npm test         # 5 vitest passing — APRP fixture invariants
npm run typecheck
npm run smoke    # 1-step wiring check, no daemon
npm run demo     # full 10-step run
npm run verify-output -- --file <log>
```

5/5 tests assert: chain=base + recipient=0x1111…1111 (matches reference policy allow rule), fresh nonce per call, expiry 5 min ahead, task_id includes step+framework, provider_url segments per framework so policy can match per step class.
