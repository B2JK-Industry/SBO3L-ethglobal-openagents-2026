//! Live AWS KMS round-trip — gated on `AWS_KMS_TEST_ENABLED=1`.
//!
//! Compiled only with `--features eth_kms_aws`. Skips silently when
//! the gating env var is unset, so default `cargo test` runs (which
//! never have AWS creds) don't touch the network.
//!
//! # How Daniel runs this in R15
//!
//! ```text
//! export AWS_REGION=us-east-1
//! export AWS_ACCESS_KEY_ID=...
//! export AWS_SECRET_ACCESS_KEY=...
//! export SBO3L_ETH_AWS_KMS_KEY_ID=arn:aws:kms:us-east-1:...:key/...
//! export AWS_KMS_TEST_ENABLED=1
//! cargo test -p sbo3l-core --features eth_kms_aws --test aws_kms_live -- --nocapture
//! ```
//!
//! Provisioning the key + IAM policy: see `docs/kms-aws-setup.md`.

#![cfg(feature = "eth_kms_aws")]

use sbo3l_core::signers::eth::EthSigner;
use sbo3l_core::signers::eth_kms_aws_live::AwsEthKmsLiveSigner;

/// Skip helper — returns `true` if the gating env var is set.
fn live_enabled() -> bool {
    std::env::var("AWS_KMS_TEST_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[test]
fn live_sign_and_recover_round_trip() {
    if !live_enabled() {
        eprintln!(
            "SKIP: live_sign_and_recover_round_trip — set AWS_KMS_TEST_ENABLED=1 + AWS creds + \
             SBO3L_ETH_AWS_KMS_KEY_ID to enable."
        );
        return;
    }

    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sbo3l_core::signers::eth_kms_common::address_from_verifying_key;

    let signer = AwsEthKmsLiveSigner::from_env("audit").expect("AwsEthKmsLiveSigner::from_env");
    let claimed_addr = signer.eth_address().expect("eth_address");
    eprintln!("live AWS KMS signer address = {claimed_addr}");

    let digest = [0xABu8; 32];
    let sig_hex = signer.sign_digest_hex(&digest).expect("sign_digest_hex");
    eprintln!("live AWS KMS signature = {sig_hex}");

    // Recover address from sig + digest. Must match `claimed_addr` —
    // this is the contract every on-chain ecrecover relies on.
    let raw = hex::decode(sig_hex.strip_prefix("0x").unwrap()).unwrap();
    assert_eq!(raw.len(), 65);
    let sig = Signature::from_slice(&raw[..64]).unwrap();
    let recid = RecoveryId::try_from(raw[64]).unwrap();
    let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid)
        .expect("recover from live signature");
    let recovered_addr = address_from_verifying_key(&recovered);
    assert_eq!(
        recovered_addr, claimed_addr,
        "live AWS KMS: ecrecover must match cached address"
    );
}

#[test]
fn live_pubkey_caching_one_round_trip() {
    if !live_enabled() {
        eprintln!("SKIP: live_pubkey_caching_one_round_trip");
        return;
    }
    let signer = AwsEthKmsLiveSigner::from_env("audit").expect("from_env");
    // Two address() calls — the second must be cached (no network).
    // We can't directly assert "no network" here without wrapping the
    // SDK client, but we can assert the value is stable.
    let a1 = signer.eth_address().unwrap();
    let a2 = signer.eth_address().unwrap();
    assert_eq!(a1, a2);
}
