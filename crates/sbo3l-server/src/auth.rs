//! Authorization for `POST /v1/payment-requests` (F-1).
//!
//! Two acceptable forms of `Authorization: Bearer <token>`:
//!
//! 1. **Plain bearer**: `<token>` is bcrypt-verified against the hash held in
//!    env `SBO3L_BEARER_TOKEN_HASH`. The hash is the bcrypt-style string
//!    produced by `htpasswd -nbB`, e.g. `$2y$05$...`.
//! 2. **JWT (EdDSA)**: `<token>` is a JWT signed with the Ed25519 private key
//!    whose public key is held in env `SBO3L_JWT_PUBKEY_HEX` (64 hex chars,
//!    32 bytes). The `sub` claim must equal the APRP `agent_id` in the
//!    request body, otherwise the request is rejected with
//!    `auth.agent_id_mismatch`.
//!
//! If no `Authorization` header is sent, the request is rejected with
//! `auth.required` unless `SBO3L_ALLOW_UNAUTHENTICATED=1` is set, which is a
//! development-only bypass advertised at startup with a stderr banner.
//!
//! All errors are RFC 7807-shaped via [`crate::Problem`] and never echo the
//! presented token, hash, or pubkey.

use axum::http::HeaderMap;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;

use crate::{problem, Problem};

/// Auth configuration loaded once at server startup.
///
/// `Default::default()` produces a *production-safe* config with auth
/// **required** and no validators configured — every request that lacks a
/// header is rejected with `auth.required`, every request that supplies one
/// is rejected with `auth.invalid_token`. Use [`AuthConfig::disabled`] for
/// inline tests and dev mode, or [`AuthConfig::from_env`] for the binary.
#[derive(Clone, Debug, Default)]
pub struct AuthConfig {
    /// If true, missing `Authorization` header is accepted. Dev only.
    pub allow_unauthenticated: bool,
    /// bcrypt hash of the expected bearer token (htpasswd-shaped).
    pub bearer_hash: Option<String>,
    /// Ed25519 public key in hex (64 chars) for JWT verification.
    pub jwt_pubkey_hex: Option<String>,
}

impl AuthConfig {
    /// Auth fully disabled: any request flows. Used by inline tests, by
    /// `AppState::new()`, and when `SBO3L_ALLOW_UNAUTHENTICATED=1` is set
    /// without a validator. Never use in production.
    pub fn disabled() -> Self {
        Self {
            allow_unauthenticated: true,
            bearer_hash: None,
            jwt_pubkey_hex: None,
        }
    }

    /// Build from `SBO3L_ALLOW_UNAUTHENTICATED`, `SBO3L_BEARER_TOKEN_HASH`,
    /// `SBO3L_JWT_PUBKEY_HEX`. Empty values are treated as unset.
    pub fn from_env() -> Self {
        let allow = std::env::var("SBO3L_ALLOW_UNAUTHENTICATED")
            .map(|v| v == "1")
            .unwrap_or(false);
        let bearer = std::env::var("SBO3L_BEARER_TOKEN_HASH")
            .ok()
            .filter(|s| !s.is_empty());
        let jwt = std::env::var("SBO3L_JWT_PUBKEY_HEX")
            .ok()
            .filter(|s| !s.is_empty());
        Self {
            allow_unauthenticated: allow,
            bearer_hash: bearer,
            jwt_pubkey_hex: jwt,
        }
    }
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: String,
}

fn err_required() -> Problem {
    problem(
        "auth.required",
        401,
        "Authorization required",
        "missing Authorization header; configure a bearer token or JWT, or \
         set SBO3L_ALLOW_UNAUTHENTICATED=1 for dev mode",
    )
}

fn err_invalid() -> Problem {
    problem(
        "auth.invalid_token",
        401,
        "Invalid authorization token",
        "bearer or JWT validation failed",
    )
}

fn err_agent_mismatch() -> Problem {
    problem(
        "auth.agent_id_mismatch",
        401,
        "JWT subject does not match APRP agent_id",
        "the JWT `sub` claim must equal the APRP `agent_id`",
    )
}

/// Authorize an incoming request. Returns `Ok(())` on success and a populated
/// [`Problem`] on rejection. The token is never echoed back to the caller and
/// never written to logs.
pub fn authorize(cfg: &AuthConfig, headers: &HeaderMap, body: &Value) -> Result<(), Problem> {
    let token = match read_bearer(headers)? {
        Some(t) => t,
        None => {
            return if cfg.allow_unauthenticated {
                Ok(())
            } else {
                Err(err_required())
            };
        }
    };

    if looks_like_jwt(&token) {
        let pubkey_hex = cfg.jwt_pubkey_hex.as_deref().ok_or_else(err_invalid)?;
        let claims = verify_jwt(&token, pubkey_hex)?;
        let agent_id = body
            .get("agent_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if claims.sub != agent_id {
            return Err(err_agent_mismatch());
        }
        Ok(())
    } else {
        let hash = cfg.bearer_hash.as_deref().ok_or_else(err_invalid)?;
        // bcrypt::verify yields Ok(false) on mismatch and Err(_) on a
        // malformed hash; both surface as `auth.invalid_token` so we don't
        // disclose whether the hash itself is the problem.
        if bcrypt::verify(token.as_bytes(), hash).unwrap_or(false) {
            Ok(())
        } else {
            Err(err_invalid())
        }
    }
}

fn read_bearer(headers: &HeaderMap) -> Result<Option<String>, Problem> {
    let raw = match headers.get(axum::http::header::AUTHORIZATION) {
        Some(v) => v,
        None => return Ok(None),
    };
    let s = raw.to_str().map_err(|_| err_invalid())?;
    let token = s
        .strip_prefix("Bearer ")
        .or_else(|| s.strip_prefix("bearer "))
        .ok_or_else(err_invalid)?;
    if token.is_empty() {
        return Err(err_invalid());
    }
    Ok(Some(token.to_string()))
}

/// JWT shape probe: 3 segments separated by `.` and a base64url-encoded
/// header that decodes to a JSON object (every JWT begins with `eyJ`).
fn looks_like_jwt(s: &str) -> bool {
    s.matches('.').count() == 2 && s.starts_with("eyJ")
}

fn verify_jwt(token: &str, pubkey_hex: &str) -> Result<JwtClaims, Problem> {
    let pubkey_bytes = hex::decode(pubkey_hex).map_err(|_| err_invalid())?;
    if pubkey_bytes.len() != 32 {
        return Err(err_invalid());
    }
    // jsonwebtoken 9 forwards the DecodingKey bytes straight to `ring`'s
    // ED25519 verifier (`UnparsedPublicKey::new(&signature::ED25519, ...)`),
    // which expects the **raw 32-byte public key** — not the RFC 8410 SPKI
    // wrapper. The constructor name is misleading; pass raw bytes.
    let key = DecodingKey::from_ed_der(&pubkey_bytes);
    let mut validation = Validation::new(Algorithm::EdDSA);
    // Capability tokens do not always carry exp/iat — clear required claims
    // and disable expiry validation. Production issuers SHOULD set `exp`;
    // when present it is still parsed (just not enforced here).
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    decode::<JwtClaims>(token, &key, &validation)
        .map(|d| d.claims)
        .map_err(|_| err_invalid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn default_is_production_safe_required() {
        let cfg = AuthConfig::default();
        assert!(!cfg.allow_unauthenticated);
        assert!(cfg.bearer_hash.is_none());
        assert!(cfg.jwt_pubkey_hex.is_none());
    }

    #[test]
    fn disabled_allows_no_header() {
        let cfg = AuthConfig::disabled();
        let headers = HeaderMap::new();
        assert!(authorize(&cfg, &headers, &Value::Null).is_ok());
    }

    #[test]
    fn missing_header_with_required_returns_auth_required() {
        let cfg = AuthConfig::default();
        let headers = HeaderMap::new();
        let err = authorize(&cfg, &headers, &Value::Null).unwrap_err();
        assert_eq!(err.status, 401);
        assert_eq!(err.code, "auth.required");
    }

    #[test]
    fn malformed_authorization_header_rejected() {
        let cfg = AuthConfig {
            allow_unauthenticated: false,
            bearer_hash: Some(bcrypt::hash("x", 4).unwrap()),
            jwt_pubkey_hex: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjpwdw=="),
        );
        let err = authorize(&cfg, &headers, &Value::Null).unwrap_err();
        assert_eq!(err.status, 401);
        assert_eq!(err.code, "auth.invalid_token");
    }

    #[test]
    fn looks_like_jwt_distinguishes_plain_bearer() {
        assert!(looks_like_jwt("eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiJ4In0.AAA"));
        assert!(!looks_like_jwt("plainsecrettoken"));
        assert!(!looks_like_jwt("eyJonly_one_dot.AAA"));
    }
}
