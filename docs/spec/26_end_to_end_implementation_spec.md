# End-to-End Implementation Spec

> **Účel:** Tento súbor je exekučný plán medzi dokumentáciou a produkčnou implementáciou. Keď niekto otvorí repo a chce začať kódovať, toto je poradie práce, definícia hotovo a mapa na normatívne kontrakty.

---

## 1. Source-of-truth pravidlo

Implementácia sa riadi v tomto poradí:

1. `17_interface_contracts.md` pre wire contracts, error kódy, layout a forbidden patterns.
2. `/schemas/*.json` pre JSON/YAML validačné pravidlá.
3. `docs/api/openapi.json` pre REST API, SDK a contract tests.
4. `test-corpus/` pre golden/adversarial vstupy.
5. `12_backlog.md` a `16_demo_acceptance.md` pre implementačné stories a demo gates.
6. `29_two_developer_execution_plan.md` pre aktualny two-developer execution plan.
7. `28_ethglobal_openagents_pivot.md` pre primary Open Agents sponsor package.
8. `25_ethprague_sponsor_winning_demo.md` pre secondary ETHPrague stage narrative.

Ak je medzi súbormi rozpor, implementátor musí zastaviť danú story a opraviť dokumentáciu v tom istom PR pred kódom.

---

## 2. Production baseline

MVP môže bežať s `encrypted_file` signerom iba v development móde. Produkčný release musí spĺňať:

- `server.mode = "production"`.
- `signing.allow_dev_key = false`.
- `default_backend` je `tpm`, `hsm_pkcs11`, `yubihsm_native`, `nitrokey_native`, `tee_sealed` alebo `smart_account_session`.
- Unix socket má `0600` a vlastníka `vault`.
- TCP listener je vypnutý alebo binduje iba loopback.
- Audit hash chain verifier prejde na poslednom evente.
- Migrácie sú forward-only a startup odmietne zmenený hash aplikovanej migrácie.
- Policy mutation vyžaduje admin signature a generuje audit event.
- Každý mutating endpoint podporuje `Idempotency-Key`.

---

## 3. Implementation order

### Stage A - repository and contracts

Build:

- Rust workspace root.
- `sbo3l-core` with config, error catalog, APRP types, canonical hashing.
- JSON schema validation against `/schemas`.
- Contract tests that load `test-corpus`.
- OpenAPI lint for `docs/api/openapi.json`.

Exit:

- `D-P0-01..05`.
- `D-P1-01..03`.
- All JSON files parse.
- Unknown APRP fields fail with `schema.unknown_field`.

### Stage B - local payment path

Build:

- Unix socket and loopback REST server.
- Agent identity registry.
- Decision token signer/verifier.
- `encrypted_file` dev signer.
- Mock x402 provider.

Exit:

- `D-P1-04..13`.
- `POST /v1/payment-requests` works from Python and TypeScript SDK examples.
- Signer refuses all requests without a valid decision token.

### Stage C - policy, budgets, and audit

Build:

- Policy YAML/JSON loader from `schemas/policy_v1.json`.
- Rego evaluator.
- Atomic budget reserve/commit/release.
- Hash-chained signed audit events.
- Emergency freeze and per-agent pause.

Exit:

- All primary P2 and P3 gates.
- `test-corpus/aprp/deny_prompt_injection_request.json` is schema-valid but policy-denied with `policy.deny_recipient_not_allowlisted`.
- Audit verifier detects a one-byte tamper.

### Stage D - real payments and production hardening

Build:

- Real x402 parser/verifier.
- Base Sepolia live payment path.
- RPC quorum and tx simulation.
- HSM/TPM backend and production lint.
- AppArmor/systemd hardening.

Exit:

- P4 and P5 gates.
- Production config refuses dev signer.
- Hardware signer or SoftHSM parity test passes in CI.

### Stage E - governance and attestation

Build:

- Web UI for approvals, budgets, audit, emergency.
- Push/Telegram optional notification adapters.
- TEE attestation evidence generation and verification.
- Runtime measurement drift response.

Exit:

- P6 and P7 gates.
- Attestation is linked to audit events.
- Human approval TTL and M-of-N signatures are enforced.

### Stage F - Open Agents hero and release

Build:

- `demo-agents/research-agent` real-agent harness.
- Sponsor scripts under `demo-scripts/sponsors/`.
- Red-team scripts under `demo-scripts/red-team/`.
- ENS identity proof.
- KeeperHub guarded execution adapter.
- Uniswap guarded swap adapter if time permits.
- Policy receipt + trust badge.
- Optional on-chain session key / attested module / audit anchor path for ETHPrague secondary package.
- Packaging, runbooks, security review, release docs.

Exit:

- Open Agents final demo runs end-to-end.
- `D-P8-11` runs with the real agent harness, not only a static request file.
- Final 3-minute demo works offline except optional live sponsor integrations.

---

## 4. Parallel agent work rules

- One worker owns one crate/module family at a time.
- No worker may invent a new JSON shape outside `/schemas`.
- No worker may add an error code outside `17_interface_contracts.md §3.1`.
- No worker may add a table outside `/migrations`.
- Any new demo must be addressable as `bash demo-scripts/run-single.sh <DEMO_ID>`.
- Every PR must state: changed modules, contracts touched, demo gates passed, and known deviations.

---

## 5. Required first implementation tickets

These are the first tickets before feature work starts:

| ID | Work | Files |
|---|---|---|
| S0-01 | Validate all JSON schemas and OpenAPI in CI | `/schemas`, `/docs/api/openapi.json`, `/.github/workflows/ci.yml` |
| S0-02 | Generate Rust APRP types or write strict serde types | `/schemas/aprp_v1.json`, `/crates/sbo3l-core/src/protocol/` |
| S0-03 | Add golden/adversarial corpus runner | `/test-corpus`, `/crates/sbo3l-core/tests/` |
| S0-04 | Lock `agent_id` regex and DB uniqueness | `/schemas/aprp_v1.json`, `/migrations/V001__init.sql` |
| S0-05 | Implement RFC 7807 problem mapping | `/crates/sbo3l-core/src/error.rs`, `docs/api/openapi.json` |
| S0-06 | Implement real-agent demo harness shell | `/demo-agents/research-agent/`, `/demo-scripts/run-single.sh` |

---

## 6. Production implementation checklist

- [ ] CI runs fmt, clippy, tests, schema validation, OpenAPI lint, cargo audit.
- [ ] All money values are string decimals parsed with exact decimal semantics.
- [ ] `.unwrap()` is forbidden outside tests.
- [ ] `serde(deny_unknown_fields)` or equivalent exists on all wire structs.
- [ ] All migrations replay from empty DB.
- [ ] All mutating APIs generate audit events.
- [ ] All signer calls require a valid decision token.
- [ ] Emergency freeze blocks new payments within the configured latency target.
- [ ] Key material never appears in logs, metrics, panic messages, or audit payloads.
- [ ] Production config refuses dev signing backends.
- [ ] Release artifact has checksum, provenance, and install docs.

---

## 7. Open Agents acceptance overlay

For the primary hackathon branch, the minimum winning build is:

1. Real research agent harness.
2. ENS identity proof for the agent.
3. Legit payment/action request approved.
4. Approved action routed to KeeperHub or sponsor-native execution mock.
5. Prompt-injection request denied before execution.
6. Signed policy receipt visible.
7. Hash-chained audit visible and verifiable.

Demo shortcuts are allowed only behind explicit `demo` config and must fail production lint.

---

## 8. ETHPrague acceptance overlay

For the secondary ETHPrague package, the minimum winning build is:

1. Real research agent harness.
2. Legit x402 request approved.
3. Prompt-injection request denied.
4. Hash-chained audit visible.
5. Tamper detection visible.
6. Kill switch visible.
7. Sponsor-specific callouts for x402/Base, account abstraction, hardware isolation, privacy, and ENS identity.

This overlay must not weaken production safety. Demo shortcuts are allowed only behind explicit `demo` config and must fail production lint.
