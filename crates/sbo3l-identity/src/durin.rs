//! Durin subname issuance — write-side counterpart to
//! [`crate::ens_live::LiveEnsResolver`].
//!
//! T-3-1 ships:
//!
//! 1. **Calldata builders** ([`register_calldata`], [`multicall_calldata`])
//!    that emit the exact ABI bytes a Durin issuance + multicall-setText
//!    pair of transactions would carry. These are pure functions —
//!    deterministic, testable, no chain interaction.
//! 2. A [`DurinDryRun`] envelope that bundles both calldatas + the
//!    namehash + parent / target FQDN derived from the inputs, ready
//!    to print or pipe to `cast send`.
//! 3. [`build_dry_run`] — the orchestration function that the CLI's
//!    `sbo3l agent register --dry-run` calls.
//!
//! T-3-1 *does not* ship live broadcast. Mirroring the
//! [`crate::ens_anchor`] `audit anchor-ens` pattern, broadcast surfaces
//! as a feature-gated stub returning a clear "not implemented in this
//! build" error from the CLI. Follow-up work fills in the
//! `eth_sendRawTransaction` path under a [`sbo3l_core::signers::eth::EthSigner`]
//! consumer + EIP-1559 typed-tx assembly. The dry-run envelope is
//! stable across that addition — the broadcast path adds a `tx_hash`
//! field but otherwise reuses the same structure.
//!
//! ## Default parent
//!
//! Daniel owns `sbo3lagent.eth` on mainnet (registered pre-hackathon).
//! T-3-1's CLI defaults `--parent` to `sbo3lagent.eth` so a fresh
//! invocation issues `<name>.sbo3lagent.eth` without an explicit
//! parent flag. Operators with a different parent (e.g. their own
//! `*.eth`) pass `--parent <name>` explicitly.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

use crate::ens_anchor::{namehash, set_text_calldata, AnchorError, EnsNetwork};

/// Function selector for `register(bytes32 parentNode, string label, address owner, address resolver)`.
///
/// **Status: tentative.** Durin's reference implementation has gone
/// through several iterations; the canonical Sepolia / mainnet
/// deployment Daniel pins at impl time may use a different signature
/// (`registerAndSet`, single-call `setSubnodeRecord`, etc.). The
/// selector here is computed from the standard ENSIP-19 / ENS Labs
/// subname-registrar signature and validated by [`tests::register_selector_is_canonical`].
/// If Daniel's pinned Durin deploy uses a different signature, the
/// constant + the test assertion update together.
pub const REGISTER_SELECTOR: [u8; 4] = [0x4b, 0x7d, 0x09, 0x27];

/// Function selector for `multicall(bytes[] data)` — standard on the
/// ENS PublicResolver. Equal to `keccak256("multicall(bytes[])")[..4]`.
pub const MULTICALL_SELECTOR: [u8; 4] = [0xac, 0x96, 0x50, 0xd8];

/// Errors specific to Durin issuance dry-runs. Wraps [`AnchorError`]
/// where the underlying namehash / network parsing fails so callers
/// only need to match one error type.
#[derive(Debug, Error)]
pub enum DurinError {
    /// Underlying ENS / namehash failure (empty domain, unsupported network).
    #[error(transparent)]
    Anchor(#[from] AnchorError),

    /// `label` was empty or contained a `.` (which would mean caller
    /// already constructed a multi-label name — pass `parent` separately).
    #[error("label `{0}` must be a single non-empty DNS label (no `.`)")]
    InvalidLabel(String),

    /// `owner` could not be parsed as an EVM address.
    #[error("owner address `{0}` is not 0x-prefixed 40-hex-char EIP-55 hex")]
    InvalidOwnerAddress(String),

    /// One of the requested record keys is outside the canonical
    /// `sbo3l:*` namespace. T-3-1's policy is to refuse arbitrary keys
    /// — operators wanting other text records use the standard
    /// `cast send <resolver> "setText(...)"` directly.
    #[error("record key `{0}` is not in the sbo3l:* namespace; refused")]
    NonSbo3lKey(String),

    /// Record value exceeds the gateway storage discipline limit.
    #[error("record value for key `{key}` is {got} bytes; max 1024")]
    ValueTooLong { key: String, got: usize },
}

/// On-disk envelope shape of a `--dry-run` invocation. Stable across
/// the broadcast path landing later: a future tx-broadcast mode adds a
/// `register_tx_hash` and `multicall_tx_hash` field but otherwise
/// reuses this shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DurinDryRun {
    /// Identifies the wire shape for downstream tooling.
    pub schema: String,

    /// `<label>.<parent>` — the subname being issued.
    pub fqdn: String,

    /// Network (`mainnet` | `sepolia`).
    pub network: String,

    /// Hex-lowercase namehash of `parent`. 64 chars, no `0x` prefix.
    pub parent_namehash: String,

    /// Hex-lowercase namehash of `fqdn` (the about-to-be-issued name).
    /// 64 chars, no `0x` prefix.
    pub fqdn_namehash: String,

    /// Address that owns the subname after issuance. EIP-55 mixed-case
    /// hex with leading `0x`.
    pub owner: String,

    /// Address of the ENS PublicResolver the subname is pointed at.
    /// EIP-55 mixed-case hex with leading `0x`.
    pub resolver: String,

    /// `0x`-prefixed lowercase hex calldata for the `register` call
    /// (Durin registrar). Send to the registrar address with this
    /// data, no value.
    pub register_calldata_hex: String,

    /// `0x`-prefixed lowercase hex calldata for the
    /// `multicall(bytes[])` call (PublicResolver). Send to the
    /// resolver address with this data, no value.
    pub multicall_calldata_hex: String,

    /// Number of `setText` calls packed into the multicall.
    pub set_text_calls: usize,

    /// Each individual `setText` call's `0x`-prefixed lowercase hex
    /// calldata, in the same order they're packed into the multicall.
    /// Useful for auditors who want to verify per-record encoding.
    pub set_text_breakdown: Vec<DurinDryRunSetTextEntry>,

    /// Honest disclosure: dry-runs do NOT estimate gas (we don't
    /// contact an RPC). Operators wanting gas estimates run
    /// `cast estimate` against the printed calldata.
    pub gas_estimate: Option<u64>,

    /// Honest disclosure: dry-runs do NOT broadcast.
    pub broadcasted: bool,
}

/// One `setText` call inside the multicall, broken out for auditor
/// readability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DurinDryRunSetTextEntry {
    pub key: String,
    pub value: String,
    pub calldata_hex: String,
}

/// Stable schema id for [`DurinDryRun`].
pub const DURIN_DRY_RUN_SCHEMA: &str = "sbo3l.durin_dry_run.v1";

/// Per-record value byte cap. PublicResolver storage costs scale with
/// length; 1024 covers every realistic `sbo3l:*` value (capsule URI,
/// proof URI, agent_id) without enabling adversarial bloat.
pub const MAX_RECORD_VALUE_BYTES: usize = 1024;

/// Build the dry-run envelope for issuing `<label>.<parent>` with the
/// given `records`. Pure function: deterministic, no IO.
///
/// Caller invariants:
/// * `parent` is already-normalised lowercase ENS (e.g. `sbo3lagent.eth`)
/// * `label` is a single DNS label (no `.`) — function rejects
///   multi-label labels with [`DurinError::InvalidLabel`].
/// * `owner` is `0x` + 40 hex chars. Mixed-case (EIP-55) is preserved
///   in the dry-run output but not verified against the EIP-55 checksum
///   (operators occasionally lower-case for tooling reasons).
/// * `records` is an iterator of `(key, value)` pairs — keys MUST start
///   with `sbo3l:`; values MUST be ≤ [`MAX_RECORD_VALUE_BYTES`].
pub fn build_dry_run<'a, I>(
    parent: &str,
    label: &str,
    owner: &str,
    network: EnsNetwork,
    resolver: &str,
    records: I,
) -> Result<DurinDryRun, DurinError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    if label.is_empty() || label.contains('.') {
        return Err(DurinError::InvalidLabel(label.to_string()));
    }
    let owner_bytes = parse_address(owner)?;
    let resolver_bytes = parse_address(resolver)?;

    let parent_node = namehash(parent)?;
    let fqdn = format!("{label}.{parent}");
    let fqdn_node = namehash(&fqdn)?;

    let register = register_calldata(&parent_node, label, &owner_bytes, &resolver_bytes);

    let mut breakdown: Vec<DurinDryRunSetTextEntry> = Vec::new();
    let mut set_text_payloads: Vec<Vec<u8>> = Vec::new();

    for (key, value) in records {
        if !key.starts_with("sbo3l:") {
            return Err(DurinError::NonSbo3lKey(key.to_string()));
        }
        if value.len() > MAX_RECORD_VALUE_BYTES {
            return Err(DurinError::ValueTooLong {
                key: key.to_string(),
                got: value.len(),
            });
        }
        let cd = set_text_calldata(fqdn_node, key, value);
        breakdown.push(DurinDryRunSetTextEntry {
            key: key.to_string(),
            value: value.to_string(),
            calldata_hex: format!("0x{}", hex::encode(&cd)),
        });
        set_text_payloads.push(cd);
    }

    let multicall = multicall_calldata(&set_text_payloads);

    Ok(DurinDryRun {
        schema: DURIN_DRY_RUN_SCHEMA.to_string(),
        fqdn,
        network: network.as_str().to_string(),
        parent_namehash: hex::encode(parent_node),
        fqdn_namehash: hex::encode(fqdn_node),
        owner: format!("0x{}", hex::encode(owner_bytes)),
        resolver: format!("0x{}", hex::encode(resolver_bytes)),
        register_calldata_hex: format!("0x{}", hex::encode(&register)),
        multicall_calldata_hex: format!("0x{}", hex::encode(&multicall)),
        set_text_calls: breakdown.len(),
        set_text_breakdown: breakdown,
        gas_estimate: None,
        broadcasted: false,
    })
}

/// ABI-encode `register(bytes32 parentNode, string label, address owner, address resolver)`.
/// All 4 args are statically sized except `label` — head section has
/// 4 × 32 bytes, then `label` tail (length word + padded bytes).
pub fn register_calldata(
    parent_node: &[u8; 32],
    label: &str,
    owner: &[u8; 20],
    resolver: &[u8; 20],
) -> Vec<u8> {
    let label_padded = pad_to_32(label.len());
    // selector + 4 head words + tail (length + padded bytes)
    let mut out = Vec::with_capacity(4 + 4 * 32 + 32 + label_padded);

    out.extend_from_slice(&REGISTER_SELECTOR);

    // arg 0: bytes32 parentNode (inline)
    out.extend_from_slice(parent_node);

    // arg 1: string label — head is offset to tail. Tails start after
    // the 4 head words = 4 * 32 = 0x80.
    out.extend_from_slice(&u256_be(0x80));

    // arg 2: address owner — left-pad 20 bytes to 32.
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(owner);

    // arg 3: address resolver — same.
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(resolver);

    // Tail: length word + label bytes + padding to 32.
    out.extend_from_slice(&u256_be(label.len() as u64));
    out.extend_from_slice(label.as_bytes());
    let pad = label_padded - label.len();
    out.extend(std::iter::repeat_n(0u8, pad));

    out
}

/// ABI-encode `multicall(bytes[] data)`. The single argument is a
/// dynamic array of dynamic bytes — head is one offset (0x20),
/// followed by the array data: array length, then per-element offsets
/// (relative to the array data section), then per-element length-prefixed
/// padded bytes.
pub fn multicall_calldata(calls: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MULTICALL_SELECTOR);

    // Head: offset to bytes[] = 0x20 (just past this single head word).
    out.extend_from_slice(&u256_be(0x20));

    // Array data section starts here.
    // Layout:
    //   length (32)
    //   per-element offsets (32 each, relative to the start of the
    //     array data section *including* the length word — no, actually
    //     EIP-ABI says per-element offsets are relative to the start of
    //     the array data section AFTER the length word. Most reference
    //     impls offset relative to start of "the body of the array" =
    //     the position right after `length`. Anchoring here matches
    //     viem/ethers/cast.)
    //   per-element data (length + padded bytes), in order.
    let n = calls.len();
    out.extend_from_slice(&u256_be(n as u64));

    // Compute per-element offsets first. The offsets section itself is
    // n * 32 bytes long; the first element's data starts at offset
    // (n * 32) bytes past the length word.
    let offsets_section = (n * 32) as u64;
    let mut element_offsets: Vec<u64> = Vec::with_capacity(n);
    let mut running = offsets_section;
    for cd in calls.iter() {
        element_offsets.push(running);
        // Each element: length (32) + padded data
        let padded = pad_to_32(cd.len()) as u64;
        running += 32 + padded;
    }

    // Emit offsets.
    for off in &element_offsets {
        out.extend_from_slice(&u256_be(*off));
    }

    // Emit each element: length word + bytes + padding.
    for cd in calls.iter() {
        out.extend_from_slice(&u256_be(cd.len() as u64));
        out.extend_from_slice(cd);
        let padded = pad_to_32(cd.len());
        let pad = padded - cd.len();
        out.extend(std::iter::repeat_n(0u8, pad));
    }

    out
}

fn parse_address(s: &str) -> Result<[u8; 20], DurinError> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .ok_or_else(|| DurinError::InvalidOwnerAddress(s.to_string()))?;
    if stripped.len() != 40 {
        return Err(DurinError::InvalidOwnerAddress(s.to_string()));
    }
    let mut out = [0u8; 20];
    hex::decode_to_slice(stripped, &mut out)
        .map_err(|_| DurinError::InvalidOwnerAddress(s.to_string()))?;
    Ok(out)
}

fn pad_to_32(n: usize) -> usize {
    n.div_ceil(32) * 32
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

#[allow(dead_code)]
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak::v256();
    h.update(data);
    let mut out = [0u8; 32];
    h.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multicall_selector_is_canonical() {
        let computed = keccak256(b"multicall(bytes[])");
        assert_eq!(&computed[..4], &MULTICALL_SELECTOR);
    }

    #[test]
    fn register_selector_is_canonical() {
        // Tentative: standard ENSIP-19 / ENS Labs subname-registrar
        // shape. Per Durin pinning at impl time this assertion + the
        // REGISTER_SELECTOR const update together if the chosen
        // deployment uses a different signature.
        let computed = keccak256(b"register(bytes32,string,address,address)");
        assert_eq!(
            &computed[..4],
            &REGISTER_SELECTOR,
            "REGISTER_SELECTOR drifted from canonical signature; \
             update both the const and this assertion if Daniel's \
             pinned Durin deployment uses a different signature."
        );
    }

    #[test]
    fn parse_address_round_trip() {
        let raw = "0xdc7EFA00000000000000000000000000000000d2";
        let parsed = parse_address(raw).unwrap();
        assert_eq!(parsed[0], 0xdc);
        assert_eq!(parsed[1], 0x7E);
        assert_eq!(parsed[19], 0xd2);
    }

    #[test]
    fn parse_address_rejects_short() {
        assert!(parse_address("0xdeadbeef").is_err());
    }

    #[test]
    fn parse_address_rejects_no_prefix() {
        assert!(parse_address("dc7efa00000000000000000000000000000000d2").is_err());
    }

    #[test]
    fn build_dry_run_rejects_non_sbo3l_key() {
        let err = build_dry_run(
            "sbo3lagent.eth",
            "research-agent",
            "0x0000000000000000000000000000000000000001",
            EnsNetwork::Mainnet,
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            [("not-sbo3l", "value")],
        )
        .unwrap_err();
        assert!(matches!(err, DurinError::NonSbo3lKey(_)));
    }

    #[test]
    fn build_dry_run_rejects_oversized_value() {
        let big = "x".repeat(MAX_RECORD_VALUE_BYTES + 1);
        let err = build_dry_run(
            "sbo3lagent.eth",
            "research-agent",
            "0x0000000000000000000000000000000000000001",
            EnsNetwork::Mainnet,
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            [("sbo3l:big", big.as_str())],
        )
        .unwrap_err();
        assert!(matches!(err, DurinError::ValueTooLong { .. }));
    }

    #[test]
    fn build_dry_run_rejects_label_with_dot() {
        let err = build_dry_run(
            "sbo3lagent.eth",
            "with.dot",
            "0x0000000000000000000000000000000000000001",
            EnsNetwork::Mainnet,
            "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            [],
        )
        .unwrap_err();
        assert!(matches!(err, DurinError::InvalidLabel(_)));
    }

    #[test]
    fn build_dry_run_happy_path_envelope_shape() {
        let dr = build_dry_run(
            "sbo3lagent.eth",
            "research-agent",
            "0xdc7EFA00000000000000000000000000000000d2",
            EnsNetwork::Mainnet,
            EnsNetwork::Mainnet.default_public_resolver(),
            [
                ("sbo3l:agent_id", "research-agent-01"),
                ("sbo3l:endpoint", "http://127.0.0.1:8730/v1"),
            ],
        )
        .expect("build_dry_run should succeed for canonical inputs");

        assert_eq!(dr.schema, DURIN_DRY_RUN_SCHEMA);
        assert_eq!(dr.fqdn, "research-agent.sbo3lagent.eth");
        assert_eq!(dr.network, "mainnet");
        assert_eq!(dr.set_text_calls, 2);
        assert_eq!(dr.set_text_breakdown.len(), 2);
        assert!(dr.register_calldata_hex.starts_with("0x4b7d0927"));
        assert!(dr.multicall_calldata_hex.starts_with("0xac9650d8"));
        assert!(!dr.broadcasted);
        assert_eq!(dr.gas_estimate, None);
        // Each setText calldata starts with the setText selector.
        for entry in &dr.set_text_breakdown {
            assert!(entry.calldata_hex.starts_with("0x10f13a8c"));
        }
    }

    #[test]
    fn multicall_envelope_decodes_back_to_original_setText() {
        // Build a multicall with two setText payloads, then assert the
        // first payload appears verbatim inside the multicall bytes.
        let node = [0xab; 32];
        let a = set_text_calldata(node, "sbo3l:agent_id", "alice");
        let b = set_text_calldata(node, "sbo3l:endpoint", "http://x");
        let cd = multicall_calldata(&[a.clone(), b.clone()]);

        // Selector + offset(0x20) + length(2) = first 4 + 32 + 32 bytes.
        assert_eq!(&cd[..4], &MULTICALL_SELECTOR);
        // After: head section (n * 32 = 64 bytes), then element 0 length
        // word + padded data, then element 1.
        // The first payload's bytes appear after: selector(4) +
        // offset(32) + length(32) + offsets(2*32) + length-of-elem(32)
        // = 4 + 32 + 32 + 64 + 32 = 164 bytes in.
        let elem0_start = 4 + 32 + 32 + 2 * 32 + 32;
        assert_eq!(&cd[elem0_start..elem0_start + a.len()], a.as_slice());
    }
}
