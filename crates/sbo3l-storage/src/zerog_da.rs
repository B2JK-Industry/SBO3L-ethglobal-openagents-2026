//! 0G Data Availability (DA) layer publishing for `sbo3l audit publish` (T-6-2).
//!
//! Sister to [`crate::zerog_backend`] but for the **DA** product, not Storage.
//! 0G's DA service receives blob commitments and provides a globally-anchored
//! identifier (`blob_id`) plus a verifiable proof that the blob is retrievable
//! by any DA node — useful for audit-trail anchoring where we want
//! "data was published" guarantees stronger than a single indexer's word.
//!
//! ## Status: source-side complete, live deploy gated on faucet
//!
//! Per memory `zerog_bounty_intel.md` (and confirmed by Heidi UAT iteration
//! 6): the 0G Galileo DA testnet endpoint is **documented-flaky**. The faucet
//! is browser-only (Cloudflare Turnstile), the DA RPC drops connections
//! mid-disperse, and we don't have a stable bearer-token issuance path.
//!
//! This module ships:
//!
//! 1. [`ZeroGDataAvailability`] — the publish-side trait
//! 2. [`ZeroGDABackend`] — production HTTP client, 1s/3s retry schedule
//!    matching the Storage backend, 8s per-attempt timeout (worst case
//!    ~28s before fallback). Errors out with the same browser-fallback
//!    pointer the Storage backend uses.
//! 3. [`MockDABackend`] — deterministic-blob_id fallback used by the CLI
//!    when `--da mock` is set. The blob_id is `sha256(payload)` so it's
//!    reproducible offline; honest about being a mock (CLI prints a
//!    `mock-da:` prefix on every line, mirroring the audit checkpoint
//!    pattern). This is the load-bearing path for demos when the real
//!    testnet is down.
//! 4. [`LocalFileDABackend`] — disk-only sister, mirror of
//!    [`crate::zerog_backend::LocalFileBackend`]. Useful for offline
//!    fixture generation in CI.
//!
//! ## Honest scope
//!
//! - **Testnet only.** No mainnet path.
//! - **No production guarantees.** When DA is down, the operator gets a
//!   clear error pointing at the browser-tool fallback OR the mock backend.
//! - **No commitment-proof verification.** A real client would re-derive the
//!   KZG commitment from the returned `blob_id` and verify proof-of-storage.
//!   We trust the indexer's response shape; if the gateway lies the
//!   downstream audit-bundle verifier doesn't catch it. Adding KZG
//!   verification requires `arkworks` or a `c-kzg` binding — out of scope
//!   for this PR; tracked in `docs/0g-integration.md`.
//!
//! ## Example
//!
//! ```no_run
//! use sbo3l_storage::zerog_da::{ZeroGDataAvailability, ZeroGDABackend};
//!
//! let da = ZeroGDABackend::new("https://da-rpc-testnet.0g.ai");
//! let payload = br#"{"hello":"world"}"#;
//! let r = da.publish(payload).expect("publish failed (testnet flake?)");
//! println!("published: blob_id={} backend={}", r.blob_id, r.backend);
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Identifier returned by a DA publish. Counterpart to
/// [`crate::zerog_backend::RemoteRef`] for the Storage path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaRef {
    /// The DA-assigned blob identifier. For 0G this is a base16 / base64
    /// string the indexer returns; for the mock backend it's `sha256:` +
    /// 64 hex chars; for the local-file backend it's the absolute file
    /// path.
    pub blob_id: String,
    /// Static tag: `"0g-da"`, `"mock-da"`, or `"local-da"`.
    pub backend: &'static str,
    /// When the publish completed (RFC3339, UTC).
    pub published_at: DateTime<Utc>,
    /// Endpoint the publish was sent to (or `mock://` / `file://...`).
    pub endpoint: String,
}

/// Errors a DA backend can fail with. `BrowserFallback` mirrors the
/// Storage backend's terminal-state pattern — operators get a concrete
/// recovery path instead of a stack trace.
#[derive(Debug, Error)]
pub enum DaBackendError {
    #[error(
        "0G DA publish failed after {attempts} attempt(s): {last_error}. \
         DA testnet is documented-flaky; fall back to `--da mock` for a \
         deterministic offline blob_id, or use the browser tool at \
         https://storagescan-galileo.0g.ai/tool"
    )]
    BrowserFallback { attempts: u32, last_error: String },
    #[error("local-da backend write failed: {0}")]
    LocalIo(#[from] std::io::Error),
    #[error("malformed DA gateway response (no blob_id): {0}")]
    MalformedResponse(String),
}

/// Publish-side interface. Synchronous on purpose — see module docs.
pub trait ZeroGDataAvailability {
    /// Publish `payload` to the DA layer. Returns a [`DaRef`] carrying
    /// the assigned blob_id + the endpoint that issued it (so the CLI
    /// can record byte-for-byte where the publish landed).
    fn publish(&self, payload: &[u8]) -> Result<DaRef, DaBackendError>;
}

/// Default 0G Galileo DA testnet endpoint. The brief documents this
/// URL; the CLI also lets it be overridden via `SBO3L_ZEROG_DA_URL`.
pub const DEFAULT_ZEROG_DA_URL: &str = "https://da-rpc-testnet.0g.ai";

/// Per-attempt retry delays. Mirror of the Storage schedule.
pub const DEFAULT_RETRY_DELAYS_MS: [u64; 2] = [1_000, 3_000];

/// Production 0G DA backend. Carries an explicit `reqwest::blocking::Client`
/// for the same testability story as the Storage backend.
pub struct ZeroGDABackend {
    endpoint: String,
    http: reqwest::blocking::Client,
    retry_delays_ms: Vec<u64>,
}

impl ZeroGDABackend {
    /// Build a backend pointed at the supplied DA URL with sensible
    /// defaults: 8s per-request timeout + 1s/3s retry schedule.
    pub fn new(endpoint: impl Into<String>) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest blocking client builds with default config");
        Self {
            endpoint: endpoint.into(),
            http,
            retry_delays_ms: DEFAULT_RETRY_DELAYS_MS.to_vec(),
        }
    }

    /// Test-friendly constructor: skip all sleeps so retry-behaviour tests
    /// don't burn wall-clock time per assertion.
    pub fn with_no_retry_delays(endpoint: impl Into<String>) -> Self {
        Self {
            retry_delays_ms: vec![0, 0],
            ..Self::new(endpoint)
        }
    }

    /// Override retry delays. Length defines the retry count.
    pub fn with_retry_delays_ms(mut self, delays: Vec<u64>) -> Self {
        self.retry_delays_ms = delays;
        self
    }

    /// Override the HTTP client. Tests use this to set a short timeout so
    /// `httpmock`'s `.delay(...)` simulation actually trips a timeout.
    pub fn with_http_client(mut self, http: reqwest::blocking::Client) -> Self {
        self.http = http;
        self
    }

    pub fn max_attempts(&self) -> u32 {
        self.retry_delays_ms.len() as u32 + 1
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

/// 0G DA gateway success response. The actual API returns more fields
/// (commitment, proof, etc.) — we only care about the assigned blob_id.
/// `serde(rename)` covers the two casing styles 0G's gateway has used.
#[derive(Debug, Deserialize)]
struct DaGatewayOk {
    #[serde(rename = "blobId", alias = "blob_id", alias = "blobID")]
    blob_id: Option<String>,
}

impl ZeroGDataAvailability for ZeroGDABackend {
    fn publish(&self, payload: &[u8]) -> Result<DaRef, DaBackendError> {
        let url = format!("{}/disperse_blob", self.endpoint.trim_end_matches('/'));
        let max = self.max_attempts();
        let mut last_error = String::new();
        for attempt in 0..max {
            if attempt > 0 {
                let delay = self.retry_delays_ms[(attempt - 1) as usize];
                if delay > 0 {
                    std::thread::sleep(Duration::from_millis(delay));
                }
            }
            match self
                .http
                .post(&url)
                .header("content-type", "application/octet-stream")
                .body(payload.to_vec())
                .send()
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let text = resp
                            .text()
                            .unwrap_or_else(|e| format!("(body read error: {e})"));
                        match serde_json::from_str::<DaGatewayOk>(&text) {
                            Ok(ok) => {
                                if let Some(blob_id) = ok.blob_id {
                                    if blob_id.is_empty() {
                                        return Err(DaBackendError::MalformedResponse(format!(
                                            "DA gateway returned empty blob_id: {text}"
                                        )));
                                    }
                                    return Ok(DaRef {
                                        blob_id,
                                        backend: "0g-da",
                                        published_at: Utc::now(),
                                        endpoint: self.endpoint.clone(),
                                    });
                                }
                                return Err(DaBackendError::MalformedResponse(format!(
                                    "DA gateway 200 has no blob_id field: {text}"
                                )));
                            }
                            Err(e) => {
                                return Err(DaBackendError::MalformedResponse(format!(
                                    "DA gateway 200 not parseable JSON ({e}): {text}"
                                )));
                            }
                        }
                    }
                    last_error = format!(
                        "DA gateway returned HTTP {status}: {}",
                        resp.text().unwrap_or_default()
                    );
                }
                Err(e) => {
                    last_error = format!("transport: {e}");
                }
            }
        }
        Err(DaBackendError::BrowserFallback {
            attempts: max,
            last_error,
        })
    }
}

/// Deterministic-blob_id mock backend. Used by the CLI's `--da mock`
/// path when the real testnet is down + we still need a reproducible
/// demo / CI fixture. Honest about being a mock: the `backend` tag is
/// `"mock-da"` and the CLI gates its outputs behind a `mock-da:` prefix
/// (mirroring the audit-checkpoint pattern).
///
/// `blob_id = "sha256:" + hex(sha256(payload))` — same payload always
/// produces the same blob_id, no network, no time dependency.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockDABackend;

impl MockDABackend {
    pub fn new() -> Self {
        Self
    }
}

impl ZeroGDataAvailability for MockDABackend {
    fn publish(&self, payload: &[u8]) -> Result<DaRef, DaBackendError> {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let digest = hasher.finalize();
        let hex_digest = hex::encode(digest);
        Ok(DaRef {
            blob_id: format!("sha256:{hex_digest}"),
            backend: "mock-da",
            published_at: Utc::now(),
            endpoint: "mock://".to_string(),
        })
    }
}

/// Disk-only sister backend. Writes the payload to `<dir>/<basename>`
/// and returns `file://<absolute-path>` as the blob_id. Useful for
/// offline fixture generation in CI; not a real DA path.
pub struct LocalFileDABackend {
    pub dir: PathBuf,
    basename: String,
}

impl LocalFileDABackend {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            basename: "audit-blob.bin".to_string(),
        }
    }

    pub fn with_basename(mut self, name: impl Into<String>) -> Self {
        self.basename = name.into();
        self
    }
}

impl ZeroGDataAvailability for LocalFileDABackend {
    fn publish(&self, payload: &[u8]) -> Result<DaRef, DaBackendError> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(&self.basename);
        let mut f = std::fs::File::create(&path)?;
        f.write_all(payload)?;
        f.sync_all()?;
        let abs = path.canonicalize().unwrap_or(path).display().to_string();
        Ok(DaRef {
            blob_id: format!("file://{abs}"),
            backend: "local-da",
            published_at: Utc::now(),
            endpoint: format!("file://{}", self.dir.display()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method, MockServer};

    #[test]
    fn mock_backend_blob_id_is_sha256_of_payload() {
        let m = MockDABackend::new();
        let payload = b"hello world";
        let r = m.publish(payload).expect("mock backend never fails");
        assert_eq!(r.backend, "mock-da");
        assert_eq!(r.endpoint, "mock://");
        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            r.blob_id,
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn mock_backend_is_deterministic() {
        let m = MockDABackend::new();
        let r1 = m.publish(b"abc").unwrap();
        let r2 = m.publish(b"abc").unwrap();
        assert_eq!(r1.blob_id, r2.blob_id);
        let r3 = m.publish(b"def").unwrap();
        assert_ne!(r1.blob_id, r3.blob_id);
    }

    #[test]
    fn local_file_backend_writes_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let backend = LocalFileDABackend::new(tmp.path().to_path_buf()).with_basename("test.bin");
        let payload = b"local-da bytes";
        let r = backend.publish(payload).expect("write succeeds");
        assert_eq!(r.backend, "local-da");
        assert!(r.blob_id.starts_with("file://"));
        let written = std::fs::read(tmp.path().join("test.bin")).unwrap();
        assert_eq!(written, payload);
    }

    #[test]
    fn ok_response_returns_blob_id() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(Method::POST).path("/disperse_blob");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"blobId":"0xc0ffee","commitment":"0xdeadbeef"}"#);
        });
        let backend = ZeroGDABackend::with_no_retry_delays(server.base_url());
        let r = backend.publish(b"payload").unwrap();
        assert_eq!(r.backend, "0g-da");
        assert_eq!(r.blob_id, "0xc0ffee");
        mock.assert();
    }

    #[test]
    fn snake_case_blob_id_alias_is_accepted() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(Method::POST).path("/disperse_blob");
            then.status(200).body(r#"{"blob_id":"0xabc123"}"#);
        });
        let backend = ZeroGDABackend::with_no_retry_delays(server.base_url());
        let r = backend.publish(b"x").unwrap();
        assert_eq!(r.blob_id, "0xabc123");
    }

    #[test]
    fn missing_blob_id_in_200_errors_clearly() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(Method::POST).path("/disperse_blob");
            then.status(200).body(r#"{"commitment":"0xdead"}"#);
        });
        let backend = ZeroGDABackend::with_no_retry_delays(server.base_url());
        let err = backend.publish(b"x").unwrap_err();
        assert!(matches!(err, DaBackendError::MalformedResponse(_)));
    }

    #[test]
    fn empty_blob_id_in_200_errors_clearly() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(Method::POST).path("/disperse_blob");
            then.status(200).body(r#"{"blobId":""}"#);
        });
        let backend = ZeroGDABackend::with_no_retry_delays(server.base_url());
        let err = backend.publish(b"x").unwrap_err();
        assert!(matches!(err, DaBackendError::MalformedResponse(m) if m.contains("empty")));
    }

    #[test]
    fn malformed_200_json_errors_clearly() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(Method::POST).path("/disperse_blob");
            then.status(200).body(r#"<html>not json</html>"#);
        });
        let backend = ZeroGDABackend::with_no_retry_delays(server.base_url());
        let err = backend.publish(b"x").unwrap_err();
        assert!(matches!(err, DaBackendError::MalformedResponse(m) if m.contains("not parseable")));
    }

    #[test]
    fn server_5xx_retries_then_browser_fallback() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(Method::POST).path("/disperse_blob");
            then.status(503).body("upstream gateway down");
        });
        let backend = ZeroGDABackend::with_no_retry_delays(server.base_url());
        let err = backend.publish(b"x").unwrap_err();
        match err {
            DaBackendError::BrowserFallback {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 3);
                assert!(last_error.contains("503"));
            }
            other => panic!("expected BrowserFallback, got {other:?}"),
        }
        mock.assert_hits(3);
    }

    #[test]
    fn max_attempts_matches_documented_policy() {
        // The brief documents a 3-attempt policy (1s + 3s retries).
        let backend = ZeroGDABackend::new("https://x");
        assert_eq!(backend.max_attempts(), 3);
        // with_no_retry_delays must preserve the same arity.
        let backend = ZeroGDABackend::with_no_retry_delays("https://x");
        assert_eq!(backend.max_attempts(), 3);
    }
}
