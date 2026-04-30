# `sbo3l passport` — proof-carrying execution capsules

> *Local production-shaped lifecycle, not remote governance.*

`sbo3l passport {run, verify, explain}` (Passport P1.1 + P2.1) is the operator-facing surface for the SBO3L Passport capsule — the portable, offline-verifiable proof artifact that wraps one SBO3L decision plus its surrounding identity, request, policy, execution, audit, and verification context. Schema and source-of-truth doc:

- `schemas/sbo3l.passport_capsule.v1.json` — the wire-format contract.
- `docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md` — the product-level definition.

The CLI **wraps** existing SBO3L primitives — APRP, PolicyReceipt, SignedAuditEvent, AuditCheckpoint, ENS records, mock executors. It does NOT redefine them and it does NOT reimplement cryptography, audit-chain semantics, or the policy engine. Live integration is intentionally out of scope for P2.1; `--mode live` is rejected with exit 2.

## Subcommands

### `sbo3l passport run <APRP> --db <PATH> --agent <ENS> --resolver offline-fixture --ens-fixture <PATH> --executor {keeperhub,uniswap} --mode mock --out <PATH>`

Drives the existing offline pipeline end-to-end and emits a `sbo3l.passport_capsule.v1` JSON to `--out`:

1. Load APRP from `<APRP>`.
2. Look up the active policy from `<DB>` via PSM-A3's `Storage::policy_current`.
3. Resolve the agent's ENS records via `OfflineEnsResolver::from_file(<ens_fixture>)`.
4. Build an in-process `sbo3l-server` `AppState`, drive the request through `POST /v1/payment-requests` via the same `tower::oneshot` pattern the research-agent harness uses (no daemon).
5. Allow path → call the mock executor (`KeeperHubExecutor::local_mock` or `UniswapExecutor::local_mock`) and record `execution_ref` (`kh-<ULID>` / `uni-<ULID>`).
6. Deny path → executor is **never** called; `execution.status = "not_called"`, `execution.execution_ref = null`. This is the hard truthfulness rule from P1.1 (tampered_001 fixture).
7. Reopen storage, look up the just-appended audit event, create + persist a checkpoint via PSM-A4's `Storage::audit_checkpoint_create`.
8. Compose the capsule per schema; **self-verify** before writing — refuses to emit a capsule that wouldn't pass `passport verify`.
9. Atomic write to `<--out>` (tempfile in same directory + rename). A reader who opens the path mid-write either sees the prior contents or the complete new file — never a half-written JSON.

| Exit | Meaning |
| --- | --- |
| 0 | Capsule emitted to `<--out>`. |
| 1 | IO / parse failure (file missing, executor IO error, capsule write failure). |
| 2 | Invalid input (bad APRP, ENS resolution failed, `--mode live`, no active policy, capsule self-verify failed). |

#### Truthfulness rules enforced by `passport run`

- `--mode live` is rejected before any work is done. Live integration lands in P5.1 / P6.1 with concrete credentials + `live_evidence`.
- Deny capsules **always** carry `execution.status = "not_called"` and `execution.execution_ref = null` regardless of the supplied `--executor`. The executor is not invoked at all on the deny path.
- Mock-mode capsules carry `execution.live_evidence = null`. The schema's `live_evidence.minProperties: 1` constraint additionally rejects an empty `{}` object (closing the "live with empty evidence" loophole).
- The capsule's `audit.checkpoint.mock_anchor` is `true` (schema-locked `const true`). PSM-A4's `mock_anchor_ref` (`local-mock-anchor-<16 hex>`) flows through verbatim — no onchain claim.
- The CLI re-derives the capsule and **self-verifies** before writing. If the assembled capsule fails either schema validation or any of P1.1's cross-field invariants (request_hash agreement, policy_hash agreement, decision-result agreement, agent_id agreement, audit_event_id agreement, checkpoint↔outer event_hash agreement), it refuses to write the file and exits 2.

### `sbo3l passport verify --path <PATH>` (P1.1)

Structural verification of a capsule JSON. Runs `sbo3l-core::passport::verify_capsule` — the embedded schema followed by 8 cross-field truthfulness invariants. Documented in detail at the source-of-truth doc; unchanged from PR #42 / P1.1.

| Exit | Meaning |
| --- | --- |
| 0 | Capsule verifies (schema + every invariant). |
| 1 | IO / parse failure. |
| 2 | Malformed / tampered / internally inconsistent (with `(capsule.<code>)` in stderr). |

### `sbo3l passport explain --path <PATH> [--json]`

Reads + verifies a capsule, then prints a 6–10 line human summary (or `--json` structured object). On verifier failure exits 2 with the same `(capsule.<code>)` shape as `verify`, so any tooling that branches on verify codes also works for explain.

```text
$ sbo3l passport explain --path artifacts/passport-allow.json
SBO3L Passport — capsule explanation
  agent:        research-agent-01 (research-agent.team.eth), resolver=offline-fixture
  policy:       v1, hash=e044f13c5acb…
  decision:     ALLOW (matched_rule=allow-small-x402-api-call)
  execution:    keeperhub (mode=mock, status=submitted, ref=kh-01KQCETWAHCKRRRJ5YZGVPVDZ6)
  audit:        event_id=evt-01KQCETWAG7Q4G0RDH7W7V443G
  checkpoint:   mock_anchor_ref=local-mock-anchor-93b877470f65596a
  doctor:       not_run, offline-verifiable: yes, live-claims: 0
```

`--json` emits the same content as a small JSON object suitable for piping into `jq` or other static surfaces.

| Exit | Meaning |
| --- | --- |
| 0 | Explanation produced. |
| 1 | IO / parse failure. |
| 2 | Capsule failed verification (same code as `passport verify`). |

## Production-shaped runner integration

`bash demo-scripts/run-production-shaped-mock.sh` step **§10b** (Passport P2.1 — REAL today) emits and verifies two capsules end-to-end against a real audit chain:

- `demo-scripts/artifacts/passport-allow.json` — `legit-x402` ALLOW path with KeeperHub mock executor.
- `demo-scripts/artifacts/passport-deny.json` — `prompt-injection` DENY path; executor never called.

Both round-trip through `passport verify` before the runner moves on. The runner's tally bumps from `24 real / 0 mock / 1 skipped` (post-PSM-A4) to **`26 real / 0 mock / 1 skipped`** (post-Passport-P2.1) — `--include-final-demo` remains the only SKIPPED line.

The 13-gate hackathon demo (`demo-scripts/run-openagents-final.sh`) is **untouched** — Passport surfaces live in the production-shaped runner only.

## Out of scope on this CLI surface (Passport P2.1)

These belong to later Passport phases:

- **`sbo3l passport resolve`** (P2.1+ stretch) — pure ENS-records-only lookup.
- **`sbo3l-mcp` server tools** (P3.1) — `sbo3l.run_guarded_execution`, `sbo3l.verify_capsule`, etc., wrapping the same logic.
- **Live ENS resolver** — `LiveEnsResolver` (in `crates/sbo3l-identity/src/ens_live.rs`) ships on `main`. Operator activates it via `SBO3L_ENS_RPC_URL`. Smoke: `cargo run -p sbo3l-identity --example ens_live_smoke` against `sbo3lagent.eth` (mainnet).
- **Live KeeperHub envelope** (P5.1) — `--mode live` lands here with concrete credentials + `live_evidence`.
- **Uniswap quote evidence in capsule** (P6.1) — quote id / route / freshness / slippage cap captured into the capsule's execution block.
- **Trust-badge / operator-console capsule panels** (P2.2 — Developer B) — the static UI rendering of the capsule.
- **Public proof page** (P7.1) — GitHub Pages hosting of trust-badge + operator-console + selected capsule JSON.
