# Production transition checklist

This is the single-page operator checklist for flipping each SBO3L
surface from **mock** to **live**. Every section follows the same shape:

1. **Status today** — what runs today, with verifiable evidence.
2. **What live needs** — schema / API / credentials prerequisites.
3. **Env vars / endpoints / credentials** — exact strings.
4. **Code change** — the smallest implementable Rust diff.
5. **Verification** — how to prove live works without trusting the
   adapter author's word.
6. **Truthfulness invariants** — what must remain true after the flip.

Every surface stays mock until the corresponding code change lands AND
its verification step passes against a real backend. **No section in
this document is a claim that any surface is production-ready today.**

The companion docs are:

- [`demo-fixtures/README.md`](../demo-fixtures/README.md) — production-shaped
  mock fixtures + cross-links to runners.
- [`FEEDBACK.md`](../FEEDBACK.md) — partner feedback and live-integration
  asks (KeeperHub, ENS, Uniswap), including the four `sbo3l_*` envelope
  fields and the optional `X-SBO3L-*` response headers that the
  KeeperHub live path would consume.

---

## ENS resolver

### Status today

`sbo3l_identity::OfflineEnsResolver` reads
[`demo-fixtures/ens-records.json`](../demo-fixtures/ens-records.json)
(single-agent today) and matches the published `sbo3l:policy_hash`
against the active SBO3L policy hash. Gate 7 of
`demo-scripts/run-openagents-final.sh` exercises this end-to-end with no
network access. The catalogue shape for multi-agent deployments lives in
[`demo-fixtures/mock-ens-registry.json`](../demo-fixtures/mock-ens-registry.json)
and is documented in
[`mock-ens-registry.md`](../demo-fixtures/mock-ens-registry.md).

### What live needs

- A JSON-RPC endpoint to a node on the target network (mainnet / Sepolia / Holesky).
- The agent's ENS name registered with the `sbo3l:*` text records set
  on its Public Resolver.
- A `LiveEnsResolver` Rust impl alongside `OfflineEnsResolver`.

### Env vars / endpoints / credentials

| Name | Purpose | Example |
|---|---|---|
| `SBO3L_ENS_RPC_URL` | JSON-RPC endpoint | `https://mainnet.infura.io/v3/<API_KEY>` |
| `SBO3L_ENS_NETWORK` | network selector | `mainnet` / `sepolia` / `holesky` |
| `SBO3L_ENS_LIVE` | operator-side gate | `1` (CI never sets it) |

The `<API_KEY>` is operator-provided; **never committed to the repo**.

### Code change

1. New file `crates/sbo3l-identity/src/live.rs` with
   `LiveEnsResolver` impl of `EnsResolver`.
2. New constructor `LiveEnsResolver::from_env()`.
3. Caller code (`AppState`, demo harness, sponsor demo script) selects
   `LiveEnsResolver` when `SBO3L_ENS_LIVE=1` is set; otherwise
   `OfflineEnsResolver` stays default.

### Verification

- Unit test: `LiveEnsResolver` against a `Box<dyn JsonRpcClient>` trait
  with a fake substituted in.
- Operator smoke (gated by `SBO3L_ENS_LIVE=1`, never CI): resolve
  `research-agent.team.eth` against the configured RPC; output matches
  the offline fixture for the same name when both are populated
  identically.

### Truthfulness invariants

- `OfflineEnsResolver` continues to be the default; `SBO3L_ENS_LIVE=1`
  is the only switch.
- The 13-gate demo continues to use the offline resolver — no test
  depends on a live ENS endpoint.
- ENS data drift (live record missing, malformed, or contradicting the
  active policy hash) surfaces as an explicit error, never a fallback
  to "trust the live record".

---

## KeeperHub guarded execution

### Status today

`KeeperHubExecutor::local_mock()` returns a deterministic `kh-<ULID>`
execution_ref and prints `mock: true` in demo output. Denied receipts
are refused **before any I/O** in
`KeeperHubExecutor::execute()` (decision check at the top of the
function). The four `sbo3l_*` envelope fields and the optional
`X-SBO3L-*` response headers are documented in
[`FEEDBACK.md` §KeeperHub](../FEEDBACK.md) and
[`mock-keeperhub-sandbox.md`](../demo-fixtures/mock-keeperhub-sandbox.md).

### What live needs

- A public KeeperHub workflow webhook submission/result schema (the
  schema-publication ask is in [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md)).
- Sandbox / production credentials.
- Answers to the open questions raised in
  [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md): token-prefix naming
  (`kh_*` vs `wfb_*`), `executionId` lookup path, idempotency overlap
  with PSM-A2, the four `sbo3l_*` envelope fields, and the optional
  `X-SBO3L-*` response headers.

### Env vars / endpoints / credentials

| Name | Purpose | Example |
|---|---|---|
| `SBO3L_KEEPERHUB_WEBHOOK_URL` | workflow webhook URL | `https://api.keeperhub.example/v1/workflows/run` |
| `SBO3L_KEEPERHUB_TOKEN` | bearer token (`kh_*` for platform API, `wfb_*` for workflow webhooks — disambiguation request in [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md)) | `wfb_<32+ alphanumerics>` |
| `SBO3L_KEEPERHUB_LIVE` | operator-side gate | `1` (CI never sets it) |

**Never committed to the repo.** `git grep` for
`kh_[A-Za-z0-9]{8,}` / `wfb_[A-Za-z0-9]{8,}` / `KEEPERHUB_TOKEN` /
`KEEPERHUB_API_KEY` under `crates/`, `demo-scripts/`, `demo-fixtures/`,
`test-corpus/` returns zero matches today and must continue to.

### Code change

Summary (the upstream feedback / open-question backing for each item is
in [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md)):

1. `KeeperHubLiveConfig::from_env()` reads the env vars.
2. `KeeperHubMode::Live(KeeperHubLiveConfig)` + `execute_live()`.
3. New `ExecutionError` variants for `Configuration`, `Network`,
   `HttpStatus(u16)`, `Parse`.
4. Unit + integration (in-process HTTP server) tests; never a real
   network call from CI.
5. `demo-scripts/sponsors/keeperhub-guarded-execution.sh` learns
   `SBO3L_KEEPERHUB_LIVE=1`.

### Verification

- Unit tests against a fake `HttpClient` cover happy 2xx, 4xx, 5xx,
  network error, timeout, parse error.
- Integration test asserts the four `sbo3l_*` envelope fields are
  present and match the receipt/audit values, that the `Authorization:
  Bearer ...` header is set from the env var, and that the body is
  JCS-canonical bytes.
- Operator smoke (gated by `SBO3L_KEEPERHUB_LIVE=1`): real network
  call to a real KeeperHub workflow, execution_ref captured into the
  SBO3L audit bundle.

### Truthfulness invariants

- Denied receipts continue to be refused before any I/O.
- Demo runner output continues to label every mock as `mock: true`. The
  live path emits `mock: false` and a real `executionId`, never both.
- No KeeperHub credentials, tokens, secrets, or webhook URLs land in
  the repo. They are environment-only.
- Non-2xx / unparseable response → explicit `ExecutionError`. Never
  silent fallback to `local_mock`.

---

## Uniswap guarded swap

### Status today

`UniswapExecutor::local_mock()` returns a deterministic `uni-<ULID>`
execution_ref. `UniswapExecutor::live()` is intentionally stubbed and
returns `BackendOffline`. The swap-policy guard
(`sbo3l_execution::uniswap::evaluate_swap`) runs **before** any
executor call and is independent of mock vs live. Catalogue documented
in [`mock-uniswap-quotes.md`](../demo-fixtures/mock-uniswap-quotes.md).

### What live needs

- Uniswap Trading API access (or equivalent quote endpoint).
- The four upstream improvements requested in
  [`FEEDBACK.md` §Uniswap](../FEEDBACK.md):
  - **signed quotes** (server-issued `quote_id` + `expires_at` + canonical hash)
  - **route token enumeration** for multi-hop allowlist checks
  - **first-class slippage caps in the request**
- Optionally, a sandbox endpoint that doesn't require funded onchain
  state for the operator smoke test.

### Env vars / endpoints / credentials

| Name | Purpose | Example |
|---|---|---|
| `SBO3L_UNISWAP_API_URL` | Trading API quote endpoint | `https://trade-api.uniswap.example/v1/quote` |
| `SBO3L_UNISWAP_API_KEY` | API key | `<API_KEY>` |
| `SBO3L_UNISWAP_CHAIN` | chain selector | `mainnet` / `base` / `arbitrum` / `optimism` |
| `SBO3L_UNISWAP_LIVE` | operator-side gate | `1` (CI never sets it) |

### Code change

1. `UniswapLiveConfig::from_env()` in
   `crates/sbo3l-execution/src/uniswap.rs`.
2. Replace `UniswapExecutor::live()`'s `BackendOffline` stub with a
   real HTTP GET against the configured endpoint.
3. `demo-scripts/sponsors/uniswap-guarded-swap.sh` learns
   `SBO3L_UNISWAP_LIVE=1`. Default remains `local_mock`.
4. Quote-freshness check tightens once the live API guarantees
   server-issued `expires_at` (today the check is `(relaxed)` against
   the static fixture's `fetched_at_unix`).

### Verification

- Unit tests: `evaluate_swap` against the three
  [`mock-uniswap-quotes.json`](../demo-fixtures/mock-uniswap-quotes.json)
  scenarios (happy / slippage / recipient violation) — already covered
  today.
- Integration test (in-process HTTP server) asserts the request shape
  and parses a sample 200 response.
- Operator smoke (gated by `SBO3L_UNISWAP_LIVE=1`): real Trading API
  call returning a quote that the swap-policy guard then evaluates.

### Truthfulness invariants

- The swap-policy guard runs before any executor call regardless of
  mock vs live.
- The `(relaxed)` quote-freshness flag is visible in demo output today
  and disappears only when live mode + signed quotes ship.
- No Uniswap credentials in the repo.
- Multi-hop quotes touching off-allowlist tokens deny by default.

---

## Signer / Mock-KMS / HSM

### Status today

`AppState::new()` constructs deterministic `DevSigner`s from public
seeds in `crates/sbo3l-server/src/lib.rs:54-55` (`audit-signer-v1` +
`[11u8; 32]`, `decision-signer-v1` + `[7u8; 32]`). These are clearly
labelled `⚠ DEV ONLY ⚠` in the source and in `SUBMISSION_NOTES.md`.
`AppState::with_signers(...)` is the production injection point.

The catalogue shape for the `sbo3l key list --mock` CLI is
documented in
[`mock-kms-keys.json`](../demo-fixtures/mock-kms-keys.json) and its
[companion guide](../demo-fixtures/mock-kms-keys.md).

### What live needs

Stage 1 is **shipped**. Stage 2 is the production-deployment work.

- **Stage 1: Mock-KMS CLI (PSM-A1.9) — DONE in PR #28.** Persistent
  `mock_kms_keys` SQLite table (V005) + `sbo3l key {init,list,rotate} --mock`
  CLI. Every operation requires `--mock` and prefixes every output line
  with `mock-kms:`. **Mock — not production-grade.**
- **Stage 2: production KMS / HSM — pending.** A real `Signer` impl
  backed by AWS KMS, GCP KMS, Azure Key Vault, or an HSM SDK. This is
  what the rest of this section documents.

### Env vars / endpoints / credentials

| Name | Purpose | Example |
|---|---|---|
| `SBO3L_SIGNER_BACKEND` | which signer impl to construct | `dev` (default, `⚠ DEV ONLY ⚠`) / `mock_kms` (PSM-A1.9) / `aws_kms` / `gcp_kms` / `azure_kv` / `hsm` |
| `SBO3L_KMS_REGION` | KMS region (AWS) | `us-east-1` |
| `SBO3L_KMS_ENDPOINT` | KMS endpoint override | `https://kms.us-east-1.amazonaws.com` |
| `SBO3L_AUDIT_SIGNER_KEY_ID` | KMS key id for audit signer | `arn:aws:kms:us-east-1:<acct>:key/<uuid>` |
| `SBO3L_RECEIPT_SIGNER_KEY_ID` | KMS key id for receipt signer | `arn:aws:kms:us-east-1:<acct>:key/<uuid>` |

Provider credentials follow each provider's standard discovery chain
(IAM role / `AWS_*` env vars for AWS, `GOOGLE_APPLICATION_CREDENTIALS`
for GCP, etc.). **Never committed to the repo.**

### Code change

#### Stage 1 (PSM-A1.9 — DONE in PR #28)

Already on `main`. No further code change needed.

- Persistent SQLite-backed keyring in `crates/sbo3l-storage/src/mock_kms_store.rs` + migration `migrations/V005__mock_kms_keys.sql`.
- `sbo3l key {init,list,rotate} --mock --db <path>` CLI in `crates/sbo3l-cli/src/key.rs`.
- `--root-seed` is a CLI input only; it is **never** persisted to SQLite (only the per-version `verifying_key_hex` is). Rotate refuses with exit 2 on a mismatched seed.
- The `sbo3l doctor` `mock_kms_keys` row reports `ok` once V005 is applied.

Pending follow-up (B-side, optional cleanup): teach `demo-scripts/run-production-shaped-mock.sh` step 9 to read the audit / receipt verification pubkeys from `sbo3l key list --mock` output instead of the hardcoded constants. Internal cleanup, not a correctness fix — the runner today exercises the CLI lifecycle in step 3 and uses the existing pubkey constants in step 9.

#### Stage 2 (production KMS / HSM)

1. New `Signer` trait variant per backend.
2. Construct via `AppState::with_signers(...)` from the configured
   backend. `AppState::new()` continues to use deterministic dev seeds
   for offline development.

### Verification

- Stage 1 — `cargo test -p sbo3l-cli` covers the key-list / rotate
  commands; the production-shaped runner's bundle-verify step uses the
  CLI-emitted pubkeys.
- Stage 2 — vendor-specific integration tests (mock SDK clients in
  unit tests; real KMS/HSM in operator smoke only).

### Truthfulness invariants

- `AppState::new()` continues to be deterministic-dev-seed and clearly
  labelled `⚠ DEV ONLY ⚠`.
- Production `verifying_key_hex` values must NEVER equal the two public
  dev pubkeys (`66be7e33...0c473a` and `ea4a6c63...46d22c`). If a CI or
  production deployment ever observes one of those, that is an
  immediate key-management failure — flag and fail loud.
- `sbo3l key list` output for production deployments must NEVER carry
  the `mock: true` flag.

---

## Cross-cutting flip checklist

Before flipping any single surface from mock to live, verify all of:

- [ ] Existing 13-gate `demo-scripts/run-openagents-final.sh` still passes.
- [ ] `python3 demo-fixtures/test_fixtures.py` still passes.
- [ ] No new `http(s)://<real-host>/` URL is committed to any fixture
  under `demo-fixtures/`.
- [ ] No credentials, tokens, secrets, private keys committed anywhere.
  `git grep` for vendor-specific patterns must continue to return zero.
- [ ] The mock path remains a constructor / config option — never
  removed. Operators with no live credentials must continue to be able
  to run the demo offline.
- [ ] Demo output still labels mocks as `mock: true`. Live output
  emits `mock: false` and the real backend identifier.
- [ ] Trust-badge and operator-console schema (`sbo3l-demo-summary-v1`)
  is unchanged unless the live path adds a genuinely new field, in
  which case the schema bump is coordinated across both viewers in the
  same PR.

After flipping any surface, update:

- The relevant `demo-fixtures/mock-*.md` page's "Exact replacement step"
  section to note the live path is shipped (with the merged PR / SHA).
- This file's "Status today" section.
- The runner's REAL/MOCK/SKIP tally in
  `demo-scripts/run-production-shaped-mock.sh`.

---

## What this checklist is NOT

- It is not a claim that any of these flips have happened. Every
  surface stays mock today.
- It is not a substitute for the per-surface design docs. The
  per-fixture `.md` files in `demo-fixtures/` are authoritative for
  fixture shape; [`FEEDBACK.md`](../FEEDBACK.md) is authoritative for
  the partner-facing live-integration asks.
- It is not exhaustive. Production-grade deployments need TLS
  termination, observability, log retention, secret rotation, RBAC,
  and the rest of the operational checklist that lives in
  `docs/spec/29_two_developer_execution_plan.md` (pre-hackathon spec)
  and is out of scope for the hackathon submission.
