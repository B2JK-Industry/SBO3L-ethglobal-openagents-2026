# `register-fleet.sh` on Sepolia: parent-name options

**Status:** decision pending (Daniel).
**Scope:** unblock `scripts/register-fleet.sh` against
`scripts/fleet-config/agents-5.yaml` + `agents-60.yaml` on Sepolia
testnet so the post-broadcast manifest auto-PR (#173) can fire end
to end.
**Why this exists:** the YAML configs name `parent: sbo3lagent.eth`,
which Daniel owns on **mainnet** only. Sepolia ENS is a separate
deployment with its own registry; mainnet ownership doesn't carry
over. The script can't issue subnames under a parent we don't own
on the target chain.

## TL;DR

Two viable paths:

- **(a) Register a free Sepolia parent (`sbo3l-test.eth`).**
  Cleanest narrative — same shape as mainnet, distinct testnet
  apex, no name collision.
- **(b) Reconfigure to an existing Daniel-owned Sepolia name.**
  Faster if such a name already exists; uglier narrative if the
  name was registered for an unrelated purpose.

This doc documents both. Pick one, edit
`scripts/fleet-config/agents-{5,60}.yaml`, run the script, the
manifest cascade in #173 takes it from there.

## Path A — register `sbo3l-test.eth` on Sepolia (recommended)

### Why it's the cleaner path

- The Sepolia ENS App lets anyone register a `.eth` second-level
  name **for free**. No gas-by-USD trade-off; just gas in test ETH
  and Daniel's wallet already holds `0.1 SEP-ETH`
  (`alchemy_rpc_endpoints.md`).
- The narrative ties cleanly: `sbo3lagent.eth` (mainnet, canonical
  apex) ↔ `sbo3l-test.eth` (Sepolia, fleet rehearsal apex). Judges
  reading the proof artefacts can tell the two apart at a glance.
- The fleet manifest schema doesn't change. The only edit is one
  line per YAML: `parent: sbo3l-test.eth`.

### Steps

1. **Register the name** — single web flow at
   `https://sepolia.app.ens.domains/sbo3l-test.eth/register`. Sepolia
   `.eth` registrations cost ~0.001 SEP-ETH for one year (essentially
   free; Daniel's wallet has 100x that).
2. **Set the resolver** — by default the registrar sets the Sepolia
   PublicResolver (`0x8FADE66B79cC9f707aB26799354482EB93a5B7dD`,
   pinned in `crates/sbo3l-identity/src/ens_anchor.rs`). No further
   action needed unless we want to swap to the SBO3L OffchainResolver
   for the fleet (then `setResolver` on `sbo3l-test.eth` to
   `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`).
3. **Edit YAML configs:**
   ```diff
   # scripts/fleet-config/agents-5.yaml
   - parent: "sbo3lagent.eth"
   + parent: "sbo3l-test.eth"
   ```
   ```diff
   # scripts/fleet-config/agents-60.yaml
   - parent: "sbo3lagent.eth"
   + parent: "sbo3l-test.eth"
   ```
4. **Run the script:**
   ```bash
   export SEPOLIA_RPC_URL='https://eth-sepolia.g.alchemy.com/v2/<key>'
   export SBO3L_ETH_PRIVATE_KEY='<hex>'   # Daniel's wallet
   ./scripts/register-fleet.sh scripts/fleet-config/agents-5.yaml
   ```
   `register-fleet.sh` reads the YAML, derives Ed25519 keypairs from
   the `seed_doc`, builds `setSubnodeRecord` calldata, broadcasts
   one tx per agent (5 in the small fleet, 60 in the large), then
   writes the populated manifest at
   `docs/proof/ens-fleet-agents-5-${DATE}.json`.
5. **Manifest cascade lands** — `scripts/commit-fleet-manifest.sh`
   (#173, merged) detects the populated manifest and opens a PR
   committing it to main with the chain-side proof links.

### Cost ceiling

- Registration: one-shot, ~0.001 SEP-ETH (free testnet ETH).
- Per-agent issuance: ~80 k gas × 5 (or 60) agents at Sepolia gas
  prices = negligible.
- Re-runs are idempotent if `setSubnodeRecord` is invoked on an
  already-issued subname (the registry just no-ops the owner-set
  if it's the same wallet).

## Path B — reconfigure to an existing Daniel-owned Sepolia name

### When this is the right path

- Daniel already owns a relevant `.eth` name on Sepolia from a
  prior project that we can repurpose.
- The name is short and meaningful (no UUID-prefixed throwaway
  names; the fleet manifest would carry the parent in every
  agent's FQDN, so an ugly parent is permanently in the artefacts).

### Steps

1. **Confirm ownership** —
   ```bash
   cast call \
     0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
     "owner(bytes32)(address)" \
     $(cast namehash <existing-name>.eth) \
     --rpc-url $SEPOLIA_RPC_URL
   ```
   Should return Daniel's wallet address
   (`0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231`).
2. **Set the resolver** if it isn't already pointing at a usable
   PublicResolver:
   ```bash
   cast send \
     0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
     "setResolver(bytes32,address)" \
     $(cast namehash <existing-name>.eth) \
     0x8FADE66B79cC9f707aB26799354482EB93a5B7dD \
     --rpc-url $SEPOLIA_RPC_URL \
     --private-key $SBO3L_ETH_PRIVATE_KEY
   ```
3. **Edit YAML configs** as in Path A, substituting the chosen
   parent name.
4. **Run the script** — same command as Path A.

### Caveat

Whatever name we pick is now part of every agent's FQDN in the
populated manifest, every demo screenshot, and every judge
verification command. Pick something we'd be happy to read aloud
during the demo video.

## What `register-fleet.sh` does (regardless of path)

For each agent in the YAML:

1. Derive Ed25519 keypair deterministically from
   `seed_doc + label`.
2. Build the `sbo3l agent register --broadcast` invocation with
   the seven canonical text records, the parent's namehash, and
   the agent's ENS owner address.
3. Broadcast the tx. The CLI emits the receipt + the populated
   manifest entry.
4. Append to `docs/proof/ens-fleet-${CONFIG_BASE}-${DATE}.json`.

The output manifest is identical in shape across mainnet and
Sepolia — only the `network` field, parent, and the on-chain tx
hashes differ. The cross-agent verification protocol (T-3-4)
doesn't care which network the records live on; it asserts
verifiable signatures rooted at whatever ENS pubkey it can
resolve.

## After registration

- `docs/proof/ens-fleet-agents-5-${DATE}.json` and
  `docs/proof/ens-fleet-agents-60-${DATE}.json` get auto-PR'd by the
  cascade in #173.
- `apps/trust-dns-viz/bench.html?source=mainnet-fleet` works against
  either network as long as the manifest's `network` field is
  honoured by the renderer. (It is — the renderer reads the manifest's
  `network`, not a hardcoded one.)
- The `sbo3l agent verify-ens` smoke test gains a Sepolia mode:
  ```bash
  SBO3L_ENS_RPC_URL=$SEPOLIA_RPC_URL \
    sbo3l agent verify-ens research-agent.<parent>.eth --network sepolia
  ```

## Recommendation

Path A. The narrative cost of "we registered a clean testnet
parent" is negligible (< 1 minute for the ENS App flow); the
narrative cost of an ugly parent name in every fleet artefact is
permanent. Daniel's call though.

## After the choice

Drop a one-line "chose path A" or "chose path B with parent `<name>.eth`"
note in the issue / PR thread that lands the YAML edit, so future
readers don't have to re-derive why the parent looks the way it
does. The repo is a hackathon submission; provenance helps the
post-mortem.
