//! ENS Universal Resolver integration (T-4-5).
//!
//! Wraps the canonical ENS Universal Resolver contract so the
//! five `sbo3l:*` text records can be resolved in a **single
//! `eth_call`**, down from the 1+5 calls that
//! [`crate::ens_live::LiveEnsResolver`] performs.
//!
//! ## How it works
//!
//! `UniversalResolver.resolve(bytes name, bytes data)` accepts a
//! DNS-encoded ENS name plus the inner calldata to forward to
//! whichever resolver the registry has registered for that name.
//! We pack a single `multicall(bytes[])` call carrying all five
//! `text(node, key)` queries; the universal resolver does the
//! registry lookup + multicall dispatch in one shot and returns
//! `(bytes result, address resolver)` — the `result` is the
//! ABI-encoded `bytes[]` from the inner multicall.
//!
//! Two layers of dynamic-ABI decoding peel that apart:
//!
//! 1. Outer: `(bytes, address)` → the multicall return + the
//!    resolver address that handled it.
//! 2. Multicall return: `bytes[]` → N entries, each the raw return
//!    of one inner `text(node, key)` call.
//! 3. Each entry: `(string)` → the actual record value.
//!
//! ## Scope: on-chain resolver fast path
//!
//! Useful when the resolved name's records live directly on a
//! standard PublicResolver — exactly the shape of mainnet
//! `sbo3lagent.eth` (the SBO3L apex). For names backed by an
//! ENSIP-10 OffchainResolver the universal resolver still propagates
//! the inner `OffchainLookup` revert; the existing
//! [`crate::ccip_read`] flow + [`LiveEnsResolver`] remain the right
//! tools there. This module is therefore a strict optimisation: it
//! never makes things worse, and on the apex it cuts five round-trips
//! to one.
//!
//! ## Address pinning
//!
//! The Universal Resolver address per network is taken from the
//! canonical ENS deployments (the same constants `viem` uses).
//! Override via [`UniversalResolver::with_address`] if a future
//! ENS upgrade reshuffles the registry — the contract ABI is the
//! stable contract-of-trust here, not the address.

use crate::ccip_read::{CcipError, OFFCHAIN_LOOKUP_SELECTOR};
use crate::durin::MULTICALL_SELECTOR;
use crate::ens::{EnsRecords, EnsResolver, ResolveError};
use crate::ens_anchor::{namehash, EnsNetwork};
use crate::ens_live::{JsonRpcTransport, RpcError, SBO3L_TEXT_KEYS, TEXT_SELECTOR};

/// Selector for `resolve(bytes name, bytes data)` on UniversalResolver.
/// Pinned in tests against `keccak256("resolve(bytes,bytes)")[..4]`.
pub const UNIVERSAL_RESOLVE_SELECTOR: [u8; 4] = [0x90, 0x61, 0xb9, 0x23];

/// Mainnet ENS Universal Resolver (latest stable as of 2026 Q2).
/// Same address `viem` ships with; override with
/// [`UniversalResolver::with_address`] if a future redeploy moves it.
pub const UNIVERSAL_RESOLVER_MAINNET: &str = "0xce01f8eee7E479C928F8919abD53E553a36CeF67";

/// Sepolia ENS Universal Resolver. Pair to [`UNIVERSAL_RESOLVER_MAINNET`].
pub const UNIVERSAL_RESOLVER_SEPOLIA: &str = "0xc8Af999e38273D658BE1b921b88A9Ddf005769cC";

/// Universal-resolver-side error surface.
#[derive(Debug, thiserror::Error)]
pub enum UniversalError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),

    #[error("DNS-encode failed: label longer than 63 bytes ({0} bytes)")]
    DnsLabelTooLong(usize),

    #[error("response too short: {0} bytes")]
    ResponseTooShort(usize),

    #[error("ABI decode error: {0}")]
    AbiDecode(String),

    #[error("CCIP-Read fallback required (offchain resolver) — use LiveEnsResolver")]
    OffchainResolverRequiresCcipFlow,

    #[error("multicall returned {0} entries, expected {1}")]
    MulticallArityMismatch(usize, usize),

    #[error(transparent)]
    Rpc(#[from] RpcError),

    #[error(transparent)]
    Ccip(#[from] CcipError),
}

impl From<UniversalError> for ResolveError {
    fn from(e: UniversalError) -> Self {
        match e {
            UniversalError::Resolve(r) => r,
            other => ResolveError::Io(std::io::Error::other(other.to_string())),
        }
    }
}

/// Universal-resolver client. Generic over the JSON-RPC transport
/// for the same testability story as [`LiveEnsResolver`]: production
/// uses `ReqwestTransport`, tests inject a fake.
pub struct UniversalResolver<T: JsonRpcTransport> {
    transport: T,
    network: EnsNetwork,
    address: String,
}

impl<T: JsonRpcTransport> UniversalResolver<T> {
    /// Construct with the canonical address for `network`.
    pub fn new(transport: T, network: EnsNetwork) -> Self {
        let address = match network {
            EnsNetwork::Mainnet => UNIVERSAL_RESOLVER_MAINNET,
            EnsNetwork::Sepolia => UNIVERSAL_RESOLVER_SEPOLIA,
        };
        Self {
            transport,
            network,
            address: address.to_string(),
        }
    }

    /// Override the universal-resolver address. For ENS upgrades or
    /// custom deployments. The address must be a `0x`-prefixed
    /// 40-hex-char string; not validated here — RPC will surface
    /// any malformed input as a server error.
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        self.address = address.into();
        self
    }

    pub fn network(&self) -> EnsNetwork {
        self.network
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    /// Resolve all five `sbo3l:*` text records in a single eth_call.
    /// Returns [`UniversalError::OffchainResolverRequiresCcipFlow`]
    /// if the resolved name is behind an ENSIP-10 OffchainResolver
    /// — caller should fall back to [`LiveEnsResolver`] which knows
    /// the CCIP-Read dance.
    pub fn resolve_all(&self, name: &str) -> Result<EnsRecords, UniversalError> {
        let node = namehash(name)
            .map_err(|_| UniversalError::Resolve(ResolveError::UnknownName(name.to_string())))?;

        // Build inner multicall: [text(node, key) for key in SBO3L_TEXT_KEYS]
        let inner_calls: Vec<Vec<u8>> = SBO3L_TEXT_KEYS
            .iter()
            .map(|k| encode_text_call(&node, k))
            .collect();
        let multicall_data = encode_multicall(&inner_calls);

        // Build outer: UniversalResolver.resolve(dnsName, multicallData)
        let dns_name = dns_encode(name)?;
        let outer_data = encode_universal_resolve(&dns_name, &multicall_data);
        let hex_data = format!("0x{}", hex::encode(&outer_data));

        let raw = self.transport.eth_call(&self.address, &hex_data);

        let response_hex = match raw {
            Ok(s) => s,
            Err(RpcError::Server { message, .. }) if message_contains_offchain_lookup(&message) => {
                return Err(UniversalError::OffchainResolverRequiresCcipFlow);
            }
            Err(e) => return Err(UniversalError::Rpc(e)),
        };

        // Outer decode: (bytes result, address resolver)
        let response_bytes = strip_0x_decode(&response_hex)?;
        let (multicall_result, _resolver_addr) = decode_outer_resolve_response(&response_bytes)?;

        // Multicall decode: bytes[]
        let inner_returns = decode_bytes_array(&multicall_result)?;
        if inner_returns.len() != SBO3L_TEXT_KEYS.len() {
            return Err(UniversalError::MulticallArityMismatch(
                inner_returns.len(),
                SBO3L_TEXT_KEYS.len(),
            ));
        }

        // Each entry: (string) — decode all five.
        let mut values: Vec<String> = Vec::with_capacity(SBO3L_TEXT_KEYS.len());
        for (i, raw) in inner_returns.iter().enumerate() {
            let s = decode_string_tuple(raw).map_err(|e| {
                UniversalError::AbiDecode(format!(
                    "text record {} ({}): {}",
                    i, SBO3L_TEXT_KEYS[i], e
                ))
            })?;
            if s.is_empty() {
                return Err(UniversalError::Resolve(ResolveError::MissingRecord(
                    static_key_label(SBO3L_TEXT_KEYS[i]),
                    name.to_string(),
                )));
            }
            values.push(s);
        }

        Ok(EnsRecords {
            agent_id: values[0].clone(),
            endpoint: values[1].clone(),
            policy_hash: values[2].clone(),
            audit_root: values[3].clone(),
            proof_uri: values[4].clone(),
        })
    }
}

impl<T: JsonRpcTransport> EnsResolver for UniversalResolver<T> {
    fn resolve(&self, name: &str) -> Result<EnsRecords, ResolveError> {
        self.resolve_all(name).map_err(|e| e.into())
    }
}

fn static_key_label(key: &str) -> &'static str {
    match key {
        "sbo3l:agent_id" => "agent_id",
        "sbo3l:endpoint" => "endpoint",
        "sbo3l:policy_hash" => "policy_hash",
        "sbo3l:audit_root" => "audit_root",
        "sbo3l:proof_uri" => "proof_uri",
        _ => "unknown",
    }
}

/// DNS-encode an ENS name. Each label is prefixed with its length
/// byte; the whole thing is null-terminated. Per RFC 1035 a single
/// label may be at most 63 bytes — anything longer is rejected.
pub fn dns_encode(name: &str) -> Result<Vec<u8>, UniversalError> {
    let mut out = Vec::with_capacity(name.len() + 2);
    if !name.is_empty() {
        for label in name.split('.') {
            if label.is_empty() {
                continue;
            }
            let bytes = label.as_bytes();
            if bytes.len() > 63 {
                return Err(UniversalError::DnsLabelTooLong(bytes.len()));
            }
            out.push(bytes.len() as u8);
            out.extend_from_slice(bytes);
        }
    }
    out.push(0);
    Ok(out)
}

/// Encode `text(bytes32 node, string key)` calldata. Mirrors
/// `ens_live::encode_text_call` — kept here to avoid making that
/// function `pub` purely to support a new caller in this module.
fn encode_text_call(node: &[u8; 32], key: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 3 + key.len() + 32);
    out.extend_from_slice(&TEXT_SELECTOR);
    out.extend_from_slice(node);
    out.extend_from_slice(&u256_be(0x40));
    out.extend_from_slice(&u256_be(key.len() as u64));
    out.extend_from_slice(key.as_bytes());
    let padded = key.len().div_ceil(32) * 32;
    out.extend(std::iter::repeat_n(0u8, padded - key.len()));
    out
}

/// Encode `multicall(bytes[] data)`. Selector + dynamic `bytes[]`
/// payload. ABI layout for `bytes[]`:
///   word 0: offset to array (always 0x20 here — single arg)
///   word 1: array length N
///   words 2..2+N: head offsets (relative to start of the array,
///                  i.e., from word 1 down)
///   ...padded element bodies (length || padded bytes) ...
fn encode_multicall(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 64 + items.len() * 64);
    out.extend_from_slice(&MULTICALL_SELECTOR);

    // Arg head: offset to array body = 32.
    out.extend_from_slice(&u256_be(0x20));

    // Array body: length || head offsets || tails.
    out.extend_from_slice(&u256_be(items.len() as u64));

    // Compute padded sizes per item, then offsets relative to start
    // of *array body* (i.e., from the length word forward — but
    // the standard ABI offsets are from after the length word).
    // Per Solidity's bytes[] encoding: offsets are from the start
    // of the head section (= the first head word, just after the
    // length word).
    let mut tails: Vec<Vec<u8>> = Vec::with_capacity(items.len());
    let mut padded_sizes: Vec<usize> = Vec::with_capacity(items.len());
    for item in items {
        let mut tail = Vec::with_capacity(32 + item.len() + 32);
        tail.extend_from_slice(&u256_be(item.len() as u64));
        tail.extend_from_slice(item);
        let pad = item.len().div_ceil(32) * 32 - item.len();
        tail.extend(std::iter::repeat_n(0u8, pad));
        padded_sizes.push(tail.len());
        tails.push(tail);
    }

    // Heads section: N words, each = offset to that element's tail
    // measured from the START of the heads section.
    let heads_size = items.len() * 32;
    let mut cursor = heads_size as u64;
    for size in &padded_sizes {
        out.extend_from_slice(&u256_be(cursor));
        cursor += *size as u64;
    }
    for tail in &tails {
        out.extend_from_slice(tail);
    }

    out
}

/// Encode `UniversalResolver.resolve(bytes name, bytes data)`.
fn encode_universal_resolve(name: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 64 + name.len() + data.len() + 128);
    out.extend_from_slice(&UNIVERSAL_RESOLVE_SELECTOR);

    // Two heads: offset to name (0x40), offset to data (after name's tail).
    let name_padded = name.len().div_ceil(32) * 32;
    let data_offset: u64 = 0x40 + 32 + name_padded as u64;

    out.extend_from_slice(&u256_be(0x40));
    out.extend_from_slice(&u256_be(data_offset));

    // name tail
    out.extend_from_slice(&u256_be(name.len() as u64));
    out.extend_from_slice(name);
    out.extend(std::iter::repeat_n(0u8, name_padded - name.len()));

    // data tail
    out.extend_from_slice(&u256_be(data.len() as u64));
    out.extend_from_slice(data);
    let data_padded = data.len().div_ceil(32) * 32;
    out.extend(std::iter::repeat_n(0u8, data_padded - data.len()));

    out
}

/// Decode `(bytes result, address resolver)` — the
/// UniversalResolver.resolve return tuple.
fn decode_outer_resolve_response(b: &[u8]) -> Result<(Vec<u8>, [u8; 20]), UniversalError> {
    if b.len() < 64 {
        return Err(UniversalError::ResponseTooShort(b.len()));
    }
    let result_offset = read_u64_word(b, 0)? as usize;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&b[64 - 20..64]);

    let result = read_dynamic_bytes(b, result_offset)?;
    Ok((result, addr))
}

/// Decode `bytes[]` (the multicall return).
fn decode_bytes_array(b: &[u8]) -> Result<Vec<Vec<u8>>, UniversalError> {
    if b.is_empty() {
        return Ok(Vec::new());
    }
    if b.len() < 32 {
        return Err(UniversalError::ResponseTooShort(b.len()));
    }
    let count = read_u64_word(b, 0)? as usize;
    let heads_start = 32usize;
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(count);
    for i in 0..count {
        let head_off = heads_start + i * 32;
        if b.len() < head_off + 32 {
            return Err(UniversalError::AbiDecode(format!("bytes[] head {i} OOB")));
        }
        let element_off_rel = read_u64_word(b, head_off)? as usize;
        // bytes[] element offsets are relative to the start of the
        // heads section, which is `heads_start` here.
        let element_off = heads_start + element_off_rel;
        let entry = read_dynamic_bytes(b, element_off)?;
        out.push(entry);
    }
    Ok(out)
}

/// Decode `(string)` — the standard ABI-encoded string-tuple shape
/// returned by `Resolver.text(node, key)`.
fn decode_string_tuple(b: &[u8]) -> Result<String, UniversalError> {
    if b.is_empty() {
        return Ok(String::new());
    }
    if b.len() < 64 {
        return Err(UniversalError::ResponseTooShort(b.len()));
    }
    let len = read_u64_word(b, 32)? as usize;
    let start = 64usize;
    let end = start + len;
    if b.len() < end {
        return Err(UniversalError::AbiDecode(format!(
            "string tail OOB: end={end} len={}",
            b.len()
        )));
    }
    String::from_utf8(b[start..end].to_vec())
        .map_err(|e| UniversalError::AbiDecode(format!("non-utf8 string: {e}")))
}

fn read_dynamic_bytes(b: &[u8], offset: usize) -> Result<Vec<u8>, UniversalError> {
    if b.len() < offset + 32 {
        return Err(UniversalError::AbiDecode(format!(
            "len-word at {offset} OOB"
        )));
    }
    let len = read_u64_word(b, offset)? as usize;
    let start = offset + 32;
    let end = start + len;
    if b.len() < end {
        return Err(UniversalError::AbiDecode(format!(
            "bytes content {start}..{end} OOB ({})",
            b.len()
        )));
    }
    Ok(b[start..end].to_vec())
}

fn read_u64_word(b: &[u8], off: usize) -> Result<u64, UniversalError> {
    if b.len() < off + 32 {
        return Err(UniversalError::AbiDecode(format!("word at {off} OOB")));
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&b[off + 24..off + 32]);
    Ok(u64::from_be_bytes(buf))
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

fn strip_0x_decode(s: &str) -> Result<Vec<u8>, UniversalError> {
    let body = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    hex::decode(body).map_err(|e| UniversalError::AbiDecode(format!("hex: {e}")))
}

/// Heuristic — RPC servers report contract reverts as e.g.
/// `execution reverted: OffchainLookup` or include the selector
/// `0x556f1830` somewhere in the message. We accept either.
fn message_contains_offchain_lookup(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("offchainlookup") || lower.contains("0x556f1830")
}

/// Public for callers that want to handle the OffchainLookup
/// classification explicitly.
pub fn is_offchain_lookup_revert(revert: &[u8]) -> bool {
    revert.len() >= 4 && revert[..4] == OFFCHAIN_LOOKUP_SELECTOR
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn dns_encode_simple() {
        let out = dns_encode("research.team.eth").unwrap();
        // \x08research\x04team\x03eth\x00
        assert_eq!(out, b"\x08research\x04team\x03eth\x00".to_vec());
    }

    #[test]
    fn dns_encode_empty_returns_root() {
        let out = dns_encode("").unwrap();
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn dns_encode_tolerates_trailing_dot() {
        let with_dot = dns_encode("foo.eth.").unwrap();
        let without = dns_encode("foo.eth").unwrap();
        assert_eq!(with_dot, without);
    }

    #[test]
    fn dns_encode_rejects_long_label() {
        let label = "a".repeat(64);
        let err = dns_encode(&label).unwrap_err();
        assert!(matches!(err, UniversalError::DnsLabelTooLong(64)));
    }

    #[test]
    fn universal_resolve_selector_matches_keccak() {
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"resolve(bytes,bytes)");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &UNIVERSAL_RESOLVE_SELECTOR);
    }

    #[test]
    fn multicall_selector_matches_keccak() {
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"multicall(bytes[])");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &MULTICALL_SELECTOR);
    }

    #[test]
    fn encode_multicall_two_items_decodes_back() {
        let a = vec![0xaa; 4];
        let b = vec![0xbb; 5];
        let raw = encode_multicall(&[a.clone(), b.clone()]);
        // First 4 bytes selector, then strip and try decoding the
        // same-shape `bytes[]` we ship.
        assert_eq!(&raw[..4], &MULTICALL_SELECTOR);
        // arg-head offset (0x20)
        assert_eq!(read_u64_word(&raw[4..], 0).unwrap(), 0x20);

        // Skip selector + 0x20 head, then we're at the array body.
        // body = length(2) || head_off_a(0x40) || head_off_b(...)
        //      || tail_a (length(4) || 0xaa*4 padded)
        //      || tail_b (length(5) || 0xbb*5 padded)
        let body = &raw[4 + 32..];
        let recovered = decode_bytes_array(body).unwrap();
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0], a);
        assert_eq!(recovered[1], b);
    }

    #[test]
    fn encode_universal_resolve_round_trip() {
        let name = dns_encode("foo.eth").unwrap();
        let inner = vec![0xde, 0xad, 0xbe, 0xef];
        let raw = encode_universal_resolve(&name, &inner);
        assert_eq!(&raw[..4], &UNIVERSAL_RESOLVE_SELECTOR);

        // Args section starts at byte 4. The first head is name
        // offset (0x40), the second is data offset.
        let args = &raw[4..];
        assert_eq!(read_u64_word(args, 0).unwrap(), 0x40);

        // Read name from word at 0x40
        let name_off = 0x40usize;
        let name_back = read_dynamic_bytes(args, name_off).unwrap();
        assert_eq!(name_back, name);

        // Read data from second head's offset
        let data_off = read_u64_word(args, 32).unwrap() as usize;
        let data_back = read_dynamic_bytes(args, data_off).unwrap();
        assert_eq!(data_back, inner);
    }

    /// Single-shot fake transport that returns one canned response
    /// regardless of the call. Lets us pin the full happy-path
    /// decode against a hand-built ABI fixture.
    struct OneShotTransport {
        canned: RefCell<Option<String>>,
    }
    impl JsonRpcTransport for OneShotTransport {
        fn eth_call(&self, _to: &str, _data: &str) -> Result<String, RpcError> {
            self.canned
                .borrow_mut()
                .take()
                .ok_or_else(|| RpcError::Decode("transport drained".into()))
        }
    }

    /// Build a valid `(bytes, address)` outer return whose `bytes`
    /// payload is a `bytes[]` of N `(string)` tuples — exactly the
    /// shape the universal resolver returns for a multicall of
    /// text() calls.
    fn build_outer_response(strings: &[&str]) -> Vec<u8> {
        // Inner `bytes[]` body: length || heads || tails
        let mut tails: Vec<Vec<u8>> = Vec::new();
        let mut padded_sizes: Vec<usize> = Vec::new();
        for s in strings {
            // Each entry is a `(string)` tuple = head(0x20) || length || padded-bytes
            let mut tuple = Vec::new();
            tuple.extend_from_slice(&u256_be(0x20));
            tuple.extend_from_slice(&u256_be(s.len() as u64));
            tuple.extend_from_slice(s.as_bytes());
            let pad = s.len().div_ceil(32) * 32 - s.len();
            tuple.extend(std::iter::repeat_n(0u8, pad));

            // Wrap as bytes element (length-prefixed dynamic bytes)
            let mut elem = Vec::new();
            elem.extend_from_slice(&u256_be(tuple.len() as u64));
            elem.extend_from_slice(&tuple);
            let elem_pad = tuple.len().div_ceil(32) * 32 - tuple.len();
            elem.extend(std::iter::repeat_n(0u8, elem_pad));
            padded_sizes.push(elem.len());
            tails.push(elem);
        }
        let mut inner = Vec::new();
        inner.extend_from_slice(&u256_be(strings.len() as u64));
        let heads_size = strings.len() * 32;
        let mut cursor = heads_size as u64;
        for size in &padded_sizes {
            inner.extend_from_slice(&u256_be(cursor));
            cursor += *size as u64;
        }
        for tail in &tails {
            inner.extend_from_slice(tail);
        }

        // Outer (bytes, address) tuple. Head: offset(0x40) || addr
        let mut outer = Vec::new();
        outer.extend_from_slice(&u256_be(0x40));
        // address (right-padded to 32 bytes — addr in low 20)
        let mut addr_word = [0u8; 32];
        for (i, b) in [0xaa; 20].iter().enumerate() {
            addr_word[12 + i] = *b;
        }
        outer.extend_from_slice(&addr_word);

        // bytes payload: length || padded body
        outer.extend_from_slice(&u256_be(inner.len() as u64));
        outer.extend_from_slice(&inner);
        let inner_pad = inner.len().div_ceil(32) * 32 - inner.len();
        outer.extend(std::iter::repeat_n(0u8, inner_pad));

        outer
    }

    #[test]
    fn happy_path_decodes_five_records() {
        let canned = build_outer_response(&[
            "research-agent-01",
            "http://127.0.0.1:8730/v1",
            "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json",
        ]);
        let hex = format!("0x{}", hex::encode(canned));
        let transport = OneShotTransport {
            canned: RefCell::new(Some(hex)),
        };
        let r = UniversalResolver::new(transport, EnsNetwork::Mainnet);
        let recs = r.resolve_all("sbo3lagent.eth").unwrap();
        assert_eq!(recs.agent_id, "research-agent-01");
        assert_eq!(recs.endpoint, "http://127.0.0.1:8730/v1");
        assert_eq!(
            recs.policy_hash,
            "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf"
        );
    }

    #[test]
    fn empty_record_surfaces_missing_record() {
        let canned = build_outer_response(&["", "x", "x", "x", "x"]);
        let hex = format!("0x{}", hex::encode(canned));
        let transport = OneShotTransport {
            canned: RefCell::new(Some(hex)),
        };
        let r = UniversalResolver::new(transport, EnsNetwork::Mainnet);
        let err = r.resolve_all("sbo3lagent.eth").unwrap_err();
        match err {
            UniversalError::Resolve(ResolveError::MissingRecord(field, _)) => {
                assert_eq!(field, "agent_id");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn offchain_lookup_revert_is_classified() {
        // Server-style error message — most public RPCs surface
        // contract reverts as a JSON-RPC error with code 3 / revert
        // data in `data`. The heuristic catches the textual form too.
        let mut payload = vec![0u8; 0];
        payload.extend_from_slice(&OFFCHAIN_LOOKUP_SELECTOR);
        assert!(is_offchain_lookup_revert(&payload));
        assert!(!is_offchain_lookup_revert(&[0; 3]));
        assert!(message_contains_offchain_lookup(
            "execution reverted: OffchainLookup"
        ));
        assert!(message_contains_offchain_lookup("revert with 0x556f1830"));
        assert!(!message_contains_offchain_lookup(
            "vanilla revert: bad data"
        ));
    }

    #[test]
    fn mainnet_address_constant_is_canonical_form() {
        // 0x + 40 hex chars
        assert_eq!(UNIVERSAL_RESOLVER_MAINNET.len(), 42);
        assert!(UNIVERSAL_RESOLVER_MAINNET.starts_with("0x"));
        assert!(UNIVERSAL_RESOLVER_MAINNET[2..]
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn with_address_overrides_default() {
        struct NoopTransport;
        impl JsonRpcTransport for NoopTransport {
            fn eth_call(&self, _to: &str, _data: &str) -> Result<String, RpcError> {
                Err(RpcError::Decode("noop".into()))
            }
        }
        let r = UniversalResolver::new(NoopTransport, EnsNetwork::Mainnet)
            .with_address("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
        assert_eq!(r.address(), "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
    }
}
