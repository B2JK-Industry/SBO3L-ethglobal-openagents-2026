//! JSON Canonicalization Scheme (RFC 8785) and SHA-256 hex helpers.

use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn canonical_json(value: &serde_json::Value) -> Result<Vec<u8>> {
    let s = serde_json_canonicalizer::to_string(value)
        .map_err(|e| CoreError::Canonicalization(e.to_string()))?;
    Ok(s.into_bytes())
}

/// Compute the canonical SHA-256 hash of an APRP request value.
///
/// Follows §2.3 of `docs/spec/17_interface_contracts.md`:
///   `request_hash = sha256(JCS-canonical-json(request))`
pub fn request_hash(value: &serde_json::Value) -> Result<String> {
    let bytes = canonical_json(value)?;
    Ok(sha256_hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_vector() {
        let h = sha256_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn canonical_json_sorts_keys() {
        let v: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":2}"#).unwrap();
        let bytes = canonical_json(&v).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn request_hash_is_deterministic() {
        let v: serde_json::Value = serde_json::from_str(r#"{"foo":"bar","baz":[1,2,3]}"#).unwrap();
        let h1 = request_hash(&v).unwrap();
        let h2 = request_hash(&v).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    /// Lock the canonical request hash of `golden_001_minimal.json` per
    /// §11.1 of `docs/spec/17_interface_contracts.md`. Any change to the
    /// canonicalizer or to the fixture must update both places.
    #[test]
    fn golden_aprp_hash_is_locked() {
        let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
        let expected = include_str!("../../../test-corpus/aprp/golden_001_minimal.hash").trim();
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        let h = request_hash(&v).unwrap();
        assert_eq!(h, expected, "golden APRP request_hash drifted");
    }
}
