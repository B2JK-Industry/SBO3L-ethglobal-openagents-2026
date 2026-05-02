# `sbo3l --backend 0g-storage` — CLI reference

The `0g-storage` backend lets `sbo3l` write audit-chain anchors to the 0G chain and capsule payloads to 0G Storage. This page documents every flag, every env var, and every error mode.

> **Status (2026-05-03):** The CLI flag itself is wired (Dev 1 Task C). The 0G testnet deployment of `Sbo3lAuditAnchor.sol` is pinned by Dev 4 Task C. The browser-upload fallback used by `/proof` is wired by Dev 2 Task C. See [`docs/partner-onepagers/0g.md`](../partner-onepagers/0g.md) for the full integration picture.

## Subcommands that accept `--backend 0g-storage`

| Subcommand | What changes when `--backend 0g-storage` is set |
|---|---|
| `sbo3l audit anchor` | Publishes the audit-chain root to `Sbo3lAuditAnchor.sol` on the 0G testnet chain instead of Sepolia. |
| `sbo3l passport export` | Uploads the bundle (receipt + audit-chain prefix + policy snapshot) to 0G Storage and prints the resulting storage root. |
| `sbo3l audit verify-anchor <tx>` | Fetches the anchor event from the 0G chain and verifies the published root matches the local audit-chain digest. |

## Required configuration

### Env vars (production-shape)

```sh
# 0G chain RPC. Mainnet RPC will be added after the testnet integration
# soaks for ≥ 30 days under the dual-anchor cron in apps/sbo3l-playground-api.
export SBO3L_0G_EVM_RPC="https://evmrpc-testnet.0g.ai"

# 0G Storage gateway. Used by `passport export` for capsule uploads.
export SBO3L_0G_STORAGE_RPC="https://storage-testnet.0g.ai"

# Anchor contract address. Pinned in
# crates/sbo3l-anchor/contracts/deployed.json under key "0g-testnet".
# Override only when verifying against a custom deploy.
export SBO3L_0G_ANCHOR_CONTRACT="0xPINNED_BY_DEV4"

# Publisher private key — the wallet that signs the on-chain anchor
# transaction. MUST hold enough 0G testnet ETH (~0.01 covers a year of
# 6h-cadence anchors at 24K gas each).
export SBO3L_0G_PUBLISHER_PRIVATE_KEY="0x..."
```

### Per-flag overrides

Every env var has a CLI flag equivalent for one-shot use:

```sh
sbo3l audit anchor --backend 0g-storage \
  --rpc https://evmrpc-testnet.0g.ai \
  --contract 0xPINNED_BY_DEV4 \
  --private-key 0x...

sbo3l passport export --capsule receipt.json \
  --backend 0g-storage \
  --storage-rpc https://storage-testnet.0g.ai \
  --out-stdout
```

CLI flags take precedence over env vars. Missing required configuration produces a deterministic error code (`exit 2`) with a message naming the missing variable; SBO3L never silently falls back to a different backend on misconfiguration.

## Common flows

### Anchor the latest audit-chain checkpoint to 0G

```sh
sbo3l audit anchor --backend 0g-storage
# → Anchors the current audit_root to the pinned 0G contract.
# → Prints the on-chain TX hash + block number + chain_length committed.
# → exits 0 on confirmed inclusion (waits for 1 confirmation by default).
```

The `--confirmations N` flag controls how many blocks to wait. Defaults to 1 on testnet; production deploys SHOULD set 6+ for finality assurance.

### Export a Passport bundle to 0G Storage

```sh
sbo3l passport export --capsule receipt.json --backend 0g-storage
# → Uploads the bundle (receipt + audit segment + policy snapshot)
#   to 0G Storage as a single content-addressed object.
# → Prints the storage root (32-byte hex) on stdout.
# → exits 0 on successful upload + retrieval verification.
```

The retrieval verification step is non-optional: after upload, the CLI re-fetches the object by root and recomputes the local hash to confirm round-trip integrity. This catches the SDK chunk-merge issue described in the partner one-pager (`docs/partner-onepagers/0g.md` "Storage SDK timeout caveat") at upload time rather than later.

### Verify an existing anchor

```sh
sbo3l audit verify-anchor 0xANCHOR_TX --backend 0g-storage
# → Fetches the Anchored event from the 0G chain.
# → Recomputes the local audit-chain root for the same chain_length.
# → Exits 0 only if the on-chain root byte-matches the local recomputation.
```

This is the canonical "is the audit chain still consistent with what we anchored" check. A non-zero exit is a tampering signal, not a routine error.

## Error codes

| Code | Meaning | Fix |
|---|---|---|
| 0 | Success | — |
| 1 | Anchor mismatch — local recomputed root differs from on-chain value | Investigate audit-chain integrity; this is a tampering or storage-corruption signal |
| 2 | Missing required configuration (env var or flag) | Set the env var named in the error message |
| 3 | RPC unreachable | Check `SBO3L_0G_EVM_RPC` and 0G testnet status |
| 4 | Storage upload timeout | The known 0G SDK chunk-merge issue. Browser path falls back; CLI path retries 3× then exits 4 |
| 5 | Insufficient publisher balance | Top up the wallet at `SBO3L_0G_PUBLISHER_PRIVATE_KEY`; faucet at https://docs.0g.ai/ |
| 6 | Unknown contract address (not in `deployed.json`) | Pin the address in `crates/sbo3l-anchor/contracts/deployed.json` or pass `--contract` explicitly |

## CI integration

For workspaces that want CI to gate on green 0G anchors:

```yaml
# .github/workflows/audit-anchor-0g.yml
on:
  schedule:
    - cron: "0 */6 * * *"   # every 6h
jobs:
  anchor:
    runs-on: ubuntu-latest
    env:
      SBO3L_0G_EVM_RPC: ${{ secrets.OG_EVM_RPC }}
      SBO3L_0G_PUBLISHER_PRIVATE_KEY: ${{ secrets.OG_PUBLISHER_KEY }}
      SBO3L_0G_ANCHOR_CONTRACT: ${{ vars.OG_ANCHOR_CONTRACT }}
    steps:
      - run: cargo install sbo3l-cli --version 1.x
      - run: sbo3l audit anchor --backend 0g-storage --confirmations 3
```

The dual-anchor cron in `apps/sbo3l-playground-api` (see `DEPLOY.md` Phase 4) runs the same flow against both Sepolia and 0G in parallel, so any single chain outage doesn't break the audit-anchor cadence.

## When to use 0G vs Sepolia

| Use case | Recommended backend |
|---|---|
| Hackathon submission with limited mainnet ETH | Sepolia (cheap, well-understood) |
| Cross-agent reputation flow on 0G ecosystem | 0G (anchor + storage in same trust domain) |
| Production deploy with regulatory audit needs | Both (dual-anchor cron, see above) |
| Demonstrating "works on multiple L2s" | Both |
| Local dev / unit tests | `--backend mock` (deterministic, no network) |

## Compatibility

The `0g-storage` backend coexists with `sepolia` and `mock`:

- A capsule anchored on Sepolia and re-anchored on 0G is the same capsule — `passport verify --strict` accepts the same artifact regardless of which chains it was anchored to.
- The audit-chain digest is platform-agnostic; 0G and Sepolia commit to the same bytes.
- `audit verify-anchor` MUST be told which chain to read from (via `--backend`); it does not auto-detect.

## Roadmap

- **Mainnet 0G deploy.** Currently testnet-only. After the production playground's 6h cron has run dual-anchor for ≥ 30 days with zero anchor mismatches, we'll deploy `Sbo3lAuditAnchor.sol` to 0G mainnet and pin the address in `deployed.json`.
- **Cross-rollup verification.** A capsule anchored on chain A could carry a side-channel proof of an anchor on chain B. Out of scope for this CLI but tracked in [`docs/dev3/scope-cut-report.md`](../dev3/scope-cut-report.md).
- **Direct browser SDK.** When 0G ships the chunk-merge fix, `apps/marketing/public/wasm/upload-0g.ts` switches from the multipart fallback to the SDK path in one commit.

## See also

- [`docs/partner-onepagers/0g.md`](../partner-onepagers/0g.md) — the integration narrative
- [`crates/sbo3l-anchor/`](../../crates/sbo3l-anchor/) — anchor contract + Rust client
- [`crates/sbo3l-cli/src/main.rs`](../../crates/sbo3l-cli/src/main.rs) — CLI source
- 0G docs: https://docs.0g.ai/
