# `mock-ens-registry.json` — production-shaped ENS registry mock

A multi-agent ENS text-record catalogue, shaped exactly like the data
Mandate's `mandate_identity::EnsResolver` consumes when resolving an
agent's `mandate:*` text records. **This is fixture data — no real ENS
registrations are involved.**

## What it demonstrates

- The `mandate:*` text-record convention (`mandate:agent_id`,
  `mandate:endpoint`, `mandate:policy_hash`, `mandate:audit_root`,
  `mandate:receipt_schema`) used by Mandate to bind an ENS name to its
  Mandate identity, declared policy hash, and receipt schema.
- The **catalogue shape** for multi-agent deployments — three
  deterministic agent identities (`research-agent`, `trading-agent`,
  `support-agent`) so adapter authors can see what the resolver returns
  beyond the single-agent case in
  [`ens-records.json`](ens-records.json) (which is consumed by the
  13-gate demo today).
- The convention that `mandate:audit_root` is a 32-byte (64-hex)
  audit-chain root anchor, and that `mandate:policy_hash` is a 32-byte
  canonical policy hash matching the active Mandate policy.

## What live system it stands in for

A live ENS resolver against mainnet or Sepolia testnet, reading text
records from the public ENS Registry + Public Resolver contracts.
The `EnsResolver` trait in `crates/mandate-identity/` already
abstracts the backend; the in-tree `OfflineEnsResolver` is the offline
fixture-driven implementation, and a future `LiveEnsResolver` would
implement the same trait against a JSON-RPC endpoint.

## Exact replacement step

1. Add a new resolver implementation alongside `OfflineEnsResolver`,
   e.g. `crates/mandate-identity/src/live.rs::LiveEnsResolver`,
   implementing `EnsResolver` against the public ENS Registry +
   Public Resolver contracts via JSON-RPC.
2. Configure the resolver via env vars:
   - `MANDATE_ENS_RPC_URL` — the JSON-RPC endpoint
     (e.g. an Infura / Alchemy / self-hosted node URL).
   - `MANDATE_ENS_NETWORK` — `mainnet` | `sepolia` | `holesky`.
3. Switch `AppState` (or the demo harness) to construct
   `LiveEnsResolver::new(...)` instead of
   `OfflineEnsResolver::from_file(...)` when the env vars are present;
   default remains `OfflineEnsResolver` so offline behaviour is
   preserved when no env vars are set.
4. Test discipline: live ENS reads must never run in CI; gate them
   behind a `MANDATE_ENS_LIVE=1` operator-side flag, paralleling the
   `MANDATE_KEEPERHUB_LIVE=1` and `MANDATE_UNISWAP_LIVE=1` patterns
   captured in
   [`docs/production-transition-checklist.md`](../docs/production-transition-checklist.md).
5. Update `demo-fixtures/README.md`'s ENS row of the production
   transition checklist to mark live as **shipped** once the trait
   implementation lands.

See
[`docs/production-transition-checklist.md` § ENS](../docs/production-transition-checklist.md#ens-resolver)
for the env-var / endpoint / credentials matrix.

## Truthfulness invariants

- Every entry in `registry` has the five `mandate:*` text records.
- All `mandate:audit_root` values are zero-filled (placeholder root, not
  real chain state).
- All `mandate:endpoint` URLs are loopback (`http://127.0.0.1:*`) — no
  real network destination is named.
- The fixture's envelope (`mock: true`, `schema`, `explanation`,
  `live_replacement`) is enforced by
  [`test_fixtures.py`](test_fixtures.py).

## Where this fixture is referenced

- [`README.md`](README.md) §B3 fixtures
- [`test_fixtures.py`](test_fixtures.py) (validator)
- [`../docs/production-transition-checklist.md`](../docs/production-transition-checklist.md) §ENS resolver
- The pre-existing single-agent fixture [`ens-records.json`](ens-records.json)
  is the one consumed by `demo-scripts/run-openagents-final.sh` (gate 7);
  this file is the catalogue shape, not the runtime input.
