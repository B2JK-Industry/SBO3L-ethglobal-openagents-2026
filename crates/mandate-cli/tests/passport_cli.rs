//! Integration tests for `mandate passport verify --path <capsule>` (P1.1).
//!
//! Drives the real `mandate` binary against every fixture in
//! `test-corpus/passport/` and asserts the documented exit-code shape:
//! 0 on golden, 2 on every tampered shape (with the matching error
//! code in stderr).

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mandate"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from("../../test-corpus/passport").join(name)
}

fn run(fixture: &str) -> std::process::Output {
    Command::new(cli_bin())
        .args(["passport", "verify", "--path"])
        .arg(corpus(fixture))
        .output()
        .expect("spawn mandate")
}

#[test]
fn golden_capsule_verifies_exit_zero() {
    let out = run("golden_001_allow_keeperhub_mock.json");
    assert!(
        out.status.success(),
        "golden must verify; stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("structural verify: ok"),
        "stdout missing ok line; got: {stdout}"
    );
    assert!(
        stdout.contains("mandate.passport_capsule.v1"),
        "stdout must surface schema id; got: {stdout}"
    );
}

#[test]
fn missing_file_returns_exit_one() {
    let out = Command::new(cli_bin())
        .args(["passport", "verify", "--path", "/nonexistent/capsule.json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("read"), "got: {stderr}");
}

#[test]
fn invalid_json_returns_exit_one() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("bad.json");
    std::fs::write(&path, b"{not json").unwrap();
    let out = Command::new(cli_bin())
        .args(["passport", "verify", "--path"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("parse"), "got: {stderr}");
}

#[test]
fn deny_with_execution_ref_rejects_with_exit_two() {
    let out = run("tampered_001_deny_with_execution_ref.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.deny_with_execution"),
        "got: {stderr}"
    );
}

#[test]
fn mock_anchor_marked_live_rejects_with_exit_two() {
    let out = run("tampered_002_mock_anchor_marked_live.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

#[test]
fn live_mode_without_evidence_rejects_with_exit_two() {
    let out = run("tampered_003_live_mode_without_evidence.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.live_without_evidence"),
        "got: {stderr}"
    );
}

#[test]
fn request_hash_mismatch_rejects_with_exit_two() {
    let out = run("tampered_004_request_hash_mismatch.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.request_hash_mismatch"),
        "got: {stderr}"
    );
}

#[test]
fn policy_hash_mismatch_rejects_with_exit_two() {
    let out = run("tampered_005_policy_hash_mismatch.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.policy_hash_mismatch"),
        "got: {stderr}"
    );
}

#[test]
fn malformed_checkpoint_rejects_with_exit_two() {
    let out = run("tampered_006_malformed_checkpoint.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

#[test]
fn unknown_field_rejects_with_exit_two() {
    let out = run("tampered_007_unknown_field.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}
