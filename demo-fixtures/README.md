# Demo fixtures

Two kinds of fixture live here:

1. **Pre-existing demo fixtures** — consumed directly by the demo runner
   (`demo-scripts/run-openagents-final.sh`) and the per-sponsor scripts.
2. **Production-shaped mock fixtures** — added in B3 to demonstrate the
   *shape* of integrations that Mandate would consume against real
   backends, with deterministic local data, no secrets, no live URLs,
   and no production claim. These fixtures are not yet wired into any
   runner; they are reference material for adapter authors and reviewers.

## Pre-existing demo fixtures

| Path | Purpose |
|---|---|
| `ens-records.json` | Single-agent ENS text-record fixture consumed by `mandate_identity::OfflineEnsResolver` in the 13-gate demo (gate 7). |
| `uniswap/swap-policy.json` | Swap-policy parameters (token allowlist, max notional, max slippage, treasury recipient) consumed by `mandate_execution::uniswap::evaluate_swap`. |
| `uniswap/mandate-policy.json` | Swap-aware Mandate policy used by gate 9 of the demo. |
| `uniswap/quote-USDC-ETH.json` | Bounded happy-path Uniswap quote fixture. |
| `uniswap/quote-USDC-RUG.json` | Adversarial rug-token quote fixture (used to exercise the swap-policy + Mandate deny path). |

## Production-shaped mock fixtures (B3)

Four JSON files demonstrating live-shape integrations against deterministic
local data. Each carries an envelope that the validation test enforces:

```json
{
  "schema":            "mandate-mock-<surface>-v1",
  "mock":              true,
  "explanation":       "<≥40 chars: what this fixture demonstrates>",
  "live_replacement":  "<≥40 chars: what the live integration would replace this with>",
  ...
}
```

| Fixture | Surface | What live integration would replace it |
|---|---|---|
| [`mock-ens-registry.json`](mock-ens-registry.json) | ENS text-record registry across multiple agent identities (catalogue view) | Live ENS resolver against mainnet/Sepolia text records via the public Registry + Public Resolver contracts. The `mandate_identity::EnsResolver` trait already abstracts this; switching is a constructor swap. |
| [`mock-keeperhub-sandbox.json`](mock-keeperhub-sandbox.json) | KeeperHub workflow webhook submit/result envelopes (success / idempotency-conflict / not-approved-local / lookup-status) | Real KeeperHub workflow webhook responses once a public submission/result schema and credentials are available. See [`docs/keeperhub-live-spike.md`](../docs/keeperhub-live-spike.md) and FEEDBACK.md §KeeperHub. |
| [`mock-uniswap-quotes.json`](mock-uniswap-quotes.json) | Uniswap quote catalogue (happy path, slippage violation, recipient-allowlist violation) shaped for the swap-policy guard | Live Uniswap Trading API quote endpoint. `UniswapExecutor::live()` is intentionally stubbed (`BackendOffline`) until that wiring lands. |
| [`mock-kms-keys.json`](mock-kms-keys.json) | Public verification-key metadata (Ed25519) for Mandate's two demo signers — same deterministic dev seeds the production-shaped runner uses | Real KMS / HSM key-listing API output (AWS KMS `ListKeys`+`GetPublicKey`, GCP KMS, Azure Key Vault, or HSM). Production deployments inject signers via `AppState::with_signers`. |

### Truthfulness invariants enforced by the test

`python3 demo-fixtures/test_fixtures.py` (stdlib-only, mirrors
`trust-badge/test_build.py` and `operator-console/test_build.py`)
asserts the following for every `mock-*.json` in this directory:

- parses as JSON
- declares the four envelope fields above
- `schema` matches `^mandate-mock-[a-z0-9-]+-v\d+$`
- `mock` is exactly `true`
- `explanation` and `live_replacement` are non-empty (≥ 40 chars each)
- contains no http/https URL outside the safe set: RFC 2606 reserved
  hostnames (`example.*`, `*.invalid`, `localhost`), `127.0.0.1`, and
  the existing `schemas.mandate.dev` $id pattern
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
  continue to consume `mandate-demo-summary-v1` unchanged.
- **No secrets.** Every fixture is committed in the open repo and is
  meant to be inspected line by line. The two `verifying_key_hex`
  values in `mock-kms-keys.json` are public Ed25519 verification keys
  derived from public seeds already in `crates/mandate-server/src/lib.rs`.
- **No production claim.** Every fixture says `mock: true` and points at
  what would replace it in a real deployment.
