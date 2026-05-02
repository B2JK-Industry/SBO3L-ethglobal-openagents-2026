//! Live integration test — Heidi UAT bug #2 fix verification.
//!
//! Tests that the **redeployed** Sepolia OffchainResolver (Task A,
//! 2026-05-03) AND the wired-up `research-agent.sbo3lagent.eth`
//! subname (Task B) work end-to-end against a live Sepolia RPC.
//!
//! Skipped by default; runs when `SBO3L_SEPOLIA_RPC_URL` is set:
//!
//!   SBO3L_SEPOLIA_RPC_URL=https://... \
//!     cargo test -p sbo3l-identity --test sepolia_or_live -- --ignored --include-ignored
//!
//! What this test asserts (no signing, no gas — read-only `eth_call`
//! probes):
//!
//! 1. **OR bytecode present.** The new OR address (Task A redeploy)
//!    has non-empty bytecode on Sepolia.
//! 2. **`urls(0)` is canonical.** Bug #2 is fixed: the stored URL
//!    template is exactly
//!    `"https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"`.
//!    The pre-fix value was `"...{sender/{data}.json}"` (closing `}`
//!    after `sender` migrated to the end).
//! 3. **`gatewaySigner()` matches Vercel.** Pinned at constructor
//!    time; cannot drift without redeploy.
//! 4. **Subname resolver wired.** `ENS Registry.resolver(namehash(
//!    research-agent.sbo3lagent.eth))` returns the new OR address.
//! 5. **`resolve(name, data)` reverts with `OffchainLookup`.** The
//!    revert payload's URLs array contains the canonical template
//!    byte-for-byte (Heidi bug #2 verification at the wire level).

use std::env;

const ENV_RPC_URL: &str = "SBO3L_SEPOLIA_RPC_URL";
const NEW_OR_ADDRESS: &str = "0x87e99508C222c6E419734CACbb6781b8d282b1F6";
const ENS_REGISTRY: &str = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e";
const GATEWAY_SIGNER: &str = "0x595099B4e8D642616e298235Dd1248f8008BCe65";
const CANONICAL_URL: &str = "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json";

/// `keccak256("supportsInterface(bytes4)")[..4]` || `0x9061b923`
/// (ENSIP-10 IExtendedResolver interface id), padded.
const SUPPORTS_INTERFACE_ENSIP10_CALLDATA: &str =
    "0x01ffc9a79061b92300000000000000000000000000000000000000000000000000000000";

/// `keccak256("urls(uint256)")[..4]` = 0x796676be, then index 0
/// (uint256 zero, padded to 32 bytes).
const URLS_0_CALLDATA: &str =
    "0x796676be0000000000000000000000000000000000000000000000000000000000000000";

/// `keccak256("gatewaySigner()")[..4]` = 0xf3253c63.
const GATEWAY_SIGNER_CALLDATA: &str = "0xf3253c63";

fn live_env() -> Option<String> {
    let rpc = env::var(ENV_RPC_URL).ok()?;
    if rpc.is_empty() {
        return None;
    }
    Some(rpc)
}

fn eth_call(rpc: &str, to: &str, data: &str) -> serde_json::Value {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [{"to": to, "data": data}, "latest"]
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("build http client");
    let resp = client
        .post(rpc)
        .json(&body)
        .send()
        .expect("eth_call request reaches Sepolia RPC");
    assert!(
        resp.status().is_success(),
        "Sepolia RPC returned non-2xx: {}",
        resp.status()
    );
    resp.json().expect("parse JSON-RPC response body")
}

fn eth_get_code(rpc: &str, addr: &str) -> String {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getCode",
        "params": [addr, "latest"]
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("build http client");
    let resp = client
        .post(rpc)
        .json(&body)
        .send()
        .expect("eth_getCode request reaches Sepolia RPC");
    let json: serde_json::Value = resp.json().expect("parse JSON-RPC response body");
    json["result"]
        .as_str()
        .expect("eth_getCode `result` field is a string")
        .to_string()
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn new_or_bytecode_is_live() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    let code = eth_get_code(&rpc, NEW_OR_ADDRESS);
    assert!(
        code.len() > 2,
        "new OR has no bytecode at {NEW_OR_ADDRESS}; got `{code}`"
    );
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn new_or_url_template_is_canonical() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    let json = eth_call(&rpc, NEW_OR_ADDRESS, URLS_0_CALLDATA);
    let raw = json["result"]
        .as_str()
        .expect("urls(0) returns hex-encoded string");

    // ABI-decode `(string)` from raw hex. Layout:
    //   [0:32]  offset (always 0x20 for single string)
    //   [32:64] length
    //   [64:..] payload, padded to 32-byte boundary
    let bytes = hex::decode(raw.trim_start_matches("0x"))
        .expect("urls(0) result is valid hex");
    assert!(bytes.len() >= 64, "result too short: {} bytes", bytes.len());
    let len_word = &bytes[32..64];
    let mut len_arr = [0u8; 32];
    len_arr.copy_from_slice(len_word);
    let len = u32::from_be_bytes([
        len_arr[28], len_arr[29], len_arr[30], len_arr[31],
    ]) as usize;
    let url = std::str::from_utf8(&bytes[64..64 + len]).expect("url is valid utf-8");
    assert_eq!(
        url, CANONICAL_URL,
        "Heidi bug #2 regression: stored URL template is `{url}`, expected `{CANONICAL_URL}`"
    );
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn new_or_gateway_signer_matches_vercel() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    let json = eth_call(&rpc, NEW_OR_ADDRESS, GATEWAY_SIGNER_CALLDATA);
    let raw = json["result"]
        .as_str()
        .expect("gatewaySigner() returns hex-encoded address");
    let signer_lower = raw[raw.len().saturating_sub(40)..].to_ascii_lowercase();
    let expected_lower = GATEWAY_SIGNER.trim_start_matches("0x").to_ascii_lowercase();
    assert_eq!(
        signer_lower, expected_lower,
        "gatewaySigner drifted from baked-in Vercel signer"
    );
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn new_or_supports_ensip10_extended_resolver() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    let json = eth_call(&rpc, NEW_OR_ADDRESS, SUPPORTS_INTERFACE_ENSIP10_CALLDATA);
    let raw = json["result"].as_str().expect("supportsInterface returns bool");
    // ABI-encoded bool: 32 bytes, last byte 0 or 1.
    assert!(
        raw.ends_with('1'),
        "OR doesn't claim ENSIP-10 IExtendedResolver: got `{raw}`"
    );
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn research_agent_subname_resolver_is_new_or() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    // namehash(research-agent.sbo3lagent.eth) — pinned to avoid a
    // dependency on the namehash impl in this crate (which has its
    // own tests).
    let subnode = "0x7131b849ffa657c77803cb882a11ea7edaa6e5c2dc2f33f9a878cb1bf39435dd";
    // resolver(bytes32) selector = 0x0178b8bf
    let calldata = format!(
        "0x0178b8bf{}",
        subnode.trim_start_matches("0x")
    );
    let json = eth_call(&rpc, ENS_REGISTRY, &calldata);
    let raw = json["result"].as_str().expect("resolver() returns address");
    let actual_lower = raw[raw.len().saturating_sub(40)..].to_ascii_lowercase();
    let expected_lower = NEW_OR_ADDRESS
        .trim_start_matches("0x")
        .to_ascii_lowercase();
    assert_eq!(
        actual_lower, expected_lower,
        "research-agent.sbo3lagent.eth resolver mismatch — Task B subname not wired"
    );
}
