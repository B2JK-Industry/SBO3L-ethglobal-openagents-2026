//! Integration test for `eth_signer_from_env` + the local backend.
//!
//! Compiled only with `--features eth_signer`. Exercises the factory
//! path the daemon's startup code will use, plus the round-trip
//! property (sign + recover-pubkey-from-signature → matches the
//! signer's reported address).

#![cfg(feature = "eth_signer")]

use std::io::Write;

use sbo3l_core::signers::{eth_signer_from_env, EthLocalFileSigner, EthSigner};

fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
    let f = tempfile::NamedTempFile::new().expect("temp");
    let mut handle = f.reopen().expect("reopen");
    handle.write_all(bytes).expect("write");
    f
}

/// Recover the address from a 65-byte hex signature + the digest it
/// signed. Mirrors what an on-chain `ecrecover` produces; the
/// signer's reported address must match this recovered value.
fn recover_address_from_sig(sig_hex: &str, digest: &[u8; 32]) -> String {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use tiny_keccak::{Hasher as _, Keccak};

    let raw = hex::decode(sig_hex.strip_prefix("0x").unwrap_or(sig_hex)).unwrap();
    assert_eq!(raw.len(), 65);
    let sig = Signature::from_slice(&raw[..64]).unwrap();
    let recid = RecoveryId::try_from(raw[64]).unwrap();
    let vk = VerifyingKey::recover_from_prehash(digest, &sig, recid).unwrap();
    let encoded = vk.to_encoded_point(false);
    let mut h = Keccak::v256();
    h.update(&encoded.as_bytes()[1..]);
    let mut hash = [0u8; 32];
    h.finalize(&mut hash);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    sbo3l_core::signers::eip55_checksum(&addr)
}

#[test]
fn factory_default_backend_is_local_file() {
    // Default `SBO3L_ETH_SIGNER_BACKEND` (unset) → `local_file`.
    // Set the file path env, factory should construct without error.
    let f = write_temp(b"0x0303030303030303030303030303030303030303030303030303030303030303");
    unsafe {
        std::env::set_var(
            "SBO3L_ETH_LOCAL_FILE_PATH_AUDIT",
            f.path().display().to_string(),
        );
        std::env::remove_var("SBO3L_ETH_SIGNER_BACKEND");
    }
    let signer = eth_signer_from_env("audit").expect("factory must succeed");
    assert!(signer.eth_address().unwrap().starts_with("0x"));
    unsafe {
        std::env::remove_var("SBO3L_ETH_LOCAL_FILE_PATH_AUDIT");
    }
}

#[test]
fn factory_unknown_backend_returns_unknown_backend_error() {
    unsafe {
        std::env::set_var("SBO3L_ETH_SIGNER_BACKEND", "yubikey");
    }
    // `Box<dyn EthSigner>` doesn't impl Debug, so `expect_err`
    // doesn't compile. Match on the result manually.
    let result = eth_signer_from_env("audit");
    match result {
        Ok(_) => panic!("yubikey is not a known backend; factory must reject"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(msg.contains("yubikey"), "got: {msg}");
        }
    }
    unsafe {
        std::env::remove_var("SBO3L_ETH_SIGNER_BACKEND");
    }
}

#[test]
fn signer_address_matches_address_recovered_from_signature() {
    // The contract: `eth_address()` MUST equal the address an
    // on-chain `ecrecover` would derive from a sig + digest. If
    // these drift, every sponsor that verifies an SBO3L-signed
    // tx via ecrecover would reject.
    let f = write_temp(b"0x0404040404040404040404040404040404040404040404040404040404040404");
    let signer = EthLocalFileSigner::from_path("audit", f.path().to_path_buf()).unwrap();
    let claimed = signer.eth_address().unwrap();
    let digest: [u8; 32] = [0xAB; 32];
    let sig_hex = signer.sign_digest_hex(&digest).unwrap();
    let recovered = recover_address_from_sig(&sig_hex, &digest);
    assert_eq!(
        claimed, recovered,
        "signer.eth_address() must match the address recovered from sign(d) — \
         this is the contract every on-chain ecrecover relies on"
    );
}

#[test]
fn deterministic_signature_across_two_calls_with_same_secret_and_digest() {
    // Daemon-style determinism property: the same (secret, digest)
    // pair produces byte-identical signatures across calls. Lets a
    // demo replay the same audit-anchor tx without burning entropy.
    let f1 = write_temp(b"0x0505050505050505050505050505050505050505050505050505050505050505");
    let f2 = write_temp(b"0x0505050505050505050505050505050505050505050505050505050505050505");
    let s1 = EthLocalFileSigner::from_path("audit", f1.path().to_path_buf()).unwrap();
    let s2 = EthLocalFileSigner::from_path("audit", f2.path().to_path_buf()).unwrap();
    let digest: [u8; 32] = [0xCC; 32];
    assert_eq!(
        s1.sign_digest_hex(&digest).unwrap(),
        s2.sign_digest_hex(&digest).unwrap()
    );
    assert_eq!(s1.eth_address().unwrap(), s2.eth_address().unwrap());
}
