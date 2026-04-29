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
    run_path(corpus(fixture))
}

fn run_path(path: PathBuf) -> std::process::Output {
    Command::new(cli_bin())
        .args(["passport", "verify", "--path"])
        .arg(path)
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
fn live_mode_empty_evidence_rejects_with_exit_two() {
    let out = run("tampered_008_live_mode_empty_evidence.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

#[test]
fn live_mode_with_concrete_evidence_verifies_exit_zero() {
    let mut v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(corpus("golden_001_allow_keeperhub_mock.json")).unwrap(),
    )
    .unwrap();
    let execution = v["execution"].as_object_mut().unwrap();
    execution.insert("mode".into(), serde_json::Value::String("live".into()));
    execution.insert(
        "live_evidence".into(),
        serde_json::json!({
            "transport": "https",
            "response_ref": "keeperhub-execution-01HTAWX5K3R8YV9NQB7C6P2DGS"
        }),
    );

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("live.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

    let out = run_path(path);
    assert!(
        out.status.success(),
        "live capsule with evidence must verify; stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
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
