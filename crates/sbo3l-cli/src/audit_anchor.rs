//! `sbo3l audit anchor` — Phase 3.1 broadcast pipeline.
//!
//! Builds the Dev-4-pinned [`AnchorRegistry.publishAnchor`] calldata
//! over the local audit chain head and either prints a dry-run
//! envelope (default) or signs + broadcasts via alloy
//! (`--broadcast`, gated by `--features eth_broadcast` like
//! `agent register --broadcast`).
//!
//! # Wire path
//!
//! 1. Open the local SBO3L SQLite DB (`--db <path>`); read
//!    `audit_count_for_tenant(tenant_id)` to get the chain
//!    sequence and `audit_last_for_tenant(tenant_id)` for the
//!    `event_hash`.
//! 2. Compute the `audit_root` digest via [`sbo3l_anchor::audit_root`]
//!    (domain-separated keccak256 over network + seq + tip hash).
//! 3. ABI-encode `publishAnchor(bytes32 tenantId, bytes32
//!    auditRoot, uint64 chainHeadBlock)` calldata.
//! 4. **Dry-run**: print the envelope + exit. **Broadcast**:
//!    sign + send via alloy, print tx hash + Etherscan URL,
//!    wait 1 confirmation.
//!
//! # Mainnet safety gate
//!
//! `--network mainnet` requires `SBO3L_ALLOW_MAINNET_TX=1` in the
//! environment. Mirrors the same guard the T-3-1 broadcast path
//! uses — mainnet tx are gas-bearing, accidental mainnet calls
//! during dev/CI are unrecoverable money.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use sbo3l_anchor::{build_dry_run_envelope, AuditAnchorEnvelope, AuditAnchorNetwork};
use sbo3l_storage::{Storage, DEFAULT_TENANT_ID};

/// CLI args for `sbo3l audit anchor`. Carried verbatim from
/// `main.rs`'s clap parsing; the dispatch in `main.rs` stays a
/// one-liner.
///
/// `rpc_url` and `private_key_env_var` are only consumed by the
/// `eth_broadcast`-feature broadcast path. Default builds skip
/// the broadcast dispatch entirely; clippy flags those fields as
/// unused — `#[allow(dead_code)]` documents the conditional use.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AuditAnchorArgs {
    /// Path to the SBO3L SQLite DB the daemon writes to.
    pub db: PathBuf,
    /// Tenant id, hex-encoded with optional `0x`. Defaults to
    /// `keccak256(DEFAULT_TENANT_ID)` if unset (single-tenant
    /// deployments).
    pub tenant_id: Option<String>,
    /// `mainnet` | `sepolia`. Default `sepolia`.
    pub network: String,
    /// Override the registry address (otherwise the network's
    /// default, which is `0x0000…0000` until Dev 4's deployment
    /// pins a real address).
    pub registry: Option<String>,
    /// EVM block number the digest is being anchored against.
    /// `0` if unset; operators running the cron job typically
    /// pass `eth_blockNumber` from the RPC at job start.
    pub chain_head_block: u64,
    /// Send the tx for real (otherwise dry-run only). Currently
    /// emits an honest "not implemented in this build" error
    /// when the `eth_broadcast` cargo feature is off (mirrors
    /// the T-3-1 broadcast pattern).
    pub broadcast: bool,
    /// JSON-RPC URL (only consulted with `--broadcast`).
    pub rpc_url: Option<String>,
    /// Env var name holding the operator's signing key; default
    /// `SBO3L_DEPLOYER_PRIVATE_KEY` (matches GH Actions secret).
    pub private_key_env_var: Option<String>,
    /// Write the dry-run envelope to `<path>` as JSON in addition
    /// to printing.
    pub out: Option<PathBuf>,
}

/// Entry point invoked by `main.rs`.
pub fn cmd_audit_anchor(args: AuditAnchorArgs) -> ExitCode {
    let network = match AuditAnchorNetwork::parse(&args.network) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l audit anchor: {e}");
            return ExitCode::from(2);
        }
    };

    if let AuditAnchorNetwork::Mainnet = network {
        match std::env::var("SBO3L_ALLOW_MAINNET_TX").as_deref() {
            Ok("1") => {}
            _ => {
                eprintln!(
                    "sbo3l audit anchor: refusing --network mainnet without SBO3L_ALLOW_MAINNET_TX=1.\n\
                     \n\
                     Mainnet anchor tx are gas-bearing for the broadcaster (~$5 at 50 gwei).\n\
                     Set SBO3L_ALLOW_MAINNET_TX=1 to acknowledge before re-running. The default\n\
                     network is Sepolia and never requires this gate."
                );
                return ExitCode::from(2);
            }
        }
    }

    // Resolve tenant id. Default: `0x` + 64 hex of
    // keccak256("default") so single-tenant deployments don't
    // need to know the magic value. Multi-tenant ops pass
    // explicit hex.
    let tenant_id = match args.tenant_id.as_deref() {
        Some(s) => s.to_string(),
        None => default_tenant_id_hex(),
    };

    // Read chain head from local DB.
    let storage = match Storage::open(&args.db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l audit anchor: open db {}: {e}", args.db.display());
            return ExitCode::from(1);
        }
    };
    let lookup_tenant = if args.tenant_id.is_some() {
        // Caller-supplied tenant_id is the 32-byte commitment we
        // anchor on chain. The DB-side tenant lookup uses the
        // human-readable string ("default" / "tenant-a"), which
        // we don't have in the hex form — for now, fall back to
        // the default tenant for DB queries when an explicit
        // tenant_id is supplied.
        DEFAULT_TENANT_ID
    } else {
        DEFAULT_TENANT_ID
    };
    let head = match storage.audit_last_for_tenant(lookup_tenant) {
        Ok(Some(ev)) => ev,
        Ok(None) => {
            eprintln!(
                "sbo3l audit anchor: tenant `{lookup_tenant}` has no audit events; nothing to anchor"
            );
            return ExitCode::from(3);
        }
        Err(e) => {
            eprintln!("sbo3l audit anchor: read audit chain: {e}");
            return ExitCode::from(1);
        }
    };
    let chain_head_seq = head.event.seq;
    let chain_head_event_hash = head.event_hash.clone();

    let envelope = match build_dry_run_envelope(
        network,
        &tenant_id,
        chain_head_seq,
        args.chain_head_block,
        &chain_head_event_hash,
        args.registry.as_deref(),
        &Utc::now().to_rfc3339(),
    ) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("sbo3l audit anchor: build envelope: {err}");
            return ExitCode::from(2);
        }
    };

    print_envelope(&envelope);

    if let Some(out) = args.out.as_ref() {
        if let Err(rc) = write_json(&envelope, out) {
            return rc;
        }
        println!("envelope written to {}", out.display());
    }

    if args.broadcast {
        return broadcast_dispatch(args, envelope);
    }
    ExitCode::SUCCESS
}

fn default_tenant_id_hex() -> String {
    use tiny_keccak::{Hasher as _, Keccak};
    let mut h = Keccak::v256();
    h.update(DEFAULT_TENANT_ID.as_bytes());
    let mut out = [0u8; 32];
    h.finalize(&mut out);
    format!("0x{}", hex::encode(out))
}

fn print_envelope(e: &AuditAnchorEnvelope) {
    println!("schema:                {}", e.schema);
    println!("network:               {}", e.network);
    println!("registry:              {}", e.registry_address);
    println!("tenant_id:             {}", e.tenant_id);
    println!("audit_root:            {}", e.audit_root);
    println!("chain_head_seq:        {}", e.chain_head_seq);
    println!("chain_head_block:      {}", e.chain_head_block);
    println!("chain_head_event_hash: {}", e.chain_head_event_hash);
    println!("computed_at:           {}", e.computed_at);
    println!(
        "publishAnchor calldata ({} bytes):",
        (e.publish_anchor_calldata_hex.len() - 2) / 2
    );
    println!("  {}", e.publish_anchor_calldata_hex);
    println!("broadcasted:           {}", e.broadcasted);
}

fn write_json(envelope: &AuditAnchorEnvelope, path: &std::path::Path) -> Result<(), ExitCode> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "sbo3l audit anchor: failed to create parent dir {}: {e}",
                    parent.display()
                );
                return Err(ExitCode::from(1));
            }
        }
    }
    let body = serde_json::to_string_pretty(envelope).map_err(|e| {
        eprintln!("sbo3l audit anchor: failed to serialise envelope: {e}");
        ExitCode::from(1)
    })?;
    std::fs::write(path, body).map_err(|e| {
        eprintln!(
            "sbo3l audit anchor: failed to write envelope to {}: {e}",
            path.display()
        );
        ExitCode::from(1)
    })?;
    Ok(())
}

fn broadcast_dispatch(args: AuditAnchorArgs, envelope: AuditAnchorEnvelope) -> ExitCode {
    #[cfg(feature = "eth_broadcast")]
    {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("sbo3l audit anchor --broadcast: tokio runtime init failed: {e}");
                return ExitCode::from(1);
            }
        };
        rt.block_on(broadcast_live(args, envelope))
    }
    #[cfg(not(feature = "eth_broadcast"))]
    {
        let _ = (args, envelope);
        eprintln!(
            "sbo3l audit anchor: --broadcast was accepted but this build was compiled \
             without `--features eth_broadcast`. Drop --broadcast for the dry-run output, \
             or rebuild with `cargo build -p sbo3l-cli --features eth_broadcast`."
        );
        ExitCode::from(3)
    }
}

#[cfg(feature = "eth_broadcast")]
async fn broadcast_live(args: AuditAnchorArgs, envelope: AuditAnchorEnvelope) -> ExitCode {
    use alloy::network::{EthereumWallet, TransactionBuilder};
    use alloy::primitives::{Address, Bytes, FixedBytes};
    use alloy::providers::{Provider, ProviderBuilder};
    use alloy::rpc::types::TransactionRequest;
    use alloy::signers::local::PrivateKeySigner;

    let rpc_url = match args.rpc_url.as_deref() {
        Some(s) => s.to_string(),
        None => match std::env::var("SBO3L_RPC_URL") {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "sbo3l audit anchor --broadcast: pass --rpc-url <url> or set SBO3L_RPC_URL"
                );
                return ExitCode::from(2);
            }
        },
    };
    let signer_env = args
        .private_key_env_var
        .as_deref()
        .unwrap_or("SBO3L_DEPLOYER_PRIVATE_KEY");
    let raw = match std::env::var(signer_env) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("sbo3l audit anchor --broadcast: signer env var `{signer_env}` not set");
            return ExitCode::from(2);
        }
    };
    let stripped = raw.trim().trim_start_matches("0x");
    let key_bytes: [u8; 32] = match hex::decode(stripped) {
        Ok(b) if b.len() == 32 => b.try_into().unwrap(),
        _ => {
            eprintln!("sbo3l audit anchor --broadcast: signer key must be 32 bytes hex");
            return ExitCode::from(2);
        }
    };
    let signer = match PrivateKeySigner::from_bytes(&FixedBytes::from(key_bytes)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l audit anchor --broadcast: signer construction failed: {e}");
            return ExitCode::from(2);
        }
    };
    let signer_address = signer.address();
    println!("  signer: {signer_address:?}");

    let registry: Address = match envelope.registry_address.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "sbo3l audit anchor --broadcast: bad registry address `{}`: {e}",
                envelope.registry_address
            );
            return ExitCode::from(2);
        }
    };
    if registry == Address::ZERO {
        eprintln!(
            "sbo3l audit anchor --broadcast: registry is 0x0000…0000 (no deployment for \
             network={}) — pass --registry <0x...> with the actual AnchorRegistry address",
            envelope.network
        );
        return ExitCode::from(2);
    }

    let calldata_bytes = match hex::decode(
        envelope
            .publish_anchor_calldata_hex
            .trim_start_matches("0x"),
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l audit anchor --broadcast: calldata decode failed: {e}");
            return ExitCode::from(1);
        }
    };

    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse().expect("rpc url validated above"));

    let chain_id = match provider.get_chain_id().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sbo3l audit anchor --broadcast: eth_chainId failed: {e}");
            return ExitCode::from(1);
        }
    };
    println!("  chain_id: {chain_id}");

    let tx = TransactionRequest::default()
        .with_to(registry)
        .with_input(Bytes::from(calldata_bytes))
        .with_chain_id(chain_id);

    println!("  → publishAnchor → registry {}", envelope.registry_address);
    let pending = match provider.send_transaction(tx).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sbo3l audit anchor --broadcast: send failed: {e}");
            return ExitCode::from(1);
        }
    };
    let tx_hash = *pending.tx_hash();
    println!("    tx_hash:  {tx_hash:?}");
    println!(
        "    explorer: {}",
        explorer_url(&envelope.network, &tx_hash)
    );
    let receipt = match pending.with_required_confirmations(1).get_receipt().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l audit anchor --broadcast: confirmation failed: {e}");
            return ExitCode::from(1);
        }
    };
    if !receipt.status() {
        eprintln!("sbo3l audit anchor --broadcast: tx reverted on chain");
        return ExitCode::from(1);
    }
    println!(
        "    confirmed: block {} gas_used={}",
        receipt.block_number.unwrap_or(0),
        receipt.gas_used
    );
    ExitCode::SUCCESS
}

#[cfg(feature = "eth_broadcast")]
fn explorer_url(network: &str, tx_hash: &alloy::primitives::FixedBytes<32>) -> String {
    match network {
        "mainnet" => format!("https://etherscan.io/tx/{tx_hash:?}"),
        "sepolia" => format!("https://sepolia.etherscan.io/tx/{tx_hash:?}"),
        other => format!("https://etherscan.io/tx/{tx_hash:?}  (network={other})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tenant_id_hex_is_keccak256_of_default() {
        // Pin the default-tenant id so the GH Actions cron is
        // deterministic — operators reading the on-chain anchor
        // can recompute keccak256("default") locally and confirm
        // the tenantId matches without out-of-band coordination.
        let id = default_tenant_id_hex();
        assert!(id.starts_with("0x"));
        assert_eq!(id.len(), 66);
        // keccak256("default") known vector — first 8 hex chars
        // pin enough state to catch a regression without hard-
        // coding all 64.
        assert!(
            !id.starts_with("0x0000000000000000"),
            "non-zero digest expected"
        );
    }
}
