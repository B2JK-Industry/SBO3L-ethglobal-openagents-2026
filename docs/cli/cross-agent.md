# Cross-agent verification protocol (T-3-4)

**Audience:** SBO3L daemon authors and integrators (LangChain /
CrewAI / etc.) wiring agent-to-agent authentication on top of ENS.

**Outcome:** in three calls, two SBO3L agents authenticate each other
without trusting any CA, shared session token, or out-of-band
registration. ENS is the only rendezvous point.

## Why this matters for the ENS bounty

The ENS bounty's "Most Creative" track wants on-chain identity that
*does* something new. Cross-agent verification is the load-bearing
claim: ENS is **the only** identity layer two SBO3L agents need to
share. If you trust the ENS resolver pointer, you trust the agent.
Everything else (capabilities, reputation, audit chain head) hangs
off the same ENS namehash.

## Wire protocol

Three artifacts:

| Artifact | Role | Who emits |
|---|---|---|
| [`CrossAgentChallenge`](../../crates/sbo3l-identity/src/cross_agent.rs) | what the initiator wants verified | Agent A |
| [`SignedChallenge`](../../crates/sbo3l-identity/src/cross_agent.rs) | challenge + Ed25519 signature | Agent A |
| [`CrossAgentTrust`](../../crates/sbo3l-identity/src/cross_agent.rs) | verifier's receipt | Agent B |

```
Agent A                              Agent B
  │                                    │
  │  1. build_challenge(               │
  │       fqdn, audit_head, nonce)     │
  │     → CrossAgentChallenge          │
  │                                    │
  │  2. sign_challenge(                │
  │       challenge, signing_key)      │
  │     → SignedChallenge              │
  │                                    │
  │ ────── SignedChallenge ───────────▶│
  │                                    │
  │                                    │ 3. verify_challenge(
  │                                    │      signed,
  │                                    │      &live_ens_resolver,
  │                                    │      verified_at_ms)
  │                                    │   → CrossAgentTrust
  │                                    │   (resolves A's pubkey via
  │                                    │    getEnsText, verifies
  │                                    │    Ed25519 signature)
  │                                    │
  │ ◀──── CrossAgentTrust ─────────────│
```

The protocol is **stateless**. The verifier doesn't need to keep a
session — just a fresh ENS lookup and a signature check per call.

## Wire format

`CrossAgentChallenge` is JCS-canonical JSON:

```json
{
  "schema": "sbo3l.cross_agent_challenge.v1",
  "agent_fqdn": "research-agent.sbo3lagent.eth",
  "audit_chain_head_hex": "0xfb7a...",
  "nonce_hex": "0xab12...",
  "ts_ms": 1700000000000
}
```

The 64-byte Ed25519 signature is computed over the JCS-canonical
bytes of this struct. JCS canonicalisation is the same primitive
SBO3L's audit chain uses for `payload_hash` — two implementations on
different stacks (Rust daemon, TypeScript MCP client) sign /
verify byte-identical bytes.

`SignedChallenge`:

```json
{
  "challenge": { "schema": "sbo3l.cross_agent_challenge.v1", ... },
  "signature_hex": "0x4b2a..."
}
```

`CrossAgentTrust` (verifier emits):

```json
{
  "schema": "sbo3l.cross_agent_trust.v1",
  "peer_fqdn": "research-agent.sbo3lagent.eth",
  "peer_pubkey_hex": "0x3c75...",
  "peer_audit_head_hex": "0xfb7a...",
  "signed_at_ms": 1700000000000,
  "verified_at_ms": 1700000003142,
  "valid": true,
  "rejection_reason": null
}
```

## Rejection reasons

| `rejection_reason`                          | Why                                               |
|---------------------------------------------|----------------------------------------------------|
| `schema_mismatch`                           | Challenge schema isn't `sbo3l.cross_agent_challenge.v1` |
| `peer_fqdn_not_in_ens`                      | ENS Registry says no resolver for this FQDN |
| `sbo3l_pubkey_ed25519_record_missing`       | Resolver returned empty for `sbo3l:pubkey_ed25519` |
| `sbo3l_pubkey_ed25519_record_malformed`     | Pubkey not 64 hex chars OR not on-curve |
| `signature_malformed`                       | Signature hex isn't 128 chars / not decodable |
| `signature_mismatch`                        | Sig doesn't verify under the resolved pubkey |
| `challenge_outside_freshness_window`        | `\|verified_at - ts\| > 5 min` |

## Freshness window

`FRESHNESS_WINDOW_MS = 5 * 60 * 1000` (5 minutes). Initiator wall-clock
must be within ±5 min of verifier's wall-clock; otherwise the receipt
is `valid: false` with `challenge_outside_freshness_window`.

Replay protection: the verifier MAY cache `(agent_fqdn, nonce_hex)`
for the freshness TTL and reject duplicate nonces. The trait doesn't
mandate caching — operators with strict requirements add it externally
(e.g. via `Storage::nonce_seen`-style table).

## Rust API

```rust
use sbo3l_identity::{
    build_challenge, sign_challenge, verify_challenge,
    LiveEnsResolver, EnsNetwork,
};
use ed25519_dalek::SigningKey;

// Initiator side (Agent A):
let signing_key = SigningKey::from_bytes(&secret_seed_32_bytes);
let challenge = build_challenge(
    "research-agent.sbo3lagent.eth",
    &audit_chain_head_hex,
    &fresh_nonce_hex,
)?;
let signed = sign_challenge(&challenge, &signing_key)?;
// Send `signed` over the wire (HTTP, WebSocket, MCP, etc.)

// Verifier side (Agent B):
let resolver = LiveEnsResolver::from_env(EnsNetwork::Mainnet)?;
let trust = verify_challenge(
    &received_signed_challenge,
    &resolver,
    SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
)?;
if trust.valid {
    // Honour delegation, accept attestation, etc.
} else {
    eprintln!("rejected: {:?}", trust.rejection_reason);
}
```

`LiveEnsResolver` implements [`PubkeyResolver`](../../crates/sbo3l-identity/src/cross_agent.rs)
out of the box; testing flows inject an in-memory map.

## Pair test

The `crates/sbo3l-identity/src/cross_agent.rs` test module includes a
**pair test** where two agents in the same process cross-verify each
other's challenges:

```rust
#[test]
fn pair_swap_each_verifies_the_other() {
    // A → B, then B → A. Each receipt pins its own peer.
}
```

Plus 12 more tests covering schema drift, tampered audit head,
unknown peer, malformed pubkey/signature record, signature byte-flip,
stale + future freshness windows, JSON round-trip, and forward-compat
`deny_unknown_fields`.

## Sponsor framing

Two SBO3L agents need ZERO out-of-band setup to authenticate each
other. The verifier reads ENS, checks a signature, emits a receipt.
That's it. No CA, no enrolment server, no shared session.

This is what makes ENS *agent trust DNS*, not just *agent identity
DNS*: the resolver pointer is load-bearing for runtime
authentication, not just for resolving "who claims to be at this
name."

## See also

- `crates/sbo3l-identity/src/cross_agent.rs` — protocol implementation.
- `crates/sbo3l-identity/src/ens_live.rs::LiveEnsResolver::resolve_raw_text`
  — the underlying single-record reader.
- `docs/cli/agent-verify.md` — `sbo3l agent verify-ens` (the static
  pair to this dynamic protocol; `verify-ens` checks ENS records
  match expectations, `cross_agent` checks a signed runtime
  challenge).
- ENS bounty intel: `ens_bounty_intel.md` memory note (Dhaiwat /
  Simon ses.eth contacts).
