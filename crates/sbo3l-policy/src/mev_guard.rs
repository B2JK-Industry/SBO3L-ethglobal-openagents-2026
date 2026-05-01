//! MEV protection guard for swap-style intents.
//!
//! Sits between APRP submit and execution: given a fresh on-chain quote and the
//! swap parameters the caller intends to broadcast, the guard refuses to forward
//! the swap when either the recipient is off the allowlist or the user's
//! `amount_out_min` is so far below the quote that a sandwich attacker has room
//! to extract value.
//!
//! The slippage check is the load-bearing one: classic sandwich attacks exploit
//! the gap between the quoted price and the user's stated minimum. A 50 bps
//! ceiling is the canonical default — the same threshold Uniswap's Auto Router
//! uses by default.
//!
//! The guard is intentionally schema-light so it can be invoked from the
//! daemon, the CLI, or a unit test without dragging the full APRP request type
//! across module boundaries. Callers map their request shape into [`Quote`] +
//! [`SwapIntent`] before evaluation.

use serde::{Deserialize, Serialize};

/// Maximum value a slippage tolerance can take. 100% in basis points.
const MAX_BPS: u32 = 10_000;

/// Reasonable upper bound for `max_slippage_bps`. Anything above 30%
/// (3000 bps) is almost certainly a misconfiguration on a like-for-like
/// stable-pair or low-vol swap; rather than silently allow it, configs above
/// this fail at construction time.
const SANE_SLIPPAGE_CEILING_BPS: u32 = 3_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MevGuardConfig {
    /// Maximum slippage between the quote and the swap's `amount_out_min`,
    /// expressed in basis points (1 bp = 0.01%). Inclusive: a swap exactly at
    /// `max_slippage_bps` passes.
    pub max_slippage_bps: u32,
    /// Lowercased EVM addresses that are allowed to receive swap output.
    /// Empty = nobody is allowed (fail-closed).
    pub recipient_allowlist: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MevGuardConfigError {
    #[error("max_slippage_bps={0} exceeds sane ceiling of {SANE_SLIPPAGE_CEILING_BPS}")]
    SlippageTooHigh(u32),
    #[error("max_slippage_bps={0} exceeds {MAX_BPS} (100%)")]
    SlippageOutOfRange(u32),
    #[error("recipient '{0}' is not a valid EVM address (expected 0x + 40 hex chars)")]
    BadRecipient(String),
}

impl MevGuardConfig {
    /// Validate the config and normalise allowlist entries to lowercase. EVM
    /// addresses are case-insensitive (EIP-55 is a checksum overlay only), so
    /// we compare lowercase against lowercase to avoid false-negative denies.
    pub fn try_new(
        max_slippage_bps: u32,
        recipient_allowlist: impl IntoIterator<Item = String>,
    ) -> Result<Self, MevGuardConfigError> {
        if max_slippage_bps > MAX_BPS {
            return Err(MevGuardConfigError::SlippageOutOfRange(max_slippage_bps));
        }
        if max_slippage_bps > SANE_SLIPPAGE_CEILING_BPS {
            return Err(MevGuardConfigError::SlippageTooHigh(max_slippage_bps));
        }
        let mut normalised = Vec::new();
        for r in recipient_allowlist {
            if !is_evm_address(&r) {
                return Err(MevGuardConfigError::BadRecipient(r));
            }
            normalised.push(r.to_ascii_lowercase());
        }
        Ok(Self {
            max_slippage_bps,
            recipient_allowlist: normalised,
        })
    }
}

/// On-chain quote captured at the moment of policy evaluation.
///
/// `expected_amount_out` is what the pool *currently* says you would receive
/// for `amount_in`, denominated in the same token-decimals as `amount_out_min`
/// in [`SwapIntent`]. Same-decimal symmetry is the caller's responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub expected_amount_out: u128,
}

/// Caller's intended swap. `amount_out_min` is the slippage floor that would be
/// passed verbatim to the AMM (e.g. SwapRouter02's `exactInputSingle`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapIntent {
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub amount_out_min: u128,
    pub recipient: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MevGuardOutcome {
    Allowed,
    DeniedSlippage {
        actual_bps: u32,
        max_bps: u32,
    },
    DeniedRecipient {
        recipient: String,
    },
    /// The intent and quote disagree on which token pair / amount is being
    /// traded — a malformed input that we refuse rather than silently let
    /// through. Catches a class of bugs where the caller rebuilt the quote
    /// against the wrong pool but kept the intent unchanged.
    DeniedQuoteMismatch,
}

impl MevGuardOutcome {
    /// Stable string code suitable for embedding in a deny receipt's
    /// `deny_code` field. Keep these in sync with `docs/spec/19_deny_codes.md`
    /// (forthcoming) and any docs that branch on the code.
    pub fn deny_code(&self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::DeniedSlippage { .. } => Some("policy.deny_mev_slippage_too_high"),
            Self::DeniedRecipient { .. } => Some("policy.deny_mev_recipient_off_allowlist"),
            Self::DeniedQuoteMismatch => Some("policy.deny_mev_quote_mismatch"),
        }
    }
}

/// Evaluate a single swap against the MEV guard config + a fresh quote.
///
/// Order of checks (fail-closed): quote-mismatch → recipient → slippage. The
/// recipient check runs before the slippage check so a swap to an attacker
/// address denies as `recipient_off_allowlist` even if its slippage is
/// otherwise within tolerance.
pub fn evaluate(config: &MevGuardConfig, quote: &Quote, intent: &SwapIntent) -> MevGuardOutcome {
    if !quote.token_in.eq_ignore_ascii_case(&intent.token_in)
        || !quote.token_out.eq_ignore_ascii_case(&intent.token_out)
        || quote.amount_in != intent.amount_in
    {
        return MevGuardOutcome::DeniedQuoteMismatch;
    }

    let recipient_lc = intent.recipient.to_ascii_lowercase();
    if !config
        .recipient_allowlist
        .iter()
        .any(|r| r == &recipient_lc)
    {
        return MevGuardOutcome::DeniedRecipient {
            recipient: intent.recipient.clone(),
        };
    }

    // A min ≥ quote means the caller is demanding at-or-above the current
    // price. Sandwich is impossible: any front-run that lifts the price
    // forces the user's swap below their floor and reverts. Allow.
    if intent.amount_out_min >= quote.expected_amount_out {
        return MevGuardOutcome::Allowed;
    }

    let gap = quote.expected_amount_out - intent.amount_out_min;
    // bps = gap / expected * 10000 — compute as u128 then cast back, saturating
    // at MAX_BPS so wildly low mins always trip the ceiling rather than wrap.
    let actual_bps_u128 = gap.saturating_mul(MAX_BPS as u128) / quote.expected_amount_out;
    let actual_bps = if actual_bps_u128 > MAX_BPS as u128 {
        MAX_BPS
    } else {
        actual_bps_u128 as u32
    };

    if actual_bps > config.max_slippage_bps {
        MevGuardOutcome::DeniedSlippage {
            actual_bps,
            max_bps: config.max_slippage_bps,
        }
    } else {
        MevGuardOutcome::Allowed
    }
}

fn is_evm_address(s: &str) -> bool {
    if s.len() != 42 {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes[0] != b'0' || (bytes[1] != b'x' && bytes[1] != b'X') {
        return false;
    }
    bytes[2..].iter().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WETH: &str = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14";
    const USDC: &str = "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238";
    const RECIPIENT: &str = "0xdc7efa1234567890abcdef1234567890abcdef12";
    const ATTACKER: &str = "0xbadc0de1234567890abcdef1234567890abcd111";

    fn cfg() -> MevGuardConfig {
        MevGuardConfig::try_new(50, [RECIPIENT.to_string()]).unwrap()
    }

    fn base_quote() -> Quote {
        // 1 WETH in → 3000 USDC out (6-decimal USDC: 3_000_000_000 wei).
        Quote {
            token_in: WETH.to_string(),
            token_out: USDC.to_string(),
            amount_in: 1_000_000_000_000_000_000,
            expected_amount_out: 3_000_000_000,
        }
    }

    fn intent_within_tolerance() -> SwapIntent {
        // amount_out_min = 2_985_000_000 = expected * (10000 - 50) / 10000
        // → exactly 50 bps slippage, which is the inclusive ceiling.
        SwapIntent {
            token_in: WETH.to_string(),
            token_out: USDC.to_string(),
            amount_in: 1_000_000_000_000_000_000,
            amount_out_min: 2_985_000_000,
            recipient: RECIPIENT.to_string(),
        }
    }

    #[test]
    fn clean_swap_within_slippage_is_allowed() {
        let outcome = evaluate(&cfg(), &base_quote(), &intent_within_tolerance());
        assert_eq!(outcome, MevGuardOutcome::Allowed);
        assert!(outcome.deny_code().is_none());
    }

    #[test]
    fn sandwich_shaped_swap_is_denied() {
        // amount_out_min = 2_700_000_000 → expected - min = 300M / 3000M = 1000 bps
        // (10% slippage). With max_slippage_bps=50, that is sandwich-vulnerable.
        let mut intent = intent_within_tolerance();
        intent.amount_out_min = 2_700_000_000;
        let outcome = evaluate(&cfg(), &base_quote(), &intent);
        match outcome {
            MevGuardOutcome::DeniedSlippage {
                actual_bps,
                max_bps,
            } => {
                assert_eq!(actual_bps, 1000);
                assert_eq!(max_bps, 50);
            }
            other => panic!("expected DeniedSlippage, got {other:?}"),
        }
        assert_eq!(
            evaluate(&cfg(), &base_quote(), &intent).deny_code(),
            Some("policy.deny_mev_slippage_too_high")
        );
    }

    #[test]
    fn off_allowlist_recipient_is_denied() {
        let mut intent = intent_within_tolerance();
        intent.recipient = ATTACKER.to_string();
        let outcome = evaluate(&cfg(), &base_quote(), &intent);
        match outcome {
            MevGuardOutcome::DeniedRecipient { ref recipient } => {
                assert_eq!(recipient, ATTACKER);
            }
            other => panic!("expected DeniedRecipient, got {other:?}"),
        }
        assert_eq!(
            outcome.deny_code(),
            Some("policy.deny_mev_recipient_off_allowlist")
        );
    }

    #[test]
    fn recipient_check_runs_before_slippage_check() {
        // Sandwich-shaped slippage AND off-allowlist recipient → must surface
        // recipient deny first so the operator sees the more severe failure
        // (recipient compromise) rather than the structural one (slippage).
        let mut intent = intent_within_tolerance();
        intent.amount_out_min = 1; // hilariously sandwich-vulnerable
        intent.recipient = ATTACKER.to_string();
        let outcome = evaluate(&cfg(), &base_quote(), &intent);
        assert!(matches!(outcome, MevGuardOutcome::DeniedRecipient { .. }));
    }

    #[test]
    fn quote_mismatch_is_denied() {
        // Caller built a quote for WETH → USDC but the intent says WETH → DAI.
        // Refuse rather than silently evaluate slippage on irrelevant numbers.
        let intent = SwapIntent {
            token_in: WETH.to_string(),
            token_out: "0x0000000000000000000000000000000000001234".to_string(),
            amount_in: 1_000_000_000_000_000_000,
            amount_out_min: 2_985_000_000,
            recipient: RECIPIENT.to_string(),
        };
        let outcome = evaluate(&cfg(), &base_quote(), &intent);
        assert_eq!(outcome, MevGuardOutcome::DeniedQuoteMismatch);
        assert_eq!(outcome.deny_code(), Some("policy.deny_mev_quote_mismatch"));
    }

    #[test]
    fn allowlist_match_is_case_insensitive() {
        // EIP-55 checksummed addresses must match against lowercased allowlist.
        let cfg = MevGuardConfig::try_new(50, [RECIPIENT.to_string()]).unwrap();
        let mut intent = intent_within_tolerance();
        intent.recipient = RECIPIENT.to_ascii_uppercase().replace('X', "x"); // 0xAB…AB
        let outcome = evaluate(&cfg, &base_quote(), &intent);
        assert_eq!(outcome, MevGuardOutcome::Allowed);
    }

    #[test]
    fn min_at_or_above_quote_is_always_allowed() {
        // amount_out_min == expected → 0 bps slippage. Front-runners cannot
        // profit; the swap reverts on any adverse price movement.
        let mut intent = intent_within_tolerance();
        intent.amount_out_min = base_quote().expected_amount_out;
        assert_eq!(
            evaluate(&cfg(), &base_quote(), &intent),
            MevGuardOutcome::Allowed
        );
        // amount_out_min above quote: even tighter floor, still allowed (will
        // simply revert on-chain if the price drifts).
        intent.amount_out_min = base_quote().expected_amount_out + 1;
        assert_eq!(
            evaluate(&cfg(), &base_quote(), &intent),
            MevGuardOutcome::Allowed
        );
    }

    #[test]
    fn config_rejects_silly_slippage() {
        let err = MevGuardConfig::try_new(5_000, []).unwrap_err();
        assert!(matches!(err, MevGuardConfigError::SlippageTooHigh(5_000)));
        let err = MevGuardConfig::try_new(20_000, []).unwrap_err();
        assert!(matches!(
            err,
            MevGuardConfigError::SlippageOutOfRange(20_000)
        ));
    }

    #[test]
    fn config_rejects_malformed_recipient() {
        let err = MevGuardConfig::try_new(50, ["not-an-address".to_string()]).unwrap_err();
        match err {
            MevGuardConfigError::BadRecipient(s) => assert_eq!(s, "not-an-address"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn empty_allowlist_denies_all_recipients() {
        let cfg = MevGuardConfig::try_new(50, []).unwrap();
        let outcome = evaluate(&cfg, &base_quote(), &intent_within_tolerance());
        assert!(matches!(outcome, MevGuardOutcome::DeniedRecipient { .. }));
    }
}
