//! B7 — Uniswap V3 QuoterV2 live quote on Sepolia.
//!
//! Calls `quoteExactInputSingle((address,address,uint256,uint24,uint160))`
//! on the QuoterV2 contract via JSON-RPC `eth_call`. The function
//! uses the revert-and-decode trick internally, but its OUTER
//! signature returns the four uint values directly via the regular
//! return path, so an `eth_call` simulation produces normal return
//! data (the simulation includes the revert/catch dance internally).
//!
//! The integration is structured around a [`JsonRpcTransport`]
//! trait so the production HTTP path and offline tests share one
//! code path. CI uses an in-process [`FakeTransport`] (private to
//! tests); production uses [`ReqwestTransport`] backed by
//! `reqwest::blocking`.
//!
//! Truthfulness rules:
//!
//! - The selector is **derived in tests** from the canonical type
//!   string — hardcoding without that pin is the kind of drift that
//!   silently breaks live integration. The test would fail loudly if
//!   the constant ever drifted from `keccak256(...)`.
//! - The Sepolia QuoterV2 address (`0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3`)
//!   is documented at developers.uniswap.org/contracts/v3/reference
//!   /deployments/ethereum-deployments. Hardcoding well-known public
//!   infrastructure is fine; the agent's parameters (token addresses,
//!   amount, fee tier) come from operator-supplied env / context.

use std::time::Duration;

use serde::{Deserialize, Serialize};
#[cfg(test)]
use tiny_keccak::{Hasher, Keccak};

/// Sepolia QuoterV2 deployment address. Source:
/// developers.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments
pub const SEPOLIA_QUOTER_V2_ADDRESS: &str = "0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3";

/// EIP-155 Sepolia chain id. Surfaced into the evidence so an
/// auditor can confirm the quote came from Sepolia, not mainnet.
pub const SEPOLIA_CHAIN_ID: u64 = 11_155_111;

/// Sepolia WETH9 token address. Surfaced as the safe default
/// `token_in` when the operator provides no override.
pub const SEPOLIA_WETH: &str = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14";

/// Mainnet QuoterV2 deployment address. Source:
/// developers.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments
///
/// Used by `sbo3l uniswap swap --network mainnet` (Task D) to
/// price-quote `--amount-in` against the live mainnet pool before
/// computing `amountOutMinimum` from the slippage cap. Mainnet calls
/// are gated behind `SBO3L_ALLOW_MAINNET_TX=1` at the CLI layer.
pub const MAINNET_QUOTER_V2_ADDRESS: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";

/// EIP-155 mainnet chain id (1).
pub const MAINNET_CHAIN_ID: u64 = 1;

/// Mainnet WETH9 token address. Canonical wrapped-ETH; same address
/// every Ethereum tooling has used since 2017.
pub const MAINNET_WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

/// Selector for `quoteExactInputSingle((address,address,uint256,uint24,uint160))`.
/// Pinned in tests against `keccak256(canonical_type_string)[0..4]`.
pub const QUOTE_EXACT_INPUT_SINGLE_SELECTOR: [u8; 4] = [0xc6, 0xa5, 0x02, 0x6a];

/// Reasons a JSON-RPC call can fail. Surfaces back to the executor
/// caller through `ExecutionError::BackendOffline` (transport / IO)
/// or `ExecutionError::Integration` (decode / server-rejected).
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("RPC HTTP error: {0}")]
    Http(String),
    #[error("RPC reported error: code={code} message={message}")]
    Server { code: i64, message: String },
    #[error("malformed eth_call response: {0}")]
    Decode(String),
    #[error("RPC parse error: {0}")]
    Parse(String),
}

/// Synchronous JSON-RPC transport. Production uses
/// [`ReqwestTransport`]; tests inject a fake.
pub trait JsonRpcTransport: Send + Sync {
    /// Call `eth_call` against `to` with `data` (both `0x`-prefixed
    /// hex). Returns the `0x`-prefixed hex result string.
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
pub struct ReqwestTransport {
    rpc_url: String,
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new(rpc_url: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest::blocking::Client builder cannot fail with default config");
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

/// Operator-supplied Uniswap config. `token_in`, `token_out` and
/// `fee_tier` are required for any live quote; the rest carry
/// sensible Sepolia defaults via [`Self::sepolia_default`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LiveConfig {
    /// JSON-RPC endpoint URL. Stored only so the evidence can record
    /// `quote_source` carrying the network + quoter; the actual HTTP
    /// transport ([`JsonRpcTransport`]) holds its own copy.
    pub rpc_url: String,
    pub quoter: String,
    pub chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub fee_tier: u32,
    /// `amountIn` in token base units (wei for 18-decimal tokens).
    /// Decimal string for serde stability.
    pub amount_in_wei: String,
}

impl LiveConfig {
    /// Sepolia config with sane defaults. Caller fills in
    /// `token_out` (and `token_in` if not WETH).
    pub fn sepolia_default(
        token_in: String,
        token_out: String,
        fee_tier: u32,
        amount_in_wei: String,
        rpc_url: String,
    ) -> Self {
        Self {
            rpc_url,
            quoter: SEPOLIA_QUOTER_V2_ADDRESS.to_string(),
            chain_id: SEPOLIA_CHAIN_ID,
            token_in,
            token_out,
            fee_tier,
            amount_in_wei,
        }
    }

    /// Mainnet config with sane defaults. Pair to
    /// [`Self::sepolia_default`] for the `sbo3l uniswap swap
    /// --network mainnet` flow. Caller supplies `token_in` /
    /// `token_out` / `amount_in_wei` / `rpc_url`; `quoter` and
    /// `chain_id` are pinned to the canonical mainnet QuoterV2 +
    /// chain id.
    pub fn mainnet_default(
        token_in: String,
        token_out: String,
        fee_tier: u32,
        amount_in_wei: String,
        rpc_url: String,
    ) -> Self {
        Self {
            rpc_url,
            quoter: MAINNET_QUOTER_V2_ADDRESS.to_string(),
            chain_id: MAINNET_CHAIN_ID,
            token_in,
            token_out,
            fee_tier,
            amount_in_wei,
        }
    }
}

/// Decoded `quoteExactInputSingle` return tuple. All values
/// rendered as decimal strings so the evidence struct stays JSON-
/// safe regardless of decimal precision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuoteResult {
    pub amount_out: String,
    pub sqrt_price_x96_after: String,
    pub initialized_ticks_crossed: u32,
    pub gas_estimate: String,
}

/// Build the calldata for `quoteExactInputSingle`. Public so
/// callers can pre-build, log, or attach it to the evidence.
pub fn encode_quote_call(config: &LiveConfig) -> Result<Vec<u8>, RpcError> {
    let mut out = Vec::with_capacity(4 + 5 * 32);
    out.extend_from_slice(&QUOTE_EXACT_INPUT_SINGLE_SELECTOR);
    // Static struct: encoded inline, 5 * 32 = 160 bytes.
    out.extend_from_slice(&address_padded(&config.token_in)?);
    out.extend_from_slice(&address_padded(&config.token_out)?);
    out.extend_from_slice(&dec_uint256_be(&config.amount_in_wei)?);
    out.extend_from_slice(&u32_to_uint256_be(config.fee_tier));
    // sqrtPriceLimitX96 = 0 (no limit).
    out.extend_from_slice(&[0u8; 32]);
    Ok(out)
}

pub fn quote_exact_input_single<T: JsonRpcTransport + ?Sized>(
    transport: &T,
    config: &LiveConfig,
) -> Result<QuoteResult, RpcError> {
    let data = encode_quote_call(config)?;
    let hex_data = format!("0x{}", hex::encode(&data));
    let raw = transport.eth_call(&config.quoter, &hex_data)?;
    decode_quote_response(&raw)
}

fn decode_quote_response(hex_response: &str) -> Result<QuoteResult, RpcError> {
    let body = strip_0x(hex_response)?;
    let bytes = hex::decode(body).map_err(|e| RpcError::Decode(format!("hex decode: {e}")))?;
    if bytes.len() < 4 * 32 {
        return Err(RpcError::Decode(format!(
            "quote response too short: {} bytes (need 128)",
            bytes.len()
        )));
    }
    let amount_out = uint256_be_to_dec(&bytes[0..32])?;
    let sqrt_price_x96_after = uint256_be_to_dec(&bytes[32..64])?;
    let initialized_ticks_crossed = uint256_be_to_u32(&bytes[64..96])?;
    let gas_estimate = uint256_be_to_dec(&bytes[96..128])?;
    Ok(QuoteResult {
        amount_out,
        sqrt_price_x96_after,
        initialized_ticks_crossed,
        gas_estimate,
    })
}

fn strip_0x(s: &str) -> Result<&str, RpcError> {
    s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .ok_or_else(|| RpcError::Decode(format!("response missing `0x` prefix: {s:?}")))
}

fn address_padded(addr: &str) -> Result<[u8; 32], RpcError> {
    let stripped = strip_0x(addr)?;
    if stripped.len() != 40 {
        return Err(RpcError::Decode(format!(
            "address must be 20 bytes (40 hex chars): {addr}"
        )));
    }
    let bytes =
        hex::decode(stripped).map_err(|e| RpcError::Decode(format!("address hex decode: {e}")))?;
    let mut padded = [0u8; 32];
    padded[12..].copy_from_slice(&bytes);
    Ok(padded)
}

fn u32_to_uint256_be(n: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[28..].copy_from_slice(&n.to_be_bytes());
    out
}

/// Parse a u256 decimal string (e.g. "1000000000000000000") into
/// a 32-byte big-endian buffer. Rejects values larger than 2^256
/// implicitly (won't fit) and any non-decimal-digit input.
fn dec_uint256_be(decimal: &str) -> Result<[u8; 32], RpcError> {
    if decimal.is_empty() {
        return Err(RpcError::Decode("amount_in_wei is empty".into()));
    }
    if !decimal.bytes().all(|c| c.is_ascii_digit()) {
        return Err(RpcError::Decode(format!(
            "amount_in_wei must be decimal digits: {decimal}"
        )));
    }
    // Left-fold base-10. Caps at 32 bytes; anything overflowing is
    // surfaced as Decode.
    let mut buf = [0u8; 32];
    for c in decimal.bytes() {
        let d = c - b'0';
        let mut carry = d as u16;
        for byte in buf.iter_mut().rev() {
            let prod = (*byte as u16) * 10 + carry;
            *byte = (prod & 0xff) as u8;
            carry = prod >> 8;
        }
        if carry != 0 {
            return Err(RpcError::Decode(format!(
                "amount_in_wei overflows uint256: {decimal}"
            )));
        }
    }
    Ok(buf)
}

fn uint256_be_to_dec(b: &[u8]) -> Result<String, RpcError> {
    if b.len() != 32 {
        return Err(RpcError::Decode(format!(
            "uint256 slice not 32 bytes: {}",
            b.len()
        )));
    }
    if b.iter().all(|&x| x == 0) {
        return Ok("0".to_string());
    }
    // Repeated divide-by-10. Up to ~78 digits. O(n^2) is fine here.
    let mut buf = b.to_vec();
    let mut digits = Vec::with_capacity(80);
    while !buf.iter().all(|&x| x == 0) {
        let mut rem: u32 = 0;
        for byte in buf.iter_mut() {
            let v = (rem << 8) | *byte as u32;
            *byte = (v / 10) as u8;
            rem = v % 10;
        }
        digits.push((rem as u8 + b'0') as char);
    }
    digits.reverse();
    Ok(digits.into_iter().collect())
}

fn uint256_be_to_u32(b: &[u8]) -> Result<u32, RpcError> {
    if b.len() != 32 {
        return Err(RpcError::Decode("uint256 slice not 32 bytes".into()));
    }
    if !b[..28].iter().all(|&x| x == 0) {
        return Err(RpcError::Decode("uint256 doesn't fit in u32".into()));
    }
    let mut be = [0u8; 4];
    be.copy_from_slice(&b[28..32]);
    Ok(u32::from_be_bytes(be))
}

#[cfg(test)]
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Test-only helpers re-exposed so the cross-module live-mode
/// integration tests in `uniswap.rs` can reuse the same fake
/// transport + ABI helper rather than duplicating them. Compiled
/// only in `#[cfg(test)]`.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use std::sync::Mutex;

    type Expectation = (String, String, Result<String, RpcError>);

    pub struct FakeTransport {
        scripted: Mutex<Vec<Expectation>>,
        calls: Mutex<Vec<(String, String)>>,
    }

    impl FakeTransport {
        pub fn new() -> Self {
            Self {
                scripted: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
            }
        }
        pub fn expect(&self, to: &str, data_prefix: &str, response: Result<String, RpcError>) {
            self.scripted
                .lock()
                .unwrap()
                .push((to.to_string(), data_prefix.to_string(), response));
        }
        pub fn calls(&self) -> Vec<(String, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl JsonRpcTransport for FakeTransport {
        fn eth_call(&self, to: &str, data: &str) -> Result<String, RpcError> {
            self.calls
                .lock()
                .unwrap()
                .push((to.to_string(), data.to_string()));
            let mut script = self.scripted.lock().unwrap();
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

    pub fn abi_encode_quad(amount_out: &str, sqrt_price: &str, ticks: u32, gas: &str) -> String {
        let mut out = Vec::with_capacity(128);
        out.extend_from_slice(&dec_uint256_be(amount_out).unwrap());
        out.extend_from_slice(&dec_uint256_be(sqrt_price).unwrap());
        out.extend_from_slice(&u32_to_uint256_be(ticks));
        out.extend_from_slice(&dec_uint256_be(gas).unwrap());
        format!("0x{}", hex::encode(out))
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::{abi_encode_quad, FakeTransport};
    use super::*;

    /// Pinned: derive selector from the canonical type string, assert
    /// it matches `QUOTE_EXACT_INPUT_SINGLE_SELECTOR`. This is the
    /// load-bearing test — drift here means live mode talks to the
    /// wrong function and silently returns garbage.
    #[test]
    fn quote_selector_matches_canonical_signature() {
        let derived = keccak256(b"quoteExactInputSingle((address,address,uint256,uint24,uint160))");
        assert_eq!(
            derived[..4],
            QUOTE_EXACT_INPUT_SINGLE_SELECTOR,
            "QUOTE_EXACT_INPUT_SINGLE_SELECTOR drifted; expected {:?}, got {:?}",
            &derived[..4],
            QUOTE_EXACT_INPUT_SINGLE_SELECTOR
        );
    }

    #[test]
    fn dec_uint256_round_trips_through_be() {
        for &n in &[0u128, 1, 42, 1_000_000_000_000_000_000, u128::MAX] {
            let s = n.to_string();
            let be = dec_uint256_be(&s).unwrap();
            let back = uint256_be_to_dec(&be).unwrap();
            assert_eq!(back, s, "round-trip failed for {n}");
        }
    }

    #[test]
    fn dec_uint256_rejects_non_digits() {
        assert!(matches!(dec_uint256_be("0x123"), Err(RpcError::Decode(_))));
        assert!(matches!(dec_uint256_be("123abc"), Err(RpcError::Decode(_))));
        assert!(matches!(dec_uint256_be(""), Err(RpcError::Decode(_))));
    }

    #[test]
    fn address_padded_left_zero_pads_20_bytes() {
        let p = address_padded("0x1111111111111111111111111111111111111111").unwrap();
        // First 12 bytes zero, last 20 bytes the address.
        assert!(p[..12].iter().all(|&b| b == 0));
        assert_eq!(&p[12..], &[0x11u8; 20]);
    }

    #[test]
    fn address_padded_rejects_short() {
        assert!(matches!(address_padded("0x1234"), Err(RpcError::Decode(_))));
    }

    #[test]
    fn encode_quote_call_layout_pinned() {
        let cfg = LiveConfig {
            rpc_url: "http://x.invalid".into(),
            quoter: SEPOLIA_QUOTER_V2_ADDRESS.into(),
            chain_id: SEPOLIA_CHAIN_ID,
            token_in: SEPOLIA_WETH.into(),
            token_out: "0x0000000000000000000000000000000000000022".into(),
            fee_tier: 3000,
            amount_in_wei: "1000000000000000000".into(),
        };
        let cd = encode_quote_call(&cfg).unwrap();
        // selector(4) + 5 * uint256(32) = 164.
        assert_eq!(cd.len(), 164);
        assert_eq!(cd[..4], QUOTE_EXACT_INPUT_SINGLE_SELECTOR);
        // Last 32 bytes = sqrtPriceLimitX96 = 0.
        assert!(cd[132..].iter().all(|&b| b == 0));
        // Fee tier (3000 = 0x0BB8) at the right offset (4 + 32*3).
        let fee_word = &cd[4 + 32 * 3..4 + 32 * 4];
        assert_eq!(u32::from_be_bytes(fee_word[28..].try_into().unwrap()), 3000);
    }

    fn cfg() -> LiveConfig {
        LiveConfig::sepolia_default(
            SEPOLIA_WETH.to_string(),
            "0x0000000000000000000000000000000000000022".to_string(),
            3000,
            "1000000000000000000".to_string(),
            "http://example.invalid".to_string(),
        )
    }

    #[test]
    fn happy_path_decodes_quad_return() {
        let t = FakeTransport::new();
        t.expect(
            SEPOLIA_QUOTER_V2_ADDRESS,
            "0xc6a5026a",
            Ok(abi_encode_quad(
                "2500000000000000000",
                "1234567890123456789",
                7,
                "85000",
            )),
        );
        let q = quote_exact_input_single(&t, &cfg()).unwrap();
        assert_eq!(q.amount_out, "2500000000000000000");
        assert_eq!(q.sqrt_price_x96_after, "1234567890123456789");
        assert_eq!(q.initialized_ticks_crossed, 7);
        assert_eq!(q.gas_estimate, "85000");
        // Verify the call hit the right contract.
        let calls = t.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.eq_ignore_ascii_case(SEPOLIA_QUOTER_V2_ADDRESS));
        assert!(calls[0].1.starts_with("0xc6a5026a"));
    }

    #[test]
    fn server_error_surfaces_as_rpc_error_server() {
        let t = FakeTransport::new();
        t.expect(
            SEPOLIA_QUOTER_V2_ADDRESS,
            "0xc6a5026a",
            Err(RpcError::Server {
                code: 3,
                message: "execution reverted".into(),
            }),
        );
        let err = quote_exact_input_single(&t, &cfg()).unwrap_err();
        match err {
            RpcError::Server { code, message } => {
                assert_eq!(code, 3);
                assert!(message.contains("execution reverted"));
            }
            other => panic!("expected Server, got {other:?}"),
        }
    }

    #[test]
    fn http_timeout_surfaces_as_rpc_error_http() {
        let t = FakeTransport::new();
        t.expect(
            SEPOLIA_QUOTER_V2_ADDRESS,
            "0xc6a5026a",
            Err(RpcError::Http("timeout".into())),
        );
        let err = quote_exact_input_single(&t, &cfg()).unwrap_err();
        assert!(matches!(err, RpcError::Http(_)));
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn malformed_response_surfaces_as_decode_error() {
        let t = FakeTransport::new();
        // 64 bytes — only 2 of the 4 expected uint256s.
        let short = format!("0x{}", "00".repeat(64));
        t.expect(SEPOLIA_QUOTER_V2_ADDRESS, "0xc6a5026a", Ok(short));
        let err = quote_exact_input_single(&t, &cfg()).unwrap_err();
        assert!(matches!(err, RpcError::Decode(_)));
    }

    #[test]
    fn missing_0x_prefix_surfaces_as_decode_error() {
        let t = FakeTransport::new();
        let response = "no_prefix_here".to_string();
        t.expect(SEPOLIA_QUOTER_V2_ADDRESS, "0xc6a5026a", Ok(response));
        let err = quote_exact_input_single(&t, &cfg()).unwrap_err();
        assert!(matches!(err, RpcError::Decode(_)));
    }
}
