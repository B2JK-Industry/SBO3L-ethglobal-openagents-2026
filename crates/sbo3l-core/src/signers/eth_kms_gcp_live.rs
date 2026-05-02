//! `eth_kms_gcp_live` — live GCP KMS [`EthSigner`] backend (R14 P3).
//!
//! Compiled only with `--features eth_kms_gcp`. Mirrors the AWS path
//! shape but goes through `google-cloud-kms` 0.6 + `google-cloud-auth`
//! 0.17 (the yoshidan family — same module set as the rest of the
//! Rust GCP ecosystem).
//!
//! # Honest status
//!
//! No real KMS round-trip has been verified — Daniel does NOT have GCP
//! creds in this round. Unit tests exercise the response-decoding logic
//! against synthetic inputs; the gated integration test in
//! `tests/gcp_kms_live.rs` skips cleanly unless `GCP_KMS_TEST_ENABLED=1`
//! is set with `GOOGLE_APPLICATION_CREDENTIALS` pointing at a real
//! service-account JSON. Daniel runs the gated integration test in R15.
//!
//! # API differences vs AWS KMS (the gotchas)
//!
//! - GCP returns the public key as **PEM** (RFC 7468). We strip the
//!   `-----BEGIN PUBLIC KEY-----` envelope, base64-decode, then run
//!   the same SubjectPublicKeyInfo parser the AWS path uses.
//! - GCP `AsymmetricSign` takes a `Digest { sha256: ... }` field
//!   rather than a separate `MessageType::Digest` flag. The signature
//!   bytes are still ASN.1 DER, so the DER-to-rsv path is shared.
//! - GCP key spec is `EC_SIGN_SECP256K1_SHA256` (algorithm enum 31)
//!   — we cross-check against the `algorithm` field on the
//!   `PublicKey` response so a mis-provisioned key fails fast.

use std::sync::OnceLock;

use async_trait::async_trait;
use google_cloud_googleapis::cloud::kms::v1::{
    digest::Digest as DigestKind, AsymmetricSignRequest, Digest, GetPublicKeyRequest,
};
use google_cloud_kms::client::{Client as GcpKmsClient, ClientConfig};
use k256::ecdsa::VerifyingKey;

use super::eth_kms_common::{address_from_verifying_key, der_to_rsv, parse_spki_secp256k1};
use super::{eth::EthSigner, SignerError};

/// `CryptoKeyVersionAlgorithm::EcSignSecp256k1Sha256` (proto enum value
/// 31). Hardcoded to avoid a re-export through every dependent crate.
const ALG_EC_SIGN_SECP256K1_SHA256: i32 = 31;

/// Minimal surface this signer needs from GCP KMS. Mirrors the AWS
/// trait shape — wraps the two RPCs we use so unit tests can fake.
#[async_trait]
pub trait GcpClient: Send + Sync {
    /// `AsymmetricSign` with a SHA-256 digest. Returns DER signature
    /// bytes.
    async fn asymmetric_sign(
        &self,
        key_name: &str,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, SignerError>;

    /// `GetPublicKey`. Returns the PEM string + algorithm enum value.
    async fn get_public_key(&self, key_name: &str) -> Result<(String, i32), SignerError>;
}

/// SDK-backed adapter — wraps a `google_cloud_kms::client::Client`.
pub struct SdkGcpClient {
    inner: GcpKmsClient,
}

impl SdkGcpClient {
    pub fn new(inner: GcpKmsClient) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl GcpClient for SdkGcpClient {
    async fn asymmetric_sign(
        &self,
        key_name: &str,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, SignerError> {
        let req = AsymmetricSignRequest {
            name: key_name.to_string(),
            digest: Some(Digest {
                digest: Some(DigestKind::Sha256(digest.to_vec())),
            }),
            digest_crc32c: None,
            data: vec![],
            data_crc32c: None,
        };
        let resp = self
            .inner
            .asymmetric_sign(req, None)
            .await
            .map_err(|e| SignerError::Kms(format!("gcp kms sign({key_name}): {e}")))?;
        if resp.signature.is_empty() {
            return Err(SignerError::Kms(format!(
                "gcp kms sign({key_name}): empty signature in response"
            )));
        }
        Ok(resp.signature)
    }

    async fn get_public_key(&self, key_name: &str) -> Result<(String, i32), SignerError> {
        let req = GetPublicKeyRequest {
            name: key_name.to_string(),
        };
        let resp =
            self.inner.get_public_key(req, None).await.map_err(|e| {
                SignerError::Kms(format!("gcp kms get_public_key({key_name}): {e}"))
            })?;
        Ok((resp.pem, resp.algorithm))
    }
}

/// Live GCP KMS secp256k1 EVM signer.
pub struct GcpEthKmsLiveSigner {
    client: Box<dyn GcpClient>,
    key_name: String,
    cached_verifying_key: OnceLock<VerifyingKey>,
    cached_address: OnceLock<String>,
}

impl std::fmt::Debug for GcpEthKmsLiveSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcpEthKmsLiveSigner")
            .field("key_name", &self.key_name)
            .field(
                "cached_pubkey_present",
                &self.cached_verifying_key.get().is_some(),
            )
            .finish()
    }
}

impl GcpEthKmsLiveSigner {
    /// Construct from env. Reads `SBO3L_ETH_GCP_KMS_KEY_NAME` (the full
    /// `projects/.../cryptoKeyVersions/N` resource name). Auth is via
    /// the standard GCP credentials chain (`GOOGLE_APPLICATION_CREDENTIALS`,
    /// metadata server, gcloud login).
    pub fn from_env(_role: &str) -> Result<Self, SignerError> {
        let key_name = std::env::var("SBO3L_ETH_GCP_KMS_KEY_NAME")
            .map_err(|_| SignerError::MissingEnv("SBO3L_ETH_GCP_KMS_KEY_NAME"))?;
        if key_name.is_empty() {
            return Err(SignerError::MissingEnv("SBO3L_ETH_GCP_KMS_KEY_NAME"));
        }
        // Codex P1 fix (#324): same nested-runtime hazard as
        // `block_on` below. Daemon code calls `from_env` at startup,
        // which runs inside `#[tokio::main]`'s runtime; building a
        // fresh runtime here would panic with the nested-runtime
        // error.
        let build_client = async {
            let cfg = ClientConfig::default()
                .with_auth()
                .await
                .map_err(|e| SignerError::Kms(format!("gcp kms: auth: {e}")))?;
            GcpKmsClient::new(cfg)
                .await
                .map_err(|e| SignerError::Kms(format!("gcp kms: client: {e}")))
        };
        let client = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(build_client))?,
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| SignerError::Kms(format!("gcp kms: build tokio rt: {e}")))?;
                rt.block_on(build_client)?
            }
        };
        Self::with_client(Box::new(SdkGcpClient::new(client)), key_name)
    }

    /// Construct with an explicit client. Used by unit tests + callers
    /// that want to share a configured client.
    pub fn with_client(client: Box<dyn GcpClient>, key_name: String) -> Result<Self, SignerError> {
        let s = Self {
            client,
            key_name,
            cached_verifying_key: OnceLock::new(),
            cached_address: OnceLock::new(),
        };
        s.address()?;
        Ok(s)
    }

    /// Synchronous block-on helper for the `EthSigner` impl.
    ///
    /// **Codex P1 fix (#324):** the previous impl built a fresh
    /// runtime per call. That panics when invoked from a Tokio
    /// worker thread (the daemon's normal context). Mirrors the
    /// AWS-side fix in `eth_kms_aws_live.rs::block_on`: detect via
    /// `Handle::try_current()` and use `block_in_place` inside a
    /// runtime, fall back to building one outside.
    fn block_on<T>(
        &self,
        fut: impl std::future::Future<Output = Result<T, SignerError>>,
    ) -> Result<T, SignerError> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| SignerError::Kms(format!("gcp kms: build tokio rt: {e}")))?;
                rt.block_on(fut)
            }
        }
    }

    fn verifying_key(&self) -> Result<&VerifyingKey, SignerError> {
        if let Some(vk) = self.cached_verifying_key.get() {
            return Ok(vk);
        }
        let key_name = self.key_name.clone();
        let (pem, algorithm) =
            self.block_on(async { self.client.get_public_key(&key_name).await })?;
        if algorithm != 0 && algorithm != ALG_EC_SIGN_SECP256K1_SHA256 {
            return Err(SignerError::KeySpecMismatch {
                key_id: self.key_name.clone(),
                found_spec: format!("CryptoKeyVersionAlgorithm({algorithm})"),
            });
        }
        let der = pem_to_der(&pem)?;
        let vk = parse_spki_secp256k1(&der)?;
        let _ = self.cached_verifying_key.set(vk);
        Ok(self.cached_verifying_key.get().expect("just set"))
    }

    fn address(&self) -> Result<&str, SignerError> {
        if let Some(addr) = self.cached_address.get() {
            return Ok(addr);
        }
        let vk = self.verifying_key()?;
        let addr = address_from_verifying_key(vk);
        let _ = self.cached_address.set(addr);
        Ok(self.cached_address.get().expect("just set"))
    }
}

impl EthSigner for GcpEthKmsLiveSigner {
    fn sign_digest_hex(&self, digest: &[u8; 32]) -> Result<String, SignerError> {
        let key_name = self.key_name.clone();
        let der = self.block_on(async { self.client.asymmetric_sign(&key_name, digest).await })?;
        let vk = self.verifying_key()?;
        let sig_bytes = der_to_rsv(&der, digest, vk)?;
        Ok(format!("0x{}", hex::encode(sig_bytes)))
    }

    fn eth_address(&self) -> Result<String, SignerError> {
        Ok(self.address()?.to_string())
    }

    fn key_id(&self) -> &str {
        &self.key_name
    }
}

/// Strip a `-----BEGIN PUBLIC KEY-----` PEM envelope to its raw DER
/// bytes. Tolerant of CRLF / LF line endings + variable header spacing
/// (real GCP responses use LF; we accept both for robustness when the
/// PEM travels through a Windows-shaped relay).
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>, SignerError> {
    use base64::Engine as _;
    const BEGIN: &str = "-----BEGIN PUBLIC KEY-----";
    const END: &str = "-----END PUBLIC KEY-----";
    let begin = pem
        .find(BEGIN)
        .ok_or_else(|| SignerError::Kms("gcp pem: missing BEGIN PUBLIC KEY header".to_string()))?;
    let after_begin = begin + BEGIN.len();
    let end = pem
        .find(END)
        .ok_or_else(|| SignerError::Kms("gcp pem: missing END PUBLIC KEY footer".to_string()))?;
    if end <= after_begin {
        return Err(SignerError::Kms(
            "gcp pem: footer before header".to_string(),
        ));
    }
    let body: String = pem[after_begin..end]
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.as_bytes())
        .map_err(|e| SignerError::Kms(format!("gcp pem: base64 decode: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use k256::ecdsa::signature::hazmat::PrehashSigner;
    use k256::ecdsa::{RecoveryId, Signature, SigningKey};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// See `eth_kms_aws_live::tests::env_lock` for the rationale —
    /// env-var tests serialised under one mutex per module to avoid
    /// the parallel-test race that flips `set_var`/`remove_var`
    /// between sibling tests.
    fn env_lock() -> &'static Mutex<()> {
        static M: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| Mutex::new(()))
    }

    /// Hand-rolled fake. Returns SHA-256-style ECDSA over the digest
    /// (matching what real KMS would do for an
    /// EC_SIGN_SECP256K1_SHA256 key).
    struct FakeGcp {
        signing: SigningKey,
        algorithm: i32,
        pem_override: Option<String>,
        sign_calls: Arc<AtomicUsize>,
        get_public_key_calls: Arc<AtomicUsize>,
        sign_error: Option<String>,
        get_public_key_error: Option<String>,
    }

    impl FakeGcp {
        fn new(secret: [u8; 32]) -> Self {
            Self {
                signing: SigningKey::from_bytes((&secret).into()).unwrap(),
                algorithm: ALG_EC_SIGN_SECP256K1_SHA256,
                pem_override: None,
                sign_calls: Arc::new(AtomicUsize::new(0)),
                get_public_key_calls: Arc::new(AtomicUsize::new(0)),
                sign_error: None,
                get_public_key_error: None,
            }
        }

        fn pem(&self) -> String {
            if let Some(p) = &self.pem_override {
                return p.clone();
            }
            // Build the same SPKI shape the AWS fake uses, then base64
            // wrap it as PEM.
            let pk = self.signing.verifying_key().to_encoded_point(false);
            let pk_bytes = pk.as_bytes();
            let mut spki = Vec::with_capacity(88);
            spki.extend_from_slice(&[
                0x30, 0x56, 0x30, 0x10, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06,
                0x05, 0x2b, 0x81, 0x04, 0x00, 0x0a, 0x03, 0x42, 0x00,
            ]);
            spki.extend_from_slice(pk_bytes);
            let b64 = base64::engine::general_purpose::STANDARD.encode(&spki);
            // Standard PEM line wrapping at 64 chars.
            let mut wrapped = String::new();
            for chunk in b64.as_bytes().chunks(64) {
                wrapped.push_str(std::str::from_utf8(chunk).unwrap());
                wrapped.push('\n');
            }
            format!("-----BEGIN PUBLIC KEY-----\n{wrapped}-----END PUBLIC KEY-----\n")
        }
    }

    #[async_trait]
    impl GcpClient for FakeGcp {
        async fn asymmetric_sign(
            &self,
            _key_name: &str,
            digest: &[u8; 32],
        ) -> Result<Vec<u8>, SignerError> {
            self.sign_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(e) = &self.sign_error {
                return Err(SignerError::Kms(e.clone()));
            }
            let (sig, _): (Signature, RecoveryId) = self
                .signing
                .sign_prehash(digest)
                .map_err(|e| SignerError::Kms(format!("fake sign: {e}")))?;
            Ok(sig.to_der().as_bytes().to_vec())
        }

        async fn get_public_key(&self, _key_name: &str) -> Result<(String, i32), SignerError> {
            self.get_public_key_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(e) = &self.get_public_key_error {
                return Err(SignerError::Kms(e.clone()));
            }
            Ok((self.pem(), self.algorithm))
        }
    }

    fn fixed_secret() -> [u8; 32] {
        [0x22; 32]
    }

    fn make_signer() -> GcpEthKmsLiveSigner {
        let fake = FakeGcp::new(fixed_secret());
        GcpEthKmsLiveSigner::with_client(
            Box::new(fake),
            "projects/test/locations/us/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1".to_string(),
        )
        .unwrap()
    }

    #[test]
    fn constructor_caches_pubkey_with_one_get_public_key_call() {
        let fake = FakeGcp::new(fixed_secret());
        let counter = fake.get_public_key_calls.clone();
        let signer =
            GcpEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        for _ in 0..5 {
            let _ = signer.eth_address().unwrap();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sign_digest_hex_round_trip_recovers_signers_address() {
        let signer = make_signer();
        let digest = [0xCD; 32];
        let sig_hex = signer.sign_digest_hex(&digest).unwrap();
        let raw = hex::decode(sig_hex.strip_prefix("0x").unwrap()).unwrap();
        let sig = Signature::from_slice(&raw[..64]).unwrap();
        let recid = RecoveryId::try_from(raw[64]).unwrap();
        let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
        let addr = address_from_verifying_key(&recovered);
        assert_eq!(addr, signer.eth_address().unwrap());
    }

    #[test]
    fn pem_to_der_round_trip() {
        let fake = FakeGcp::new(fixed_secret());
        let pem = fake.pem();
        let der = pem_to_der(&pem).unwrap();
        let vk = parse_spki_secp256k1(&der).unwrap();
        let local = SigningKey::from_bytes((&fixed_secret()).into()).unwrap();
        assert_eq!(&vk, local.verifying_key());
    }

    #[test]
    fn pem_to_der_handles_crlf_line_endings() {
        let fake = FakeGcp::new(fixed_secret());
        let pem = fake.pem().replace('\n', "\r\n");
        let der = pem_to_der(&pem).unwrap();
        // SPKI for secp256k1 is exactly 88 bytes.
        assert_eq!(der.len(), 88);
    }

    #[test]
    fn pem_to_der_rejects_missing_begin() {
        let bad = "no header here\nMFY=\n-----END PUBLIC KEY-----\n";
        let err = pem_to_der(bad).expect_err("must reject");
        match err {
            SignerError::Kms(m) => assert!(m.contains("BEGIN"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn pem_to_der_rejects_missing_end() {
        let bad = "-----BEGIN PUBLIC KEY-----\nMFY=\n";
        let err = pem_to_der(bad).expect_err("must reject");
        match err {
            SignerError::Kms(m) => assert!(m.contains("END"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn pem_to_der_rejects_invalid_base64() {
        let bad = "-----BEGIN PUBLIC KEY-----\nthis is not base64!@#$\n-----END PUBLIC KEY-----\n";
        let err = pem_to_der(bad).expect_err("must reject");
        match err {
            SignerError::Kms(m) => assert!(m.contains("base64"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn constructor_rejects_wrong_algorithm() {
        let mut fake = FakeGcp::new(fixed_secret());
        // RSA_SIGN_PSS_2048_SHA256 = enum value 5 (definitely not
        // secp256k1).
        fake.algorithm = 5;
        let err = GcpEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string())
            .expect_err("must reject");
        match err {
            SignerError::KeySpecMismatch { found_spec, .. } => {
                assert!(found_spec.contains("CryptoKeyVersionAlgorithm(5)"));
            }
            other => panic!("expected KeySpecMismatch, got {other:?}"),
        }
    }

    #[test]
    fn constructor_accepts_unspecified_algorithm() {
        // GCP returns algorithm = 0 (UNSPECIFIED) on some legacy
        // responses; we accept it rather than fail-closed since the
        // pubkey parse step still validates the actual point.
        let mut fake = FakeGcp::new(fixed_secret());
        fake.algorithm = 0;
        let signer = GcpEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string());
        assert!(signer.is_ok());
    }

    #[test]
    fn constructor_propagates_get_public_key_error() {
        let mut fake = FakeGcp::new(fixed_secret());
        fake.get_public_key_error = Some("PermissionDenied".to_string());
        let err = GcpEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string())
            .expect_err("must propagate");
        match err {
            SignerError::Kms(m) => assert!(m.contains("PermissionDenied"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn sign_propagates_kms_error() {
        let fake = FakeGcp::new(fixed_secret());
        let mut signer =
            GcpEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap();
        let mut bad = FakeGcp::new(fixed_secret());
        bad.sign_error = Some("ResourceExhausted".to_string());
        signer.client = Box::new(bad);
        let err = signer.sign_digest_hex(&[0u8; 32]).expect_err("must error");
        match err {
            SignerError::Kms(m) => assert!(m.contains("ResourceExhausted"), "got: {m}"),
            other => panic!("expected Kms, got {other:?}"),
        }
    }

    #[test]
    fn sign_n_times_calls_pubkey_only_once() {
        let fake = FakeGcp::new(fixed_secret());
        let sign_counter = fake.sign_calls.clone();
        let pk_counter = fake.get_public_key_calls.clone();
        let signer =
            GcpEthKmsLiveSigner::with_client(Box::new(fake), "test-key".to_string()).unwrap();
        for _ in 0..3 {
            signer.sign_digest_hex(&[0xEE; 32]).unwrap();
        }
        assert_eq!(sign_counter.load(Ordering::SeqCst), 3);
        assert_eq!(pk_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn from_env_missing_var_errors_clearly() {
        let _guard = env_lock().lock().unwrap();
        let original = std::env::var("SBO3L_ETH_GCP_KMS_KEY_NAME").ok();
        unsafe {
            std::env::remove_var("SBO3L_ETH_GCP_KMS_KEY_NAME");
        }
        let err = GcpEthKmsLiveSigner::from_env("audit").expect_err("must error");
        match err {
            SignerError::MissingEnv("SBO3L_ETH_GCP_KMS_KEY_NAME") => {}
            other => panic!("expected MissingEnv, got {other:?}"),
        }
        unsafe {
            if let Some(v) = original {
                std::env::set_var("SBO3L_ETH_GCP_KMS_KEY_NAME", v);
            }
        }
    }

    #[test]
    fn key_id_returns_configured_value() {
        let signer = make_signer();
        assert!(signer.key_id().contains("cryptoKeyVersions/1"));
    }

    #[test]
    fn signature_byte_identical_across_two_calls() {
        let signer = make_signer();
        let digest = [0x77; 32];
        let s1 = signer.sign_digest_hex(&digest).unwrap();
        let s2 = signer.sign_digest_hex(&digest).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn signature_address_matches_local_signer() {
        let signer = make_signer();
        let local = SigningKey::from_bytes((&fixed_secret()).into()).unwrap();
        let local_addr = address_from_verifying_key(local.verifying_key());
        assert_eq!(signer.eth_address().unwrap(), local_addr);
    }

    #[test]
    fn sign_returns_65_bytes() {
        let signer = make_signer();
        let sig_hex = signer.sign_digest_hex(&[0u8; 32]).unwrap();
        let raw = hex::decode(sig_hex.strip_prefix("0x").unwrap()).unwrap();
        assert_eq!(raw.len(), 65);
        assert!(raw[64] <= 1, "v must be 0 or 1, got {}", raw[64]);
    }

    #[test]
    fn empty_key_name_env_treated_as_missing() {
        let _guard = env_lock().lock().unwrap();
        let original = std::env::var("SBO3L_ETH_GCP_KMS_KEY_NAME").ok();
        unsafe {
            std::env::set_var("SBO3L_ETH_GCP_KMS_KEY_NAME", "");
        }
        let err = GcpEthKmsLiveSigner::from_env("audit").expect_err("must reject empty");
        match err {
            SignerError::MissingEnv(_) => {}
            other => panic!("expected MissingEnv, got {other:?}"),
        }
        unsafe {
            std::env::remove_var("SBO3L_ETH_GCP_KMS_KEY_NAME");
            if let Some(v) = original {
                std::env::set_var("SBO3L_ETH_GCP_KMS_KEY_NAME", v);
            }
        }
    }
}
