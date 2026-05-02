//! T-3-1 broadcast — `sbo3l agent register --broadcast` real path.
//!
//! Compiled only with `--features eth_broadcast`. Without the feature
//! the CLI still parses `--broadcast` and prints a clear "rebuild
//! with --features eth_broadcast" error from
//! [`crate::agent::broadcast_not_available`], preserving the pre-1.0.1
//! exit code 3 contract.
//!
//! Pivoted from Durin's gateway model (PR #116) to canonical ENS
//! Registry calls. Daniel owns `sbo3lagent.eth` on mainnet so calling
//! `Registry.setSubnodeRecord` directly issues `<name>.sbo3lagent.eth`
//! without any third-party registrar contract.
//!
//! # Wire
//!
//! Two transactions, in order:
//!
//! 1. `Registry.setSubnodeRecord(parent_node, label_hash, owner, resolver, ttl=0)`
//!    → recipient: [`sbo3l_identity::ens_live::ENS_REGISTRY_ADDRESS`].
//!    Issues the subname and sets owner + resolver in one tx.
//! 2. `Resolver.multicall(setText × N)` → recipient: the public
//!    resolver address (from
//!    [`sbo3l_identity::EnsNetwork::default_public_resolver`]).
//!    Sets every `sbo3l:*` text record in one tx.
//!
//! Each tx waits exactly one confirmation before the next is sent —
//! the second depends on the first having committed (the resolver
//! reads `setText` from the storage slot the registry just wrote).
//!
//! # Env vars
//!
//! * `SBO3L_RPC_URL` — JSON-RPC endpoint. Supports HTTP/HTTPS. The
//!   network tag (`--network sepolia` | `mainnet`) selects only the
//!   default resolver address; the RPC URL is authoritative for the
//!   actual chain ID.
//! * `SBO3L_SIGNER_KEY` — 32-byte hex private key (`0x`-prefixed or
//!   bare). Default env var name; override with `--private-key-env-var`.

use std::process::ExitCode;

use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::{Address, Bytes, FixedBytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use sbo3l_identity::durin::multicall_calldata;
use sbo3l_identity::ens_anchor::{
    label_hash, namehash, set_subnode_record_calldata, set_text_calldata,
};
use sbo3l_identity::ens_live::ENS_REGISTRY_ADDRESS;
use sbo3l_identity::EnsNetwork;

use crate::agent::AgentRegisterArgs;

const DEFAULT_SIGNER_ENV: &str = "SBO3L_SIGNER_KEY";
const DEFAULT_RPC_ENV: &str = "SBO3L_RPC_URL";

pub async fn cmd_broadcast(args: AgentRegisterArgs, network: EnsNetwork) -> ExitCode {
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

    // Owner defaults to the signer's address — broadcast is the one
    // case where the dry-run-required `--owner` becomes optional,
    // because we know the signer's address authoritatively.
    let owner_str = args
        .owner
        .clone()
        .unwrap_or_else(|| format!("{signer_address:?}"));
    let owner = match parse_address_str(&owner_str, "owner") {
        Ok(a) => a,
        Err(rc) => return rc,
    };

    let resolver_str = args
        .resolver
        .clone()
        .unwrap_or_else(|| network.default_public_resolver().to_string());
    let resolver = match parse_address_str(&resolver_str, "resolver") {
        Ok(a) => a,
        Err(rc) => return rc,
    };

    let parent_node = match namehash(&args.parent) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: bad --parent: {e}");
            return ExitCode::from(2);
        }
    };
    if args.name.contains('.') || args.name.is_empty() {
        eprintln!("sbo3l agent register --broadcast: --name must be a single non-empty DNS label (no `.`)");
        return ExitCode::from(2);
    }
    let label = label_hash(&args.name);

    let records = match super::agent::parse_records_pub(&args.records_json) {
        Ok(r) => r,
        Err(rc) => return rc,
    };

    // The FQDN namehash equals `keccak256(parent_node || label_hash)`
    // by EIP-137 recursion — same value `namehash(<fqdn>)` produces.
    // Calling namehash directly avoids re-implementing keccak here.
    let fqdn_str = format!("{}.{}", args.name, args.parent);
    let fqdn_namehash = match namehash(&fqdn_str) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: fqdn namehash failed: {e}");
            return ExitCode::from(2);
        }
    };

    let set_text_calls: Vec<Vec<u8>> = records
        .iter()
        .map(|(k, v)| set_text_calldata(fqdn_namehash, k, v))
        .collect();
    let multicall = multicall_calldata(&set_text_calls);

    let registry_calldata = set_subnode_record_calldata(parent_node, label, owner, resolver, 0);

    let fqdn = fqdn_str;
    println!("sbo3l agent register --broadcast");
    println!("  network:        {}", network.as_str());
    println!("  fqdn:           {fqdn}");
    println!("  signer:         {signer_address:?}");
    println!("  owner:          0x{}", hex::encode(owner));
    println!("  resolver:       0x{}", hex::encode(resolver));
    println!("  rpc:            {}", redact_rpc(&rpc_url));

    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse().expect("validated above"));

    let chain_id = match provider.get_chain_id().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: eth_chainId failed: {e}");
            return ExitCode::from(1);
        }
    };
    println!("  chain_id:       {chain_id}");

    // Tx 1 — Registry.setSubnodeRecord
    let registry_addr: Address = ENS_REGISTRY_ADDRESS.parse().expect("constant address");
    let tx1 = TransactionRequest::default()
        .with_to(registry_addr)
        .with_input(Bytes::from(registry_calldata))
        .with_chain_id(chain_id);

    println!("  → tx1: setSubnodeRecord → ENS Registry ({ENS_REGISTRY_ADDRESS})…");
    let pending1 = match provider.send_transaction(tx1).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: tx1 send failed: {e}");
            return ExitCode::from(1);
        }
    };
    let tx1_hash = *pending1.tx_hash();
    println!("    tx_hash:      {tx1_hash:?}");
    println!("    explorer:     {}", explorer_url(network, &tx1_hash));
    let receipt1 = match pending1.with_required_confirmations(1).get_receipt().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: tx1 confirmation failed: {e}");
            return ExitCode::from(1);
        }
    };
    if !receipt1.status() {
        eprintln!("sbo3l agent register --broadcast: tx1 reverted on chain");
        return ExitCode::from(1);
    }
    println!(
        "    confirmed:    block {} gas_used={}",
        receipt1.block_number.unwrap_or(0),
        receipt1.gas_used
    );

    // Tx 2 — Resolver.multicall(setText × N)
    let resolver_addr: Address = resolver.into();
    let tx2 = TransactionRequest::default()
        .with_to(resolver_addr)
        .with_input(Bytes::from(multicall))
        .with_chain_id(chain_id);

    println!(
        "  → tx2: multicall(setText x {}) → resolver…",
        set_text_calls.len()
    );
    let pending2 = match provider.send_transaction(tx2).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: tx2 send failed: {e}");
            return ExitCode::from(1);
        }
    };
    let tx2_hash = *pending2.tx_hash();
    println!("    tx_hash:      {tx2_hash:?}");
    println!("    explorer:     {}", explorer_url(network, &tx2_hash));
    let receipt2 = match pending2.with_required_confirmations(1).get_receipt().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: tx2 confirmation failed: {e}");
            return ExitCode::from(1);
        }
    };
    if !receipt2.status() {
        eprintln!("sbo3l agent register --broadcast: tx2 reverted on chain");
        return ExitCode::from(1);
    }
    println!(
        "    confirmed:    block {} gas_used={}",
        receipt2.block_number.unwrap_or(0),
        receipt2.gas_used
    );

    println!("---");
    println!("registered:    {fqdn}");
    println!("subname owner: 0x{}", hex::encode(owner));
    ExitCode::SUCCESS
}

fn resolve_rpc_url(args: &AgentRegisterArgs) -> Result<String, ExitCode> {
    if let Some(s) = args.rpc_url.as_deref() {
        return validate_url(s);
    }
    match std::env::var(DEFAULT_RPC_ENV) {
        Ok(s) => validate_url(&s),
        Err(_) => {
            eprintln!(
                "sbo3l agent register --broadcast: pass --rpc-url <url> or set {DEFAULT_RPC_ENV} \
                 (Sepolia Alchemy / Infura / PublicNode)."
            );
            Err(ExitCode::from(2))
        }
    }
}

fn validate_url(s: &str) -> Result<String, ExitCode> {
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        eprintln!(
            "sbo3l agent register --broadcast: rpc url must be http:// or https://; got `{s}`"
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
                "sbo3l agent register --broadcast: signer env var `{env}` not set. \
                 Export 32-byte hex private key (0x-prefixed or bare)."
            );
            return Err(ExitCode::from(2));
        }
    };
    let stripped = raw.trim().trim_start_matches("0x");
    let bytes = match hex::decode(stripped) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: signer key not valid hex: {e}");
            return Err(ExitCode::from(2));
        }
    };
    if bytes.len() != 32 {
        eprintln!(
            "sbo3l agent register --broadcast: signer key must be 32 bytes; got {}",
            bytes.len()
        );
        return Err(ExitCode::from(2));
    }
    let arr: [u8; 32] = bytes.try_into().expect("len checked");
    PrivateKeySigner::from_bytes(&FixedBytes::from(arr)).map_err(|e| {
        eprintln!("sbo3l agent register --broadcast: signer construction failed: {e}");
        ExitCode::from(2)
    })
}

fn parse_address_str(s: &str, label: &str) -> Result<[u8; 20], ExitCode> {
    let stripped = s.trim().trim_start_matches("0x");
    if stripped.len() != 40 {
        eprintln!(
            "sbo3l agent register --broadcast: {label} address must be 0x + 40 hex; got `{s}`"
        );
        return Err(ExitCode::from(2));
    }
    let bytes = match hex::decode(stripped) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l agent register --broadcast: {label} address not hex: {e}");
            return Err(ExitCode::from(2));
        }
    };
    bytes.try_into().map_err(|_| {
        eprintln!("sbo3l agent register --broadcast: {label} address wrong length");
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
    // Alchemy / Infura URLs embed the API key in the path. Print
    // scheme://host (truncate path) so the operator can confirm the
    // right network without leaking the key into stdout / shared
    // logs. Avoids a `url` crate dep — string-only parse is fine
    // since we already validated the http/https prefix.
    let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
    let after_scheme = &url[scheme_end..];
    let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    format!(
        "{}{}/<redacted>",
        &url[..scheme_end],
        &after_scheme[..host_end]
    )
}
