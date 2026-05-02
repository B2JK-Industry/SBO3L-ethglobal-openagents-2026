# ERC-8004 Identity Registry Integration

**Audience:** auditors and judges verifying that an SBO3L agent's
on-chain identity points at a verifiable proof artifact (Passport
capsule).

**Outcome:** in five minutes you can resolve any SBO3L agent's
ERC-8004 entry and use the `metadataUri` field to fetch + verify the
agent's Passport capsule with `sbo3l passport verify --strict`.

## What ERC-8004 ships

ERC-8004 ("Trustless Agents") defines three on-chain registries:

| Registry             | Purpose                                                   | T-4-2 touches?      |
|----------------------|-----------------------------------------------------------|---------------------|
| Identity Registry    | Maps agent address → metadata, DID, ENS namehash          | **Yes** (write)     |
| Reputation Registry  | Cross-agent endorsements, validator feedback              | T-4-3 / amplifier   |
| Validation Registry  | Validator attestations on agent outputs                   | Out of scope        |

T-4-2 wires SBO3L into the **Identity Registry**: each agent's
on-chain entry includes the URI of its published Passport capsule, so
an auditor can go `address → ERC-8004 entry → capsule URI → verify`
without trusting any single party.

## Deployment fallback (Q-T42-1)

Daniel resolved Q-T42-1 (canonical Sepolia deployment) on 2026-05-01
as **A→B fallback**:

1. **A: try canonical first.** If Etherscan's verified-contract
   listing at the canonical address has the expected ABI at impl
   time, use it.
2. **B: deploy reference impl ourselves.** Pin the ENS Labs / ERC-8004
   reference contract to a specific commit, deploy on Sepolia
   (~$3 free testnet gas), pin our deploy tx hash + verified address
   in this doc.

**Status: A pending Daniel confirmation.** Until pinned,
[`sbo3l_identity::erc8004::ChainConfig::for_network`] returns
`RegistryNotPinned` so broadcast paths refuse cleanly. The dry-run
path works against any explicit registry address via
[`ChainConfig::explicit`] for previewing calldata.

## CLI usage (T-4-2 main PR follow-up; landed separately)

The `--erc8004-register` flag layers on top of `sbo3l agent register`
(T-3-1):

```bash
sbo3l agent register \
  --name research-agent \
  --network sepolia \
  --records '{"sbo3l:agent_id":"research-agent-01", ...}' \
  --owner 0xdc7EFA00000000000000000000000000000000d2 \
  --erc8004-register \
  --capsule-uri "https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsules/research-agent-01.json"
```

When both T-3-1 and T-4-2 broadcasts are wired (separate follow-up),
this becomes a 3-tx flow:

1. Durin `register` (T-3-1) — issues the subname.
2. PublicResolver `multicall(setText × 7)` (T-3-1) — sets sbo3l:* records.
3. ERC-8004 Identity Registry `registerAgent(address,string,string,bytes32)` — registers the agent's metadata URI.

## Auditor flow (E2E demo)

```bash
# 1. Read the ERC-8004 entry by agent address.
cast call $REGISTRY "getAgent(uint256)" $AGENT_ID --rpc-url $SEPOLIA_RPC

# 2. Extract metadataUri (capsule URL).
CAPSULE_URI=$(... extract from getAgent response ...)

# 3. Download + verify.
curl -sS $CAPSULE_URI -o /tmp/capsule.json
sbo3l passport verify --strict --path /tmp/capsule.json
```

If steps 1-3 succeed end-to-end, T-4-2 has shipped its core promise:
off-chain proof reachable from on-chain identity, no trust required
in any single party.

## Calldata reference

Function: `registerAgent(address agentAddress, string metadataUri, string did, bytes32 ensNode)`
Selector: `0x5a27c211` — pinned by recompute test in
`crates/sbo3l-identity/src/erc8004.rs::tests::register_agent_selector_is_canonical`.

DID default: `did:ens:<fqdn>` (per Q-T42-3 resolution; leans into the
SBO3L ENS story).

## Out of scope (follow-ups)

- **CLI `--erc8004-register` flag** — extends T-3-1's
  `sbo3l agent register` subcommand. Lives in a separate PR
  alongside the broadcast path so the dry-run UX layers cleanly.
- **`getAgent(uint256)` reader** — needed by the auditor flow above.
  Adds to `crates/sbo3l-identity/src/erc8004.rs` once we have a
  registry to read from.
- **Reputation Registry integration** — T-4-3 publishes reputation to
  the ERC-8004 Reputation Registry alongside the ENS
  `sbo3l:reputation` text record.
- **ERC-8004 reference contract pinning** — Daniel pins commit at
  impl time; if scenario B (deploy ourselves), this doc records the
  deploy tx hash + verified Etherscan link.

## References

- ERC-8004 draft: `https://eips.ethereum.org/EIPS/eip-8004` (verify
  status at impl time).
- W3C DID core: `https://www.w3.org/TR/did-core/`.
- T-4-2 design doc: `docs/design/T-4-2-erc8004-prep.md`.
- T-3-1 sibling (Durin issuance, ENS subname): `docs/cli/agent.md`.
