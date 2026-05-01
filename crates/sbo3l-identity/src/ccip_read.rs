//! CCIP-Read (ENSIP-10 / EIP-3668) client-side decoder.
//!
//! The gateway lives in `apps/ccip-gateway/` (TypeScript / Vercel
//! function); this module is the **Rust counterpart** for SBO3L's own
//! tooling — `sbo3l passport resolve <name>` and friends — so we can
//! resolve subnames behind the OffchainResolver without trusting
//! third-party clients.
//!
//! T-4-1 ships:
//!
//! * [`parse_offchain_lookup_revert`] — extract sender / urls /
//!   callData / callbackFunction / extraData from the standard
//!   `OffchainLookup(address,string[],bytes,bytes4,bytes)` revert
//!   payload that an OffchainResolver returns from `text()` / `addr()`.
//! * [`decode_gateway_response_body`] — parse the gateway's
//!   `{"data": "0x...", "ttl": N}` JSON.
//! * [`decode_gateway_data`] — split the gateway's `data` field into
//!   `(value, expires, signature)`.
//! * [`decode_string_result`] — pull the actual string out of
//!   `value`, which is the ABI-encoded `(string)` tuple that
//!   `abi.decode(..., (string))` would unwrap.
//!
//! Signature verification (recover the gateway signer's address from
//! the signature, compare to the OffchainResolver's expected signer)
//! is the missing piece. Adding it requires `k256` or
//! `secp256k1`; T-4-1 leaves that as a follow-up so the dep tree
//! stays minimal — the trust model for our internal tools right now
//! is "trust the gateway URL TLS cert + the OffchainResolver
//! contract", which matches what `viem.getEnsText` does today.

use serde::Deserialize;
use thiserror::Error;

/// Standard ENSIP-10 / EIP-3668 OffchainLookup error selector =
/// keccak256("OffchainLookup(address,string[],bytes,bytes4,bytes)")[..4].
pub const OFFCHAIN_LOOKUP_SELECTOR: [u8; 4] = [0x55, 0x6f, 0x18, 0x30];

#[derive(Debug, Error)]
pub enum CcipError {
    #[error("revert payload too short: {0} bytes")]
    RevertTooShort(usize),

    #[error("revert selector did not match OffchainLookup")]
    NotOffchainLookup,

    #[error("ABI decode error: {0}")]
    AbiDecode(String),

    #[error("gateway response JSON parse: {0}")]
    Json(#[from] serde_json::Error),

    #[error("hex decode: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("gateway data must be hex with `0x` prefix")]
    MissingHexPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffchainLookup {
    pub sender: [u8; 20],
    pub urls: Vec<String>,
    pub call_data: Vec<u8>,
    pub callback_selector: [u8; 4],
    pub extra_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayResponse {
    pub value: Vec<u8>,
    pub expires: u64,
    pub signature: Vec<u8>,
}

/// Raw shape returned by the gateway. Distinct from
/// [`GatewayResponse`] which is the decoded inner ABI tuple.
#[derive(Debug, Deserialize)]
pub struct GatewayBody {
    pub data: String,
    #[allow(dead_code)]
    pub ttl: u64,
}

/// Parse the revert bytes of a `text(node, key)` or `addr(node)`
/// `eth_call` against an OffchainResolver. The first 4 bytes are the
/// OffchainLookup error selector; the remainder is ABI-encoded args.
pub fn parse_offchain_lookup_revert(revert: &[u8]) -> Result<OffchainLookup, CcipError> {
    if revert.len() < 4 {
        return Err(CcipError::RevertTooShort(revert.len()));
    }
    if revert[..4] != OFFCHAIN_LOOKUP_SELECTOR {
        return Err(CcipError::NotOffchainLookup);
    }
    decode_lookup_args(&revert[4..])
}

fn decode_lookup_args(b: &[u8]) -> Result<OffchainLookup, CcipError> {
    // ABI layout (head section, 5 words = 5 * 32 bytes):
    //   word 0: address sender   (20 bytes left-padded)
    //   word 1: offset to urls   (string[])
    //   word 2: offset to callData (bytes)
    //   word 3: bytes4 callbackFunction (left-aligned in 32-byte slot)
    //   word 4: offset to extraData (bytes)
    if b.len() < 5 * 32 {
        return Err(CcipError::AbiDecode(
            "lookup args head too short".to_string(),
        ));
    }
    let mut sender = [0u8; 20];
    sender.copy_from_slice(&b[12..32]);

    let urls_offset = read_u64_word(b, 32)?;
    let calldata_offset = read_u64_word(b, 64)?;
    let mut callback_selector = [0u8; 4];
    callback_selector.copy_from_slice(&b[96..100]);
    let extra_data_offset = read_u64_word(b, 128)?;

    let urls = decode_string_array(b, urls_offset as usize)?;
    let call_data = decode_bytes(b, calldata_offset as usize)?;
    let extra_data = decode_bytes(b, extra_data_offset as usize)?;

    Ok(OffchainLookup {
        sender,
        urls,
        call_data,
        callback_selector,
        extra_data,
    })
}

fn read_u64_word(b: &[u8], off: usize) -> Result<u64, CcipError> {
    if b.len() < off + 32 {
        return Err(CcipError::AbiDecode(format!("word at offset {off} OOB")));
    }
    // Last 8 bytes of the 32-byte big-endian word.
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&b[off + 24..off + 32]);
    Ok(u64::from_be_bytes(buf))
}

fn decode_bytes(b: &[u8], offset: usize) -> Result<Vec<u8>, CcipError> {
    if b.len() < offset + 32 {
        return Err(CcipError::AbiDecode(format!(
            "bytes len-word at {offset} OOB"
        )));
    }
    let len = read_u64_word(b, offset)? as usize;
    let start = offset + 32;
    let end = start + len;
    if b.len() < end {
        return Err(CcipError::AbiDecode(format!(
            "bytes content at {start}..{end} OOB"
        )));
    }
    Ok(b[start..end].to_vec())
}

fn decode_string_array(b: &[u8], offset: usize) -> Result<Vec<String>, CcipError> {
    if b.len() < offset + 32 {
        return Err(CcipError::AbiDecode("string[] len-word OOB".to_string()));
    }
    let len = read_u64_word(b, offset)? as usize;
    let head_start = offset + 32;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let head_off = head_start + i * 32;
        let elem_off = read_u64_word(b, head_off)? as usize + head_start;
        let bytes = decode_bytes(b, elem_off)?;
        out.push(
            String::from_utf8(bytes)
                .map_err(|e| CcipError::AbiDecode(format!("non-utf8 url: {e}")))?,
        );
    }
    Ok(out)
}

/// Parse the gateway's JSON body to a [`GatewayBody`].
pub fn decode_gateway_response_body(body: &[u8]) -> Result<GatewayBody, CcipError> {
    Ok(serde_json::from_slice(body)?)
}

/// Decode the gateway's `data` field into the
/// `(bytes value, uint64 expires, bytes signature)` triple.
pub fn decode_gateway_data(data_hex: &str) -> Result<GatewayResponse, CcipError> {
    let stripped = data_hex
        .strip_prefix("0x")
        .or_else(|| data_hex.strip_prefix("0X"))
        .ok_or(CcipError::MissingHexPrefix)?;
    let raw = hex::decode(stripped)?;

    // ABI head: 3 words = 96 bytes.
    //   word 0: offset to value
    //   word 1: uint64 expires (right-padded in 32-byte slot)
    //   word 2: offset to signature
    if raw.len() < 96 {
        return Err(CcipError::AbiDecode(
            "gateway data head too short".to_string(),
        ));
    }
    let value_offset = read_u64_word(&raw, 0)? as usize;
    let expires = read_u64_word(&raw, 32)?;
    let signature_offset = read_u64_word(&raw, 64)? as usize;

    let value = decode_bytes(&raw, value_offset)?;
    let signature = decode_bytes(&raw, signature_offset)?;

    Ok(GatewayResponse {
        value,
        expires,
        signature,
    })
}

/// Decode an ABI-encoded `(string)` tuple as a UTF-8 [`String`]. The
/// gateway returns text records this way for `text(node, key)`
/// queries.
pub fn decode_string_result(result_bytes: &[u8]) -> Result<String, CcipError> {
    // ABI: head word = offset (always 0x20 for single-string),
    // followed by length + padded bytes.
    if result_bytes.len() < 64 {
        return Err(CcipError::AbiDecode(
            "string result too short".to_string(),
        ));
    }
    let len = read_u64_word(result_bytes, 32)? as usize;
    let start = 64;
    let end = start + len;
    if result_bytes.len() < end {
        return Err(CcipError::AbiDecode("string content OOB".to_string()));
    }
    String::from_utf8(result_bytes[start..end].to_vec())
        .map_err(|e| CcipError::AbiDecode(format!("non-utf8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offchain_lookup_selector_matches_keccak() {
        // Recompute and assert vs the const so we never silently drift.
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"OffchainLookup(address,string[],bytes,bytes4,bytes)");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &OFFCHAIN_LOOKUP_SELECTOR);
    }

    #[test]
    fn parse_short_revert_returns_too_short() {
        let err = parse_offchain_lookup_revert(&[0x55, 0x6f]).unwrap_err();
        assert!(matches!(err, CcipError::RevertTooShort(2)));
    }

    #[test]
    fn parse_wrong_selector_returns_not_offchain_lookup() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef, 0u8, 0u8];
        let err = parse_offchain_lookup_revert(&bytes).unwrap_err();
        assert!(matches!(err, CcipError::NotOffchainLookup));
    }

    #[test]
    fn decode_gateway_response_body_happy() {
        let body = br#"{"data":"0x1234","ttl":60}"#;
        let parsed = decode_gateway_response_body(body).unwrap();
        assert_eq!(parsed.data, "0x1234");
        assert_eq!(parsed.ttl, 60);
    }

    #[test]
    fn decode_gateway_response_body_rejects_garbage() {
        let body = br#"not json"#;
        let err = decode_gateway_response_body(body).unwrap_err();
        assert!(matches!(err, CcipError::Json(_)));
    }

    #[test]
    fn decode_gateway_data_rejects_no_prefix() {
        let err = decode_gateway_data("1234").unwrap_err();
        assert!(matches!(err, CcipError::MissingHexPrefix));
    }

    #[test]
    fn decode_gateway_data_rejects_short() {
        let err = decode_gateway_data("0x00").unwrap_err();
        assert!(matches!(err, CcipError::Hex(_) | CcipError::AbiDecode(_)));
    }

    #[test]
    fn decode_string_result_round_trip() {
        // Hand-built ABI-encoded tuple: (string).
        // head: 0x20 (offset)
        // tail: length=5, "hello", padded to 32
        let mut buf = vec![0u8; 32];
        buf[31] = 0x20; // offset = 32
        // length = 5
        let mut len_word = [0u8; 32];
        len_word[31] = 5;
        buf.extend_from_slice(&len_word);
        // "hello" + 27 bytes padding
        buf.extend_from_slice(b"hello");
        buf.extend_from_slice(&[0u8; 27]);
        let s = decode_string_result(&buf).unwrap();
        assert_eq!(s, "hello");
    }
}
