//! Webhook signing + verification primitives.
//!
//! Two complementary surfaces, both on the same `WebhookSigner`
//! / `WebhookVerifier` types:
//!
//! 1. **Outbound** — when SBO3L sends a webhook to a downstream
//!    consumer (KeeperHub, sponsor executors, partner audit
//!    receivers), it signs the body and attaches the signature in
//!    the `X-SBO3L-Signature: sha256=<hex>` header. The receiver
//!    verifies with the published SBO3L signing pubkey. Mirrors
//!    Stripe / GitHub webhook conventions so existing
//!    consumer libraries work.
//!
//! 2. **Inbound** — when SBO3L receives a webhook from an upstream
//!    sponsor (executor callback, KeeperHub workflow ack), it
//!    expects an `X-Upstream-Signature: <hex>` header that the
//!    sender computed over the same canonical body. SBO3L verifies
//!    with the upstream's published pubkey before consuming the
//!    body.
//!
//! # Canonical body
//!
//! - JSON bodies → JCS-canonical bytes (RFC 8785). Same canonicaliser
//!   the request_hash + audit chain use, so a sender can reuse one
//!   signing pipeline across all SBO3L primitives.
//! - Non-JSON bodies → the raw bytes verbatim. Useful for binary
//!   payloads (e.g., capsule attachments) where re-canonicalisation
//!   would be lossy.
//!
//! # Replay protection
//!
//! Every signed envelope binds a `nonce` (16-byte random ULID) and a
//! `timestamp_unix` to the body via the signed string:
//!
//! ```text
//! signed_string = "{timestamp_unix}.{nonce}.{body_hash_hex}"
//! signature = sign(signed_string, signing_key)
//! ```
//!
//! The receiver:
//! 1. Rejects if `|now - timestamp_unix| > 300s` (5-minute window).
//! 2. Rejects if `nonce` was seen in the last 10 minutes (caller-
//!    provided seen-nonce store).
//! 3. Re-canonicalises the body, recomputes the body hash, and
//!    verifies the signature.
//!
//! Step 1 + 2 together prevent capture-and-replay even if the
//! signature is leaked.

use crate::error::{CoreError, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Default replay window — receivers reject envelopes whose
/// `timestamp_unix` is more than this many seconds away from the
/// current time. Five minutes mirrors Stripe's webhook tolerance.
pub const REPLAY_WINDOW_SECS: i64 = 300;

/// Canonical body — the bytes the signature is computed over after
/// the timestamp + nonce binding. Always-rederive-able from the
/// raw body, so a downstream consumer doesn't need to trust an
/// out-of-band hint about which canonicalisation strategy was used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBody {
    /// 32-byte SHA-256 of the canonical bytes, hex-encoded
    /// (lowercase, no `0x` prefix).
    pub body_hash_hex: String,
    /// `"json"` (JCS-canonicalised) or `"bytes"` (raw verbatim).
    /// Carried in the signed envelope so the receiver knows which
    /// strategy to apply on re-derivation.
    pub kind: BodyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    /// JCS-canonicalised JSON. The body MUST parse as valid JSON;
    /// re-canonicalisation makes the signature stable across
    /// whitespace / key-order variations.
    Json,
    /// Raw bytes verbatim. Used for binary attachments where
    /// canonicalisation would be lossy.
    Bytes,
}

/// Compute the canonical body hash. `body` is the over-the-wire
/// payload bytes; `kind` selects the canonicalisation strategy.
pub fn canonicalise_body(body: &[u8], kind: BodyKind) -> Result<CanonicalBody> {
    let canonical_bytes: Vec<u8> = match kind {
        BodyKind::Json => {
            let v: serde_json::Value = serde_json::from_slice(body).map_err(|e| {
                CoreError::Canonicalization(format!("webhook body claimed json but invalid: {e}"))
            })?;
            serde_json_canonicalizer::to_string(&v)
                .map_err(|e| CoreError::Canonicalization(format!("webhook jcs failed: {e}")))?
                .into_bytes()
        }
        BodyKind::Bytes => body.to_vec(),
    };
    let mut hasher = Sha256::new();
    hasher.update(&canonical_bytes);
    let digest = hasher.finalize();
    Ok(CanonicalBody {
        body_hash_hex: hex::encode(digest),
        kind,
    })
}

/// Signed envelope shape. Wire-stable JSON for embedding in
/// protocols where a sidecar header isn't available; the Stripe-style
/// `X-SBO3L-Signature` header carries the equivalent fields
/// concatenated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WebhookEnvelope {
    /// Unix seconds at sign time.
    pub timestamp_unix: i64,
    /// Random nonce — 26-char ULID. The receiver caches recently-
    /// seen nonces to prevent replay even within the timestamp
    /// window.
    pub nonce: String,
    /// `BodyKind` that produced `body_hash_hex`.
    pub kind: BodyKind,
    /// SHA-256 of the canonical body, lowercase hex (no `0x`).
    pub body_hash_hex: String,
    /// Ed25519 signature over `format!("{timestamp_unix}.{nonce}.{body_hash_hex}")`,
    /// hex-encoded (128 hex chars).
    pub signature_hex: String,
    /// Verifying-key fingerprint — first 8 hex chars of
    /// `sha256(verifying_key)`. Useful when the receiver supports
    /// multiple senders + needs to route to the right pubkey.
    pub key_fingerprint: String,
}

impl WebhookEnvelope {
    /// String the signature covers. Public so callers that prefer to
    /// build the envelope by hand can pin it.
    pub fn signed_string(&self) -> String {
        format!(
            "{}.{}.{}",
            self.timestamp_unix, self.nonce, self.body_hash_hex
        )
    }

    /// Stripe-style header value: `sha256=<hex>` after concatenating
    /// the timestamp + nonce + signature. Receivers split on `=` and
    /// `,` to recover the components. Format mirrors Stripe's
    /// `Stripe-Signature: t=<ts>,v1=<sig>` so existing webhook
    /// libraries can be adapted with minimal work.
    pub fn header_value(&self) -> String {
        format!(
            "t={},nonce={},kind={},sha256={}",
            self.timestamp_unix,
            self.nonce,
            match self.kind {
                BodyKind::Json => "json",
                BodyKind::Bytes => "bytes",
            },
            self.signature_hex,
        )
    }

    /// Parse a header value back into an envelope. Body hash is NOT
    /// in the header — the receiver re-derives it from the body.
    /// Returns `(timestamp, nonce, kind, signature_hex)`.
    pub fn parse_header(value: &str) -> Result<(i64, String, BodyKind, String)> {
        let mut t = None;
        let mut nonce = None;
        let mut kind = None;
        let mut sig = None;
        for part in value.split(',') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix("t=") {
                t = Some(rest.parse::<i64>().map_err(|e| {
                    CoreError::Canonicalization(format!("webhook header t= bad: {e}"))
                })?);
            } else if let Some(rest) = part.strip_prefix("nonce=") {
                nonce = Some(rest.to_string());
            } else if let Some(rest) = part.strip_prefix("kind=") {
                kind = Some(match rest {
                    "json" => BodyKind::Json,
                    "bytes" => BodyKind::Bytes,
                    other => {
                        return Err(CoreError::Canonicalization(format!(
                            "webhook header kind= unknown: {other}"
                        )))
                    }
                });
            } else if let Some(rest) = part.strip_prefix("sha256=") {
                sig = Some(rest.to_string());
            }
        }
        Ok((
            t.ok_or_else(|| CoreError::Canonicalization("webhook header missing t=".into()))?,
            nonce.ok_or_else(|| {
                CoreError::Canonicalization("webhook header missing nonce=".into())
            })?,
            kind.ok_or_else(|| CoreError::Canonicalization("webhook header missing kind=".into()))?,
            sig.ok_or_else(|| {
                CoreError::Canonicalization("webhook header missing sha256=".into())
            })?,
        ))
    }
}

fn key_fingerprint(verifying: &VerifyingKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifying.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..4])
}

/// Sign a webhook body. `now_unix` is injected (not pulled from
/// `SystemTime`) so callers can pin determinism in tests + control
/// the clock source in production.
pub fn sign_webhook(
    signing_key: &SigningKey,
    body: &[u8],
    kind: BodyKind,
    nonce: &str,
    now_unix: i64,
) -> Result<WebhookEnvelope> {
    if nonce.is_empty() {
        return Err(CoreError::Canonicalization(
            "webhook nonce must not be empty".into(),
        ));
    }
    let canonical = canonicalise_body(body, kind)?;
    let signed_str = format!("{}.{}.{}", now_unix, nonce, canonical.body_hash_hex);
    let signature: Signature = signing_key.sign(signed_str.as_bytes());
    Ok(WebhookEnvelope {
        timestamp_unix: now_unix,
        nonce: nonce.to_string(),
        kind,
        body_hash_hex: canonical.body_hash_hex,
        signature_hex: hex::encode(signature.to_bytes()),
        key_fingerprint: key_fingerprint(&signing_key.verifying_key()),
    })
}

/// Errors that distinguish recoverable from non-recoverable verify
/// failures. `Replay` is the only one a sender can cure by retrying
/// with a fresh nonce; everything else means the receiver should
/// drop the request.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VerifyError {
    #[error("webhook signature timestamp out of window: now={now} ts={ts} window={window}s")]
    StaleTimestamp { now: i64, ts: i64, window: i64 },
    #[error("webhook nonce {0} replayed")]
    Replay(String),
    #[error("webhook body re-canonicalisation produced different hash")]
    BodyHashMismatch,
    #[error("webhook signature verification failed")]
    BadSignature,
    #[error("webhook signature_hex must be 128 lowercase-hex chars; got {got}")]
    BadSignatureFormat { got: usize },
    #[error("webhook canonicalisation: {0}")]
    Canon(String),
}

/// Verify a webhook body against a signed envelope.
///
/// `seen_nonce_check` returns `true` iff the nonce has already been
/// observed within the replay window. Caller-supplied so the
/// receiver can plug in a Redis / SQLite / in-memory cache of seen
/// nonces.
pub fn verify_webhook(
    verifying_key: &VerifyingKey,
    body: &[u8],
    envelope: &WebhookEnvelope,
    now_unix: i64,
    window_secs: i64,
    seen_nonce_check: impl Fn(&str) -> bool,
) -> std::result::Result<(), VerifyError> {
    // 1. Replay window — reject envelopes whose timestamp is too
    // old or too far in the future.
    let drift = (now_unix - envelope.timestamp_unix).abs();
    if drift > window_secs {
        return Err(VerifyError::StaleTimestamp {
            now: now_unix,
            ts: envelope.timestamp_unix,
            window: window_secs,
        });
    }
    // 2. Nonce dedup — reject if seen.
    if seen_nonce_check(&envelope.nonce) {
        return Err(VerifyError::Replay(envelope.nonce.clone()));
    }
    // 3. Body integrity — re-canonicalise + recompute the hash;
    // mismatch means the body was tampered with.
    let canonical =
        canonicalise_body(body, envelope.kind).map_err(|e| VerifyError::Canon(e.to_string()))?;
    if canonical.body_hash_hex != envelope.body_hash_hex {
        return Err(VerifyError::BodyHashMismatch);
    }
    // 4. Signature — only verify after the cheap checks pass so a
    // flood of malformed envelopes can't burn signature CPU.
    let sig_bytes =
        hex::decode(&envelope.signature_hex).map_err(|_| VerifyError::BadSignatureFormat {
            got: envelope.signature_hex.len(),
        })?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|v: Vec<u8>| VerifyError::BadSignatureFormat { got: v.len() * 2 })?;
    let signature = Signature::from_bytes(&sig_arr);
    let signed_str = format!(
        "{}.{}.{}",
        envelope.timestamp_unix, envelope.nonce, envelope.body_hash_hex
    );
    verifying_key
        .verify(signed_str.as_bytes(), &signature)
        .map_err(|_| VerifyError::BadSignature)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    fn fresh_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    fn no_replay(_: &str) -> bool {
        false
    }

    #[test]
    fn round_trip_signs_and_verifies_against_same_body() {
        let sk = fresh_key();
        let vk = sk.verifying_key();
        let body = br#"{"agent_id":"research-agent-01","value":"100"}"#.as_slice();
        let env = sign_webhook(
            &sk,
            body,
            BodyKind::Json,
            "01HTAWX5K3R8YV9NQB7C6P2DGS",
            1_714_606_800,
        )
        .unwrap();
        verify_webhook(
            &vk,
            body,
            &env,
            1_714_606_800,
            REPLAY_WINDOW_SECS,
            no_replay,
        )
        .unwrap();
    }

    #[test]
    fn byte_flip_in_body_rejected() {
        let sk = fresh_key();
        let vk = sk.verifying_key();
        let body = br#"{"a":1,"b":2}"#.as_slice();
        let env = sign_webhook(&sk, body, BodyKind::Json, "n1", 100).unwrap();
        // Flip a byte — same JSON shape, but value changed
        let tampered = br#"{"a":1,"b":3}"#.as_slice();
        let err = verify_webhook(&vk, tampered, &env, 100, REPLAY_WINDOW_SECS, no_replay)
            .expect_err("byte flip must be rejected");
        // Re-canonicalisation produces a different hash — caught at
        // the body-integrity step before we even reach the signature
        // check. (A different layer of defence — even if the
        // signature somehow validated, the hash mismatch fails first.)
        assert_eq!(err, VerifyError::BodyHashMismatch);
    }

    #[test]
    fn json_body_signature_stable_across_key_order() {
        // Sender canonicalises with one key order, receiver with
        // another. JCS guarantees both produce identical bytes.
        let sk = fresh_key();
        let vk = sk.verifying_key();
        let body_a = br#"{"a":1,"b":2}"#.as_slice();
        let body_b = br#"{"b":2,"a":1}"#.as_slice(); // same JSON, different key order
        let env = sign_webhook(&sk, body_a, BodyKind::Json, "n2", 100).unwrap();
        // Receiver gets body_b on the wire; verification still
        // passes because JCS re-orders + agrees on canonical bytes.
        verify_webhook(&vk, body_b, &env, 100, REPLAY_WINDOW_SECS, no_replay).unwrap();
    }

    #[test]
    fn wrong_pubkey_signature_rejected() {
        let sender_sk = fresh_key();
        let other_sk = fresh_key();
        let other_vk = other_sk.verifying_key();
        let body = br#"{"x":1}"#.as_slice();
        let env = sign_webhook(&sender_sk, body, BodyKind::Json, "n3", 100).unwrap();
        let err = verify_webhook(&other_vk, body, &env, 100, REPLAY_WINDOW_SECS, no_replay)
            .expect_err("verifying with wrong pubkey must reject");
        assert_eq!(err, VerifyError::BadSignature);
    }

    #[test]
    fn timestamp_out_of_window_rejected() {
        let sk = fresh_key();
        let vk = sk.verifying_key();
        let body = br#"{"x":1}"#.as_slice();
        let env = sign_webhook(&sk, body, BodyKind::Json, "n4", 100).unwrap();
        // 1000 seconds later — way outside the 300s window.
        let err = verify_webhook(&vk, body, &env, 1100, 300, no_replay)
            .expect_err("stale timestamp must be rejected");
        assert!(matches!(err, VerifyError::StaleTimestamp { .. }));
    }

    #[test]
    fn replayed_nonce_rejected_even_within_window() {
        let sk = fresh_key();
        let vk = sk.verifying_key();
        let body = br#"{"x":1}"#.as_slice();
        let env = sign_webhook(&sk, body, BodyKind::Json, "n5", 100).unwrap();
        // Replay store always returns true — nonce already seen.
        let err = verify_webhook(&vk, body, &env, 100, REPLAY_WINDOW_SECS, |_| true)
            .expect_err("replayed nonce must be rejected");
        assert_eq!(err, VerifyError::Replay("n5".into()));
    }

    #[test]
    fn header_value_round_trips_via_parse_header() {
        let sk = fresh_key();
        let body = br#"{"x":1}"#.as_slice();
        let env = sign_webhook(&sk, body, BodyKind::Json, "n6", 100).unwrap();
        let header = env.header_value();
        let (t, nonce, kind, sig) = WebhookEnvelope::parse_header(&header).unwrap();
        assert_eq!(t, env.timestamp_unix);
        assert_eq!(nonce, env.nonce);
        assert_eq!(kind, env.kind);
        assert_eq!(sig, env.signature_hex);
    }

    #[test]
    fn raw_bytes_kind_does_not_canonicalise() {
        let sk = fresh_key();
        let vk = sk.verifying_key();
        // Two byte sequences that JCS-canonicalise to the same JSON
        // but ARE different raw bytes. With Bytes kind, the
        // signature is over the verbatim bytes — verifying with a
        // different byte sequence MUST fail.
        let body_a = b"hello\nworld".as_slice();
        let body_b = b"hello world".as_slice(); // different bytes
        let env = sign_webhook(&sk, body_a, BodyKind::Bytes, "n7", 100).unwrap();
        let err = verify_webhook(&vk, body_b, &env, 100, REPLAY_WINDOW_SECS, no_replay)
            .expect_err("byte-mode mismatch must reject");
        assert_eq!(err, VerifyError::BodyHashMismatch);
    }

    #[test]
    fn empty_nonce_rejected_at_sign_time() {
        let sk = fresh_key();
        let body = b"x".as_slice();
        assert!(sign_webhook(&sk, body, BodyKind::Bytes, "", 0).is_err());
    }

    #[test]
    fn malformed_signature_hex_rejected() {
        let sk = fresh_key();
        let vk = sk.verifying_key();
        let body = br#"{"x":1}"#.as_slice();
        let mut env = sign_webhook(&sk, body, BodyKind::Json, "n9", 100).unwrap();
        // Truncate the signature to 100 chars — should fail length
        // check before crypto verify.
        env.signature_hex.truncate(100);
        let err = verify_webhook(&vk, body, &env, 100, REPLAY_WINDOW_SECS, no_replay)
            .expect_err("truncated signature must be rejected");
        assert!(matches!(err, VerifyError::BadSignatureFormat { .. }));
    }

    #[test]
    fn canonical_body_hash_is_deterministic() {
        let body = br#"{"b":2,"a":1}"#.as_slice();
        let h1 = canonicalise_body(body, BodyKind::Json).unwrap();
        let h2 = canonicalise_body(body, BodyKind::Json).unwrap();
        assert_eq!(h1.body_hash_hex, h2.body_hash_hex);
        // 64 hex chars (256 bits).
        assert_eq!(h1.body_hash_hex.len(), 64);
    }

    #[test]
    fn key_fingerprint_is_8_hex_chars() {
        let sk = fresh_key();
        let env = sign_webhook(&sk, b"x".as_slice(), BodyKind::Bytes, "n10", 0).unwrap();
        assert_eq!(env.key_fingerprint.len(), 8);
        assert!(env.key_fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
