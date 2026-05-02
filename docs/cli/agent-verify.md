# `sbo3l agent verify-ens`

**Audience:** auditors / judges / cross-agent attesters who want to
verify that an SBO3L agent's on-chain ENS identity matches a known
expected pubkey, capability set, or capsule URI — without trusting any
single party.

**Outcome:** in one CLI command, every `sbo3l:*` text record on the
agent's ENS name resolves through `LiveEnsResolver` (PublicNode /
Alchemy / any chain RPC), each is checked against the operator's
expectations, and PASS / FAIL is printed with a per-record breakdown.

## Quick smoke

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
sbo3l agent verify-ens sbo3lagent.eth --network mainnet
```

Output (post-T-3-3 broadcast):

```
verify-ens: research-agent.sbo3lagent.eth  (network: mainnet)
---
  —       sbo3l:agent_id            actual="research-agent-01"  expected="(no expectation)"
  —       sbo3l:endpoint            actual="https://app.sbo3l.dev/v1"  expected="(no expectation)"
  —       sbo3l:pubkey_ed25519      actual="0x3c754c…22003"  expected="(no expectation)"
  —       sbo3l:policy_url          actual="https://b2jk-industry.github.io/…"  expected="(no expectation)"
  —       sbo3l:capabilities        actual="[\"x402-purchase\"]"  expected="(no expectation)"
  …
---
  totals: pass=0 fail=0 skip=8 absent=0
  verdict: PASS
```

A pure dump (no expectations) prints every record and exits 0. To
turn this into an *assertion*, pass `--expected-pubkey` or
`--expected-records`.

## Asserting a specific pubkey

```bash
sbo3l agent verify-ens research-agent.sbo3lagent.eth \
  --expected-pubkey 0x3c754c3aad07da711d90ef16665f46c53ad050c9b3764a68d444551ca3d22003
# verdict: PASS    if record matches
# verdict: FAIL    otherwise (exit code 2)
```

The `0x` prefix and 64 hex chars are required (32-byte Ed25519
pubkey). Mixed case is normalised to lowercase before comparison.

## Asserting via local key file

If you hold the agent's secret seed (e.g. you generated it via
`scripts/derive-fleet-keys.py --print-secrets`), point at it directly:

```bash
sbo3l agent verify-ens research-agent.sbo3lagent.eth \
  --key-file /tmp/research-agent.seed
```

The CLI accepts either:
- 32 raw bytes (the `ed25519-dalek` seed format), or
- 64 hex chars in UTF-8 (`derive-fleet-keys.py` emits this with
  `--print-secrets`).

It derives the Ed25519 pubkey from the seed and asserts against
`sbo3l:pubkey_ed25519`.

`--key-file` is mutually exclusive with `--expected-pubkey`.

## Asserting multiple records

```bash
sbo3l agent verify-ens research-agent.sbo3lagent.eth \
  --expected-records '{
    "sbo3l:agent_id": "research-agent-01",
    "sbo3l:endpoint": "https://app.sbo3l.dev/v1",
    "sbo3l:capabilities": "[\"x402-purchase\"]"
  }'
```

Each record listed in the JSON object is asserted. Records present
on-chain but **not** listed are reported as `skip` (no expectation —
informational only).

## Output as JSON

For pipelines / CI:

```bash
sbo3l agent verify-ens research-agent.sbo3lagent.eth \
  --expected-pubkey 0x3c754c... \
  --json
```

Emits `sbo3l.verify_ens_report.v1` envelope:

```json
{
  "schema": "sbo3l.verify_ens_report.v1",
  "fqdn": "research-agent.sbo3lagent.eth",
  "network": "mainnet",
  "checks": [
    { "key": "sbo3l:agent_id", "actual": "research-agent-01", "expected": null, "verdict": "skip" },
    { "key": "sbo3l:pubkey_ed25519", "actual": "0x3c754c…", "expected": "0x3c754c…", "verdict": "pass" },
    ...
  ],
  "verdict": "pass"
}
```

## Verdicts

| Verdict | Meaning |
|---|---|
| `pass`   | record present, matches expected |
| `fail`   | record present, doesn't match expected (OR expected but absent) |
| `skip`   | record present, no expectation supplied — informational |
| `absent` | record unset on-chain, no expectation supplied — informational |

Exit codes:

| Code | Meaning |
|---|---|
| 0 | All checks PASS (or were `skip`/`absent` with no expectations to fail). |
| 1 | Resolution error (RPC unreachable, namehash failed, etc.). |
| 2 | One or more checks FAIL, OR malformed args. |

## Live integration test

```bash
SBO3L_LIVE_ETH=1 \
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
cargo test -p sbo3l-cli --test agent_verify_live -- --include-ignored
```

Verifies the canonical 5 records on `sbo3lagent.eth` (the apex
Daniel set up pre-hackathon — see memory note
`submission_2026-04-30_live_verification.md`). Skipped cleanly without
the env vars.

The 60-agent fleet variant lives under
`scripts/resolve-fleet.sh docs/proof/ens-fleet-60-2026-05-01.json`
(T-3-4 amplifier).

## Common patterns

### Pair to T-3-1 register

After `sbo3l agent register --broadcast` lands an agent's records
on-chain, verify the broadcast was correct:

```bash
# Same records JSON the broadcast used:
sbo3l agent verify-ens research-agent.sbo3lagent.eth \
  --expected-records "$RECORDS_JSON"
```

### Cross-agent attestation gate

T-3-4's cross-agent verification protocol (separate ticket) calls
this CLI internally before honouring a delegation request:

```rust
// Pseudocode — see crates/sbo3l-identity/src/cross_agent.rs
if !verify_ens_records(peer_fqdn, expected_pubkey).pass {
    return Refusal::PeerEnsMismatch;
}
```

### Reputation-gated delegation

Once T-4-3's reputation publisher ships, `sbo3l:reputation` is also
read here and surfaced in the report — agents below
`Reputation::DEFAULT_REFUSAL_THRESHOLD` are flagged.

## See also

- `crates/sbo3l-identity/src/ens_live.rs::LiveEnsResolver::resolve_raw_text`
  — the underlying single-record resolver.
- `docs/cli/agent.md` — `sbo3l agent register` (the issuance pair).
- `docs/cli/ens-fleet.md` — bulk fleet runbook.
- `crates/sbo3l-cli/tests/agent_verify_live.rs` — the live test.
