# [WIP] Implement Mandate ETHGlobal Open Agents vertical

> Draft / work-in-progress. Opened early per the orchestrator workflow so reviewers
> (and Codex) can see the foundations as the rest of the vertical lands. The PR
> is **not** ready to merge — see [Status](#status) and [Pending slices](#pending-slices).

## Summary

Mandate is a local policy, budget, receipt and audit firewall for AI agents.

> Don't give your agent a wallet. Give it a mandate.

This branch lays the foundations: a strict APRP wire format, JCS canonical
request hashing, JSON Schema validation, an Ed25519 dev signer, signed policy
receipts and signed decision tokens. The remaining work is the policy engine,
storage, HTTP API, real research-agent harness and sponsor adapters. CI is
green for everything that exists today.

## Status

| Slice | State |
|---|---|
| Repo bootstrap, README, AI_USAGE, SUBMISSION_NOTES, FEEDBACK | done |
| Planning artifacts under `docs/spec/` (verbatim from pre-hackathon repo) | done |
| Rust workspace + 8 crate skeletons | done |
| `mandate` CLI: `aprp validate`, `aprp hash`, `aprp run-corpus`, `schema`, `verify-audit` (stub) | done |
| APRP v1 types with `deny_unknown_fields` + serde round-trip | done |
| JCS canonical hashing (locked golden hash `c0bd2fab…`) | done |
| JSON Schema validation with embedded schemas + local refs | done |
| Ed25519 dev signer (deterministic seed support) | done |
| Policy receipt v1: sign + verify + schema check | done |
| Decision token v1: sign + verify + schema check | done |
| CI: `fmt`, `clippy -D warnings`, `test`, schema/OpenAPI validators | done, green |
| Partial `demo-scripts/run-openagents-final.sh` runner | done |
| Policy YAML model + rule evaluator + budget tracker | **pending** |
| SQLite storage + hash-chained audit log | **pending** |
| Payment-request HTTP API | **pending** |
| Research-agent harness (legit-x402, prompt-injection) | **pending** |
| ENS identity adapter | **pending** |
| KeeperHub guarded-execution adapter | **pending** |
| Uniswap guarded-swap adapter (stretch) | **pending** |
| Final demo runner with full sponsor flow | **pending** |
| Codex review, feedback addressed | **pending** |

## CI

Latest run: **success** on commit `e6042ef` —
[run 25019731298](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/actions/runs/25019731298)

- ✅ `Rust check`: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets` (23 unit tests pass)
- ✅ `Validate JSON schemas / OpenAPI`: metaschemas pass, test corpus validates with a local `referencing.Registry` (mapping each `$id` to its file in `schemas/`), OpenAPI validates with the file's URI as base.

Earlier runs in this branch failed because the Python validators tried to fetch the canonical schema URLs (`https://schemas.mandate.dev/x402/v1.json`) over the network. The fix is in `scripts/validate_schemas.py` (registry) and `scripts/validate_openapi.py` (base URI). The schema `$id` values were left untouched — they are correct for the published contract.

## Demo

```bash
bash demo-scripts/run-openagents-final.sh
```

Currently runs the gates that are wired and clearly labels what's still pending. Sample output:

```
1. Build CLI                ok cargo build --bin mandate
2. APRP schema gate         ok golden + adversarial + corpus
3. Locked golden APRP hash  ok c0bd2fab4a7d4686d686edcc9c8356315cd66b820a2072493bf758a1eeb500db
4. Pending slices           TODO policy engine, storage, HTTP API, harness, ENS, KeeperHub, Uniswap
```

Once each pending slice lands, the runner is extended in the same commit so the demo always reflects what's truly green.

## Tests run locally on this branch

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets             # 23 pass
python scripts/validate_schemas.py               # 6 schemas, 4 corpus fixtures
python scripts/validate_openapi.py               # docs/api/openapi.json
bash demo-scripts/run-openagents-final.sh        # partial
```

## Known limitations / blockers

- **No live policy or budget enforcement yet** — the receipts and decision tokens compile and verify, but the engine that decides allow/deny is the next slice.
- **No persistent storage yet** — receipts are in-memory only. The audit hash chain test fixture is read-only at the moment.
- **No real ENS / KeeperHub / Uniswap calls yet** — adapters are skeleton crates.
- **Production hardening out of scope** — TEE attestation, HSM signers and TPM are explicitly post-hackathon (see `docs/spec/29_two_developer_execution_plan.md` §6).

## ETHGlobal compliance notes

- Public repo created at hackathon start, history begins with first commit on `main` (`init Mandate hackathon repo`) — see `docs/spec/30_ethglobal_submission_compliance.md` §2-§3.
- Pre-hackathon planning artifacts live under `docs/spec/` and are clearly attributed in `README.md` and `AI_USAGE.md`.
- Frequent, focused commits — no giant final dump.
- AI usage disclosed in `AI_USAGE.md`.
- Partner prizes and "live vs mocked" disclosure in `SUBMISSION_NOTES.md`.

## AI usage

See [`AI_USAGE.md`](AI_USAGE.md). Coding assistants (Claude Code) used for scaffolding, schema validation, signer/receipt code and CI; planning/architecture/threat model authored before the hackathon.

## Partner integrations targeted

1. **KeeperHub** — guarded execution (Mandate decides, KeeperHub executes).
2. **ENS** — agent identity + policy/audit metadata via text records.
3. **Uniswap** — guarded swap (stretch).

Only "live" or faithfully-disclosed local mocks will be claimed in the final submission.

## Next exact commits

1. Policy YAML model + Rust-side rule evaluator (`mandate-policy`).
2. Budget tracker (per_tx / daily / per_provider) backed by SQLite (`mandate-storage`).
3. Hash-chained audit log + verifier (`mandate-storage`, `mandate verify-audit` CLI).
4. `POST /v1/payment-requests` HTTP route in `mandate-server` driving the full pipeline.
5. Research-agent harness with `legit-x402` and `prompt-injection` scenarios.
6. ENS identity adapter (testnet + offline fixture).
7. KeeperHub adapter (live MCP/API where possible, faithful mock otherwise).
8. End-to-end demo runner replacing the partial runner.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
