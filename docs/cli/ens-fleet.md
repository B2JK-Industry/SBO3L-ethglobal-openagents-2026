# SBO3L ENS Agent Fleet — runbook

**Audience:** Daniel (or any operator with an SBO3L deployer key) issuing
agent subnames under `sbo3lagent.eth` for the Phase 2 ENS amplifier.

**Outcome:** in ~5 minutes you have N agent subnames live on Sepolia
(or mainnet behind the explicit double-gate), each carrying the
canonical 5 `sbo3l:*` text records, with a manifest committed to
`docs/proof/ens-fleet-<date>.json` that any reviewer can re-resolve
via PublicNode in <5 seconds.

This runbook covers two ticket scopes:

| Ticket | Fleet config                           | Agents | Approx gas (Sepolia) | Approx gas (mainnet @ 50 gwei) |
|--------|----------------------------------------|--------|----------------------|--------------------------------|
| T-3-3  | `scripts/fleet-config/agents-5.yaml`   | 5      | ~0.06 SEP-ETH (~$0.20) | ~$300 |
| T-3-4  | `scripts/fleet-config/agents-60.yaml`  | 60     | ~0.7 SEP-ETH (~$2.40)  | ~$3 600 |

## Prereqs (one-time)

1. **`#116` (T-3-1) merged on main.** Provides `sbo3l agent register`
   for the dry-run calldata. Verify with
   `sbo3l agent register --help | head -5`.
2. **Foundry installed.** `cast` is the broadcast vehicle until Dev 1's
   T-3-1 broadcast slice ships. Check via `cast --version`.
3. **`docs/design/T-4-1-offchain-resolver-deploy.md` step 1-3 complete.**
   The OffchainResolver address goes into the `resolver:` field of
   the YAML config (or leave empty to use PublicResolver as fallback).
4. **Funded deployer wallet.** Memory note `alchemy_rpc_endpoints.md`
   confirms `0xdc7EFA…D231` has 0.1 SEP-ETH; mainnet path needs
   ~0.7-12 ETH depending on fleet size + gas price.
5. **Daniel registered the parent on the chosen network.** Mainnet
   `sbo3lagent.eth` is owned. Sepolia parent registration is a
   prereq for T-3-3 / T-3-4 — Daniel handles via the ENS App.

## Step 1 — Derive pubkeys (deterministic, repeatable)

```bash
python3 scripts/derive-fleet-keys.py \
  --config scripts/fleet-config/agents-5.yaml \
  --output-pubkeys scripts/fleet-config/agents-5.pubkeys.json
```

The pubkey JSON is committed to the repo so reviewers can re-derive
byte-for-byte without running the script. Secret seeds are NEVER
written to disk; the registration script regenerates them in-memory
on each broadcast.

## Step 2 — Smoke-test the dry-run

Per the standing rule "the dry-run is the whole artifact" (mirrors
the `audit_anchor_ens` pattern), every operator with the same YAML
should see byte-identical calldata. Run a dry-run against one agent
to verify your local toolchain matches:

```bash
sbo3l agent register \
  --name research-agent \
  --parent sbo3lagent.eth \
  --network sepolia \
  --records "$(python3 -c '
import json
print(json.dumps({
    "sbo3l:agent_id":       "research-agent-01",
    "sbo3l:endpoint":       "https://app.sbo3l.dev/v1",
    "sbo3l:pubkey_ed25519": "3c754c3aad07da711d90ef16665f46c53ad050c9b3764a68d444551ca3d22003",
    "sbo3l:policy_url":     "https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/policies/research-agent-01.json",
    "sbo3l:capabilities":   "[\"x402-purchase\"]",
}))')" \
  --owner 0xCAFEBABE00000000000000000000000000DEADBE

# Expected last line: `broadcasted: false (dry-run does NOT contact an RPC)`
```

If the calldata starts with `0x4b7d0927` (the canonical Durin
`register` selector recompute-pinned by
`crates/sbo3l-identity/src/durin.rs::tests::register_selector_is_canonical`),
your toolchain matches the rest of the team.

## Step 3 — Broadcast

```bash
export SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/...
export SBO3L_DEPLOYER_PRIVATE_KEY=0x...
# Optional: override the Durin registrar if dry-run envelope omits it.
# export DURIN_REGISTRAR_ADDR=0x...

./scripts/register-fleet.sh scripts/fleet-config/agents-5.yaml
```

What happens per agent:

1. `python3 scripts/derive-fleet-keys.py` regenerates the 32-byte
   secret seed for the agent in-memory (no disk write).
2. `sbo3l agent register --dry-run --out /tmp/...` produces the
   calldata envelope.
3. `cast send <durin_registrar> <register_calldata>` issues the
   subname.
4. `cast send <public_resolver> <multicall_calldata>` writes all 5
   `sbo3l:*` text records in one tx.
5. `docs/proof/ens-fleet-<date>.json` updates with both tx hashes +
   Etherscan link.

Failure handling:
- **dry-run failed** — script logs the agent and skips; manifest
  records `status: dry_run_failed`. Other agents continue.
- **register tx failed** — manifest records `status: register_failed`.
- **multicall tx failed** — manifest records
  `status: partial_register_only`; the subname exists but its records
  are empty. Re-run the script with the same YAML to retry the
  multicall (no-op for already-set records).

Mainnet path:

```bash
export SBO3L_ALLOW_MAINNET_TX=1
export MAINNET_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/...
# Edit scripts/fleet-config/agents-5.yaml: network: mainnet
./scripts/register-fleet.sh scripts/fleet-config/agents-5.yaml
```

The script refuses without both the env var AND the explicit
`network: mainnet` in the YAML — same double-gate as T-3-1.

## Step 4 — Verify

```bash
./scripts/resolve-fleet.sh docs/proof/ens-fleet-agents-5-2026-05-01.json
```

Output:

```
  OK   research-agent.sbo3lagent.eth → research-agent-01
  OK   trading-agent.sbo3lagent.eth → trading-agent-01
  OK   swap-agent.sbo3lagent.eth → swap-agent-01
  OK   audit-agent.sbo3lagent.eth → audit-agent-01
  OK   coordinator-agent.sbo3lagent.eth → coordinator-agent-01
===================================================================
  5 agents | 5 resolved | 0 failed | <5s elapsed
===================================================================
```

If any agent fails to resolve:
- Wait 30s for ENS propagation (some RPCs cache stale resolver
  pointers).
- Try a different RPC: `SBO3L_RESOLVE_RPC_URL=https://1rpc.io/eth ./scripts/resolve-fleet.sh ...`
- Verify on Etherscan that the multicall tx succeeded — not just the
  register tx (a `partial_register_only` status means subname exists
  but records are empty).

## Step 5 — Commit the manifest

```bash
git add docs/proof/ens-fleet-agents-5-2026-05-01.json
git commit -m "chore(proof): T-3-3 mainnet fleet manifest (5 agents)"
```

The manifest is publishable proof: any reviewer holding it can
independently re-resolve and verify on-chain.

## Step 6 — Pin in memory

Update `live_rpc_endpoints_known.md` memory note with the deployed
fleet's parent + first FQDN so future Dev 4 sessions can re-derive
without re-reading this doc.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `cast send` returns no tx hash | RPC rate limit / nonce stale | Re-run; the script's per-agent loop is idempotent. |
| Records empty after broadcast | `multicall` tx reverted | Check Etherscan for the register tx; if it succeeded but multicall reverted, the resolver isn't pointing at PublicResolver yet. Run `cast send <ENS Registry> "setResolver(...)"` per the T-4-1 deploy runbook. |
| Subname exists but viem can't resolve | OffchainResolver pointer not flipped | Subnames inherit the parent's resolver; until step 4 of T-4-1's deploy runbook lands, only direct PublicResolver `text(...)` calls work. |
| Pubkey mismatch with another operator | Wrong `seed_doc` or `label` | Re-run `derive-fleet-keys.py --print-secrets` against the canonical YAML; pubkeys are SHA-256 deterministic. |

## See also

- `crates/sbo3l-identity/src/durin.rs` — calldata builders.
- `docs/cli/agent.md` — `sbo3l agent register` reference.
- `docs/design/T-4-1-offchain-resolver-deploy.md` — OffchainResolver +
  parent-resolver flip.
- `schemas/sbo3l.ens_fleet_manifest.v1.json` — manifest JSON schema.
- ENS bounty narrative — `docs/proof/ens-narrative.md` (T-4-4).
