//! ENS resolver that reads SBO3L text records from a real Ethereum
//! JSON-RPC endpoint (B3 resolution side).
//!
//! Two-step resolution per ENSIP-1:
//! 1. Call `ENSRegistry.resolver(node)` to get the resolver address
//!    actually registered for this name. Fails fast (`UnknownName`)
//!    when the name isn't registered or doesn't have a resolver set.
//! 2. Call `Resolver.text(node, key)` for each of the five SBO3L
//!    text-record keys (`sbo3l:agent_id`, `sbo3l:endpoint`,
//!    `sbo3l:policy_hash`, `sbo3l:audit_root`, `sbo3l:receipt_schema`).
//!    Any missing key surfaces as `MissingRecord`.
//!
//! ENS bounty rule "no hardcoded values": the *agent's* identity
//! (policy_hash, audit_root, etc.) is read from chain. Network-level
//! contract addresses (the ENS Registry, the Public Resolver) are
//! well-known public infrastructure and ARE hardcoded — the rule is
//! about the agent, not the protocol.
//!
//! Testability: HTTP/JSON-RPC is hidden behind the `JsonRpcTransport`
//! trait. The production impl ([`ReqwestTransport`]) uses
//! `reqwest::blocking`. Tests inject a fake transport that returns
//! canned responses — no network, no mockito server, fully offline
//! CI.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ens::{EnsRecords, EnsResolver, ResolveError};
use crate::ens_anchor::{namehash, EnsNetwork};

/// ENS Registry address. Same on every network the registry has
/// been deployed on (mainnet, sepolia, holesky, ...). Public
/// infrastructure; hardcoded.
pub const ENS_REGISTRY_ADDRESS: &str = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e";

/// Selector for `resolver(bytes32 node) returns (address)`.
/// Pinned in tests against `keccak256("resolver(bytes32)")[0..4]`.
pub const RESOLVER_SELECTOR: [u8; 4] = [0x01, 0x78, 0xb8, 0xbf];

/// Selector for `text(bytes32 node, string key) returns (string)`.
/// Pinned in tests against `keccak256("text(bytes32,string)")[0..4]`.
pub const TEXT_SELECTOR: [u8; 4] = [0x59, 0xd1, 0xd4, 0x3c];

/// The five SBO3L text-record keys read by [`LiveEnsResolver`].
/// Order is the field order of [`EnsRecords`]; the resolver makes
/// one `text()` call per key, in this order.
pub const SBO3L_TEXT_KEYS: [&str; 5] = [
    "sbo3l:agent_id",
    "sbo3l:endpoint",
    "sbo3l:policy_hash",
    "sbo3l:audit_root",
    "sbo3l:receipt_schema",
];

/// Reasons a JSON-RPC call can fail. Surfaced through
/// [`ResolveError::Io`] / [`ResolveError::Json`] so callers don't
/// need to fan in a new error type.
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("RPC HTTP error: {0}")]
    Http(String),
    #[error("RPC JSON parse error: {0}")]
    Parse(String),
    #[error("RPC reported error: {code} {message}")]
    Server { code: i64, message: String },
    #[error("malformed eth_call response: {0}")]
    Decode(String),
}

impl From<RpcError> for ResolveError {
    fn from(e: RpcError) -> Self {
        ResolveError::Io(std::io::Error::other(e.to_string()))
    }
}

/// Synchronous JSON-RPC transport. Production uses
/// [`ReqwestTransport`]; tests inject a fake.
pub trait JsonRpcTransport {
    /// Call `eth_call` against `to` with `data` (both `0x`-prefixed
    /// hex strings). Returns the `0x`-prefixed hex result string.
    fn eth_call(&self, to: &str, data: &str) -> Result<String, RpcError>;
}

#[derive(Debug, Serialize)]
struct EthCallTx<'a> {
    to: &'a str,
    data: &'a str,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    result: Option<String>,
    error: Option<JsonRpcErrorBody>,
    #[allow(dead_code)]
    id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcErrorBody {
    code: i64,
    message: String,
}

/// Production [`JsonRpcTransport`] backed by `reqwest::blocking`.
/// Builds on top of the workspace reqwest with the additional
/// `blocking` feature (scoped to this crate).
pub struct ReqwestTransport {
    rpc_url: String,
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new(rpc_url: String) -> Self {
        // Default timeout matches a generous user expectation. The
        // operator can wire in a custom client through
        // `with_client` if they need different timing.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest::blocking::Client::builder cannot fail with default config");
        Self { rpc_url, client }
    }

    pub fn with_client(rpc_url: String, client: reqwest::blocking::Client) -> Self {
        Self { rpc_url, client }
    }
}

impl JsonRpcTransport for ReqwestTransport {
    fn eth_call(&self, to: &str, data: &str) -> Result<String, RpcError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [
                EthCallTx { to, data },
                "latest"
            ],
            "id": 1u64
        });
        let resp = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .map_err(|e| RpcError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(RpcError::Http(format!("status {}", resp.status())));
        }
        let parsed: JsonRpcResponse = resp.json().map_err(|e| RpcError::Parse(e.to_string()))?;
        if let Some(err) = parsed.error {
            return Err(RpcError::Server {
                code: err.code,
                message: err.message,
            });
        }
        parsed
            .result
            .ok_or_else(|| RpcError::Decode("no result and no error in response".into()))
    }
}

/// Live ENS resolver. Generic over a [`JsonRpcTransport`] so the
/// production HTTP impl and the in-test fake share one code path.
pub struct LiveEnsResolver<T: JsonRpcTransport> {
    transport: T,
    network: EnsNetwork,
}

impl<T: JsonRpcTransport> LiveEnsResolver<T> {
    pub fn new(transport: T, network: EnsNetwork) -> Self {
        Self { transport, network }
    }

    pub fn network(&self) -> EnsNetwork {
        self.network
    }
}

impl LiveEnsResolver<ReqwestTransport> {
    /// Construct a resolver from `SBO3L_ENS_RPC_URL`. Returns
    /// [`ResolveError::Io`] if the env var is unset or empty — the
    /// caller (CLI / demo script) decides whether to fall back to
    /// the offline resolver or surface the error.
    pub fn from_env(network: EnsNetwork) -> Result<Self, ResolveError> {
        let url = std::env::var("SBO3L_ENS_RPC_URL").map_err(|_| {
            ResolveError::Io(std::io::Error::other(
                "SBO3L_ENS_RPC_URL not set; cannot construct LiveEnsResolver",
            ))
        })?;
        if url.is_empty() {
            return Err(ResolveError::Io(std::io::Error::other(
                "SBO3L_ENS_RPC_URL is empty",
            )));
        }
        Ok(Self::new(ReqwestTransport::new(url), network))
    }
}

impl<T: JsonRpcTransport> EnsResolver for LiveEnsResolver<T> {
    fn resolve(&self, name: &str) -> Result<EnsRecords, ResolveError> {
        let node = namehash(name).map_err(|_| ResolveError::UnknownName(name.to_string()))?;

        // Step 1: ENSRegistry.resolver(node) → resolver address.
        let resolver_addr = call_resolver(&self.transport, &node)?;
        if is_zero_address(&resolver_addr) {
            return Err(ResolveError::UnknownName(name.to_string()));
        }

        // Step 2: Resolver.text(node, key) for each SBO3L key.
        let mut values = HashMap::with_capacity(SBO3L_TEXT_KEYS.len());
        for key in SBO3L_TEXT_KEYS {
            let value = call_text(&self.transport, &resolver_addr, &node, key)?;
            if value.is_empty() {
                return Err(ResolveError::MissingRecord(
                    static_key_label(key),
                    name.to_string(),
                ));
            }
            values.insert(key, value);
        }

        // Reassemble into the EnsRecords struct, preserving the
        // serde rename order of the existing offline format.
        Ok(EnsRecords {
            agent_id: values.remove("sbo3l:agent_id").unwrap_or_default(),
            endpoint: values.remove("sbo3l:endpoint").unwrap_or_default(),
            policy_hash: values.remove("sbo3l:policy_hash").unwrap_or_default(),
            audit_root: values.remove("sbo3l:audit_root").unwrap_or_default(),
            receipt_schema: values.remove("sbo3l:receipt_schema").unwrap_or_default(),
        })
    }
}

/// Map runtime key → static-str expected by `MissingRecord`. The
/// trait already uses `&'static str` here so the live path can't
/// silently leak a heap-allocated string into that variant.
fn static_key_label(key: &str) -> &'static str {
    match key {
        "sbo3l:agent_id" => "agent_id",
        "sbo3l:endpoint" => "endpoint",
        "sbo3l:policy_hash" => "policy_hash",
        "sbo3l:audit_root" => "audit_root",
        "sbo3l:receipt_schema" => "receipt_schema",
        _ => "unknown",
    }
}

fn call_resolver<T: JsonRpcTransport>(transport: &T, node: &[u8; 32]) -> Result<String, RpcError> {
    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(&RESOLVER_SELECTOR);
    data.extend_from_slice(node);
    let hex_data = format!("0x{}", hex::encode(&data));
    let raw = transport.eth_call(ENS_REGISTRY_ADDRESS, &hex_data)?;
    decode_address(&raw)
}

fn call_text<T: JsonRpcTransport>(
    transport: &T,
    resolver_addr: &str,
    node: &[u8; 32],
    key: &str,
) -> Result<String, RpcError> {
    let data = encode_text_call(node, key);
    let hex_data = format!("0x{}", hex::encode(&data));
    let raw = transport.eth_call(resolver_addr, &hex_data)?;
    decode_string(&raw)
}

fn encode_text_call(node: &[u8; 32], key: &str) -> Vec<u8> {
    // text(bytes32 node, string key): selector || node (32 B) ||
    // string offset (32 B = 0x40) || string length (32 B) || padded
    // string bytes.
    let mut out = Vec::with_capacity(4 + 32 * 3 + key.len() + 32);
    out.extend_from_slice(&TEXT_SELECTOR);
    out.extend_from_slice(node);
    let key_offset: u64 = 0x40;
    out.extend_from_slice(&u256_be(key_offset));
    out.extend_from_slice(&u256_be(key.len() as u64));
    out.extend_from_slice(key.as_bytes());
    let padded = key.len().div_ceil(32) * 32;
    let pad = padded - key.len();
    out.extend(std::iter::repeat_n(0u8, pad));
    out
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

/// Decode an `eth_call` result whose ABI-encoded body is a single
/// `address`. The on-the-wire shape is 32 bytes with the address
/// right-aligned (12 leading zero bytes). Returns the lowercase
/// `0x`-prefixed 42-char address string.
fn decode_address(hex_response: &str) -> Result<String, RpcError> {
    let body = strip_0x(hex_response)?;
    let bytes = hex::decode(body).map_err(|e| RpcError::Decode(format!("hex decode: {e}")))?;
    if bytes.len() < 32 {
        return Err(RpcError::Decode(format!(
            "address response too short: {} bytes",
            bytes.len()
        )));
    }
    // First 12 bytes must be zero.
    if !bytes[..12].iter().all(|&b| b == 0) {
        return Err(RpcError::Decode(
            "address response: leading 12 bytes must be zero".into(),
        ));
    }
    let addr_bytes = &bytes[12..32];
    Ok(format!("0x{}", hex::encode(addr_bytes)))
}

/// Decode an `eth_call` result whose ABI-encoded body is a single
/// `string memory`. The on-the-wire shape is offset (32 B, typically
/// 0x20) || length (32 B) || padded UTF-8 bytes.
fn decode_string(hex_response: &str) -> Result<String, RpcError> {
    let body = strip_0x(hex_response)?;
    let bytes = hex::decode(body).map_err(|e| RpcError::Decode(format!("hex decode: {e}")))?;
    // Empty `eth_call` result (the resolver returned the type's default
    // — for `string`, an empty string) — surfaced as empty string so
    // the caller can distinguish "missing record" from "real value".
    if bytes.is_empty() {
        return Ok(String::new());
    }
    if bytes.len() < 64 {
        return Err(RpcError::Decode(format!(
            "string response too short: {} bytes",
            bytes.len()
        )));
    }
    let offset = u256_to_usize(&bytes[..32])?;
    if offset + 32 > bytes.len() {
        return Err(RpcError::Decode(format!(
            "string offset {offset} past response of {} bytes",
            bytes.len()
        )));
    }
    let length = u256_to_usize(&bytes[offset..offset + 32])?;
    let start = offset + 32;
    let end = start + length;
    if end > bytes.len() {
        return Err(RpcError::Decode(format!(
            "string length {length} past response (start={start}, end={end}, total={})",
            bytes.len()
        )));
    }
    let s = std::str::from_utf8(&bytes[start..end])
        .map_err(|e| RpcError::Decode(format!("utf8 decode: {e}")))?;
    Ok(s.to_string())
}

fn u256_to_usize(b: &[u8]) -> Result<usize, RpcError> {
    if b.len() != 32 {
        return Err(RpcError::Decode("u256 slice not 32 bytes".into()));
    }
    // We expect small numbers (offset / length ≤ a few KiB). Reject
    // anything that overflows usize — that shape can't be a valid
    // text record.
    if !b[..24].iter().all(|&x| x == 0) {
        return Err(RpcError::Decode(
            "u256 too large to be a string offset/length".into(),
        ));
    }
    let mut be = [0u8; 8];
    be.copy_from_slice(&b[24..32]);
    Ok(u64::from_be_bytes(be) as usize)
}

fn strip_0x(s: &str) -> Result<&str, RpcError> {
    s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .ok_or_else(|| RpcError::Decode(format!("response missing `0x` prefix: {s:?}")))
}

fn is_zero_address(addr: &str) -> bool {
    addr.eq_ignore_ascii_case("0x0000000000000000000000000000000000000000")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use tiny_keccak::{Hasher, Keccak};

    fn keccak256(data: &[u8]) -> [u8; 32] {
        let mut hasher = Keccak::v256();
        hasher.update(data);
        let mut out = [0u8; 32];
        hasher.finalize(&mut out);
        out
    }

    #[test]
    fn resolver_selector_matches_signature() {
        let derived = keccak256(b"resolver(bytes32)");
        assert_eq!(derived[..4], RESOLVER_SELECTOR);
    }

    #[test]
    fn text_selector_matches_signature() {
        let derived = keccak256(b"text(bytes32,string)");
        assert_eq!(derived[..4], TEXT_SELECTOR);
    }

    /// One scripted expectation: `(to, data_prefix, response)`.
    type Expectation = (String, String, Result<String, RpcError>);

    /// In-test fake transport. Records calls and replays scripted
    /// responses. Not threadsafe — that's fine for unit tests.
    struct FakeTransport {
        scripted: RefCell<Vec<Expectation>>,
        calls: RefCell<Vec<(String, String)>>,
    }

    impl FakeTransport {
        fn new() -> Self {
            Self {
                scripted: RefCell::new(Vec::new()),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn expect(&self, to: &str, data_prefix: &str, response: Result<String, RpcError>) {
            self.scripted
                .borrow_mut()
                .push((to.to_string(), data_prefix.to_string(), response));
        }
    }

    impl JsonRpcTransport for FakeTransport {
        fn eth_call(&self, to: &str, data: &str) -> Result<String, RpcError> {
            self.calls
                .borrow_mut()
                .push((to.to_string(), data.to_string()));
            let mut script = self.scripted.borrow_mut();
            // Pop the next expectation matching this `to`.
            let pos = script
                .iter()
                .position(|(t, dp, _)| t.eq_ignore_ascii_case(to) && data.starts_with(dp.as_str()));
            match pos {
                Some(i) => script.remove(i).2,
                None => Err(RpcError::Decode(format!(
                    "FakeTransport: no expectation matched to={to} data={data}"
                ))),
            }
        }
    }

    /// Encode a string into the ABI-`returns(string)` shape so we
    /// can build canned responses for the fake transport.
    fn abi_encode_string_response(s: &str) -> String {
        let mut out = Vec::with_capacity(64 + s.len() + 32);
        out.extend_from_slice(&u256_be(0x20));
        out.extend_from_slice(&u256_be(s.len() as u64));
        out.extend_from_slice(s.as_bytes());
        let pad = (s.len().div_ceil(32) * 32) - s.len();
        out.extend(std::iter::repeat_n(0u8, pad));
        format!("0x{}", hex::encode(out))
    }

    fn abi_encode_address_response(addr_no_prefix: &str) -> String {
        // 12 zero bytes + 20 address bytes.
        format!("0x{}{}", "00".repeat(12), addr_no_prefix)
    }

    fn full_records_fixture() -> [(&'static str, &'static str); 5] {
        [
            ("sbo3l:agent_id", "research-agent-01"),
            ("sbo3l:endpoint", "http://127.0.0.1:8730/v1"),
            (
                "sbo3l:policy_hash",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            (
                "sbo3l:audit_root",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            (
                "sbo3l:receipt_schema",
                "https://schemas.sbo3l.dev/policy-receipt/v1.json",
            ),
        ]
    }

    #[test]
    fn happy_path_resolves_all_five_records() {
        let transport = FakeTransport::new();
        let resolver_addr = "0xed79b9b96c6f44ee7b8e1ad1c2519bba2cdcc7d3";

        // Step 1: registry.resolver(node) → 32-byte left-padded address.
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(
                resolver_addr.strip_prefix("0x").unwrap(),
            )),
        );
        // Step 2: resolver.text(node, "sbo3l:*") for each key.
        for (k, v) in full_records_fixture() {
            transport.expect(
                resolver_addr,
                "0x59d1d43c",
                Ok(abi_encode_string_response(v)),
            );
            let _ = k; // ordering is enforced by our resolver loop
        }

        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Mainnet);
        let recs = resolver.resolve("research-agent.team.eth").unwrap();
        let expected = full_records_fixture();
        assert_eq!(recs.agent_id, expected[0].1);
        assert_eq!(recs.endpoint, expected[1].1);
        assert_eq!(recs.policy_hash, expected[2].1);
        assert_eq!(recs.audit_root, expected[3].1);
        assert_eq!(recs.receipt_schema, expected[4].1);
    }

    #[test]
    fn unknown_name_when_resolver_is_zero_address() {
        let transport = FakeTransport::new();
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(&"00".repeat(20))),
        );
        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Sepolia);
        let err = resolver.resolve("ghost.eth").unwrap_err();
        assert!(matches!(err, ResolveError::UnknownName(_)));
    }

    #[test]
    fn missing_text_record_surfaces_as_missing_record_error() {
        let transport = FakeTransport::new();
        let resolver_addr = "0xed79b9b96c6f44ee7b8e1ad1c2519bba2cdcc7d3";
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(
                resolver_addr.strip_prefix("0x").unwrap(),
            )),
        );
        // First text() returns empty string — that's the
        // "missing record" condition.
        transport.expect(
            resolver_addr,
            "0x59d1d43c",
            Ok(abi_encode_string_response("")),
        );
        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Mainnet);
        let err = resolver.resolve("incomplete.eth").unwrap_err();
        match err {
            ResolveError::MissingRecord(field, _) => assert_eq!(field, "agent_id"),
            other => panic!("expected MissingRecord, got {other:?}"),
        }
    }

    #[test]
    fn rpc_server_error_surfaces_as_io_error() {
        let transport = FakeTransport::new();
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Err(RpcError::Server {
                code: -32000,
                message: "rate limited".into(),
            }),
        );
        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Mainnet);
        let err = resolver.resolve("any.eth").unwrap_err();
        assert!(matches!(err, ResolveError::Io(_)));
        let msg = err.to_string();
        assert!(
            msg.contains("rate limited") && msg.contains("-32000"),
            "expected RPC error to surface code + message, got: {msg}"
        );
    }

    #[test]
    fn http_transport_error_surfaces_as_io_error() {
        let transport = FakeTransport::new();
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Err(RpcError::Http("timeout".into())),
        );
        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Mainnet);
        let err = resolver.resolve("any.eth").unwrap_err();
        assert!(matches!(err, ResolveError::Io(_)));
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn malformed_address_response_surfaces_as_io_error() {
        let transport = FakeTransport::new();
        // 32 bytes but with non-zero prefix — must be rejected.
        let bad = format!(
            "0xff{}{}",
            "00".repeat(11),
            "ed79b9b96c6f44ee7b8e1ad1c2519bba2cdcc7d3"
        );
        transport.expect(ENS_REGISTRY_ADDRESS, "0x0178b8bf", Ok(bad));
        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Mainnet);
        let err = resolver.resolve("any.eth").unwrap_err();
        assert!(matches!(err, ResolveError::Io(_)));
    }

    #[test]
    fn from_env_errors_when_unset() {
        // Save and clear the var. There is no `Result::expect_err`
        // for env::var; we just unset and confirm behaviour.
        let saved = std::env::var("SBO3L_ENS_RPC_URL").ok();
        std::env::remove_var("SBO3L_ENS_RPC_URL");
        let r = LiveEnsResolver::from_env(EnsNetwork::Mainnet);
        assert!(matches!(r, Err(ResolveError::Io(_))));
        // Restore so we don't pollute other tests in the same process.
        if let Some(v) = saved {
            std::env::set_var("SBO3L_ENS_RPC_URL", v);
        }
    }

    #[test]
    fn encode_text_call_layout_matches_abi() {
        let node = [0u8; 32];
        let key = "sbo3l:agent_id"; // 14 bytes → padded to 32
        let cd = encode_text_call(&node, key);
        // selector(4) + node(32) + offset(32) + length(32) + padded_key(32)
        assert_eq!(cd.len(), 4 + 32 + 32 + 32 + 32);
        assert_eq!(cd[..4], TEXT_SELECTOR);
        // length word.
        let len_word = &cd[68..100];
        assert_eq!(u64::from_be_bytes(len_word[24..].try_into().unwrap()), 14);
        // key bytes at the right offset.
        assert_eq!(&cd[100..100 + 14], key.as_bytes());
        // Padding bytes are zero.
        assert!(cd[100 + 14..132].iter().all(|&b| b == 0));
    }

    #[test]
    fn decode_string_round_trips() {
        let raw = abi_encode_string_response("hello world");
        let s = decode_string(&raw).unwrap();
        assert_eq!(s, "hello world");
    }

    #[test]
    fn decode_address_zero_is_normalised_to_lowercase() {
        let raw = abi_encode_address_response(&"00".repeat(20));
        let a = decode_address(&raw).unwrap();
        assert!(is_zero_address(&a));
    }
}
