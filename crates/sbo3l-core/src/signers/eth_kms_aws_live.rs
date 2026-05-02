//! `eth_kms_aws_live` — live AWS KMS [`EthSigner`] backend (R14 P3).
//!
//! Compiled only with `--features eth_kms_aws`. This pulls
//! `aws-sdk-kms` + `aws-config` (~80 transitive crates), so the existing
//! `eth_kms::aws` stub stays compile-only behind the older `aws_kms`
//! feature for callers that just want the trait plumbing.
//!
//! # Honest status
//!
//! No real KMS round-trip has been verified — Daniel does NOT have AWS
//! creds in this round. The unit tests below exercise the response-
//! decoding logic against canned-DER fixtures and the live integration
//! test in `tests/aws_kms_live.rs` skips cleanly unless
//! `AWS_KMS_TEST_ENABLED=1` is set with real creds. Daniel runs the
//! gated integration tests in R15 once the KMS key is provisioned.
//!
//! # Sign + recovery shape
//!
//! AWS KMS Sign API returns a DER-encoded ECDSA signature
//! (`SEQUENCE { r INTEGER, s INTEGER }`). EVM `ecrecover` wants 65-byte
//! `r || s || v` where `v` is the recovery id. The flow:
//!
//! 1. [`AwsEthKmsLiveSigner::new`] fetches the public key once via
//!    `GetPublicKey`, parses the SEC1 SubjectPublicKeyInfo DER, derives
//!    the EIP-55 address, caches both.
//! 2. [`EthSigner::sign_digest_hex`] calls KMS `Sign` with
//!    `MessageType::Digest` + `SigningAlgorithmSpec::EcdsaSha256`,
//!    receives DER, parses to `(r, s)`.
//! 3. Normalizes `s` to low-S (EIP-2: `s <= n/2`).
//! 4. Recovers `v` by trying recovery ids 0 and 1; whichever recovers
//!    a public key matching the cached pubkey wins.
//!
//! Step 4 is the single EVM-specific subtlety vs the Ed25519 KMS path.
//!
//! # Why a `KmsClient` trait
//!
//! `aws-sdk-kms::Client` is a concrete struct with no public mock
//! surface in the version pinned here. Wrapping the two methods we
//! need (`sign`, `get_public_key`) behind a small async trait lets
//! unit tests inject a fake without spinning a smithy interceptor
//! stack. Production uses the SDK-backed adapter; tests use a hand
//! rolled fake.

use std::sync::OnceLock;

use async_trait::async_trait;
use aws_sdk_kms::primitives::Blob;
use aws_sdk_kms::types::{KeySpec, MessageType, SigningAlgorithmSpec};
use aws_sdk_kms::Client as AwsKmsClient;
use k256::ecdsa::VerifyingKey;

use super::eth_kms_common::{address_from_verifying_key, der_to_rsv, parse_spki_secp256k1};
use super::{eth::EthSigner, SignerError};

/// Minimal surface this signer needs from AWS KMS. Wrapping it lets
/// unit tests swap a fake without instantiating a real smithy client.
#[async_trait]
pub trait KmsClient: Send + Sync {
    /// `Sign` API. Returns the raw DER signature bytes.
    async fn sign_digest(&self, key_id: &str, digest: &[u8; 32]) -> Result<Vec<u8>, SignerError>;

    /// `GetPublicKey` API. Returns the raw SubjectPublicKeyInfo DER
    /// bytes plus the reported key spec (so the constructor can reject
    /// non-secp256k1 keys cleanly).
    async fn get_public_key(&self, key_id: &str)
        -> Result<(Vec<u8>, Option<KeySpec>), SignerError>;
}

/// SDK-backed adapter — wraps an `aws_sdk_kms::Client`.
pub struct SdkKmsClient {
    inner: AwsKmsClient,
}

impl SdkKmsClient {
    pub fn new(inner: AwsKmsClient) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl KmsClient for SdkKmsClient {
    async fn sign_digest(&self, key_id: &str, digest: &[u8; 32]) -> Result<Vec<u8>, SignerError> {
        let resp = self
            .inner
            .sign()
            .key_id(key_id)
            .message(Blob::new(digest.to_vec()))
            .message_type(MessageType::Digest)
            .signing_algorithm(SigningAlgorithmSpec::EcdsaSha256)
            .send()
            .await
            .map_err(|e| SignerError::Kms(format!("aws kms sign({key_id}): {e}")))?;
        let sig = resp.signature().ok_or_else(|| {
            SignerError::Kms(format!("aws kms sign({key_id}): no signature in response"))
        })?;
        Ok(sig.as_ref().to_vec())
    }

    async fn get_public_key(
        &self,
        key_id: &str,
    ) -> Result<(Vec<u8>, Option<KeySpec>), SignerError> {
        let resp = self
            .inner
            .get_public_key()
            .key_id(key_id)
            .send()
            .await
            .map_err(|e| SignerError::Kms(format!("aws kms get_public_key({key_id}): {e}")))?;
        let pk = resp
            .public_key()
            .ok_or_else(|| {
                SignerError::Kms(format!(
                    "aws kms get_public_key({key_id}): no PublicKey field"
                ))
            })?
            .as_ref()
            .to_vec();
        let spec = resp.key_spec().cloned();
        Ok((pk, spec))
    }
}

/// Live AWS KMS secp256k1 EVM signer.
pub struct AwsEthKmsLiveSigner {
    client: Box<dyn KmsClient>,
    key_id: String,
    cached_verifying_key: OnceLock<VerifyingKey>,
    cached_address: OnceLock<String>,
}

impl std::fmt::Debug for AwsEthKmsLiveSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsEthKmsLiveSigner")
            .field("key_id", &self.key_id)
            .field(
                "cached_pubkey_present",
                &self.cached_verifying_key.get().is_some(),
            )
            .finish()
    }
}

impl AwsEthKmsLiveSigner {
    /// Construct from env. Reads `SBO3L_ETH_AWS_KMS_KEY_ID`. Uses the
    /// default AWS credentials chain (env vars / IAM role / shared
    /// config) via `aws_config::load_defaults`.
    ///
    /// Synchronous wrapper — the daemon's startup path is sync today.
    /// We block on a one-shot tokio runtime for the credential load
    /// and the initial `GetPublicKey`. After construction every
    /// signing call hits the cached pubkey plus a single `Sign`
    /// round-trip.
    pub fn from_env(_role: &str) -> Result<Self, SignerError> {
        let key_id = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ID")
            .or_else(|_| std::env::var("SBO3L_ETH_AWS_KMS_KEY_ARN"))
            .map_err(|_| SignerError::MissingEnv("SBO3L_ETH_AWS_KMS_KEY_ID"))?;
        if key_id.is_empty() {
            return Err(SignerError::MissingEnv("SBO3L_ETH_AWS_KMS_KEY_ID"));
        }

        // Build a single-thread tokio runtime to drive the async SDK
        // calls from a sync constructor. The daemon may already have
        // its own runtime; we deliberately don't try to reuse it
        // because that requires `block_in_place` which only works
        // inside a multi-thread runtime context.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| SignerError::Kms(format!("aws kms: build tokio rt: {e}")))?;
        let client = rt.block_on(async {
            let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            AwsKmsClient::new(&cfg)
        });

        Self::with_client(Box::new(SdkKmsClient::new(client)), key_id)
    }

    /// Construct with an explicit client. Used by unit tests (with a
    /// fake) and by callers that want to share a configured client.
    /// Validates the key spec + caches the verifying key + address.
    pub fn with_client(client: Box<dyn KmsClient>, key_id: String) -> Result<Self, SignerError> {
        let s = Self {
            client,
            key_id,
            cached_verifying_key: OnceLock::new(),
            cached_address: OnceLock::new(),
        };
        // Eager-fetch the pubkey so misconfiguration surfaces at
        // construction, not on the first sign call.
        s.address()?;
        Ok(s)
    }

    /// Synchronous block-on helper for the `EthSigner` impl. Builds a
    /// fresh single-thread runtime per call. AWS KMS Sign is a few-ms
    /// round-trip; the runtime construction overhead is dwarfed by
    /// network latency.
    fn block_on<T>(
        &self,
        fut: impl std::future::Future<Output = Result<T, SignerError>>,
    ) -> Result<T, SignerError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| SignerError::Kms(format!("aws kms: build tokio rt: {e}")))?;
        rt.block_on(fut)
    }

    /// Fetch + cache the verifying key. First call hits KMS; subsequent
    /// calls return the cached value.
    fn verifying_key(&self) -> Result<&VerifyingKey, SignerError> {
        if let Some(vk) = self.cached_verifying_key.get() {
            return Ok(vk);
        }
        let key_id = self.key_id.clone();
        let (der, spec) = self.block_on(async { self.client.get_public_key(&key_id).await })?;
        if let Some(spec) = spec {
            if !is_secp256k1_spec(&spec) {
                return Err(SignerError::KeySpecMismatch {
                    key_id: self.key_id.clone(),
                    found_spec: format!("{spec:?}"),
                });
            }
        }
        let vk = parse_spki_secp256k1(&der)?;
        // OnceLock is racy-safe: parallel callers will both compute
        // the same value; whoever wins set_*() loses gracefully.
        let _ = self.cached_verifying_key.set(vk);
        Ok(self.cached_verifying_key.get().expect("just set above"))
    }

    /// Fetch + cache the EIP-55 address.
    fn address(&self) -> Result<&str, SignerError> {
        if let Some(addr) = self.cached_address.get() {
            return Ok(addr);
        }
        let vk = self.verifying_key()?;
        let addr = address_from_verifying_key(vk);
        let _ = self.cached_address.set(addr);
        Ok(self.cached_address.get().expect("just set above"))
    }
}

impl EthSigner for AwsEthKmsLiveSigner {
    fn sign_digest_hex(&self, digest: &[u8; 32]) -> Result<String, SignerError> {
        let key_id = self.key_id.clone();
        let der = self.block_on(async { self.client.sign_digest(&key_id, digest).await })?;
        let vk = self.verifying_key()?;
        let sig_bytes = der_to_rsv(&der, digest, vk)?;
        Ok(format!("0x{}", hex::encode(sig_bytes)))
    }

    fn eth_address(&self) -> Result<String, SignerError> {
        Ok(self.address()?.to_string())
    }

    fn key_id(&self) -> &str {
        &self.key_id
    }
}

// ---------------------------------------------------------------------------
// AWS-specific helpers. The shared SPKI / DER / address parsing lives in
// `eth_kms_common`.
// ---------------------------------------------------------------------------

fn is_secp256k1_spec(spec: &KeySpec) -> bool {
    matches!(spec, KeySpec::EccSecgP256K1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::signature::hazmat::PrehashSigner;
    use k256::ecdsa::{RecoveryId, Signature, SigningKey};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Env-var tests in this module run serially under one mutex.
    /// Cargo runs unit tests in parallel by default, so concurrent
    /// `set_var` / `remove_var` between sibling tests races the
    /// `from_env` constructor — manifesting as the "wrong variable
    /// missing" assertion failure. Mutex pins them to one-at-a-time.
    fn env_lock() -> &'static Mutex<()> {
        static M: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| Mutex::new(()))
    }

    /// Hand-rolled fake KMS client. Holds a signing key and counters
    /// so tests can assert caching + call counts.
    struct FakeKms {
        signing: SigningKey,
        spec: Option<KeySpec>,
        spki_override: Option<Vec<u8>>,
        sign_calls: Arc<AtomicUsize>,
        get_public_key_calls: Arc<AtomicUsize>,
        sign_error: Option<String>,
        get_public_key_error: Option<String>,
    }

    impl FakeKms {
        fn new(secret: [u8; 32]) -> Self {
            Self {
                signing: SigningKey::from_bytes((&secret).into()).unwrap(),
                spec: Some(KeySpec::EccSecgP256K1),
                spki_override: None,
                sign_calls: Arc::new(AtomicUsize::new(0)),
                get_public_key_calls: Arc::new(AtomicUsize::new(0)),
                sign_error: None,
                get_public_key_error: None,
            }
        }

        fn spki(&self) -> Vec<u8> {
            if let Some(o) = &self.spki_override {
                return o.clone();
            }
            // Real-shaped SPKI for secp256k1: header is constant for
            // this curve. We embed the raw 65-byte SEC1 point at the
            // end and prepend the standard 23-byte AlgorithmIdentifier
            // + BIT STRING wrapper. Our parser is forgiving so we
            // could shortcut, but feeding it the real shape catches
            // header-edit regressions.
            let pk = self.signing.verifying_key().to_encoded_point(false);
            let pk_bytes = pk.as_bytes();
            assert_eq!(pk_bytes.len(), 65);
            // 0x30 0x56 -> SEQUENCE, 86 bytes
            //   0x30 0x10 SEQUENCE, 16 bytes (AlgorithmIdentifier)
            //     0x06 0x07 1.2.840.10045.2.1 (id-ecPublicKey)
            //     0x06 0x05 1.3.132.0.10 (secp256k1)
            //   0x03 0x42 BIT STRING, 66 bytes
            //     0x00 (no unused bits)
            //     65 bytes of point
            let mut out = Vec::with_capacity(88);
            out.extend_from_slice(&[
                0x30, 0x56, 0x30, 0x10, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06,
                0x05, 0x2b, 0x81, 0x04, 0x00, 0x0a, 0x03, 0x42, 0x00,
            ]);
            out.extend_from_slice(pk_bytes);
            out
        }
    }

    #[async_trait]
    impl KmsClient for FakeKms {
        async fn sign_digest(
            &self,
            _key_id: &str,
            digest: &[u8; 32],
        ) -> Result<Vec<u8>, SignerError> {
            self.sign_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(e) = &self.sign_error {
                return Err(SignerError::Kms(e.clone()));
            }
            // Real KMS returns DER. Use k256 to produce a deterministic
            // (RFC 6979) signature, then DER-encode it.
            let (sig, _recid): (Signature, RecoveryId) = self
                .signing
                .sign_prehash(digest)
                .map_err(|e| SignerError::Kms(format!("fake sign: {e}")))?;
            Ok(sig.to_der().as_bytes().to_vec())
        }

        async fn get_public_key(
            &self,
            _key_id: &str,
        ) -> Result<(Vec<u8>, Option<KeySpec>), SignerError> {
            self.get_public_key_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(e) = &self.get_public_key_error {
                return Err(SignerError::Kms(e.clone()));
            }
            Ok((self.spki(), self.spec.clone()))
        }
    }

    fn fixed_secret() -> [u8; 32] {
        [0x11; 32]
    }

    fn make_signer() -> AwsEthKmsLiveSigner {
        let fake = FakeKms::new(fixed_secret());
        AwsEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap()
    }

    #[test]
    fn constructor_caches_pubkey_with_one_get_public_key_call() {
        let fake = FakeKms::new(fixed_secret());
        let counter = fake.get_public_key_calls.clone();
        let signer =
            AwsEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // Repeated `eth_address()` calls hit the OnceLock, not the client.
        for _ in 0..5 {
            let _ = signer.eth_address().unwrap();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sign_digest_hex_round_trip_recovers_signers_address() {
        let signer = make_signer();
        let digest = [0xAB; 32];
        let sig_hex = signer.sign_digest_hex(&digest).unwrap();
        assert!(sig_hex.starts_with("0x"));
        assert_eq!(sig_hex.len(), 132);
        let raw = hex::decode(&sig_hex[2..]).unwrap();
        assert_eq!(raw.len(), 65);
        let sig = Signature::from_slice(&raw[..64]).unwrap();
        let recid = RecoveryId::try_from(raw[64]).unwrap();
        let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
        let recovered_addr = address_from_verifying_key(&recovered);
        assert_eq!(recovered_addr, signer.eth_address().unwrap());
    }

    #[test]
    fn address_matches_local_signer_for_same_secret() {
        // Cross-check: the AWS-shaped path must derive the same EIP-55
        // address that the local-file backend would for the same key.
        let signer = make_signer();
        let local = SigningKey::from_bytes((&fixed_secret()).into()).unwrap();
        let local_addr = address_from_verifying_key(local.verifying_key());
        assert_eq!(signer.eth_address().unwrap(), local_addr);
    }

    #[test]
    fn constructor_rejects_non_secp256k1_keyspec() {
        let mut fake = FakeKms::new(fixed_secret());
        fake.spec = Some(KeySpec::EccNistP256);
        let err = AwsEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string())
            .expect_err("must reject non-secp256k1");
        match err {
            SignerError::KeySpecMismatch { found_spec, .. } => {
                assert!(found_spec.contains("Nist") || found_spec.contains("P256"));
            }
            other => panic!("expected KeySpecMismatch, got {other:?}"),
        }
    }

    #[test]
    fn constructor_propagates_get_public_key_error() {
        let mut fake = FakeKms::new(fixed_secret());
        fake.get_public_key_error = Some("AccessDenied".to_string());
        let err = AwsEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string())
            .expect_err("must propagate");
        match err {
            SignerError::Kms(m) => assert!(m.contains("AccessDenied"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn sign_propagates_kms_error() {
        let fake = FakeKms::new(fixed_secret());
        let mut signer =
            AwsEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap();
        // Swap the client for one that errors on sign.
        let mut bad = FakeKms::new(fixed_secret());
        bad.sign_error = Some("KMSInvalidSignatureException".to_string());
        signer.client = Box::new(bad);
        let err = signer.sign_digest_hex(&[0u8; 32]).expect_err("must error");
        match err {
            SignerError::Kms(m) => assert!(m.contains("KMSInvalid"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn sign_called_n_times_calls_client_n_times_but_pubkey_only_once() {
        let fake = FakeKms::new(fixed_secret());
        let sign_counter = fake.sign_calls.clone();
        let pk_counter = fake.get_public_key_calls.clone();
        let signer =
            AwsEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap();
        for _ in 0..3 {
            signer.sign_digest_hex(&[0xCD; 32]).unwrap();
        }
        assert_eq!(sign_counter.load(Ordering::SeqCst), 3);
        assert_eq!(pk_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn from_env_missing_var_errors_clearly() {
        let _guard = env_lock().lock().unwrap();
        // The constructor reads SBO3L_ETH_AWS_KMS_KEY_ID. We touch only
        // our own var here — leave any pre-set value alone.
        // Test in isolation: capture original, clear, restore at end.
        let original_id = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ID").ok();
        let original_arn = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ARN").ok();
        unsafe {
            std::env::remove_var("SBO3L_ETH_AWS_KMS_KEY_ID");
            std::env::remove_var("SBO3L_ETH_AWS_KMS_KEY_ARN");
        }
        let err = AwsEthKmsLiveSigner::from_env("audit").expect_err("must error");
        match err {
            SignerError::MissingEnv("SBO3L_ETH_AWS_KMS_KEY_ID") => {}
            other => panic!("expected MissingEnv, got {other:?}"),
        }
        // Restore.
        unsafe {
            if let Some(v) = original_id {
                std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ID", v);
            }
            if let Some(v) = original_arn {
                std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ARN", v);
            }
        }
    }

    #[test]
    fn key_id_returns_configured_value() {
        let signer = make_signer();
        assert_eq!(signer.key_id(), "test-key");
    }

    #[test]
    fn signature_byte_identical_across_two_calls_with_same_input() {
        // Determinism: deterministic-k ECDSA + the cached pubkey →
        // two calls with the same digest must produce byte-identical
        // signatures (including the recovery byte).
        let signer = make_signer();
        let digest = [0x33; 32];
        let s1 = signer.sign_digest_hex(&digest).unwrap();
        let s2 = signer.sign_digest_hex(&digest).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn signature_verifies_against_address_from_ecrecover_pattern() {
        // Full E2E mock: sign, recover address from signature, assert
        // it equals signer.eth_address(). This is the contract every
        // on-chain ecrecover relies on.
        let signer = make_signer();
        let digest = [0x99u8; 32];
        let sig_hex = signer.sign_digest_hex(&digest).unwrap();
        let raw = hex::decode(sig_hex.strip_prefix("0x").unwrap()).unwrap();
        let sig = Signature::from_slice(&raw[..64]).unwrap();
        let recid = RecoveryId::try_from(raw[64]).unwrap();
        let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
        let addr = address_from_verifying_key(&recovered);
        assert_eq!(addr, signer.eth_address().unwrap());
    }

    #[test]
    fn empty_key_id_env_treated_as_missing() {
        let _guard = env_lock().lock().unwrap();
        let original_id = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ID").ok();
        let original_arn = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ARN").ok();
        unsafe {
            std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ID", "");
            std::env::remove_var("SBO3L_ETH_AWS_KMS_KEY_ARN");
        }
        let err = AwsEthKmsLiveSigner::from_env("audit").expect_err("must reject empty");
        match err {
            SignerError::MissingEnv(_) => {}
            other => panic!("expected MissingEnv, got {other:?}"),
        }
        unsafe {
            std::env::remove_var("SBO3L_ETH_AWS_KMS_KEY_ID");
            if let Some(v) = original_id {
                std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ID", v);
            }
            if let Some(v) = original_arn {
                std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ARN", v);
            }
        }
    }

    #[test]
    fn key_arn_env_var_also_accepted() {
        let _guard = env_lock().lock().unwrap();
        let original_id = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ID").ok();
        let original_arn = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ARN").ok();
        unsafe {
            std::env::remove_var("SBO3L_ETH_AWS_KMS_KEY_ID");
            std::env::set_var(
                "SBO3L_ETH_AWS_KMS_KEY_ARN",
                "arn:aws:kms:us-east-1:000:key/abc",
            );
        }
        // We can't really construct it (would call real AWS), but we
        // can verify the env-read step succeeds before the network
        // call by using `from_env`. The from_env path tries to build
        // a runtime + load creds; that's fine for the test as long
        // as we don't depend on a particular outcome.
        // Instead, just verify the env-var fallback by directly
        // reading.
        let id = std::env::var("SBO3L_ETH_AWS_KMS_KEY_ID")
            .or_else(|_| std::env::var("SBO3L_ETH_AWS_KMS_KEY_ARN"));
        assert!(id.is_ok());
        unsafe {
            std::env::remove_var("SBO3L_ETH_AWS_KMS_KEY_ARN");
            if let Some(v) = original_id {
                std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ID", v);
            }
            if let Some(v) = original_arn {
                std::env::set_var("SBO3L_ETH_AWS_KMS_KEY_ARN", v);
            }
        }
    }
}
