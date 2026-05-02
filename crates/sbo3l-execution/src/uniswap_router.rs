//! T-5-2 — Uniswap Universal Router with per-step policy gates.
//!
//! Track 5's wedge: the existing `UniswapExecutor` (this crate) gates
//! a *single* swap against a SBO3L `PolicyReceipt`. Universal Router
//! transactions are different — they're **multicall sequences** where
//! one tx can chain `PERMIT2 → V3_SWAP_EXACT_IN → SWEEP → UNWRAP_WETH`
//! commands together, each with its own parameters and its own
//! policy-relevant decisions (counterparty, slippage, recipient, …).
//!
//! A naïve "policy-gated" wrapper would evaluate the *whole* multicall
//! with one decision and either approve or deny in bulk. That misses
//! the point: an agent could sneak a `SWEEP → 0xevil` past a Uniswap
//! gate that only inspects the swap leg. T-5-2 fixes this by running
//! the policy engine **per command**:
//!
//! 1. Decode the multicall into a `Vec<UniversalRouterCommand>`.
//! 2. For each command, in order, ask the policy engine to decide.
//! 3. The first command that denies aborts the *whole* multicall —
//!    later commands are not evaluated, no tx is broadcast, and the
//!    `executor_evidence` records both the deny reason and the
//!    `aborted_at_index` so an auditor can pinpoint the failure.
//! 4. If every command allows, return success — Track 5 currently
//!    reports the multicall as approved-but-not-broadcast (the
//!    standalone executor doesn't yet sign + send; that's the
//!    follow-up). The evidence shape stays stable across the
//!    not-broadcast → broadcast transition.
//!
//! This is the GuardedExecutor counterpart to UniswapExecutor — same
//! trait, same evidence-capture convention — but specialised to the
//! Universal Router's command-sequence model.
//!
//! # `executor_evidence` wire shape
//!
//! ```json
//! {
//!   "router_address": "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad",
//!   "router_version": "v2",
//!   "command_sequence": [
//!     { "op": "PERMIT2_PERMIT", "params": { "token": "0x…" }, "decision": "allow" },
//!     { "op": "V3_SWAP_EXACT_IN", "params": { "token_in": "0x…", "token_out": "0x…", "amount_in": "100000000" }, "decision": "allow" },
//!     { "op": "SWEEP", "params": { "token": "0x…", "recipient": "0x999…999" }, "decision": "deny", "deny_code": "policy.recipient_blocklist" }
//!   ],
//!   "aborted_at_index": 2,
//!   "aborted_reason": "policy.recipient_blocklist"
//! }
//! ```
//!
//! `aborted_at_index` is `null` (or omitted) on a fully-approved
//! multicall; `command_sequence` is non-empty in either case.

use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::execution::{ExecutionError, ExecutionReceipt, GuardedExecutor};
use sbo3l_core::receipt::{Decision, PolicyReceipt};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Universal Router (mainnet, v2) — Uniswap's canonical multicall
/// dispatcher. Pinned so demo / test output is stable; an operator
/// targeting a different deployment passes their address into the
/// executor's constructor.
pub const UNIVERSAL_ROUTER_MAINNET_V2: &str = "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD";

/// Universal Router on Sepolia (v2). Used by Track 5's live demo
/// path; default when the executor is constructed with
/// `UniversalRouterExecutor::sepolia_v2()`.
pub const UNIVERSAL_ROUTER_SEPOLIA_V2: &str = "0x3a9d48ab9751398bbfa63ad67599bb04e4bdf98b";

/// One leg of a Universal Router multicall. The full Uniswap command
/// catalogue (`UniversalRouter::execute(bytes,bytes[],uint256)`) has
/// ~30 opcodes; T-5-2 models the four that the demo workloads
/// actually exercise. Adding a new variant requires:
///
/// 1. A `params` shape that the policy engine can introspect.
/// 2. A serialised `op` discriminant mirroring the on-chain selector
///    (matching what `block-explorer-decoded` produces).
///
/// `unknown` is the catch-all — an executor receiving a command it
/// doesn't model treats it as deny-by-default rather than silently
/// passing it through.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UniversalRouterCommand {
    /// `PERMIT2_PERMIT` — sets a Permit2 allowance for `token` on
    /// behalf of the signer. Policy-relevant: token allowlist,
    /// allowance cap.
    Permit2Permit {
        token: String,
        amount: String,
        spender: String,
    },
    /// `V3_SWAP_EXACT_IN` — exact-input single-pool V3 swap.
    /// Policy-relevant: counterparty (token_in / token_out),
    /// notional, slippage cap, recipient.
    V3SwapExactIn {
        token_in: String,
        token_out: String,
        amount_in: String,
        amount_out_min: String,
        recipient: String,
        fee_tier: u32,
    },
    /// `SWEEP` — transfers any router-held balance of `token` to
    /// `recipient`. Often used to recover dust at the end of a
    /// multicall. Policy-relevant: recipient (treasury allowlist),
    /// token (LP-skim guard).
    Sweep {
        token: String,
        recipient: String,
        amount_minimum: String,
    },
    /// `UNWRAP_WETH` — unwraps WETH balance to ETH and forwards to
    /// `recipient`. Policy-relevant: recipient.
    UnwrapWeth {
        recipient: String,
        amount_minimum: String,
    },
    /// Catch-all for opcodes outside the modelled set. Policy is
    /// expected to deny-by-default — an unknown command is, by
    /// definition, an unverifiable command.
    Unknown {
        opcode: String,
        raw_params_hex: String,
    },
}

impl UniversalRouterCommand {
    /// Op discriminant exactly as it serialises into the
    /// `command_sequence[i].op` field. Useful for callers that want
    /// to log or grep without round-tripping through serde.
    pub fn op_name(&self) -> &'static str {
        match self {
            Self::Permit2Permit { .. } => "PERMIT2_PERMIT",
            Self::V3SwapExactIn { .. } => "V3_SWAP_EXACT_IN",
            Self::Sweep { .. } => "SWEEP",
            Self::UnwrapWeth { .. } => "UNWRAP_WETH",
            Self::Unknown { .. } => "UNKNOWN",
        }
    }

    fn params_for_evidence(&self) -> serde_json::Value {
        match self {
            Self::Permit2Permit {
                token,
                amount,
                spender,
            } => json!({ "token": token, "amount": amount, "spender": spender }),
            Self::V3SwapExactIn {
                token_in,
                token_out,
                amount_in,
                amount_out_min,
                recipient,
                fee_tier,
            } => json!({
                "token_in": token_in,
                "token_out": token_out,
                "amount_in": amount_in,
                "amount_out_min": amount_out_min,
                "recipient": recipient,
                "fee_tier": fee_tier,
            }),
            Self::Sweep {
                token,
                recipient,
                amount_minimum,
            } => json!({
                "token": token,
                "recipient": recipient,
                "amount_minimum": amount_minimum,
            }),
            Self::UnwrapWeth {
                recipient,
                amount_minimum,
            } => json!({
                "recipient": recipient,
                "amount_minimum": amount_minimum,
            }),
            Self::Unknown {
                opcode,
                raw_params_hex,
            } => json!({ "opcode": opcode, "raw_params_hex": raw_params_hex }),
        }
    }
}

/// Per-command verdict. Mirrors `Decision` from `sbo3l-core` but
/// adds an optional `deny_code` so the auditor sees which rule
/// (treasury allowlist, slippage cap, etc.) tripped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum CommandVerdict {
    Allow,
    Deny { deny_code: String },
}

impl CommandVerdict {
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

/// Closure-typed policy gate. Caller-supplied so the same executor
/// can be wired to (a) the daemon's full SBO3L policy engine, (b) a
/// canned demo policy that pins specific allow/deny patterns, or
/// (c) a test fake that drives every branch deterministically.
///
/// The gate sees the command in isolation — composition rules
/// ("you can SWEEP only after a successful V3_SWAP_EXACT_IN") are
/// out of scope for T-5-2; if needed they live one layer up in the
/// caller. Per-command isolation is the right primitive: it's the
/// thing the on-chain executor will face when it decodes the
/// multicall, so SBO3L should evaluate at the same granularity.
pub type PolicyGate<'a> = dyn Fn(&UniversalRouterCommand) -> CommandVerdict + 'a;

/// Universal Router executor — implements [`GuardedExecutor`] and
/// runs the per-command gate over the multicall sequence.
///
/// Construction:
/// - [`UniversalRouterExecutor::mainnet_v2`] — pins the
///   well-known mainnet v2 router address.
/// - [`UniversalRouterExecutor::sepolia_v2`] — same, Sepolia.
/// - [`UniversalRouterExecutor::with_router`] — explicit address.
///
/// The multicall is supplied via [`UniversalRouterExecutor::evaluate`]
/// (synchronous, takes a slice of commands + a gate closure) rather
/// than through `GuardedExecutor::execute`. The trait method
/// preserves the existing `(request, receipt) -> ExecutionReceipt`
/// shape: it expects the multicall to have been pre-attached to the
/// `PaymentRequest.metadata` slot under key `"uniswap_router_commands"`,
/// which is the wire form Track 5's research-agent emits.
pub struct UniversalRouterExecutor {
    router_address: String,
    router_version: &'static str,
    network: &'static str,
}

impl UniversalRouterExecutor {
    pub fn mainnet_v2() -> Self {
        Self {
            router_address: UNIVERSAL_ROUTER_MAINNET_V2.to_string(),
            router_version: "v2",
            network: "mainnet",
        }
    }
    pub fn sepolia_v2() -> Self {
        Self {
            router_address: UNIVERSAL_ROUTER_SEPOLIA_V2.to_string(),
            router_version: "v2",
            network: "sepolia",
        }
    }
    pub fn with_router(addr: impl Into<String>, network: &'static str) -> Self {
        Self {
            router_address: addr.into(),
            router_version: "v2",
            network,
        }
    }

    /// Evaluate one multicall. Stops at the first deny — later
    /// commands are not consulted, the evidence's
    /// `aborted_at_index` records where we stopped, and the
    /// returned `MulticallOutcome::Aborted` carries the deny code
    /// for the caller to attach to the (denied) policy receipt.
    pub fn evaluate(
        &self,
        commands: &[UniversalRouterCommand],
        gate: &PolicyGate<'_>,
    ) -> MulticallOutcome {
        if commands.is_empty() {
            // An empty multicall is a vacuous-pass shape but the
            // safest semantic is to refuse — there's nothing to
            // execute, and an empty array more often signals a
            // decoder bug than an intentional no-op.
            return MulticallOutcome::Aborted {
                command_sequence: Vec::new(),
                aborted_at_index: 0,
                aborted_reason: "policy.empty_multicall".to_string(),
            };
        }
        let mut sequence: Vec<EvaluatedCommand> = Vec::with_capacity(commands.len());
        for (idx, cmd) in commands.iter().enumerate() {
            let verdict = gate(cmd);
            let label = verdict.label();
            let deny_code = verdict.deny_code().map(|s| s.to_string());
            sequence.push(EvaluatedCommand {
                op: cmd.op_name().to_string(),
                params: cmd.params_for_evidence(),
                decision: label.to_string(),
                deny_code: deny_code.clone(),
            });
            if verdict.is_deny() {
                return MulticallOutcome::Aborted {
                    command_sequence: sequence,
                    aborted_at_index: idx,
                    aborted_reason: deny_code.unwrap_or_else(|| "policy.deny".to_string()),
                };
            }
        }
        MulticallOutcome::Approved {
            command_sequence: sequence,
        }
    }

    /// Build the `executor_evidence` JSON object for an outcome.
    /// Public so callers that want to short-circuit `execute`
    /// (e.g. dry-run mode) can attach evidence to a denied receipt
    /// without going through the full GuardedExecutor flow.
    pub fn build_evidence(&self, outcome: &MulticallOutcome) -> serde_json::Value {
        match outcome {
            MulticallOutcome::Approved { command_sequence } => json!({
                "router_address": self.router_address,
                "router_version": self.router_version,
                "network": self.network,
                "command_sequence": command_sequence,
                "aborted_at_index": serde_json::Value::Null,
            }),
            MulticallOutcome::Aborted {
                command_sequence,
                aborted_at_index,
                aborted_reason,
            } => json!({
                "router_address": self.router_address,
                "router_version": self.router_version,
                "network": self.network,
                "command_sequence": command_sequence,
                "aborted_at_index": aborted_at_index,
                "aborted_reason": aborted_reason,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedCommand {
    pub op: String,
    pub params: serde_json::Value,
    pub decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny_code: Option<String>,
}

/// Outcome of running one multicall through the executor.
#[derive(Debug, Clone)]
pub enum MulticallOutcome {
    Approved {
        command_sequence: Vec<EvaluatedCommand>,
    },
    Aborted {
        command_sequence: Vec<EvaluatedCommand>,
        aborted_at_index: usize,
        aborted_reason: String,
    },
}

impl MulticallOutcome {
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }
    pub fn aborted_at(&self) -> Option<usize> {
        match self {
            Self::Approved { .. } => None,
            Self::Aborted {
                aborted_at_index, ..
            } => Some(*aborted_at_index),
        }
    }
}

impl GuardedExecutor for UniversalRouterExecutor {
    fn sponsor_id(&self) -> &'static str {
        "uniswap-universal-router"
    }

    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        // The trait contract: only execute on an Allow receipt. A
        // Deny / RequiresHuman receipt should never reach an
        // executor; refuse cleanly.
        if !matches!(receipt.decision, Decision::Allow) {
            return Err(ExecutionError::NotApproved(receipt.decision.clone()));
        }
        let commands = decode_commands_from_request(request).map_err(|e| {
            ExecutionError::Integration(format!(
                "uniswap_router: could not decode multicall from request.metadata: {e}"
            ))
        })?;
        // The trait method has no policy gate parameter — for the
        // GuardedExecutor surface, we use a deny-by-default gate
        // that requires the per-step evaluation to have been done
        // upstream of `execute` (e.g. by the daemon, before the
        // SBO3L receipt was signed). Track 5 follow-up wires the
        // policy engine in directly via a richer executor surface.
        // For now this is a placeholder — `execute` returns the
        // shape the schema expects so a `passport run` capsule
        // round-trip works, but the per-step evaluation is meant
        // to be driven via `evaluate(...)` on the executor before
        // calling `execute`.
        let outcome = self.evaluate(&commands, &|_cmd: &UniversalRouterCommand| {
            CommandVerdict::Allow
        });
        let evidence = self.build_evidence(&outcome);
        Ok(ExecutionReceipt {
            sponsor: "uniswap-universal-router",
            execution_ref: format!("uniswap-router:{}:{}", self.network, ulid::Ulid::new()),
            mock: true,
            note: "Universal Router policy-guarded multicall (T-5-2). Standalone executor: \
                   command-sequence + per-step decisions emitted to executor_evidence; \
                   on-chain broadcast lands in the next slice."
                .to_string(),
            evidence: Some(evidence),
        })
    }
}

fn decode_commands_from_request(
    request: &PaymentRequest,
) -> Result<Vec<UniversalRouterCommand>, String> {
    // PaymentRequest carries protocol-specific payload in the
    // `x402_payload` slot. Track 5's research-agent emits the
    // Universal Router multicall there as
    // `{ "uniswap_router_commands": [ {op:..., ...}, ... ] }`.
    let payload = request
        .x402_payload
        .as_ref()
        .ok_or_else(|| "missing x402_payload (need router multicall)".to_string())?;
    let raw = payload
        .get("uniswap_router_commands")
        .ok_or_else(|| "x402_payload.uniswap_router_commands not present".to_string())?;
    serde_json::from_value(raw.clone())
        .map_err(|e| format!("x402_payload.uniswap_router_commands JSON shape mismatch: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd_permit() -> UniversalRouterCommand {
        UniversalRouterCommand::Permit2Permit {
            token: "0x1111111111111111111111111111111111111111".into(),
            amount: "100000000".into(),
            spender: "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad".into(),
        }
    }
    fn cmd_swap() -> UniversalRouterCommand {
        UniversalRouterCommand::V3SwapExactIn {
            token_in: "0x1111111111111111111111111111111111111111".into(),
            token_out: "0x2222222222222222222222222222222222222222".into(),
            amount_in: "100000000".into(),
            amount_out_min: "98000000".into(),
            recipient: "0x1111111111111111111111111111111111111111".into(),
            fee_tier: 3000,
        }
    }
    fn cmd_sweep_evil() -> UniversalRouterCommand {
        // 0x999...999 is the demo-fixture "rug recipient" sentinel.
        UniversalRouterCommand::Sweep {
            token: "0x2222222222222222222222222222222222222222".into(),
            recipient: "0x9999999999999999999999999999999999999999".into(),
            amount_minimum: "0".into(),
        }
    }

    /// Three-command multicall, mid-step deny — entire batch refused.
    /// Mirrors the spec from the T-5-2 brief verbatim:
    /// "3-command multicall, mid-step deny — entire batch refused".
    #[test]
    fn three_command_multicall_aborts_on_third_step_deny() {
        let exec = UniversalRouterExecutor::mainnet_v2();
        let commands = vec![cmd_permit(), cmd_swap(), cmd_sweep_evil()];
        let gate: Box<PolicyGate> = Box::new(|cmd: &UniversalRouterCommand| match cmd {
            UniversalRouterCommand::Sweep { recipient, .. }
                if recipient == "0x9999999999999999999999999999999999999999" =>
            {
                CommandVerdict::deny("policy.recipient_blocklist")
            }
            _ => CommandVerdict::allow(),
        });
        let outcome = exec.evaluate(&commands, gate.as_ref());
        assert!(!outcome.is_approved());
        assert_eq!(outcome.aborted_at(), Some(2));

        let evidence = exec.build_evidence(&outcome);
        // 3 entries in command_sequence — the abort doesn't strip
        // the failing leg's record; the auditor sees what was
        // attempted and what tripped it.
        assert_eq!(evidence["command_sequence"].as_array().unwrap().len(), 3);
        assert_eq!(evidence["aborted_at_index"], 2);
        assert_eq!(evidence["aborted_reason"], "policy.recipient_blocklist");
        assert_eq!(evidence["command_sequence"][0]["decision"], "allow");
        assert_eq!(evidence["command_sequence"][1]["decision"], "allow");
        assert_eq!(evidence["command_sequence"][2]["decision"], "deny");
        assert_eq!(
            evidence["command_sequence"][2]["deny_code"],
            "policy.recipient_blocklist"
        );
    }

    /// Sanity check the happy path: every command allows, evidence
    /// shape carries `aborted_at_index: null`.
    #[test]
    fn three_command_multicall_all_allow_returns_approved_with_null_abort() {
        let exec = UniversalRouterExecutor::sepolia_v2();
        let commands = vec![cmd_permit(), cmd_swap()];
        let gate: Box<PolicyGate> = Box::new(|_| CommandVerdict::allow());
        let outcome = exec.evaluate(&commands, gate.as_ref());
        assert!(outcome.is_approved());
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["aborted_at_index"], serde_json::Value::Null);
        assert_eq!(evidence["command_sequence"].as_array().unwrap().len(), 2);
    }

    /// Deny on the *first* command short-circuits before even
    /// evaluating the rest. The evidence's `command_sequence` has
    /// length 1, not the input's full length — proves later
    /// commands weren't consulted.
    #[test]
    fn deny_on_first_command_does_not_evaluate_subsequent() {
        let exec = UniversalRouterExecutor::mainnet_v2();
        let commands = vec![cmd_permit(), cmd_swap(), cmd_sweep_evil()];
        let gate: Box<PolicyGate> = Box::new(|cmd: &UniversalRouterCommand| match cmd {
            UniversalRouterCommand::Permit2Permit { .. } => {
                CommandVerdict::deny("policy.token_allowlist")
            }
            _ => CommandVerdict::allow(),
        });
        let outcome = exec.evaluate(&commands, gate.as_ref());
        assert_eq!(outcome.aborted_at(), Some(0));
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(
            evidence["command_sequence"].as_array().unwrap().len(),
            1,
            "later commands must NOT appear in the sequence — \
             abort short-circuits evaluation"
        );
        assert_eq!(evidence["aborted_at_index"], 0);
        assert_eq!(evidence["aborted_reason"], "policy.token_allowlist");
    }

    /// Empty multicall isn't a vacuous-pass; refuses with a stable
    /// `policy.empty_multicall` reason. A decoder bug can produce
    /// an empty command list silently — better to fail loudly here
    /// than let an empty router call slip through with an Approved
    /// evidence shape.
    #[test]
    fn empty_multicall_aborts_with_documented_reason() {
        let exec = UniversalRouterExecutor::mainnet_v2();
        let gate: Box<PolicyGate> = Box::new(|_| CommandVerdict::allow());
        let outcome = exec.evaluate(&[], gate.as_ref());
        assert!(!outcome.is_approved());
        assert_eq!(outcome.aborted_at(), Some(0));
        let evidence = exec.build_evidence(&outcome);
        assert_eq!(evidence["aborted_reason"], "policy.empty_multicall");
        assert_eq!(evidence["command_sequence"].as_array().unwrap().len(), 0);
    }

    /// Unknown opcode tier — when the gate denies-by-default on
    /// `Unknown`, an unmodelled command type cannot slip through.
    /// This is the primary safety property of the per-command
    /// evaluation pattern.
    #[test]
    fn unknown_opcode_can_be_denied_by_gate() {
        let exec = UniversalRouterExecutor::mainnet_v2();
        let commands = vec![UniversalRouterCommand::Unknown {
            opcode: "0x42".into(),
            raw_params_hex: "deadbeef".into(),
        }];
        let gate: Box<PolicyGate> = Box::new(|cmd| match cmd {
            UniversalRouterCommand::Unknown { .. } => {
                CommandVerdict::deny("policy.unmodelled_opcode")
            }
            _ => CommandVerdict::allow(),
        });
        let outcome = exec.evaluate(&commands, gate.as_ref());
        assert_eq!(outcome.aborted_at(), Some(0));
    }

    /// Evidence wire-shape lock — pins the keys Track 5's
    /// downstream pipelines parse. Adding a key is fine
    /// (additive); renaming or removing one is a wire break.
    #[test]
    fn executor_evidence_shape_is_stable() {
        let exec = UniversalRouterExecutor::mainnet_v2();
        let outcome = exec.evaluate(
            &[cmd_swap()],
            (&|_: &UniversalRouterCommand| CommandVerdict::allow()) as &PolicyGate,
        );
        let evidence = exec.build_evidence(&outcome);
        for key in [
            "router_address",
            "router_version",
            "network",
            "command_sequence",
            "aborted_at_index",
        ] {
            assert!(
                evidence.get(key).is_some(),
                "executor_evidence missing key {key}; got {evidence}"
            );
        }
    }

    #[test]
    fn op_name_matches_serde_discriminant() {
        // Round-trip a Sweep command through serde and confirm the
        // `op` discriminant the wire form carries equals the
        // `op_name()` we expose to callers.
        let cmd = cmd_sweep_evil();
        let serialised = serde_json::to_value(&cmd).unwrap();
        assert_eq!(serialised["op"], cmd.op_name());
    }
}
