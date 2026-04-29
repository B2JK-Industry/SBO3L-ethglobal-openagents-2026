# `mock-ens-registry.json` — production-shaped ENS registry mock

A multi-agent ENS text-record catalogue, shaped exactly like the data
SBO3L's `sbo3l_identity::EnsResolver` consumes when resolving an
agent's `sbo3l:*` text records. **This is fixture data — no real ENS
registrations are involved.**

## What it demonstrates

- The `sbo3l:*` text-record convention (`sbo3l:agent_id`,
  `sbo3l:endpoint`, `sbo3l:policy_hash`, `sbo3l:audit_root`,
  `sbo3l:receipt_schema`) used by SBO3L to bind an ENS name to its
  SBO3L identity, declared policy hash, and receipt schema.
- The **catalogue shape** for multi-agent deployments — three
  deterministic agent identities (`research-agent`, `trading-agent`,
  `support-agent`) so adapter authors can see what the resolver returns
  beyond the single-agent case in
  [`ens-records.json`](ens-records.json) (which is consumed by the
  13-gate demo today).
- The convention that `sbo3l:audit_root` is a 32-byte (64-hex)
  audit-chain root anchor, and that `sbo3l:policy_hash` is a 32-byte
  canonical policy hash matching the active SBO3L policy.

## What live system it stands in for

A live ENS resolver against mainnet or Sepolia testnet, reading text
records from the public ENS Registry + Public Resolver contracts.
The `EnsResolver` trait in `crates/sbo3l-identity/` already
abstracts the backend; the in-tree `OfflineEnsResolver` is the offline
fixture-driven implementation, and a future `LiveEnsResolver` would
implement the same trait against a JSON-RPC endpoint.

## Exact replacement step

1. Add a new resolver implementation alongside `OfflineEnsResolver`,
   e.g. `crates/sbo3l-identity/src/live.rs::LiveEnsResolver`,
   implementing `EnsResolver` against the public ENS Registry +
   Public Resolver contracts via JSON-RPC.
2. Configure the resolver via env vars:
   - `SBO3L_ENS_RPC_URL` — the JSON-RPC endpoint
     (e.g. an Infura / Alchemy / self-hosted node URL).
   - `SBO3L_ENS_NETWORK` — `mainnet` | `sepolia` | `holesky`.
3. Switch `AppState` (or the demo harness) to construct
   `LiveEnsResolver::new(...)` instead of
   `OfflineEnsResolver::from_file(...)` when the env vars are present;
   default remains `OfflineEnsResolver` so offline behaviour is
   preserved when no env vars are set.
4. Test discipline: live ENS reads must never run in CI; gate them
   behind a `SBO3L_ENS_LIVE=1` operator-side flag, paralleling the
   `SBO3L_KEEPERHUB_LIVE=1` and `SBO3L_UNISWAP_LIVE=1` patterns
   captured in
   [`docs/production-transition-checklist.md`](../docs/production-transition-checklist.md).
5. Update `demo-fixtures/README.md`'s ENS row of the production
   transition checklist to mark live as **shipped** once the trait
   implementation lands.

See
[`docs/production-transition-checklist.md` § ENS](../docs/production-transition-checklist.md#ens-resolver)
for the env-var / endpoint / credentials matrix.

## Truthfulness invariants

- Every entry in `registry` has the five `sbo3l:*` text records.
- All `sbo3l:audit_root` values are zero-filled (placeholder root, not
  real chain state).
- All `sbo3l:endpoint` URLs are loopback (`http://127.0.0.1:*`) — no
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
