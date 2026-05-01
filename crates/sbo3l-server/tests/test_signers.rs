//! F-5 integration tests: Signer trait + factory + DevSigner lockout.
//!
//! These tests exercise the surface of `sbo3l_core::signers::*` from a
//! consumer crate's perspective — same path the daemon's startup code
//! takes. AWS / GCP integration tests are behind feature flags + the
//! nightly matrix (Daniel provides KMS test keys); this PR covers
//! compile-clean across all 4 backends + the DevSigner lockout flow.

use sbo3l_core::signer::{verify_hex, DevSigner};
use sbo3l_core::signers::{signer_from_env, Signer, SignerError};
use std::sync::Mutex;

/// Tests in this file mutate `SBO3L_SIGNER_BACKEND` /
/// `SBO3L_DEV_ONLY_SIGNER` / `SBO3L_AWS_KMS_KEY_ID`. tokio's default
/// test runtime parallelises tests by default; serialising via this
/// mutex prevents env-var clobbering across threads. (Cargo's
/// `--test-threads=1` would also work but a per-suite mutex is more
/// explicit.)
static ENV_GUARD: Mutex<()> = Mutex::new(());

fn with_env<R>(
    backend: Option<&str>,
    dev_only: Option<&str>,
    aws_key: Option<&str>,
    gcp_key: Option<&str>,
    f: impl FnOnce() -> R,
) -> R {
    let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
    let prev_backend = std::env::var("SBO3L_SIGNER_BACKEND").ok();
    let prev_dev = std::env::var("SBO3L_DEV_ONLY_SIGNER").ok();
    let prev_aws = std::env::var("SBO3L_AWS_KMS_KEY_ID").ok();
    let prev_gcp = std::env::var("SBO3L_GCP_KMS_KEY_NAME").ok();
    set_env("SBO3L_SIGNER_BACKEND", backend);
    set_env("SBO3L_DEV_ONLY_SIGNER", dev_only);
    set_env("SBO3L_AWS_KMS_KEY_ID", aws_key);
    set_env("SBO3L_GCP_KMS_KEY_NAME", gcp_key);
    let out = f();
    set_env("SBO3L_SIGNER_BACKEND", prev_backend.as_deref());
    set_env("SBO3L_DEV_ONLY_SIGNER", prev_dev.as_deref());
    set_env("SBO3L_AWS_KMS_KEY_ID", prev_aws.as_deref());
    set_env("SBO3L_GCP_KMS_KEY_NAME", prev_gcp.as_deref());
    out
}

fn set_env(key: &str, value: Option<&str>) {
    // SAFETY: protected by ENV_GUARD across this test suite. The Rust
    // 2024 edition flagged std::env::set_var as unsafe due to multi-
    // threaded races; the mutex makes those races impossible inside
    // this test file. Other tests in the same crate that touch env
    // vars share the same risk and rely on the same convention.
    unsafe {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

// ----------------------- Signer trait basics -----------------------

#[test]
fn signer_trait_round_trip_with_devsigner() {
    // DevSigner directly impls Signer (blanket impl in signers::dev).
    // Pin that the wire format matches the existing verify_hex path.
    let s = DevSigner::from_seed("test-key", [42u8; 32]);
    let msg = b"hello F-5";
    let sig_hex = Signer::sign_hex(&s, msg).expect("sign_hex");
    let pk_hex = Signer::verifying_key_hex(&s).expect("verifying_key_hex");
    assert_eq!(pk_hex, s.verifying_key_hex());
    assert_eq!(Signer::key_id(&s), "test-key");
    verify_hex(&pk_hex, msg, &sig_hex).expect("verify_hex round-trip");
}

#[test]
fn signer_box_dyn_dispatch_works() {
    // The factory returns `Box<dyn Signer>`; check dynamic dispatch
    // produces a verifiable signature.
    let signer: Box<dyn Signer> = with_env(Some("dev"), Some("1"), None, None, || {
        signer_from_env("audit")
    })
    .expect("dev signer constructs when SBO3L_DEV_ONLY_SIGNER=1");
    let msg = b"box dyn";
    let sig_hex = signer.sign_hex(msg).expect("dyn sign_hex");
    let pk_hex = signer.verifying_key_hex().expect("dyn verifying_key_hex");
    verify_hex(&pk_hex, msg, &sig_hex).expect("dyn-Signer signature verifies");
}

// ----------------------- factory: dev backend -----------------------

#[test]
fn factory_dev_without_lockout_flag_returns_dev_only_lockout() {
    let result = with_env(
        Some("dev"),
        None, // <-- SBO3L_DEV_ONLY_SIGNER unset
        None,
        None,
        || signer_from_env("audit"),
    );
    assert!(matches!(result, Err(SignerError::DevOnlyLockout)));
}

#[test]
fn factory_dev_with_lockout_flag_succeeds() {
    let result = with_env(Some("dev"), Some("1"), None, None, || {
        signer_from_env("audit")
    });
    assert!(result.is_ok(), "dev backend with flag should succeed");
}

#[test]
fn factory_dev_with_lockout_flag_other_value_fails() {
    // Only literal "1" engages the override. Anything else stays
    // locked out.
    for sentinel in ["0", "true", "yes", "ENABLED"] {
        let result = with_env(Some("dev"), Some(sentinel), None, None, || {
            signer_from_env("audit")
        });
        assert!(
            matches!(result, Err(SignerError::DevOnlyLockout)),
            "SBO3L_DEV_ONLY_SIGNER={sentinel:?} must NOT engage the override"
        );
    }
}

#[test]
fn factory_default_backend_is_dev_when_env_unset() {
    // SBO3L_SIGNER_BACKEND unset → defaults to dev. Without the
    // lockout flag, that's still DevOnlyLockout. Pin both behaviours.
    let result = with_env(None, None, None, None, || signer_from_env("audit"));
    assert!(matches!(result, Err(SignerError::DevOnlyLockout)));

    let result = with_env(None, Some("1"), None, None, || signer_from_env("audit"));
    assert!(result.is_ok());
}

// ----------------------- factory: KMS backends -----------------------

#[test]
fn factory_unknown_backend_returns_unknown_backend_error() {
    let result = with_env(Some("vault_secret_provider"), None, None, None, || {
        signer_from_env("audit")
    });
    match result {
        Err(SignerError::UnknownBackend(name)) => {
            assert_eq!(name, "vault_secret_provider");
        }
        Err(other) => panic!("expected UnknownBackend, got {other}"),
        Ok(_) => panic!("expected UnknownBackend, got Ok(signer)"),
    }
}

#[cfg(not(feature = "aws_kms"))]
#[test]
fn factory_aws_kms_without_feature_returns_backend_not_compiled() {
    let result = with_env(
        Some("aws_kms"),
        None,
        Some("alias/sbo3l-test"),
        None,
        || signer_from_env("audit"),
    );
    assert!(matches!(
        result,
        Err(SignerError::BackendNotCompiled("aws_kms"))
    ));
}

#[cfg(not(feature = "gcp_kms"))]
#[test]
fn factory_gcp_kms_without_feature_returns_backend_not_compiled() {
    let result = with_env(
        Some("gcp_kms"),
        None,
        None,
        Some("projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1"),
        || signer_from_env("audit"),
    );
    assert!(matches!(
        result,
        Err(SignerError::BackendNotCompiled("gcp_kms"))
    ));
}

#[cfg(feature = "aws_kms")]
#[test]
fn factory_aws_kms_without_key_id_env_returns_missing_env() {
    let result = with_env(
        Some("aws_kms"),
        None,
        None, // <-- key id unset
        None,
        || signer_from_env("audit"),
    );
    match result {
        Err(SignerError::MissingEnv("SBO3L_AWS_KMS_KEY_ID")) => {}
        Err(other) => panic!("expected MissingEnv, got {other}"),
        Ok(_) => panic!("expected MissingEnv, got Ok(signer)"),
    }
}

#[cfg(feature = "aws_kms")]
#[test]
fn factory_aws_kms_with_key_id_constructs_stub_then_sign_returns_kms_error() {
    // The stub constructs successfully when the env is set, but
    // sign_hex returns SignerError::Kms("not yet implemented") until
    // the SDK wiring lands in the nightly task.
    let signer = with_env(
        Some("aws_kms"),
        None,
        Some("alias/sbo3l-test"),
        None,
        || signer_from_env("audit"),
    )
    .expect("aws_kms stub should construct with key id env set");
    assert_eq!(signer.key_id(), "alias/sbo3l-test");
    let res = signer.sign_hex(b"x");
    assert!(matches!(res, Err(SignerError::Kms(_))));
}

// ---------- interop: every backend's signature is Ed25519 wire format ----------

#[test]
fn dev_signer_signature_verifies_with_existing_verify_hex() {
    // Pin "Receipt verification works interchangeably across signers
    // (signature format identical)" by demonstrating that the trait's
    // sign_hex output verifies via the same `verify_hex` the cli/tests
    // use today. KMS backends ship the same wire format on the same
    // contract; once their SDK wiring lands they pass this test
    // identically (the nightly job exercises that).
    let signer: Box<dyn Signer> = with_env(Some("dev"), Some("1"), None, None, || {
        signer_from_env("receipt")
    })
    .unwrap();

    let msg = b"interop check";
    let sig = signer.sign_hex(msg).unwrap();
    let pk = signer.verifying_key_hex().unwrap();
    verify_hex(&pk, msg, &sig).expect("dyn-Signer signature must verify");
}

#[test]
fn role_to_key_id_is_stable_per_role() {
    // Two daemon restarts under the same role+lockout produce the
    // same key_id (and the same verifying key, given deterministic
    // dev seeds). Pin the property because production rotation logic
    // assumes key_id is stable until the operator triggers a change.
    let mk = || -> (String, String) {
        let s = with_env(Some("dev"), Some("1"), None, None, || {
            signer_from_env("audit")
        })
        .unwrap();
        (s.key_id().to_string(), s.verifying_key_hex().unwrap())
    };
    let (k1, v1) = mk();
    let (k2, v2) = mk();
    assert_eq!(k1, k2);
    assert_eq!(v1, v2);
}
