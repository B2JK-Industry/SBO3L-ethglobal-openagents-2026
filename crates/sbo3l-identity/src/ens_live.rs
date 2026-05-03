//! ENS resolver that reads SBO3L text records from a real Ethereum
//! JSON-RPC endpoint (B3 resolution side).
//!
//! Two-step resolution per ENSIP-1:
//! 1. Call `ENSRegistry.resolver(node)` to get the resolver address
//!    actually registered for this name. Fails fast (`UnknownName`)
//!    when the name isn't registered or doesn't have a resolver set.
//! 2. Call `Resolver.text(node, key)` for each of the five SBO3L
//!    text-record keys (`sbo3l:agent_id`, `sbo3l:endpoint`,
//!    `sbo3l:policy_hash`, `sbo3l:audit_root`, `sbo3l:proof_uri`).
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
    "sbo3l:proof_uri",
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
    /// Solidity revert. The `data` is the raw revert bytes (selector
    /// || ABI args). Distinct from `Server` so the CCIP-Read follow
    /// path can detect the `OffchainLookup` selector and dispatch
    /// without depending on the human-readable `message`.
    #[error("contract reverted ({} bytes of revert data)", data.len())]
    Reverted { data: Vec<u8> },
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
    ///
    /// On a Solidity revert, return [`RpcError::Reverted`] carrying
    /// the raw revert bytes (selector || ABI args) so callers can
    /// dispatch on the selector — the CCIP-Read follow path needs
    /// access to the `OffchainLookup` payload regardless of the
    /// upstream RPC's human-readable message.
    fn eth_call(&self, to: &str, data: &str) -> Result<String, RpcError>;

    /// Fetch a CCIP-Read gateway URL via HTTP GET. Default impl
    /// surfaces `RpcError::Http("transport does not support HTTP
    /// GET")` — only the live transport needs to implement this; the
    /// test fakes that don't exercise CCIP-Read can leave it
    /// unimplemented. Returns the raw response body bytes.
    fn http_get(&self, _url: &str) -> Result<Vec<u8>, RpcError> {
        Err(RpcError::Http(
            "JsonRpcTransport::http_get not implemented for this transport".into(),
        ))
    }
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
    /// Solidity revert payload; geth/anvil/Alchemy populate this when
    /// a `view` call reverts. Spec: JSON-RPC error data is "Primitive
    /// or structured value". Geth uses a `0x`-hex string for
    /// `eth_call` reverts. Alchemy occasionally wraps it as
    /// `{ "data": "0x..." }`. We accept both shapes (string OR object
    /// with `.data`).
    #[serde(default)]
    data: Option<RevertDataField>,
}

/// `data` field on a JSON-RPC error. Geth returns a hex string;
/// Alchemy sometimes returns `{ "data": "0x...", "message": "..." }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RevertDataField {
    String(String),
    Object {
        #[serde(default)]
        data: Option<String>,
    },
}

impl RevertDataField {
    fn as_hex(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            Self::Object { data } => data.as_deref(),
        }
    }
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
            // If the error carries a hex `data` payload, that's a
            // Solidity revert — surface as `Reverted` so callers can
            // dispatch on the selector. Otherwise fall back to the
            // generic `Server` variant.
            if let Some(hex_data) = err.data.as_ref().and_then(|d| d.as_hex()) {
                if let Some(stripped) = hex_data
                    .strip_prefix("0x")
                    .or_else(|| hex_data.strip_prefix("0X"))
                {
                    if let Ok(bytes) = hex::decode(stripped) {
                        return Err(RpcError::Reverted { data: bytes });
                    }
                }
            }
            return Err(RpcError::Server {
                code: err.code,
                message: err.message,
            });
        }
        parsed
            .result
            .ok_or_else(|| RpcError::Decode("no result and no error in response".into()))
    }

    fn http_get(&self, url: &str) -> Result<Vec<u8>, RpcError> {
        let resp = self
            .client
            .get(url)
            .send()
            .map_err(|e| RpcError::Http(format!("CCIP gateway GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(RpcError::Http(format!(
                "CCIP gateway GET {url}: status {}",
                resp.status()
            )));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| RpcError::Http(format!("CCIP gateway body {url}: {e}")))
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

    /// Read a single ENS text record by raw key. Used by
    /// `sbo3l agent verify-ens` (T-3-2) which needs to read keys
    /// outside the canonical [`SBO3L_TEXT_KEYS`] set (e.g.
    /// `sbo3l:pubkey_ed25519`, `sbo3l:capabilities`,
    /// `sbo3l:policy_url`).
    ///
    /// Resolution dispatch:
    ///
    /// 1. Walk the registry to find the resolver (climbing parent
    ///    names if the leaf has no direct entry — ENSIP-10 wildcard
    ///    behaviour).
    /// 2. If the resolver advertises ENSIP-10 (`supportsInterface(
    ///    0x9061b923) == true`), call `resolve(dnsEncode(name),
    ///    text(node,key))` and follow the EIP-3668 `OffchainLookup`
    ///    revert through the gateway. This is the load-bearing path
    ///    for SBO3L's own subnames behind the OffchainResolver
    ///    (loop-7 UAT fix — without this we couldn't resolve our own
    ///    `research-agent.sbo3lagent.eth` even though viem can).
    /// 3. Otherwise call `text(node, key)` directly (legacy
    ///    PublicResolver path — covers mainnet `sbo3lagent.eth` and
    ///    other apex names with text records pinned on-chain).
    ///
    /// Returns `Ok(None)` for empty / unset records (PublicResolver
    /// convention: missing record → empty string), `Ok(Some(value))`
    /// otherwise.
    pub fn resolve_raw_text(&self, name: &str, key: &str) -> Result<Option<String>, ResolveError> {
        let (resolver_addr, node) = self.find_resolver(name)?;
        let value = if is_zero_address(&resolver_addr) {
            return Err(ResolveError::UnknownName(name.to_string()));
        } else if supports_ensip10(&self.transport, &resolver_addr).unwrap_or(false) {
            call_text_via_ensip10(&self.transport, &resolver_addr, name, &node, key)?
        } else {
            call_text(&self.transport, &resolver_addr, &node, key)?
        };
        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    /// Resolve `name` to the contract that holds its records. Climbs
    /// parent names if the leaf has no direct registry entry — this
    /// is the ENSIP-10 wildcard / parent-resolver pattern that
    /// CCIP-Read clients (viem, ethers) implement. Returns the
    /// resolver address paired with the namehash of the *original*
    /// leaf (not the parent we found the resolver on) — the inner
    /// `text(node, key)` calldata uses the leaf's namehash so the
    /// resolver's gateway dispatches by the right node.
    fn find_resolver(&self, name: &str) -> Result<(String, [u8; 32]), ResolveError> {
        let leaf_node = namehash(name).map_err(|_| ResolveError::UnknownName(name.to_string()))?;
        // Try the leaf first.
        let direct = call_resolver(&self.transport, &leaf_node)?;
        if !is_zero_address(&direct) {
            return Ok((direct, leaf_node));
        }
        // Walk parents. `a.b.c.eth` -> `b.c.eth` -> `c.eth` -> `eth`.
        let mut remaining = name;
        while let Some(idx) = remaining.find('.') {
            remaining = &remaining[idx + 1..];
            if remaining.is_empty() {
                break;
            }
            let parent_node =
                namehash(remaining).map_err(|_| ResolveError::UnknownName(name.to_string()))?;
            let parent_resolver = call_resolver(&self.transport, &parent_node)?;
            if !is_zero_address(&parent_resolver) {
                return Ok((parent_resolver, leaf_node));
            }
        }
        Err(ResolveError::UnknownName(name.to_string()))
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
            proof_uri: values.remove("sbo3l:proof_uri").unwrap_or_default(),
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
        "sbo3l:proof_uri" => "proof_uri",
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

/// `supportsInterface(bytes4)` selector = `0x01ffc9a7`. ENSIP-10
/// `IExtendedResolver` interface id = `0x9061b923`.
const SUPPORTS_INTERFACE_SELECTOR: [u8; 4] = [0x01, 0xff, 0xc9, 0xa7];
const ENSIP10_INTERFACE_ID: [u8; 4] = [0x90, 0x61, 0xb9, 0x23];

/// Probe `supportsInterface(0x9061b923)` on the resolver. ENSIP-10
/// resolvers (the OffchainResolver lineage) advertise this; legacy
/// PublicResolver returns false. We treat any error from the call
/// (resolver doesn't implement ERC-165, RPC blip, etc.) as "no" so
/// the caller falls back to direct `text()` — consistent with viem's
/// pragmatic dispatch.
fn supports_ensip10<T: JsonRpcTransport>(
    transport: &T,
    resolver_addr: &str,
) -> Result<bool, RpcError> {
    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(&SUPPORTS_INTERFACE_SELECTOR);
    // 4-byte interface id, left-aligned in a 32-byte slot.
    let mut padded = [0u8; 32];
    padded[..4].copy_from_slice(&ENSIP10_INTERFACE_ID);
    data.extend_from_slice(&padded);
    let hex_data = format!("0x{}", hex::encode(&data));
    let raw = transport.eth_call(resolver_addr, &hex_data)?;
    let body = strip_0x(&raw)?;
    let bytes = hex::decode(body).map_err(|e| RpcError::Decode(format!("hex decode: {e}")))?;
    if bytes.len() < 32 {
        return Ok(false);
    }
    // ABI-encoded bool: 31 zero bytes + (0|1).
    Ok(bytes[31] == 1)
}

/// ENSIP-10 + EIP-3668 path: call `resolve(dnsEncode(name),
/// text(node, key))` on `resolver_addr`. If it reverts with
/// `OffchainLookup`, follow the gateway round-trip via
/// [`crate::ccip_read::follow_offchain_lookup`] and decode the
/// returned `(string)` tuple.
///
/// Errors:
/// - `RpcError::Reverted` with a non-OffchainLookup selector
///   surfaces verbatim (e.g. `SignatureExpired` from the callback).
/// - `RpcError::Decode` if the gateway's signed result doesn't
///   round-trip to a UTF-8 string (e.g. an `addr(node)` followed by
///   text decoder — caller bug, not gateway bug).
fn call_text_via_ensip10<T: JsonRpcTransport>(
    transport: &T,
    resolver_addr: &str,
    name: &str,
    node: &[u8; 32],
    key: &str,
) -> Result<String, RpcError> {
    let inner = encode_text_call(node, key);
    let dns_name =
        dns_encode(name).map_err(|e| RpcError::Decode(format!("DNS-encode {name}: {e}")))?;
    let outer = encode_resolve_call(&dns_name, &inner);
    let hex_data = format!("0x{}", hex::encode(&outer));

    match transport.eth_call(resolver_addr, &hex_data) {
        Ok(raw) => {
            // `resolve()` returns `bytes` — the inner-call's actual
            // return ABI-encoded inside. For `text()`, that's
            // `(string)`. Outer hex → outer bytes → `(bytes)` tuple
            // → inner bytes → `(string)` tuple.
            let body = strip_0x(&raw)?;
            let outer_bytes =
                hex::decode(body).map_err(|e| RpcError::Decode(format!("hex decode: {e}")))?;
            let inner_bytes = decode_outer_bytes_tuple(&outer_bytes)?;
            decode_string_from_inner(&inner_bytes)
        }
        Err(RpcError::Reverted { data }) => {
            let lookup = crate::ccip_read::parse_offchain_lookup_revert(&data).map_err(|e| {
                RpcError::Decode(format!(
                    "ENSIP-10 resolver {resolver_addr} reverted with non-OffchainLookup data: {e}"
                ))
            })?;
            let inner_bytes = crate::ccip_read::follow_offchain_lookup(transport, &lookup)?;
            decode_string_from_inner(&inner_bytes)
        }
        Err(e) => Err(e),
    }
}

/// Encode `IExtendedResolver.resolve(bytes name, bytes data)` —
/// selector `0x9061b923` (same selector the UniversalResolver uses,
/// because they share the same function signature).
fn encode_resolve_call(name: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 64 + name.len() + data.len() + 128);
    out.extend_from_slice(&[0x90, 0x61, 0xb9, 0x23]); // resolve(bytes,bytes)

    // Two heads.
    let name_padded = name.len().div_ceil(32) * 32;
    let data_offset: u64 = 0x40 + 32 + name_padded as u64;
    out.extend_from_slice(&u256_be(0x40));
    out.extend_from_slice(&u256_be(data_offset));

    // name tail.
    out.extend_from_slice(&u256_be(name.len() as u64));
    out.extend_from_slice(name);
    out.extend(std::iter::repeat_n(0u8, name_padded - name.len()));

    // data tail.
    out.extend_from_slice(&u256_be(data.len() as u64));
    out.extend_from_slice(data);
    let data_padded = data.len().div_ceil(32) * 32;
    out.extend(std::iter::repeat_n(0u8, data_padded - data.len()));

    out
}

/// DNS-encode an ENS name. Uses the same algorithm as
/// `crate::universal::dns_encode` but with this crate's error type so
/// the ENSIP-10 path can stay inside `ens_live`'s error surface.
fn dns_encode(name: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(name.len() + 2);
    if !name.is_empty() {
        for label in name.split('.') {
            if label.is_empty() {
                continue;
            }
            let bytes = label.as_bytes();
            if bytes.len() > 63 {
                return Err(format!(
                    "DNS label too long ({} bytes; max 63)",
                    bytes.len()
                ));
            }
            out.push(bytes.len() as u8);
            out.extend_from_slice(bytes);
        }
    }
    out.push(0);
    Ok(out)
}

/// Decode a top-level ABI `(bytes)` tuple. `eth_call` of `resolve()`
/// returns the dynamic `bytes` payload wrapped in this single-element
/// tuple shape: head word = offset (0x20), then length-prefixed body.
fn decode_outer_bytes_tuple(b: &[u8]) -> Result<Vec<u8>, RpcError> {
    if b.len() < 64 {
        return Err(RpcError::Decode(format!(
            "(bytes) tuple too short: {} bytes",
            b.len()
        )));
    }
    let offset = u256_to_usize(&b[..32])?;
    if b.len() < offset + 32 {
        return Err(RpcError::Decode(format!(
            "(bytes) length-word at offset {offset} OOB"
        )));
    }
    let len = u256_to_usize(&b[offset..offset + 32])?;
    let start = offset + 32;
    let end = start + len;
    if b.len() < end {
        return Err(RpcError::Decode(format!(
            "(bytes) content {start}..{end} OOB ({} bytes total)",
            b.len()
        )));
    }
    Ok(b[start..end].to_vec())
}

/// Decode a string from an ABI-encoded `(string)` tuple's *inner*
/// bytes — the shape `text(node, key)` returns. Mirror of
/// [`crate::ccip_read::decode_string_result`] but with this crate's
/// error type for symmetry with [`call_text`]'s [`decode_string`].
fn decode_string_from_inner(inner: &[u8]) -> Result<String, RpcError> {
    if inner.is_empty() {
        return Ok(String::new());
    }
    if inner.len() < 64 {
        return Err(RpcError::Decode(format!(
            "string tuple too short: {} bytes",
            inner.len()
        )));
    }
    let offset = u256_to_usize(&inner[..32])?;
    if inner.len() < offset + 32 {
        return Err(RpcError::Decode(format!(
            "string length-word at offset {offset} OOB"
        )));
    }
    let len = u256_to_usize(&inner[offset..offset + 32])?;
    let start = offset + 32;
    let end = start + len;
    if inner.len() < end {
        return Err(RpcError::Decode(format!(
            "string content {start}..{end} OOB ({} bytes total)",
            inner.len()
        )));
    }
    String::from_utf8(inner[start..end].to_vec())
        .map_err(|e| RpcError::Decode(format!("non-UTF-8 string record: {e}")))
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

    type HttpExpectation = (String, Result<Vec<u8>, RpcError>);

    /// In-test fake transport. Records calls and replays scripted
    /// responses. Not threadsafe — that's fine for unit tests.
    struct FakeTransport {
        scripted: RefCell<Vec<Expectation>>,
        calls: RefCell<Vec<(String, String)>>,
        http_scripted: RefCell<Vec<HttpExpectation>>,
        http_calls: RefCell<Vec<String>>,
    }

    impl FakeTransport {
        fn new() -> Self {
            Self {
                scripted: RefCell::new(Vec::new()),
                calls: RefCell::new(Vec::new()),
                http_scripted: RefCell::new(Vec::new()),
                http_calls: RefCell::new(Vec::new()),
            }
        }
        fn expect(&self, to: &str, data_prefix: &str, response: Result<String, RpcError>) {
            self.scripted
                .borrow_mut()
                .push((to.to_string(), data_prefix.to_string(), response));
        }
        fn expect_http(&self, url_prefix: &str, body: Result<Vec<u8>, RpcError>) {
            self.http_scripted
                .borrow_mut()
                .push((url_prefix.to_string(), body));
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

        fn http_get(&self, url: &str) -> Result<Vec<u8>, RpcError> {
            self.http_calls.borrow_mut().push(url.to_string());
            let mut script = self.http_scripted.borrow_mut();
            let pos = script
                .iter()
                .position(|(prefix, _)| url.starts_with(prefix));
            match pos {
                Some(i) => script.remove(i).1,
                None => Err(RpcError::Http(format!(
                    "FakeTransport: no http_get expectation for {url}"
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
                "sbo3l:proof_uri",
                "https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json",
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
        assert_eq!(recs.proof_uri, expected[4].1);
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

    /// Build a `(bytes)` ABI-tuple from a payload. Used by the
    /// CCIP-Read tests below to wrap inner-text bytes as the
    /// `eth_call` of `resolveCallback` would return them.
    fn abi_encode_outer_bytes_tuple(payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + payload.len() + 32);
        out.extend_from_slice(&u256_be(0x20));
        out.extend_from_slice(&u256_be(payload.len() as u64));
        out.extend_from_slice(payload);
        let pad = payload.len().div_ceil(32) * 32 - payload.len();
        out.extend(std::iter::repeat_n(0u8, pad));
        out
    }

    /// Build the inner-`(string)` payload returned by an ENSIP-10
    /// callback for a `text(node, key)` query.
    fn abi_encode_inner_string(s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + s.len() + 32);
        out.extend_from_slice(&u256_be(0x20));
        out.extend_from_slice(&u256_be(s.len() as u64));
        out.extend_from_slice(s.as_bytes());
        let pad = s.len().div_ceil(32) * 32 - s.len();
        out.extend(std::iter::repeat_n(0u8, pad));
        out
    }

    /// Build the gateway-side `(value, expires, signature)` triple
    /// in the shape the OffchainResolver's `resolveCallback` accepts.
    /// The test path doesn't verify the signature on-chain, but the
    /// ABI shape must be byte-correct so our `encode_callback_call`
    /// + `decode_outer_bytes_tuple` round-trip lines up.
    fn abi_encode_signed_response(value: &[u8], expires: u64, signature: &[u8]) -> Vec<u8> {
        // `(bytes value, uint64 expires, bytes signature)`:
        // head: 3 words = 96 bytes.
        //   word 0: offset to value = 0x60
        //   word 1: expires (right-aligned in 32-byte slot)
        //   word 2: offset to signature = 0x60 + 32 + padded(value)
        let value_padded = value.len().div_ceil(32) * 32;
        let sig_padded = signature.len().div_ceil(32) * 32;
        let mut out = Vec::with_capacity(96 + 32 + value_padded + 32 + sig_padded);
        out.extend_from_slice(&u256_be(0x60));
        out.extend_from_slice(&u256_be(expires));
        out.extend_from_slice(&u256_be(0x60 + 32 + value_padded as u64));
        // value tail.
        out.extend_from_slice(&u256_be(value.len() as u64));
        out.extend_from_slice(value);
        out.extend(std::iter::repeat_n(0u8, value_padded - value.len()));
        // signature tail.
        out.extend_from_slice(&u256_be(signature.len() as u64));
        out.extend_from_slice(signature);
        out.extend(std::iter::repeat_n(0u8, sig_padded - signature.len()));
        out
    }

    /// Build an `OffchainLookup(address, string[], bytes, bytes4,
    /// bytes)` revert payload: selector || ABI args.
    fn abi_encode_offchain_lookup_revert(
        sender: &[u8; 20],
        urls: &[&str],
        call_data: &[u8],
        callback_selector: &[u8; 4],
        extra_data: &[u8],
    ) -> Vec<u8> {
        // 5 head words = 160 bytes:
        //   word 0: address sender (20 right-padded in 32)
        //   word 1: offset to urls (string[])
        //   word 2: offset to callData (bytes)
        //   word 3: bytes4 callbackFunction (left-aligned)
        //   word 4: offset to extraData (bytes)
        let mut head = Vec::with_capacity(160);
        let mut sender_word = [0u8; 32];
        sender_word[12..].copy_from_slice(sender);
        head.extend_from_slice(&sender_word);
        // Offsets fixed up below.
        head.extend_from_slice(&[0u8; 32]); // urls offset
        head.extend_from_slice(&[0u8; 32]); // calldata offset
        let mut cb_word = [0u8; 32];
        cb_word[..4].copy_from_slice(callback_selector);
        head.extend_from_slice(&cb_word);
        head.extend_from_slice(&[0u8; 32]); // extraData offset

        // Tails — order: urls, callData, extraData.
        // urls tail: length || N head offsets || padded element bodies.
        let mut urls_tail = Vec::new();
        urls_tail.extend_from_slice(&u256_be(urls.len() as u64));
        let heads_len = urls.len() * 32;
        let mut cursor = heads_len as u64;
        let mut url_bodies: Vec<Vec<u8>> = Vec::new();
        for u in urls {
            let bytes = u.as_bytes();
            let padded = bytes.len().div_ceil(32) * 32;
            let mut body = Vec::with_capacity(32 + padded);
            body.extend_from_slice(&u256_be(bytes.len() as u64));
            body.extend_from_slice(bytes);
            body.extend(std::iter::repeat_n(0u8, padded - bytes.len()));
            url_bodies.push(body);
        }
        for body in &url_bodies {
            urls_tail.extend_from_slice(&u256_be(cursor));
            cursor += body.len() as u64;
        }
        for body in url_bodies {
            urls_tail.extend_from_slice(&body);
        }

        let mut calldata_tail = Vec::new();
        calldata_tail.extend_from_slice(&u256_be(call_data.len() as u64));
        calldata_tail.extend_from_slice(call_data);
        let cd_pad = call_data.len().div_ceil(32) * 32 - call_data.len();
        calldata_tail.extend(std::iter::repeat_n(0u8, cd_pad));

        let mut extra_tail = Vec::new();
        extra_tail.extend_from_slice(&u256_be(extra_data.len() as u64));
        extra_tail.extend_from_slice(extra_data);
        let ed_pad = extra_data.len().div_ceil(32) * 32 - extra_data.len();
        extra_tail.extend(std::iter::repeat_n(0u8, ed_pad));

        // Fix offsets.
        let urls_offset: u64 = 160; // start of tails section
        let calldata_offset: u64 = urls_offset + urls_tail.len() as u64;
        let extra_offset: u64 = calldata_offset + calldata_tail.len() as u64;
        head[32..64].copy_from_slice(&u256_be(urls_offset));
        head[64..96].copy_from_slice(&u256_be(calldata_offset));
        head[128..160].copy_from_slice(&u256_be(extra_offset));

        let mut out = Vec::with_capacity(
            4 + head.len() + urls_tail.len() + calldata_tail.len() + extra_tail.len(),
        );
        out.extend_from_slice(&crate::ccip_read::OFFCHAIN_LOOKUP_SELECTOR);
        out.extend_from_slice(&head);
        out.extend_from_slice(&urls_tail);
        out.extend_from_slice(&calldata_tail);
        out.extend_from_slice(&extra_tail);
        out
    }

    /// End-to-end CCIP-Read follow on the `text(node, key)` path, the
    /// load-bearing scenario for `sbo3l agent verify-ens
    /// research-agent.sbo3lagent.eth` in the loop-7 UAT bug. Verifies:
    /// 1. supportsInterface(0x9061b923) → true → ENSIP-10 dispatch
    /// 2. resolve(dnsEncode(name), text(node,key)) reverts with
    ///    OffchainLookup
    /// 3. gateway HTTP GET decodes; callback eth_call returns the
    ///    record value
    #[test]
    fn resolve_raw_text_follows_offchain_lookup_via_ensip10() {
        let transport = FakeTransport::new();
        let resolver_addr = "0x87e99508c222c6e419734cacbb6781b8d282b1f6";
        let resolver_addr_bytes = {
            let mut a = [0u8; 20];
            hex::decode_to_slice("87e99508c222c6e419734cacbb6781b8d282b1f6", &mut a).unwrap();
            a
        };

        // Step 1: registry.resolver(node) → 32-byte left-padded address.
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(
                "87e99508c222c6e419734cacbb6781b8d282b1f6",
            )),
        );

        // Step 2: supportsInterface(0x9061b923) → true.
        let true_word = format!("0x{}{}", "00".repeat(31), "01");
        transport.expect(resolver_addr, "0x01ffc9a7", Ok(true_word));

        // Step 3: resolve(name, data) reverts with OffchainLookup.
        let urls = vec!["https://gateway.test/api/{sender}/{data}.json"];
        let inner_call_data = encode_text_call(&[0u8; 32], "sbo3l:agent_id");
        let callback_selector = [0xb4, 0xa8, 0x5b, 0x71]; // resolveCallback selector (arbitrary for the test)
        let extra_data = inner_call_data.clone();
        let revert_bytes = abi_encode_offchain_lookup_revert(
            &resolver_addr_bytes,
            &urls,
            &inner_call_data,
            &callback_selector,
            &extra_data,
        );
        transport.expect(
            resolver_addr,
            "0x9061b923",
            Err(RpcError::Reverted { data: revert_bytes }),
        );

        // Step 4: gateway HTTP GET → signed response.
        let inner_string_payload = abi_encode_inner_string("research-agent-01");
        let gateway_data = abi_encode_signed_response(&inner_string_payload, u64::MAX, &[0u8; 65]);
        let gateway_body = serde_json::json!({
            "data": format!("0x{}", hex::encode(&gateway_data)),
            "ttl": 60u64,
        })
        .to_string()
        .into_bytes();
        transport.expect_http("https://gateway.test/api/", Ok(gateway_body));

        // Step 5: callback eth_call → outer (bytes) tuple wrapping
        //         the inner (string) tuple.
        let outer = abi_encode_outer_bytes_tuple(&inner_string_payload);
        transport.expect(
            resolver_addr,
            &format!("0x{}", hex::encode(callback_selector)),
            Ok(format!("0x{}", hex::encode(outer))),
        );

        // The leaf name doesn't have to namehash to anything specific
        // for the test; the fake matches by data prefix not by node.
        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Sepolia);
        let v = resolver
            .resolve_raw_text("research-agent.sbo3lagent.eth", "sbo3l:agent_id")
            .unwrap();
        assert_eq!(v.as_deref(), Some("research-agent-01"));
    }

    /// Regression: the legacy direct-`text()` path must still work
    /// for resolvers that don't advertise ENSIP-10 (e.g. the ENS
    /// PublicResolver on mainnet `sbo3lagent.eth`).
    #[test]
    fn resolve_raw_text_legacy_text_path_when_resolver_not_ensip10() {
        let transport = FakeTransport::new();
        let resolver_addr = "0xed79b9b96c6f44ee7b8e1ad1c2519bba2cdcc7d3";

        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(
                "ed79b9b96c6f44ee7b8e1ad1c2519bba2cdcc7d3",
            )),
        );
        // supportsInterface returns false (last byte 0).
        let false_word = format!("0x{}", "00".repeat(32));
        transport.expect(resolver_addr, "0x01ffc9a7", Ok(false_word));
        transport.expect(
            resolver_addr,
            "0x59d1d43c",
            Ok(abi_encode_string_response("research-agent-01")),
        );

        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Mainnet);
        let v = resolver
            .resolve_raw_text("sbo3lagent.eth", "sbo3l:agent_id")
            .unwrap();
        assert_eq!(v.as_deref(), Some("research-agent-01"));
    }

    /// `find_resolver` walks parent names when the leaf has no
    /// resolver entry — ENSIP-10 wildcard pattern.
    #[test]
    fn find_resolver_walks_parents_when_leaf_has_no_entry() {
        let transport = FakeTransport::new();
        let parent_resolver = "0x87e99508c222c6e419734cacbb6781b8d282b1f6";

        // Leaf returns 0x0 (no direct entry).
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(&"00".repeat(20))),
        );
        // Parent returns the OR.
        transport.expect(
            ENS_REGISTRY_ADDRESS,
            "0x0178b8bf",
            Ok(abi_encode_address_response(
                "87e99508c222c6e419734cacbb6781b8d282b1f6",
            )),
        );

        let resolver = LiveEnsResolver::new(transport, EnsNetwork::Sepolia);
        let (addr, _node) = resolver
            .find_resolver("research-agent.sbo3lagent.eth")
            .unwrap();
        assert_eq!(addr.to_ascii_lowercase(), parent_resolver);
    }
}
