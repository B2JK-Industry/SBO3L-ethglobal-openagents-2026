//! IPFS-backed decentralised policy registry (R13 P5).
//!
//! New ENS text-record convention: `sbo3l:policy_cid`. Value is a
//! single IPFS CID (CIDv1, base32-encoded) pointing at the
//! signed-policy bundle the agent currently runs.
//!
//! ## Why CID instead of HTTP URL
//!
//! `sbo3l:policy_url` (existing) points at an HTTP endpoint — a
//! consumer trusts the URL's TLS + the operator's hosting to serve
//! the same bytes the `sbo3l:policy_hash` commits to. That's a
//! reasonable trust model when the operator is reliable, but it
//! pre-supposes:
//!
//! - The HTTP host stays online.
//! - The TLS cert chain stays trusted.
//! - The serving operator is (a) honest about which bytes match the
//!   committed hash and (b) not under cease-and-desist pressure.
//!
//! IPFS via a content-addressed CID removes (a) — the CID *is* the
//! hash, so retrieving any bytes that match the CID retrieves the
//! committed bytes. (b) reduces to "any IPFS pinning service on the
//! planet has the bytes" rather than "the operator's HTTP host
//! specifically." (c) is unchanged — TLS-style availability still
//! depends on the network.
//!
//! ## Wire format
//!
//! ENS text record:
//!
//! ```text
//! key:   sbo3l:policy_cid
//! value: ipfs://bafy...   (CIDv1, base32-encoded, ipfs:// prefix optional)
//! ```
//!
//! The bare CID without `ipfs://` is also accepted — viem clients
//! normalise both forms.
//!
//! Companion existing record `sbo3l:policy_hash` (32-byte JCS+SHA-256
//! commitment) provides redundant verification: the CID's
//! content-addressing + the SHA-256 commitment must agree on the
//! bundle bytes. If they disagree, the consumer treats the agent as
//! broken (don't trust either record).
//!
//! ## Scope of this module
//!
//! Pure-function helpers:
//!
//! - [`extract_cid`]: parse `ipfs://<cid>` or bare `<cid>` to the
//!   canonical CID string.
//! - [`is_valid_cidv1_base32`]: validate the CID shape (starts with
//!   `bafy` for dag-pb / `bafk` for raw / `bafkrei` for raw + sha-256;
//!   all-lowercase base32 alphabet, length within bounds).
//! - [`gateway_url`]: convert a CID to a public-gateway HTTPS URL
//!   for clients that don't speak IPFS natively.
//! - Round-trip stability: serialising back to a `sbo3l:policy_cid`
//!   text record value yields the canonical `ipfs://<cid>` form.
//!
//! Network IO (publishing to web3.storage, reading via Helia / kubo)
//! lives in operator tooling; this module is pure logic.

use thiserror::Error;

/// Valid CIDv1 base32 alphabet — RFC 4648 lowercase. Anything else
/// in a CID string is malformed.
const CID_BASE32_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

/// Public IPFS gateway URL template. Used by `gateway_url()` to
/// produce a clickable HTTPS URL from a CID for consumers that
/// don't run an IPFS daemon. The default is the ipfs.io public
/// gateway; operators wanting a different gateway override at the
/// call site.
pub const DEFAULT_IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

/// ENS text-record key under which the policy CID lives.
pub const POLICY_CID_TEXT_KEY: &str = "sbo3l:policy_cid";

/// Parse errors for the CID surface.
#[derive(Debug, Error)]
pub enum CidError {
    #[error("empty CID value")]
    Empty,
    #[error("CID too short: {0} bytes (CIDv1 base32 ~ 59 chars min)")]
    TooShort(usize),
    #[error("CID too long: {0} bytes (sanity cap at 200)")]
    TooLong(usize),
    #[error("CID does not start with a CIDv1 prefix (bafy/bafk/bafkrei): `{0}`")]
    BadPrefix(String),
    #[error("CID contains non-base32 character at position {0}")]
    BadChar(usize),
    #[error("`ipfs://` URI form must have a non-empty CID after the prefix")]
    UriEmptyCid,
}

/// Extract the canonical CID from either an `ipfs://<cid>` URI or
/// a bare `<cid>` string. Whitespace around the input is trimmed.
pub fn extract_cid(input: &str) -> Result<String, CidError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CidError::Empty);
    }
    let cid = if let Some(rest) = trimmed.strip_prefix("ipfs://") {
        if rest.is_empty() {
            return Err(CidError::UriEmptyCid);
        }
        rest
    } else if let Some(rest) = trimmed.strip_prefix("IPFS://") {
        // Some clients normalise to uppercase scheme; tolerate.
        if rest.is_empty() {
            return Err(CidError::UriEmptyCid);
        }
        rest
    } else {
        trimmed
    };
    Ok(cid.to_string())
}

/// Validate that `cid` is a syntactically-plausible CIDv1 in base32
/// lowercase. Accepts the three SBO3L cares about:
///
/// - `bafy*` — dag-pb (default for IPFS UnixFS objects)
/// - `bafk*` — raw codec
/// - `bafkrei*` — raw + sha-256 (most common for SBO3L policy
///   bundles, since the bundle is a JSON file with a known hash)
///
/// This is **syntactic** validation only — it doesn't fetch the CID
/// or verify the bytes hash to the CID. That's the consumer's job.
pub fn is_valid_cidv1_base32(cid: &str) -> Result<(), CidError> {
    if cid.is_empty() {
        return Err(CidError::Empty);
    }
    if cid.len() < 50 {
        return Err(CidError::TooShort(cid.len()));
    }
    if cid.len() > 200 {
        return Err(CidError::TooLong(cid.len()));
    }
    if !cid.starts_with("bafy") && !cid.starts_with("bafk") {
        return Err(CidError::BadPrefix(cid.to_string()));
    }
    for (i, byte) in cid.bytes().enumerate() {
        if !CID_BASE32_ALPHABET.contains(&byte) {
            return Err(CidError::BadChar(i));
        }
    }
    Ok(())
}

/// Convert a CID into a public-gateway HTTPS URL.
///
/// Default gateway: `https://ipfs.io/ipfs/`. Override via
/// `with_gateway` for operators who want their own pinning service
/// (e.g. `https://w3s.link/ipfs/` for web3.storage,
/// `https://gateway.pinata.cloud/ipfs/`).
pub fn gateway_url(cid: &str) -> Result<String, CidError> {
    is_valid_cidv1_base32(cid)?;
    Ok(format!("{DEFAULT_IPFS_GATEWAY}{cid}"))
}

/// Same as [`gateway_url`] with a caller-supplied gateway prefix.
/// The prefix MUST end with `/`.
pub fn with_gateway(cid: &str, gateway_prefix: &str) -> Result<String, CidError> {
    is_valid_cidv1_base32(cid)?;
    let prefix = if gateway_prefix.ends_with('/') {
        gateway_prefix.to_string()
    } else {
        format!("{gateway_prefix}/")
    };
    Ok(format!("{prefix}{cid}"))
}

/// Render a CID into the canonical `sbo3l:policy_cid` text-record
/// value: `ipfs://<cid>`. Used by the publisher side; the consumer
/// side calls `extract_cid` to reverse.
pub fn to_text_record_value(cid: &str) -> Result<String, CidError> {
    is_valid_cidv1_base32(cid)?;
    Ok(format!("ipfs://{cid}"))
}

/// Round-trip extract → validate. Convenience for "given a raw text
/// record value, return the validated bare CID or error."
pub fn parse_text_record_value(raw: &str) -> Result<String, CidError> {
    let cid = extract_cid(raw)?;
    is_valid_cidv1_base32(&cid)?;
    Ok(cid)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic CID — a real-shape CIDv1 base32 dag-pb value. Used
    /// across multiple tests as a stable fixture. Length 59
    /// (canonical CIDv1 base32 sha-256 length).
    const FIXTURE_CID: &str = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";

    /// Raw codec CID — also valid; used to verify the prefix check
    /// accepts both `bafy` and `bafk`.
    const FIXTURE_CID_RAW: &str = "bafkreid7q3jnqwf2zksxd6lhhq3pxkonqhwhvomyxqv7n3jbdwjpvw74m4";

    #[test]
    fn extract_cid_strips_ipfs_prefix() {
        assert_eq!(
            extract_cid("ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
                .unwrap(),
            FIXTURE_CID
        );
    }

    #[test]
    fn extract_cid_accepts_bare_form() {
        assert_eq!(extract_cid(FIXTURE_CID).unwrap(), FIXTURE_CID);
    }

    #[test]
    fn extract_cid_trims_whitespace() {
        assert_eq!(
            extract_cid("  ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi  ")
                .unwrap(),
            FIXTURE_CID
        );
    }

    #[test]
    fn extract_cid_tolerates_uppercase_scheme() {
        assert_eq!(
            extract_cid("IPFS://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
                .unwrap(),
            FIXTURE_CID
        );
    }

    #[test]
    fn extract_cid_rejects_empty() {
        assert!(matches!(extract_cid("").unwrap_err(), CidError::Empty));
        assert!(matches!(extract_cid("   ").unwrap_err(), CidError::Empty));
    }

    #[test]
    fn extract_cid_rejects_empty_uri_cid() {
        assert!(matches!(
            extract_cid("ipfs://").unwrap_err(),
            CidError::UriEmptyCid
        ));
    }

    #[test]
    fn is_valid_cidv1_base32_accepts_bafy() {
        assert!(is_valid_cidv1_base32(FIXTURE_CID).is_ok());
    }

    #[test]
    fn is_valid_cidv1_base32_accepts_bafk() {
        assert!(is_valid_cidv1_base32(FIXTURE_CID_RAW).is_ok());
    }

    #[test]
    fn is_valid_cidv1_base32_rejects_short() {
        assert!(matches!(
            is_valid_cidv1_base32("bafy").unwrap_err(),
            CidError::TooShort(_)
        ));
    }

    #[test]
    fn is_valid_cidv1_base32_rejects_long() {
        let long = "bafy".to_string() + &"a".repeat(300);
        assert!(matches!(
            is_valid_cidv1_base32(&long).unwrap_err(),
            CidError::TooLong(_)
        ));
    }

    #[test]
    fn is_valid_cidv1_base32_rejects_bad_prefix() {
        // CIDv0 (Qm...) is not accepted — we require CIDv1.
        let cidv0 = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG".to_string() + &"a".repeat(20);
        let err = is_valid_cidv1_base32(&cidv0).unwrap_err();
        assert!(matches!(err, CidError::BadPrefix(_)));
    }

    #[test]
    fn is_valid_cidv1_base32_rejects_uppercase_chars() {
        // Construct a fake CID that would pass length but has uppercase.
        // Note: real CIDv1 base32 is always lowercase. Uppercase = malformed.
        let bad = "BAFY".to_string() + &FIXTURE_CID[4..];
        let err = is_valid_cidv1_base32(&bad).unwrap_err();
        assert!(matches!(err, CidError::BadPrefix(_)));
    }

    #[test]
    fn is_valid_cidv1_base32_rejects_special_chars() {
        let bad = format!("bafybe!{}", &FIXTURE_CID[6..]);
        let err = is_valid_cidv1_base32(&bad).unwrap_err();
        assert!(matches!(err, CidError::BadChar(_)));
    }

    #[test]
    fn gateway_url_uses_default_ipfs_io() {
        let url = gateway_url(FIXTURE_CID).unwrap();
        assert_eq!(url, format!("https://ipfs.io/ipfs/{FIXTURE_CID}"));
    }

    #[test]
    fn with_gateway_uses_supplied_prefix() {
        let url = with_gateway(FIXTURE_CID, "https://w3s.link/ipfs/").unwrap();
        assert_eq!(url, format!("https://w3s.link/ipfs/{FIXTURE_CID}"));
    }

    #[test]
    fn with_gateway_appends_trailing_slash() {
        let url = with_gateway(FIXTURE_CID, "https://w3s.link/ipfs").unwrap();
        assert_eq!(url, format!("https://w3s.link/ipfs/{FIXTURE_CID}"));
    }

    #[test]
    fn to_text_record_value_round_trips() {
        let value = to_text_record_value(FIXTURE_CID).unwrap();
        assert_eq!(value, format!("ipfs://{FIXTURE_CID}"));
        assert_eq!(parse_text_record_value(&value).unwrap(), FIXTURE_CID);
    }

    #[test]
    fn parse_text_record_value_accepts_both_forms() {
        let with_uri = format!("ipfs://{FIXTURE_CID}");
        assert_eq!(parse_text_record_value(&with_uri).unwrap(), FIXTURE_CID);
        assert_eq!(parse_text_record_value(FIXTURE_CID).unwrap(), FIXTURE_CID);
    }

    #[test]
    fn parse_text_record_value_rejects_invalid_cid() {
        assert!(parse_text_record_value("ipfs://not-a-cid").is_err());
    }

    #[test]
    fn policy_cid_text_key_is_canonical() {
        assert_eq!(POLICY_CID_TEXT_KEY, "sbo3l:policy_cid");
    }
}
