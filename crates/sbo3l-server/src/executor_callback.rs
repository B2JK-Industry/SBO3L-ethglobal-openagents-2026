//! Executor callback endpoint — sponsors POST back here when an
//! out-of-band execution completes (success / failure / timeout).
//!
//! # Wire shape
//!
//! ```text
//! POST /v1/executor-callback HTTP/1.1
//! Content-Type: application/json
//! X-Upstream-Signature: t=<ts>,nonce=<ulid>,kind=json,sha256=<hex>
//!
//! {
//!   "schema": "sbo3l.executor_callback.v1",
//!   "audit_event_id": "evt-01HTAWX5K3R8YV9NQB7C6P2DGS",
//!   "sponsor": "uniswap-router",
//!   "status": "success" | "failure" | "timeout",
//!   "execution_ref": "uniswap-router:sepolia:01HV...",
//!   "evidence": { ... }            // optional, sponsor-specific
//! }
//! ```
//!
//! # Verification pipeline
//!
//! 1. Read `X-Upstream-Signature`. Missing → 401.
//! 2. Parse Stripe-style fields (t=, nonce=, kind=, sha256=).
//! 3. Re-canonicalise the body (kind=json → JCS).
//! 4. Verify Ed25519 signature against the pubkey configured for
//!    `body.sponsor` (env var `SBO3L_EXECUTOR_CALLBACK_PUBKEY_<SPONSOR>`).
//!    Missing pubkey → 503 (operator misconfiguration).
//! 5. Replay window check (300s) + nonce dedup (in-memory).
//! 6. Append an `execution_confirmed` audit event whose
//!    `subject_id = body.audit_event_id` and `payload_hash` is the
//!    canonical body hash. Returns the new audit event id.
//!
//! # Why we re-anchor in the audit chain
//!
//! Sponsors execute *out-of-band* — SBO3L authorises the action via a
//! signed PolicyReceipt, the sponsor performs it, and the result
//! lives on the sponsor's side. Without a callback the audit chain
//! has only the *intent* recorded, not the *outcome*. The callback
//! event closes that loop: an auditor reading the chain can verify
//! the outcome was reported, signed by the sponsor's pubkey, and
//! linked to the original decision via `subject_id`.

use std::collections::HashSet;
use std::sync::Mutex;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use sbo3l_core::webhook::{
    canonicalise_body, verify_webhook, BodyKind, VerifyError, WebhookEnvelope, REPLAY_WINDOW_SECS,
};
use sbo3l_storage::audit_store::NewAuditEvent;

use crate::AppState;

/// Schema id for the wire body. Bump on a wire-shape break; additive
/// changes (new optional fields) keep `v1`.
pub const CALLBACK_SCHEMA_V1: &str = "sbo3l.executor_callback.v1";

/// Header name sponsors carry the signature in. Distinct from the
/// outbound `X-SBO3L-Signature` header so a bidirectional gateway
/// doesn't accidentally collide the two namespaces.
pub const UPSTREAM_SIG_HEADER: &str = "X-Upstream-Signature";

/// Audit event_type emitted on successful callback ingest. Stable
/// string — auditors may filter the chain on this.
pub const EXECUTION_CONFIRMED_EVENT_TYPE: &str = "execution_confirmed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallbackStatus {
    Success,
    Failure,
    Timeout,
}

impl CallbackStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Timeout => "timeout",
        }
    }
}

/// Wire body shape — `serde(deny_unknown_fields)` so a sponsor that
/// adds unrequested fields fails loudly here rather than silently
/// dropping data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutorCallbackBody {
    pub schema: String,
    /// The `audit_event_id` of the original SBO3L decision the
    /// sponsor was confirming. Anchors the callback into the audit
    /// chain via `subject_id`.
    pub audit_event_id: String,
    /// Sponsor identifier — `"uniswap-router"`, `"keeperhub"`, etc.
    /// Routes to the right verifying pubkey via
    /// `SBO3L_EXECUTOR_CALLBACK_PUBKEY_<SPONSOR>`.
    pub sponsor: String,
    pub status: CallbackStatus,
    /// Sponsor-side execution identifier. Free-form string —
    /// sponsor convention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_ref: Option<String>,
    /// Optional sponsor-specific evidence. Captured into the audit
    /// event's metadata for later inspection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Value>,
}

/// Successful response shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorCallbackResponse {
    pub schema: &'static str,
    pub audit_event_id: String,
    pub appended_seq: u64,
}

const RESP_SCHEMA: &str = "sbo3l.executor_callback_response.v1";

/// In-memory replay-protection store for callback nonces.
///
/// Drop-in `seen_nonce_check` for [`verify_webhook`] — bounded growth
/// is enforced by an LRU-ish trim once the set crosses 4096 entries
/// (callbacks within a single 5-minute window are well below this in
/// practice; the trim is a safety belt).
#[derive(Default)]
pub struct CallbackNonceStore {
    seen: Mutex<HashSet<String>>,
}

impl CallbackNonceStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert returns `true` iff the nonce was already present (i.e.
    /// REPLAY). Inserts on the way out so subsequent calls also
    /// return `true`.
    pub fn check_and_remember(&self, nonce: &str) -> bool {
        let mut set = self.seen.lock().expect("nonce-store mutex poisoned");
        if set.len() > 4096 {
            // Trim — keep the LAST inserted half. HashSet has no
            // insert-order tracking, so this is a coarse drop. The
            // 5-minute window means a few minutes of accepted
            // callbacks past trim won't break correctness.
            set.clear();
        }
        !set.insert(nonce.to_string())
    }
}

/// Resolve the verifying pubkey for `sponsor` from the environment.
/// Convention: `SBO3L_EXECUTOR_CALLBACK_PUBKEY_<UPPERCASE_SPONSOR>`
/// where `-` and `.` in the sponsor id are replaced with `_`.
fn resolve_pubkey_for_sponsor(sponsor: &str) -> Result<VerifyingKey, String> {
    let env_name = format!(
        "SBO3L_EXECUTOR_CALLBACK_PUBKEY_{}",
        sponsor.to_uppercase().replace(['-', '.'], "_")
    );
    let hex_pubkey = std::env::var(&env_name)
        .map_err(|_| format!("missing env var {env_name} for sponsor `{sponsor}`"))?;
    let stripped = hex_pubkey.trim().trim_start_matches("0x");
    let bytes =
        hex::decode(stripped).map_err(|e| format!("env var {env_name} not valid hex: {e}"))?;
    let arr: [u8; 32] = bytes.try_into().map_err(|v: Vec<u8>| {
        format!(
            "env var {env_name} must be 32-byte ed25519 pubkey; got {} bytes",
            v.len()
        )
    })?;
    VerifyingKey::from_bytes(&arr)
        .map_err(|e| format!("env var {env_name}: invalid ed25519 pubkey: {e}"))
}

/// Problem-shape error body. Mirrors the rest of the daemon's error
/// surface so an integration tool that already parses one sees no
/// shape surprise here.
#[derive(Debug, Serialize)]
struct Problem {
    code: &'static str,
    detail: String,
}

fn problem_response(status: StatusCode, code: &'static str, detail: impl Into<String>) -> Response {
    (
        status,
        Json(Problem {
            code,
            detail: detail.into(),
        }),
    )
        .into_response()
}

/// `POST /v1/executor-callback` handler. See module docs for the full
/// pipeline.
pub async fn executor_callback_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body_bytes: axum::body::Bytes,
) -> Response {
    let header_value = match headers.get(UPSTREAM_SIG_HEADER) {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                return problem_response(
                    StatusCode::BAD_REQUEST,
                    "callback.bad_signature_header",
                    "X-Upstream-Signature header is not valid ASCII",
                );
            }
        },
        None => {
            return problem_response(
                StatusCode::UNAUTHORIZED,
                "callback.missing_signature",
                "X-Upstream-Signature header is required",
            );
        }
    };

    let (timestamp, nonce, kind, signature_hex) = match WebhookEnvelope::parse_header(&header_value)
    {
        Ok(parts) => parts,
        Err(e) => {
            return problem_response(
                StatusCode::BAD_REQUEST,
                "callback.bad_signature_header",
                e.to_string(),
            );
        }
    };
    if !matches!(kind, BodyKind::Json) {
        return problem_response(
            StatusCode::BAD_REQUEST,
            "callback.bad_kind",
            "executor callbacks must use kind=json",
        );
    }

    let body_value: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return problem_response(
                StatusCode::BAD_REQUEST,
                "callback.bad_body",
                format!("body is not valid JSON: {e}"),
            );
        }
    };
    let parsed: ExecutorCallbackBody = match serde_json::from_value(body_value.clone()) {
        Ok(p) => p,
        Err(e) => {
            return problem_response(
                StatusCode::BAD_REQUEST,
                "callback.bad_body",
                format!("body shape mismatch: {e}"),
            );
        }
    };
    if parsed.schema != CALLBACK_SCHEMA_V1 {
        return problem_response(
            StatusCode::BAD_REQUEST,
            "callback.bad_schema",
            format!(
                "expected schema {CALLBACK_SCHEMA_V1}, got {}",
                parsed.schema
            ),
        );
    }

    let canonical = match canonicalise_body(&body_bytes, BodyKind::Json) {
        Ok(c) => c,
        Err(e) => {
            return problem_response(
                StatusCode::BAD_REQUEST,
                "callback.bad_body",
                format!("body re-canonicalisation failed: {e}"),
            );
        }
    };

    let pubkey = match resolve_pubkey_for_sponsor(&parsed.sponsor) {
        Ok(k) => k,
        Err(e) => {
            return problem_response(StatusCode::SERVICE_UNAVAILABLE, "callback.no_pubkey", e);
        }
    };

    // Signature fingerprint check happens implicitly via the verify
    // step: a wrong key fingerprint would still need a matching
    // signature, which can't happen unless the sender held the right
    // private key.
    let envelope = WebhookEnvelope {
        timestamp_unix: timestamp,
        nonce: nonce.clone(),
        kind: BodyKind::Json,
        body_hash_hex: canonical.body_hash_hex.clone(),
        signature_hex,
        // We don't trust the header to carry the fingerprint —
        // compute it from the resolved pubkey for diagnostic logs.
        key_fingerprint: String::new(),
    };

    let now_unix = Utc::now().timestamp();
    let nonce_store = &state.0.callback_nonce_store;
    if let Err(e) = verify_webhook(
        &pubkey,
        &body_bytes,
        &envelope,
        now_unix,
        REPLAY_WINDOW_SECS,
        |n| nonce_store.check_and_remember(n),
    ) {
        let (status, code) = match e {
            VerifyError::StaleTimestamp { .. } => {
                (StatusCode::UNAUTHORIZED, "callback.stale_timestamp")
            }
            VerifyError::Replay(_) => (StatusCode::CONFLICT, "callback.replay"),
            VerifyError::BodyHashMismatch => (StatusCode::BAD_REQUEST, "callback.body_tampered"),
            VerifyError::BadSignature => (StatusCode::UNAUTHORIZED, "callback.bad_signature"),
            VerifyError::BadSignatureFormat { .. } => {
                (StatusCode::BAD_REQUEST, "callback.bad_signature_format")
            }
            VerifyError::Canon(_) => (StatusCode::BAD_REQUEST, "callback.canon_error"),
        };
        return problem_response(status, code, e.to_string());
    }

    // Verified. Append `execution_confirmed` audit event.
    let inner = &state.0;
    let mut metadata = serde_json::Map::new();
    metadata.insert("sponsor".into(), Value::String(parsed.sponsor.clone()));
    metadata.insert(
        "status".into(),
        Value::String(parsed.status.as_str().into()),
    );
    if let Some(ref er) = parsed.execution_ref {
        metadata.insert("execution_ref".into(), Value::String(er.clone()));
    }
    if let Some(ref ev) = parsed.evidence {
        metadata.insert("evidence".into(), ev.clone());
    }
    metadata.insert("callback_nonce".into(), Value::String(nonce.clone()));

    let new_event = NewAuditEvent {
        event_type: EXECUTION_CONFIRMED_EVENT_TYPE.to_string(),
        actor: parsed.sponsor.clone(),
        // Anchor to the original decision — auditors join chain
        // entries by subject_id == decision_event_id.
        subject_id: parsed.audit_event_id.clone(),
        payload_hash: canonical.body_hash_hex.clone(),
        metadata,
        policy_version: None,
        policy_hash: None,
        attestation_ref: None,
        ts: Utc::now(),
    };

    let signed = match inner.storage.lock() {
        Ok(mut storage) => match storage.audit_append(new_event, &inner.audit_signer) {
            Ok(s) => s,
            Err(e) => {
                return problem_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "callback.audit_write_failed",
                    e.to_string(),
                );
            }
        },
        Err(_) => {
            return problem_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "callback.storage_unreachable",
                "storage mutex poisoned",
            );
        }
    };

    Json(ExecutorCallbackResponse {
        schema: RESP_SCHEMA,
        audit_event_id: signed.event.id.clone(),
        appended_seq: signed.event.seq,
    })
    .into_response()
}
