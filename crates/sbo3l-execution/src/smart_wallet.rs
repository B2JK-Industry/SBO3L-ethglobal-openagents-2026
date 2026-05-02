//! T-5-3 — Smart Wallet abstraction with per-call policy gates.
//!
//! Track 5 deepening: agents increasingly target Smart Wallet
//! interfaces (ERC-4337 bundlers, Coinbase Universal Account,
//! Safe modules) instead of plain EOAs. The interesting policy
//! surface is the **bundle**: a single userOp can carry multiple
//! calls, each with its own intent + counterparty + gas envelope.
//!
//! Gating the bundle as one decision is the same mistake T-5-2's
//! [`crate::uniswap_router::UniversalRouterExecutor`] avoids for
//! Universal Router multicalls — an agent could bury an
//! out-of-policy `transfer` in a Smart Wallet bundle that the
//! outer policy gate doesn't introspect. T-5-3 fixes this by
//! running **two gates per call**:
//!
//! 1. **Pre-tx gate** — sees the call's `intent`, `risk_class`,
//!    target, calldata. Decides whether the call should attempt
//!    execution at all. A pre-tx deny short-circuits the entire
//!    bundle (no later call is simulated, no on-chain
//!    interaction). Mirrors the per-command pattern in
//!    `uniswap_router.rs`.
//!
//! 2. **Post-tx gate** — sees the simulated outcome: `gas_used`,
//!    revert flag, return-data length. Enforces gas-budget
//!    assertions and detects suspicious revert patterns. A
//!    post-tx deny stops the bundle from continuing but the
//!    completed calls' state changes are recorded honestly in
//!    `executor_evidence` (a userOp simulator can roll back; the
//!    on-chain bundler obviously cannot — the evidence reflects
//!    what was tried).
//!
//! Intermediate revert: a call that simulates as `reverted=true`
//! is recorded with `outcome="reverted"`; the remaining calls in
//! the bundle are NOT simulated, the bundle as a whole is
//! reported as `BundleOutcome::Reverted`, and the caller
//! receives a clean `Result` (not a panic / not a swallowed
//! error). This matches ERC-4337's userOp atomicity: the whole
//! op succeeds or the whole op reverts; partial commits aren't
//! a thing.
//!
//! # Wallet kinds
//!
//! - [`WalletKind::Eoa`] — vanilla externally-owned account. The
//!   "bundle" is a single tx; `evaluate_bundle` accepts a
//!   1-element slice. Pre-tx gate runs once, post-tx gate runs
//!   once.
//! - [`WalletKind::SmartWallet4337`] — ERC-4337 userOp model.
//!   Multi-call bundles supported. Atomic — one revert, whole
//!   userOp reverts.
//! - [`WalletKind::UniversalAccount`] — Coinbase Universal Account
//!   wrapper. Treated identically to 4337 for the policy gate
//!   shape (the differences are at the bundler / paymaster layer,
//!   below SBO3L's policy surface).
//!
//! # `executor_evidence` wire shape
//!
//! ```json
//! {
//!   "wallet_kind": "smart_wallet_4337",
//!   "bundle_calls": [
//!     {
//!       "target": "0xrouter…",
//!       "intent": "purchase_data",
//!       "risk_class": "low",
//!       "pre_decision": "allow",
//!       "post_decision": "allow",
//!       "outcome": "executed",
//!       "gas_used": 142000
//!     },
//!     {
//!       "target": "0xtoken…",
//!       "intent": "transfer",
//!       "risk_class": "high",
//!       "pre_decision": "deny",
//!       "deny_code": "policy.high_risk_transfer",
//!       "outcome": "skipped"
//!     }
//!   ],
//!   "aborted_at_index": 1,
//!   "aborted_reason": "policy.high_risk_transfer",
//!   "aborted_phase": "pre_tx"
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::json;

/// Wallet category for the call bundle. Kept as an enum rather
/// than a string so a future impl that branches on
/// gas-estimation strategy (4337 paymaster vs. plain EOA gas
/// price) can match exhaustively.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalletKind {
    Eoa,
    SmartWallet4337,
    UniversalAccount,
}

impl WalletKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Eoa => "eoa",
            Self::SmartWallet4337 => "smart_wallet_4337",
            Self::UniversalAccount => "universal_account",
        }
    }
}

/// One call in a Smart Wallet bundle. Mirrors the policy-relevant
/// fields a userOp carries — target, calldata, value, plus the
/// SBO3L-side annotations (intent, risk_class) the agent attaches
/// for policy introspection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WalletCall {
    /// EVM address — `0x` + 40 hex.
    pub target: String,
    /// Hex-encoded calldata.
    pub calldata_hex: String,
    /// Wei (string-encoded big-int — matches AppRP `Money.value`
    /// shape so a future bridge from PaymentRequest to bundle
    /// reuses the same field).
    pub value_wei: String,
    /// Agent-supplied intent. Drives the pre-tx gate's
    /// risk-class lookup.
    pub intent: String,
    /// Agent-supplied risk class. Pre-tx gate uses this to
    /// route to the right policy slot.
    pub risk_class: String,
    /// Operator's gas budget for this call (units, not gwei). The
    /// post-tx gate compares against the simulated `gas_used` and
    /// denies on overshoot. `None` = no per-call gas budget; rely
    /// on bundle-level limit (set on the executor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_budget: Option<u64>,
}

/// Verdict from a policy gate for one phase of one call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum GateVerdict {
    Allow,
    Deny { deny_code: String },
}

impl GateVerdict {
    pub fn allow() -> Self {
        Self::Allow
    }
    pub fn deny(code: impl Into<String>) -> Self {
        Self::Deny {
            deny_code: code.into(),
        }
    }
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny { .. } => "deny",
        }
    }
    fn deny_code(&self) -> Option<&str> {
        match self {
            Self::Allow => None,
            Self::Deny { deny_code } => Some(deny_code),
        }
    }
}

/// Pre-tx gate signature: sees the call before any simulation,
/// decides whether to attempt it. Caller-supplied so the daemon's
/// SBO3L policy engine, a demo canned policy, or a test fake can
/// each be plugged in.
pub type PreTxGate<'a> = dyn Fn(&WalletCall) -> GateVerdict + 'a;

/// Post-tx gate signature: sees the simulated outcome (gas used,
/// revert flag, return-data length), decides whether to accept.
/// Same closure-typed pattern as the pre-tx gate.
pub type PostTxGate<'a> = dyn Fn(&WalletCall, &SimulationOutcome) -> GateVerdict + 'a;

/// What a call simulator returned. `reverted=true` means the call
/// failed on-chain (revert in the EVM sense); a clean run is
/// `reverted=false`. Used only for the post-tx gate's input + the
/// evidence record — the executor doesn't dispatch on it directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationOutcome {
    pub gas_used: u64,
    pub reverted: bool,
    /// Hex-encoded return data. Empty string when nothing returned.
    #[serde(default)]
    pub return_data_hex: String,
    /// On revert, the on-chain reason (decoded from `Error(string)`
    /// or 4-byte selector). `None` when not reverted or not
    /// decodable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert_reason: Option<String>,
}

impl SimulationOutcome {
    pub fn ok(gas_used: u64) -> Self {
        Self {
            gas_used,
            reverted: false,
            return_data_hex: String::new(),
            revert_reason: None,
        }
    }
    pub fn revert(gas_used: u64, reason: impl Into<String>) -> Self {
        Self {
            gas_used,
            reverted: true,
            return_data_hex: String::new(),
            revert_reason: Some(reason.into()),
        }
    }
}

/// Caller-supplied simulator. Same closure-typed pattern: the
/// daemon's bundler-RPC client wires in here for live use; tests
/// inject a fake that returns canned outcomes.
pub type Simulator<'a> = dyn Fn(&WalletCall) -> SimulationOutcome + 'a;

/// Smart Wallet executor with per-call policy gates.
pub struct SmartWalletExecutor {
    wallet_kind: WalletKind,
    /// Bundle-level gas ceiling. The post-tx gate cumulates
    /// `gas_used` across calls; if the running total exceeds this,
    /// the bundle aborts. `None` = no bundle ceiling.
    bundle_gas_ceiling: Option<u64>,
}

impl SmartWalletExecutor {
    pub fn new(wallet_kind: WalletKind) -> Self {
        Self {
            wallet_kind,
            bundle_gas_ceiling: None,
        }
    }
    pub fn with_bundle_gas_ceiling(mut self, ceiling: u64) -> Self {
        self.bundle_gas_ceiling = Some(ceiling);
        self
    }
    pub fn wallet_kind(&self) -> WalletKind {
        self.wallet_kind
    }

    /// Run the bundle through both gates + the simulator. Returns
    /// a [`BundleOutcome`] capturing the per-call decisions; the
    /// caller can pull `executor_evidence` via [`Self::build_evidence`].
    ///
    /// Semantics:
    /// - Empty bundle → `Aborted { reason: "policy.empty_bundle" }`.
    /// - Pre-tx deny → bundle aborts at that call; later calls
    ///   are not simulated; outcome is `BundleOutcome::AbortedPreTx`.
    /// - Simulator reverts → call recorded with `outcome=reverted`;
    ///   later calls are NOT simulated (4337 atomicity); outcome
    ///   is `BundleOutcome::Reverted`.
    /// - Post-tx deny (gas budget overshoot, etc.) → bundle aborts
    ///   at that call (this call's state changes are still
    ///   recorded; in real execution the userOp would have
    ///   committed); outcome is `BundleOutcome::AbortedPostTx`.
    /// - All allow + no revert → `BundleOutcome::Approved`.
    pub fn evaluate_bundle(
        &self,
        calls: &[WalletCall],
        pre_tx: &PreTxGate<'_>,
        post_tx: &PostTxGate<'_>,
        simulator: &Simulator<'_>,
    ) -> BundleOutcome {
        if calls.is_empty() {
            return BundleOutcome::AbortedPreTx {
                executed: Vec::new(),
                aborted_at_index: 0,
                aborted_reason: "policy.empty_bundle".to_string(),
            };
        }
        let mut executed: Vec<EvaluatedCall> = Vec::with_capacity(calls.len());
        let mut total_gas: u64 = 0;
        for (idx, call) in calls.iter().enumerate() {
            let pre = pre_tx(call);
            if pre.is_deny() {
                let reason = pre
                    .deny_code()
                    .map(str::to_string)
                    .unwrap_or_else(|| "policy.deny".to_string());
                executed.push(EvaluatedCall {
                    call: call.clone(),
                    pre_decision: pre.label().to_string(),
                    pre_deny_code: pre.deny_code().map(str::to_string),
                    post_decision: None,
                    post_deny_code: None,
                    outcome: "skipped".to_string(),
                    gas_used: None,
                    revert_reason: None,
                });
                return BundleOutcome::AbortedPreTx {
                    executed,
                    aborted_at_index: idx,
                    aborted_reason: reason,
                };
            }

            let sim = simulator(call);
            total_gas = total_gas.saturating_add(sim.gas_used);

            // Bundle-level ceiling — fail-closed before considering
            // post-tx gate so an operator-set bundle ceiling is
            // load-bearing even when the post-tx gate is permissive.
            if let Some(ceiling) = self.bundle_gas_ceiling {
                if total_gas > ceiling {
                    executed.push(EvaluatedCall {
                        call: call.clone(),
                        pre_decision: pre.label().to_string(),
                        pre_deny_code: None,
                        post_decision: Some("deny".to_string()),
                        post_deny_code: Some("policy.bundle_gas_ceiling".to_string()),
                        outcome: if sim.reverted { "reverted" } else { "executed" }.to_string(),
                        gas_used: Some(sim.gas_used),
                        revert_reason: sim.revert_reason.clone(),
                    });
                    return BundleOutcome::AbortedPostTx {
                        executed,
                        aborted_at_index: idx,
                        aborted_reason: "policy.bundle_gas_ceiling".to_string(),
                    };
                }
            }

            // Per-call gas-budget check — runs alongside the
            // user-supplied post-tx gate so an operator can rely on
            // call.gas_budget without writing the comparison
            // themselves.
            if let Some(budget) = call.gas_budget {
                if sim.gas_used > budget {
                    executed.push(EvaluatedCall {
                        call: call.clone(),
                        pre_decision: pre.label().to_string(),
                        pre_deny_code: None,
                        post_decision: Some("deny".to_string()),
                        post_deny_code: Some("policy.gas_budget_exceeded".to_string()),
                        outcome: if sim.reverted { "reverted" } else { "executed" }.to_string(),
                        gas_used: Some(sim.gas_used),
                        revert_reason: sim.revert_reason.clone(),
                    });
                    return BundleOutcome::AbortedPostTx {
                        executed,
                        aborted_at_index: idx,
                        aborted_reason: "policy.gas_budget_exceeded".to_string(),
                    };
                }
            }

            let post = post_tx(call, &sim);
            let outcome_label = if sim.reverted { "reverted" } else { "executed" };
            executed.push(EvaluatedCall {
                call: call.clone(),
                pre_decision: pre.label().to_string(),
                pre_deny_code: None,
                post_decision: Some(post.label().to_string()),
                post_deny_code: post.deny_code().map(str::to_string),
                outcome: outcome_label.to_string(),
                gas_used: Some(sim.gas_used),
                revert_reason: sim.revert_reason.clone(),
            });

            if sim.reverted {
                return BundleOutcome::Reverted {
                    executed,
                    aborted_at_index: idx,
                    aborted_reason: format!(
                        "evm.revert: {}",
                        sim.revert_reason
                            .clone()
                            .unwrap_or_else(|| "<unknown>".to_string())
                    ),
                };
            }
            if post.is_deny() {
                let reason = post
                    .deny_code()
                    .map(str::to_string)
                    .unwrap_or_else(|| "policy.deny".to_string());
                return BundleOutcome::AbortedPostTx {
                    executed,
                    aborted_at_index: idx,
                    aborted_reason: reason,
                };
            }
        }
        BundleOutcome::Approved { executed }
    }

    pub fn build_evidence(&self, outcome: &BundleOutcome) -> serde_json::Value {
        match outcome {
            BundleOutcome::Approved { executed } => json!({
                "wallet_kind": self.wallet_kind.label(),
                "bundle_calls": executed,
                "aborted_at_index": serde_json::Value::Null,
            }),
            BundleOutcome::AbortedPreTx {
                executed,
                aborted_at_index,
                aborted_reason,
            } => json!({
                "wallet_kind": self.wallet_kind.label(),
                "bundle_calls": executed,
                "aborted_at_index": aborted_at_index,
                "aborted_reason": aborted_reason,
                "aborted_phase": "pre_tx",
            }),
            BundleOutcome::AbortedPostTx {
                executed,
                aborted_at_index,
                aborted_reason,
            } => json!({
                "wallet_kind": self.wallet_kind.label(),
                "bundle_calls": executed,
                "aborted_at_index": aborted_at_index,
                "aborted_reason": aborted_reason,
                "aborted_phase": "post_tx",
            }),
            BundleOutcome::Reverted {
                executed,
                aborted_at_index,
                aborted_reason,
            } => json!({
                "wallet_kind": self.wallet_kind.label(),
                "bundle_calls": executed,
                "aborted_at_index": aborted_at_index,
                "aborted_reason": aborted_reason,
                "aborted_phase": "evm_revert",
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedCall {
    #[serde(flatten)]
    pub call: WalletCall,
    pub pre_decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_deny_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_deny_code: Option<String>,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revert_reason: Option<String>,
}

/// Outcome of running one bundle through the executor.
#[derive(Debug, Clone)]
pub enum BundleOutcome {
    Approved {
        executed: Vec<EvaluatedCall>,
    },
    AbortedPreTx {
        executed: Vec<EvaluatedCall>,
        aborted_at_index: usize,
        aborted_reason: String,
    },
    AbortedPostTx {
        executed: Vec<EvaluatedCall>,
        aborted_at_index: usize,
        aborted_reason: String,
    },
    Reverted {
        executed: Vec<EvaluatedCall>,
        aborted_at_index: usize,
        aborted_reason: String,
    },
}

impl BundleOutcome {
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }
    pub fn is_reverted(&self) -> bool {
        matches!(self, Self::Reverted { .. })
    }
    pub fn aborted_at(&self) -> Option<usize> {
        match self {
            Self::Approved { .. } => None,
            Self::AbortedPreTx {
                aborted_at_index, ..
            }
            | Self::AbortedPostTx {
                aborted_at_index, ..
            }
            | Self::Reverted {
                aborted_at_index, ..
            } => Some(*aborted_at_index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call_purchase() -> WalletCall {
        WalletCall {
            target: "0x1111111111111111111111111111111111111111".into(),
            calldata_hex: "0xdeadbeef".into(),
            value_wei: "0".into(),
            intent: "purchase_data".into(),
            risk_class: "low".into(),
            gas_budget: Some(200_000),
        }
    }
    fn call_transfer_high_risk() -> WalletCall {
        WalletCall {
            target: "0x2222222222222222222222222222222222222222".into(),
            calldata_hex: "0xa9059cbb".into(),
            value_wei: "1000000000000000000".into(),
            intent: "transfer".into(),
            risk_class: "high".into(),
            gas_budget: Some(100_000),
        }
    }
    fn call_router_swap() -> WalletCall {
        WalletCall {
            target: "0x3333333333333333333333333333333333333333".into(),
            calldata_hex: "0x12345678".into(),
            value_wei: "0".into(),
            intent: "purchase_compute".into(),
            risk_class: "low".into(),
            gas_budget: Some(300_000),
        }
    }

    fn allow_all_pre() -> Box<PreTxGate<'static>> {
        Box::new(|_: &WalletCall| GateVerdict::allow())
    }
    fn allow_all_post() -> Box<PostTxGate<'static>> {
        Box::new(|_: &WalletCall, _: &SimulationOutcome| GateVerdict::allow())
    }
    fn ok_simulator(gas: u64) -> Box<Simulator<'static>> {
        Box::new(move |_: &WalletCall| SimulationOutcome::ok(gas))
    }

    /// Three-call bundle, mid-call EVM revert — bundle returns
    /// `Reverted`, later calls are NOT simulated, evidence records
    /// the revert reason. Mirrors the brief's "intermediate revert
    /// returns to caller cleanly" requirement.
    #[test]
    fn intermediate_revert_returns_cleanly_with_evidence() {
        let exec = SmartWalletExecutor::new(WalletKind::SmartWallet4337);
        let calls = vec![
            call_purchase(),
            call_transfer_high_risk(),
            call_router_swap(),
        ];

        let pre: Box<PreTxGate> = allow_all_pre();
        let post: Box<PostTxGate> = allow_all_post();
        let sim: Box<Simulator> = Box::new(|c: &WalletCall| {
            // Second call (the transfer) reverts; first + third
            // would simulate cleanly if called.
            if c.intent == "transfer" {
                SimulationOutcome::revert(50_000, "ERC20: transfer to zero address")
            } else {
                SimulationOutcome::ok(150_000)
            }
        });

        let outcome = exec.evaluate_bundle(&calls, pre.as_ref(), post.as_ref(), sim.as_ref());
        assert!(outcome.is_reverted(), "got {outcome:?}");
        assert_eq!(outcome.aborted_at(), Some(1));

        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["wallet_kind"], "smart_wallet_4337");
        assert_eq!(evidence["aborted_phase"], "evm_revert");
        assert_eq!(evidence["aborted_at_index"], 1);
        // Bundle calls array contains the 2 evaluated calls (the
        // one that succeeded + the reverter); the third call was
        // NOT evaluated.
        assert_eq!(evidence["bundle_calls"].as_array().unwrap().len(), 2);
        assert_eq!(evidence["bundle_calls"][0]["outcome"], "executed");
        assert_eq!(evidence["bundle_calls"][1]["outcome"], "reverted");
        assert_eq!(
            evidence["bundle_calls"][1]["revert_reason"],
            "ERC20: transfer to zero address"
        );
    }

    /// Pre-tx deny on call 2 of 3 — evidence records the first
    /// allow + the deny + skipped flag, the third call doesn't
    /// appear (not evaluated).
    #[test]
    fn pre_tx_deny_skips_remaining_calls() {
        let exec = SmartWalletExecutor::new(WalletKind::SmartWallet4337);
        let calls = vec![
            call_purchase(),
            call_transfer_high_risk(),
            call_router_swap(),
        ];

        let pre: Box<PreTxGate> = Box::new(|c: &WalletCall| match c.risk_class.as_str() {
            "high" => GateVerdict::deny("policy.high_risk_transfer"),
            _ => GateVerdict::allow(),
        });
        let post: Box<PostTxGate> = allow_all_post();
        let sim: Box<Simulator> = ok_simulator(150_000);

        let outcome = exec.evaluate_bundle(&calls, pre.as_ref(), post.as_ref(), sim.as_ref());
        assert!(!outcome.is_approved());
        assert_eq!(outcome.aborted_at(), Some(1));

        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["aborted_phase"], "pre_tx");
        assert_eq!(evidence["aborted_reason"], "policy.high_risk_transfer");
        assert_eq!(evidence["bundle_calls"].as_array().unwrap().len(), 2);
        assert_eq!(evidence["bundle_calls"][0]["outcome"], "executed");
        assert_eq!(evidence["bundle_calls"][1]["outcome"], "skipped");
        assert_eq!(evidence["bundle_calls"][1]["pre_decision"], "deny");
    }

    /// Post-tx gate denies on gas-budget overshoot via the
    /// per-call `gas_budget` field. The first call's gas_budget
    /// is 200_000; sim returns 250_000 → deny.
    #[test]
    fn per_call_gas_budget_overshoot_aborts_bundle_post_tx() {
        let exec = SmartWalletExecutor::new(WalletKind::SmartWallet4337);
        let calls = vec![call_purchase()];
        let pre: Box<PreTxGate> = allow_all_pre();
        let post: Box<PostTxGate> = allow_all_post();
        let sim: Box<Simulator> = ok_simulator(250_000);

        let outcome = exec.evaluate_bundle(&calls, pre.as_ref(), post.as_ref(), sim.as_ref());
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["aborted_phase"], "post_tx");
        assert_eq!(evidence["aborted_reason"], "policy.gas_budget_exceeded");
        assert_eq!(
            evidence["bundle_calls"][0]["post_deny_code"],
            "policy.gas_budget_exceeded"
        );
    }

    /// Bundle-level gas ceiling fires after cumulative gas
    /// crosses the limit, even if each individual call is within
    /// its own per-call budget.
    #[test]
    fn bundle_gas_ceiling_aborts_after_cumulative_overshoot() {
        let exec =
            SmartWalletExecutor::new(WalletKind::SmartWallet4337).with_bundle_gas_ceiling(200_000);
        let calls = vec![call_purchase(), call_router_swap()];
        let pre: Box<PreTxGate> = allow_all_pre();
        let post: Box<PostTxGate> = allow_all_post();
        let sim: Box<Simulator> = ok_simulator(150_000); // each, cumulative 300k

        let outcome = exec.evaluate_bundle(&calls, pre.as_ref(), post.as_ref(), sim.as_ref());
        assert!(!outcome.is_approved());
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["aborted_reason"], "policy.bundle_gas_ceiling");
        assert_eq!(evidence["aborted_at_index"], 1);
    }

    #[test]
    fn empty_bundle_aborts_with_documented_reason() {
        let exec = SmartWalletExecutor::new(WalletKind::Eoa);
        let outcome = exec.evaluate_bundle(
            &[],
            allow_all_pre().as_ref(),
            allow_all_post().as_ref(),
            ok_simulator(0).as_ref(),
        );
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["aborted_reason"], "policy.empty_bundle");
        assert_eq!(evidence["aborted_phase"], "pre_tx");
    }

    /// Happy path — every call allows, none reverts, evidence
    /// has aborted_at_index = null and every call's
    /// outcome=executed.
    #[test]
    fn fully_approved_bundle_emits_null_abort_index() {
        let exec = SmartWalletExecutor::new(WalletKind::UniversalAccount);
        let calls = vec![call_purchase(), call_router_swap()];
        let outcome = exec.evaluate_bundle(
            &calls,
            allow_all_pre().as_ref(),
            allow_all_post().as_ref(),
            ok_simulator(150_000).as_ref(),
        );
        assert!(outcome.is_approved());
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["wallet_kind"], "universal_account");
        assert_eq!(evidence["aborted_at_index"], serde_json::Value::Null);
        for c in evidence["bundle_calls"].as_array().unwrap() {
            assert_eq!(c["outcome"], "executed");
            assert_eq!(c["pre_decision"], "allow");
        }
    }

    /// EOA wallet kind serialises with the documented label —
    /// downstream consumers (passport capsule, executor_evidence
    /// readers) parse on this discriminant.
    #[test]
    fn wallet_kind_labels_are_stable() {
        assert_eq!(WalletKind::Eoa.label(), "eoa");
        assert_eq!(WalletKind::SmartWallet4337.label(), "smart_wallet_4337");
        assert_eq!(WalletKind::UniversalAccount.label(), "universal_account");
    }

    /// Wire-shape lock — pins the keys T-5-3's downstream
    /// pipelines parse. Adding a key is fine; renaming or
    /// removing one is a wire break.
    #[test]
    fn executor_evidence_shape_is_stable() {
        let exec = SmartWalletExecutor::new(WalletKind::SmartWallet4337);
        let outcome = exec.evaluate_bundle(
            &[call_purchase()],
            allow_all_pre().as_ref(),
            allow_all_post().as_ref(),
            ok_simulator(100_000).as_ref(),
        );
        let evidence = exec.build_evidence(&outcome);
        for key in ["wallet_kind", "bundle_calls", "aborted_at_index"] {
            assert!(
                evidence.get(key).is_some(),
                "executor_evidence missing key {key}; got {evidence}"
            );
        }
    }
}
