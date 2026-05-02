//! T-3-2 live integration test for `sbo3l agent verify-ens`.
//!
//! Skipped by default (marked `#[ignore]`). Activates when both:
//!
//!   SBO3L_LIVE_ETH=1
//!   SBO3L_ENS_RPC_URL=https://...   (mainnet RPC)
//!
//! Resolves the canonical sbo3l:* records on `sbo3lagent.eth`
//! (Daniel's mainnet apex) via PublicNode / Alchemy / any chain RPC.
//! Asserts the records that DO exist on-chain (agent_id, endpoint,
//! policy_hash, audit_root, proof_uri) come back non-empty. Records
//! that don't exist on the apex (pubkey_ed25519, policy_url,
//! capabilities — those land at fleet-broadcast time per T-3-3) are
//! permitted to be absent.
//!
//! The test uses `LiveEnsResolver` directly rather than the CLI
//! binary so it can run inside `cargo test --workspace`. The CLI
//! shape itself is covered by the unit tests in
//! `crates/sbo3l-cli/src/agent_verify.rs`.

use std::env;

use sbo3l_identity::ens_anchor::EnsNetwork;
use sbo3l_identity::ens_live::LiveEnsResolver;

const ENV_GATE: &str = "SBO3L_LIVE_ETH";
const ENV_RPC: &str = "SBO3L_ENS_RPC_URL";
const APEX: &str = "sbo3lagent.eth";

fn live_active() -> bool {
    env::var(ENV_GATE).as_deref() == Ok("1")
        && env::var(ENV_RPC).map(|v| !v.is_empty()).unwrap_or(false)
}

#[test]
#[ignore = "live integration test; set SBO3L_LIVE_ETH=1 + SBO3L_ENS_RPC_URL and run with --include-ignored"]
fn live_verify_apex_canonical_records_present() {
    if !live_active() {
        eprintln!(
            "SKIP: live verify-ens test requires {ENV_GATE}=1 + {ENV_RPC} set; \
             cleanly skipping."
        );
        return;
    }

    let resolver = LiveEnsResolver::from_env(EnsNetwork::Mainnet)
        .expect("LiveEnsResolver from SBO3L_ENS_RPC_URL");

    // Canonical records present on the apex pre-T-3-3 broadcast.
    // These land via the manual ENS App setup Daniel did pre-hackathon
    // — see memory note `submission_2026-04-30_live_verification.md`.
    for key in [
        "sbo3l:agent_id",
        "sbo3l:endpoint",
        "sbo3l:policy_hash",
        "sbo3l:audit_root",
        "sbo3l:proof_uri",
    ] {
        let value = resolver
            .resolve_raw_text(APEX, key)
            .unwrap_or_else(|e| panic!("resolve_raw_text {APEX} {key}: {e}"));
        let value = value.unwrap_or_else(|| {
            panic!("{APEX} record {key} resolved as None — apex must carry the canonical 5")
        });
        assert!(
            !value.is_empty(),
            "{APEX} record {key} resolved as empty string"
        );
        eprintln!("  {APEX} {key} = {value}");
    }
}

#[test]
#[ignore = "live integration test; set SBO3L_LIVE_ETH=1 + SBO3L_ENS_RPC_URL and run with --include-ignored"]
fn live_verify_apex_fleet_records_absent_or_present() {
    if !live_active() {
        return;
    }

    let resolver = LiveEnsResolver::from_env(EnsNetwork::Mainnet)
        .expect("LiveEnsResolver from SBO3L_ENS_RPC_URL");

    // T-3-3 fleet-time records — pubkey_ed25519, policy_url,
    // capabilities. Pre-broadcast, these are absent on the apex
    // itself; post-broadcast, they exist on the per-agent subnames.
    // Either outcome is valid for the apex; we just assert the
    // resolver doesn't error.
    for key in [
        "sbo3l:pubkey_ed25519",
        "sbo3l:policy_url",
        "sbo3l:capabilities",
    ] {
        let value = resolver
            .resolve_raw_text(APEX, key)
            .unwrap_or_else(|e| panic!("resolve_raw_text {APEX} {key}: {e}"));
        match value {
            Some(v) => eprintln!("  {APEX} {key} = {v}  (post-broadcast)"),
            None => eprintln!("  {APEX} {key} = absent  (pre-broadcast — expected)"),
        }
    }
}
