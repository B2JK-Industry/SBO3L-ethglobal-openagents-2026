//! Cross-agent verification protocol (T-3-4).
//!
//! Defines how two SBO3L agents authenticate each other on the wire,
//! using ENS as the rendezvous point for the peer's Ed25519 pubkey:
//!
//! ```text
//!   Agent A                            Agent B
//!     │                                  │
//!     │  builds CrossAgentChallenge      │
//!     │  with audit_chain_head + nonce   │
//!     │  + ts_ms; signs with its         │
//!     │  Ed25519 secret                  │
//!     │                                  │
//!     │ ───── SignedChallenge ──────────▶│
//!     │                                  │
//!     │                                  │ resolves A's
//!     │                                  │ sbo3l:pubkey_ed25519
//!     │                                  │ via getEnsText,
//!     │                                  │ verifies signature,
//!     │                                  │ emits CrossAgentTrust.
//!     │                                  │
//!     │ ◀──── CrossAgentTrust ───────────│
//! ```
//!
//! The protocol is **stateless** — the verifier doesn't need to keep
//! a session, just a fresh ENS lookup and a signature check. The
//! challenge carries the initiator's current audit-chain head so the
//! verifier can pin the trust receipt against a specific moment in
//! the initiator's audit timeline (any tampering with later events
//! shifts the head and breaks the receipt's pinning).
//!
//! The wire format is JCS-canonical JSON of the [`CrossAgentChallenge`]
//! struct — same canonicalisation pattern the audit chain uses for
//! its `payload_hash`. Two implementations of this protocol on
//! different stacks (Rust daemon, TypeScript MCP client) sign / verify
//! byte-identical bytes.
//!
//! ## Sponsor framing
//!
//! This is the load-bearing claim for the ENS bounty's "trust DNS"
//! framing: ENS is the **only** thing two agents need to share to
//! authenticate each other. No CA, no shared session token, no
//! out-of-band registration. The peer presents the challenge, the
//! verifier reads ENS, the verifier checks the signature. That's it.

use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ens_live::{JsonRpcTransport, LiveEnsResolver};

/// Schema id pinned in the wire format.
pub const CHALLENGE_SCHEMA: &str = "sbo3l.cross_agent_challenge.v1";

/// Schema id for the trust receipt.
pub const TRUST_SCHEMA: &str = "sbo3l.cross_agent_trust.v1";

/// ENS text-record key the verifier reads to learn the initiator's
/// signing pubkey. T-3-3 fleet writes this record at registration time.
pub const PUBKEY_RECORD_KEY: &str = "sbo3l:pubkey_ed25519";

/// Wire-format challenge an agent presents to a peer. Serialised as
/// JCS-canonical JSON before signing — every field is mandatory and
/// `additionalProperties: false` is enforced via `deny_unknown_fields`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CrossAgentChallenge {
    /// Always [`CHALLENGE_SCHEMA`].
    pub schema: String,

    /// Initiator's fully-qualified ENS name (e.g.
    /// `research-agent.sbo3lagent.eth`). Verifier looks up
    /// `PUBKEY_RECORD_KEY` against this name to retrieve the
    /// expected pubkey.
    pub agent_fqdn: String,

    /// Hex-encoded 32-byte digest of the initiator's audit chain at
    /// challenge-build time. Same digest the F-7 audit-checkpoint
    /// publisher computes; pinning here lets the verifier later
    /// detect any retroactive tampering of the initiator's history.
    pub audit_chain_head_hex: String,

    /// Hex-encoded 16-byte fresh nonce. Replay-protection: the
    /// verifier MAY cache `(agent_fqdn, nonce)` for a TTL window
    /// and reject duplicates.
    pub nonce_hex: String,

    /// Initiator's wall-clock at challenge-build time, milliseconds
    /// since Unix epoch. Verifier MAY enforce a freshness bound
    /// (e.g. ±5 minutes from its own clock). Stale challenges are
    /// rejected with [`CrossAgentReject::ExpiredOrFutureChallenge`].
    pub ts_ms: u64,
}

/// Signed envelope: the challenge plus the initiator's Ed25519
/// signature over its JCS-canonical bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignedChallenge {
    pub challenge: CrossAgentChallenge,
    /// `0x`-prefixed lowercase hex of the 64-byte Ed25519 signature.
    pub signature_hex: String,
}

/// Trust receipt the verifier emits after a successful check. Empty
/// `rejection_reason` + `valid: true` means the challenge passed all
/// invariants. `valid: false` means the receipt is informational —
/// the verifier saw the challenge, refused it, and recorded why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CrossAgentTrust {
    pub schema: String,
    pub peer_fqdn: String,
    pub peer_pubkey_hex: String,
    pub peer_audit_head_hex: String,
    pub signed_at_ms: u64,
    pub verified_at_ms: u64,
    pub valid: bool,
    pub rejection_reason: Option<String>,
}

/// Why a challenge was rejected. Surfaced in
/// [`CrossAgentTrust::rejection_reason`] when `valid: false`.
#[derive(Debug, Clone, Copy)]
pub enum CrossAgentReject {
    SchemaMismatch,
    UnknownPeer,
    PubkeyRecordMissing,
    PubkeyRecordMalformed,
    SignatureMalformed,
    SignatureMismatch,
    ExpiredOrFutureChallenge,
}

impl CrossAgentReject {
    fn as_str(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "schema_mismatch",
            Self::UnknownPeer => "peer_fqdn_not_in_ens",
            Self::PubkeyRecordMissing => "sbo3l_pubkey_ed25519_record_missing",
            Self::PubkeyRecordMalformed => "sbo3l_pubkey_ed25519_record_malformed",
            Self::SignatureMalformed => "signature_malformed",
            Self::SignatureMismatch => "signature_mismatch",
            Self::ExpiredOrFutureChallenge => "challenge_outside_freshness_window",
        }
    }
}

#[derive(Debug, Error)]
pub enum CrossAgentError {
    #[error("JCS canonicalisation failed: {0}")]
    Jcs(String),

    #[error("ENS resolver: {0}")]
    EnsResolve(String),

    #[error("hex decode failed for {field}: {error}")]
    HexDecode { field: &'static str, error: String },

    #[error("system clock returned a pre-epoch instant: {0}")]
    SystemClock(String),
}

/// Three-way result of looking up an agent's
/// `sbo3l:pubkey_ed25519` text record.
///
/// The verifier maps each variant to a distinct trust-receipt
/// rejection so the operator can take the right corrective action:
///
/// | Variant | Rejection reason | Operator fix |
/// |---|---|---|
/// | `Found(hex)` | (proceeds to signature verification) | n/a |
/// | `PubkeyMissing` | `sbo3l_pubkey_ed25519_record_missing` | agent owner sets the record |
/// | `UnknownPeer` | `peer_fqdn_not_in_ens` | register the FQDN before re-trying |
/// | `Error(msg)` | propagated as `CrossAgentError::EnsResolve` | inspect RPC / network |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubkeyLookup {
    Found(String),
    PubkeyMissing,
    UnknownPeer,
    Error(String),
}

/// Trait for the ENS lookup the verifier needs. Production binds this
/// to [`LiveEnsResolver`]; tests inject an in-memory map.
///
/// Implementations return [`PubkeyLookup`] rather than a plain
/// `Result<Option<String>, _>` so the verifier can distinguish:
///
/// - **Found** — record present; verify the signature.
/// - **PubkeyMissing** — FQDN registered in ENS but pubkey record
///   unset (recoverable: agent owner runs `setText`).
/// - **UnknownPeer** — FQDN not registered in ENS at all
///   (recoverable: register the FQDN first).
/// - **Error** — hard failure (RPC down, malformed namehash);
///   propagated up as [`CrossAgentError::EnsResolve`].
pub trait PubkeyResolver {
    fn resolve_pubkey(&self, fqdn: &str) -> PubkeyLookup;
}

impl<T: JsonRpcTransport> PubkeyResolver for LiveEnsResolver<T> {
    fn resolve_pubkey(&self, fqdn: &str) -> PubkeyLookup {
        match self.resolve_raw_text(fqdn, PUBKEY_RECORD_KEY) {
            Ok(Some(value)) => PubkeyLookup::Found(value),
            // ENS Registry has a resolver pointer for this name, but
            // the `sbo3l:pubkey_ed25519` record is unset. Distinct
            // from UnknownPeer below — same FQDN, different recovery
            // ("ask the agent owner to set the record").
            Ok(None) => PubkeyLookup::PubkeyMissing,
            // ENS Registry says no resolver for this FQDN at all.
            // Surface the dedicated UnknownPeer signal so the verifier
            // can distinguish it from "name in ENS but pubkey absent"
            // — this is the codex P1 from #167 (without this mapping a
            // verifier crashes when it should refuse cleanly).
            Err(crate::ens::ResolveError::UnknownName(_)) => PubkeyLookup::UnknownPeer,
            Err(e) => PubkeyLookup::Error(format!("ENS resolver: {e}")),
        }
    }
}

/// Maximum allowed clock skew between initiator and verifier.
pub const FRESHNESS_WINDOW_MS: u64 = 5 * 60 * 1000;

/// Build a fresh challenge using the system clock. Caller supplies
/// the audit-chain head + nonce.
pub fn build_challenge(
    agent_fqdn: &str,
    audit_head_hex: &str,
    nonce_hex: &str,
) -> Result<CrossAgentChallenge, CrossAgentError> {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CrossAgentError::SystemClock(e.to_string()))?
        .as_millis() as u64;
    Ok(CrossAgentChallenge {
        schema: CHALLENGE_SCHEMA.to_string(),
        agent_fqdn: agent_fqdn.to_string(),
        audit_chain_head_hex: audit_head_hex.to_string(),
        nonce_hex: nonce_hex.to_string(),
        ts_ms,
    })
}

/// Sign a challenge with the supplied Ed25519 secret. Pure function:
/// canonicalises the challenge to JCS, signs the bytes, returns the
/// envelope.
pub fn sign_challenge(
    challenge: &CrossAgentChallenge,
    key: &SigningKey,
) -> Result<SignedChallenge, CrossAgentError> {
    let bytes = jcs_bytes(challenge)?;
    let sig = key.sign(&bytes);
    Ok(SignedChallenge {
        challenge: challenge.clone(),
        signature_hex: format!("0x{}", hex::encode(sig.to_bytes())),
    })
}

/// Verify a signed challenge against a peer's ENS-resolved pubkey
/// and emit a trust receipt. Pure with respect to the resolver
/// interface — tests inject a fake.
///
/// `verified_at_ms` is the verifier's wall-clock; supply
/// `SystemTime::now()` in production. Tests can pass a fixed value.
pub fn verify_challenge<R: PubkeyResolver>(
    signed: &SignedChallenge,
    resolver: &R,
    verified_at_ms: u64,
) -> Result<CrossAgentTrust, CrossAgentError> {
    // Schema match — refuse anything not pinned to v1 to leave room
    // for forward-incompatible bumps without silently accepting them.
    if signed.challenge.schema != CHALLENGE_SCHEMA {
        return Ok(reject(
            signed,
            "",
            verified_at_ms,
            CrossAgentReject::SchemaMismatch,
        ));
    }

    // Freshness window. ts_ms must be within ±FRESHNESS_WINDOW_MS of
    // verifier's clock.
    let drift = verified_at_ms as i128 - signed.challenge.ts_ms as i128;
    if drift.unsigned_abs() > FRESHNESS_WINDOW_MS as u128 {
        return Ok(reject(
            signed,
            "",
            verified_at_ms,
            CrossAgentReject::ExpiredOrFutureChallenge,
        ));
    }

    // Resolve peer pubkey via ENS. Three-way map:
    //   Found(hex)        → continue to signature verify
    //   UnknownPeer       → reject with peer_fqdn_not_in_ens
    //   PubkeyMissing     → reject with sbo3l_pubkey_ed25519_record_missing
    //   Error(msg)        → propagate as CrossAgentError::EnsResolve
    let pubkey_hex = match resolver.resolve_pubkey(&signed.challenge.agent_fqdn) {
        PubkeyLookup::Found(p) => p,
        PubkeyLookup::UnknownPeer => {
            return Ok(reject(
                signed,
                "",
                verified_at_ms,
                CrossAgentReject::UnknownPeer,
            ));
        }
        PubkeyLookup::PubkeyMissing => {
            return Ok(reject(
                signed,
                "",
                verified_at_ms,
                CrossAgentReject::PubkeyRecordMissing,
            ));
        }
        PubkeyLookup::Error(msg) => {
            return Err(CrossAgentError::EnsResolve(msg));
        }
    };

    let pubkey = match parse_ed25519_pubkey(&pubkey_hex) {
        Some(k) => k,
        None => {
            return Ok(reject(
                signed,
                &pubkey_hex,
                verified_at_ms,
                CrossAgentReject::PubkeyRecordMalformed,
            ));
        }
    };

    // Decode the signature.
    let sig_bytes = match decode_hex_64(&signed.signature_hex) {
        Some(b) => b,
        None => {
            return Ok(reject(
                signed,
                &pubkey_hex,
                verified_at_ms,
                CrossAgentReject::SignatureMalformed,
            ));
        }
    };
    let sig = Signature::from_bytes(&sig_bytes);

    // Re-canonicalise the challenge and verify.
    let bytes = jcs_bytes(&signed.challenge)?;
    if pubkey.verify(&bytes, &sig).is_err() {
        return Ok(reject(
            signed,
            &pubkey_hex,
            verified_at_ms,
            CrossAgentReject::SignatureMismatch,
        ));
    }

    Ok(CrossAgentTrust {
        schema: TRUST_SCHEMA.to_string(),
        peer_fqdn: signed.challenge.agent_fqdn.clone(),
        peer_pubkey_hex: pubkey_hex,
        peer_audit_head_hex: signed.challenge.audit_chain_head_hex.clone(),
        signed_at_ms: signed.challenge.ts_ms,
        verified_at_ms,
        valid: true,
        rejection_reason: None,
    })
}

/// JCS-canonical bytes of any `Serialize` value. Mirrors the audit
/// chain's hashing input.
fn jcs_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CrossAgentError> {
    serde_json_canonicalizer::to_string(value)
        .map(|s| s.into_bytes())
        .map_err(|e| CrossAgentError::Jcs(e.to_string()))
}

fn reject(
    signed: &SignedChallenge,
    peer_pubkey_hex: &str,
    verified_at_ms: u64,
    reason: CrossAgentReject,
) -> CrossAgentTrust {
    CrossAgentTrust {
        schema: TRUST_SCHEMA.to_string(),
        peer_fqdn: signed.challenge.agent_fqdn.clone(),
        peer_pubkey_hex: peer_pubkey_hex.to_string(),
        peer_audit_head_hex: signed.challenge.audit_chain_head_hex.clone(),
        signed_at_ms: signed.challenge.ts_ms,
        verified_at_ms,
        valid: false,
        rejection_reason: Some(reason.as_str().to_string()),
    }
}

fn parse_ed25519_pubkey(hex_str: &str) -> Option<VerifyingKey> {
    let stripped = hex_str
        .strip_prefix("0x")
        .or_else(|| hex_str.strip_prefix("0X"))
        .unwrap_or(hex_str);
    if stripped.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(stripped, &mut bytes).ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

fn decode_hex_64(hex_str: &str) -> Option<[u8; 64]> {
    let stripped = hex_str
        .strip_prefix("0x")
        .or_else(|| hex_str.strip_prefix("0X"))
        .unwrap_or(hex_str);
    if stripped.len() != 128 {
        return None;
    }
    let mut bytes = [0u8; 64];
    hex::decode_to_slice(stripped, &mut bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// In-memory test resolver: maps fqdn → pubkey hex.
    struct FakePubkeyResolver {
        map: HashMap<String, String>,
    }

    impl FakePubkeyResolver {
        fn new() -> Self {
            Self {
                map: HashMap::new(),
            }
        }
        fn insert(&mut self, fqdn: &str, pubkey_hex: &str) {
            self.map.insert(fqdn.to_string(), pubkey_hex.to_string());
        }
    }

    impl PubkeyResolver for FakePubkeyResolver {
        fn resolve_pubkey(&self, fqdn: &str) -> PubkeyLookup {
            // The fake resolver knows nothing about ENS Registry
            // semantics, so a missing fqdn surfaces as UnknownPeer
            // (matching the LiveEnsResolver shape). Tests that want
            // the PubkeyMissing path register the fqdn explicitly
            // with an empty value via `insert_missing`.
            match self.map.get(fqdn) {
                Some(value) if value.is_empty() => PubkeyLookup::PubkeyMissing,
                Some(value) => PubkeyLookup::Found(value.clone()),
                None => PubkeyLookup::UnknownPeer,
            }
        }
    }

    impl FakePubkeyResolver {
        /// Register an FQDN as "in ENS but pubkey record absent" so
        /// tests can drive the `PubkeyRecordMissing` rejection path
        /// distinct from `UnknownPeer`.
        #[allow(dead_code)]
        fn insert_missing(&mut self, fqdn: &str) {
            self.map.insert(fqdn.to_string(), String::new());
        }
    }

    fn fixed_key(seed: &[u8; 32]) -> SigningKey {
        SigningKey::from_bytes(seed)
    }

    fn make_challenge(fqdn: &str, ts_ms: u64) -> CrossAgentChallenge {
        CrossAgentChallenge {
            schema: CHALLENGE_SCHEMA.to_string(),
            agent_fqdn: fqdn.to_string(),
            audit_chain_head_hex: "0xdeadbeef".repeat(8),
            nonce_hex: "0x".to_string() + &"ab".repeat(16),
            ts_ms,
        }
    }

    #[test]
    fn happy_path_pair_verifies() {
        // Agent A signs; Agent B verifies via fake resolver.
        let a_seed = [11u8; 32];
        let a_key = fixed_key(&a_seed);
        let a_pub_hex = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));

        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub_hex);

        let trust = verify_challenge(&signed, &resolver, now).unwrap();

        assert!(trust.valid, "{:?}", trust.rejection_reason);
        assert_eq!(trust.peer_fqdn, "research-agent.sbo3lagent.eth");
        assert_eq!(trust.peer_pubkey_hex, a_pub_hex);
        assert_eq!(trust.signed_at_ms, now);
        assert_eq!(trust.verified_at_ms, now);
        assert_eq!(trust.schema, TRUST_SCHEMA);
    }

    #[test]
    fn tampered_audit_head_fails_signature() {
        let a_key = fixed_key(&[12u8; 32]);
        let a_pub_hex = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        let mut signed = sign_challenge(&challenge, &a_key).unwrap();

        // Flip a byte in the audit head — sig over original challenge
        // bytes won't match the tampered re-canonicalisation.
        signed.challenge.audit_chain_head_hex = "0xcafebabe".repeat(8);

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub_hex);

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("signature_mismatch")
        );
    }

    #[test]
    fn unknown_peer_rejects_clean() {
        let a_key = fixed_key(&[13u8; 32]);
        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("ghost.sbo3lagent.eth", now);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        // Resolver has no entry for this fqdn.
        let resolver = FakePubkeyResolver::new();

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        // FQDN not in ENS at all → the dedicated UnknownPeer signal,
        // not the misleading PubkeyRecordMissing. Pre-fix this test
        // expected `sbo3l_pubkey_ed25519_record_missing`; the new
        // distinction makes the operator's recovery path obvious.
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("peer_fqdn_not_in_ens")
        );
    }

    #[test]
    fn pubkey_record_missing_distinct_from_unknown_peer() {
        // Same FQDN registered in ENS but with no sbo3l:pubkey_ed25519
        // record set yet — reject with PubkeyRecordMissing, NOT
        // UnknownPeer. Operator fix is "set the record", not
        // "register the FQDN".
        let a_key = fixed_key(&[19u8; 32]);
        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert_missing("research-agent.sbo3lagent.eth");

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("sbo3l_pubkey_ed25519_record_missing")
        );
    }

    #[test]
    fn pubkey_record_malformed_rejects() {
        let a_key = fixed_key(&[14u8; 32]);
        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", "not-a-hex-pubkey");

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("sbo3l_pubkey_ed25519_record_malformed")
        );
    }

    #[test]
    fn signature_byte_flip_rejects() {
        let a_key = fixed_key(&[15u8; 32]);
        let a_pub_hex = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        let mut signed = sign_challenge(&challenge, &a_key).unwrap();

        // Flip the last hex char of the signature.
        let len = signed.signature_hex.len();
        let mut chars: Vec<char> = signed.signature_hex.chars().collect();
        chars[len - 1] = if chars[len - 1] == '0' { '1' } else { '0' };
        signed.signature_hex = chars.into_iter().collect();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub_hex);

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("signature_mismatch")
        );
    }

    #[test]
    fn stale_challenge_rejects() {
        let a_key = fixed_key(&[16u8; 32]);
        let a_pub_hex = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let now: u64 = 1_700_000_000_000;
        let stale = now.saturating_sub(FRESHNESS_WINDOW_MS + 1);
        let challenge = make_challenge("research-agent.sbo3lagent.eth", stale);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub_hex);

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("challenge_outside_freshness_window")
        );
    }

    #[test]
    fn future_challenge_rejects() {
        let a_key = fixed_key(&[17u8; 32]);
        let a_pub_hex = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let now: u64 = 1_700_000_000_000;
        let future = now + FRESHNESS_WINDOW_MS + 1;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", future);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub_hex);

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(
            trust.rejection_reason.as_deref(),
            Some("challenge_outside_freshness_window")
        );
    }

    #[test]
    fn schema_mismatch_rejects() {
        let a_key = fixed_key(&[18u8; 32]);
        let a_pub_hex = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let now: u64 = 1_700_000_000_000;
        let mut challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        // Pretend a v2 challenge was sent against this v1 verifier.
        challenge.schema = "sbo3l.cross_agent_challenge.v2".to_string();
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub_hex);

        let trust = verify_challenge(&signed, &resolver, now).unwrap();
        assert!(!trust.valid);
        assert_eq!(trust.rejection_reason.as_deref(), Some("schema_mismatch"));
    }

    #[test]
    fn jcs_canonicalisation_is_stable() {
        // Re-serialising the same struct yields identical bytes.
        let challenge = make_challenge("a.sbo3lagent.eth", 1_700_000_000_000);
        let a = jcs_bytes(&challenge).unwrap();
        let b = jcs_bytes(&challenge).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn pair_swap_each_verifies_the_other() {
        // Two agents in the same test cross-verify each other's
        // challenges. This is the "pair test" Daniel asked for.
        let a_key = fixed_key(&[20u8; 32]);
        let b_key = fixed_key(&[21u8; 32]);
        let a_pub = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let b_pub = format!("0x{}", hex::encode(b_key.verifying_key().to_bytes()));

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("a.sbo3lagent.eth", &a_pub);
        resolver.insert("b.sbo3lagent.eth", &b_pub);

        let now: u64 = 1_700_000_000_000;

        // A → B
        let a_challenge = make_challenge("a.sbo3lagent.eth", now);
        let a_signed = sign_challenge(&a_challenge, &a_key).unwrap();
        let trust_a = verify_challenge(&a_signed, &resolver, now).unwrap();
        assert!(trust_a.valid, "A→B failed: {:?}", trust_a.rejection_reason);
        assert_eq!(trust_a.peer_fqdn, "a.sbo3lagent.eth");

        // B → A
        let b_challenge = make_challenge("b.sbo3lagent.eth", now);
        let b_signed = sign_challenge(&b_challenge, &b_key).unwrap();
        let trust_b = verify_challenge(&b_signed, &resolver, now).unwrap();
        assert!(trust_b.valid, "B→A failed: {:?}", trust_b.rejection_reason);
        assert_eq!(trust_b.peer_fqdn, "b.sbo3lagent.eth");

        // Each receipt pins its own peer (no cross-contamination).
        assert_ne!(trust_a.peer_pubkey_hex, trust_b.peer_pubkey_hex);
    }

    #[test]
    fn build_challenge_uses_schema_and_supplied_fields() {
        let c = build_challenge(
            "research-agent.sbo3lagent.eth",
            "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            "0xabababababababababababababababab",
        )
        .unwrap();
        assert_eq!(c.schema, CHALLENGE_SCHEMA);
        assert_eq!(c.agent_fqdn, "research-agent.sbo3lagent.eth");
        assert!(c.ts_ms > 0);
    }

    #[test]
    fn signed_envelope_round_trips_through_json() {
        // Wire-shape sanity: serialise + parse is identity, and the
        // parsed envelope still verifies.
        let a_key = fixed_key(&[22u8; 32]);
        let a_pub = format!("0x{}", hex::encode(a_key.verifying_key().to_bytes()));
        let now: u64 = 1_700_000_000_000;
        let challenge = make_challenge("research-agent.sbo3lagent.eth", now);
        let signed = sign_challenge(&challenge, &a_key).unwrap();

        let json = serde_json::to_string(&signed).unwrap();
        let parsed: SignedChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(signed, parsed);

        let mut resolver = FakePubkeyResolver::new();
        resolver.insert("research-agent.sbo3lagent.eth", &a_pub);
        let trust = verify_challenge(&parsed, &resolver, now).unwrap();
        assert!(trust.valid);
    }

    #[test]
    fn signed_envelope_rejects_unknown_top_level_keys() {
        // deny_unknown_fields forward-compat: a hostile client adding
        // a `payload_extra` field should fail to parse, not silently
        // get ignored.
        let json = r#"{
            "challenge": {
                "schema": "sbo3l.cross_agent_challenge.v1",
                "agent_fqdn": "a.sbo3lagent.eth",
                "audit_chain_head_hex": "00",
                "nonce_hex": "00",
                "ts_ms": 1
            },
            "signature_hex": "0x00",
            "payload_extra": "evil"
        }"#;
        assert!(serde_json::from_str::<SignedChallenge>(json).is_err());
    }
}
