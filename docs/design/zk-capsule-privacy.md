# ZK / commitment-based privacy — design + status (R13 P4 → R14 P1 LIVE primitive)

**Status update (R14 P1):** the scaffold is no longer a mock. The
Rust module
([`crates/sbo3l-core/src/zk_capsule.rs`](../../crates/sbo3l-core/src/zk_capsule.rs))
ships a **real cryptographic primitive**: Ristretto-based Pedersen
commitments + a Schnorr proof-of-knowledge of the opening, both
implemented against `curve25519-dalek` v4. 16 unit tests including
hiding (different randomness → different commitments), binding
(post-commitment message tamper detected), Schnorr round trip,
wrong-message rejection, tampered-commitment rejection,
tampered-response rejection, randomised-proof distinctness, and
JSON serialisation round trip.

**Scope honest-trim is preserved**: the full Groth16 SNARK over
"prove valid SBO3L capsule whose signature verifies + audit chain
links + decision is allow" is still multi-day work (circom circuit
+ trusted-setup ceremony + browser snarkjs). What we ship here is
the **strictly narrower** primitive — commitment-based selective
disclosure / anti-front-running / timed disclosure — which is the
foundational ZK property and works as real cryptography today.
The R13 design below describes the full Groth16 plan; the Rust
module's `ZkCapsuleVerifier` trait + types are preserved for the
future-Groth16-shape surface.

---

## Original R13 P4 design (preserved for context)

Round 13 P4 asked for circom + groth16 + browser snarkjs + verifier-accepts-ZK-or-full toggle in
6h. That's truly multi-day work for a meaningful circuit; rushing
it produces a brittle artefact that breaks under any input change.
This doc covers what we'd build, the trust model, the integration
points, and the honest scope-trim. The Rust verifier scaffold lived
at [`crates/sbo3l-core/src/zk_capsule.rs`](../../crates/sbo3l-core/src/zk_capsule.rs) — feature-gated, no real cryptography
yet, ready to consume real proofs once the circuit lands.
**Companion:** [`docs/concepts/trust-dns-manifesto.md`](../concepts/trust-dns-manifesto.md) —
ENS as the **public** identity surface; ZK is the optional
**private** complement.

## Why ZK on capsules

A SBO3L Passport capsule today carries:

- The full request body (APRP).
- The signed receipt (PolicyReceipt with policy decision).
- The audit chain prefix the receipt linked to.
- The executor evidence (Uniswap quote, KH job id, etc).

Every consumer who wants to verify "did this agent run a valid
policy?" has to read the full capsule. That's fine for
public-good auditing (regulators, partner platforms, post-mortem
reviews) but exposes:

- The exact request shape — useful to a competitor reverse-
  engineering the agent's strategy.
- The audit chain prefix — leaks request volume + cadence.
- The executor evidence — reveals which sponsor surface the agent
  uses.

A ZK-redacted capsule would let an agent prove **"I have a valid
SBO3L capsule that an auditor would accept"** without revealing any
of the above. The verifier learns yes-or-no; the agent's internals
stay private.

## Architecture sketch

### Public input

A consumer who wants to gate an action on "this agent has a valid
capsule from the SBO3L pubkey" provides:

```
public {
    sbo3l_pubkey:   32 bytes  (the team's signing key, stable per release)
    challenge_hash: 32 bytes  (request-bound nonce so proofs can't replay)
    request_class:  8 bits    (low-bit fingerprint: "this proves a
                              swap-style capsule" without revealing
                              swap details)
}
```

The agent supplies a Groth16 proof that says: *"I know a
PassportCapsule whose signature verifies under `sbo3l_pubkey`,
whose audit chain links to a head no older than `challenge_hash`'s
30-day window, and whose policy decision is `allow`."*

### Private witness

```
private {
    full_capsule:       opaque bytes
    sbo3l_signature:    64 bytes  (the agent's stored receipt sig)
    audit_chain_prefix: opaque bytes (proven against capsule's audit_root)
}
```

The circuit:

1. Hashes `full_capsule` to a digest, asserts the agent's stored
   signature recovers under `sbo3l_pubkey`.
2. Walks the audit chain prefix (commit/reveal of intermediate
   hashes) to assert linkage to the capsule's `audit_root`.
3. Asserts the capsule's `decision == allow`.
4. Outputs: nothing besides the public inputs (proof of
   knowledge).

### Verifier

ZK verifier (Groth16) sits next to the existing structural
verifier. Consumers pick:

- **Structural verify** — full transparency, today's path. Capsule
  + signature + chain segment in plaintext.
- **ZK verify** — agent supplies proof + public inputs, consumer
  runs `groth16.verify(vk, public, proof)`. Returns yes/no.

The ZK path is **strictly optional**. The structural path stays
canonical; ZK is a privacy add-on for agents that want it.

## Why this is multi-day (not 6 hours)

A meaningful Groth16 circuit for the above:

1. **Trusted setup ceremony.** Powers-of-tau MPC + circuit-specific
   setup. Must be reproducible; participants must publish hashes.
   Even a single-participant "test" setup needs a verifying key
   publishable to the repo.
2. **Circuit authoring.** ~500-1000 lines of circom for the
   signature-verify + chain-walk + decision-check. R1CS
   compilation + witness generation.
3. **Browser integration.** snarkjs in a worker thread; the
   proving key alone is 5-50 MB depending on circuit size.
   Loading + caching strategy needed.
4. **End-to-end testing.** At minimum: prove + verify against a
   known capsule, prove against a tampered capsule (must fail),
   prove with stale chain head (must fail), prove with deny
   capsule (must fail).
5. **Rust verifier (server-side).** `arkworks` Groth16 backend or
   `bellman` — needs the verifying key in the binary, fixed-time
   verification.

Round 13 budgeted 6 hours. A focused engineer with a circom-ready
prior can do (1)-(2) in 2 days; (3)-(5) is another 2-3 days. A
hackathon-shipped circuit cut to fit 6h would skip the trusted
setup, hand-wire a stub circuit, and ship a non-functional demo.
That's a worse claim than the honest "design doc + scaffold."

## Honest trim

We ship two artefacts in this PR:

1. **This design doc.** Walks the architecture so a future engineer
   picks up the task without re-deriving the shape.
2. **Rust verifier scaffold** at
   [`crates/sbo3l-core/src/zk_capsule.rs`](../../crates/sbo3l-core/src/zk_capsule.rs).
   Trait + types only. Feature-gated behind `zk_capsule_verifier`
   so callers can plumb the verification surface today and the
   real verifier slots in later. Mock implementation for tests.

Both are publishable on their own. The full circuit + browser
integration lands as a follow-up that does the cryptography
correctly rather than the tooling sloppily.

## Future work (when the time exists)

Order of operations to finish:

1. Author the circuit in circom (~2 days). Test against
   known-good + known-bad capsule fixtures.
2. Run a 1-of-1 trusted setup ceremony for hackathon scope; pin
   the verifying key + circuit hash in the repo. Document a
   "production deployment requires multi-party ceremony" caveat.
3. snarkjs worker integration in `apps/marketing/`. Prove at the
   capsule-issue site (the agent's daemon, not the marketing
   site, in production).
4. Replace the Rust scaffold's mock verifier with `arkworks`-backed
   `groth16::verify` against the pinned verifying key.
5. Add the toggle to `apps/marketing/src/components/Demo3.tsx`:
   "verify with ZK" vs "verify with full data."

## Reading list

- [circom 2 docs](https://docs.circom.io/) — circuit DSL.
- [snarkjs](https://github.com/iden3/snarkjs) — in-browser
  prover/verifier.
- [arkworks Groth16](https://github.com/arkworks-rs/groth16) —
  Rust verifier reference impl.
- [Vitalik on trusted setup](https://vitalik.eth.limo/general/2022/03/14/trustedsetup.html) — ceremony posture.

## Closing

Privacy is a real win for agent identity, and ZK is the right tool
for the redaction the SBO3L Passport needs. We're not shipping the
circuit in this round — we're shipping the design + the scaffold +
the integration plan so the work can land cleanly when the
multi-day window opens. Better to declare the gap than to fill it
with a non-cryptographic stub.
