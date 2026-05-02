//! Token-gated agent identity (Phase 3.5).
//!
//! Conditionally accept an agent registration based on the
//! prospective operator's on-chain holdings. Three concrete shapes:
//!
//! 1. **ERC-721 ownership** — the operator owns at least one NFT
//!    in a designated collection (any token of that collection
//!    qualifies), OR a specific tokenId. Implemented as
//!    [`Erc721Gate`].
//! 2. **ERC-1155 balance** — the operator's `balanceOf(owner,
//!    tokenId)` meets a minimum threshold. Implemented as
//!    [`Erc1155Gate`].
//! 3. **Composite gates** — `AnyOf` (at least one passes) and
//!    `AllOf` (every gate passes) via [`AnyOfGates`] /
//!    [`AllOfGates`]. Composition is intentionally restricted to
//!    these two combinators so a misconfigured policy can't
//!    silently underspecify the trust boundary.
//!
//! ## Risk classes
//!
//! Three pinned configurations cover the common ranges:
//!
//! | Class    | Composition                                                   |
//! |----------|---------------------------------------------------------------|
//! | Low      | Any single ERC-721 ownership in a designated wide collection. |
//! | Medium   | A specific ERC-1155 with a documented minimum balance.        |
//! | High     | `AllOf` two ERC-721 gates (e.g. team membership + holder NFT).|
//!
//! Each risk class is documented + hand-built in
//! [`risk_class_low`], [`risk_class_medium`], [`risk_class_high`]
//! so an operator can lift the helper directly into a deployment
//! script without re-deriving the gate composition.
//!
//! ## Why not on-chain?
//!
//! The gate is *off-chain*: SBO3L's daemon checks the holdings
//! before accepting an APRP that registers a new agent. This is
//! deliberate — the on-chain ENS Registry doesn't know what a
//! "qualifying NFT" is, and forcing the gate on-chain would
//! require either a bespoke registrar contract or an
//! ERC-1155-aware ENS resolver. Off-chain checks composes with
//! existing ENS infrastructure: the operator passes the
//! gate check, then runs the standard `setSubnodeRecord` flow.
//!
//! ## Trust model
//!
//! The verifier is the SBO3L instance running the gate. A
//! malicious instance could lie about gate results, but every
//! decision is audit-chained alongside the agent registration —
//! a third party reading the audit log can re-derive the gate
//! check from the on-chain state at the captured block height
//! (`evidence_block`) and confirm the verdict. The gate is a
//! *commitment*, not a black box.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ens_live::{JsonRpcTransport, RpcError};

/// `ownerOf(uint256)` selector — `keccak256("ownerOf(uint256)")[..4]`.
pub const ERC721_OWNER_OF_SELECTOR: [u8; 4] = [0x63, 0x52, 0x21, 0x1e];

/// `balanceOf(address)` selector (ERC-721) — `keccak256("balanceOf(address)")[..4]`.
pub const ERC721_BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// `balanceOf(address,uint256)` selector (ERC-1155) — `keccak256("balanceOf(address,uint256)")[..4]`.
pub const ERC1155_BALANCE_OF_SELECTOR: [u8; 4] = [0x00, 0xfd, 0xd5, 0x8e];

/// Token-gate errors. Surfaces enough context for the audit-log
/// row that records the gate check to be self-explanatory.
#[derive(Debug, Error)]
pub enum GateError {
    #[error(transparent)]
    Rpc(#[from] RpcError),

    #[error("malformed owner address: {0}")]
    MalformedOwner(String),

    #[error("malformed contract address: {0}")]
    MalformedContract(String),

    #[error("ABI decode error: {0}")]
    AbiDecode(String),

    #[error("composite gate is empty (no inner gates)")]
    EmptyComposite,
}

/// Result of running a gate check. Carries enough breadcrumb to
/// audit-log the decision: the operator's address, the contract
/// queried, and the underlying numeric or address result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub passed: bool,
    pub gate_label: String,
    /// `0x`-prefixed lowercase hex address checked.
    pub owner: String,
    pub contract: String,
    /// Free-form supporting evidence — e.g. `"balance=3"` or
    /// `"owner=0xabc…"`. Audit-log rows include this verbatim.
    pub evidence: String,
}

/// One token-gate check. Implementations consume a
/// [`JsonRpcTransport`] so the same code path runs against a real
/// chain (production) and a fake transport (tests).
pub trait TokenGate {
    fn check(&self, owner: &str) -> Result<GateResult, GateError>;
    fn label(&self) -> &str;
}

/// ERC-721 ownership check. Two operating modes:
///
/// - `token_id = Some(n)`: the operator owns the specific token n
///   (queries `ownerOf(n)` and compares). Use this when one
///   particular NFT in the collection is the credential.
/// - `token_id = None`: the operator owns at least one token in
///   the collection (queries `balanceOf(owner)` ≥ 1). Use this
///   when collection membership alone is the credential.
pub struct Erc721Gate<T: JsonRpcTransport> {
    pub label: String,
    pub contract: String,
    pub token_id: Option<u64>,
    transport: T,
}

impl<T: JsonRpcTransport> Erc721Gate<T> {
    pub fn new(transport: T, label: impl Into<String>, contract: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            contract: contract.into(),
            token_id: None,
            transport,
        }
    }

    pub fn with_token_id(mut self, token_id: u64) -> Self {
        self.token_id = Some(token_id);
        self
    }
}

impl<T: JsonRpcTransport> TokenGate for Erc721Gate<T> {
    fn check(&self, owner: &str) -> Result<GateResult, GateError> {
        let owner_normalised = normalise_address(owner)?;
        let contract_normalised = normalise_address(&self.contract)?;

        if let Some(token_id) = self.token_id {
            // Specific token: ownerOf(token_id) == owner ?
            let calldata = encode_owner_of(token_id);
            let raw = self.transport.eth_call(
                &contract_normalised,
                &format!("0x{}", hex::encode(&calldata)),
            )?;
            let owner_returned = decode_address_response(&raw)?;
            let passed = addr_eq(&owner_returned, &owner_normalised);
            Ok(GateResult {
                passed,
                gate_label: self.label.clone(),
                owner: owner_normalised,
                contract: contract_normalised,
                evidence: format!("token_id={token_id}, ownerOf={owner_returned}"),
            })
        } else {
            // Collection membership: balanceOf(owner) >= 1
            let calldata = encode_balance_of_address(&owner_normalised)?;
            let raw = self.transport.eth_call(
                &contract_normalised,
                &format!("0x{}", hex::encode(&calldata)),
            )?;
            let balance = decode_uint256_response(&raw)?;
            let passed = balance >= 1;
            Ok(GateResult {
                passed,
                gate_label: self.label.clone(),
                owner: owner_normalised,
                contract: contract_normalised,
                evidence: format!("balanceOf={balance}"),
            })
        }
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// ERC-1155 balance check: `balanceOf(owner, token_id) >= min_balance`.
pub struct Erc1155Gate<T: JsonRpcTransport> {
    pub label: String,
    pub contract: String,
    pub token_id: u64,
    pub min_balance: u128,
    transport: T,
}

impl<T: JsonRpcTransport> Erc1155Gate<T> {
    pub fn new(
        transport: T,
        label: impl Into<String>,
        contract: impl Into<String>,
        token_id: u64,
        min_balance: u128,
    ) -> Self {
        Self {
            label: label.into(),
            contract: contract.into(),
            token_id,
            min_balance,
            transport,
        }
    }
}

impl<T: JsonRpcTransport> TokenGate for Erc1155Gate<T> {
    fn check(&self, owner: &str) -> Result<GateResult, GateError> {
        let owner_normalised = normalise_address(owner)?;
        let contract_normalised = normalise_address(&self.contract)?;

        let calldata = encode_erc1155_balance_of(&owner_normalised, self.token_id)?;
        let raw = self.transport.eth_call(
            &contract_normalised,
            &format!("0x{}", hex::encode(&calldata)),
        )?;
        let balance = decode_uint256_response(&raw)?;
        let passed = balance >= self.min_balance;
        Ok(GateResult {
            passed,
            gate_label: self.label.clone(),
            owner: owner_normalised,
            contract: contract_normalised,
            evidence: format!(
                "token_id={}, balance={}, threshold={}",
                self.token_id, balance, self.min_balance
            ),
        })
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// `AnyOf` composite — passes if at least one inner gate passes.
/// Returns the first passing gate's [`GateResult`]; if every gate
/// fails, returns the last failing one (so the audit log has the
/// most-recently-checked evidence). Empty composite is rejected
/// at construction time.
pub struct AnyOfGates {
    pub label: String,
    pub gates: Vec<Box<dyn TokenGate>>,
}

impl AnyOfGates {
    pub fn new(
        label: impl Into<String>,
        gates: Vec<Box<dyn TokenGate>>,
    ) -> Result<Self, GateError> {
        if gates.is_empty() {
            return Err(GateError::EmptyComposite);
        }
        Ok(Self {
            label: label.into(),
            gates,
        })
    }
}

impl TokenGate for AnyOfGates {
    fn check(&self, owner: &str) -> Result<GateResult, GateError> {
        let mut last_failure: Option<GateResult> = None;
        for gate in &self.gates {
            let r = gate.check(owner)?;
            if r.passed {
                return Ok(GateResult {
                    passed: true,
                    gate_label: format!("{} via {}", self.label, r.gate_label),
                    owner: r.owner,
                    contract: r.contract,
                    evidence: r.evidence,
                });
            }
            last_failure = Some(r);
        }
        let r = last_failure.expect("non-empty composite always produces at least one result");
        Ok(GateResult {
            passed: false,
            gate_label: format!("{} ({} branches all failed)", self.label, self.gates.len()),
            owner: r.owner,
            contract: r.contract,
            evidence: r.evidence,
        })
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// `AllOf` composite — passes only if every inner gate passes.
/// Returns the first failing gate's [`GateResult`] for fast
/// debuggability; if every gate passes, returns a synthetic
/// success result summarising the composite. Empty composite is
/// rejected at construction time.
pub struct AllOfGates {
    pub label: String,
    pub gates: Vec<Box<dyn TokenGate>>,
}

impl AllOfGates {
    pub fn new(
        label: impl Into<String>,
        gates: Vec<Box<dyn TokenGate>>,
    ) -> Result<Self, GateError> {
        if gates.is_empty() {
            return Err(GateError::EmptyComposite);
        }
        Ok(Self {
            label: label.into(),
            gates,
        })
    }
}

impl TokenGate for AllOfGates {
    fn check(&self, owner: &str) -> Result<GateResult, GateError> {
        let mut summaries: Vec<String> = Vec::with_capacity(self.gates.len());
        let mut contracts: Vec<String> = Vec::with_capacity(self.gates.len());
        let mut owner_normalised = owner.to_string();
        for gate in &self.gates {
            let r = gate.check(owner)?;
            owner_normalised = r.owner.clone();
            if !r.passed {
                return Ok(GateResult {
                    passed: false,
                    gate_label: format!("{} (failed at {})", self.label, r.gate_label),
                    owner: r.owner,
                    contract: r.contract,
                    evidence: r.evidence,
                });
            }
            if !r.contract.is_empty() {
                contracts.push(r.contract.clone());
            }
            summaries.push(format!("{}: {}", r.gate_label, r.evidence));
        }
        Ok(GateResult {
            passed: true,
            gate_label: format!("{} ({} branches all passed)", self.label, self.gates.len()),
            owner: owner_normalised,
            // Aggregate every queried contract address so auditors
            // re-verifying a high-risk multi-contract decision can see
            // exactly which contracts were checked. Failure path
            // already preserves the failing branch's contract above.
            contract: contracts.join(", "),
            evidence: summaries.join("; "),
        })
    }

    fn label(&self) -> &str {
        &self.label
    }
}

// ============================================================
// Risk-class helpers (operator-facing presets)
// ============================================================

/// Risk class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskClass {
    Low,
    Medium,
    High,
}

/// Low-risk preset: a single ERC-721 collection-membership gate.
/// Suitable for "anyone in this DAO can register an agent of this
/// class" policies. Caller supplies the collection address +
/// human-readable label.
pub fn risk_class_low<T: JsonRpcTransport>(
    transport: T,
    label: impl Into<String>,
    collection_address: impl Into<String>,
) -> Erc721Gate<T> {
    Erc721Gate::new(transport, label, collection_address)
}

/// Medium-risk preset: an ERC-1155 balance gate with a documented
/// minimum threshold. Suitable for "this credential token must
/// be held with a non-trivial balance" — e.g. a multi-class NFT
/// where balance encodes role tier.
pub fn risk_class_medium<T: JsonRpcTransport>(
    transport: T,
    label: impl Into<String>,
    contract_address: impl Into<String>,
    token_id: u64,
    min_balance: u128,
) -> Erc1155Gate<T> {
    Erc1155Gate::new(transport, label, contract_address, token_id, min_balance)
}

/// High-risk preset: an `AllOf` composite of two ERC-721 gates.
/// Suitable for "team membership AND holder credential both
/// required" policies — e.g. a SAFE multisig signer NFT plus a
/// project-team allowlist NFT.
pub fn risk_class_high<T1: JsonRpcTransport + 'static, T2: JsonRpcTransport + 'static>(
    label: impl Into<String>,
    gate_a: Erc721Gate<T1>,
    gate_b: Erc721Gate<T2>,
) -> Result<AllOfGates, GateError> {
    AllOfGates::new(label, vec![Box::new(gate_a), Box::new(gate_b)])
}

// ============================================================
// ABI encoding helpers
// ============================================================

fn encode_owner_of(token_id: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32);
    out.extend_from_slice(&ERC721_OWNER_OF_SELECTOR);
    out.extend_from_slice(&u256_be(token_id));
    out
}

fn encode_balance_of_address(owner_hex: &str) -> Result<Vec<u8>, GateError> {
    let owner = parse_address(owner_hex)?;
    let mut out = Vec::with_capacity(4 + 32);
    out.extend_from_slice(&ERC721_BALANCE_OF_SELECTOR);
    let mut padded = [0u8; 32];
    padded[12..32].copy_from_slice(&owner);
    out.extend_from_slice(&padded);
    Ok(out)
}

fn encode_erc1155_balance_of(owner_hex: &str, token_id: u64) -> Result<Vec<u8>, GateError> {
    let owner = parse_address(owner_hex)?;
    let mut out = Vec::with_capacity(4 + 64);
    out.extend_from_slice(&ERC1155_BALANCE_OF_SELECTOR);
    let mut padded = [0u8; 32];
    padded[12..32].copy_from_slice(&owner);
    out.extend_from_slice(&padded);
    out.extend_from_slice(&u256_be(token_id));
    Ok(out)
}

fn decode_address_response(hex_response: &str) -> Result<String, GateError> {
    let stripped = hex_response
        .strip_prefix("0x")
        .or_else(|| hex_response.strip_prefix("0X"))
        .unwrap_or(hex_response);
    let bytes = hex::decode(stripped).map_err(|e| GateError::AbiDecode(format!("hex: {e}")))?;
    if bytes.len() < 32 {
        return Err(GateError::AbiDecode(format!(
            "address response too short: {} bytes",
            bytes.len()
        )));
    }
    if !bytes[..12].iter().all(|&b| b == 0) {
        return Err(GateError::AbiDecode(
            "address response: leading 12 bytes must be zero".into(),
        ));
    }
    Ok(format!("0x{}", hex::encode(&bytes[12..32])))
}

fn decode_uint256_response(hex_response: &str) -> Result<u128, GateError> {
    let stripped = hex_response
        .strip_prefix("0x")
        .or_else(|| hex_response.strip_prefix("0X"))
        .unwrap_or(hex_response);
    let bytes = hex::decode(stripped).map_err(|e| GateError::AbiDecode(format!("hex: {e}")))?;
    if bytes.len() < 32 {
        return Err(GateError::AbiDecode(format!(
            "uint256 response too short: {} bytes",
            bytes.len()
        )));
    }
    // u128 caps at 2^128; reject values above that as "balance too
    // large to represent" rather than silently truncating. In
    // practice ERC-721 / ERC-1155 balances never exceed u64; u128
    // is generous headroom.
    if !bytes[..16].iter().all(|&b| b == 0) {
        return Err(GateError::AbiDecode(
            "uint256 response exceeds u128 — balance too large".into(),
        ));
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&bytes[16..32]);
    Ok(u128::from_be_bytes(buf))
}

fn parse_address(hex: &str) -> Result<[u8; 20], GateError> {
    let stripped = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    if stripped.len() != 40 {
        return Err(GateError::MalformedOwner(format!(
            "expected 40 hex chars, got {}",
            stripped.len()
        )));
    }
    let bytes =
        hex::decode(stripped).map_err(|e| GateError::MalformedOwner(format!("hex: {e}")))?;
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn normalise_address(hex: &str) -> Result<String, GateError> {
    let bytes = parse_address(hex)?;
    Ok(format!("0x{}", hex::encode(bytes)))
}

/// Case-insensitive 0x-prefix-tolerant address comparator. EIP-55
/// mixed-case forms and lowercase forms compare equal. Local copy
/// to avoid coupling token_gate to the contracts pin module which
/// lives on a parallel branch (will be deduplicated in a follow-up
/// once both branches land on main).
fn addr_eq(a: &str, b: &str) -> bool {
    let a_stripped = a
        .strip_prefix("0x")
        .or_else(|| a.strip_prefix("0X"))
        .unwrap_or(a);
    let b_stripped = b
        .strip_prefix("0x")
        .or_else(|| b.strip_prefix("0X"))
        .unwrap_or(b);
    a_stripped.eq_ignore_ascii_case(b_stripped)
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Test transport that returns a queue of canned responses. One
    /// response per `eth_call` in FIFO order.
    struct CannedTransport {
        responses: RefCell<VecDeque<String>>,
    }

    impl CannedTransport {
        fn new(responses: &[&str]) -> Self {
            Self {
                responses: RefCell::new(responses.iter().map(|s| s.to_string()).collect()),
            }
        }
    }

    impl JsonRpcTransport for CannedTransport {
        fn eth_call(&self, _to: &str, _data: &str) -> Result<String, RpcError> {
            self.responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| RpcError::Decode("transport drained".into()))
        }
    }

    fn padded_address(addr: &str) -> String {
        let stripped = addr.strip_prefix("0x").unwrap_or(addr);
        format!("0x{}{}", "0".repeat(24), stripped)
    }

    fn padded_uint(n: u128) -> String {
        let mut bytes = [0u8; 32];
        let n_bytes = n.to_be_bytes();
        bytes[16..32].copy_from_slice(&n_bytes);
        format!("0x{}", hex::encode(bytes))
    }

    const COLLECTION: &str = "0x1111111111111111111111111111111111111111";
    const OWNER: &str = "0x2222222222222222222222222222222222222222";
    const STRANGER: &str = "0x3333333333333333333333333333333333333333";

    #[test]
    fn erc721_owner_of_selector_matches_keccak() {
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"ownerOf(uint256)");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &ERC721_OWNER_OF_SELECTOR);
    }

    #[test]
    fn erc721_balance_of_selector_matches_keccak() {
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"balanceOf(address)");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &ERC721_BALANCE_OF_SELECTOR);
    }

    #[test]
    fn erc1155_balance_of_selector_matches_keccak() {
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"balanceOf(address,uint256)");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &ERC1155_BALANCE_OF_SELECTOR);
    }

    #[test]
    fn erc721_owner_of_specific_token_passes_when_owner_matches() {
        let transport = CannedTransport::new(&[&padded_address(OWNER)]);
        let gate = Erc721Gate::new(transport, "team-nft", COLLECTION).with_token_id(42);
        let r = gate.check(OWNER).unwrap();
        assert!(r.passed);
        assert_eq!(r.gate_label, "team-nft");
        assert!(r.evidence.contains("token_id=42"));
    }

    #[test]
    fn erc721_owner_of_specific_token_fails_when_owner_differs() {
        let transport = CannedTransport::new(&[&padded_address(STRANGER)]);
        let gate = Erc721Gate::new(transport, "team-nft", COLLECTION).with_token_id(42);
        let r = gate.check(OWNER).unwrap();
        assert!(!r.passed);
    }

    #[test]
    fn erc721_collection_membership_passes_when_balance_at_least_one() {
        let transport = CannedTransport::new(&[&padded_uint(3)]);
        let gate = Erc721Gate::new(transport, "dao-membership", COLLECTION);
        let r = gate.check(OWNER).unwrap();
        assert!(r.passed);
        assert!(r.evidence.contains("balanceOf=3"));
    }

    #[test]
    fn erc721_collection_membership_fails_when_balance_zero() {
        let transport = CannedTransport::new(&[&padded_uint(0)]);
        let gate = Erc721Gate::new(transport, "dao-membership", COLLECTION);
        let r = gate.check(OWNER).unwrap();
        assert!(!r.passed);
        assert!(r.evidence.contains("balanceOf=0"));
    }

    #[test]
    fn erc1155_balance_passes_when_at_threshold() {
        let transport = CannedTransport::new(&[&padded_uint(5)]);
        let gate = Erc1155Gate::new(transport, "credential-1155", COLLECTION, 7, 5);
        let r = gate.check(OWNER).unwrap();
        assert!(r.passed);
        assert!(r.evidence.contains("balance=5"));
        assert!(r.evidence.contains("threshold=5"));
    }

    #[test]
    fn erc1155_balance_fails_below_threshold() {
        let transport = CannedTransport::new(&[&padded_uint(2)]);
        let gate = Erc1155Gate::new(transport, "credential-1155", COLLECTION, 7, 5);
        let r = gate.check(OWNER).unwrap();
        assert!(!r.passed);
    }

    #[test]
    fn any_of_passes_on_first_passing_branch() {
        // First gate fails (balance 0), second passes (balance 1) — should pass.
        let t1 = CannedTransport::new(&[&padded_uint(0)]);
        let t2 = CannedTransport::new(&[&padded_uint(1)]);
        let any = AnyOfGates::new(
            "any-of-two",
            vec![
                Box::new(Erc721Gate::new(t1, "first", COLLECTION)),
                Box::new(Erc721Gate::new(t2, "second", COLLECTION)),
            ],
        )
        .unwrap();
        let r = any.check(OWNER).unwrap();
        assert!(r.passed);
        assert!(r.gate_label.contains("any-of-two"));
        assert!(r.gate_label.contains("second"));
    }

    #[test]
    fn any_of_fails_when_every_branch_fails() {
        let t1 = CannedTransport::new(&[&padded_uint(0)]);
        let t2 = CannedTransport::new(&[&padded_uint(0)]);
        let any = AnyOfGates::new(
            "any-of-two",
            vec![
                Box::new(Erc721Gate::new(t1, "first", COLLECTION)),
                Box::new(Erc721Gate::new(t2, "second", COLLECTION)),
            ],
        )
        .unwrap();
        let r = any.check(OWNER).unwrap();
        assert!(!r.passed);
        assert!(r.gate_label.contains("2 branches all failed"));
    }

    #[test]
    fn any_of_empty_composite_rejected_at_construction() {
        match AnyOfGates::new("empty", vec![]) {
            Err(GateError::EmptyComposite) => {}
            Err(other) => panic!("wrong error variant: {other:?}"),
            Ok(_) => panic!("empty composite should have been rejected"),
        }
    }

    #[test]
    fn all_of_passes_only_when_every_branch_passes() {
        let t1 = CannedTransport::new(&[&padded_uint(2)]);
        let t2 = CannedTransport::new(&[&padded_uint(1)]);
        let all = AllOfGates::new(
            "all-of-two",
            vec![
                Box::new(Erc721Gate::new(t1, "first", COLLECTION)),
                Box::new(Erc721Gate::new(t2, "second", COLLECTION)),
            ],
        )
        .unwrap();
        let r = all.check(OWNER).unwrap();
        assert!(r.passed);
        assert!(r.evidence.contains("first:"));
        assert!(r.evidence.contains("second:"));
    }

    #[test]
    fn all_of_fails_at_first_failing_branch() {
        let t1 = CannedTransport::new(&[&padded_uint(2)]);
        // Second gate would be checked next and would fail, but we
        // only need t1 to fail to short-circuit. Test the
        // short-circuit path by failing on the first branch.
        let t1_fail = CannedTransport::new(&[&padded_uint(0)]);
        let t2 = CannedTransport::new(&[]); // never queried
        let all = AllOfGates::new(
            "all-of-two",
            vec![
                Box::new(Erc721Gate::new(t1_fail, "first-fail", COLLECTION)),
                Box::new(Erc721Gate::new(t2, "second-skipped", COLLECTION)),
            ],
        )
        .unwrap();
        let r = all.check(OWNER).unwrap();
        assert!(!r.passed);
        assert!(r.gate_label.contains("first-fail"));
        // Drop unused transport
        let _ = t1;
    }

    #[test]
    fn all_of_empty_composite_rejected_at_construction() {
        match AllOfGates::new("empty", vec![]) {
            Err(GateError::EmptyComposite) => {}
            Err(other) => panic!("wrong error variant: {other:?}"),
            Ok(_) => panic!("empty composite should have been rejected"),
        }
    }

    #[test]
    fn malformed_owner_address_rejected() {
        let transport = CannedTransport::new(&[]);
        let gate = Erc721Gate::new(transport, "x", COLLECTION);
        let err = gate.check("0xnothex").unwrap_err();
        assert!(matches!(err, GateError::MalformedOwner(_)));
    }

    #[test]
    fn malformed_contract_address_rejected() {
        let transport = CannedTransport::new(&[]);
        let gate = Erc721Gate::new(transport, "x", "0xnothex");
        let err = gate.check(OWNER).unwrap_err();
        assert!(matches!(err, GateError::MalformedOwner(_)));
    }

    #[test]
    fn risk_class_low_helper_constructs_collection_gate() {
        let t = CannedTransport::new(&[&padded_uint(1)]);
        let gate = risk_class_low(t, "low-class", COLLECTION);
        let r = gate.check(OWNER).unwrap();
        assert!(r.passed);
        assert_eq!(r.gate_label, "low-class");
    }

    #[test]
    fn risk_class_medium_helper_constructs_1155_gate() {
        let t = CannedTransport::new(&[&padded_uint(10)]);
        let gate = risk_class_medium(t, "medium-class", COLLECTION, 1, 5);
        let r = gate.check(OWNER).unwrap();
        assert!(r.passed);
    }

    #[test]
    fn owner_address_normalised_to_lowercase_in_result() {
        let upper_owner = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let transport = CannedTransport::new(&[&padded_uint(1)]);
        let gate = Erc721Gate::new(transport, "x", COLLECTION);
        let r = gate.check(upper_owner).unwrap();
        assert_eq!(r.owner, upper_owner.to_lowercase());
    }

    #[test]
    fn balance_above_u128_rejected() {
        // Set high bits of the 32-byte response to force the
        // u128-overflow branch.
        let mut hex_resp = String::from("0x");
        for _ in 0..16 {
            hex_resp.push_str("ff");
        }
        for _ in 0..16 {
            hex_resp.push_str("00");
        }
        let transport = CannedTransport::new(&[&hex_resp]);
        let gate = Erc721Gate::new(transport, "x", COLLECTION);
        let err = gate.check(OWNER).unwrap_err();
        assert!(matches!(err, GateError::AbiDecode(_)));
    }

    #[test]
    fn gate_result_serializes_to_json() {
        let r = GateResult {
            passed: true,
            gate_label: "test".into(),
            owner: OWNER.into(),
            contract: COLLECTION.into(),
            evidence: "balanceOf=5".into(),
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: GateResult = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }
}
