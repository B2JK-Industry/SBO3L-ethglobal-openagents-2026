//! Multi-chain reputation broadcast (R11 P2).
//!
//! Compiled only with `--features eth_broadcast`. Companion to the
//! single-chain broadcaster in [`crate::agent_reputation_broadcast`].
//! Same harness pattern (alloy + PrivateKeySigner + redacted RPC log)
//! extended to **N target chains in one CLI invocation**.
//!
//! ## How it works
//!
//! 1. Compute the v2 reputation score ONCE (from the events file).
//! 2. For each chain in `--multi-chain <list>`: read per-chain RPC URL
//!    from `SBO3L_RPC_URL_<UPPERCASE_CHAIN>`, read per-chain registry
//!    address from `SBO3L_REPUTATION_REGISTRY_<UPPERCASE_CHAIN>` (or
//!    fall back to ENS `setText`), sign + send the tx, wait one
//!    confirmation, capture tx hash + Etherscan link.
//! 3. Print a per-chain summary table.
//!
//! ## Why per-chain signatures (not "single signature replayed")
//!
//! `SBO3LReputationRegistry`'s digest binds to `address(this)` —
//! sigs from one deploy are NOT valid on another deploy at a
//! different address. That's an intentional security property: a
//! malicious deploy at a chosen address can't replay sigs from the
//! canonical deploy. The "same score across chains" property is
//! preserved at the **score** level, not the **signature** level.
//! Same agent, same score, N per-chain signatures. The audit log
//! captures all N tx hashes.
//!
//! ## Chain registry
//!
//! Per-chain config is keyed by a short label. Initial set
//! (Daniel's R11 P2 spec):
//!
//! | Label              | chain_id | RPC env var                            |
//! |--------------------|----------|----------------------------------------|
//! | `sepolia`          | 11155111 | `SBO3L_RPC_URL_SEPOLIA`                |
//! | `optimism-sepolia` | 11155420 | `SBO3L_RPC_URL_OPTIMISM_SEPOLIA`       |
//! | `base-sepolia`     | 84532    | `SBO3L_RPC_URL_BASE_SEPOLIA`           |
//! | `mainnet`          | 1        | `SBO3L_RPC_URL_MAINNET`                |
//! | `optimism`         | 10       | `SBO3L_RPC_URL_OPTIMISM`               |
//! | `base`             | 8453     | `SBO3L_RPC_URL_BASE`                   |
//!
//! Mainnet entries (`mainnet`, `optimism`, `base`) require
//! `SBO3L_ALLOW_MAINNET_TX=1`. Multi-chain mode that includes any
//! mainnet entry refuses without the gate.

use std::process::ExitCode;

use crate::agent_reputation::ReputationPublishArgs;

/// Chain label → (chain_id, RPC env var, is_mainnet, optional pinned
/// registry address). The `registry_addr` slot is `None` until
/// `scripts/deploy-reputation-registry.sh <network>` runs and the
/// resulting address is pinned here. With `None`, multi-chain
/// broadcast falls back to ENS resolver `setText` for chains that
/// have native ENS, and skips chains that don't.
#[derive(Debug)]
pub struct ChainSpec {
    pub label: &'static str,
    pub chain_id: u64,
    pub rpc_env: &'static str,
    pub is_mainnet: bool,
    pub registry_addr: Option<&'static str>,
}

const CHAINS: &[ChainSpec] = &[
    ChainSpec {
        label: "sepolia",
        chain_id: 11155111,
        rpc_env: "SBO3L_RPC_URL_SEPOLIA",
        is_mainnet: false,
        // Pin after running `./scripts/deploy-reputation-registry.sh sepolia`.
        registry_addr: None,
    },
    ChainSpec {
        label: "optimism-sepolia",
        chain_id: 11155420,
        rpc_env: "SBO3L_RPC_URL_OPTIMISM_SEPOLIA",
        is_mainnet: false,
        // Pin after running `./scripts/deploy-reputation-registry.sh optimism-sepolia`.
        registry_addr: None,
    },
    ChainSpec {
        label: "base-sepolia",
        chain_id: 84532,
        rpc_env: "SBO3L_RPC_URL_BASE_SEPOLIA",
        is_mainnet: false,
        // Pin after running `./scripts/deploy-reputation-registry.sh base-sepolia`.
        registry_addr: None,
    },
    ChainSpec {
        label: "mainnet",
        chain_id: 1,
        rpc_env: "SBO3L_RPC_URL_MAINNET",
        is_mainnet: true,
        registry_addr: None,
    },
    ChainSpec {
        label: "optimism",
        chain_id: 10,
        rpc_env: "SBO3L_RPC_URL_OPTIMISM",
        is_mainnet: true,
        registry_addr: None,
    },
    ChainSpec {
        label: "base",
        chain_id: 8453,
        rpc_env: "SBO3L_RPC_URL_BASE",
        is_mainnet: true,
        registry_addr: None,
    },
];

fn lookup_chain(label: &str) -> Option<&'static ChainSpec> {
    CHAINS.iter().find(|c| c.label == label)
}

/// Parse a comma-separated chain list. Returns an Err with the
/// offending label if any entry is unknown. Trims whitespace
/// around each entry so `"sepolia, optimism-sepolia"` is accepted.
pub fn parse_chain_list(list: &str) -> Result<Vec<&'static ChainSpec>, String> {
    let mut out = Vec::new();
    for raw in list.split(',') {
        let label = raw.trim();
        if label.is_empty() {
            continue;
        }
        match lookup_chain(label) {
            Some(spec) => out.push(spec),
            None => {
                let known: Vec<&str> = CHAINS.iter().map(|c| c.label).collect();
                return Err(format!(
                    "unknown chain '{label}'. Known: {}",
                    known.join(", ")
                ));
            }
        }
    }
    if out.is_empty() {
        return Err("multi-chain list is empty".into());
    }
    Ok(out)
}

/// Multi-chain broadcast entry point. Dispatched from
/// `agent_reputation::cmd_agent_reputation_publish` when
/// `--multi-chain` is set AND the `eth_broadcast` feature is on.
#[cfg(feature = "eth_broadcast")]
pub async fn cmd_broadcast_multi(args: ReputationPublishArgs) -> ExitCode {
    use sbo3l_identity::ens_anchor::EnsNetwork;

    let list = match args.multi_chain.as_deref() {
        Some(s) => s,
        None => {
            eprintln!(
                "sbo3l agent reputation-publish --multi-chain: --multi-chain flag is required for this dispatch"
            );
            return ExitCode::from(2);
        }
    };
    let chains = match parse_chain_list(list) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --multi-chain: {e}");
            return ExitCode::from(2);
        }
    };

    // Mainnet double-gate applies if any chain in the list is a
    // mainnet entry.
    let any_mainnet = chains.iter().any(|c| c.is_mainnet);
    if any_mainnet && std::env::var("SBO3L_ALLOW_MAINNET_TX").as_deref() != Ok("1") {
        eprintln!(
            "sbo3l agent reputation-publish --multi-chain: refusing without \
             SBO3L_ALLOW_MAINNET_TX=1 (one or more chains in the list is mainnet). \
             Set the env var to acknowledge before re-running."
        );
        return ExitCode::from(2);
    }

    println!("sbo3l agent reputation-publish --multi-chain");
    println!("  fqdn:           {}", args.fqdn);
    println!(
        "  chains:         {}",
        chains
            .iter()
            .map(|c| c.label)
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut successes = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for spec in &chains {
        println!();
        println!("─── chain: {} (id {}) ───", spec.label, spec.chain_id);

        // Surface the pinned registry address if any. Pin happens
        // post-deploy by editing this file; until then this slot is
        // None and the broadcast falls back to the ENS resolver
        // setText path. Once pinned, a follow-up wires
        // SBO3LReputationRegistry.writeReputation(...) as the
        // broadcast target.
        if let Some(registry) = spec.registry_addr {
            println!("  registry:    {registry} (target-switch wires in follow-up)");
        }

        // Per-chain RPC URL.
        let rpc_url = match std::env::var(spec.rpc_env) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                let msg = format!("skipped: per-chain RPC env var {} not set", spec.rpc_env);
                println!("  {msg}");
                failures.push(format!("{}: {msg}", spec.label));
                continue;
            }
        };

        // Map to EnsNetwork for the existing single-chain broadcaster.
        // Sepolia + mainnet have native ENS; the other entries
        // require SBO3LReputationRegistry deploys (a follow-up).
        let ens_network = match spec.chain_id {
            1 => EnsNetwork::Mainnet,
            11155111 => EnsNetwork::Sepolia,
            _ => {
                let msg =
                    "skipped: native ENS not deployed; requires SBO3LReputationRegistry follow-up";
                println!("  {msg}");
                failures.push(format!("{}: {msg}", spec.label));
                continue;
            }
        };

        // Build per-chain args (override --rpc-url + --network).
        let per_chain_args = ReputationPublishArgs {
            fqdn: args.fqdn.clone(),
            events: args.events.clone(),
            network: match ens_network {
                EnsNetwork::Mainnet => "mainnet".to_string(),
                EnsNetwork::Sepolia => "sepolia".to_string(),
            },
            resolver: args.resolver.clone(),
            out: None, // single-chain envelope output suppressed in multi-chain mode
            broadcast: true,
            rpc_url: Some(rpc_url),
            private_key_env_var: args.private_key_env_var.clone(),
            multi_chain: None, // prevent re-entry
        };

        let code =
            crate::agent_reputation_broadcast::cmd_broadcast(per_chain_args, ens_network).await;
        // ExitCode doesn't expose its inner u8; format-and-inspect
        // is the stable test pattern (see `agent_reputation::tests::
        // broadcast_without_feature_returns_exit3`).
        let formatted = format!("{code:?}");
        if formatted.contains("(0)") || formatted.contains("Success") {
            successes += 1;
        } else {
            failures.push(format!("{}: per-chain broadcast failed", spec.label));
        }
    }

    println!();
    println!("─── summary ───");
    println!("  succeeded: {successes}");
    println!("  failed:    {}", failures.len());
    for f in &failures {
        println!("    - {f}");
    }

    if successes == 0 {
        ExitCode::from(1)
    } else if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        // Partial success — exit 0 with the failures already printed.
        // Operator runs `sbo3l agent verify-ens <fqdn> --network <chain>`
        // per chain to double-check, then re-runs --multi-chain with
        // a narrower list for the failures.
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_chain_list_happy_path() {
        let chains = parse_chain_list("sepolia, optimism-sepolia,base-sepolia").unwrap();
        assert_eq!(chains.len(), 3);
        assert_eq!(chains[0].chain_id, 11155111);
        assert_eq!(chains[1].chain_id, 11155420);
        assert_eq!(chains[2].chain_id, 84532);
    }

    #[test]
    fn parse_chain_list_rejects_unknown() {
        let err = parse_chain_list("sepolia,polygon-zkevm").unwrap_err();
        assert!(err.contains("unknown chain 'polygon-zkevm'"));
        assert!(err.contains("Known:"));
    }

    #[test]
    fn parse_chain_list_rejects_empty() {
        let err = parse_chain_list("").unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn parse_chain_list_skips_blank_entries() {
        let chains = parse_chain_list(",sepolia, ,").unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].label, "sepolia");
    }

    #[test]
    fn parse_chain_list_includes_mainnet_chains() {
        let chains = parse_chain_list("mainnet,optimism,base").unwrap();
        assert_eq!(chains.len(), 3);
        assert!(chains.iter().all(|c| c.is_mainnet));
    }

    #[test]
    fn lookup_chain_known_set() {
        for label in ["sepolia", "optimism-sepolia", "base-sepolia", "mainnet"] {
            assert!(lookup_chain(label).is_some(), "lookup failed for {label}");
        }
    }

    #[test]
    fn lookup_chain_returns_none_for_unknown() {
        assert!(lookup_chain("polygon-zkevm").is_none());
    }
}
