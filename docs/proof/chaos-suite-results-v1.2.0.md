# Chaos suite results — v1.2.0 pre-tag verification

> **Run:** 2026-05-02 ~07:29-07:52 CEST (Heidi end-to-end on `agent/qa/r8-walkthrough-and-fixes` after #226 + #227 merged)
> **Binary:** `target/release/sbo3l-server` built from main HEAD (`172d7c2` post-#227 merge + the chaos-margin fix in this branch)
> **Result:** **5/5 PASS** — chaos gate clears for the v1.2.0 tag per `docs/release/v1.2.0-prep.md`

## Summary

| # | Scenario | Result | Asserts |
|---|---|---|---|
| 1 | `01-daemon-crash-mid-tx` | ✅ PASS | audit chain grew 1 → 2 after SIGKILL+restart; post-restart audit_events count == 2 (one per nonce submitted) |
| 2 | `02-storage-corruption` | ✅ PASS | sqlite3 byte-flip on payload_hash detected; strict-hash verifier rejected; structural verifier accepted (linkage byte intact, by design) |
| 3 | `03-sponsor-partition` | ✅ PASS | KH webhook to RFC 5737 192.0.2.1 timed out; idempotency replay on same key returned 409 within grace window |
| 4 | `04-concurrent-race` | ✅ PASS | 50 concurrent same-Idempotency-Key POSTs → **50×200 + 0×409 + audit chain grew by exactly 1 event** (state machine cached all replays cleanly) |
| 5 | `05-clock-skew` | ✅ PASS | APRP with `expiry: 120s ago` rejected with `protocol.aprp_expired` HTTP 400; follow-up valid request returned HTTP 200 (budget unaffected) |

## Findings closed in this run

Two real findings surfaced in the [round-4 chaos run](../../scripts/chaos/artifacts/) and were fixed before this v1.2.0 verification:

| Finding | Filed | Fixed by | Verified |
|---|---|---|---|
| **CHAOS-1 (P1)** — audit chain not advancing after SIGKILL+restart | [#218](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/issues/218) | [#227](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/227) — root cause: `daemon_start` helper was `rm -f`'ing the DB on restart (test-infra bug, not server bug); fix splits `daemon_db_reset` from `daemon_start` | scenario 01 PASS |
| **CHAOS-2 (P0-SECURITY)** — server signed receipts for expired APRPs | [#219](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/issues/219) | [#226](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/226) — `EXPIRY_SKEW_SECS = 60` tolerance + `protocol.aprp_expired` 400 with RFC 7807 problem-detail body; rejection happens pre-pipeline (before nonce claim) | scenario 05 PASS (after fixture margin bumped from `-60s` → `-120s` to clear the boundary cleanly) |

## Why 5/5 PASS matters

Per the v1.2.0 release runbook (`docs/release/v1.2.0-prep.md`), the chaos suite is a hard gate: **failure of any scenario blocks the tag**. With all 5 scenarios green:

- Audit-chain integrity holds across restart (CHAOS-1)
- Tamper-evidence holds at the strict-hash layer (chaos-02)
- Idempotency state machine holds under sponsor partition (chaos-03)
- Idempotency state machine holds under 50× concurrent same-key load (chaos-04)
- APRP `expiry` enforcement holds — load-bearing identity claim per `01-identity.md` (chaos-05)

Combined with the regression sweep on main (cargo `441/441`, 13/13 demo gates, 26/0/1 production-shaped runner), the v1.2.0 tag has a clean signal-to-publish.

## Reproducibility

```bash
cargo build --release -p sbo3l-server
SBO3L_SERVER_BIN=target/release/sbo3l-server bash scripts/chaos/run-all.sh
# expect: 5/5 PASS in scripts/chaos/artifacts/summary.txt
```

Per-scenario artifacts at `scripts/chaos/artifacts/<id>/{result.txt, before.json, after.json}`. The `daemon.log` files are gitignored (run-specific noise; the result.txt is the canonical record).

## Phase 1 exit-gate "8/8 adversarial fail-closed" → now "9/9"

The Phase 1 exit gate's adversarial test claim was 8/8: empty body, unknown field, nonce replay, prompt-injection, oversized payload, idempotency same/same + same/diff, audit-chain tamper. The CHAOS-2 fix adds a 9th: **expiry-skew rejection**. Worth updating the exit-gate doc on the next pass.
