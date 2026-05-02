# SBO3L chaos engineering suite

> **Audience:** QA + Release (Heidi) + anyone hardening SBO3L for production.
> **Outcome:** five scenarios that prove the daemon's hash-chained audit log + idempotency state machine + budget transactions are tamper-evident and recoverable under realistic failure modes.
> **When to run:** before every minor release tag (v1.X.0). Estimated 5-10 minutes total.

## Scenarios

| # | Scenario | What it proves | Expected behaviour |
|---|---|---|---|
| 1 | **Daemon crash mid-tx** | audit-chain recovery from disk | strict-hash verifier accepts the on-disk chain; partial transactions roll back; no orphan audit events |
| 2 | **Storage corruption (byte-flip)** | error path + audit-replay | strict-hash verifier rejects the tampered row with `audit.event_hash_mismatch`; structural verifier (linkage-only) flags the gap |
| 3 | **Network partition (sponsor unreachable)** | pending-state cleanup | idempotency row in `failed` state with non-zero `created_at`; reclaim after grace window succeeds |
| 4 | **Concurrent identical requests (race)** | idempotency holds | exactly one 200 (or 200 cached); the rest 409 `protocol.idempotency_in_flight`; audit chain has exactly one event |
| 5 | **Clock skew** | expiry enforcement | requests with past `expiry` denied `protocol.expired`; budget not incremented; audit row records the deny |

Each scenario captures audit-chain state before/after to `artifacts/<scenario>/{before.json,after.json,result.txt}`.

## Run all

```bash
cd scripts/chaos
bash run-all.sh
# expect: 5/5 PASS in artifacts/summary.txt
```

Individual scenarios:

```bash
bash 01-daemon-crash-mid-tx.sh
bash 02-storage-corruption.sh
bash 03-sponsor-partition.sh
bash 04-concurrent-race.sh
bash 05-clock-skew.sh
```

## Prerequisites

- `sbo3l` CLI installed (`cargo install sbo3l-cli --version 1.0.1`)
- `sbo3l-server` binary built (`cargo build -p sbo3l-server`)
- `sqlite3` (for byte-flip scenario)
- `jq`, `curl` (for HTTP smoke)
- A free port at `:8730` (override via `SBO3L_LISTEN`)

Scenarios run against in-memory SBO3L_POLICY=`reference_low_risk.json` unless overridden via env.

## What each scenario does NOT prove

- These are **smoke**, not chaos at scale. They validate the recovery paths exist; production-grade chaos (kernel-level disk corruption, network packet drops at the kernel level, etc.) is beyond a hackathon scope.
- Scenarios 1 and 2 modify the SQLite file directly. Concurrent multi-process access during the chaos run is undefined; close any `sbo3l-server` instances before running.
- Clock-skew (scenario 5) does NOT actually move the system clock — it crafts an APRP with a deliberately-past `expiry` and asserts the daemon rejects.

## Re-running for tag day

The v1.2.0 release runbook (`docs/release/v1.2.0-prep.md`) includes "run chaos suite + capture artifacts" as a pre-tag step. Failure of any scenario blocks the tag.
