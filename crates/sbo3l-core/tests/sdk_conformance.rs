//! SDK conformance — Rust runner.
//!
//! Loads `test-corpus/sdk-conformance/manifest.json` and exercises
//! the Rust SDK's structural verifier against every listed fixture.
//! The TS + Py runners (sister tests in their own crates) walk the
//! same manifest and assert the same outcomes — drift between SDKs
//! is the regression mode this catches.
//!
//! Adding a fixture: drop the .json under test-corpus/passport/,
//! append a vector entry to manifest.json, run all three runners.

use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use sbo3l_core::passport::verify_capsule;

#[derive(Debug, Deserialize)]
struct Manifest {
    schema: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    name: String,
    fixture: String,
    schema_version: u32,
    verify_ok: bool,
    /// Optional list of SDK names that currently disagree with
    /// `verify_ok`. The Rust runner is the canonical reference, so
    /// presence of "rust" here means the manifest claim is wrong
    /// (treat as test failure to prompt update). Other entries
    /// (e.g., "python") are honored by their respective runners.
    #[serde(default)]
    known_drift: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    comment: Option<String>,
}

const MANIFEST_SCHEMA: &str = "sbo3l.sdk_conformance_manifest.v1";

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("test-corpus")
}

fn load_manifest() -> Manifest {
    let path = corpus_root().join("sdk-conformance").join("manifest.json");
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read manifest at {}: {e}", path.display());
    });
    serde_json::from_str(&raw).expect("parse manifest")
}

fn load_capsule(rel: &str) -> Value {
    let path = corpus_root().join(rel);
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read fixture {}: {e}", path.display());
    });
    serde_json::from_str(&raw).expect("parse fixture json")
}

#[test]
fn manifest_schema_id_matches_documented_value() {
    let m = load_manifest();
    assert_eq!(
        m.schema, MANIFEST_SCHEMA,
        "manifest.json schema id drifted; the SDK runners parse on this exact string"
    );
    assert!(
        !m.vectors.is_empty(),
        "manifest must have at least one vector"
    );
}

/// One test runs the entire corpus + reports per-vector mismatches
/// in a single failure message — easier to triage than 19 separate
/// failing tests when the manifest evolves.
#[test]
fn rust_sdk_matches_every_manifest_vector() {
    let manifest = load_manifest();
    let mut failures: Vec<String> = Vec::new();

    for vector in &manifest.vectors {
        // Rust is the canonical reference SDK. If the manifest
        // claims Rust drifts on a vector, treat that as a manifest
        // bug — the conformance pin should follow Rust.
        if vector.known_drift.iter().any(|s| s == "rust") {
            failures.push(format!(
                "[{}] manifest lists 'rust' in known_drift — Rust is the reference SDK; \
                 update verify_ok to match Rust's behavior or fix the Rust verifier",
                vector.name
            ));
            continue;
        }
        let capsule = load_capsule(&vector.fixture);
        let result = verify_capsule(&capsule);
        let actual_ok = result.is_ok();
        if actual_ok != vector.verify_ok {
            failures.push(format!(
                "[{}] expected verify_ok={}, got {} (err={})",
                vector.name,
                vector.verify_ok,
                actual_ok,
                result
                    .as_ref()
                    .err()
                    .map_or(String::new(), |e| e.to_string())
            ));
        }
        // Sanity: capsule's `schema` field starts with the expected
        // version prefix.
        let claimed_schema = capsule
            .get("schema")
            .and_then(|s| s.as_str())
            .unwrap_or("<missing>");
        let expected_prefix = format!("sbo3l.passport_capsule.v{}", vector.schema_version);
        // Tampered fixtures may carry a bogus schema id — only assert
        // shape on golden vectors.
        if vector.name.contains("golden") && !claimed_schema.starts_with(&expected_prefix) {
            failures.push(format!(
                "[{}] expected schema starting with {}, got {}",
                vector.name, expected_prefix, claimed_schema
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "SDK conformance manifest mismatch ({} failures):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// Pin the manifest size so an accidental delete (someone clears a
/// fixture during a refactor) shows up here. Update both the
/// manifest and this constant together when adding/removing
/// vectors.
#[test]
fn manifest_vector_count_is_stable() {
    let m = load_manifest();
    assert_eq!(
        m.vectors.len(),
        19,
        "manifest vector count drifted — update test-corpus/sdk-conformance/manifest.json + this assertion together"
    );
}
