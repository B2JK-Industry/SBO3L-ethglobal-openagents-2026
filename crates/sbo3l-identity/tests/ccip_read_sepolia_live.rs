//! Live integration test — loop-7 UAT fix verification.
//!
//! Tests that `LiveEnsResolver::resolve_raw_text` follows EIP-3668
//! OffchainLookup end-to-end for the SBO3L Sepolia subname, matching
//! the behaviour viem / ethers / the ENS App provide today.
//!
//! Skipped by default; runs when `SBO3L_SEPOLIA_RPC_URL` is set:
//!
//!   SBO3L_SEPOLIA_RPC_URL=https://... \
//!     cargo test -p sbo3l-identity --test ccip_read_sepolia_live \
//!       -- --ignored --include-ignored
//!
//! The expectation is byte-for-byte the same value `viem` returns
//! from `getEnsText({ name: 'research-agent.sbo3lagent.eth', key:
//! 'sbo3l:agent_id' })` — `"research-agent-01"`.

use sbo3l_identity::ens_anchor::EnsNetwork;
use sbo3l_identity::ens_live::{LiveEnsResolver, ReqwestTransport};

const ENV_RPC_URL: &str = "SBO3L_SEPOLIA_RPC_URL";

fn live_env() -> Option<String> {
    let rpc = std::env::var(ENV_RPC_URL).ok()?;
    if rpc.is_empty() {
        return None;
    }
    Some(rpc)
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn research_agent_subname_resolves_via_ccip_read() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    let transport = ReqwestTransport::new(rpc);
    let resolver = LiveEnsResolver::new(transport, EnsNetwork::Sepolia);
    let v = resolver
        .resolve_raw_text("research-agent.sbo3lagent.eth", "sbo3l:agent_id")
        .expect("CCIP-Read follow returns a value, not an error");
    assert_eq!(
        v.as_deref(),
        Some("research-agent-01"),
        "the gateway-signed agent_id record didn't match the expected fixture"
    );
}

#[test]
#[ignore = "live integration test; set SBO3L_SEPOLIA_RPC_URL and run with --include-ignored"]
fn research_agent_endpoint_record_resolves_via_ccip_read() {
    let Some(rpc) = live_env() else {
        eprintln!("SKIP: {ENV_RPC_URL} not set; skipping cleanly.");
        return;
    };
    let transport = ReqwestTransport::new(rpc);
    let resolver = LiveEnsResolver::new(transport, EnsNetwork::Sepolia);
    let v = resolver
        .resolve_raw_text("research-agent.sbo3lagent.eth", "sbo3l:endpoint")
        .expect("CCIP-Read follow returns a value, not an error");
    // The exact endpoint value is fixture-dependent; assert non-empty
    // + scheme prefix so the test stays correct as fixtures evolve.
    let s = v.expect("endpoint record is set on the gateway fixture");
    assert!(
        s.starts_with("http://") || s.starts_with("https://"),
        "endpoint record has unexpected shape: {s:?}"
    );
}
