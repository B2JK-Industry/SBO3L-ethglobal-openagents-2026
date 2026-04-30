//! B7 live smoke: call Sepolia QuoterV2 via the real reqwest
//! transport + print the quote. Operator-supplied env vars:
//!
//!   SBO3L_UNISWAP_RPC_URL          — required (e.g. Alchemy free-tier)
//!   SBO3L_UNISWAP_TOKEN_OUT        — required (Sepolia ERC20 address)
//!   SBO3L_UNISWAP_TOKEN_IN         — defaults to Sepolia WETH
//!   SBO3L_UNISWAP_FEE_TIER         — defaults to 3000 (V3 0.3% tier)
//!   SBO3L_UNISWAP_AMOUNT_IN_WEI    — defaults to 1e18 (1 WETH)
//!
//! Use:
//!
//!   cargo run -p sbo3l-execution --example uniswap_live_smoke
//!
//! Prints the four QuoteResult fields (amount_out, sqrt_price_x96_after,
//! initialized_ticks_crossed, gas_estimate) and exits with the usual
//! sponsor-error mapping (BackendOffline → 1, Integration → 2). CI
//! does NOT run this example — it requires a real RPC URL and is
//! not part of the test suite.

use std::process::ExitCode;

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::execution::{ExecutionError, GuardedExecutor};
use sbo3l_core::receipt::{
    Decision, EmbeddedSignature, PolicyReceipt, ReceiptType, SignatureAlgorithm,
};
use sbo3l_execution::UniswapExecutor;

fn main() -> ExitCode {
    let exec = match UniswapExecutor::live_from_env() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("uniswap_live_smoke: configuration error: {e}");
            eprintln!(
                "Required env vars: SBO3L_UNISWAP_RPC_URL, \
                 SBO3L_UNISWAP_TOKEN_OUT. Optional: SBO3L_UNISWAP_TOKEN_IN \
                 (default: Sepolia WETH), SBO3L_UNISWAP_FEE_TIER \
                 (default: 3000), SBO3L_UNISWAP_AMOUNT_IN_WEI \
                 (default: 1e18)."
            );
            return ExitCode::from(2);
        }
    };

    let request = stub_payment_request();
    let receipt = stub_allow_receipt();

    match exec.execute(&request, &receipt) {
        Ok(r) => {
            println!("uniswap_live_smoke: SUCCESS");
            println!("  sponsor:       {}", r.sponsor);
            println!("  execution_ref: {}", r.execution_ref);
            println!("  mock:          {}", r.mock);
            println!("  note:          {}", r.note);
            if let Some(evidence) = r.evidence.as_ref() {
                println!("  evidence:");
                let pretty = serde_json::to_string_pretty(evidence)
                    .unwrap_or_else(|e| format!("<serialise failed: {e}>"));
                for line in pretty.lines() {
                    println!("    {line}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(ExecutionError::BackendOffline(msg)) => {
            eprintln!("uniswap_live_smoke: BackendOffline: {msg}");
            ExitCode::from(1)
        }
        Err(ExecutionError::Integration(msg)) => {
            eprintln!("uniswap_live_smoke: Integration: {msg}");
            ExitCode::from(2)
        }
        Err(other) => {
            eprintln!("uniswap_live_smoke: unexpected error: {other}");
            ExitCode::from(2)
        }
    }
}

/// Load the workspace's golden APRP fixture. The smoke binary just
/// needs *some* valid PaymentRequest to drive the live quote — the
/// PaymentRequest content is opaque to the live path (the live
/// quote uses LiveConfig for token addresses + amount, not the
/// APRP fields).
fn stub_payment_request() -> PaymentRequest {
    let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
    let v: serde_json::Value = serde_json::from_str(raw).expect("golden APRP must parse as JSON");
    serde_json::from_value(v).expect("golden APRP must deserialise into PaymentRequest")
}

fn stub_allow_receipt() -> PolicyReceipt {
    PolicyReceipt {
        receipt_type: ReceiptType::PolicyReceiptV1,
        version: 1,
        agent_id: "research-agent-01".to_string(),
        decision: Decision::Allow,
        deny_code: None,
        request_hash: "1".repeat(64),
        policy_hash: "2".repeat(64),
        policy_version: Some(1),
        audit_event_id: "evt-smoke-uniswap-live".to_string(),
        execution_ref: None,
        issued_at: chrono::Utc::now(),
        expires_at: None,
        signature: EmbeddedSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            key_id: "smoke".to_string(),
            signature_hex: "0".repeat(128),
        },
    }
}
