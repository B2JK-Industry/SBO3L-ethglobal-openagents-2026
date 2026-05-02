//! `eth_kms_common` — DER + SPKI helpers shared by the AWS and GCP
//! KMS EthSigner backends.
//!
//! Compiled when EITHER `eth_kms_aws` OR `eth_kms_gcp` is on (the
//! `cfg(any(...))` predicate at the module declaration site). Both
//! KMS APIs return ASN.1-DER signatures over secp256k1 + SubjectPublic
//! KeyInfo-encoded public keys; the parsing logic is identical, so it
//! lives here rather than being duplicated in each backend.
//!
//! See `eth_kms_aws_live::AwsEthKmsLiveSigner` for the high-level
//! flow these helpers participate in.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use k256::EncodedPoint;
use tiny_keccak::{Hasher as _, Keccak};

use super::eth_local::eip55_checksum;
use super::SignerError;

/// Parse a SubjectPublicKeyInfo DER (`SEQUENCE { algorithm, BIT STRING }`)
/// holding a SEC1-uncompressed secp256k1 public key, return a
/// [`VerifyingKey`].
///
/// Both AWS KMS and GCP KMS return X.509 SPKI per RFC 5280; the BIT
/// STRING holds the 0x04 || X || Y SEC1-uncompressed point. We don't
/// parse the algorithm OID here — the calling backend validates the
/// key spec / algorithm enum separately. Strategy: scan for the last
/// 65-byte window starting with 0x04 that decodes as a valid
/// secp256k1 point. Matches the trick `ethers-rs` uses in its own
/// KMS adapters.
pub fn parse_spki_secp256k1(spki_der: &[u8]) -> Result<VerifyingKey, SignerError> {
    if spki_der.len() < 65 {
        return Err(SignerError::Kms(format!(
            "kms: SPKI DER too short ({} bytes; need >= 65)",
            spki_der.len()
        )));
    }
    for start in (0..=spki_der.len() - 65).rev() {
        if spki_der[start] != 0x04 {
            continue;
        }
        let candidate = &spki_der[start..start + 65];
        if let Ok(point) = EncodedPoint::from_bytes(candidate) {
            if let Ok(vk) = VerifyingKey::from_encoded_point(&point) {
                return Ok(vk);
            }
        }
    }
    Err(SignerError::Kms(
        "kms: SPKI DER did not contain a parseable secp256k1 point".to_string(),
    ))
}

/// Compute the EIP-55 address from a [`VerifyingKey`]. Mirrors the
/// derivation in [`crate::signers::eth_local`].
pub fn address_from_verifying_key(vk: &VerifyingKey) -> String {
    let encoded = vk.to_encoded_point(false);
    let pk_bytes = encoded.as_bytes();
    debug_assert_eq!(pk_bytes.len(), 65);
    debug_assert_eq!(pk_bytes[0], 0x04);
    let mut h = Keccak::v256();
    h.update(&pk_bytes[1..]);
    let mut hash = [0u8; 32];
    h.finalize(&mut hash);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    eip55_checksum(&addr)
}

/// Parse an ASN.1 DER `SEQUENCE { r INTEGER, s INTEGER }` into a
/// 64-byte `r || s` blob. Both integers are zero-padded to 32 bytes
/// (DER strips leading zeros and re-adds a 0x00 sign byte for high
/// bit scalars; we undo both).
pub fn der_to_rs(der: &[u8]) -> Result<[u8; 64], SignerError> {
    let mut p = 0usize;
    if der.is_empty() || der[p] != 0x30 {
        return Err(SignerError::Kms(format!(
            "der_to_rs: not a SEQUENCE (got 0x{:02x})",
            der.first().copied().unwrap_or(0)
        )));
    }
    p += 1;
    let (seq_len, ll) = read_der_len(&der[p..])?;
    p += ll;
    if seq_len + p > der.len() {
        return Err(SignerError::Kms(format!(
            "der_to_rs: truncated SEQUENCE (need {} more bytes)",
            seq_len + p - der.len()
        )));
    }
    let r = read_der_int(der, &mut p)?;
    let s = read_der_int(der, &mut p)?;
    if r.len() > 32 || s.len() > 32 {
        return Err(SignerError::Kms(format!(
            "der_to_rs: r/s longer than 32 bytes (r={}, s={})",
            r.len(),
            s.len()
        )));
    }
    let mut out = [0u8; 64];
    out[32 - r.len()..32].copy_from_slice(r);
    out[64 - s.len()..64].copy_from_slice(s);
    Ok(out)
}

fn read_der_len(buf: &[u8]) -> Result<(usize, usize), SignerError> {
    if buf.is_empty() {
        return Err(SignerError::Kms("der_to_rs: empty length".to_string()));
    }
    let first = buf[0];
    if first < 0x80 {
        return Ok((first as usize, 1));
    }
    let n = (first & 0x7f) as usize;
    if n == 0 || n > 4 || buf.len() < 1 + n {
        return Err(SignerError::Kms(format!(
            "der_to_rs: bad length prefix 0x{first:02x}"
        )));
    }
    let mut len = 0usize;
    for &b in &buf[1..=n] {
        len = (len << 8) | b as usize;
    }
    Ok((len, 1 + n))
}

fn read_der_int<'a>(buf: &'a [u8], p: &mut usize) -> Result<&'a [u8], SignerError> {
    if *p >= buf.len() || buf[*p] != 0x02 {
        return Err(SignerError::Kms(
            "der_to_rs: expected INTEGER tag".to_string(),
        ));
    }
    *p += 1;
    let (len, ll) = read_der_len(&buf[*p..])?;
    *p += ll;
    if *p + len > buf.len() {
        return Err(SignerError::Kms("der_to_rs: truncated INTEGER".to_string()));
    }
    let mut bytes = &buf[*p..*p + len];
    if bytes.len() > 1 && bytes[0] == 0x00 && bytes[1] & 0x80 != 0 {
        bytes = &bytes[1..];
    }
    *p += len;
    Ok(bytes)
}

/// Parse DER, normalize to low-S, recover `v`, return 65-byte
/// `r || s || v`. KMS APIs don't return `v` themselves — we recover
/// it by trying both 0 and 1 and matching the cached pubkey.
pub fn der_to_rsv(
    der: &[u8],
    digest: &[u8; 32],
    expected_pubkey: &VerifyingKey,
) -> Result<[u8; 65], SignerError> {
    let rs = der_to_rs(der)?;
    let mut sig = Signature::from_slice(&rs)
        .map_err(|e| SignerError::Kms(format!("der_to_rsv: bad r||s: {e}")))?;
    if let Some(normalized) = sig.normalize_s() {
        sig = normalized;
    }
    for v in 0u8..=1 {
        let recid = RecoveryId::try_from(v)
            .map_err(|e| SignerError::Kms(format!("der_to_rsv: bad recid {v}: {e}")))?;
        if let Ok(recovered) = VerifyingKey::recover_from_prehash(digest, &sig, recid) {
            if &recovered == expected_pubkey {
                let mut out = [0u8; 65];
                out[..64].copy_from_slice(&sig.to_bytes());
                out[64] = v;
                return Ok(out);
            }
        }
    }
    Err(SignerError::Kms(
        "der_to_rsv: neither recovery id produced the cached pubkey".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::signature::hazmat::PrehashSigner;
    use k256::ecdsa::SigningKey;

    #[test]
    fn der_to_rs_strips_leading_zero_for_high_bit_scalars() {
        let mut der = Vec::new();
        der.push(0x30);
        der.push(70);
        der.extend_from_slice(&[0x02, 33, 0x00, 0x80]);
        der.extend_from_slice(&[0u8; 31]);
        der.extend_from_slice(&[0x02, 33, 0x00, 0x90]);
        der.extend_from_slice(&[0u8; 31]);
        let rs = der_to_rs(&der).unwrap();
        assert_eq!(rs[0], 0x80);
        assert_eq!(rs[32], 0x90);
        assert!(rs[1..32].iter().all(|&b| b == 0));
        assert!(rs[33..64].iter().all(|&b| b == 0));
    }

    #[test]
    fn der_to_rs_pads_short_integers_to_32_bytes() {
        let der = vec![0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02];
        let rs = der_to_rs(&der).unwrap();
        assert!(rs[..31].iter().all(|&b| b == 0));
        assert_eq!(rs[31], 0x01);
        assert!(rs[32..63].iter().all(|&b| b == 0));
        assert_eq!(rs[63], 0x02);
    }

    #[test]
    fn der_to_rs_rejects_non_sequence() {
        let bad = vec![0x31, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02];
        let err = der_to_rs(&bad).expect_err("must reject non-SEQUENCE");
        match err {
            SignerError::Kms(m) => assert!(m.contains("SEQUENCE"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn der_to_rs_rejects_truncated_sequence() {
        let bad = vec![0x30, 70, 0x02, 0x01];
        let err = der_to_rs(&bad).expect_err("must reject truncated SEQUENCE");
        match err {
            SignerError::Kms(m) => {
                assert!(m.contains("truncated") || m.contains("INTEGER"), "got: {m}")
            }
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn der_to_rs_rejects_wrong_inner_tag() {
        let bad = vec![0x30, 0x04, 0x04, 0x02, 0x00, 0x01];
        let err = der_to_rs(&bad).expect_err("must reject non-INTEGER inner");
        match err {
            SignerError::Kms(m) => assert!(m.contains("INTEGER"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn der_to_rs_rejects_oversized_integer() {
        let mut der = Vec::new();
        der.push(0x30);
        der.push(70);
        der.extend_from_slice(&[0x02, 33]);
        der.extend_from_slice(&[0xFF; 33]);
        der.extend_from_slice(&[0x02, 33]);
        der.extend_from_slice(&[0xFF; 33]);
        let err = der_to_rs(&der).expect_err("must reject oversized integer");
        match err {
            SignerError::Kms(m) => assert!(m.contains("32 bytes"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    /// Build a real-shaped 88-byte secp256k1 SPKI from a SigningKey.
    fn spki_from(signing: &SigningKey) -> Vec<u8> {
        let pk = signing.verifying_key().to_encoded_point(false);
        let pk_bytes = pk.as_bytes();
        let mut spki = Vec::with_capacity(88);
        spki.extend_from_slice(&[
            0x30, 0x56, 0x30, 0x10, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06,
            0x05, 0x2b, 0x81, 0x04, 0x00, 0x0a, 0x03, 0x42, 0x00,
        ]);
        spki.extend_from_slice(pk_bytes);
        spki
    }

    #[test]
    fn parse_spki_returns_correct_pubkey() {
        let signing = SigningKey::from_bytes((&[0x11u8; 32]).into()).unwrap();
        let spki = spki_from(&signing);
        let vk = parse_spki_secp256k1(&spki).unwrap();
        assert_eq!(&vk, signing.verifying_key());
    }

    #[test]
    fn parse_spki_rejects_too_short_input() {
        let err = parse_spki_secp256k1(&[0x30, 0x00]).expect_err("must reject");
        match err {
            SignerError::Kms(m) => assert!(m.contains("too short"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn parse_spki_rejects_garbage() {
        let err = parse_spki_secp256k1(&[0x42; 100]).expect_err("must reject");
        match err {
            SignerError::Kms(m) => assert!(m.contains("not contain"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn parse_spki_handles_88_byte_canonical_secp256k1_form() {
        let signing = SigningKey::from_bytes((&[0xAAu8; 32]).into()).unwrap();
        let spki = spki_from(&signing);
        assert_eq!(spki.len(), 88);
        let vk = parse_spki_secp256k1(&spki).unwrap();
        assert_eq!(&vk, signing.verifying_key());
    }

    #[test]
    fn der_to_rsv_normalizes_high_s() {
        let signing = SigningKey::from_bytes((&[0x44u8; 32]).into()).unwrap();
        let digest = [0x55u8; 32];
        let (sig, _): (Signature, RecoveryId) = signing.sign_prehash(&digest).unwrap();
        // Force high-S form by negating s.
        let s_field = sig.s();
        let high_s = -*s_field;
        let mut high_s_bytes = [0u8; 32];
        high_s_bytes.copy_from_slice(&high_s.to_bytes());
        let mut hi_sig_bytes = [0u8; 64];
        hi_sig_bytes[..32].copy_from_slice(&sig.r().to_bytes());
        hi_sig_bytes[32..].copy_from_slice(&high_s_bytes);
        let hi_sig = Signature::from_slice(&hi_sig_bytes).unwrap();
        let der = hi_sig.to_der();

        let rsv = der_to_rsv(der.as_bytes(), &digest, signing.verifying_key()).unwrap();
        let recovered_sig = Signature::from_slice(&rsv[..64]).unwrap();
        // Low-S means s <= n/2. The Signature type already enforces
        // this on the wire via `normalize_s` (called inside der_to_rsv);
        // the simplest cross-check is "the bytes are <= n/2".
        let s_bytes: [u8; 32] = recovered_sig.s().to_bytes().into();
        // n/2 (secp256k1 group order halved):
        // 7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0
        let half_n: [u8; 32] = [
            0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46,
            0x68, 0x1B, 0x20, 0xA0,
        ];
        assert!(
            s_bytes <= half_n,
            "der_to_rsv must normalize to low-S; got s={}",
            hex::encode(s_bytes)
        );
    }

    #[test]
    fn der_to_rsv_errors_when_pubkey_doesnt_match() {
        let signing_a = SigningKey::from_bytes((&[0x77u8; 32]).into()).unwrap();
        let signing_b = SigningKey::from_bytes((&[0x88u8; 32]).into()).unwrap();
        let digest = [0x66u8; 32];
        let (sig, _): (Signature, RecoveryId) = signing_a.sign_prehash(&digest).unwrap();
        let der = sig.to_der().as_bytes().to_vec();
        let err =
            der_to_rsv(&der, &digest, signing_b.verifying_key()).expect_err("must not recover");
        match err {
            SignerError::Kms(m) => assert!(m.contains("recovery id"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn der_to_rsv_round_trips_normal_signature() {
        let signing = SigningKey::from_bytes((&[0x33u8; 32]).into()).unwrap();
        let digest = [0x99u8; 32];
        let (sig, _): (Signature, RecoveryId) = signing.sign_prehash(&digest).unwrap();
        let der = sig.to_der().as_bytes().to_vec();
        let rsv = der_to_rsv(&der, &digest, signing.verifying_key()).unwrap();
        // Recover from the rsv and check we get back the right pubkey.
        let recovered_sig = Signature::from_slice(&rsv[..64]).unwrap();
        let recid = RecoveryId::try_from(rsv[64]).unwrap();
        let recovered = VerifyingKey::recover_from_prehash(&digest, &recovered_sig, recid).unwrap();
        assert_eq!(&recovered, signing.verifying_key());
    }

    #[test]
    fn address_from_verifying_key_matches_eip55() {
        // Address should be EIP-55 formatted with leading 0x.
        let signing = SigningKey::from_bytes((&[0x55u8; 32]).into()).unwrap();
        let addr = address_from_verifying_key(signing.verifying_key());
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
    }
}
