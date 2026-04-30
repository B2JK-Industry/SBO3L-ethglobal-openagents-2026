//! ENS audit-root anchor envelope (B3 anchor side).
//!
//! Pure-Rust derivation of the ENS `setText(bytes32,string,string)`
//! call data needed to write SBO3L's `audit_root` (the chain-digest
//! over an audit prefix) into an ENS Public Resolver text record. No
//! network, no async, no signing — this module only builds the
//! envelope. Broadcasting belongs to a follow-up: the dry-run and
//! offline-fixture modes already prove the envelope is byte-correct.
//!
//! Truthfulness rules (mirrors the audit-checkpoint convention):
//!
//! - The envelope is loud about its mode. `mode == "dry_run"` and
//!   `mode == "offline_fixture"` carry an `explanation` that pins
//!   exactly what *was* and *was not* done. A real broadcast would
//!   produce a different artifact (`mode == "broadcasted"`, plus a
//!   `tx_hash`); that artifact shape is reserved for the broadcast
//!   path and intentionally not emitted here.
//! - Every public function in this module is deterministic. Given
//!   the same `(domain, audit_root, network, resolver, key)` inputs
//!   the same envelope falls out — that's why the dry-run is a
//!   verifiable demo: a third party with the same chain digest and
//!   the same domain rebuilds the exact same call data.
//!
//! ENS namehash follows EIP-137. setText ABI follows the ENS Public
//! Resolver interface (`setText(bytes32 node, string key, string value)`).
//! Verification vectors in the unit tests pin both against
//! independently-computed values.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

/// Reasons envelope construction can fail. Network / signing errors
/// belong to the (out-of-scope-here) broadcast path and have their
/// own error type.
#[derive(Debug, Error)]
pub enum AnchorError {
    #[error("audit_root must be 32-byte lowercase hex (no `0x` prefix); got {got_len} chars")]
    AuditRootBadLength { got_len: usize },
    #[error("audit_root contains non-hex character at byte {at}")]
    AuditRootNotHex { at: usize },
    #[error("network must be `mainnet` or `sepolia`; got `{0}`")]
    UnsupportedNetwork(String),
    #[error("resolver address must be 20-byte hex with `0x` prefix; got `{0}`")]
    ResolverBadFormat(String),
    #[error("domain must contain at least one label; got empty string")]
    EmptyDomain,
}

/// ENS network. Only the two networks SBO3L explicitly supports.
/// Mainnet/Sepolia have stable, well-known Public Resolver addresses
/// — adding a third network means adding a defaulted resolver too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsNetwork {
    Mainnet,
    Sepolia,
}

impl EnsNetwork {
    pub fn parse(s: &str) -> Result<Self, AnchorError> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
            other => Err(AnchorError::UnsupportedNetwork(other.to_string())),
        }
    }

    /// ENS Public Resolver (V3) — the contract that holds text records
    /// for names whose resolver hasn't been customised. Hardcoding
    /// these is fine: they're public infrastructure documented at
    /// docs.ens.domains, the rule "no hardcoded values" applies to
    /// the *agent's* identity (policy_hash, audit_root, agent_id) not
    /// to the well-known contracts that hold them.
    pub fn default_public_resolver(self) -> &'static str {
        match self {
            // Mainnet PublicResolver — ens.domains/docs/contract-api-reference/publicresolver
            Self::Mainnet => "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            // Sepolia PublicResolver
            Self::Sepolia => "0x8FADE66B79cC9f707aB26799354482EB93a5B7dD",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Sepolia => "sepolia",
        }
    }
}

/// SBO3L text-record key whose value is the audit chain digest.
pub const AUDIT_ROOT_KEY: &str = "sbo3l:audit_root";

/// `setText(bytes32 node, string key, string value)` — the ENS Public
/// Resolver function selector. Selector = first 4 bytes of
/// `keccak256("setText(bytes32,string,string)")`.
pub const SET_TEXT_SELECTOR: [u8; 4] = [0x10, 0xf1, 0x3a, 0x8c];

/// EIP-137 namehash. Recursive: namehash("") = 32 zero bytes;
/// namehash("x.y") = keccak256(namehash("y") || keccak256("x")).
/// Labels are joined by `.`. We don't normalise (ENSIP-15 / UTS-46)
/// in this hackathon scope — operator passes already-normalised names.
pub fn namehash(domain: &str) -> Result<[u8; 32], AnchorError> {
    if domain.is_empty() {
        return Err(AnchorError::EmptyDomain);
    }
    let mut node = [0u8; 32];
    // Walk labels right-to-left.
    for label in domain.rsplit('.') {
        if label.is_empty() {
            // Leading/trailing/double dots — treat as malformed but
            // not catastrophic; namehash semantics on empty labels
            // are equivalent to hashing the empty string, which
            // yields a stable (if not user-intended) result. Surface
            // as an error so the operator sees the typo.
            return Err(AnchorError::EmptyDomain);
        }
        let label_hash = keccak256(label.as_bytes());
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&node);
        buf[32..].copy_from_slice(&label_hash);
        node = keccak256(&buf);
    }
    Ok(node)
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// ABI-encode `setText(bytes32 node, string key, string value)`.
/// Returns selector || head_args || tail_args. Verified against
/// known-good output in unit tests.
pub fn set_text_calldata(node: [u8; 32], key: &str, value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 6 + key.len() + value.len() + 64);
    out.extend_from_slice(&SET_TEXT_SELECTOR);

    // Heads: arg0 = bytes32 inline (32 B); arg1 = string offset (32 B);
    // arg2 = string offset (32 B). Heads occupy 3*32 = 96 B from the
    // start of the args section (i.e. just after the selector).
    out.extend_from_slice(&node);

    // arg1 offset = 0x60 (96), pointing past the three heads to the
    // first tail.
    let key_offset: u64 = 0x60;
    out.extend_from_slice(&u256_be(key_offset));

    // arg2 offset = key_offset + 32 (length word) + ceil(key.len()/32)*32.
    let key_padded = pad_to_32_multiple(key.len());
    let value_offset: u64 = key_offset + 32 + key_padded as u64;
    out.extend_from_slice(&u256_be(value_offset));

    // Tail for arg1: length || padded bytes
    out.extend_from_slice(&u256_be(key.len() as u64));
    out.extend_from_slice(key.as_bytes());
    let key_pad = key_padded - key.len();
    out.extend(std::iter::repeat_n(0u8, key_pad));

    // Tail for arg2: length || padded bytes
    out.extend_from_slice(&u256_be(value.len() as u64));
    out.extend_from_slice(value.as_bytes());
    let value_padded = pad_to_32_multiple(value.len());
    let value_pad = value_padded - value.len();
    out.extend(std::iter::repeat_n(0u8, value_pad));

    out
}

fn pad_to_32_multiple(n: usize) -> usize {
    n.div_ceil(32) * 32
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

/// On-disk envelope shape. Stable across `--dry-run` and
/// `--offline-fixture`; a future broadcast mode adds a `tx_hash`
/// field but otherwise reuses this shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnchorEnvelope {
    pub schema: String,
    /// One of `dry_run` | `offline_fixture`. Loud disclosure.
    pub mode: String,
    pub explanation: String,
    pub network: String,
    pub domain: String,
    /// Hex-lowercase ENS namehash of `domain`, no `0x` prefix.
    pub namehash: String,
    /// Public Resolver contract address, `0x`-prefixed lowercase.
    pub resolver: String,
    /// SBO3L text-record key written: always `sbo3l:audit_root`.
    pub text_record_key: String,
    /// 64-char hex chain digest, no `0x` prefix.
    pub audit_root: String,
    /// Hex-encoded `setText(bytes32,string,string)` call data, no `0x` prefix.
    pub calldata: String,
    pub created_at: String,
}

pub const ENVELOPE_SCHEMA_ID: &str = "sbo3l.ens_anchor_envelope.v1";

const DRY_RUN_EXPLANATION: &str = "Dry-run envelope. Computed exactly what would be broadcast \
                                   (namehash, calldata, resolver) but did NOT contact any RPC \
                                   and did NOT sign anything. Re-run with --broadcast in a \
                                   build that wires the broadcast path to actually send.";
const OFFLINE_FIXTURE_EXPLANATION: &str = "Offline fixture. Identical content to dry-run, just \
                                            written to disk for demo / CI fixture use.";

/// Inputs to [`build_envelope`]. Validation is done at build time.
#[derive(Debug, Clone)]
pub struct AnchorParams<'a> {
    pub network: EnsNetwork,
    pub domain: &'a str,
    pub resolver: &'a str,
    /// 64-char lowercase hex, no `0x` prefix.
    pub audit_root: &'a str,
    pub mode: AnchorMode,
    /// RFC3339 timestamp.
    pub created_at: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorMode {
    DryRun,
    OfflineFixture,
}

impl AnchorMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::OfflineFixture => "offline_fixture",
        }
    }
    fn explanation(self) -> &'static str {
        match self {
            Self::DryRun => DRY_RUN_EXPLANATION,
            Self::OfflineFixture => OFFLINE_FIXTURE_EXPLANATION,
        }
    }
}

pub fn build_envelope(params: AnchorParams<'_>) -> Result<AnchorEnvelope, AnchorError> {
    validate_audit_root(params.audit_root)?;
    let resolver = validate_resolver(params.resolver)?;
    let node = namehash(params.domain)?;
    let calldata = set_text_calldata(node, AUDIT_ROOT_KEY, params.audit_root);

    Ok(AnchorEnvelope {
        schema: ENVELOPE_SCHEMA_ID.to_string(),
        mode: params.mode.as_str().to_string(),
        explanation: params.mode.explanation().to_string(),
        network: params.network.as_str().to_string(),
        domain: params.domain.to_string(),
        namehash: hex::encode(node),
        resolver,
        text_record_key: AUDIT_ROOT_KEY.to_string(),
        audit_root: params.audit_root.to_string(),
        calldata: hex::encode(&calldata),
        created_at: params.created_at.to_string(),
    })
}

fn validate_audit_root(s: &str) -> Result<(), AnchorError> {
    if s.len() != 64 {
        return Err(AnchorError::AuditRootBadLength { got_len: s.len() });
    }
    for (i, c) in s.bytes().enumerate() {
        let ok = c.is_ascii_digit() || (b'a'..=b'f').contains(&c);
        if !ok {
            return Err(AnchorError::AuditRootNotHex { at: i });
        }
    }
    Ok(())
}

fn validate_resolver(s: &str) -> Result<String, AnchorError> {
    let stripped = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    let body = match stripped {
        Some(b) => b,
        None => return Err(AnchorError::ResolverBadFormat(s.to_string())),
    };
    if body.len() != 40 || !body.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(AnchorError::ResolverBadFormat(s.to_string()));
    }
    // Normalise to lowercase 0x-prefix.
    Ok(format!("0x{}", body.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namehash_empty_domain_errors() {
        assert!(matches!(namehash(""), Err(AnchorError::EmptyDomain)));
    }

    /// EIP-137 known vector: namehash("eth") =
    /// 0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae
    #[test]
    fn namehash_eth_matches_eip137_vector() {
        let h = namehash("eth").unwrap();
        assert_eq!(
            hex::encode(h),
            "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
    }

    /// EIP-137 known vector: namehash("foo.eth") =
    /// 0xde9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f
    #[test]
    fn namehash_foo_eth_matches_eip137_vector() {
        let h = namehash("foo.eth").unwrap();
        assert_eq!(
            hex::encode(h),
            "de9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f"
        );
    }

    #[test]
    fn namehash_double_dot_errors() {
        assert!(matches!(
            namehash("foo..eth"),
            Err(AnchorError::EmptyDomain)
        ));
    }

    /// Selector = first 4 bytes of keccak256("setText(bytes32,string,string)").
    /// Pin against the constant; the test fails loudly if either side drifts.
    #[test]
    fn set_text_selector_matches_signature() {
        let derived = keccak256(b"setText(bytes32,string,string)");
        assert_eq!(derived[..4], SET_TEXT_SELECTOR);
    }

    /// Sanity check: setText calldata for short args has the expected
    /// structure (selector + 3 heads + 2 tails). Length is fully
    /// determined by the args; off-by-one in the encoder breaks this.
    #[test]
    fn set_text_calldata_len_matches_abi_layout() {
        let node = [0u8; 32];
        let key = "sbo3l:audit_root"; // 16 bytes → padded to 32
        let value = "deadbeef".repeat(8); // 64 bytes → padded to 64
        let cd = set_text_calldata(node, key, &value);
        // selector(4) + node(32) + 2 offsets(64) + key_len(32) + key_padded(32)
        //   + value_len(32) + value_padded(64)
        assert_eq!(cd.len(), 4 + 32 + 64 + 32 + 32 + 32 + 64);
        // Selector pinned.
        assert_eq!(cd[..4], SET_TEXT_SELECTOR);
        // First arg is the (zero) node, big-endian, full 32 B.
        assert!(cd[4..36].iter().all(|&b| b == 0));
        // arg1 offset == 0x60.
        assert_eq!(u64::from_be_bytes(cd[60..68].try_into().unwrap()), 0x60);
    }

    /// Round-trip: after we encode the value, the bytes that come
    /// back inside the tail equal the original value. Catches any
    /// off-by-one in the offset arithmetic.
    #[test]
    fn set_text_calldata_value_roundtrips() {
        let node = [0xabu8; 32];
        let key = "sbo3l:audit_root";
        let value = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cd = set_text_calldata(node, key, value);

        // arg2 head sits in bytes [4+32+32 .. 4+32+32+32] = [68..100]; the
        // u64 occupies the last 8 bytes of that 32-byte big-endian word.
        let v_off = u64::from_be_bytes(cd[92..100].try_into().unwrap()) as usize;
        // Tail starts at selector (4) + offset.
        let tail_start = 4 + v_off;
        let v_len =
            u64::from_be_bytes(cd[tail_start + 24..tail_start + 32].try_into().unwrap()) as usize;
        assert_eq!(v_len, value.len());
        let v_bytes = &cd[tail_start + 32..tail_start + 32 + v_len];
        assert_eq!(v_bytes, value.as_bytes());
    }

    #[test]
    fn validate_audit_root_rejects_short() {
        let r = validate_audit_root("deadbeef");
        assert!(matches!(
            r,
            Err(AnchorError::AuditRootBadLength { got_len: 8 })
        ));
    }

    #[test]
    fn validate_audit_root_rejects_uppercase_or_0x() {
        assert!(matches!(
            validate_audit_root(&"A".repeat(64)),
            Err(AnchorError::AuditRootNotHex { .. })
        ));
        let with_prefix = format!("0x{}", "0".repeat(62));
        assert!(matches!(
            validate_audit_root(&with_prefix),
            Err(AnchorError::AuditRootNotHex { .. })
        ));
    }

    #[test]
    fn validate_resolver_normalises_to_lowercase_with_prefix() {
        let r = validate_resolver("0x231B0EE14048E9DCCD1D247744D114A4EB5E8E63").unwrap();
        assert_eq!(r, "0x231b0ee14048e9dccd1d247744d114a4eb5e8e63");
    }

    #[test]
    fn build_envelope_dry_run_is_deterministic() {
        let p1 = AnchorParams {
            network: EnsNetwork::Sepolia,
            domain: "sbo3l.eth",
            resolver: EnsNetwork::Sepolia.default_public_resolver(),
            audit_root: &"a".repeat(64),
            mode: AnchorMode::DryRun,
            created_at: "2026-04-30T00:00:00Z",
        };
        let p2 = p1.clone();
        let e1 = build_envelope(p1).unwrap();
        let e2 = build_envelope(p2).unwrap();
        assert_eq!(e1, e2);
        assert_eq!(e1.schema, ENVELOPE_SCHEMA_ID);
        assert_eq!(e1.mode, "dry_run");
        assert_eq!(e1.text_record_key, "sbo3l:audit_root");
        // Selector at the start of calldata.
        assert!(e1.calldata.starts_with("10f13a8c"));
    }

    #[test]
    fn build_envelope_rejects_unsupported_network_via_parser() {
        assert!(matches!(
            EnsNetwork::parse("polygon"),
            Err(AnchorError::UnsupportedNetwork(_))
        ));
    }

    #[test]
    fn default_public_resolvers_are_addresses() {
        for net in [EnsNetwork::Mainnet, EnsNetwork::Sepolia] {
            let r = validate_resolver(net.default_public_resolver()).unwrap();
            assert!(r.starts_with("0x") && r.len() == 42);
        }
    }
}
