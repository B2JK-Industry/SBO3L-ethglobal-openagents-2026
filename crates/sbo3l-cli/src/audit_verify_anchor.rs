//! `sbo3l audit verify-anchor <tx>` — read-side anchor verifier.
//!
//! Read-side counterpart to [`crate::audit_anchor`]. Given an
//! Ethereum tx hash that broadcast a `publishAnchor` call to Dev 4's
//! AnchorRegistry, this command:
//!
//! 1. Fetches the tx via JSON-RPC `eth_getTransactionByHash`.
//! 2. Decodes the 4-byte selector + 3 × 32-byte ABI args from the
//!    tx's `input` field.
//! 3. Asserts the selector matches
//!    [`sbo3l_anchor::PUBLISH_ANCHOR_SELECTOR`] (`0xa212dc0a`).
//! 4. Extracts `(tenant_id, audit_root, chain_head_block)` from
//!    the decoded calldata.
//! 5. Re-computes the local audit root via
//!    [`sbo3l_anchor::audit_root`] over the chain head in the
//!    supplied `--db`.
//! 6. Compares the two roots and prints `✅ verified` or
//!    `❌ mismatch` with a diff.
//!
//! # Wire path — what the verifier needs to trust
//!
//! - The Ethereum RPC endpoint (read-only — no signer).
//! - The local SBO3L SQLite DB (must contain the chain head the tx
//!   anchored). For the canonical demo this is the same DB the
//!   broadcast was issued against.
//!
//! No private keys, no broadcast — this is a pure read + recompute
//! flow. Safe to run from any judge's terminal against any public
//! Sepolia RPC.

use std::path::PathBuf;
use std::process::ExitCode;

use sbo3l_anchor::{audit_root, AuditAnchorNetwork, PUBLISH_ANCHOR_SELECTOR};
use sbo3l_storage::{Storage, DEFAULT_TENANT_ID};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VerifyAnchorArgs {
    /// 0x-prefixed Ethereum tx hash (66 chars including `0x`).
    pub tx_hash: String,
    /// `mainnet` | `sepolia`. Default `sepolia`.
    pub network: String,
    /// Local SBO3L SQLite DB to recompute the audit root from.
    pub db: PathBuf,
    /// JSON-RPC URL. Falls back to `SBO3L_RPC_URL` env, else a
    /// well-known public Sepolia endpoint as a last resort.
    pub rpc_url: Option<String>,
}

/// JSON-RPC request body shape — minimal hand-roll so we don't
/// pull a JSON-RPC client crate.
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

/// `eth_getTransactionByHash` result shape — only the fields we
/// consume.
#[derive(Deserialize, Debug, Default)]
struct EthTransaction {
    /// Hex-encoded `0x...` calldata (the function selector + ABI
    /// args concatenated).
    #[serde(default)]
    input: Option<String>,
    /// Recipient — should equal the AnchorRegistry contract.
    #[serde(default)]
    to: Option<String>,
    /// Block number this tx was mined in (hex). Useful for
    /// reporting; not used in verification.
    #[serde(default, rename = "blockNumber")]
    block_number: Option<String>,
}

pub fn cmd_audit_verify_anchor(args: VerifyAnchorArgs) -> ExitCode {
    let network = match AuditAnchorNetwork::parse(&args.network) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l audit verify-anchor: {e}");
            return ExitCode::from(2);
        }
    };

    if !looks_like_tx_hash(&args.tx_hash) {
        eprintln!(
            "sbo3l audit verify-anchor: tx_hash must be 0x + 64 hex chars; got `{}`",
            args.tx_hash
        );
        return ExitCode::from(2);
    }

    let rpc_url = match resolve_rpc_url(&args, network) {
        Ok(s) => s,
        Err(rc) => return rc,
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l audit verify-anchor: tokio runtime init failed: {e}");
            return ExitCode::from(1);
        }
    };

    let tx = match runtime.block_on(fetch_tx(&rpc_url, &args.tx_hash)) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sbo3l audit verify-anchor: fetch tx: {e}");
            return ExitCode::from(1);
        }
    };

    let input = match tx.input.as_deref() {
        Some(s) if !s.is_empty() && s != "0x" => s,
        _ => {
            eprintln!(
                "sbo3l audit verify-anchor: tx has no calldata (transfer or non-contract call)"
            );
            return ExitCode::from(1);
        }
    };

    let decoded = match decode_publish_anchor(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("sbo3l audit verify-anchor: decode calldata: {e}");
            return ExitCode::from(1);
        }
    };

    println!("┌──────────────────────────────────────────────────────");
    println!("│  sbo3l audit verify-anchor");
    println!("├──────────────────────────────────────────────────────");
    println!("│  tx_hash:           {}", args.tx_hash);
    println!("│  network:           {}", network.as_str());
    if let Some(b) = tx.block_number.as_deref() {
        println!("│  mined in block:    {b}");
    }
    if let Some(to) = tx.to.as_deref() {
        println!("│  contract:          {to}");
    }
    println!("│  selector:          0x{}", hex::encode(decoded.selector));
    println!("│  → publishAnchor(bytes32,bytes32,uint64) ✅");
    println!("│  on-chain tenant:   0x{}", hex::encode(decoded.tenant_id));
    println!(
        "│  on-chain root:     0x{}",
        hex::encode(decoded.audit_root)
    );
    println!("│  on-chain block:    {}", decoded.chain_head_block);

    // Recompute local audit_root.
    let local_root_hex = match recompute_local_root(&args.db, network.as_str()) {
        Ok(h) => h,
        Err(rc) => return rc,
    };
    println!("│  local root:        {}", local_root_hex);

    // Comparison — both are 32-byte values; on-chain is bytes,
    // local is `0x` + 64 hex.
    let local_stripped = local_root_hex.trim_start_matches("0x");
    let mut local_bytes = [0u8; 32];
    if hex::decode_to_slice(local_stripped, &mut local_bytes).is_err() {
        eprintln!("sbo3l audit verify-anchor: local root not valid 32-byte hex");
        return ExitCode::from(1);
    }

    if local_bytes == decoded.audit_root {
        println!("├──────────────────────────────────────────────────────");
        println!("│  ✅ VERIFIED — on-chain root matches local recomputation");
        println!("└──────────────────────────────────────────────────────");
        ExitCode::SUCCESS
    } else {
        println!("├──────────────────────────────────────────────────────");
        println!("│  ❌ MISMATCH — on-chain root disagrees with local DB");
        println!("│  on-chain:  0x{}", hex::encode(decoded.audit_root));
        println!("│  local:     0x{}", hex::encode(local_bytes));
        println!("│");
        println!("│  Possible causes:");
        println!("│   - Local DB has been mutated since the anchor tx was sent");
        println!("│   - The tx anchored a DIFFERENT chain head (different seq) than current");
        println!("│   - Verifier ran against the wrong network or wrong DB");
        println!("└──────────────────────────────────────────────────────");
        ExitCode::from(1)
    }
}

/// `(selector, tenant_id, audit_root, chain_head_block)` decoded
/// from a `publishAnchor` calldata blob.
#[derive(Debug)]
struct DecodedPublishAnchor {
    selector: [u8; 4],
    tenant_id: [u8; 32],
    audit_root: [u8; 32],
    chain_head_block: u64,
}

fn decode_publish_anchor(input_hex: &str) -> Result<DecodedPublishAnchor, String> {
    let stripped = input_hex.strip_prefix("0x").unwrap_or(input_hex);
    let raw = hex::decode(stripped).map_err(|e| format!("hex decode: {e}"))?;
    if raw.len() != 4 + 32 + 32 + 32 {
        return Err(format!(
            "calldata length is {} bytes; expected 100 (4 selector + 3 × 32 args)",
            raw.len()
        ));
    }
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&raw[..4]);
    if selector != PUBLISH_ANCHOR_SELECTOR {
        return Err(format!(
            "selector is 0x{} but expected 0x{} (publishAnchor); tx targets a different function",
            hex::encode(selector),
            hex::encode(PUBLISH_ANCHOR_SELECTOR),
        ));
    }
    let mut tenant_id = [0u8; 32];
    tenant_id.copy_from_slice(&raw[4..36]);
    let mut audit_root = [0u8; 32];
    audit_root.copy_from_slice(&raw[36..68]);
    // uint64 lives in the LOW 8 bytes of a 32-byte word (big-endian
    // padding fills bytes 68..92 with zeros).
    let mut block_word = [0u8; 8];
    block_word.copy_from_slice(&raw[92..100]);
    let chain_head_block = u64::from_be_bytes(block_word);
    Ok(DecodedPublishAnchor {
        selector,
        tenant_id,
        audit_root,
        chain_head_block,
    })
}

fn recompute_local_root(db_path: &std::path::Path, network: &str) -> Result<String, ExitCode> {
    let storage = Storage::open(db_path).map_err(|e| {
        eprintln!(
            "sbo3l audit verify-anchor: open db {}: {e}",
            db_path.display()
        );
        ExitCode::from(1)
    })?;
    let head = storage
        .audit_last_for_tenant(DEFAULT_TENANT_ID)
        .map_err(|e| {
            eprintln!("sbo3l audit verify-anchor: read audit chain: {e}");
            ExitCode::from(1)
        })?
        .ok_or_else(|| {
            eprintln!(
                "sbo3l audit verify-anchor: tenant `{DEFAULT_TENANT_ID}` has no audit events"
            );
            ExitCode::from(3)
        })?;
    audit_root(network, head.event.seq, &head.event_hash).map_err(|e| {
        eprintln!("sbo3l audit verify-anchor: recompute root: {e}");
        ExitCode::from(1)
    })
}

fn looks_like_tx_hash(s: &str) -> bool {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    stripped.len() == 64 && stripped.chars().all(|c| c.is_ascii_hexdigit())
}

fn resolve_rpc_url(
    args: &VerifyAnchorArgs,
    network: AuditAnchorNetwork,
) -> Result<String, ExitCode> {
    if let Some(s) = args.rpc_url.as_deref() {
        return Ok(s.to_string());
    }
    if let Ok(s) = std::env::var("SBO3L_RPC_URL") {
        if !s.is_empty() {
            return Ok(s);
        }
    }
    // Last-resort public RPC. PublicNode is documented as a
    // working read-side endpoint per `live_rpc_endpoints_known.md`.
    let fallback = match network {
        AuditAnchorNetwork::Mainnet => "https://ethereum-rpc.publicnode.com",
        AuditAnchorNetwork::Sepolia => "https://ethereum-sepolia-rpc.publicnode.com",
    };
    eprintln!(
        "sbo3l audit verify-anchor: no --rpc-url + no SBO3L_RPC_URL — falling back to public {fallback}"
    );
    Ok(fallback.to_string())
}

async fn fetch_tx(rpc_url: &str, tx_hash: &str) -> Result<EthTransaction, String> {
    let body = RpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "eth_getTransactionByHash",
        params: vec![serde_json::Value::String(tx_hash.to_string())],
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    let resp = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("rpc call: {e}"))?;
    let parsed: RpcResponse<EthTransaction> = resp
        .json()
        .await
        .map_err(|e| format!("rpc response parse: {e}"))?;
    if let Some(err) = parsed.error {
        return Err(format!("rpc error {}: {}", err.code, err.message));
    }
    parsed
        .result
        .ok_or_else(|| format!("rpc returned null result for tx {tx_hash} (mined? right network?)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the calldata layout against a known-good envelope.
    /// Builds a calldata blob from the anchor crate's encoder, then
    /// decodes it back via the verifier's decoder — the round-trip
    /// MUST yield the same components.
    #[test]
    fn decode_round_trip_with_anchor_crate_encoder() {
        let tenant = format!("0x{}", "b".repeat(64));
        let root_hex = sbo3l_anchor::audit_root("sepolia", 42, &"a".repeat(64)).unwrap();
        let cd_bytes = sbo3l_anchor::publish_anchor_calldata(&tenant, &root_hex, 1234);
        let cd_hex = format!("0x{}", hex::encode(&cd_bytes));
        let decoded = decode_publish_anchor(&cd_hex).expect("decode");
        assert_eq!(decoded.selector, PUBLISH_ANCHOR_SELECTOR);
        assert_eq!(decoded.tenant_id, [0xbb; 32]);
        let mut expected_root = [0u8; 32];
        hex::decode_to_slice(&root_hex[2..], &mut expected_root).unwrap();
        assert_eq!(decoded.audit_root, expected_root);
        assert_eq!(decoded.chain_head_block, 1234);
    }

    #[test]
    fn decode_rejects_wrong_selector() {
        // Build a calldata blob with a bogus selector but otherwise
        // valid 100-byte length. Verifier MUST reject — we don't
        // want a `claimTenant` tx (different selector) to be
        // mistakenly verified as a `publishAnchor`.
        let mut cd = vec![0xde, 0xad, 0xbe, 0xef];
        cd.extend_from_slice(&[0u8; 96]);
        let cd_hex = format!("0x{}", hex::encode(&cd));
        let err = decode_publish_anchor(&cd_hex).unwrap_err();
        assert!(err.contains("publishAnchor"), "got: {err}");
    }

    #[test]
    fn decode_rejects_short_calldata() {
        let cd = vec![0u8; 50];
        let err = decode_publish_anchor(&format!("0x{}", hex::encode(&cd))).unwrap_err();
        assert!(err.contains("100"), "got: {err}");
    }

    #[test]
    fn looks_like_tx_hash_basic_shape() {
        assert!(looks_like_tx_hash(&format!("0x{}", "a".repeat(64))));
        assert!(looks_like_tx_hash(&"a".repeat(64))); // no prefix is fine
        assert!(!looks_like_tx_hash("0x123")); // too short
        assert!(!looks_like_tx_hash(&format!("0x{}", "g".repeat(64)))); // non-hex
    }
}
