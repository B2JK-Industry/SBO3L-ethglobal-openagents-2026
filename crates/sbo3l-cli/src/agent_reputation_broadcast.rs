//! T-4-7 reputation broadcast — `sbo3l agent reputation-publish --broadcast`
//! real path.
//!
//! Compiled only with `--features eth_broadcast`. Mirrors the T-3-1
//! broadcast harness in [`crate::agent_broadcast`] — same alloy stack,
//! same env-var conventions (`SBO3L_RPC_URL`, `SBO3L_SIGNER_KEY`),
//! same redacted-RPC log line, same Etherscan link emission.
//!
//! # Wire
//!
//! One transaction:
//!
//! 1. `Resolver.setText(node, "sbo3l:reputation_score", "<score>")`
//!    → recipient: the resolver address (default: the network's
//!    PublicResolver per
//!    [`sbo3l_identity::EnsNetwork::default_public_resolver`]; can
//!    be overridden via `--resolver`).
//!
//! Single-tx flow because the reputation record is just a setText —
//! no subname issuance, no multicall. Mainnet path additionally
//! requires `SBO3L_ALLOW_MAINNET_TX=1` (enforced upstream in the
//! dispatch in [`crate::agent_reputation::cmd_agent_reputation_publish`]).
//!
//! # Inputs / outputs
//!
//! Same args as the dry-run path: `--fqdn`, `--events`, `--network`,
//! optional `--resolver`. Plus the broadcast-only env vars
//! `SBO3L_SIGNER_KEY` (32-byte hex private key) and
//! `SBO3L_RPC_URL` (or override via `--rpc-url`).
//!
//! Output: stdout prints the score, the resolver address, the tx
//! hash, the Etherscan link, the confirmed block, and the gas used.
//! On confirmation failure the tx hash is still printed so the
//! operator can investigate manually.

use std::process::ExitCode;

use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::{Address, Bytes, FixedBytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use sbo3l_identity::ens_anchor::{namehash, set_text_calldata, EnsNetwork};
use sbo3l_identity::reputation_publisher::{
    build_publish_envelope, PublishMode, ReputationEventInput, ReputationPublishParams,
    REPUTATION_TEXT_KEY,
};

use crate::agent_reputation::ReputationPublishArgs;

const DEFAULT_SIGNER_ENV: &str = "SBO3L_SIGNER_KEY";
const DEFAULT_RPC_ENV: &str = "SBO3L_RPC_URL";

pub async fn cmd_broadcast(args: ReputationPublishArgs, network: EnsNetwork) -> ExitCode {
    let rpc_url = match resolve_rpc_url(&args) {
        Ok(s) => s,
        Err(rc) => return rc,
    };
    let signer_env = args
        .private_key_env_var
        .as_deref()
        .unwrap_or(DEFAULT_SIGNER_ENV);
    let signer = match load_signer(signer_env) {
        Ok(s) => s,
        Err(rc) => return rc,
    };
    let signer_address = signer.address();

    let resolver_str = args
        .resolver
        .clone()
        .unwrap_or_else(|| network.default_public_resolver().to_string());
    let resolver = match parse_address_str(&resolver_str, "resolver") {
        Ok(a) => a,
        Err(rc) => return rc,
    };

    // Read events file + compute score via the same pure-function
    // publisher the dry-run path uses. Same input always re-derives
    // the same score; the broadcast-side and dry-run-side outputs
    // are byte-identical apart from the actual on-chain side
    // effect.
    let raw_events = match std::fs::read_to_string(&args.events) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "sbo3l agent reputation-publish --broadcast: read events file {}: {e}",
                args.events.display()
            );
            return ExitCode::from(2);
        }
    };
    let events: Vec<ReputationEventInput> = match serde_json::from_str(&raw_events) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "sbo3l agent reputation-publish --broadcast: parse events file {}: {e}",
                args.events.display()
            );
            return ExitCode::from(2);
        }
    };

    let envelope = match build_publish_envelope(
        ReputationPublishParams {
            network,
            domain: &args.fqdn,
            resolver: &resolver_str,
            created_at: &current_rfc3339(),
            mode: PublishMode::DryRun,
        },
        &events,
    ) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --broadcast: envelope build failed: {e}");
            return ExitCode::from(2);
        }
    };

    let node = match namehash(&args.fqdn) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --broadcast: bad --fqdn: {e}");
            return ExitCode::from(2);
        }
    };
    let calldata = set_text_calldata(node, REPUTATION_TEXT_KEY, &envelope.score.to_string());

    println!("sbo3l agent reputation-publish --broadcast");
    println!("  network:        {}", network.as_str());
    println!("  fqdn:           {}", args.fqdn);
    println!("  score:          {} (from {} events)", envelope.score, envelope.event_count);
    println!("  signer:         {signer_address:?}");
    println!("  resolver:       0x{}", hex::encode(resolver));
    println!("  text key:       {REPUTATION_TEXT_KEY}");
    println!("  rpc:            {}", redact_rpc(&rpc_url));

    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse().expect("validated above"));

    let chain_id = match provider.get_chain_id().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --broadcast: eth_chainId failed: {e}");
            return ExitCode::from(1);
        }
    };
    println!("  chain_id:       {chain_id}");

    let resolver_addr: Address = resolver.into();
    let tx = TransactionRequest::default()
        .with_to(resolver_addr)
        .with_input(Bytes::from(calldata))
        .with_chain_id(chain_id);

    println!(
        "  → tx: setText({REPUTATION_TEXT_KEY}, \"{}\") → resolver…",
        envelope.score
    );
    let pending = match provider.send_transaction(tx).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --broadcast: tx send failed: {e}");
            return ExitCode::from(1);
        }
    };
    let tx_hash = *pending.tx_hash();
    println!("    tx_hash:      {tx_hash:?}");
    println!("    explorer:     {}", explorer_url(network, &tx_hash));
    let receipt = match pending.with_required_confirmations(1).get_receipt().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --broadcast: tx confirmation failed: {e}");
            return ExitCode::from(1);
        }
    };
    if !receipt.status() {
        eprintln!("sbo3l agent reputation-publish --broadcast: tx reverted on chain");
        return ExitCode::from(1);
    }
    println!(
        "    confirmed:    block {} gas_used={}",
        receipt.block_number.unwrap_or(0),
        receipt.gas_used
    );

    println!("---");
    println!("published:     {} → {} on {}", REPUTATION_TEXT_KEY, envelope.score, network.as_str());
    println!("verify:        sbo3l agent verify-ens {} --network {}", args.fqdn, network.as_str());
    ExitCode::SUCCESS
}

fn resolve_rpc_url(args: &ReputationPublishArgs) -> Result<String, ExitCode> {
    if let Some(s) = args.rpc_url.as_deref() {
        return validate_url(s);
    }
    match std::env::var(DEFAULT_RPC_ENV) {
        Ok(s) => validate_url(&s),
        Err(_) => {
            eprintln!(
                "sbo3l agent reputation-publish --broadcast: pass --rpc-url <url> or set {DEFAULT_RPC_ENV} \
                 (Sepolia Alchemy / Infura / PublicNode)."
            );
            Err(ExitCode::from(2))
        }
    }
}

fn validate_url(s: &str) -> Result<String, ExitCode> {
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        eprintln!(
            "sbo3l agent reputation-publish --broadcast: rpc url must be http:// or https://; got `{s}`"
        );
        return Err(ExitCode::from(2));
    }
    Ok(s.to_string())
}

fn load_signer(env: &str) -> Result<PrivateKeySigner, ExitCode> {
    let raw = match std::env::var(env) {
        Ok(s) => s,
        Err(_) => {
            eprintln!(
                "sbo3l agent reputation-publish --broadcast: signer env var `{env}` not set. \
                 Export 32-byte hex private key (0x-prefixed or bare)."
            );
            return Err(ExitCode::from(2));
        }
    };
    let stripped = raw.trim().trim_start_matches("0x");
    let bytes = match hex::decode(stripped) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l agent reputation-publish --broadcast: signer key not valid hex: {e}");
            return Err(ExitCode::from(2));
        }
    };
    if bytes.len() != 32 {
        eprintln!(
            "sbo3l agent reputation-publish --broadcast: signer key must be 32 bytes; got {}",
            bytes.len()
        );
        return Err(ExitCode::from(2));
    }
    let arr: [u8; 32] = bytes.try_into().expect("len checked");
    PrivateKeySigner::from_bytes(&FixedBytes::from(arr)).map_err(|e| {
        eprintln!("sbo3l agent reputation-publish --broadcast: signer construction failed: {e}");
        ExitCode::from(2)
    })
}

fn parse_address_str(s: &str, label: &str) -> Result<[u8; 20], ExitCode> {
    let stripped = s.trim().trim_start_matches("0x");
    if stripped.len() != 40 {
        eprintln!(
            "sbo3l agent reputation-publish --broadcast: {label} address must be 0x + 40 hex; got `{s}`"
        );
        return Err(ExitCode::from(2));
    }
    let bytes = match hex::decode(stripped) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "sbo3l agent reputation-publish --broadcast: {label} address not hex: {e}"
            );
            return Err(ExitCode::from(2));
        }
    };
    bytes.try_into().map_err(|_| {
        eprintln!("sbo3l agent reputation-publish --broadcast: {label} address wrong length");
        ExitCode::from(2)
    })
}

fn explorer_url(network: EnsNetwork, tx_hash: &FixedBytes<32>) -> String {
    match network {
        EnsNetwork::Mainnet => format!("https://etherscan.io/tx/{tx_hash:?}"),
        EnsNetwork::Sepolia => format!("https://sepolia.etherscan.io/tx/{tx_hash:?}"),
    }
}

fn redact_rpc(url: &str) -> String {
    let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
    let after_scheme = &url[scheme_end..];
    let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    format!(
        "{}{}/<redacted>",
        &url[..scheme_end],
        &after_scheme[..host_end]
    )
}

fn current_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}
