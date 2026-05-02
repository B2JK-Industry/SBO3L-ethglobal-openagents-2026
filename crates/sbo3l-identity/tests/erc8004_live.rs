//! Live integration test for the ERC-8004 Identity Registry (T-4-2).
//!
//! **Skipped by default.** Activates only when both env vars are
//! present:
//!
//! - `SBO3L_ERC8004_ADDR` — 0x-prefixed 40-hex-char address of a
//!   deployed Identity Registry contract on the configured chain.
//!   Per Q-T42-1's A→B fallback, this is either Daniel's pinned
//!   canonical address or our reference-impl deploy.
//! - `SBO3L_SEPOLIA_RPC_URL` — Sepolia JSON-RPC endpoint. Memory
//!   note `alchemy_rpc_endpoints.md` has the canonical URL.
//!
//! What the test does (read-only, no broadcast — broadcast lands in
//! the T-4-2 follow-up that wires
//! `sbo3l_core::signers::eth::EthSigner`):
//!
//! 1. Builds a deterministic `registerAgent` calldata via
//!    [`sbo3l_identity::erc8004::build_dry_run`] for a fixed agent.
//! 2. Sends `eth_call` against the registry with that calldata. We
//!    expect the call to either return a `bytes32` agentId-shaped
//!    response (canonical contract responds to view calls
//!    pre-registration) OR revert (most ERC-8004 reference impls
//!    require non-view registration). Either path means the
//!    contract speaks the expected ABI.
//! 3. Asserts the response (or revert reason) carries no obvious
//!    "wrong selector" signal.
//!
//! Once the broadcast follow-up ships, this test extends to the full
//! `registerAgent` → `getAgent(agentId)` round-trip.
//!
//! **Why a separate test file vs `#[cfg(test)]`?** Integration tests
//! in `tests/` get their own `cargo test --test <name>` target that
//! CI can gate. The default `cargo test --workspace` invocation
//! includes them; the `#[ignore]` attribute opts out by default,
//! `--include-ignored` opts in.

use std::env;

use sbo3l_identity::ens_anchor::EnsNetwork;
use sbo3l_identity::erc8004::{build_dry_run, ChainConfig, RegisterRequest};
use serde_json::json;

const ENV_REGISTRY: &str = "SBO3L_ERC8004_ADDR";
const ENV_RPC_URL: &str = "SBO3L_SEPOLIA_RPC_URL";

fn live_env() -> Option<(String, String)> {
    let registry = env::var(ENV_REGISTRY).ok()?;
    let rpc_url = env::var(ENV_RPC_URL).ok()?;
    if registry.is_empty() || rpc_url.is_empty() {
        return None;
    }
    Some((registry, rpc_url))
}

fn parse_address(s: &str) -> [u8; 20] {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    let mut out = [0u8; 20];
    hex::decode_to_slice(stripped, &mut out).expect("registry address must be 0x + 40 hex");
    out
}

#[test]
#[ignore = "live integration test; set SBO3L_ERC8004_ADDR + SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn live_register_agent_calldata_targets_real_contract() {
    let (registry_hex, rpc_url) = match live_env() {
        Some(x) => x,
        None => {
            eprintln!(
                "SKIP: live ERC-8004 test requires {ENV_REGISTRY} + {ENV_RPC_URL} env vars; \
                 unset, skipping cleanly."
            );
            return;
        }
    };

    let registry = parse_address(&registry_hex);
    let cfg = ChainConfig::explicit(EnsNetwork::Sepolia, registry);
    let req = RegisterRequest {
        agent_address: [0xaa; 20],
        metadata_uri: "https://example.com/capsule.json",
        did: None,
        ens_fqdn: "research-agent.sbo3lagent.eth",
    };

    let dr = build_dry_run(cfg, req).expect("dry-run builds for live AC fixture");
    assert!(
        dr.register_calldata_hex.starts_with("0x5a27c211"),
        "calldata starts with the canonical registerAgent selector"
    );

    // Hit the real chain via eth_call. We don't expect this to
    // succeed semantically (registerAgent is a state-mutating fn,
    // not a view fn), but we DO expect:
    //   - the RPC to respond
    //   - the contract to be deployed at `registry`
    //   - the revert reason (if any) to NOT be the generic
    //     "function selector not recognised" signal
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [
            {
                "to": format!("0x{}", hex::encode(registry)),
                "data": dr.register_calldata_hex,
            },
            "latest"
        ]
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("build http client");

    let resp = client
        .post(&rpc_url)
        .json(&body)
        .send()
        .expect("eth_call request reaches Sepolia RPC");

    assert!(
        resp.status().is_success(),
        "Sepolia RPC returned non-2xx: {}",
        resp.status()
    );

    let json: serde_json::Value = resp.json().expect("parse JSON-RPC response body");

    // Either a "result" or an "error" — both are acceptable signals
    // that the contract is reachable. What we DON'T want is a
    // null result + null error (RPC misconfigured) or a generic
    // "execution reverted: function not found" signal.
    let has_result = json.get("result").is_some() && !json["result"].is_null();
    let has_error = json.get("error").is_some() && !json["error"].is_null();

    assert!(
        has_result || has_error,
        "expected `result` or `error` from eth_call; got {json}"
    );

    if has_result {
        // `eth_call` returns `"0x"` (zero bytes) when the target
        // address has no code at all (EOA) or when a contract returns
        // an empty payload. Treating that as success is a false
        // positive — it lets a misconfigured `SBO3L_ERC8004_ADDR`
        // pointing at an EOA pass the test. Require the result to be
        // a non-empty hex string. ABI-encoded returns are 32-byte
        // aligned, so we accept any length ≥ 2 (the "0x" prefix) +
        // 64 hex chars = 66, but in practice anything > 2 indicates
        // real contract execution.
        let result_str = json["result"]
            .as_str()
            .expect("`result` field is a string per JSON-RPC spec");
        assert!(
            result_str.len() > 2 && result_str.starts_with("0x"),
            "`eth_call` returned empty `\"0x\"` — target at {registry_hex} has no code or returned empty. Did SBO3L_ERC8004_ADDR get pointed at an EOA?"
        );
    }

    if let Some(err) = json.get("error") {
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
        assert!(
            !msg.contains("function selector was not recognized"),
            "registry at {registry_hex} doesn't speak registerAgent; selector mismatch: {msg}"
        );
    }
}
