# `sbo3l agent`

**Audience:** operators issuing per-agent ENS subnames under
`sbo3lagent.eth` (Daniel's mainnet apex) so each SBO3L agent has a
verifiable on-chain identity.

**Outcome:** in five minutes you have a printable, copy-paste-runnable
calldata pair (`register` + `multicall(setText × N)`) for a fresh
subname like `research-agent.sbo3lagent.eth`, with all `sbo3l:*` text
records pre-populated. T-3-1 ships the dry-run path; broadcast lands
in a follow-up.

## Quick smoke

```bash
sbo3l agent register \
  --name research-agent \
  --records '{
    "sbo3l:agent_id": "research-agent-01",
    "sbo3l:endpoint": "http://127.0.0.1:8730/v1",
    "sbo3l:policy_hash": "e044f13c5acb...",
    "sbo3l:audit_root": "0x0000...",
    "sbo3l:proof_uri": "https://b2jk-industry.github.io/.../capsule.json",
    "sbo3l:capability": "x402-purchase",
    "sbo3l:reputation": "0"
  }' \
  --owner 0xdc7EFA00000000000000000000000000000000d2
```

The default `--parent sbo3lagent.eth` and `--network sepolia` mean a
no-flag invocation produces a Sepolia-shaped envelope for
`<name>.sbo3lagent.eth`. Production mainnet calldata requires the
two-flag opt-in described under [Mainnet safety gate](#mainnet-safety-gate).

The output prints the schema id, FQDN, namehashes, owner / resolver
addresses, the `register` calldata (Durin), and the
`multicall(setText × N)` calldata (PublicResolver), plus a per-record
breakdown auditors can verify byte-for-byte. Two transactions worth
of calldata, ready to pipe into `cast send`.

## What the dry-run *is* and *is not*

**Is:**

- A pure function over the inputs. Same inputs always yield the same
  envelope; auditors can re-derive bit-identically.
- The whole product surface for T-3-1's main PR. Anyone who can read
  the hex calldata can verify what *would* be broadcast, even though
  the broadcast path isn't wired yet.
- Honest about scope: `broadcasted: false`, `gas_estimate: none`.

**Is not:**

- A live tx broadcast. `--broadcast` is a recognised flag but returns
  `exit 3 — not implemented` in this build. The follow-up wires
  `sbo3l_core::signers::eth::EthSigner` (F-5) and assembles an
  EIP-1559 typed-tx via `eth_sendRawTransaction`.
- A gas estimator. Run `cast estimate` against the printed calldata
  for that.
- A signer. The `--owner` flag is currently *required*; once the
  EthSigner factory ships, the signer's `eth_address()` becomes the
  default.

## Mainnet safety gate

Mainnet calldata is gas-bearing for the broadcaster (~$60 at 50 gwei).
T-3-1 enforces a **double opt-in** to keep an accidental script run
from producing mainnet bytes:

| Knob                              | Required for mainnet? | Required for sepolia? |
|-----------------------------------|----------------------:|----------------------:|
| `--network mainnet` (explicit)    | yes                  | n/a                   |
| `SBO3L_ALLOW_MAINNET_TX=1` (env)  | yes                  | no                    |

Without the env, `--network mainnet` exits with code 2 and a clear
pointer to this doc. The default `--network sepolia` never asks.

## Records (`sbo3l:*` namespace)

Only keys prefixed `sbo3l:` are accepted. Operators wanting other
text records (legacy `email`, `url`, etc.) call PublicResolver
`setText(...)` directly via `cast send`; T-3-1 is purpose-built for
the canonical SBO3L set:

| Key                  | Purpose                                                      |
|----------------------|--------------------------------------------------------------|
| `sbo3l:agent_id`     | Stable identifier for the agent (e.g. `research-agent-01`).  |
| `sbo3l:endpoint`     | HTTP endpoint of the daemon serving this agent.              |
| `sbo3l:policy_hash`  | JCS+SHA-256 hash of the activated policy snapshot.           |
| `sbo3l:audit_root`   | Last published audit-chain digest (T-3-3 amplifier updates). |
| `sbo3l:proof_uri`    | URL of a published `sbo3l.passport_capsule.v2` artifact.     |
| `sbo3l:capability`   | Comma-separated whitelist (e.g. `x402-purchase,uniswap-swap`).|
| `sbo3l:reputation`   | T-4-3 publishes; integer 0-100 from audit chain success rate. |

Per-record value cap is 1024 bytes. Operators wanting larger values
should publish a URI (`sbo3l:proof_uri`) and host the body off-chain.

## Output JSON shape

`--out path/to/envelope.json` writes the same envelope that prints
to stdout, in `sbo3l.durin_dry_run.v1` schema. Useful for piping
into `cast send` via `jq -r .register_calldata_hex`:

```bash
sbo3l agent register \
  --name research-agent \
  --owner 0xdc7EFA00000000000000000000000000000000d2 \
  --records '{"sbo3l:agent_id":"research-agent-01"}' \
  --out /tmp/durin.json

# Inspect:
jq . /tmp/durin.json

# Send (operator path; T-3-1 broadcast follow-up wraps this):
cast send \
  $(jq -r '.resolver' /tmp/durin.json) \
  $(jq -r '.multicall_calldata_hex' /tmp/durin.json) \
  --rpc-url $SEPOLIA_RPC_URL \
  --private-key $SBO3L_SEPOLIA_PRIVATE_KEY
```

## Selectors (audited values)

T-3-1 pins these as constants and re-derives them in unit tests so
they can never silently drift:

| Function                                              | Selector       |
|-------------------------------------------------------|----------------|
| `register(bytes32,string,address,address)` (Durin)    | `0x4b7d0927`  |
| `multicall(bytes[])` (PublicResolver)                 | `0xac9650d8`  |
| `setText(bytes32,string,string)` (PublicResolver)     | `0x10f13a8c`  |

If Daniel's pinned Durin deployment uses a different `register`
signature, the constant + the matching test in
`crates/sbo3l-identity/src/durin.rs` update together. The CLI's
`register_calldata_hex` will start with the new selector after
recompilation.

## Exit codes

| Code | Meaning                                                                    |
|-----:|-----------------------------------------------------------------------------|
| 0    | Dry-run succeeded; envelope printed.                                       |
| 1    | IO error (couldn't write `--out` path).                                    |
| 2    | Semantic error (bad network, bad records JSON, missing `--owner`, mainnet without env gate). |
| 3    | "Nothing to do" — `--broadcast` flag set but broadcast not implemented in this build. |

## See also

- `docs/design/T-3-1-durin-issuance-prep.md` — full pre-implementation
  design notes (resolved Q-1..Q-5, gas budget, follow-up roadmap).
- `crates/sbo3l-identity/src/durin.rs` — calldata builders + tests.
- `crates/sbo3l-cli/src/agent.rs` — CLI wiring.
- `docs/cli/audit-anchor-ens.md` — sister command for writing the
  `sbo3l:audit_root` record (mirrors this command's dry-run /
  broadcast pattern).
