//! Integration tests for `mandate key {init,list,rotate} --mock` (PSM-A1.9).
//!
//! Exercises the real `mandate` binary end-to-end: each test builds a
//! tempfile-backed SQLite db, drives the CLI with `Command::new(cli_bin())`,
//! and asserts on stdout/stderr/exit-status of the actual process.

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mandate"))
}

/// 32-byte deterministic seed used across all CLI integration tests here.
/// **Mock** — never used outside the test suite.
const TEST_ROOT_SEED: &str = "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a";

#[test]
fn init_writes_v1_row_then_list_shows_it() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");

    let init = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "init must succeed; stderr={}",
        String::from_utf8_lossy(&init.stderr)
    );
    let init_out = String::from_utf8_lossy(&init.stdout);
    assert!(
        init_out.contains("mock-kms"),
        "init output must lead with `mock-kms` disclosure; got: {init_out}"
    );

    let list = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db", db.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("audit-mock"),
        "list should show audit-mock; got: {stdout}"
    );
    assert!(
        stdout.contains("audit-mock-v1"),
        "list should show key_id; got: {stdout}"
    );
}

#[test]
fn init_is_idempotent_on_repeat() {
    // Running the same init twice should NOT fail. The second run
    // reports the existing row; both runs exit 0. Catches a regression
    // where the duplicate-row path returned non-zero.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let args = [
        "key",
        "init",
        "--mock",
        "--role",
        "audit-mock",
        "--root-seed",
        TEST_ROOT_SEED,
        "--db",
        db.to_str().unwrap(),
    ];
    let first = Command::new(cli_bin()).args(args).output().unwrap();
    assert!(first.status.success());
    let second = Command::new(cli_bin()).args(args).output().unwrap();
    assert!(
        second.status.success(),
        "second init must be idempotent; stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("already initialised"),
        "expected idempotency message; got: {stdout}"
    );
}

#[test]
fn rotate_advances_version_and_old_version_remains_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");

    // init v1
    let init = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    // rotate → v2
    let rotate = Command::new(cli_bin())
        .args([
            "key",
            "rotate",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        rotate.status.success(),
        "rotate must succeed; stderr={}",
        String::from_utf8_lossy(&rotate.stderr)
    );
    let rotate_out = String::from_utf8_lossy(&rotate.stdout);
    assert!(
        rotate_out.contains("v1 → v2"),
        "rotate should announce version change; got: {rotate_out}"
    );

    // list shows both v1 and v2
    let list = Command::new(cli_bin())
        .args([
            "key",
            "list",
            "--mock",
            "--role",
            "audit-mock",
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("audit-mock-v1"));
    assert!(stdout.contains("audit-mock-v2"));
}

#[test]
fn rotate_without_init_fails_with_clear_message() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let out = Command::new(cli_bin())
        .args([
            "key",
            "rotate",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "rotate without init must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no keyring exists") || stderr.contains("init"),
        "stderr must point at `init`; got: {stderr}"
    );
}

#[test]
fn list_with_no_keyring_prints_friendly_message() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let out = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db", db.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no keyring entries"),
        "should friendly-message; got: {stdout}"
    );
}

#[test]
fn missing_mock_flag_is_required_by_clap() {
    // Without `--mock` clap itself rejects the call (the bool is a
    // required arg via the always-set default semantics; we accept
    // either clap's "required" message or our own runtime "--mock"
    // disclosure check).
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let out = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "init without --mock must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--mock") || stderr.contains("required") || stderr.contains("acknowledge"),
        "stderr should explain --mock; got: {stderr}"
    );
}

#[test]
fn root_seed_with_wrong_length_returns_clear_error() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let out = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            "abcd", // 4 chars
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("64 hex chars"),
        "stderr should explain length; got: {stderr}"
    );
}

#[test]
fn list_filters_by_role_when_given() {
    // Init two roles, list with --role only one, assert only that
    // one's rows appear.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    for role in ["audit-mock", "decision-mock"] {
        let s = Command::new(cli_bin())
            .args([
                "key",
                "init",
                "--mock",
                "--role",
                role,
                "--root-seed",
                TEST_ROOT_SEED,
                "--db",
                db.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(s.success());
    }
    let list = Command::new(cli_bin())
        .args([
            "key",
            "list",
            "--mock",
            "--role",
            "audit-mock",
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("audit-mock-v1"));
    assert!(
        !stdout.contains("decision-mock-v1"),
        "filtered listing must not leak other roles; got: {stdout}"
    );
}
