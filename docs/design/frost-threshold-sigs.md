# FROST threshold signatures — design + status (R13 P8 → R14 P2 LIVE)

**Status update (R14 P2):** scaffold replaced with real
cryptography. The Rust module
([`crates/sbo3l-core/src/threshold_sig.rs`](../../crates/sbo3l-core/src/threshold_sig.rs))
now uses the zcash `frost-ed25519` v3 crate end-to-end:

- **Real DKG** via `frost::keys::dkg::part1` / `part2` / `part3`.
- **Real signing** via `frost::round1::commit` / `round2::sign`.
- **Real aggregation** via `frost::aggregate` producing a single
  Ed25519-Schnorr signature indistinguishable from a single-key
  signature to verifiers.
- **Real verification** via `frost::VerifyingKey::verify`.

12 unit tests including 3-of-5 + 4-of-5 round trips, below-
threshold rejection, wrong-key rejection, tampered-payload
rejection, and the cross-DKG cross-signature check (sigs from
committee A don't verify under committee B's pubkey).

**The original R13 scaffold rationale is preserved below for
historical context** — explains why we shipped scaffold-first
before getting to real cryptography.

---

## Original scaffold posture (R13 P8 archive)

Round 13 P8 originally asked for FROST 3-of-5 + corporate-board-
signoff + integration + tests in 4h. Same honest trim as R13 P4
ZK: a meaningful FROST integration is multi-day work; rushing it
produces a non-secure artefact. We shipped the scaffold first; R14
budgeted the proper time and the scaffold became real.
**Companion:** `docs/design/zk-capsule-privacy.md` (same posture
applied to ZK).

## Why threshold sigs

A SBO3L agent today has **one** signing key. Compromising that key
lets an attacker sign arbitrary receipts indistinguishable from
the legitimate agent's. For corporate / regulated deployments this
is a non-starter — a board, audit committee, or multi-stakeholder
governance group needs to authorise certain agent actions.

FROST (Flexible Round-Optimised Schnorr Threshold signatures,
RFC-draft-irtf-cfrg-frost) lets an agent's identity be backed by a
**signing committee**: a 3-of-5 (or m-of-n) group where any
m signers can collectively produce a single Ed25519 signature that
a verifier validates as if it came from a single key.

Two operational shapes:

1. **Static committee.** Five board members each hold a share;
   any three can authorise. Verifier sees the same Ed25519
   pubkey every time; rotation requires re-running the DKG.
2. **Re-shareable committee.** Same shape but supports adding /
   removing members without breaking the published pubkey
   (proactive secret sharing).

(2) is harder; (1) is the hackathon-scope target.

## Use case: corporate agent governance

```
Step 1: SBO3L corporate edition deploy.
  - 5 board members each generate a FROST keyshare via the
    `sbo3l-core::threshold_sig::dkg` flow.
  - The aggregated public key is published as the agent's
    canonical Ed25519 pubkey: `sbo3l:pubkey_ed25519` text record.
  - The agent's daemon runs as before but requires 3-of-5 sig
    aggregation before it can emit a signed receipt.

Step 2: Agent receives an APRP.
  - Daemon does its normal policy + budget + audit work.
  - Receipt construction halts pending signoff. Daemon emits a
    `SigningRequest{capsule_hash, deadline_secs}` to the board.
  - Each board member's signing client (web UI / mobile app /
    HSM-backed bot) inspects the capsule. If satisfied, signs.
  - Once 3 sigs aggregate, daemon assembles the final Ed25519 sig
    and emits the receipt.

Step 3: Verifier reads the receipt as today.
  - Single signature, single pubkey. Verifier doesn't need to know
    the receipt was threshold-signed; the FROST protocol produces
    a Schnorr signature indistinguishable from a single-key one.
```

Threshold transparency to the verifier is the elegant property: it
composes with every existing SBO3L receipt consumer without
changes.

## Architecture sketch

### DKG (one-time, per committee)

```text
1. Coordinator publishes an N-of-M target (e.g. 5 members,
   threshold 3).
2. Each member generates a polynomial + commits its coefficients
   on a public board (round 1).
3. Each member sends each other member their share of each
   polynomial (round 2, encrypted).
4. Each member aggregates received shares into their final secret
   share + computes the public verification key.
5. The committee publishes the aggregated public key.

Reference impl: `frost-ed25519` crate (zcashd's FROST).
```

### Sign

```text
1. Coordinator picks 3 members from the 5 (any subset of size m).
2. Each signer commits to a nonce (round 1).
3. Coordinator aggregates commitments into a binding factor.
4. Each signer produces a partial signature (round 2).
5. Coordinator aggregates partial sigs into the final Ed25519 sig.
```

### Verify

Same as today: `Ed25519.verify(pubkey, message, sig)`. The
threshold structure is invisible to the verifier.

## Why this is multi-day (not 4 hours)

A meaningful integration:

1. **DKG harness.** ~500-1000 lines of Rust orchestrating the
   round-1 + round-2 message flow + the network transport. Test
   suite covering: happy path, dropped messages, malicious
   coefficient detection, late-rounds reconstruction.
2. **Signing harness.** ~500-1000 lines for the round-1 + round-2
   signing flow. Test suite: 3-of-5 happy path, 4-of-5 (more
   signers than threshold), 2-of-5 (insufficient quorum, fail),
   malicious partial-sig detection, signature aggregation.
3. **Persistence.** Each member's secret share must be stored
   securely (encrypted at rest, ideally HSM-backed).
4. **Integration with the SBO3L receipt path.** The daemon's
   existing `Signer` trait gets a `ThresholdSigner` impl that
   blocks on quorum. Tests cover the existing audit-chain +
   capsule-emit pipeline working end-to-end with the new signer.
5. **Operator tooling.** A CLI for board members to inspect a
   pending signing request + sign or reject. Web UI ideal but
   CLI sufficient for hackathon.

Round 13 budgeted 4 hours. (1) alone is a 1-2 day task done
correctly; (2) another 1-2 days; (3)-(5) another 2-3 days. A 4h
ship would be a stub that doesn't actually achieve threshold
properties — strictly worse than acknowledging the gap.

## Honest trim

We ship two artefacts in this PR:

1. **This design doc.** Architecture + use case + multi-day
   honest scope.
2. **Rust trait scaffold** at
   [`crates/sbo3l-core/src/threshold_sig.rs`](../../crates/sbo3l-core/src/threshold_sig.rs).
   Trait + types + mock impl. The trait surface matches what
   `frost-ed25519` exposes; replacing the mock with the real
   crate is a contained drop-in once the multi-day integration
   work happens.

Both are publishable on their own.

## Future work

Order of operations to finish:

1. Add `frost-ed25519` as a workspace dep behind a
   `threshold_sig` feature.
2. Implement the DKG round-1/round-2 flow in
   `threshold_sig::dkg` against `frost-ed25519`'s primitives.
   Test suite as listed above.
3. Implement the sign round-1/round-2 flow in
   `threshold_sig::sign`.
4. Wire `ThresholdSigner` impl in
   `sbo3l-core::signers::threshold` consuming the above.
5. Add a CLI subcommand `sbo3l threshold sign <pending-id>` for
   board member signoff workflow.
6. Document the corporate-deploy runbook in
   `docs/cli/threshold-deploy.md`.

## Reading list

- [FROST RFC draft](https://datatracker.ietf.org/doc/draft-irtf-cfrg-frost/)
- [`frost-ed25519` crate](https://crates.io/crates/frost-ed25519)
- [Zcash's FROST docs](https://frost.zfnd.org/)
- ROAST (alternative FROST variant): https://eprint.iacr.org/2022/550

## Closing

Same posture as the ZK design: the win is real, the work is
multi-day, the scaffold lets the rest of the codebase wire the
verification surface today so the heavy crypto integration lands
cleanly when the time exists. Honest scope-trim, not skipped.
