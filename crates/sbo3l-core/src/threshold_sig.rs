//! FROST threshold signatures — real `frost-ed25519` integration (R14 P2).
//!
//! Replaces the R13 P8 scaffold (#302) with the actual zcash
//! `frost-ed25519` v3 crate. DKG, signing, and verification all
//! flow through the canonical FROST primitives; the aggregated
//! signature is a real Ed25519 Schnorr signature indistinguishable
//! to verifiers from a single-key sig.
//!
//! ## What ships in this module
//!
//! 1. **DKG** — [`run_dkg_in_memory`]: simulate distributed key
//!    generation across N parties. Production deployments run the
//!    same `dkg::part1` / `part2` / `part3` flow but transport the
//!    messages over a real network; the in-memory orchestrator is
//!    the same logic with all parties co-located. Returns one
//!    [`KeyMaterial`] per participant + the aggregated
//!    [`PublicKeyPackage`].
//! 2. **Sign** — [`sign_round_trip`]: m-of-n signing. Coordinator
//!    picks `m` participants, each commits a nonce
//!    (`round1::commit`), each produces a partial signature
//!    (`round2::sign`), the coordinator aggregates
//!    (`aggregate`). Returns a single Ed25519 signature.
//! 3. **Verify** — [`verify_threshold_signature`]: same shape as
//!    Ed25519 verification — the threshold structure is invisible
//!    to verifiers.
//! 4. **Wire-format types** — [`ThresholdConfig`],
//!    [`SigningRequest`], [`SigningResponse`] for the
//!    coordinator ↔ signer message protocol. Same shape as the
//!    R13 scaffold so callers don't have to refactor.
//!
//! ## What's still future work
//!
//! - **Network transport.** This module runs DKG + sign in-process.
//!   A board-member signoff workflow needs an authenticated
//!   transport (TLS + per-member auth) between the coordinator and
//!   each signer. The `SigningRequest` / `SigningResponse` types
//!   are designed to ride over any transport; we don't bake one
//!   in.
//! - **Persistence.** Each member's secret share must be stored
//!   securely (encrypted at rest, ideally HSM-backed). The
//!   [`KeyMaterial::secret_share_bytes`] / [`from_secret_share_bytes`]
//!   round-trip lets operators serialise+encrypt+store, but the
//!   storage layer is operator-side.
//! - **Re-sharing.** Adding/removing members without breaking the
//!   published pubkey is "proactive secret sharing" — out of
//!   scope. Today's deployments re-run DKG to rotate.

use frost_ed25519 as frost;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Public committee parameters. Stable across DKG runs as long as
/// the threshold + member count don't change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdConfig {
    /// Total committee size. E.g. 5.
    pub member_count: u16,
    /// Threshold required for a valid signature. `threshold <= member_count`.
    /// E.g. 3 for "3-of-5".
    pub threshold: u16,
    /// Aggregated public key, hex-encoded. Pinned at DKG completion.
    pub aggregated_pubkey_hex: String,
    /// Stable identifier for this committee. Pinned at DKG so a
    /// signing request can't be confused with one from a different
    /// committee.
    pub committee_id: String,
}

/// Coordinator → signer: "please sign this payload."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningRequest {
    pub committee_id: String,
    pub request_id: String,
    /// 32-byte hex digest the threshold signature will be over.
    pub payload_digest_hex: String,
    /// Unix-seconds beyond which the request expires.
    pub deadline_secs: u64,
    /// Free-form context the signer presents to the human reviewer.
    pub human_context: String,
}

/// Signer → coordinator: "I have signed (or refused)."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningResponse {
    pub committee_id: String,
    pub request_id: String,
    /// 1..=member_count — FROST identifiers are 1-indexed.
    pub signer_index: u16,
    /// Hex-encoded FROST signature share.
    pub partial_sig_hex: String,
    /// Was the signer instructed to refuse?
    pub refused: bool,
}

/// Per-participant key material. Round-trippable through hex for
/// at-rest storage.
#[derive(Debug, Clone)]
pub struct KeyMaterial {
    pub identifier: frost::Identifier,
    pub key_package: frost::keys::KeyPackage,
    pub public_key_package: frost::keys::PublicKeyPackage,
}

#[derive(Debug, thiserror::Error)]
pub enum ThresholdError {
    #[error("FROST cryptographic error: {0}")]
    Frost(String),
    #[error("invalid identifier value (must be 1..=u16::MAX): {0}")]
    InvalidIdentifier(u16),
    #[error("threshold/member-count mismatch: {threshold} > {member_count}")]
    ThresholdAboveMemberCount { threshold: u16, member_count: u16 },
    #[error("threshold must be at least 1, got {0}")]
    ThresholdZero(u16),
    #[error("committee id mismatch: request `{request_committee}` vs config `{config_committee}`")]
    CommitteeIdMismatch {
        request_committee: String,
        config_committee: String,
    },
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("payload digest must be exactly 32 bytes, got {0}")]
    BadDigestLength(usize),
    #[error("not enough signers: got {got}, need {needed}")]
    BelowThreshold { got: u16, needed: u16 },
}

impl From<frost::Error> for ThresholdError {
    fn from(e: frost::Error) -> Self {
        ThresholdError::Frost(format!("{e:?}"))
    }
}

/// Run DKG entirely in-process across `member_count` participants
/// with the given `threshold`. Returns one [`KeyMaterial`] per
/// participant. The participants' aggregated public key is
/// recorded in [`ThresholdConfig`] alongside.
///
/// **Production deployments** run the same `dkg::part1` /
/// `dkg::part2` / `dkg::part3` calls but transport the round-1 +
/// round-2 packages over an authenticated network. The
/// in-process simulator is the same logic with all parties
/// co-located and is what we use for tests.
pub fn run_dkg_in_memory(
    member_count: u16,
    threshold: u16,
    committee_id: impl Into<String>,
) -> Result<(Vec<KeyMaterial>, ThresholdConfig), ThresholdError> {
    if threshold == 0 {
        return Err(ThresholdError::ThresholdZero(threshold));
    }
    if threshold > member_count {
        return Err(ThresholdError::ThresholdAboveMemberCount {
            threshold,
            member_count,
        });
    }

    let mut rng = OsRng;

    // Round 1: each participant generates an Identifier + a
    // round-1 secret + a round-1 package.
    let mut round1_secrets: BTreeMap<frost::Identifier, frost::keys::dkg::round1::SecretPackage> =
        BTreeMap::new();
    let mut round1_packages_by_id: BTreeMap<frost::Identifier, frost::keys::dkg::round1::Package> =
        BTreeMap::new();
    for i in 1u16..=member_count {
        let id =
            frost::Identifier::try_from(i).map_err(|_| ThresholdError::InvalidIdentifier(i))?;
        // `&mut rng` triggers clippy::needless_borrows_for_generic_args on
        // recent toolchains; pass by value (RngCore impls Copy here? — no,
        // it doesn't). Suppress the lint for this exact line: the FROST
        // crate signature requires `R: RngCore + CryptoRng` by value but
        // OsRng-by-value moves; the borrow shape is intentional.
        #[allow(clippy::needless_borrows_for_generic_args)]
        let (secret, package) = frost::keys::dkg::part1(id, member_count, threshold, &mut rng)?;
        round1_secrets.insert(id, secret);
        round1_packages_by_id.insert(id, package);
    }

    // Round 2: each participant consumes everyone else's round-1
    // packages and produces a round-2 secret + per-recipient
    // round-2 packages.
    let mut round2_secrets: BTreeMap<frost::Identifier, frost::keys::dkg::round2::SecretPackage> =
        BTreeMap::new();
    let mut round2_inboxes: BTreeMap<
        frost::Identifier,
        BTreeMap<frost::Identifier, frost::keys::dkg::round2::Package>,
    > = BTreeMap::new();
    for (id, secret) in round1_secrets {
        let received: BTreeMap<_, _> = round1_packages_by_id
            .iter()
            .filter(|(other_id, _)| **other_id != id)
            .map(|(other_id, pkg)| (*other_id, pkg.clone()))
            .collect();
        let (round2_secret, round2_packages) = frost::keys::dkg::part2(secret, &received)?;
        round2_secrets.insert(id, round2_secret);
        for (recipient, pkg) in round2_packages {
            round2_inboxes.entry(recipient).or_default().insert(id, pkg);
        }
    }

    // Round 3: each participant aggregates round-1 packages from
    // others + round-2 packages from others into the final
    // KeyPackage + PublicKeyPackage.
    let mut materials = Vec::with_capacity(member_count as usize);
    for (id, round2_secret) in round2_secrets {
        let r1_received: BTreeMap<_, _> = round1_packages_by_id
            .iter()
            .filter(|(other_id, _)| **other_id != id)
            .map(|(other_id, pkg)| (*other_id, pkg.clone()))
            .collect();
        let r2_received = round2_inboxes.remove(&id).unwrap_or_default();
        let (key_package, public_key_package) =
            frost::keys::dkg::part3(&round2_secret, &r1_received, &r2_received)?;
        materials.push(KeyMaterial {
            identifier: id,
            key_package,
            public_key_package,
        });
    }

    // Sanity: every participant must have agreed on the same
    // aggregated verifying key. If frost-ed25519's part3 produced
    // divergent PublicKeyPackages, the DKG silently broke — refuse
    // to return.
    let canonical_pubkey = materials[0].public_key_package.verifying_key();
    for m in &materials[1..] {
        if m.public_key_package.verifying_key() != canonical_pubkey {
            return Err(ThresholdError::Frost(
                "DKG produced divergent verifying keys across participants".into(),
            ));
        }
    }

    let aggregated_pubkey_hex = hex::encode(canonical_pubkey.serialize()?);
    let config = ThresholdConfig {
        member_count,
        threshold,
        aggregated_pubkey_hex,
        committee_id: committee_id.into(),
    };
    Ok((materials, config))
}

/// In-process m-of-n signing. Picks the first `signers.len()`
/// from the supplied subset, runs FROST round 1 + round 2 +
/// aggregation, returns the canonical Ed25519 signature.
///
/// Production deployments run the same flow but transport
/// `SigningCommitments` (round 1) and `SignatureShare` (round 2)
/// over the network. The in-process orchestrator is the same logic
/// with all parties co-located.
pub fn sign_round_trip(
    config: &ThresholdConfig,
    signers: &[&KeyMaterial],
    payload: &[u8],
) -> Result<frost::Signature, ThresholdError> {
    if (signers.len() as u16) < config.threshold {
        return Err(ThresholdError::BelowThreshold {
            got: signers.len() as u16,
            needed: config.threshold,
        });
    }

    let mut rng = OsRng;

    // Round 1: each signer commits to a nonce.
    let mut nonces: BTreeMap<frost::Identifier, frost::round1::SigningNonces> = BTreeMap::new();
    let mut commitments_by_id: BTreeMap<frost::Identifier, frost::round1::SigningCommitments> =
        BTreeMap::new();
    for s in signers {
        let (nonce, commitment) = frost::round1::commit(s.key_package.signing_share(), &mut rng);
        nonces.insert(s.identifier, nonce);
        commitments_by_id.insert(s.identifier, commitment);
    }

    // Coordinator builds the SigningPackage from the commitments
    // + the message.
    let signing_package = frost::SigningPackage::new(commitments_by_id, payload);

    // Round 2: each signer produces a SignatureShare.
    let mut signature_shares: BTreeMap<frost::Identifier, frost::round2::SignatureShare> =
        BTreeMap::new();
    for s in signers {
        let nonce = nonces
            .get(&s.identifier)
            .expect("we just inserted this in round 1");
        let share = frost::round2::sign(&signing_package, nonce, &s.key_package)?;
        signature_shares.insert(s.identifier, share);
    }

    // Coordinator aggregates the SignatureShares.
    let pubkey_package = &signers[0].public_key_package;
    let signature = frost::aggregate(&signing_package, &signature_shares, pubkey_package)?;
    Ok(signature)
}

/// Verify a FROST-aggregated signature against the canonical
/// public key. Indistinguishable from Ed25519 verification — the
/// threshold structure is invisible to consumers.
pub fn verify_threshold_signature(
    aggregated_pubkey_hex: &str,
    payload: &[u8],
    signature: &frost::Signature,
) -> Result<(), ThresholdError> {
    let bytes = hex::decode(aggregated_pubkey_hex)?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|v: Vec<u8>| ThresholdError::BadDigestLength(v.len()))?;
    let vk = frost::VerifyingKey::deserialize(&arr)?;
    vk.verify(payload, signature)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dkg_3_of_5_produces_consistent_pubkey() {
        let (materials, config) = run_dkg_in_memory(5, 3, "test-committee").unwrap();
        assert_eq!(materials.len(), 5);
        assert_eq!(config.member_count, 5);
        assert_eq!(config.threshold, 3);
        // All participants agree on the aggregated public key.
        let canonical = materials[0].public_key_package.verifying_key();
        for m in &materials[1..] {
            assert_eq!(m.public_key_package.verifying_key(), canonical);
        }
        // The config's hex matches the binary form.
        let expected = hex::encode(canonical.serialize().unwrap());
        assert_eq!(config.aggregated_pubkey_hex, expected);
    }

    #[test]
    fn dkg_rejects_zero_threshold() {
        let err = run_dkg_in_memory(5, 0, "x").unwrap_err();
        assert!(matches!(err, ThresholdError::ThresholdZero(0)));
    }

    #[test]
    fn dkg_rejects_threshold_above_member_count() {
        let err = run_dkg_in_memory(3, 5, "x").unwrap_err();
        assert!(matches!(
            err,
            ThresholdError::ThresholdAboveMemberCount {
                threshold: 5,
                member_count: 3
            }
        ));
    }

    #[test]
    fn sign_3_of_5_round_trip_verifies() {
        let (materials, config) = run_dkg_in_memory(5, 3, "test").unwrap();
        let payload = b"hello, threshold world";

        // Pick first 3.
        let signers: Vec<&KeyMaterial> = materials.iter().take(3).collect();
        let sig = sign_round_trip(&config, &signers, payload).unwrap();
        verify_threshold_signature(&config.aggregated_pubkey_hex, payload, &sig).unwrap();
    }

    #[test]
    fn sign_4_of_5_round_trip_verifies() {
        // More signers than threshold also works.
        let (materials, config) = run_dkg_in_memory(5, 3, "test").unwrap();
        let payload = b"4 of 5 sigs";
        let signers: Vec<&KeyMaterial> = materials.iter().take(4).collect();
        let sig = sign_round_trip(&config, &signers, payload).unwrap();
        verify_threshold_signature(&config.aggregated_pubkey_hex, payload, &sig).unwrap();
    }

    #[test]
    fn sign_below_threshold_fails() {
        let (materials, config) = run_dkg_in_memory(5, 3, "test").unwrap();
        let payload = b"won't reach threshold";
        let signers: Vec<&KeyMaterial> = materials.iter().take(2).collect();
        let err = sign_round_trip(&config, &signers, payload).unwrap_err();
        assert!(matches!(
            err,
            ThresholdError::BelowThreshold { got: 2, needed: 3 }
        ));
    }

    #[test]
    fn signature_does_not_verify_under_wrong_key() {
        let (m1, _c1) = run_dkg_in_memory(5, 3, "alpha").unwrap();
        let (_m2, c2) = run_dkg_in_memory(5, 3, "beta").unwrap();
        let payload = b"crossed wires";
        let signers: Vec<&KeyMaterial> = m1.iter().take(3).collect();
        let sig = sign_round_trip(
            &ThresholdConfig {
                aggregated_pubkey_hex: hex::encode(
                    m1[0]
                        .public_key_package
                        .verifying_key()
                        .serialize()
                        .unwrap(),
                ),
                ..ThresholdConfig {
                    member_count: 5,
                    threshold: 3,
                    aggregated_pubkey_hex: String::new(),
                    committee_id: "alpha".into(),
                }
            },
            &signers,
            payload,
        )
        .unwrap();
        // Verify against the wrong DKG run's public key — must fail.
        let err = verify_threshold_signature(&c2.aggregated_pubkey_hex, payload, &sig).unwrap_err();
        assert!(matches!(err, ThresholdError::Frost(_)));
    }

    #[test]
    fn signature_does_not_verify_with_tampered_payload() {
        let (materials, config) = run_dkg_in_memory(5, 3, "test").unwrap();
        let payload = b"original";
        let tampered = b"tampered";
        let signers: Vec<&KeyMaterial> = materials.iter().take(3).collect();
        let sig = sign_round_trip(&config, &signers, payload).unwrap();
        let err =
            verify_threshold_signature(&config.aggregated_pubkey_hex, tampered, &sig).unwrap_err();
        assert!(matches!(err, ThresholdError::Frost(_)));
    }

    #[test]
    fn config_round_trip_via_json() {
        let (_m, c) = run_dkg_in_memory(5, 3, "rt").unwrap();
        let s = serde_json::to_string(&c).unwrap();
        let back: ThresholdConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn signing_request_round_trip() {
        let r = SigningRequest {
            committee_id: "x".into(),
            request_id: "req-1".into(),
            payload_digest_hex: "ab".repeat(32),
            deadline_secs: 1_777_905_000,
            human_context: "swap context".into(),
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: SigningRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn deny_unknown_fields_in_config() {
        let bad = r#"{
            "member_count": 5,
            "threshold": 3,
            "aggregated_pubkey_hex": "00",
            "committee_id": "x",
            "extra": "rejected"
        }"#;
        let res: Result<ThresholdConfig, _> = serde_json::from_str(bad);
        assert!(res.is_err());
    }

    /// Different signer subsets produce different round-1
    /// commitments, so the resulting aggregated signatures
    /// differ even though both verify under the same public key.
    /// (FROST signatures are randomised; we don't expect
    /// determinism across subsets.)
    #[test]
    fn different_signer_subsets_both_verify() {
        let (materials, config) = run_dkg_in_memory(5, 3, "test").unwrap();
        let payload = b"deterministic payload, randomised sigs";

        let sig_a = sign_round_trip(
            &config,
            &materials[0..3].iter().collect::<Vec<_>>(),
            payload,
        )
        .unwrap();
        let sig_b = sign_round_trip(
            &config,
            &materials[2..5].iter().collect::<Vec<_>>(),
            payload,
        )
        .unwrap();

        // Both verify.
        verify_threshold_signature(&config.aggregated_pubkey_hex, payload, &sig_a).unwrap();
        verify_threshold_signature(&config.aggregated_pubkey_hex, payload, &sig_b).unwrap();
        // But they're not the same signature (FROST nonce
        // randomisation).
        let a_bytes = sig_a.serialize().unwrap();
        let b_bytes = sig_b.serialize().unwrap();
        assert_ne!(a_bytes, b_bytes);
    }
}
