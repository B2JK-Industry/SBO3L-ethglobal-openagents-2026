# Demo fixtures

Two kinds of fixture live here:

1. **Pre-existing demo fixtures** — consumed directly by the demo runner
   (`demo-scripts/run-openagents-final.sh`) and the per-sponsor scripts.
2. **Production-shaped mock fixtures** — added in B3 to demonstrate the
   *shape* of integrations that SBO3L would consume against real
   backends, with deterministic local data, no secrets, no live URLs,
   and no production claim. These fixtures are not yet wired into any
   runner; they are reference material for adapter authors and reviewers.

## Pre-existing demo fixtures

| Path | Purpose |
|---|---|
| `ens-records.json` | Single-agent ENS text-record fixture consumed by `sbo3l_identity::OfflineEnsResolver` in the 13-gate demo (gate 7). |
| `uniswap/swap-policy.json` | Swap-policy parameters (token allowlist, max notional, max slippage, treasury recipient) consumed by `sbo3l_execution::uniswap::evaluate_swap`. |
| `uniswap/sbo3l-policy.json` | Swap-aware SBO3L policy used by gate 9 of the demo. |
| `uniswap/quote-USDC-ETH.json` | Bounded happy-path Uniswap quote fixture. |
| `uniswap/quote-USDC-RUG.json` | Adversarial rug-token quote fixture (used to exercise the swap-policy + SBO3L deny path). |

## Production-shaped mock fixtures (B3)

Four JSON files demonstrating live-shape integrations against deterministic
local data. Each carries an envelope that the validation test enforces.

For the **per-surface mock-to-live transition** (env vars, endpoints,
credentials, code change, verification, and truthfulness invariants),
see [`docs/production-transition-checklist.md`](../docs/production-transition-checklist.md).
Each fixture also has a sibling `.md` file in this directory documenting
what it demonstrates, what live system it stands in for, and the exact
replacement step.

```json
{
  "schema":            "sbo3l-mock-<surface>-v1",
  "mock":              true,
  "explanation":       "<≥40 chars: what this fixture demonstrates>",
  "live_replacement":  "<≥40 chars: what the live integration would replace this with>",
  ...
}
```

| Fixture | Per-fixture doc | Surface | Runner that consumes the live equivalent | Where the live transition lives in [`production-transition-checklist.md`](../docs/production-transition-checklist.md) |
|---|---|---|---|---|
| [`mock-ens-registry.json`](mock-ens-registry.json) | [`mock-ens-registry.md`](mock-ens-registry.md) | ENS text-record registry across multiple agent identities (catalogue view). The single-agent runtime input today is [`ens-records.json`](ens-records.json), consumed only by the per-sponsor ENS script invoked via gate 7 of [`demo-scripts/run-openagents-final.sh`](../demo-scripts/run-openagents-final.sh); the production-shaped runner does **not** load any ENS resolver. This catalogue is therefore reference-only today. | [`run-openagents-final.sh`](../demo-scripts/run-openagents-final.sh) gate 7 (via `OfflineEnsResolver` today; future `LiveEnsResolver` would consume live ENS records under `SBO3L_ENS_LIVE=1`). | [§ ENS resolver](../docs/production-transition-checklist.md#ens-resolver) |
| [`mock-keeperhub-sandbox.json`](mock-keeperhub-sandbox.json) | [`mock-keeperhub-sandbox.md`](mock-keeperhub-sandbox.md) | KeeperHub workflow webhook submit/result envelopes (success / idempotency-conflict / not-approved-local / lookup-status). | [`demo-scripts/sponsors/keeperhub-guarded-execution.sh`](../demo-scripts/sponsors/keeperhub-guarded-execution.sh) (today: `KeeperHubExecutor::local_mock()`; future: `KeeperHubExecutor::live()` under `SBO3L_KEEPERHUB_LIVE=1`; the upstream live-integration asks live in [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md)). | [§ KeeperHub guarded execution](../docs/production-transition-checklist.md#keeperhub-guarded-execution) |
| [`mock-uniswap-quotes.json`](mock-uniswap-quotes.json) | [`mock-uniswap-quotes.md`](mock-uniswap-quotes.md) | Uniswap quote catalogue (happy path, slippage violation, recipient-allowlist violation) shaped for the swap-policy guard. | [`demo-scripts/sponsors/uniswap-guarded-swap.sh`](../demo-scripts/sponsors/uniswap-guarded-swap.sh) gate 9 (today: `UniswapExecutor::local_mock()`; future: `UniswapExecutor::live()` under `SBO3L_UNISWAP_LIVE=1`). | [§ Uniswap guarded swap](../docs/production-transition-checklist.md#uniswap-guarded-swap) |
| [`mock-kms-keys.json`](mock-kms-keys.json) | [`mock-kms-keys.md`](mock-kms-keys.md) | Public verification-key metadata (Ed25519) for SBO3L's two demo signers — same deterministic dev seeds the production-shaped runner uses. | [`demo-scripts/run-production-shaped-mock.sh`](../demo-scripts/run-production-shaped-mock.sh) step 9 (today: hardcoded constants; future: `sbo3l key list --mock --format json` under PSM-A1.9). | [§ Signer / Mock-KMS / HSM](../docs/production-transition-checklist.md#signer--mock-kms--hsm) |

### Truthfulness invariants enforced by the test

`python3 demo-fixtures/test_fixtures.py` (stdlib-only, mirrors
`trust-badge/test_build.py` and `operator-console/test_build.py`)
asserts the following for every `mock-*.json` in this directory:

- parses as JSON
- declares the four envelope fields above
- `schema` matches `^sbo3l-mock-[a-z0-9-]+-v\d+$`
- `mock` is exactly `true`
- `explanation` and `live_replacement` are non-empty (≥ 40 chars each)
- contains no http/https URL outside the safe set: RFC 2606 reserved
  hostnames (`example.*`, `*.invalid`, `localhost`), `127.0.0.1`, and
  the existing `schemas.sbo3l.dev` $id pattern
- contains no secret-looking strings: PEM private-key blocks,
  `private_key`/`signing_key`/`seed_hex`/`seed_bytes` fields with hex
  values, `kh_*` or `wfb_*` workflow tokens
- if the fixture sets `no_private_material: true`, no
  `signing_key_hex`/`private_key_hex`/`seed_hex`/`seed_bytes_hex` fields
  carrying ≥ 32-hex-char values appear anywhere in the document

Falsifiable: a future PR that drops a `mock: true`, leaks a `kh_*`
token, or smuggles in a real URL fails the test loudly.

### Run

```bash
python3 demo-fixtures/test_fixtures.py
```

### Honest scope

- **No runner wiring in B3.** These fixtures are reference material for
  adapter authors and reviewers; the production-shaped runner does not
  yet consume them. Wiring them in is a candidate for a follow-up B-side
  PR (B3.v2 if useful, or it may stay reference-only).
- **No transcript schema bump.** The trust badge and operator console
  continue to consume `sbo3l-demo-summary-v1` unchanged.
- **No secrets.** Every fixture is committed in the open repo and is
  meant to be inspected line by line. The two `verifying_key_hex`
  values in `mock-kms-keys.json` are public Ed25519 verification keys
  derived from public seeds already in `crates/sbo3l-server/src/lib.rs`.
- **No production claim.** Every fixture says `mock: true` and points at
  what would replace it in a real deployment.
