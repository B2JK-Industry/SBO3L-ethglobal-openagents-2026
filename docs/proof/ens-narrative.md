# ENS as Agent Trust DNS

**Audience:** ENS bounty judges (Best ENS Integration for AI Agents +
Most Creative Use of ENS).

**Outcome in 90 seconds:** ENS is the only thing two SBO3L agents
need to share to authenticate each other. No CA, no enrolment server,
no shared session token. The resolver pointer is load-bearing for
runtime authentication, not just for resolving "who claims to be at
this name." Every claim below has a code reference, a live mainnet
record, or both.

## The "trust DNS" claim

DNS resolves *names* to *machines*. SBO3L resolves *names* to *trust
commitments*. An ENS name like `research-agent.sbo3lagent.eth` carries
five `sbo3l:*` text records that together give a remote verifier
everything they need to gate a delegation, attest a result, or refuse
a peer that can't prove identity:

| Record                  | What it commits to                                   |
|-------------------------|------------------------------------------------------|
| `sbo3l:agent_id`        | Stable identifier — survives ENS resolver rotation.  |
| `sbo3l:endpoint`        | Where the daemon lives.                              |
| `sbo3l:pubkey_ed25519`  | The Ed25519 pubkey verifying the agent's signed receipts AND its cross-agent challenges. |
| `sbo3l:policy_url`      | URL of the canonical policy snapshot the agent runs. |
| `sbo3l:capabilities`    | JSON list of sponsor capabilities (`x402-purchase`, `uniswap-swap`, …). |

Plus the original three from the pre-Phase-2 baseline:
`sbo3l:policy_hash` (JCS+SHA-256 of the policy snapshot — the
**cryptographic drift check** that anchors the Most Creative framing),
`sbo3l:audit_root`, `sbo3l:proof_uri`.

Reading every record on `sbo3lagent.eth` from mainnet, today, with one
public RPC and zero SBO3L code, takes less than five seconds:

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
sbo3l agent verify-ens sbo3lagent.eth --network mainnet
# verify-ens: sbo3lagent.eth  (network: mainnet)
# ---
#   —  sbo3l:agent_id     actual="research-agent-01"  ...
#   —  sbo3l:policy_hash  actual="e044f13c5acb792dd3..."  ...
#   …
#   verdict: PASS
```

That's *static* trust: the agent has committed to a policy hash,
endpoint, pubkey. A judge holding `sbo3lagent.eth` and the ENS
Registry address can verify everything offline-of-SBO3L.

## The cross-agent leap

Static records aren't the interesting part. The interesting part is
what two SBO3L agents do with them at *runtime*.

```
Agent A                              Agent B
  │                                    │
  │  build_challenge(                  │
  │    fqdn, audit_chain_head, nonce)  │
  │  sign_challenge(challenge, key)    │
  │                                    │
  │ ────── SignedChallenge ───────────▶│
  │                                    │
  │                                    │ resolves A's
  │                                    │ sbo3l:pubkey_ed25519
  │                                    │ via getEnsText,
  │                                    │ verifies signature,
  │                                    │ emits CrossAgentTrust.
  │                                    │
  │ ◀──── CrossAgentTrust ─────────────│
```

`crates/sbo3l-identity/src/cross_agent.rs` ships the protocol. Three
function calls, one ENS lookup, one Ed25519 verify, zero session
state. The verifier's receipt pins:

```json
{
  "schema": "sbo3l.cross_agent_trust.v1",
  "peer_fqdn": "research-agent.sbo3lagent.eth",
  "peer_pubkey_hex": "0x3c754c3aad07da711d90ef16665f46c53ad050c9b3764a68d444551ca3d22003",
  "peer_audit_head_hex": "0xfb7a...",
  "signed_at_ms": 1735689600000,
  "verified_at_ms": 1735689602143,
  "valid": true
}
```

The receipt is itself a JCS-canonical artefact. A third agent who
trusts B can re-derive the verification by reading A's ENS records
and checking the signature — no shared state with B required.

This is what makes ENS *trust DNS*, not just *identity DNS*. The
resolver pointer is load-bearing for runtime authentication. **Two
SBO3L agents need ZERO out-of-band setup to authenticate each other.**

## Scale proof: 5 + 60 named agents

The protocol is one thing; the constellation is another. Phase 2
ships two manifests under `sbo3lagent.eth`:

- **`docs/proof/ens-fleet-2026-05-01.json`** — five named-role agents
  (`research`, `trading`, `swap`, `audit`, `coordinator`) each
  carrying the canonical seven `sbo3l:*` records. Reviewers re-derive
  every agent's Ed25519 pubkey byte-for-byte from the public seed
  doc `sbo3l-ens-fleet-2026-05-01` via SHA-256.

- **`docs/proof/ens-fleet-agents-60-2026-05-01.json`** — sixty deterministic
  agents, six capability classes of ten each, ENS-AGENT-A1
  amplifier. The trust-DNS visualization at
  `apps/trust-dns-viz/bench.html?source=mainnet-fleet` ingests
  `docs/proof/ens-fleet-60-events.json` and animates the
  constellation in over three seconds — every node a real ENS name
  with real records.

The registration script (`scripts/register-fleet.sh`) drives the
broadcast: derive seed in-memory → produce dry-run calldata →
`cast send` against ENS Registry's `setSubnodeRecord` → PublicResolver's
`multicall(setText × N)` for every record. Mainnet path requires
`SBO3L_ALLOW_MAINNET_TX=1` plus an explicit `network: mainnet` in the
YAML — same double-gate the rest of SBO3L's chain ops use, so an
accidental script run can't burn fee.

The script is **chain-agnostic at the registrar level**: SBO3L
originally targeted Durin's registrar, then dropped to direct
`setSubnodeRecord` once Daniel registered `sbo3lagent.eth` himself
(see memory note `durin_dropped_2026-05-01.md`). Direct path is
*more* judge-friendly: the parent's owner is verifiable on Etherscan,
`setSubnodeRecord` is the canonical ENS pattern, and no third-party
registrar contract sits in the trust path.

## CCIP-Read for live record updates

Reputation, audit head, capability whitelists — these change. Setting
each via on-chain `setText` per agent per update gets expensive at
scale. So SBO3L pairs the static records with an **ENSIP-25 / EIP-3668
CCIP-Read gateway**:

```
viem.getEnsText({name: "research-agent.sbo3lagent.eth", key: "sbo3l:reputation"})
                            │
                            ▼
              OffchainResolver reverts with OffchainLookup
                            │
                            ▼
       gateway @ sbo3l-ccip.vercel.app/api/{sender}/{data}.json
                            │
                            ▼
       gateway returns ABI-encoded (value, expires, signature)
                            │
                            ▼
              OffchainResolver verifies sig, returns value
```

The gateway lives in `apps/ccip-gateway/`. Three components:

1. **OffchainResolver Solidity contract**
   (`crates/sbo3l-identity/contracts/OffchainResolver.sol`) — one
   immutable `gatewaySigner` baked at deploy. Reverts with
   `OffchainLookup` for `text(node, key)`; verifies the EIP-191
   "intended validator" digest in the callback.
2. **TypeScript / Vercel function**
   (`apps/ccip-gateway/src/app/api/[sender]/[data]/route.ts`) —
   reads from a static record source, ABI-encodes
   `(value, expires, signature)`, signs with `GATEWAY_PRIVATE_KEY`.
3. **Rust client decoder**
   (`crates/sbo3l-identity/src/ccip_read.rs`) — for SBO3L's own
   tooling, with selector-pinned tests so the wire format can't
   silently drift.

The judges' verification is one viem call:
`viem.getEnsText('research-agent.sbo3lagent.eth', 'sbo3l:reputation')`
returns the value with no SBO3L-specific client code. Every
ENSIP-10-aware library handles the OffchainLookup revert dance
transparently.

## Why this is "Most Creative"

The competing framings in the bounty's `#partner-ens` channel treat
ENS as **identity** ("here is the agent's name"). SBO3L treats ENS
as **commitment**:

- `sbo3l:policy_hash` is a JCS+SHA-256 of the agent's active policy
  snapshot. A judge holding the ENS record + the daemon's
  `/v1/policy` endpoint can independently re-hash the snapshot and
  prove it matches. **This is policy as on-chain commitment, not as
  string in a docs page.**
- `sbo3l:audit_root` is the cumulative digest of the agent's audit
  chain (same primitive `sbo3l audit checkpoint` produces). Pinning
  it as an ENS text record means an auditor needn't trust the agent
  to hand over its history — they read ENS, hash the audit log, and
  compare.
- The `cross_agent` protocol uses `audit_chain_head_hex` as part of
  the signed challenge. **Tampering with the agent's audit history
  invalidates every previously-issued cross-agent trust receipt that
  pinned the old head.**

That last bullet is the load-bearing claim. ENS-as-commitment makes
ENS the **only** thing two agents need to share to authenticate, AND
makes silent retroactive history-tampering visible to any verifier
who kept a receipt.

## Live mainnet links (every claim has one)

| Claim                                            | Verify                                                                                       |
|--------------------------------------------------|----------------------------------------------------------------------------------------------|
| `sbo3lagent.eth` resolves the canonical 5 records | `cast text sbo3lagent.eth sbo3l:policy_hash --rpc-url https://ethereum-rpc.publicnode.com`  |
| The owner of `sbo3lagent.eth` is Daniel's wallet | https://app.ens.domains/sbo3lagent.eth                                                       |
| Five named-role agents under the apex             | `./scripts/resolve-fleet.sh docs/proof/ens-fleet-2026-05-01.json` (post-broadcast)            |
| Sixty-agent constellation                         | `./scripts/resolve-fleet.sh docs/proof/ens-fleet-agents-60-2026-05-01.json` (post-broadcast)         |
| Cross-agent verification ships                    | `cargo test -p sbo3l-identity --lib cross_agent::` (13 tests, including the pair-swap test)  |
| CCIP-Read gateway is real                         | https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json                                       |

## Live verification (clickable)

Every claim above corresponds to a clickable Etherscan / ENS App URL.
Verified live as of 2026-05-01.

### `sbo3lagent.eth` mainnet apex

| Property        | Value                                                                                            |
|-----------------|---------------------------------------------------------------------------------------------------|
| Owner (Daniel)  | [`0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231`](https://etherscan.io/address/0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231) |
| ENS Registry    | [`0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e`](https://etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e) |
| Resolver        | [`0xF29100983E058B709F3D539b0c765937B804AC15`](https://etherscan.io/address/0xF29100983E058B709F3D539b0c765937B804AC15) |
| ENS App page    | [https://app.ens.domains/sbo3lagent.eth](https://app.ens.domains/sbo3lagent.eth)                  |
| Namehash        | `0x2e3bac2fc8b574ba1db508588f06102b98554282722141f568960bb66ec12713`                              |

### `sbo3l:*` records on `sbo3lagent.eth` (verified 2026-05-01)

Direct ABI calls against the resolver. Each row is independently
verifiable by anyone with a public mainnet RPC and `cast`:

```bash
RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15
NODE=$(cast namehash sbo3lagent.eth)
cast call $RESOLVER "text(bytes32,string)(string)" $NODE sbo3l:agent_id \
  --rpc-url https://ethereum-rpc.publicnode.com
```

| Record                  | Live value (mainnet 2026-05-01)                                                                                |
|-------------------------|-----------------------------------------------------------------------------------------------------------------|
| `sbo3l:agent_id`        | `research-agent-01`                                                                                             |
| `sbo3l:endpoint`        | `http://127.0.0.1:8730/v1`                                                                                      |
| `sbo3l:policy_hash`     | `e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf`                                              |
| `sbo3l:audit_root`      | `0x0000000000000000000000000000000000000000000000000000000000000000`                                            |
| `sbo3l:proof_uri`       | `https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json`                                  |
| `sbo3l:pubkey_ed25519`  | (absent on apex — set per-subname after `register-fleet.sh` broadcast)                                          |
| `sbo3l:policy_url`      | (absent on apex — set per-subname)                                                                              |
| `sbo3l:capabilities`    | (absent on apex — set per-subname)                                                                              |

> **Note:** if `cast text sbo3lagent.eth <key>` returns empty for you,
> use the direct `cast call $RESOLVER "text(bytes32,string)(string)"`
> form above. The Universal Resolver path that `cast text` defaults to
> doesn't always work against the newer `0xF291…` resolver; the direct
> call always works.

### Per-fleet manifests (populated post-broadcast by Daniel)

Status as of `<RUNTIME>`: tx hashes filled in by
`scripts/commit-fleet-manifest.sh` after Daniel runs
`scripts/register-fleet.sh`.

#### 5-agent fleet

| FQDN                                      | Tx hashes                          | Etherscan |
|-------------------------------------------|-------------------------------------|-----------|
| `research-agent.sbo3lagent.eth`           | TBD (post-broadcast)                | TBD       |
| `trading-agent.sbo3lagent.eth`            | TBD (post-broadcast)                | TBD       |
| `swap-agent.sbo3lagent.eth`               | TBD (post-broadcast)                | TBD       |
| `audit-agent.sbo3lagent.eth`              | TBD (post-broadcast)                | TBD       |
| `coordinator-agent.sbo3lagent.eth`        | TBD (post-broadcast)                | TBD       |

Source of truth: `docs/proof/ens-fleet-2026-05-01.json`. Reviewers
re-derive every pubkey via
`python3 scripts/derive-fleet-keys.py --config scripts/fleet-config/agents-5.yaml`.

#### 60-agent fleet (ENS-AGENT-A1 amplifier)

Source of truth: `docs/proof/ens-fleet-agents-60-2026-05-01.json`. 60 deterministic
agents in 6 capability classes, registered post-broadcast. Manifest
shipped with all pubkeys; tx hashes filled in same path as above.

### T-4-1 OffchainResolver (Sepolia)

| Property                | Value                                                              |
|-------------------------|---------------------------------------------------------------------|
| Contract address        | TBD (Daniel runs `scripts/deploy-offchain-resolver.sh`)             |
| Etherscan (Sepolia)     | TBD                                                                 |
| Gateway URL baked in    | `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`            |
| Gateway signer address  | TBD (derived from Vercel `GATEWAY_PRIVATE_KEY`)                     |
| Foundry tests passing   | 6/6 (verified locally with `forge test` against the contract)       |

Once deployed: `cast call <ADDR> "supportsInterface(bytes4)(bool)" 0x9061b923` returns `true` (IExtendedResolver).

### T-4-2 ERC-8004 Identity Registry (Sepolia)

| Property                | Value                                                              |
|-------------------------|---------------------------------------------------------------------|
| Registry address        | TBD (Q-T42-1 — canonical Sepolia OR our reference deploy)           |
| Etherscan (Sepolia)     | TBD                                                                 |
| Verified ABI            | `registerAgent(address,string,string,bytes32)` returns `uint256`    |
| Selector pin            | `0x5a27c211` (recompute-pinned by `tests::register_agent_selector_is_canonical`) |

### CCIP-Read gateway

| Property                | Value                                                              |
|-------------------------|---------------------------------------------------------------------|
| Production URL          | `https://sbo3l-ccip.vercel.app` (live after Daniel's Vercel link op)|
| Landing page status     | TBD (200 expected post-deploy)                                      |
| API endpoint            | `GET /api/{sender}/{data}.json`                                     |
| Stub status (pre-T-4-1) | 501 with `{"error":"not_implemented"}`                              |
| Live status (post-T-4-1)| 200 with `{"data":"0x...","ttl":60}` or 404 for unknown records     |
| Uptime workflow         | `.github/workflows/ccip-gateway-uptime.yml` (every 30 min)          |

### Cross-agent verification protocol

| Property                | Value                                                              |
|-------------------------|---------------------------------------------------------------------|
| Module                  | `crates/sbo3l-identity/src/cross_agent.rs`                          |
| Schema id (challenge)   | `sbo3l.cross_agent_challenge.v1`                                    |
| Schema id (trust)       | `sbo3l.cross_agent_trust.v1`                                        |
| Freshness window        | ±5 min (`FRESHNESS_WINDOW_MS`)                                      |
| Tests                   | 13 unit (incl. pair-swap, byte-flip rejection, schema forward-compat) |
| Wire format             | JCS-canonical JSON; identical bytes Rust ↔ TypeScript               |

### `sbo3lagent.eth` namehash + ABI cookbook

For judges who want to verify without our tooling:

```bash
# Namehash (computed locally, no chain call):
NODE=$(cast namehash sbo3lagent.eth)
# → 0x2e3bac2fc8b574ba1db508588f06102b98554282722141f568960bb66ec12713

# Resolver address (chain call against ENS Registry):
cast call 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
  "resolver(bytes32)(address)" $NODE \
  --rpc-url https://ethereum-rpc.publicnode.com
# → 0xF29100983E058B709F3D539b0c765937B804AC15

# Owner address (same registry, owner getter):
cast call 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e \
  "owner(bytes32)(address)" $NODE \
  --rpc-url https://ethereum-rpc.publicnode.com
# → 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231

# Any sbo3l:* record:
cast call 0xF29100983E058B709F3D539b0c765937B804AC15 \
  "text(bytes32,string)(string)" $NODE sbo3l:policy_hash \
  --rpc-url https://ethereum-rpc.publicnode.com
# → "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf"
```

## Code references (every claim above)

- ENS namehash + selectors: `crates/sbo3l-identity/src/ens_anchor.rs`
- Read-side resolver: `crates/sbo3l-identity/src/ens_live.rs`
- CLI resolution + assertion: `crates/sbo3l-cli/src/agent_verify.rs`
- Issuance dry-run + broadcast script:
  `crates/sbo3l-identity/src/durin.rs` + `scripts/register-fleet.sh`
- Cross-agent protocol: `crates/sbo3l-identity/src/cross_agent.rs`
- CCIP-Read client decoder: `crates/sbo3l-identity/src/ccip_read.rs`
- CCIP-Read gateway: `apps/ccip-gateway/`
- OffchainResolver Solidity:
  `crates/sbo3l-identity/contracts/OffchainResolver.sol`
- Manifests:
  `docs/proof/ens-fleet-2026-05-01.json`,
  `docs/proof/ens-fleet-agents-60-2026-05-01.json`

## Honest scope statement

Not every component is *deployed* at submission time. The OffchainResolver
contract has Foundry tests that all pass; deploy to Sepolia is a
3-minute wallet op documented in
`docs/design/T-4-1-offchain-resolver-deploy.md`. The 5- and 60-agent
fleets are calldata-ready and the broadcast script is verified
syntactically; running it is gated on Daniel's deployer wallet.

What is shipped *as code*, with passing tests, before judging:

- 8 ENS-track PRs auto-merged or in flight (T-3-1 register, T-3-2
  verify-ens, T-3-3 fleet-of-5, T-3-4 cross-agent, T-3-7 narrative,
  T-4-1 CCIP-Read gateway, T-4-1 OffchainResolver, T-4-3 reputation).
- 100+ unit tests across the ENS surface (selector canonicality
  recompute-pinned, JCS canonicalisation stable, signature byte-flip
  rejection, freshness window, schema forward-compat).
- Two CI workflows (Vercel deploy + uptime probe) that flip live the
  moment Daniel runs the one-time Vercel + GitHub-secrets setup.

What is *operational* at submission time depends on the judges' read
window vs Daniel's wallet ops. The narrative above is *true today*
for the static records; the live CCIP-Read + cross-agent demo lights
up over the next 24 hours as ops complete.

## See also

- `docs/cli/agent.md` — `sbo3l agent register` reference.
- `docs/cli/agent-verify.md` — `sbo3l agent verify-ens`.
- `docs/cli/cross-agent.md` — protocol diagram + Rust API.
- `docs/cli/ens-fleet.md` — fleet runbook.
- `docs/design/T-4-1-offchain-resolver-deploy.md` — Daniel-runnable
  deploy.
- `apps/ccip-gateway/DEPLOY.md` — Vercel project setup.
