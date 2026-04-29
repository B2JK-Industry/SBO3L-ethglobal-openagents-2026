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

use rust_decimal::Decimal;
use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::execution::{ExecutionError, ExecutionReceipt, GuardedExecutor};
use sbo3l_core::receipt::{Decision, PolicyReceipt};
use serde::{Deserialize, Serialize};

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

/// Guarded executor mirroring KeeperHub's pattern. Refuses anything that is
/// not an explicit `allow` policy receipt before any sponsor backend is
/// touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniswapMode {
    Live,
    LocalMock,
}

#[derive(Debug, Clone)]
pub struct UniswapExecutor {
    pub mode: UniswapMode,
}

impl UniswapExecutor {
    pub fn local_mock() -> Self {
        Self {
            mode: UniswapMode::LocalMock,
        }
    }

    pub fn live() -> Self {
        Self {
            mode: UniswapMode::Live,
        }
    }
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
            UniswapMode::LocalMock => Ok(ExecutionReceipt {
                sponsor: "uniswap",
                execution_ref: format!("uni-{}", ulid::Ulid::new()),
                mock: true,
                note: format!(
                    "local mock: would route {agent}/{intent} via Uniswap Trading API",
                    agent = request.agent_id,
                    intent = serde_json::to_string(&request.intent).unwrap_or_default(),
                ),
            }),
            UniswapMode::Live => Err(ExecutionError::BackendOffline(
                "live Uniswap backend not configured for this hackathon build; \
                 switch to UniswapMode::LocalMock or wire credentials"
                    .to_string(),
            )),
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
