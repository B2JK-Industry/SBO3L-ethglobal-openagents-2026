//! Uniswap guarded-swap adapter.
//!
//! SBO3L is not a trading bot. The Uniswap adapter exists to prove that an
//! agent which *wants* to trade through Uniswap can still be bounded by
//! SBO3L's policy boundary. Two responsibilities:
//!
//! 1. **Swap-policy guard.** A pure evaluator over a Uniswap quote and an
//!    operator-supplied swap policy: input/output token allowlists, max
//!    notional in USD, max slippage in bps, quote freshness window and
//!    treasury-recipient allowlist. Failures are returned in a structured
//!    `SwapPolicyOutcome` so the demo runner can label every check.
//! 2. **`GuardedExecutor` mirror of KeeperHub.** Once SBO3L has signed an
//!    `allow` policy receipt for a swap, this executor returns a deterministic
//!    `uni-<ULID>` execution_ref. Live mode is intentionally stubbed for the
//!    hackathon build — a real Uniswap routing API call is one function body.
//!
//! Demo line: *Agentic finance is only useful if agents can trade within
//! enforceable limits.*

use std::str::FromStr;
use std::sync::Arc;

use rust_decimal::Decimal;
use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::execution::{ExecutionError, ExecutionReceipt, GuardedExecutor};
use sbo3l_core::receipt::{Decision, PolicyReceipt};
use serde::{Deserialize, Serialize};

use crate::uniswap_live::{
    quote_exact_input_single, JsonRpcTransport, LiveConfig, QuoteResult, ReqwestTransport,
    RpcError, SEPOLIA_WETH,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwapToken {
    pub token_symbol: String,
    pub token_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
}

/// Uniswap-style quote envelope used by the demo. The shape mirrors the
/// fields a real Uniswap Trading API quote response carries; the demo's
/// fixture encoder under `demo-fixtures/uniswap/` follows this exactly.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwapQuote {
    /// A `_note` field is allowed in fixtures for human disclosure of mock
    /// status; deserializers should skip it.
    #[serde(rename = "_note", default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub quote_id: String,
    pub input: SwapToken,
    pub output: SwapToken,
    #[serde(default)]
    pub route: Vec<SwapToken>,
    pub expected_slippage_bps: u32,
    pub expires_at_unix: i64,
    pub fetched_at_unix: i64,
    pub treasury_recipient: String,
    pub chain: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwapPolicy {
    #[serde(rename = "_note", default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub agent_id: String,
    pub chain: String,
    pub input_token_allowlist: Vec<String>,
    pub output_token_allowlist: Vec<String>,
    pub max_notional_usd: String,
    pub max_slippage_bps: u32,
    pub quote_max_age_seconds: u32,
    pub treasury_recipient_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SwapCheck {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    /// True when this check was deliberately relaxed for fixture/demo
    /// purposes (only used today by `quote_freshness` against static
    /// fixture clocks). The demo runner surfaces this in output so judges
    /// see the relaxation, never a silent pass.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub relaxed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SwapPolicyOutcome {
    /// True if **any** check failed. The demo runner uses this to drive the
    /// "deny path" assertion.
    pub blocked: bool,
    pub checks: Vec<SwapCheck>,
}

impl SwapPolicyOutcome {
    pub fn first_failure(&self) -> Option<&SwapCheck> {
        self.checks.iter().find(|c| !c.ok)
    }
}

/// Evaluate a Uniswap quote against a per-agent swap policy.
///
/// `now_unix` is the wall-clock time used for the freshness check. When
/// `relax_freshness_for_static_fixture` is true we still surface the actual
/// quote age but the check passes — useful when the demo's fixture has a
/// static `fetched_at_unix` that is by definition stale.
pub fn evaluate_swap(
    quote: &SwapQuote,
    policy: &SwapPolicy,
    now_unix: i64,
    relax_freshness_for_static_fixture: bool,
) -> SwapPolicyOutcome {
    let mut checks: Vec<SwapCheck> = Vec::new();

    // 1. Input token allowlist.
    let in_ok = policy
        .input_token_allowlist
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&quote.input.token_symbol));
    checks.push(SwapCheck {
        name: "input_token_allowlisted",
        ok: in_ok,
        detail: format!(
            "actual={} allowed={:?}",
            quote.input.token_symbol, policy.input_token_allowlist
        ),
        relaxed: false,
    });

    // 2. Output token allowlist.
    let out_ok = policy
        .output_token_allowlist
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&quote.output.token_symbol));
    checks.push(SwapCheck {
        name: "output_token_allowlisted",
        ok: out_ok,
        detail: format!(
            "actual={} allowed={:?}",
            quote.output.token_symbol, policy.output_token_allowlist
        ),
        relaxed: false,
    });

    // 3. Max notional in USD. Treats input amount as USD because the demo's
    // input asset is a USD stablecoin; live mode would multiply by an oracle
    // price for non-stable inputs.
    let notional = quote
        .input
        .amount
        .as_deref()
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or(Decimal::ZERO);
    let cap = Decimal::from_str(&policy.max_notional_usd).unwrap_or(Decimal::ZERO);
    let notional_ok = notional <= cap;
    checks.push(SwapCheck {
        name: "max_notional_usd",
        ok: notional_ok,
        detail: format!("actual={} cap={}", notional, cap),
        relaxed: false,
    });

    // 4. Max slippage (bps).
    let slip_ok = quote.expected_slippage_bps <= policy.max_slippage_bps;
    checks.push(SwapCheck {
        name: "max_slippage_bps",
        ok: slip_ok,
        detail: format!(
            "actual={} cap={}",
            quote.expected_slippage_bps, policy.max_slippage_bps
        ),
        relaxed: false,
    });

    // 5. Quote freshness.
    let age = now_unix.saturating_sub(quote.fetched_at_unix);
    let strictly_fresh = age >= 0 && (age as u32) <= policy.quote_max_age_seconds;
    let fresh_ok = strictly_fresh || relax_freshness_for_static_fixture;
    checks.push(SwapCheck {
        name: "quote_freshness",
        ok: fresh_ok,
        detail: format!(
            "actual_age_seconds={} max={}",
            age, policy.quote_max_age_seconds
        ),
        relaxed: relax_freshness_for_static_fixture && !strictly_fresh,
    });

    // 6. Treasury recipient allowlist.
    let recipient_ok = policy
        .treasury_recipient_allowlist
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&quote.treasury_recipient));
    checks.push(SwapCheck {
        name: "treasury_recipient_allowlisted",
        ok: recipient_ok,
        detail: format!(
            "actual={} allowed={:?}",
            quote.treasury_recipient, policy.treasury_recipient_allowlist
        ),
        relaxed: false,
    });

    let blocked = checks.iter().any(|c| !c.ok);
    SwapPolicyOutcome { blocked, checks }
}

// ---------------------------------------------------------------------------
// P6.1 — Uniswap quote evidence
// ---------------------------------------------------------------------------

/// Compact ref to a token in the IP-1 / capsule-evidence wire form.
/// Mirrors what a real Uniswap V3 router quote response carries; the
/// mock variant uses the demo-fixture sentinel addresses
/// (`0x111…111` for treasury allowlist, `0x999…999` for the rug case)
/// so reviewers can grep for them across logs and capsules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TokenRef {
    pub symbol: String,
    pub address: String,
}

/// Sponsor-specific evidence captured by `UniswapExecutor` when an
/// allow-path swap goes through. Surfaced verbatim through
/// `ExecutionReceipt.evidence` and serialised into the capsule's
/// `execution.executor_evidence` slot by `sbo3l passport run`. The
/// schema constrains `executor_evidence` to be either `null`, omitted,
/// or a non-empty object (`oneOf null / object minProperties:1`,
/// `additionalProperties: true`); the ten fields below all serialise
/// to the same JSON shape, so an auditor reading the capsule sees the
/// same wire form they would from a real Uniswap V3 router quote.
///
/// `executor_evidence` is *mode-agnostic* — distinct from the
/// transport-level `live_evidence` slot (strictly live-only via the
/// verifier's bidirectional invariant). A mock allow path therefore
/// emits `live_evidence: null` AND a populated `executor_evidence`,
/// which is exactly what the demo's `uniswap-guarded-swap.sh` step
/// pins.
///
/// The struct is **mock-only today**: `quote_source` is hard-coded to
/// `"mock-uniswap-v3-router"` and the quote_id carries a `mock-…`
/// prefix, so demo output cannot accidentally pass for a live quote.
/// When live trading lands (P6.1 follow-up), the constructor signature
/// changes to take a real quote and `quote_source` flips to the real
/// router endpoint URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniswapQuoteEvidence {
    pub quote_id: String,
    pub quote_source: String,
    pub input_token: TokenRef,
    pub output_token: TokenRef,
    pub route_tokens: Vec<TokenRef>,
    pub notional_in: String,
    pub slippage_cap_bps: u32,
    pub quote_timestamp_unix: i64,
    pub quote_freshness_seconds: u32,
    pub recipient_address: String,
}

impl UniswapQuoteEvidence {
    /// Build a deterministic mock evidence payload from the request the
    /// executor is about to run. Fields not derivable from `request`
    /// (slippage cap, freshness window, treasury recipient) come from
    /// the demo-fixture defaults (`50 bps`, `30 s`, the
    /// `0x111…111` recipient sentinel) so the wire shape matches what
    /// the demo's `evaluate_swap` consumes.
    ///
    /// Designed for the executor's `LocalMock` arm only — see
    /// `UniswapExecutor::execute`. Live mode currently returns
    /// `BackendOffline` and never invokes this constructor.
    pub fn mock_from_request(request: &PaymentRequest) -> Self {
        let now = chrono::Utc::now().timestamp();
        let notional_in = request.amount.value.clone();
        Self {
            quote_id: format!("mock-uniswap-quote-{}", ulid::Ulid::new()),
            quote_source: "mock-uniswap-v3-router".to_string(),
            input_token: TokenRef {
                symbol: "USDC".to_string(),
                address: "0x0000000000000000000000000000000000000000".to_string(),
            },
            output_token: TokenRef {
                symbol: "ETH".to_string(),
                address: "0x0000000000000000000000000000000000000001".to_string(),
            },
            route_tokens: vec![
                TokenRef {
                    symbol: "USDC".to_string(),
                    address: "0x0000000000000000000000000000000000000000".to_string(),
                },
                TokenRef {
                    symbol: "ETH".to_string(),
                    address: "0x0000000000000000000000000000000000000001".to_string(),
                },
            ],
            notional_in,
            slippage_cap_bps: 50,
            quote_timestamp_unix: now,
            quote_freshness_seconds: 30,
            recipient_address: "0x1111111111111111111111111111111111111111".to_string(),
        }
    }

    /// Serialise to a `serde_json::Value::Object` for embedding in
    /// `ExecutionReceipt.evidence` and downstream
    /// `execution.executor_evidence`. Always returns an `Object` with
    /// at least 10 properties, satisfying the capsule schema's
    /// `executor_evidence.minProperties: 1` invariant.
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self)
            .expect("UniswapQuoteEvidence's #[derive(Serialize)] is infallible for owned fields")
    }
}

/// Guarded executor mirroring KeeperHub's pattern. Refuses anything that is
/// not an explicit `allow` policy receipt before any sponsor backend is
/// touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniswapMode {
    Live,
    LocalMock,
}

/// Live-mode context: the Uniswap V3 QuoterV2 RPC config + the
/// transport carrying the eth_call. Stored behind `Arc` so the
/// `Clone`-able `UniswapExecutor` doesn't duplicate the underlying
/// HTTP client. Absent (`None` on `UniswapExecutor.live_context`)
/// means the executor was built via the bare `Self::live()` ctor —
/// `execute()` returns `BackendOffline` in that case, mirroring the
/// pre-B7 "live mode without credentials fails loudly" contract.
pub struct LiveContext {
    pub config: LiveConfig,
    pub transport: Arc<dyn JsonRpcTransport>,
}

#[derive(Clone)]
pub struct UniswapExecutor {
    pub mode: UniswapMode,
    /// Set on the live path (`live_from_env` or `live_with_context`).
    /// Bare `live()` leaves this `None` — execute() routes to a loud
    /// `BackendOffline` error in that case (back-compat with the
    /// pre-B7 test).
    pub(crate) live_context: Option<Arc<LiveContext>>,
}

impl std::fmt::Debug for UniswapExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UniswapExecutor")
            .field("mode", &self.mode)
            .field("live_context_present", &self.live_context.is_some())
            .finish()
    }
}

/// Reasons `live_from_env` can refuse to construct. Surfaces the
/// missing env var so the operator knows which to set.
#[derive(Debug, thiserror::Error)]
pub enum LiveConfigError {
    #[error("env var {0} is not set")]
    MissingEnvVar(&'static str),
    #[error("env var {var} is set but invalid: {detail}")]
    BadEnvVar { var: &'static str, detail: String },
}

impl UniswapExecutor {
    pub fn local_mock() -> Self {
        Self {
            mode: UniswapMode::LocalMock,
            live_context: None,
        }
    }

    /// Bare live ctor — no transport, no config. Kept for back-compat
    /// with pre-B7 call sites; `execute()` returns `BackendOffline`
    /// because there's nothing to talk to. Use [`Self::live_from_env`]
    /// or [`Self::live_with_context`] for an actually-functional
    /// live executor.
    pub fn live() -> Self {
        Self {
            mode: UniswapMode::Live,
            live_context: None,
        }
    }

    /// Live ctor wired to a real Sepolia QuoterV2 via env config:
    /// - `SBO3L_UNISWAP_RPC_URL` (required)
    /// - `SBO3L_UNISWAP_TOKEN_IN` (default: Sepolia WETH)
    /// - `SBO3L_UNISWAP_TOKEN_OUT` (required)
    /// - `SBO3L_UNISWAP_FEE_TIER` (default: `3000`)
    /// - `SBO3L_UNISWAP_AMOUNT_IN_WEI` (default: `1000000000000000000`)
    pub fn live_from_env() -> Result<Self, LiveConfigError> {
        let rpc_url = read_env("SBO3L_UNISWAP_RPC_URL")?;
        let token_in =
            std::env::var("SBO3L_UNISWAP_TOKEN_IN").unwrap_or_else(|_| SEPOLIA_WETH.to_string());
        let token_out = read_env("SBO3L_UNISWAP_TOKEN_OUT")?;
        let fee_tier_raw =
            std::env::var("SBO3L_UNISWAP_FEE_TIER").unwrap_or_else(|_| "3000".to_string());
        let fee_tier: u32 = fee_tier_raw.parse().map_err(|e: std::num::ParseIntError| {
            LiveConfigError::BadEnvVar {
                var: "SBO3L_UNISWAP_FEE_TIER",
                detail: e.to_string(),
            }
        })?;
        let amount_in_wei = std::env::var("SBO3L_UNISWAP_AMOUNT_IN_WEI")
            .unwrap_or_else(|_| "1000000000000000000".to_string());

        let config = LiveConfig::sepolia_default(
            token_in,
            token_out,
            fee_tier,
            amount_in_wei,
            rpc_url.clone(),
        );
        let transport = Arc::new(ReqwestTransport::new(rpc_url));
        Ok(Self::live_with_context(LiveContext { config, transport }))
    }

    /// Live ctor that takes a fully-built [`LiveContext`]. Production
    /// uses [`Self::live_from_env`]; tests build an in-process
    /// transport and pass it here.
    pub fn live_with_context(ctx: LiveContext) -> Self {
        Self {
            mode: UniswapMode::Live,
            live_context: Some(Arc::new(ctx)),
        }
    }
}

fn read_env(var: &'static str) -> Result<String, LiveConfigError> {
    let v = std::env::var(var).map_err(|_| LiveConfigError::MissingEnvVar(var))?;
    if v.trim().is_empty() {
        return Err(LiveConfigError::BadEnvVar {
            var,
            detail: "empty".into(),
        });
    }
    Ok(v)
}

impl GuardedExecutor for UniswapExecutor {
    fn sponsor_id(&self) -> &'static str {
        "uniswap"
    }

    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        if !matches!(receipt.decision, Decision::Allow) {
            return Err(ExecutionError::NotApproved(receipt.decision.clone()));
        }
        match self.mode {
            UniswapMode::LocalMock => {
                // P6.1: attach concrete quote evidence on the allow path.
                // The CLI's `passport run` reads `ExecutionReceipt.evidence`
                // and copies it into `capsule.execution.executor_evidence`
                // (NOT `live_evidence` — that slot is strictly transport
                // -level and live-only via the verifier's bidirectional
                // invariant). The schema requires `executor_evidence` to
                // be either `null` / omitted, or an object with at least
                // one property; `UniswapQuoteEvidence::to_value`
                // satisfies the latter (10 properties).
                let evidence = UniswapQuoteEvidence::mock_from_request(request).to_value();
                Ok(ExecutionReceipt {
                    sponsor: "uniswap",
                    execution_ref: format!("uni-{}", ulid::Ulid::new()),
                    mock: true,
                    note: format!(
                        "local mock: would route {agent}/{intent} via Uniswap Trading API",
                        agent = request.agent_id,
                        intent = serde_json::to_string(&request.intent).unwrap_or_default(),
                    ),
                    evidence: Some(evidence),
                })
            }
            UniswapMode::Live => {
                let ctx = self.live_context.as_ref().ok_or_else(|| {
                    ExecutionError::BackendOffline(
                        "live Uniswap backend has no LiveContext; build via \
                         UniswapExecutor::live_from_env() or live_with_context()"
                            .to_string(),
                    )
                })?;
                let quote = quote_exact_input_single(ctx.transport.as_ref(), &ctx.config)
                    .map_err(map_rpc_err)?;
                let evidence = build_live_evidence(request, &ctx.config, &quote);
                Ok(ExecutionReceipt {
                    sponsor: "uniswap",
                    execution_ref: format!("uni-{}", ulid::Ulid::new()),
                    mock: false,
                    note: format!(
                        "live: Sepolia QuoterV2 at {} returned amountOut={} gasEstimate={}",
                        ctx.config.quoter, quote.amount_out, quote.gas_estimate
                    ),
                    evidence: Some(evidence),
                })
            }
        }
    }
}

/// Map a JSON-RPC error to the right `ExecutionError` variant.
/// Transport-level (HTTP, parse) → `BackendOffline`; protocol-level
/// (server reverted, decode) → `Integration`. Keeps the existing
/// "BackendOffline = sponsor unreachable" contract.
fn map_rpc_err(e: RpcError) -> ExecutionError {
    match e {
        RpcError::Http(s) | RpcError::Parse(s) => {
            ExecutionError::BackendOffline(format!("uniswap RPC: {s}"))
        }
        RpcError::Server { code, message } => {
            ExecutionError::Integration(format!("uniswap RPC server error {code}: {message}"))
        }
        RpcError::Decode(s) => ExecutionError::Integration(format!("uniswap RPC decode: {s}")),
    }
}

/// Build the evidence payload for a live allow-path quote. Reuses
/// the 10 [`UniswapQuoteEvidence`] fields the mock path emits and
/// extends with 4 live-only fields (`chain_id`, `amount_out`,
/// `sqrt_price_x96_after`, `gas_estimate`) — the capsule schema
/// permits `additionalProperties` on `executor_evidence`, so the
/// extra fields land cleanly without a schema bump.
///
/// `quote_source` carries the network + quoter address verbatim so
/// an auditor reading the capsule sees exactly which contract
/// answered.
fn build_live_evidence(
    request: &PaymentRequest,
    config: &LiveConfig,
    quote: &QuoteResult,
) -> serde_json::Value {
    let now = chrono::Utc::now().timestamp();
    let _ = request; // request is forwarded for future routing decisions
    let in_token = TokenRef {
        symbol: "TOKEN_IN".to_string(),
        address: config.token_in.clone(),
    };
    let out_token = TokenRef {
        symbol: "TOKEN_OUT".to_string(),
        address: config.token_out.clone(),
    };
    serde_json::json!({
        "quote_id": format!("uni-{}", ulid::Ulid::new()),
        "quote_source": format!(
            "uniswap-v3-quoter-sepolia-{}",
            config.quoter.to_lowercase()
        ),
        "input_token": in_token,
        "output_token": out_token,
        "route_tokens": [in_token.clone(), out_token.clone()],
        "notional_in": config.amount_in_wei,
        "slippage_cap_bps": 0u32,
        "quote_timestamp_unix": now,
        "quote_freshness_seconds": 30u32,
        "recipient_address": "0x0000000000000000000000000000000000000000",
        // Live-only fields (extra to UniswapQuoteEvidence shape):
        "chain_id": config.chain_id,
        "amount_out": quote.amount_out,
        "sqrt_price_x96_after": quote.sqrt_price_x96_after,
        "initialized_ticks_crossed": quote.initialized_ticks_crossed,
        "gas_estimate": quote.gas_estimate,
    })
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::uniswap_live::tests_support::{abi_encode_quad, FakeTransport};
    use crate::uniswap_live::{SEPOLIA_CHAIN_ID, SEPOLIA_QUOTER_V2_ADDRESS};

    fn aprp() -> PaymentRequest {
        let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        serde_json::from_value(v).unwrap()
    }

    fn allow_receipt() -> PolicyReceipt {
        use sbo3l_core::receipt::{EmbeddedSignature, ReceiptType, SignatureAlgorithm};
        PolicyReceipt {
            receipt_type: ReceiptType::PolicyReceiptV1,
            version: 1,
            agent_id: "research-agent-01".to_string(),
            decision: Decision::Allow,
            deny_code: None,
            request_hash: "1".repeat(64),
            policy_hash: "2".repeat(64),
            policy_version: Some(1),
            audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            execution_ref: None,
            issued_at: chrono::Utc::now(),
            expires_at: None,
            signature: EmbeddedSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: "test".to_string(),
                signature_hex: "0".repeat(128),
            },
        }
    }

    fn live_cfg() -> LiveConfig {
        LiveConfig::sepolia_default(
            SEPOLIA_WETH.to_string(),
            "0x0000000000000000000000000000000000000022".to_string(),
            3000,
            "1000000000000000000".to_string(),
            "http://example.invalid".to_string(),
        )
    }

    #[test]
    fn live_path_emits_executor_evidence_with_real_amount_out() {
        let t = FakeTransport::new();
        t.expect(
            SEPOLIA_QUOTER_V2_ADDRESS,
            "0xc6a5026a",
            Ok(abi_encode_quad(
                "2500000000000000000",
                "1234567",
                3,
                "85000",
            )),
        );
        let exec = UniswapExecutor::live_with_context(LiveContext {
            config: live_cfg(),
            transport: Arc::new(t),
        });
        let receipt = exec.execute(&aprp(), &allow_receipt()).unwrap();
        assert!(!receipt.mock, "live mode must NOT mark mock=true");
        assert_eq!(receipt.sponsor, "uniswap");
        assert!(receipt.execution_ref.starts_with("uni-"));
        let evidence = receipt
            .evidence
            .expect("live path must emit executor_evidence");
        assert_eq!(
            evidence["amount_out"].as_str().unwrap(),
            "2500000000000000000"
        );
        assert_eq!(evidence["chain_id"].as_u64().unwrap(), SEPOLIA_CHAIN_ID);
        assert!(evidence["quote_source"]
            .as_str()
            .unwrap()
            .contains("sepolia"));
        assert!(evidence["quote_source"]
            .as_str()
            .unwrap()
            .contains(&SEPOLIA_QUOTER_V2_ADDRESS.to_lowercase()));
    }

    #[test]
    fn live_path_server_revert_surfaces_as_integration_error() {
        let t = FakeTransport::new();
        t.expect(
            SEPOLIA_QUOTER_V2_ADDRESS,
            "0xc6a5026a",
            Err(RpcError::Server {
                code: 3,
                message: "execution reverted: insufficient liquidity".into(),
            }),
        );
        let exec = UniswapExecutor::live_with_context(LiveContext {
            config: live_cfg(),
            transport: Arc::new(t),
        });
        let err = exec.execute(&aprp(), &allow_receipt()).unwrap_err();
        match err {
            ExecutionError::Integration(msg) => {
                assert!(msg.contains("insufficient liquidity"));
                assert!(msg.contains("3"));
            }
            other => panic!("expected Integration, got {other:?}"),
        }
    }

    #[test]
    fn live_path_http_timeout_surfaces_as_backend_offline() {
        let t = FakeTransport::new();
        t.expect(
            SEPOLIA_QUOTER_V2_ADDRESS,
            "0xc6a5026a",
            Err(RpcError::Http("request timed out".into())),
        );
        let exec = UniswapExecutor::live_with_context(LiveContext {
            config: live_cfg(),
            transport: Arc::new(t),
        });
        let err = exec.execute(&aprp(), &allow_receipt()).unwrap_err();
        match err {
            ExecutionError::BackendOffline(msg) => {
                assert!(msg.contains("timed out"));
                assert!(msg.contains("uniswap RPC"));
            }
            other => panic!("expected BackendOffline, got {other:?}"),
        }
    }

    #[test]
    fn live_path_denied_receipt_never_calls_rpc() {
        let t = FakeTransport::new();
        // No expectations registered — any eth_call hit would fail.
        let exec = UniswapExecutor::live_with_context(LiveContext {
            config: live_cfg(),
            transport: Arc::new(t),
        });
        let mut deny = allow_receipt();
        deny.decision = Decision::Deny;
        let err = exec.execute(&aprp(), &deny).unwrap_err();
        assert!(matches!(err, ExecutionError::NotApproved(_)));
    }

    #[test]
    fn live_from_env_errors_when_rpc_url_missing() {
        // Save and clear all five vars to ensure a clean state.
        let saved: Vec<(&str, Option<String>)> = [
            "SBO3L_UNISWAP_RPC_URL",
            "SBO3L_UNISWAP_TOKEN_IN",
            "SBO3L_UNISWAP_TOKEN_OUT",
            "SBO3L_UNISWAP_FEE_TIER",
            "SBO3L_UNISWAP_AMOUNT_IN_WEI",
        ]
        .iter()
        .map(|v| (*v, std::env::var(v).ok()))
        .collect();
        for (v, _) in &saved {
            std::env::remove_var(v);
        }
        let r = UniswapExecutor::live_from_env();
        assert!(matches!(
            r,
            Err(LiveConfigError::MissingEnvVar("SBO3L_UNISWAP_RPC_URL"))
        ));
        // Restore.
        for (v, val) in saved {
            if let Some(val) = val {
                std::env::set_var(v, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_policy() -> SwapPolicy {
        SwapPolicy {
            note: None,
            agent_id: "research-agent-01".to_string(),
            chain: "base".to_string(),
            input_token_allowlist: vec!["USDC".to_string()],
            output_token_allowlist: vec!["ETH".to_string(), "WETH".to_string()],
            max_notional_usd: "20.00".to_string(),
            max_slippage_bps: 50,
            quote_max_age_seconds: 30,
            treasury_recipient_allowlist: vec![
                "0x1111111111111111111111111111111111111111".to_string()
            ],
        }
    }

    fn allow_quote() -> SwapQuote {
        SwapQuote {
            note: None,
            quote_id: "qt-allow-1".to_string(),
            input: SwapToken {
                token_symbol: "USDC".to_string(),
                token_address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
                amount: Some("5.00".to_string()),
                decimals: Some(6),
            },
            output: SwapToken {
                token_symbol: "ETH".to_string(),
                token_address: "0x0000000000000000000000000000000000000000".to_string(),
                amount: Some("0.0014".to_string()),
                decimals: Some(18),
            },
            route: vec![],
            expected_slippage_bps: 35,
            expires_at_unix: 9_999_999_999,
            fetched_at_unix: 1_700_000_000,
            treasury_recipient: "0x1111111111111111111111111111111111111111".to_string(),
            chain: "base".to_string(),
        }
    }

    #[test]
    fn legit_quote_passes_all_checks_with_relaxed_freshness() {
        let outcome = evaluate_swap(&allow_quote(), &allow_policy(), 1_700_000_005, true);
        assert!(!outcome.blocked, "outcome={:?}", outcome);
        assert_eq!(outcome.checks.len(), 6);
        assert!(outcome.checks.iter().all(|c| c.ok));
    }

    #[test]
    fn rug_token_is_blocked_by_output_allowlist() {
        let mut q = allow_quote();
        q.output.token_symbol = "RUG".to_string();
        q.output.token_address = "0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead".to_string();
        let outcome = evaluate_swap(&q, &allow_policy(), 1_700_000_005, true);
        assert!(outcome.blocked);
        let failed = outcome.first_failure().unwrap();
        assert_eq!(failed.name, "output_token_allowlisted");
    }

    #[test]
    fn over_notional_is_blocked() {
        let mut q = allow_quote();
        q.input.amount = Some("9999.00".to_string());
        let outcome = evaluate_swap(&q, &allow_policy(), 1_700_000_005, true);
        let failed = outcome
            .checks
            .iter()
            .find(|c| c.name == "max_notional_usd")
            .unwrap();
        assert!(!failed.ok);
        assert!(outcome.blocked);
    }

    #[test]
    fn over_slippage_is_blocked() {
        let mut q = allow_quote();
        q.expected_slippage_bps = 1500;
        let outcome = evaluate_swap(&q, &allow_policy(), 1_700_000_005, true);
        let failed = outcome
            .checks
            .iter()
            .find(|c| c.name == "max_slippage_bps")
            .unwrap();
        assert!(!failed.ok);
        assert!(outcome.blocked);
    }

    #[test]
    fn stale_quote_is_blocked_when_freshness_strict() {
        let outcome = evaluate_swap(&allow_quote(), &allow_policy(), 1_700_000_000 + 9999, false);
        let failed = outcome
            .checks
            .iter()
            .find(|c| c.name == "quote_freshness")
            .unwrap();
        assert!(!failed.ok);
        assert!(outcome.blocked);
    }

    #[test]
    fn relaxed_freshness_marks_relaxed_flag() {
        let outcome = evaluate_swap(&allow_quote(), &allow_policy(), 1_700_000_000 + 9999, true);
        let fresh = outcome
            .checks
            .iter()
            .find(|c| c.name == "quote_freshness")
            .unwrap();
        assert!(fresh.ok);
        assert!(fresh.relaxed);
        assert!(!outcome.blocked);
    }

    #[test]
    fn attacker_recipient_is_blocked() {
        let mut q = allow_quote();
        q.treasury_recipient = "0x9999999999999999999999999999999999999999".to_string();
        let outcome = evaluate_swap(&q, &allow_policy(), 1_700_000_005, true);
        let failed = outcome
            .checks
            .iter()
            .find(|c| c.name == "treasury_recipient_allowlisted")
            .unwrap();
        assert!(!failed.ok);
        assert!(outcome.blocked);
    }

    fn aprp() -> PaymentRequest {
        let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        serde_json::from_value(v).unwrap()
    }

    fn receipt(decision: Decision) -> PolicyReceipt {
        use sbo3l_core::receipt::{EmbeddedSignature, ReceiptType, SignatureAlgorithm};
        PolicyReceipt {
            receipt_type: ReceiptType::PolicyReceiptV1,
            version: 1,
            agent_id: "research-agent-01".to_string(),
            decision,
            deny_code: None,
            request_hash: "1".repeat(64),
            policy_hash: "2".repeat(64),
            policy_version: Some(1),
            audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGS".to_string(),
            execution_ref: None,
            issued_at: chrono::Utc::now(),
            expires_at: None,
            signature: EmbeddedSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                key_id: "test".to_string(),
                signature_hex: "0".repeat(128),
            },
        }
    }

    #[test]
    fn approved_receipt_routes_to_uniswap_mock() {
        let exec = UniswapExecutor::local_mock();
        let r = exec.execute(&aprp(), &receipt(Decision::Allow)).unwrap();
        assert_eq!(r.sponsor, "uniswap");
        assert!(r.mock);
        assert!(r.execution_ref.starts_with("uni-"));
    }

    #[test]
    fn denied_receipt_never_reaches_uniswap() {
        let exec = UniswapExecutor::local_mock();
        let err = exec.execute(&aprp(), &receipt(Decision::Deny)).unwrap_err();
        assert!(matches!(err, ExecutionError::NotApproved(_)));
    }

    #[test]
    fn live_mode_fails_loudly_without_credentials() {
        let exec = UniswapExecutor::live();
        let err = exec
            .execute(&aprp(), &receipt(Decision::Allow))
            .unwrap_err();
        assert!(matches!(err, ExecutionError::BackendOffline(_)));
    }
}
