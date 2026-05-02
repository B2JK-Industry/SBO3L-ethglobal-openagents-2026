//! Live GCP KMS round-trip — gated on `GCP_KMS_TEST_ENABLED=1`.
//!
//! Compiled only with `--features eth_kms_gcp`. Skips silently when
//! the gating env var is unset.
//!
//! # How Daniel runs this in R15
//!
//! ```text
//! export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
//! export SBO3L_ETH_GCP_KMS_KEY_NAME=projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1
//! export GCP_KMS_TEST_ENABLED=1
//! cargo test -p sbo3l-core --features eth_kms_gcp --test gcp_kms_live -- --nocapture
//! ```
//!
//! Provisioning the key + IAM role: see `docs/kms-gcp-setup.md`.

#![cfg(feature = "eth_kms_gcp")]

use sbo3l_core::signers::eth::EthSigner;
use sbo3l_core::signers::eth_kms_gcp_live::GcpEthKmsLiveSigner;

fn live_enabled() -> bool {
    std::env::var("GCP_KMS_TEST_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[test]
fn live_sign_and_recover_round_trip() {
    if !live_enabled() {
        eprintln!(
            "SKIP: live_sign_and_recover_round_trip — set GCP_KMS_TEST_ENABLED=1 + \
             GOOGLE_APPLICATION_CREDENTIALS + SBO3L_ETH_GCP_KMS_KEY_NAME to enable."
        );
        return;
    }

    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sbo3l_core::signers::eth_kms_common::address_from_verifying_key;

    let signer = GcpEthKmsLiveSigner::from_env("audit").expect("from_env");
    let claimed_addr = signer.eth_address().expect("eth_address");
    eprintln!("live GCP KMS signer address = {claimed_addr}");

    let digest = [0xCDu8; 32];
    let sig_hex = signer.sign_digest_hex(&digest).expect("sign_digest_hex");
    eprintln!("live GCP KMS signature = {sig_hex}");

    let raw = hex::decode(sig_hex.strip_prefix("0x").unwrap()).unwrap();
    assert_eq!(raw.len(), 65);
    let sig = Signature::from_slice(&raw[..64]).unwrap();
    let recid = RecoveryId::try_from(raw[64]).unwrap();
    let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid)
        .expect("recover from live signature");
    let recovered_addr = address_from_verifying_key(&recovered);
    assert_eq!(
        recovered_addr, claimed_addr,
        "live GCP KMS: ecrecover must match cached address"
    );
}

#[test]
fn live_pubkey_caching_one_round_trip() {
    if !live_enabled() {
        eprintln!("SKIP: live_pubkey_caching_one_round_trip");
        return;
    }
    let signer = GcpEthKmsLiveSigner::from_env("audit").expect("from_env");
    let a1 = signer.eth_address().unwrap();
    let a2 = signer.eth_address().unwrap();
    assert_eq!(a1, a2);
}
