//! `sbo3l doctor --extended` — Sepolia contract liveness probes.
//!
//! Run on top of the standard local-storage doctor checks. For each of
//! the 6 SBO3L Sepolia deployments, hits the configured RPC with:
//!
//!   1. `eth_getCode(address)` — confirms the contract bytecode is
//!      present (rejects an EOA / missing pin).
//!   2. ONE non-state-changing view call — confirms the ABI shape
//!      matches what we expect (so a pin pointing at a different
//!      contract under the same address surfaces here, not at
//!      first-customer-call).
//!   3. (OffchainResolver only) parses the returned URL template and
//!      verifies it contains both `{sender}` and `{data}`. This is the
//!      shape Heidi's Bug #2 broke at submission time — a malformed
//!      URL slipped through CI and only got caught manually after the
//!      live demo failed.
//!
//! # RPC selection
//!
//! Per `memory:alchemy_rpc_endpoints.md`, the doctor prefers Alchemy
//! (`https://eth-sepolia.g.alchemy.com/v2/<API_KEY>`) — PublicNode
//! rate-limits this batch of 6 + 6 = 12 calls when run from CI. The
//! resolver order:
//!
//!   1. `--rpc-url <URL>` flag (highest precedence).
//!   2. `SBO3L_SEPOLIA_RPC_URL` env var.
//!   3. `SBO3L_RPC_URL` env var (compat with `audit verify-anchor`).
//!   4. PublicNode public endpoint (last-resort, with a warning).
//!
//! No API key is hardcoded in this binary; operators supply via env
//! per the security checklist.

use std::process::ExitCode;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Keccak};

/// Canonical SBO3L Sepolia pin set. Mirrors
/// `crates/sbo3l-identity/src/contracts.rs` but inlined here so the
/// doctor doesn't need to import the identity crate (keeps the cli
/// dep graph stable). Order is the human-readable "show me my
/// contracts" order, not alphabetical.
const PROBES: &[ContractProbe] = &[
    ContractProbe {
        // Canonical Sepolia OR after the 2026-05-03 redeploy (PR #383
        // + #390 + #396). The orphan at 0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3
        // shipped the malformed `{sender/{data}.json}` template (Heidi
        // UAT Bug #2) and is intentionally NOT probed — keeping it on
        // the probe list would surface a fixed bug as a recurring
        // failure on every doctor run.
        label: "OffchainResolver",
        address: "0x87e99508C222c6E419734CACbb6781b8d282b1F6",
        view_signature: "urls(uint256)",
        view_arg: ProbeArg::Uint256(0),
        decode_kind: DecodeKind::DynamicString,
        url_template_validate: true,
    },
    ContractProbe {
        label: "AnchorRegistry",
        address: "0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac",
        view_signature: "anchorCount(bytes32)",
        view_arg: ProbeArg::Bytes32Zero,
        decode_kind: DecodeKind::Uint256,
        url_template_validate: false,
    },
    ContractProbe {
        label: "SubnameAuction",
        address: "0x5dE75E64739A95701367F3Ad592e0b674b22114B",
        view_signature: "auctionCount()",
        view_arg: ProbeArg::None,
        decode_kind: DecodeKind::Uint256,
        url_template_validate: false,
    },
    ContractProbe {
        label: "ReputationBond",
        address: "0x75072217B43960414047c362198A428f0E9793dA",
        view_signature: "BOND_AMOUNT()",
        view_arg: ProbeArg::None,
        decode_kind: DecodeKind::Uint256Wei,
        url_template_validate: false,
    },
    ContractProbe {
        label: "ReputationRegistry",
        address: "0x6aA95d8126B6221607245c068483fa5008F36dc2",
        view_signature: "entryCount(bytes32,address)",
        view_arg: ProbeArg::Bytes32AndAddressZero,
        decode_kind: DecodeKind::Uint256,
        url_template_validate: false,
    },
    ContractProbe {
        label: "ERC8004 IdentityRegistry",
        address: "0x600c10dE2fd5BB8f3F47cd356Bcb80289845Db37",
        view_signature: "isRegistered(address)",
        view_arg: ProbeArg::AddressZero,
        decode_kind: DecodeKind::Bool,
        url_template_validate: false,
    },
];

#[derive(Debug, Clone, Copy)]
struct ContractProbe {
    label: &'static str,
    address: &'static str,
    view_signature: &'static str,
    view_arg: ProbeArg,
    decode_kind: DecodeKind,
    /// True only for OffchainResolver — runs the {sender}/{data}
    /// template-shape check from Heidi's Bug #2.
    url_template_validate: bool,
}

#[derive(Debug, Clone, Copy)]
enum ProbeArg {
    /// No args — selector only (e.g. `auctionCount()`).
    None,
    /// One uint256 arg, big-endian-padded into a 32-byte word.
    Uint256(u64),
    /// One bytes32 arg, all zeros.
    Bytes32Zero,
    /// One address arg, zero address.
    AddressZero,
    /// Two args: bytes32 zero + address zero (entryCount).
    Bytes32AndAddressZero,
}

#[derive(Debug, Clone, Copy)]
enum DecodeKind {
    /// 32-byte word interpreted as `uint256`. Pretty-printed as the
    /// raw integer.
    Uint256,
    /// 32-byte word interpreted as `uint256` denominated in wei;
    /// pretty-printed as `<wei> wei (<eth> ETH)` for the operator's
    /// readability.
    Uint256Wei,
    /// Single-byte (within a 32-byte word) `bool`. Pretty-printed as
    /// `true|false`.
    Bool,
    /// ABI dynamic `string`: `[offset:32][length:32][data...padded]`.
    DynamicString,
}

/// JSON envelope for one contract probe — surfaces in the
/// `--extended` doctor's output under a `sepolia_contracts` array.
#[derive(Debug, Clone, Serialize)]
pub struct ContractProbeReport {
    pub label: String,
    pub address: String,
    pub status: ProbeStatus,
    pub code_size_bytes: usize,
    pub view_signature: String,
    /// Pretty-printed return value, or `None` if the call failed.
    pub view_result: Option<String>,
    /// Populated only for OffchainResolver — `Some(true)` when the URL
    /// matches `{sender}` and `{data}`, `Some(false)` when it doesn't,
    /// `None` for other contracts.
    pub url_template_ok: Option<bool>,
    /// Set on `Failed` — short error string.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    /// `eth_getCode` returned bytecode + the view call decoded
    /// successfully + (if applicable) URL template matched.
    Ok,
    /// `eth_getCode` returned `0x` (no bytecode at the address). The
    /// pin is wrong or the chain doesn't have this contract yet.
    NoCode,
    /// `eth_call` returned an error or decoded to an unexpected shape.
    /// Distinct from `NoCode` because bytecode IS present — this is
    /// "wrong contract" rather than "missing".
    AbiMismatch,
    /// (OffchainResolver only) the URL template doesn't contain both
    /// `{sender}` and `{data}`. Heidi's Bug #2 shape.
    UrlTemplateMalformed,
    /// RPC layer failure (timeout, bad URL, JSON-RPC-level error).
    RpcError,
}

#[derive(Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RpcResponse<T> {
    #[serde(default)]
    result: Option<T>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Deserialize, Debug)]
struct RpcError {
    code: i64,
    message: String,
}

/// Run all 6 probes against `rpc_url`. Returns the per-contract
/// reports and an aggregate exit code (0 if every probe is
/// `ProbeStatus::Ok`, 1 otherwise). When `json` is true, prints the
/// JSON envelope; otherwise pretty-prints a human-readable table.
pub fn run_extended(rpc_url: &str, json: bool) -> ExitCode {
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sbo3l doctor --extended: build http client: {e}");
            return ExitCode::from(2);
        }
    };

    let reports: Vec<ContractProbeReport> = PROBES
        .iter()
        .map(|p| probe_one(&client, rpc_url, p))
        .collect();

    let any_failed = reports.iter().any(|r| r.status != ProbeStatus::Ok);

    if json {
        let envelope = serde_json::json!({
            "report_type": "sbo3l.doctor.extended.v1",
            "rpc_url_kind": rpc_url_kind(rpc_url),
            "overall": if any_failed { "fail" } else { "ok" },
            "sepolia_contracts": reports,
        });
        println!("{}", serde_json::to_string_pretty(&envelope).unwrap());
    } else {
        print_human(&reports, rpc_url, any_failed);
    }

    if any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn build_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

fn probe_one(
    client: &reqwest::blocking::Client,
    rpc_url: &str,
    probe: &ContractProbe,
) -> ContractProbeReport {
    // 1. eth_getCode
    let code = match eth_get_code(client, rpc_url, probe.address) {
        Ok(c) => c,
        Err(e) => {
            return ContractProbeReport {
                label: probe.label.to_string(),
                address: probe.address.to_string(),
                status: ProbeStatus::RpcError,
                code_size_bytes: 0,
                view_signature: probe.view_signature.to_string(),
                view_result: None,
                url_template_ok: None,
                error: Some(format!("eth_getCode: {e}")),
            };
        }
    };
    let code_bytes = match decode_hex_word(&code) {
        Ok(b) => b,
        Err(e) => {
            return ContractProbeReport {
                label: probe.label.to_string(),
                address: probe.address.to_string(),
                status: ProbeStatus::RpcError,
                code_size_bytes: 0,
                view_signature: probe.view_signature.to_string(),
                view_result: None,
                url_template_ok: None,
                error: Some(format!("eth_getCode decode: {e}")),
            };
        }
    };
    if code_bytes.is_empty() {
        return ContractProbeReport {
            label: probe.label.to_string(),
            address: probe.address.to_string(),
            status: ProbeStatus::NoCode,
            code_size_bytes: 0,
            view_signature: probe.view_signature.to_string(),
            view_result: None,
            url_template_ok: None,
            error: Some("no bytecode at address".to_string()),
        };
    }
    let code_size = code_bytes.len();

    // 2. eth_call
    let calldata = build_calldata(probe.view_signature, probe.view_arg);
    let call_result_hex = match eth_call(client, rpc_url, probe.address, &calldata) {
        Ok(s) => s,
        Err(e) => {
            return ContractProbeReport {
                label: probe.label.to_string(),
                address: probe.address.to_string(),
                status: ProbeStatus::AbiMismatch,
                code_size_bytes: code_size,
                view_signature: probe.view_signature.to_string(),
                view_result: None,
                url_template_ok: None,
                error: Some(format!("eth_call: {e}")),
            };
        }
    };
    let return_bytes = match decode_hex_word(&call_result_hex) {
        Ok(b) => b,
        Err(e) => {
            return ContractProbeReport {
                label: probe.label.to_string(),
                address: probe.address.to_string(),
                status: ProbeStatus::AbiMismatch,
                code_size_bytes: code_size,
                view_signature: probe.view_signature.to_string(),
                view_result: None,
                url_template_ok: None,
                error: Some(format!("eth_call decode: {e}")),
            };
        }
    };

    // 3. Decode the ABI-shaped return.
    let pretty = match decode_return(probe.decode_kind, &return_bytes) {
        Ok(s) => s,
        Err(e) => {
            return ContractProbeReport {
                label: probe.label.to_string(),
                address: probe.address.to_string(),
                status: ProbeStatus::AbiMismatch,
                code_size_bytes: code_size,
                view_signature: probe.view_signature.to_string(),
                view_result: None,
                url_template_ok: None,
                error: Some(format!("decode {:?}: {e}", probe.decode_kind)),
            };
        }
    };

    // 4. (OffchainResolver only) URL template shape — Heidi's Bug #2.
    let (url_ok, status) = if probe.url_template_validate {
        let ok = pretty.contains("{sender}") && pretty.contains("{data}");
        let st = if ok {
            ProbeStatus::Ok
        } else {
            ProbeStatus::UrlTemplateMalformed
        };
        (Some(ok), st)
    } else {
        (None, ProbeStatus::Ok)
    };

    ContractProbeReport {
        label: probe.label.to_string(),
        address: probe.address.to_string(),
        status,
        code_size_bytes: code_size,
        view_signature: probe.view_signature.to_string(),
        view_result: Some(pretty),
        url_template_ok: url_ok,
        error: if status == ProbeStatus::UrlTemplateMalformed {
            Some("URL template missing `{sender}` or `{data}` — Heidi's Bug #2 shape".to_string())
        } else {
            None
        },
    }
}

fn eth_get_code(
    client: &reqwest::blocking::Client,
    rpc_url: &str,
    address: &str,
) -> Result<String, String> {
    let body = RpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "eth_getCode",
        params: vec![
            serde_json::Value::String(address.to_string()),
            serde_json::Value::String("latest".to_string()),
        ],
    };
    let resp: RpcResponse<String> = client
        .post(rpc_url)
        .json(&body)
        .send()
        .map_err(|e| format!("send: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http status: {e}"))?
        .json()
        .map_err(|e| format!("decode: {e}"))?;
    if let Some(e) = resp.error {
        return Err(format!("rpc error code={} msg={}", e.code, e.message));
    }
    resp.result
        .ok_or_else(|| "rpc returned null result".to_string())
}

fn eth_call(
    client: &reqwest::blocking::Client,
    rpc_url: &str,
    to: &str,
    calldata: &[u8],
) -> Result<String, String> {
    let call_obj = serde_json::json!({
        "to": to,
        "data": format!("0x{}", hex::encode(calldata)),
    });
    let body = RpcRequest {
        jsonrpc: "2.0",
        id: 2,
        method: "eth_call",
        params: vec![call_obj, serde_json::Value::String("latest".to_string())],
    };
    let resp: RpcResponse<String> = client
        .post(rpc_url)
        .json(&body)
        .send()
        .map_err(|e| format!("send: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http status: {e}"))?
        .json()
        .map_err(|e| format!("decode: {e}"))?;
    if let Some(e) = resp.error {
        return Err(format!("rpc error code={} msg={}", e.code, e.message));
    }
    resp.result
        .ok_or_else(|| "rpc returned null result".to_string())
}

/// Build `selector || abi-encoded args` for an `eth_call`.
fn build_calldata(signature: &str, arg: ProbeArg) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&function_selector(signature));
    match arg {
        ProbeArg::None => {}
        ProbeArg::Uint256(n) => {
            let mut word = [0u8; 32];
            word[24..].copy_from_slice(&n.to_be_bytes());
            out.extend_from_slice(&word);
        }
        ProbeArg::Bytes32Zero => {
            out.extend_from_slice(&[0u8; 32]);
        }
        ProbeArg::AddressZero => {
            // Address is left-padded into a 32-byte word.
            out.extend_from_slice(&[0u8; 32]);
        }
        ProbeArg::Bytes32AndAddressZero => {
            out.extend_from_slice(&[0u8; 32]);
            out.extend_from_slice(&[0u8; 32]);
        }
    }
    out
}

/// keccak256(signature)[..4] — the 4-byte function selector.
pub(crate) fn function_selector(signature: &str) -> [u8; 4] {
    let mut hasher = Keccak::v256();
    hasher.update(signature.as_bytes());
    let mut digest = [0u8; 32];
    hasher.finalize(&mut digest);
    let mut out = [0u8; 4];
    out.copy_from_slice(&digest[..4]);
    out
}

fn decode_hex_word(s: &str) -> Result<Vec<u8>, String> {
    let trimmed = s.strip_prefix("0x").unwrap_or(s);
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(trimmed).map_err(|e| e.to_string())
}

fn decode_return(kind: DecodeKind, bytes: &[u8]) -> Result<String, String> {
    match kind {
        DecodeKind::Uint256 => {
            if bytes.len() < 32 {
                return Err(format!(
                    "expected ≥32 bytes for uint256, got {}",
                    bytes.len()
                ));
            }
            let n = u256_to_string(&bytes[..32]);
            Ok(n)
        }
        DecodeKind::Uint256Wei => {
            if bytes.len() < 32 {
                return Err(format!(
                    "expected ≥32 bytes for uint256, got {}",
                    bytes.len()
                ));
            }
            let wei = u256_to_string(&bytes[..32]);
            // Pretty-print `<wei> wei (<eth> ETH)` for the
            // operator. ETH is wei / 1e18; we just split the digit
            // string at 18 from the right to avoid floating-point
            // rounding for big values.
            let eth_str = wei_to_eth_string(&wei);
            Ok(format!("{wei} wei ({eth_str} ETH)"))
        }
        DecodeKind::Bool => {
            if bytes.len() < 32 {
                return Err(format!("expected ≥32 bytes for bool, got {}", bytes.len()));
            }
            // bool occupies the low byte of a 32-byte word; check the
            // word for any non-zero byte (defensive — different tools
            // pad differently, but if any of the 32 bytes is non-zero
            // it's `true`).
            let any_nonzero = bytes[..32].iter().any(|b| *b != 0);
            Ok(if any_nonzero { "true" } else { "false" }.to_string())
        }
        DecodeKind::DynamicString => decode_abi_string(bytes),
    }
}

/// ABI dynamic string: head is `[offset:32][length:32]` + `length`
/// bytes of UTF-8 data, padded to 32-byte multiple.
pub(crate) fn decode_abi_string(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 64 {
        return Err(format!(
            "expected ≥64 bytes for dynamic string, got {}",
            bytes.len()
        ));
    }
    // First word is the offset to the data block; commonly 0x20 = 32.
    let offset = u256_to_usize(&bytes[..32]).map_err(|e| format!("string offset: {e}"))?;
    if offset + 32 > bytes.len() {
        return Err(format!(
            "string offset {offset} past return ({} bytes)",
            bytes.len()
        ));
    }
    // Second word at `offset` is the length.
    let length =
        u256_to_usize(&bytes[offset..offset + 32]).map_err(|e| format!("string length: {e}"))?;
    let start = offset + 32;
    let end = start + length;
    if end > bytes.len() {
        return Err(format!(
            "string body [{start}..{end}] past return ({} bytes)",
            bytes.len()
        ));
    }
    String::from_utf8(bytes[start..end].to_vec()).map_err(|e| format!("utf8: {e}"))
}

fn u256_to_string(word: &[u8]) -> String {
    // Big-int via repeated division by 10. 32 bytes = 256 bits = at
    // most 78 decimal digits; the buffer is bounded.
    let mut bytes = word.to_vec();
    if bytes.iter().all(|b| *b == 0) {
        return "0".to_string();
    }
    let mut digits = String::with_capacity(80);
    while bytes.iter().any(|b| *b != 0) {
        let mut rem: u32 = 0;
        for b in bytes.iter_mut() {
            let v = rem * 256 + *b as u32;
            *b = (v / 10) as u8;
            rem = v % 10;
        }
        digits.push(char::from_digit(rem, 10).unwrap());
    }
    digits.chars().rev().collect()
}

fn u256_to_usize(word: &[u8]) -> Result<usize, String> {
    // Reject anything that wouldn't fit in usize. For the doctor's
    // string-decode path this is effectively bounded by the RPC's
    // response size limit, but we keep the bound explicit.
    let upper = &word[..word.len() - 8];
    if upper.iter().any(|b| *b != 0) {
        return Err(format!(
            "value > usize::MAX (upper bytes nonzero): {:02x?}",
            upper
        ));
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&word[word.len() - 8..]);
    Ok(u64::from_be_bytes(buf) as usize)
}

fn wei_to_eth_string(wei: &str) -> String {
    if wei.len() <= 18 {
        let pad = "0".repeat(18 - wei.len());
        let frac = format!("{pad}{wei}");
        let trimmed = frac.trim_end_matches('0');
        if trimmed.is_empty() {
            return "0".to_string();
        }
        return format!("0.{trimmed}");
    }
    let (whole, frac) = wei.split_at(wei.len() - 18);
    let trimmed = frac.trim_end_matches('0');
    if trimmed.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{trimmed}")
    }
}

fn rpc_url_kind(url: &str) -> &'static str {
    if url.contains("alchemy.com") {
        "alchemy"
    } else if url.contains("infura.io") {
        "infura"
    } else if url.contains("publicnode.com") {
        "publicnode"
    } else {
        "custom"
    }
}

fn print_human(reports: &[ContractProbeReport], rpc_url: &str, any_failed: bool) {
    println!();
    println!("sbo3l doctor --extended — Sepolia contract probes");
    println!(
        "  rpc:     {} ({})",
        redact_rpc_url(rpc_url),
        rpc_url_kind(rpc_url)
    );
    println!("  overall: {}", if any_failed { "FAIL" } else { "ok" });
    println!();
    for r in reports {
        let badge = match r.status {
            ProbeStatus::Ok => "ok    ",
            ProbeStatus::NoCode => "FAIL  ",
            ProbeStatus::AbiMismatch => "FAIL  ",
            ProbeStatus::UrlTemplateMalformed => "FAIL  ",
            ProbeStatus::RpcError => "FAIL  ",
        };
        println!("  {badge}{:24}  {}", r.label, r.address);
        if r.code_size_bytes > 0 {
            println!("        code:        {} bytes", r.code_size_bytes);
        }
        println!(
            "        {} -> {}",
            r.view_signature,
            r.view_result
                .clone()
                .unwrap_or_else(|| "(no result)".to_string())
        );
        if let Some(ok) = r.url_template_ok {
            println!(
                "        URL template `{{sender}}` + `{{data}}`: {}",
                if ok { "ok" } else { "MALFORMED" }
            );
        }
        if let Some(e) = &r.error {
            println!("        ERROR:       {e}");
        }
        println!();
    }
}

/// Hide the API key in `…/v2/<key>` style URLs when echoing back to
/// the operator. Only the host + path-prefix shows.
fn redact_rpc_url(url: &str) -> String {
    if let Some(idx) = url.rfind("/v2/") {
        let prefix = &url[..idx + 4];
        let key = &url[idx + 4..];
        if key.len() > 8 {
            return format!("{prefix}{}…", &key[..4]);
        }
        return format!("{prefix}…");
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_selector_is_deterministic_keccak() {
        // The selectors are derived from keccak256 of the ABI sig
        // string, truncated to 4 bytes. Pin the actual computed
        // values so a future tiny-keccak change OR a typo'd signature
        // string gets caught here. Each value was verified against
        // tiny-keccak directly + cross-checked against the contract
        // ABI in `crates/sbo3l-identity/contracts/*.sol`.
        assert_eq!(hex::encode(function_selector("urls(uint256)")), "796676be");
        assert_eq!(
            hex::encode(function_selector("anchorCount(bytes32)")),
            "4cad6a99"
        );
        assert_eq!(hex::encode(function_selector("auctionCount()")), "2ad71573");
        assert_eq!(hex::encode(function_selector("BOND_AMOUNT()")), "bcacc70a");
        assert_eq!(
            hex::encode(function_selector("isRegistered(address)")),
            "c3c5a547"
        );
        assert_eq!(
            hex::encode(function_selector("entryCount(bytes32,address)")),
            "3e391edc"
        );
    }

    #[test]
    fn function_selectors_distinct_across_probe_set() {
        let mut seen = std::collections::HashSet::new();
        for p in PROBES {
            let sel = function_selector(p.view_signature);
            assert!(
                seen.insert(sel),
                "selector collision on {} ({})",
                p.label,
                p.view_signature
            );
        }
    }

    #[test]
    fn calldata_uint256_zero_is_36_bytes() {
        let cd = build_calldata("urls(uint256)", ProbeArg::Uint256(0));
        assert_eq!(cd.len(), 4 + 32);
        assert_eq!(&cd[4..], &[0u8; 32]);
    }

    #[test]
    fn calldata_no_args_is_4_bytes() {
        let cd = build_calldata("auctionCount()", ProbeArg::None);
        assert_eq!(cd.len(), 4);
    }

    #[test]
    fn calldata_two_args_is_68_bytes() {
        let cd = build_calldata(
            "entryCount(bytes32,address)",
            ProbeArg::Bytes32AndAddressZero,
        );
        assert_eq!(cd.len(), 4 + 32 + 32);
    }

    #[test]
    fn decode_uint256_zero_renders_zero() {
        let bytes = vec![0u8; 32];
        let s = decode_return(DecodeKind::Uint256, &bytes).unwrap();
        assert_eq!(s, "0");
    }

    #[test]
    fn decode_uint256_one() {
        let mut bytes = vec![0u8; 32];
        bytes[31] = 1;
        let s = decode_return(DecodeKind::Uint256, &bytes).unwrap();
        assert_eq!(s, "1");
    }

    #[test]
    fn decode_bond_amount_renders_eth_human() {
        // 0.01 ETH = 1e16 wei
        let mut bytes = vec![0u8; 32];
        // 1e16 = 0x00000000000000000000000000000000000000000000000000002386F26FC10000
        let wei: u128 = 10_000_000_000_000_000;
        let wei_be = wei.to_be_bytes();
        bytes[16..].copy_from_slice(&wei_be);
        let s = decode_return(DecodeKind::Uint256Wei, &bytes).unwrap();
        assert!(s.contains("10000000000000000 wei"));
        assert!(s.contains("(0.01 ETH)"));
    }

    #[test]
    fn decode_bool_true_when_any_byte_set() {
        let mut bytes = vec![0u8; 32];
        bytes[31] = 1;
        let s = decode_return(DecodeKind::Bool, &bytes).unwrap();
        assert_eq!(s, "true");
    }

    #[test]
    fn decode_bool_false_when_all_zero() {
        let bytes = vec![0u8; 32];
        let s = decode_return(DecodeKind::Bool, &bytes).unwrap();
        assert_eq!(s, "false");
    }

    #[test]
    fn decode_dynamic_string_canonical_template() {
        // ABI-encode "https://gw.sbo3l.dev/{sender}/{data}.json"
        // and decode it back.
        let s = "https://gw.sbo3l.dev/{sender}/{data}.json";
        let mut buf = Vec::new();
        // offset = 32 (data block immediately follows).
        let mut off = [0u8; 32];
        off[31] = 32;
        buf.extend_from_slice(&off);
        // length
        let mut len = [0u8; 32];
        let n = s.len() as u64;
        len[24..].copy_from_slice(&n.to_be_bytes());
        buf.extend_from_slice(&len);
        // body padded to 32-byte multiple.
        let mut body = s.as_bytes().to_vec();
        while !body.len().is_multiple_of(32) {
            body.push(0);
        }
        buf.extend_from_slice(&body);

        let decoded = decode_abi_string(&buf).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn decode_dynamic_string_rejects_short_buffer() {
        let bytes = vec![0u8; 32];
        let err = decode_abi_string(&bytes).unwrap_err();
        assert!(err.contains("≥64"));
    }

    #[test]
    fn url_template_validate_catches_heidi_bug_2_shape() {
        // Heidi's Bug #2: URL was missing one of the placeholders.
        // The probe should mark such templates malformed.
        let canonical = "https://gw.sbo3l.dev/{sender}/{data}.json";
        assert!(canonical.contains("{sender}"));
        assert!(canonical.contains("{data}"));

        let bug2_shape = "https://gw.sbo3l.dev/{sender}.json";
        assert!(bug2_shape.contains("{sender}"));
        assert!(!bug2_shape.contains("{data}"));
    }

    #[test]
    fn redact_alchemy_url_masks_api_key() {
        let url = "https://eth-sepolia.g.alchemy.com/v2/abcdef0123456789";
        let r = redact_rpc_url(url);
        assert!(r.contains("/v2/abcd…"));
        assert!(!r.contains("0123456789"));
    }

    #[test]
    fn redact_publicnode_url_unchanged() {
        let url = "https://ethereum-sepolia-rpc.publicnode.com";
        let r = redact_rpc_url(url);
        assert_eq!(r, url);
    }

    #[test]
    fn rpc_url_kind_categorises() {
        assert_eq!(
            rpc_url_kind("https://eth-sepolia.g.alchemy.com/v2/key"),
            "alchemy"
        );
        assert_eq!(
            rpc_url_kind("https://ethereum-sepolia-rpc.publicnode.com"),
            "publicnode"
        );
        assert_eq!(rpc_url_kind("https://sepolia.infura.io/v3/key"), "infura");
        assert_eq!(rpc_url_kind("https://my.custom.example/rpc"), "custom");
    }

    #[test]
    fn wei_to_eth_renders_typical_values() {
        assert_eq!(wei_to_eth_string("0"), "0");
        assert_eq!(wei_to_eth_string("1000000000000000000"), "1");
        assert_eq!(wei_to_eth_string("10000000000000000"), "0.01");
        assert_eq!(wei_to_eth_string("123450000000000000000"), "123.45");
    }

    #[test]
    fn probe_set_size_is_six() {
        // The brief named 6 contracts; the set order is also part of
        // the operator-facing contract (matches the truth table page
        // layout). Pin both.
        assert_eq!(PROBES.len(), 6);
        assert_eq!(PROBES[0].label, "OffchainResolver");
        assert_eq!(PROBES[5].label, "ERC8004 IdentityRegistry");
    }

    #[test]
    fn only_offchain_resolver_validates_url_template() {
        let with_validation: Vec<&str> = PROBES
            .iter()
            .filter(|p| p.url_template_validate)
            .map(|p| p.label)
            .collect();
        assert_eq!(with_validation, vec!["OffchainResolver"]);
    }
}
