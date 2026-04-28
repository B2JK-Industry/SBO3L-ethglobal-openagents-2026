//! Production-shaped **mock** KMS signer.
//!
//! `MockKmsSigner` looks and behaves like a key-managed signing service: it
//! has stable `key_id`s, explicit `key_version`s, public-key metadata, and
//! supports rotation while keeping historical keys verifiable. It is
//! deliberately **mock** — keys are derived deterministically from a local
//! root seed (no HSM, no TEE, no remote KMS, no network). The only thing
//! it shares with a real KMS is the *shape* of the API and the lifecycle
//! semantics (rotation, version-by-version verification).
//!
//! Truthfulness rules (do not violate):
//! - Every public artefact must say "mock" explicitly.
//! - The struct's seeds, public keys, and key_ids are derived from a local
//!   value; they are **not** secrets in the production sense and must not
//!   be presented as such.
//! - This is not a stepping stone "you flip one bit and it's production".
//!   A real KMS implementation would replace the `derive_signing_key`
//!   call with a call to the KMS API, AND change the trust model in many
//!   other places (key custody, audit, attestation, recovery, etc).
//!
//! What `MockKmsSigner` does provide:
//! - A versioned keyring under a stable role name (e.g. `audit-mock`).
//! - Each version has a `key_id` like `audit-mock-v1`, `audit-mock-v2`, …
//!   recorded verbatim in `EmbeddedSignature.key_id` on the receipts/
//!   audit events / decision tokens it signs.
//! - `rotate()` advances the current version. Past versions stay available
//!   for verifying earlier receipts.
//! - The signing path goes through the same `SignerBackend` trait used by
//!   `DevSigner`, so swapping backends is a one-line change at the call
//!   site.

use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use sha2::Digest;

use crate::signer::{verify_hex, DevSigner, SignerBackend, VerifyError};

/// Metadata describing one version of a mock-KMS key. Exposed via
/// `MockKmsSigner::versions()` so callers (CLI, tests, docs) can enumerate
/// the keyring without reaching into private state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockKmsKeyMeta {
    pub role: String,
    pub version: u32,
    pub key_id: String,
    pub public_hex: String,
    /// When this version was added to the keyring. Deterministic from the
    /// caller-supplied clock so tests and demos stay reproducible.
    pub created_at: DateTime<Utc>,
    /// Always `true` — this struct only represents mock keys. Surfaced as
    /// a field so JSON/CLI output cannot accidentally drop the disclosure.
    pub mock: bool,
}

/// Mock KMS-shaped Ed25519 signer with rotation.
///
/// Construct with a stable `role` (e.g. `"audit-mock"` or
/// `"decision-mock"`) and a 32-byte `root_seed`. Rotations are pure
/// in-memory state — persistence (so `mandate key rotate --mock` can
/// outlive a CLI invocation) is a follow-up wired in `mandate-storage`.
#[derive(Debug, Clone)]
pub struct MockKmsSigner {
    role: String,
    root_seed: [u8; 32],
    /// Genesis timestamp for v1; subsequent versions are spaced by one
    /// nanosecond so `created_at` is monotonically increasing without
    /// pulling in a real clock.
    genesis: DateTime<Utc>,
    versions: Vec<MockKmsKeyMeta>,
    current_index: usize,
}

impl MockKmsSigner {
    /// Build a fresh keyring with v1 derived from `(root_seed, role, 1)`.
    /// `genesis` is the deterministic "created_at" anchor for v1.
    pub fn new(role: impl Into<String>, root_seed: [u8; 32], genesis: DateTime<Utc>) -> Self {
        let role = role.into();
        let v1 = Self::build_meta(&role, 1, &root_seed, genesis);
        Self {
            role,
            root_seed,
            genesis,
            versions: vec![v1],
            current_index: 0,
        }
    }

    /// Reconstruct a keyring with `current_version` already advanced. Used
    /// by callers that persist the rotation state externally and want to
    /// rebuild the in-memory keyring on startup.
    pub fn from_versions(
        role: impl Into<String>,
        root_seed: [u8; 32],
        genesis: DateTime<Utc>,
        current_version: u32,
    ) -> Self {
        assert!(current_version >= 1, "version numbering starts at 1");
        let role = role.into();
        let versions: Vec<_> = (1..=current_version)
            .map(|v| Self::build_meta(&role, v, &root_seed, genesis))
            .collect();
        let current_index = (current_version as usize) - 1;
        Self {
            role,
            root_seed,
            genesis,
            versions,
            current_index,
        }
    }

    fn build_meta(
        role: &str,
        version: u32,
        root_seed: &[u8; 32],
        genesis: DateTime<Utc>,
    ) -> MockKmsKeyMeta {
        let signing_key = derive_signing_key(role, version, root_seed);
        let public_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let key_id = format!("{role}-v{version}");
        // Space versions by one nanosecond so the timeline is strictly
        // monotonic but still derives only from the caller's `genesis`.
        let created_at =
            genesis + chrono::Duration::nanoseconds(((version as i64).saturating_sub(1)).max(0));
        MockKmsKeyMeta {
            role: role.to_string(),
            version,
            key_id,
            public_hex,
            created_at,
            mock: true,
        }
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn current_version(&self) -> u32 {
        self.versions[self.current_index].version
    }

    /// Metadata for the current (active for new signatures) version.
    pub fn current(&self) -> &MockKmsKeyMeta {
        &self.versions[self.current_index]
    }

    /// All keyring entries from v1 through the current version, in order.
    pub fn versions(&self) -> &[MockKmsKeyMeta] {
        &self.versions
    }

    /// Look up a keyring entry by its full `key_id` (e.g. `audit-mock-v2`).
    /// Returns `None` if the id is unknown.
    pub fn key_by_id(&self, key_id: &str) -> Option<&MockKmsKeyMeta> {
        self.versions.iter().find(|m| m.key_id == key_id)
    }

    /// Look up a keyring entry by `version` (1-indexed).
    pub fn key_by_version(&self, version: u32) -> Option<&MockKmsKeyMeta> {
        self.versions.iter().find(|m| m.version == version)
    }

    /// Add a new version. The new version becomes the current signing
    /// key; previous versions remain available for verification of
    /// earlier signatures via `verify(...)`.
    pub fn rotate(&mut self) -> &MockKmsKeyMeta {
        let next_version = self.current_version() + 1;
        let meta = Self::build_meta(&self.role, next_version, &self.root_seed, self.genesis);
        self.versions.push(meta);
        self.current_index = self.versions.len() - 1;
        self.current()
    }

    fn current_signing_key(&self) -> SigningKey {
        derive_signing_key(&self.role, self.current_version(), &self.root_seed)
    }

    /// Verify a signature claimed to be produced under `key_id`. Resolves
    /// the keyring entry, then runs the standard Ed25519 verify.
    pub fn verify(
        &self,
        key_id: &str,
        message: &[u8],
        signature_hex: &str,
    ) -> Result<(), VerifyError> {
        let meta = self.key_by_id(key_id).ok_or(VerifyError::BadPublicKey)?;
        verify_hex(&meta.public_hex, message, signature_hex)
    }

    /// Convenience: derive a `DevSigner` for the *current* version. Useful
    /// when a caller wants to interoperate with code that holds onto a
    /// `DevSigner` directly (e.g. existing `audit_append` paths) without
    /// committing to the keyring lifecycle.
    pub fn current_as_dev_signer(&self) -> DevSigner {
        DevSigner::from_seed(
            self.current().key_id.clone(),
            seed_for(&self.role, self.current_version(), &self.root_seed),
        )
    }
}

impl SignerBackend for MockKmsSigner {
    fn current_key_id(&self) -> &str {
        &self.current().key_id
    }
    fn sign_hex(&self, message: &[u8]) -> String {
        use ed25519_dalek::Signer as _;
        let sk = self.current_signing_key();
        let sig = sk.sign(message);
        hex::encode(sig.to_bytes())
    }
    fn current_public_hex(&self) -> String {
        self.current().public_hex.clone()
    }
}

/// Build the `(key_id, public_hex)` pair that
/// `MockKmsSigner::new` / `rotate` would produce for `(role, version,
/// root_seed)`. Useful for callers that store keyring metadata
/// externally (e.g. SQLite via `mandate-storage::mock_kms_store`) and
/// need to derive the next version's public material without holding
/// a `MockKmsSigner` instance.
pub fn derive_key_metadata(role: &str, version: u32, root_seed: &[u8; 32]) -> (String, String) {
    let signing_key = derive_signing_key(role, version, root_seed);
    let public_hex = hex::encode(signing_key.verifying_key().to_bytes());
    let key_id = format!("{role}-v{version}");
    (key_id, public_hex)
}

/// Deterministic per-version seed. **Not** a production KDF — a real KMS
/// would never derive private keys from a public-ish role+version tuple.
/// This is sufficient for reproducible local testing where the same
/// (root_seed, role, version) triple yields the same Ed25519 keypair.
fn seed_for(role: &str, version: u32, root_seed: &[u8; 32]) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"mandate.mock_kms.v1");
    hasher.update(root_seed);
    hasher.update((role.len() as u32).to_be_bytes());
    hasher.update(role.as_bytes());
    hasher.update(version.to_be_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn derive_signing_key(role: &str, version: u32, root_seed: &[u8; 32]) -> SigningKey {
    SigningKey::from_bytes(&seed_for(role, version, root_seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s).unwrap().into()
    }

    #[test]
    fn fresh_signer_starts_at_v1() {
        let s = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        assert_eq!(s.current_version(), 1);
        assert_eq!(s.current_key_id(), "audit-mock-v1");
        assert_eq!(s.versions().len(), 1);
        assert!(s.current().mock, "metadata must surface mock = true");
    }

    #[test]
    fn key_metadata_is_deterministic() {
        // Same (role, root_seed, genesis) → same keyring exactly. This is
        // the property tests, fixtures and demos rely on.
        let a = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        let b = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        assert_eq!(a.versions(), b.versions());
    }

    #[test]
    fn different_roots_yield_different_keys() {
        let a = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        let b = MockKmsSigner::new("audit-mock", [99u8; 32], ts("2026-04-28T00:00:00Z"));
        assert_ne!(a.current().public_hex, b.current().public_hex);
    }

    #[test]
    fn signer_backend_round_trip() {
        // Sign through the trait; verify through the keyring's own verify
        // (which looks up the public key by key_id).
        let s = MockKmsSigner::new("decision-mock", [7u8; 32], ts("2026-04-28T00:00:00Z"));
        let msg = b"some canonical payload";
        let sig = SignerBackend::sign_hex(&s, msg);
        s.verify(s.current_key_id(), msg, &sig)
            .expect("self-verify must succeed");
    }

    #[test]
    fn signer_backend_reports_consistent_metadata() {
        let s = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        // current_key_id and current_public_hex must agree with the
        // keyring's metadata, otherwise verifiers and signers would
        // disagree about what just got signed.
        assert_eq!(s.current_key_id(), s.current().key_id);
        assert_eq!(s.current_public_hex(), s.current().public_hex);
    }

    #[test]
    fn rotate_advances_current_version() {
        let mut s = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        let before = s.current().clone();
        let after = s.rotate().clone();
        assert_eq!(after.version, 2);
        assert_eq!(after.key_id, "audit-mock-v2");
        assert_ne!(
            after.public_hex, before.public_hex,
            "rotation must change public key"
        );
        assert_eq!(s.current_version(), 2);
        assert_eq!(s.versions().len(), 2);
    }

    #[test]
    fn old_signature_still_verifies_after_rotation() {
        // The whole point of carrying historical keys: an audit event
        // signed before a rotation must remain verifiable after.
        let mut s = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        let v1_key_id = s.current_key_id().to_string();
        let msg = b"pre-rotation message";
        let sig = SignerBackend::sign_hex(&s, msg);

        s.rotate();
        assert_eq!(s.current_version(), 2);

        // v1 signature still verifies under the v1 keyring entry.
        s.verify(&v1_key_id, msg, &sig)
            .expect("v1 signature must still verify under v1 pubkey after rotation");
        // And the v2 pubkey must NOT verify the v1 signature.
        let v2_key_id = s.current_key_id().to_string();
        let res = s.verify(&v2_key_id, msg, &sig);
        assert!(matches!(res, Err(VerifyError::Invalid)));
    }

    #[test]
    fn wrong_key_id_returns_bad_public_key() {
        let s = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        let msg = b"x";
        let sig = SignerBackend::sign_hex(&s, msg);
        let res = s.verify("audit-mock-v999", msg, &sig);
        assert!(matches!(res, Err(VerifyError::BadPublicKey)));
    }

    #[test]
    fn from_versions_reconstructs_same_keyring_after_rotations() {
        // Persistence story (when the rotation count is stored elsewhere
        // and we rebuild the in-memory keyring at startup): reconstructing
        // with `current_version=N` produces the same N entries as
        // `new(...)` followed by `N-1` rotations.
        let mut grown = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        for _ in 0..3 {
            grown.rotate();
        }
        let restored =
            MockKmsSigner::from_versions("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"), 4);
        assert_eq!(grown.versions(), restored.versions());
        assert_eq!(grown.current_version(), restored.current_version());
    }

    #[test]
    fn current_as_dev_signer_produces_compatible_signatures() {
        // The compat shim lets MockKmsSigner interoperate with code that
        // still takes `&DevSigner`. The DevSigner it returns must produce
        // signatures verifiable under the keyring's current public key.
        let s = MockKmsSigner::new("audit-mock", [42u8; 32], ts("2026-04-28T00:00:00Z"));
        let dev = s.current_as_dev_signer();
        assert_eq!(dev.current_key_id(), s.current_key_id());
        let msg = b"compat";
        let sig = dev.sign_hex(msg);
        s.verify(s.current_key_id(), msg, &sig)
            .expect("DevSigner-produced signature must verify under MockKms keyring");
    }

    // -- Cross-cutting integration tests ------------------------------------
    //
    // The point of `SignerBackend` is that the three Mandate signing
    // surfaces (receipts, audit events, decision tokens) work with any
    // backend without code changes. Below: sign through MockKmsSigner,
    // then verify using the same code paths the daemon uses today.

    use crate::audit::{AuditEvent, SignedAuditEvent, ZERO_HASH};
    use crate::decision_token::{DecisionPayload, TxTemplate};
    use crate::receipt::{Decision, UnsignedReceipt};

    fn unsigned_receipt(audit_event_id: &str) -> UnsignedReceipt {
        UnsignedReceipt {
            agent_id: "research-agent-01".to_string(),
            decision: Decision::Allow,
            deny_code: None,
            request_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            policy_hash: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            policy_version: Some(1),
            audit_event_id: audit_event_id.to_string(),
            execution_ref: None,
            issued_at: ts("2026-04-27T12:00:01.500Z"),
            expires_at: None,
        }
    }

    fn audit_event(seq: u64) -> AuditEvent {
        AuditEvent {
            version: 1,
            seq,
            id: format!("evt-mock-kms-{seq:03}"),
            ts: ts("2026-04-27T12:00:01Z"),
            event_type: "policy_decided".to_string(),
            actor: "policy_engine".to_string(),
            subject_id: format!("pr-mock-{seq:03}"),
            payload_hash: ZERO_HASH.to_string(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: None,
            attestation_ref: None,
            prev_event_hash: ZERO_HASH.to_string(),
        }
    }

    fn decision_payload(request_hash: &str) -> DecisionPayload {
        DecisionPayload {
            version: 1,
            request_hash: request_hash.to_string(),
            decision: Decision::Allow,
            deny_code: None,
            policy_version: 1,
            policy_hash: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            tx_template: TxTemplate {
                chain_id: 8453,
                to: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
                value: "0".to_string(),
                data: "0x".to_string(),
                gas_limit: 100_000,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                nonce_hint: None,
            },
            key_id: "agent-research-01-key".to_string(),
            decision_id: "dec-mock-kms-001".to_string(),
            issued_at: ts("2026-04-27T12:00:00Z"),
            expires_at: ts("2026-04-27T12:05:00Z"),
            attestation_ref: None,
        }
    }

    #[test]
    fn receipt_round_trip_through_mock_kms() {
        // The receipt's signature.key_id MUST be the keyring's current
        // key_id (so a verifier can resolve which version produced it),
        // and verifying with the matching public key MUST succeed.
        let s = MockKmsSigner::new("decision-mock", [7u8; 32], ts("2026-04-28T00:00:00Z"));
        let receipt = unsigned_receipt("evt-mock-kms-001").sign(&s).unwrap();
        assert_eq!(receipt.signature.key_id, s.current_key_id());
        receipt
            .verify(&s.current_public_hex())
            .expect("receipt must verify under the MockKms keyring's current pubkey");
    }

    #[test]
    fn audit_event_round_trip_through_mock_kms() {
        let s = MockKmsSigner::new("audit-mock", [11u8; 32], ts("2026-04-28T00:00:00Z"));
        let signed = SignedAuditEvent::sign(audit_event(1), &s).unwrap();
        assert_eq!(signed.signature.key_id, s.current_key_id());
        signed
            .verify_signature(&s.current_public_hex())
            .expect("audit event must verify under MockKms keyring");
    }

    #[test]
    fn decision_token_round_trip_through_mock_kms() {
        // DecisionToken records signing_pubkey_hex inline, so the
        // verifier doesn't need keyring access at all.
        let s = MockKmsSigner::new("decision-mock", [9u8; 32], ts("2026-04-28T00:00:00Z"));
        let payload =
            decision_payload("c0bd2fab4a7d4686d686edcc9c8356315cd66b820a2072493bf758a1eeb500db");
        let token = payload.sign(&s).unwrap();
        assert_eq!(token.signing_pubkey_hex, s.current_public_hex());
        token.verify().expect("decision token must verify");
    }

    #[test]
    fn pre_rotation_receipt_still_verifies_after_rotation() {
        // End-to-end version of the keyring's
        // `old_signature_still_verifies_after_rotation` — but exercised
        // through the receipt code path. A receipt signed by v1 must keep
        // verifying after the keyring rotates to v2, as long as the
        // verifier uses the *v1* pubkey resolved via key_id.
        let mut s = MockKmsSigner::new("decision-mock", [7u8; 32], ts("2026-04-28T00:00:00Z"));
        let v1_pub = s.current_public_hex();
        let receipt_v1 = unsigned_receipt("evt-mock-kms-001").sign(&s).unwrap();
        assert_eq!(receipt_v1.signature.key_id, "decision-mock-v1");

        s.rotate();
        let v2_pub = s.current_public_hex();
        assert_ne!(v1_pub, v2_pub);

        // Resolve the historic pubkey via the keyring; verify succeeds.
        let resolved = s
            .key_by_id(&receipt_v1.signature.key_id)
            .expect("keyring still knows about v1");
        receipt_v1
            .verify(&resolved.public_hex)
            .expect("pre-rotation receipt must verify under the resolved v1 pubkey");

        // And new receipts signed after rotation embed the v2 key_id.
        let receipt_v2 = unsigned_receipt("evt-mock-kms-002").sign(&s).unwrap();
        assert_eq!(receipt_v2.signature.key_id, "decision-mock-v2");
        receipt_v2
            .verify(&v2_pub)
            .expect("v2 receipt must verify under v2 pubkey");
        // Cross-pollination must fail: v1 receipt MUST NOT verify under v2.
        assert!(receipt_v1.verify(&v2_pub).is_err());
    }
}
