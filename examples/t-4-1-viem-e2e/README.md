# T-4-1 viem E2E test — Sepolia OffchainResolver closeout

End-to-end check that the deployed Sepolia OffchainResolver
(`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`) speaks ENSIP-25 /
EIP-3668 CCIP-Read correctly when driven by an off-the-shelf viem
client. **No SBO3L-specific decoder, no Rust dependency, no
internal trust assumed** — exactly what an external integrator
running against the contract for the first time would do.

## What it proves

1. **Bytecode is deployed at the canonical address** —
   `eth_getCode` returns non-empty against the resolver address.
2. **The CCIP-Read gateway responds correctly** — viem's built-in
   handler catches the `OffchainLookup` revert from
   `text(node, key)`, fetches from
   `sbo3l-ccip.vercel.app/api/{sender}/{data}.json`, submits the
   signed response back to the resolver's `resolveCallback`, and
   the resolver verifies the gateway-side EIP-191 signature on
   chain.
3. **The decoded value matches what was published** — the
   end-to-end value is a plain UTF-8 string suitable for any
   ENS-aware UI (ENS App, viem.getEnsText, raw `cast text`).

## Run

```bash
pnpm install
pnpm start
# default — research-agent.sbo3l-test.eth, sbo3l:agent_id

pnpm start <fqdn> <key>
# custom name + record key
```

## Configuration

All optional. Sensible defaults — public Sepolia RPC, the canonical
resolver address.

| Env var                    | Default                                              | Purpose                          |
|----------------------------|------------------------------------------------------|----------------------------------|
| `SBO3L_SEPOLIA_RPC_URL`    | `https://ethereum-sepolia-rpc.publicnode.com`        | Sepolia JSON-RPC endpoint        |
| `SBO3L_OFFCHAIN_RESOLVER`  | `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`        | Resolver address override        |

## Expected output (paste-ready)

Once Daniel runs `register-fleet.sh` against the chosen Sepolia
apex (see [`docs/cli/ens-fleet-sepolia.md`](../../docs/cli/ens-fleet-sepolia.md)
for the path-A vs path-B decision), the run produces:

```
═══════════════════════════════════════════════════════════════
T-4-1 viem E2E test — Sepolia OffchainResolver
═══════════════════════════════════════════════════════════════
RPC:          https://ethereum-sepolia-rpc.publicnode.com
Resolver:     0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3
ENS Registry: 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
Name (FQDN):  research-agent.sbo3l-test.eth
Record key:   sbo3l:agent_id

Step 1/3 — verifying bytecode is deployed at resolver address...
  ✓ bytecode present (… hex chars).

Step 2/3 — calling resolver.resolve(dnsEncode(name), text(node, key))...
  namehash(research-agent.sbo3l-test.eth) = 0x…
  dnsEncode = 0x0e72…
  text() calldata = 0x59d1d43c…

Step 3/3 — submitting and following CCIP-Read flow...
  ✓ gateway responded; signature verified on-chain by resolver.

═══════════════════════════════════════════════════════════════
Result
═══════════════════════════════════════════════════════════════
sbo3l:agent_id = "research-agent-01"
═══════════════════════════════════════════════════════════════
```

## What happens if the FQDN isn't registered yet

The script fails cleanly at step 3 with the underlying error and a
pointer to `docs/cli/ens-fleet-sepolia.md`. Steps 1 and 2 still
run — they verify the resolver is alive even before any subname is
registered. Useful for "the contract is deployed, the gateway
isn't published yet" partial-deploy verification.

## Why a separate viem example

The Rust client at
[`crates/sbo3l-identity/src/ccip_read.rs`](../../crates/sbo3l-identity/src/ccip_read.rs)
decodes the same wire format with selector-pinned tests, but a
judge or external integrator running `pnpm start` from the repo
root proves there's no SBO3L-shaped lock-in: the CCIP-Read
protocol is what's deployed, and any ENSIP-10-aware client (viem,
ethers.js, cast, …) handles the resolution transparently.

## Address provenance

The resolver address mirrors
`sbo3l-identity::contracts::OFFCHAIN_RESOLVER_SEPOLIA` (the
canonical pinned constant under
[`crates/sbo3l-identity/src/contracts.rs`](../../crates/sbo3l-identity/src/contracts.rs)).
Drift between the Rust pin and this TS literal is caught manually
(this example is judge-facing, not test-suite-load-bearing); the
Rust pin is what production code reads.
