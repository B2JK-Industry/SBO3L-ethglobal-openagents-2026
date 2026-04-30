//! B3 live smoke: resolve an ENS name's `sbo3l:*` text records via
//! [`LiveEnsResolver`]. Operator-supplied env vars:
//!
//!   SBO3L_ENS_RPC_URL     — required (e.g. Alchemy mainnet free-tier)
//!   SBO3L_ENS_NAME        — defaults to `sbo3lagent.eth`
//!   SBO3L_ENS_NETWORK     — defaults to `mainnet`; `sepolia` also valid
//!
//! Use:
//!
//!   export SBO3L_ENS_RPC_URL='https://eth-mainnet.g.alchemy.com/v2/...'
//!   cargo run -p sbo3l-identity --example ens_live_smoke
//!
//! Prints the five SBO3L text records read from chain. Does NOT log
//! the RPC URL itself (it's a credential — operator supplies via env,
//! we never echo). CI does not run this example: no RPC URL.
//!
//! Exit codes:
//! - 0 — all five records resolved
//! - 1 — IO / configuration error (env var missing, RPC unreachable)
//! - 2 — resolution failure (UnknownName, MissingRecord, malformed
//!   response)

use std::process::ExitCode;

use sbo3l_identity::ens::EnsResolver;
use sbo3l_identity::{EnsNetwork, LiveEnsResolver, ResolveError};

fn main() -> ExitCode {
    let network_raw = std::env::var("SBO3L_ENS_NETWORK").unwrap_or_else(|_| "mainnet".to_string());
    let network = match EnsNetwork::parse(&network_raw) {
        Ok(n) => n,
        Err(e) => {
            eprintln!(
                "ens_live_smoke: SBO3L_ENS_NETWORK={network_raw} invalid: {e}. \
                 Use `mainnet` or `sepolia`."
            );
            return ExitCode::from(1);
        }
    };

    let name = std::env::var("SBO3L_ENS_NAME").unwrap_or_else(|_| "sbo3lagent.eth".to_string());

    let resolver = match LiveEnsResolver::from_env(network) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ens_live_smoke: configuration error: {e}");
            eprintln!(
                "Required env: SBO3L_ENS_RPC_URL. Optional: SBO3L_ENS_NAME \
                 (default: sbo3lagent.eth), SBO3L_ENS_NETWORK (default: mainnet)."
            );
            return ExitCode::from(1);
        }
    };

    println!("ens_live_smoke: resolving {name} on {}", network.as_str());
    match resolver.resolve(&name) {
        Ok(records) => {
            println!("  agent_id:    {}", records.agent_id);
            println!("  endpoint:    {}", records.endpoint);
            println!("  policy_hash: {}", records.policy_hash);
            println!("  audit_root:  {}", records.audit_root);
            println!("  proof_uri:   {}", records.proof_uri);
            ExitCode::SUCCESS
        }
        Err(ResolveError::Io(e)) => {
            eprintln!("ens_live_smoke: IO/RPC error: {e}");
            ExitCode::from(1)
        }
        Err(ResolveError::UnknownName(n)) => {
            eprintln!("ens_live_smoke: UnknownName: {n} has no resolver set");
            ExitCode::from(2)
        }
        Err(ResolveError::MissingRecord(field, n)) => {
            eprintln!("ens_live_smoke: MissingRecord: {n} has no sbo3l:{field} text record");
            ExitCode::from(2)
        }
        Err(other) => {
            eprintln!("ens_live_smoke: unexpected error: {other}");
            ExitCode::from(2)
        }
    }
}
